//! WESL linking. `prelude.wesl` and each `lib/*.wesl` are modules; a kernel (its file, the
//! node's `code` text and the generated accessors) is the main module. Files carry no imports:
//! each module's import list is generated from the names it uses that another module declares.
//! wesl-rs links them, stripping what the kernel does not reach, and its sourcemap maps naga's
//! errors back to a file and line.

use std::borrow::Cow;
use std::collections::{BTreeMap, BTreeSet};

use wesl::resolver::VirtualResolver;
use wesl::sourcemap::{BasicSourceMap, SourceMap};
use wesl::syntax::{ModulePath, TranslationUnit};
use wesl::{CompileOptions, Compiler};
use wgpu::naga;
use wgsl_parse::SyntaxNode;

use crate::def::DefError;

pub const PRELUDE: &str = "prelude.wesl";
const MAIN: &str = "package::kernel";

/// A shared module: its file under `kernels/`, WESL path, text (generated import line first)
/// and the names it declares.
struct Module {
    file: String,
    path: String,
    text: String,
    /// Length of the import line.
    skip: usize,
    names: BTreeSet<String>,
}

/// The prelude and `lib/` modules with their import lines.
#[derive(Default)]
pub struct Modules(Vec<Module>);

impl Modules {
    /// Reads `prelude.wesl` and every `lib/*.wesl` of `files` (path under `kernels/`, text).
    pub fn new(files: &BTreeMap<String, String>) -> (Self, Vec<DefError>) {
        let mut out = Vec::new();
        let mut errors = Vec::new();
        for (file, text) in files.iter().filter(|(p, _)| is_module(p)) {
            let path = format!(
                "package::{}",
                file.trim_end_matches(".wesl").replace('/', "::")
            );
            match declared(text, file) {
                Ok(names) => out.push(Module {
                    file: file.clone(),
                    path,
                    text: text.clone(),
                    skip: 0,
                    names,
                }),
                Err(e) => errors.push(e),
            }
        }
        let lines: Vec<String> = out
            .iter()
            .map(|m| import_line(&m.text, &m.names, &out, &m.path))
            .collect();
        for (m, line) in out.iter_mut().zip(lines) {
            m.skip = line.len();
            m.text.insert_str(0, &line);
        }
        (Self(out), errors)
    }

    /// The module files, `prelude.wesl` and `lib/*.wesl`.
    pub fn files(&self) -> impl Iterator<Item = &str> {
        self.0.iter().map(|m| m.file.as_str())
    }

    /// Whether both declare the same names in the same files (else any kernel's imports may
    /// change).
    pub fn same_names(&self, other: &Self) -> bool {
        self.0.len() == other.0.len()
            && (self.0.iter().zip(&other.0)).all(|(a, b)| a.file == b.file && a.names == b.names)
    }

    fn by_path(&self, path: &str) -> Option<&Module> {
        self.0.iter().find(|m| m.path == path)
    }
}

/// Whether `path` (under `kernels/`) is a shared module: the prelude or a `lib/` file.
pub fn is_module(path: &str) -> bool {
    path == PRELUDE || (path.starts_with("lib/") && path.ends_with(".wesl"))
}

/// Recipes can retain helper-function revisions while sharing the host's GPU buffers.
/// Compare parsed structs, aliases and resource declarations, including constants
/// referenced by their types. Older bounded stats arrays fit the same host allocation.
pub(crate) fn compatible_prelude(current: &str, incoming: &str) -> Result<(), String> {
    fn interface(source: &str) -> Result<BTreeMap<String, String>, String> {
        use wesl::syntax::{DeclarationKind, GlobalDeclaration};
        let unit: TranslationUnit = source
            .parse()
            .map_err(|e: wgsl_parse::Error| e.to_string())?;
        let mut declarations = BTreeMap::new();
        let mut constants = BTreeMap::new();
        for declaration in &unit.global_declarations {
            if matches!(&**declaration, GlobalDeclaration::Compound(_)) {
                return Err("conditional GPU declarations are unsupported".into());
            }
            let Some(name) = declaration.ident().map(|id| id.name().to_string()) else {
                continue;
            };
            let mut text: String = declaration
                .to_string()
                .chars()
                .filter(|c| !c.is_whitespace())
                .collect();
            let bound = match &**declaration {
                GlobalDeclaration::Struct(_) | GlobalDeclaration::TypeAlias(_) => true,
                GlobalDeclaration::Declaration(value) => match value.kind {
                    DeclarationKind::Var(_) | DeclarationKind::Override => true,
                    DeclarationKind::Const => {
                        constants.insert(name.clone(), text.clone());
                        false
                    }
                    _ => false,
                },
                GlobalDeclaration::Compound(_) => {
                    return Err("conditional GPU declarations are unsupported".into());
                }
                _ => false,
            };
            if bound {
                const STATS: &str =
                    "@group(0)@binding(9)var<storage,read_write>stats:array<atomic<u32>,";
                if name == "stats"
                    && let Some(capacity) =
                        text.strip_prefix(STATS).and_then(|v| v.strip_suffix(">;"))
                {
                    let count: u64 = capacity
                        .trim_end_matches('u')
                        .parse()
                        .map_err(|_| "unsupported stats capacity")?;
                    if count == 0 || count > crate::gpu::STATS_BYTES / 4 {
                        return Err("stats capacity exceeds the host allocation".into());
                    }
                    text = format!("{STATS}host_capacity>;");
                }
                declarations.insert(name, text);
            }
        }
        loop {
            let names: BTreeSet<String> = declarations
                .values()
                .flat_map(|text| {
                    text.split(|c: char| !c.is_ascii_alphanumeric() && c != '_')
                        .map(str::to_owned)
                })
                .collect();
            let additions: Vec<_> = constants
                .iter()
                .filter(|(name, _)| names.contains(*name) && !declarations.contains_key(*name))
                .map(|(name, text)| (name.clone(), text.clone()))
                .collect();
            if additions.is_empty() {
                break;
            }
            declarations.extend(additions);
        }
        Ok(declarations)
    }
    let current = interface(current)?;
    let incoming = interface(incoming)?;
    if current != incoming {
        let changed: BTreeSet<_> = current
            .keys()
            .chain(incoming.keys())
            .filter(|name| current.get(*name) != incoming.get(*name))
            .collect();
        return Err(format!("GPU declarations differ: {changed:?}"));
    }
    Ok(())
}

/// A linked kernel: WGSL for wgpu, the module files it uses, and what errors need to map back.
pub struct Linked {
    pub wgsl: String,
    pub files: BTreeSet<String>,
    map: Option<BasicSourceMap>,
    /// Byte ranges of the kernel file and the `code` text in the main module.
    file: std::ops::Range<usize>,
    code: std::ops::Range<usize>,
}

/// Links a kernel: `source` is its file's text, `code` the node's code text and `generated` the
/// accessors and entry point. Errors name the file and line.
pub fn link(
    modules: &Modules,
    file: &str,
    source: &str,
    code: &str,
    generated: &str,
) -> Result<Linked, String> {
    let body = format!("{source}\n{code}\n{generated}");
    let own = declared(&body, file).map_err(|e| e.to_string())?;
    let line = import_line(&body, &own, &modules.0, MAIN);
    let main: ModulePath = MAIN.parse().map_err(|e| format!("{e}"))?;
    let mut resolver = VirtualResolver::new();
    for m in &modules.0 {
        let path = m.path.parse().map_err(|e| format!("{}: {e}", m.file))?;
        resolver.add_module(path, Cow::Borrowed(m.text.as_str()));
    }
    let start = line.len();
    resolver.add_module(main.clone(), format!("{line}{body}").into());
    let options = CompileOptions {
        validate: false,
        ..Default::default()
    };
    let result = Compiler::new_with_resolver(options, resolver)
        .compile_module(&main)
        .map_err(|e| wesl_error(&e, modules, file))?;
    let files = result
        .modules
        .iter()
        .filter_map(|m| modules.by_path(&m.path.to_string()))
        .map(|m| m.file.clone())
        .collect();
    let code_start = start + source.len() + 1;
    Ok(Linked {
        wgsl: result.to_string(),
        files,
        map: result.sourcemap,
        file: start..start + source.len(),
        code: code_start..code_start + code.len(),
    })
}

impl Linked {
    /// naga's error on the linked WGSL, placed in the file it came from; `None` when naga
    /// accepts it.
    pub fn explain(&self, modules: &Modules, file: &str) -> Option<String> {
        let (msg, at) = naga_check(&self.wgsl).err()?;
        let first = msg
            .lines()
            .next()
            .unwrap_or("")
            .trim_start_matches("error: ");
        let place = match (&self.map, at) {
            (Some(map), Some(at)) => self.locate(at, map, modules, file),
            _ => file.to_owned(),
        };
        Some(format!("{place}: {first}"))
    }

    /// `file:line` of byte `at` of the linked WGSL: the declaration holding it, its module and
    /// item through the sourcemap, then the error's text inside that item.
    fn locate(&self, at: usize, map: &BasicSourceMap, modules: &Modules, file: &str) -> String {
        let out = &self.wgsl;
        let Ok(unit) = out.parse::<TranslationUnit>() else {
            return file.to_owned();
        };
        let Some(mangled) = unit
            .global_declarations
            .iter()
            .find(|d| d.span().start <= at && at < d.span().end)
            .and_then(|d| d.ident())
            .map(|i| i.name().to_string())
        else {
            return file.to_owned();
        };
        let (path, item) = match map.item(&mangled) {
            Some(e) => (e.path.to_string(), e.name.clone()),
            None => (MAIN.to_owned(), mangled),
        };
        let Ok(module) = path.parse::<ModulePath>() else {
            return file.to_owned();
        };
        let Some(source) = map.source(&module) else {
            return file.to_owned();
        };
        let Some(span) = source.parse::<TranslationUnit>().ok().and_then(|u| {
            u.global_declarations
                .iter()
                .find(|d| d.ident().is_some_and(|i| *i.name() == item))
                .map(|d| d.span())
        }) else {
            return format!("{file} in {item}");
        };
        let end = out[at..].find(['\n', ';']).map_or(out.len(), |e| at + e);
        let needle = unmangle(out[at..end].trim(), map);
        let pos = span.start + source[span.start..span.end].find(&needle).unwrap_or(0);
        let place = |from: usize| {
            source
                .get(from..pos)
                .map_or(1, |s| s.matches('\n').count() + 1)
        };
        match modules.by_path(&path) {
            Some(m) => format!("{}:{} in {item}", m.file, place(m.skip)),
            None if self.file.contains(&pos) => {
                format!("{file}:{} in {item}", place(self.file.start))
            }
            None if self.code.contains(&pos) => {
                format!("{file} code line {} in {item}", place(self.code.start))
            }
            None => format!("{file} generated {item}"),
        }
    }
}

/// `text` of the linked WGSL with the linker's mangled names turned back into the source's.
fn unmangle(text: &str, map: &BasicSourceMap) -> String {
    let word = |c: char| c.is_ascii_alphanumeric() || c == '_';
    let mut out = String::new();
    let mut rest = text;
    while let Some(start) = rest.find(|c: char| c.is_ascii_alphabetic() || c == '_') {
        let len = rest[start..]
            .find(|c| !word(c))
            .unwrap_or(rest.len() - start);
        let name = &rest[start..start + len];
        out.push_str(&rest[..start]);
        out.push_str(map.item(name).map_or(name, |e| e.name.as_str()));
        rest = &rest[start + len..];
    }
    out + rest
}

/// A wesl error, placed in its file when it carries a module and span.
fn wesl_error(e: &wesl::Error, modules: &Modules, file: &str) -> String {
    let wesl::Error::Error(d) = e else {
        return format!("{file}: {e}");
    };
    let module = d.detail.module_path.as_ref().map(|p| p.to_string());
    let (name, skip) = match module.as_deref().and_then(|p| modules.by_path(p)) {
        Some(m) => (m.file.as_str(), m.skip),
        None => (file, 0),
    };
    let line = match (&d.detail.source, &d.detail.span) {
        (Some(src), Some(span)) => src
            .get(skip..span.start)
            .map(|s| format!(":{}", s.matches('\n').count() + 1)),
        _ => None,
    };
    format!("{name}{}: {}", line.unwrap_or_default(), d.error)
}

/// The names `src` declares at module scope.
fn declared(src: &str, file: &str) -> Result<BTreeSet<String>, DefError> {
    let unit: TranslationUnit = src.parse().map_err(|e: wgsl_parse::Error| DefError {
        file: file.to_owned(),
        line: src
            .get(..e.span.start)
            .map_or(1, |s| s.matches('\n').count() + 1),
        message: e.error.to_string(),
    })?;
    Ok(unit
        .global_declarations
        .iter()
        .filter_map(|d| d.ident())
        .map(|i| i.name().to_string())
        .collect())
}

/// One line importing every name `text` uses that another module declares (one line, so line
/// numbers shift by exactly one).
fn import_line(text: &str, own: &BTreeSet<String>, modules: &[Module], me: &str) -> String {
    let used = idents(text);
    let mut line = String::new();
    for m in modules.iter().filter(|m| m.path != me) {
        let names: Vec<&str> = m
            .names
            .iter()
            .filter(|n| used.contains(*n) && !own.contains(*n))
            .map(String::as_str)
            .collect();
        if !names.is_empty() {
            line += &format!("import {}::{{{}}}; ", m.path, names.join(", "));
        }
    }
    line.push('\n');
    line
}

/// Identifiers in `src`, skipping comments, numbers, member accesses and attribute names.
fn idents(src: &str) -> BTreeSet<String> {
    let b = src.as_bytes();
    let mut out = BTreeSet::new();
    let mut i = 0;
    while i < b.len() {
        if b[i..].starts_with(b"//") {
            i = src[i..].find('\n').map_or(b.len(), |e| i + e);
        } else if b[i..].starts_with(b"/*") {
            i = src[i + 2..].find("*/").map_or(b.len(), |e| i + 4 + e);
        } else if b[i].is_ascii_alphabetic() || b[i] == b'_' {
            let start = i;
            while i < b.len() && (b[i].is_ascii_alphanumeric() || b[i] == b'_') {
                i += 1;
            }
            let prev = src[..start].trim_end().as_bytes().last();
            if !matches!(prev, Some(b'.' | b'@')) {
                out.insert(src[start..i].to_owned());
            }
        } else if b[i].is_ascii_digit() {
            while i < b.len() && (b[i].is_ascii_alphanumeric() || b[i] == b'_' || b[i] == b'.') {
                i += 1;
            }
        } else {
            i += 1;
        }
    }
    out
}

/// naga's message and the byte offset of its first span.
pub(crate) fn naga_check(src: &str) -> Result<(), (String, Option<usize>)> {
    let module = naga::front::wgsl::parse_str(src).map_err(|e| {
        let at = e.labels().next().and_then(|(s, _)| s.to_range());
        (e.emit_to_string(src), at.map(|r| r.start))
    })?;
    if !module
        .entry_points
        .iter()
        .any(|entry| entry.name == "main" && entry.stage == naga::ShaderStage::Compute)
    {
        return Err(("Missing compute main entry point".into(), None));
    }
    naga::valid::Validator::new(
        naga::valid::ValidationFlags::all(),
        naga::valid::Capabilities::all(),
    )
    .validate(&module)
    .map_err(|e| {
        let at = e.spans().next().and_then(|(s, _)| s.to_range());
        (e.emit_to_string(src), at.map(|r| r.start))
    })?;
    Ok(())
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    fn modules() -> Modules {
        let files = BTreeMap::from([
            (
                PRELUDE.to_owned(),
                "fn base() -> f32 { return helper(); }\n".to_owned(),
            ),
            (
                "lib/extra.wesl".to_owned(),
                "fn helper() -> f32 { return 1.0; }\nfn unused() -> f32 { return 2.0; }\n"
                    .to_owned(),
            ),
        ]);
        let (modules, errors) = Modules::new(&files);
        assert!(errors.is_empty());
        modules
    }

    #[test]
    fn links_only_what_the_kernel_reaches() {
        let m = modules();
        let src = "@compute @workgroup_size(1)\nfn main() { let x = base(); }\n";
        let linked = link(&m, "nodes/a/x.wesl", src, "", "").unwrap();
        assert!(!linked.wgsl.contains("unused"));
        assert_eq!(
            linked.files,
            BTreeSet::from([PRELUDE.to_owned(), "lib/extra.wesl".to_owned()])
        );
        assert_eq!(linked.explain(&m, "nodes/a/x.wesl"), None);
    }

    #[test]
    fn prelude_helpers_and_older_stats_are_compatible_but_gpu_layout_changes_are_not() {
        let current = include_str!("../kernels/prelude.wesl");
        let older = current.replace("array<atomic<u32>, 5120>", "array<atomic<u32>, 1024>");
        assert!(
            compatible_prelude(
                current,
                &format!("{older}\nfn extra_helper() -> f32 {{ return 0.5; }}")
            )
            .is_ok()
        );
        assert!(
            compatible_prelude(
                current,
                &current.replace("size: vec4<u32>", "size: vec4<f32>")
            )
            .is_err()
        );
        assert!(
            compatible_prelude(
                current,
                &current.replace("array<atomic<u32>, 5120>", "array<atomic<u32>, 8192>")
            )
            .is_err()
        );
        let constant = "const N: u32 = 16; struct Info { values: array<vec4<f32>, N>, } @group(0) @binding(0) var<uniform> info: Info;";
        assert!(compatible_prelude(constant, &constant.replace("= 16", "= 15")).is_err());
    }

    #[test]
    fn errors_map_to_file_and_line() {
        let m = modules();
        let src = "// x\n@compute @workgroup_size(1)\nfn main() {\n    let y: u32 = base();\n}\n";
        let linked = link(&m, "nodes/a/x.wesl", src, "", "").unwrap();
        let e = linked.explain(&m, "nodes/a/x.wesl").unwrap();
        assert!(e.starts_with("nodes/a/x.wesl:4 in main"), "{e}");
    }
}
