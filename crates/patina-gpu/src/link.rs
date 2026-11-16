//! The WESL linker and the WESL package of Patina (10.1, 10.3).

use std::borrow::Cow;
use std::path::Path;

use wesl::error::ResolveError;
use wesl::resolver::{PackageResolver, Resolver};
use wesl::sourcemap::SourceMap;
use wesl::syntax::{CompoundStatement, GlobalDeclaration, ModulePath, PathOrigin, Statement, StatementNode};
use wgpu::naga;

static IMAGE: wesl::package::StaticPackageModule =
    wesl::package::StaticPackageModule { name: "image", source: include_str!("shaders/image.wesl"), submodules: &[] };
static ROOT: wesl::package::StaticPackageModule =
    wesl::package::StaticPackageModule { name: "patina_gpu", source: "", submodules: &[&IMAGE] };

/// The WESL package of Patina. A node shader imports it as `patina_gpu::image` (10.3).
pub static PACKAGE: wesl::package::StaticPackage =
    wesl::package::StaticPackage { crate_name: "patina_gpu", root: &ROOT, dependencies: &[] };

/// Links a WESL file and the files that it imports into one WGSL module (10.1). The features set
/// the `@if` conditions. A feature that is not in the list is false. The file can import the
/// package [`PACKAGE`]. The error has the WESL file and the line of the statement or declaration.
pub fn link(file: &Path, features: &[(&str, bool)]) -> Result<String, String> {
    let main = file.file_name().map_or_else(String::new, |n| n.to_string_lossy().into_owned());
    finish(wesl::Compiler::new(options(features)).compile(file), &main)
}

/// Links WESL source text, for example the output of another node (10.3). The source can import
/// only packages, for example [`PACKAGE`]. The error gives `name` in place of a file name.
pub fn link_source(name: &str, source: &str, features: &[(&str, bool)]) -> Result<String, String> {
    let mut packages = PackageResolver::new();
    packages.add_package(&PACKAGE);
    let compiler = wesl::Compiler::new_with_resolver(options(features), Text { source, packages });
    finish(compiler.compile_module(&ModulePath::new_root()), name)
}

fn options(features: &[(&str, bool)]) -> wesl::CompileOptions {
    let mut options = wesl::CompileOptions::default();
    options.dependencies.push(&PACKAGE);
    for (name, on) in features {
        options.features.set(name, *on);
    }
    options
}

/// The linked WGSL, after a check with `naga`.
fn finish(linked: Result<wesl::CompileResult, wesl::Error>, main: &str) -> Result<String, String> {
    let linked = linked.map_err(|e| e.to_string())?;
    let wgsl = linked.to_string();
    match validate(&wgsl) {
        Ok(()) => Ok(wgsl),
        Err((line, message)) => Err(locate(&linked, main, &wgsl, line, &message)),
    }
}

/// Source text as the root module, and the packages.
struct Text<'a> {
    source: &'a str,
    packages: PackageResolver,
}

impl Resolver for Text<'_> {
    fn resolve_source<'b>(&'b self, path: &ModulePath) -> Result<Cow<'b, str>, ResolveError> {
        match path.origin {
            PathOrigin::Package(_) => self.packages.resolve_source(path),
            _ if path.components.is_empty() => Ok(Cow::Borrowed(self.source)),
            _ => Err(ResolveError::ModuleNotFound(path.clone(), "shader source text can import only packages".into())),
        }
    }
}

/// The line and the text of the first error that `naga` finds in the WGSL.
fn validate(wgsl: &str) -> Result<(), (usize, String)> {
    let line = |l: Option<naga::SourceLocation>| l.map_or(0, |l| l.line_number as usize);
    let module = naga::front::wgsl::parse_str(wgsl).map_err(|e| (line(e.location(wgsl)), e.message().to_string()))?;
    // All capabilities, because `Gpu::shader` checks the capabilities of the device.
    let mut validator = naga::valid::Validator::new(naga::valid::ValidationFlags::all(), naga::valid::Capabilities::all());
    validator.validate(&module).map_err(|e| (line(e.location(wgsl)), e.into_inner().to_string()))?;
    Ok(())
}

/// The error text with the WESL file and the line of the statement or declaration that has the
/// WGSL line.
fn locate(linked: &wesl::CompileResult, main: &str, wgsl: &str, line: usize, message: &str) -> String {
    let lines: Vec<&str> = wgsl.lines().take(line).collect();
    let place = lines.iter().rposition(|l| declared(l).is_some()).and_then(|start| {
        let name = declared(lines[start])?;
        let map = linked.sourcemap.as_ref()?;
        let (source, shown, item) = match map.item(name) {
            Some(entry) => {
                let f = map.file(&entry.path)?;
                let shown = f.path.as_ref().and_then(|p| p.file_name()).map(|n| n.to_string_lossy().into_owned());
                (f.source.as_str(), shown.or_else(|| f.display_name.clone())?, entry.name.as_str())
            }
            None => (map.default_source()?, main.to_string(), name),
        };
        let declaration = source.lines().position(|l| declared(l) == Some(item))? + 1;
        let at = statement(linked, name, &lines[start..]).and_then(|at| Some(source.get(..at)?.matches('\n').count() + 1));
        let number = at.filter(|&n| n >= declaration).unwrap_or(declaration);
        Some(format!("{shown} line {number}, in `{item}`"))
    });
    match place {
        Some(place) => format!("{place}: {message}"),
        None => format!("{main}: line {line} of the linked WGSL: {message}"),
    }
}

/// The byte offset in the WESL source of the statement on the last of `lines`. The first of
/// `lines` declares the function `name`. Equal lines count in the order of the text.
fn statement(linked: &wesl::CompileResult, name: &str, lines: &[&str]) -> Option<usize> {
    let text = lines.last()?.trim();
    let nth = lines[1..].iter().filter(|l| l.trim() == text).count().checked_sub(1)?;
    let body = linked.syntax.global_declarations.iter().find_map(|d| match &**d {
        GlobalDeclaration::Function(f) if *f.ident.name() == name => Some(&f.body),
        _ => None,
    })?;
    let mut all = Vec::new();
    statements(body, &mut all);
    let span = all.into_iter().filter(|s| s.to_string().lines().next().map(str::trim) == Some(text)).nth(nth)?.span();
    // A statement that a pass of the linker made has no span.
    (span.end > 0).then_some(span.start)
}

/// The statements of a body and of its inner bodies, in the order of the text.
fn statements<'a>(body: &'a CompoundStatement, out: &mut Vec<&'a StatementNode>) {
    for node in &body.statements {
        out.push(node);
        let inner: Vec<&CompoundStatement> = match &**node {
            Statement::Compound(c) => vec![c],
            Statement::If(s) => std::iter::once(&s.if_clause.body)
                .chain(s.else_if_clauses.iter().map(|c| &c.body))
                .chain(s.else_clause.iter().map(|c| &c.body))
                .collect(),
            Statement::Switch(s) => s.clauses.iter().map(|c| &c.body).collect(),
            Statement::Loop(s) => std::iter::once(&s.body).chain(s.continuing.iter().map(|c| &c.body)).collect(),
            Statement::For(s) => vec![&s.body],
            Statement::While(s) => vec![&s.body],
            _ => Vec::new(),
        };
        for body in inner {
            statements(body, out);
        }
    }
}

/// The name that a line declares at the top level, for example `shade` in
/// `@if(invert) fn shade(t: f32) -> vec3f {`.
pub(crate) fn declared(line: &str) -> Option<&str> {
    let mut rest = line;
    while let Some(attribute) = rest.strip_prefix('@') {
        let end = attribute.find(|c: char| !(c.is_alphanumeric() || c == '_')).unwrap_or(attribute.len());
        rest = &attribute[end..];
        if rest.starts_with('(') {
            rest = &rest[rest.find(')')? + 1..];
        }
        rest = rest.trim_start();
    }
    let rest = rest.strip_prefix("public ").unwrap_or(rest);
    let (keyword, rest) = rest.split_once([' ', '<'])?;
    if !["fn", "const", "var", "struct", "alias", "override"].contains(&keyword) {
        return None;
    }
    let rest = if keyword == "var" && line.contains("var<") { rest.split_once('>')?.1 } else { rest };
    let rest = rest.trim_start();
    let end = rest.find(|c: char| !(c.is_alphanumeric() || c == '_')).unwrap_or(rest.len());
    (end > 0).then(|| &rest[..end])
}
