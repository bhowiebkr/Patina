//! Reads the pins of a node from the `#[node]` function in its source (spec 4.1), or from the
//! `process` method of a `#[node]` struct (4.2). Reads the type of a connection kind from its
//! `#[connection]` function (6.3).

use quote::ToTokens;
use syn::{Attribute, FnArg, Ident, ImplItem, Meta, GenericArgument, Item, Pat, PathArguments, ReturnType, Signature, Type, Visibility};

/// How the node function takes an input.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Pass {
    /// `x: &T`
    Ref,
    /// `x: T`
    Take,
    /// `x: &mut T`. The input also gives an output with the same key (4.4).
    Mut,
}

#[derive(Debug)]
pub struct Input {
    pub key: String,
    pub pass: Pass,
    /// `x: &str`. Its parameter slot holds a `String`.
    pub is_str: bool,
    /// The type as the code writes it, for example `&str`.
    pub ty: String,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Output {
    None,
    One,
    Tuple(usize),
}

/// The clock domain of a node (9.1).
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub enum Domain {
    Batch,
    Frame,
}

impl Domain {
    /// Reads a domain name, as `#[node(domain = ...)]` and the graph file write it.
    pub fn parse(name: &str) -> Result<Domain, String> {
        match name {
            "batch" => Ok(Domain::Batch),
            "frame" => Ok(Domain::Frame),
            "audio" => Err("the audio domain comes at milestone 11".to_owned()),
            "event" => Err("the event domain comes at milestone 12".to_owned()),
            _ => Err(format!("{name:?} is not a domain. Use batch or frame")),
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Domain::Batch => "batch",
            Domain::Frame => "frame",
        }
    }
}

/// The signature of one node function.
#[derive(Debug)]
pub struct Node {
    /// The node function, or `process` for a stateful node.
    pub func: String,
    /// The name of the state struct of a stateful node (4.2).
    pub state: Option<String>,
    pub inputs: Vec<Input>,
    pub output: Output,
    /// The function returns a `Result`. An `Err` stops the node (4.7).
    pub fallible: bool,
    /// The value type of each output pin, in the order of `output_keys`.
    pub output_types: Vec<String>,
    /// The domain of `#[node(domain = ...)]`.
    pub domain: Option<Domain>,
}

impl Node {
    /// The output pin keys, in order: the return value, then each `&mut` input (6.4).
    pub fn output_keys(&self) -> Vec<String> {
        let mut keys = match self.output {
            Output::None => Vec::new(),
            Output::One => vec!["out".to_owned()],
            Output::Tuple(n) => (0..n).map(|i| format!("out{i}")).collect(),
        };
        let muts = self.inputs.iter().filter(|i| i.pass == Pass::Mut);
        keys.extend(muts.map(|i| i.key.clone()));
        keys
    }
}

/// Finds the one `#[node]` item of a node crate and reads its pins.
pub fn parse(source: &str) -> Result<Node, String> {
    let file = syn::parse_file(source).map_err(|e| format!("the code does not parse: {e}"))?;
    let mut found = None;
    for item in &file.items {
        let attrs = match item {
            Item::Fn(f) => &f.attrs,
            Item::Struct(s) => &s.attrs,
            Item::Enum(e) => &e.attrs,
            Item::Impl(i) => &i.attrs,
            _ => continue,
        };
        if let Some(attr) = attrs.iter().find(|a| named(a, "node"))
            && found.replace((attr, item)).is_some()
        {
            return Err("the crate has more than one #[node] item".to_owned());
        }
    }
    let (attr, item) = found.ok_or("the crate has no #[node] function or struct")?;
    let domain = read_domain(attr)?;
    let (sig, state) = match item {
        Item::Fn(func) if matches!(func.vis, Visibility::Public(_)) => (&func.sig, None),
        Item::Fn(func) => return Err(format!("make the node function `{}` pub", func.sig.ident)),
        Item::Struct(s) if !matches!(s.vis, Visibility::Public(_)) => return Err(format!("make the node struct `{}` pub", s.ident)),
        // shortcut: no generic nodes, they come in a later slice of milestone 5 (4.5).
        Item::Struct(s) if !s.generics.params.is_empty() => return Err(GENERIC.to_owned()),
        Item::Struct(s) => (process(&file, &s.ident)?, Some(s.ident.to_string())),
        _ => return Err("put #[node] on a function (spec 4.1) or a struct (spec 4.2)".to_owned()),
    };
    plain(sig)?;
    let mut inputs = Vec::new();
    let mut mut_types = Vec::new();
    for (i, arg) in sig.inputs.iter().enumerate() {
        let arg = match arg {
            FnArg::Typed(arg) => arg,
            FnArg::Receiver(r) if i == 0 && state.is_some() && matches!(&*r.ty, Type::Reference(t) if t.mutability.is_some()) => continue,
            _ if state.is_some() => return Err(PROCESS.to_owned()),
            _ => return Err("a node function takes no self".to_owned()),
        };
        let Pat::Ident(pat) = &*arg.pat else {
            return Err("give each input of the node a plain name".to_owned());
        };
        let key = pat.ident.to_string();
        let (pass, is_str) = match &*arg.ty {
            Type::Reference(r) if r.mutability.is_some() => {
                mut_types.push(type_text(&r.elem));
                (Pass::Mut, false)
            }
            Type::Reference(r) => (Pass::Ref, matches!(&*r.elem, Type::Path(p) if p.path.is_ident("str"))),
            Type::ImplTrait(_) => return Err(GENERIC.to_owned()),
            _ => (Pass::Take, false),
        };
        if pass == Pass::Mut && is_out_key(&key) {
            return Err(format!("rename the input `{key}`: its output key is used by the return value"));
        }
        inputs.push(Input { key, pass, is_str, ty: type_text(&arg.ty) });
    }
    let (value, fallible) = match &sig.output {
        ReturnType::Default => (None, false),
        ReturnType::Type(_, ty) => match result_value(ty) {
            Some(value) => (Some(value), true),
            None => (Some(&**ty), false),
        },
    };
    let (output, mut output_types) = value.map_or((Output::None, Vec::new()), shape);
    output_types.extend(mut_types);
    if state.is_some() && !matches!(sig.inputs.first(), Some(FnArg::Receiver(_))) {
        return Err(PROCESS.to_owned());
    }
    Ok(Node { func: sig.ident.to_string(), state, inputs, output, fallible, output_types, domain })
}

const GENERIC: &str = "Patina does not support generic nodes (spec 4.5) at this time";
const PROCESS: &str = "`process` takes `&mut self` and then the inputs";

fn plain(sig: &Signature) -> Result<(), String> {
    if sig.asyncness.is_some() {
        return Err("async nodes come at milestone 12".to_owned());
    }
    if !sig.generics.params.is_empty() {
        return Err(GENERIC.to_owned());
    }
    if sig.unsafety.is_some() || sig.variadic.is_some() || sig.abi.is_some() {
        return Err(format!("`{}` must be a plain safe Rust function", sig.ident));
    }
    Ok(())
}

/// Finds the `process` method of stateful node `name`, and checks its `init` method (4.2).
fn process<'a>(file: &'a syn::File, name: &Ident) -> Result<&'a Signature, String> {
    let (mut init, mut process) = (None, None);
    for item in &file.items {
        let Item::Impl(block) = item else { continue };
        if block.trait_.is_some() || !matches!(&*block.self_ty, Type::Path(p) if p.path.is_ident(name)) {
            continue;
        }
        for func in block.items.iter().filter_map(|i| if let ImplItem::Fn(f) = i { Some(f) } else { None }) {
            let slot = match func.sig.ident.to_string().as_str() {
                "init" => &mut init,
                "process" => &mut process,
                _ => continue,
            };
            if !matches!(func.vis, Visibility::Public(_)) {
                return Err(format!("make `{name}::{}` pub", func.sig.ident));
            }
            *slot = Some(&func.sig);
        }
    }
    let init = init.ok_or(format!("add `pub fn init() -> Self` to `impl {name}`"))?;
    plain(init)?;
    let is_self = |ty: &Type| matches!(ty, Type::Path(p) if p.path.is_ident("Self") || p.path.is_ident(name));
    if !init.inputs.is_empty() || !matches!(&init.output, ReturnType::Type(_, ty) if is_self(ty)) {
        return Err(format!("`{name}::init` takes no inputs and returns `Self`"));
    }
    process.ok_or(format!("add `pub fn process(&mut self, ...)` to `impl {name}`"))
}

/// Reads `domain = <name>` of `#[node(...)]` (9.1).
fn read_domain(attr: &Attribute) -> Result<Option<Domain>, String> {
    if matches!(attr.meta, Meta::Path(_)) {
        return Ok(None);
    }
    let mut domain = None;
    attr.parse_nested_meta(|meta| {
        if !meta.path.is_ident("domain") {
            return Err(meta.error("#[node] takes only `domain = <name>`"));
        }
        let name: Ident = meta.value()?.parse()?;
        domain = Some(Domain::parse(&name.to_string()).map_err(|e| meta.error(e))?);
        Ok(())
    })
    .map_err(|e| e.to_string())?;
    Ok(domain)
}

/// The function of a connection kind (6.3).
#[derive(Debug)]
pub struct Kind {
    pub func: String,
    /// The type of the values that the kind shows, as the code writes it, for example `u8`.
    pub ty: String,
}

/// Finds the one `#[connection]` function of a connection kind crate and reads its value type.
pub fn parse_kind(source: &str) -> Result<Kind, String> {
    let file = syn::parse_file(source).map_err(|e| format!("the code does not parse: {e}"))?;
    let mut found = None;
    for item in &file.items {
        if let Item::Fn(func) = item
            && marked(&func.attrs, "connection")
            && found.replace(func).is_some()
        {
            return Err("the crate has more than one #[connection] function".to_owned());
        }
    }
    let func = found.ok_or("the crate has no #[connection] function")?;
    let sig = &func.sig;
    if !matches!(func.vis, Visibility::Public(_)) {
        return Err(format!("make the connection function `{}` pub", sig.ident));
    }
    let plain = sig.asyncness.is_none() && sig.unsafety.is_none() && sig.abi.is_none() && sig.variadic.is_none();
    if !plain || !sig.generics.params.is_empty() {
        return Err("a connection function must be a plain safe Rust function with no generics".to_owned());
    }
    let mut args = sig.inputs.iter();
    if let (Some(FnArg::Typed(arg)), None) = (args.next(), args.next())
        && let Type::Reference(r) = &*arg.ty
        && r.mutability.is_none()
    {
        return Ok(Kind { func: sig.ident.to_string(), ty: type_text(&r.elem) });
    }
    Err("a connection function takes one input, a `&T` of the type that it shows".to_owned())
}

/// True when an attribute has the name `name`, also as the last part of a path.
fn marked(attrs: &[Attribute], name: &str) -> bool {
    attrs.iter().any(|a| named(a, name))
}

fn named(attr: &Attribute, name: &str) -> bool {
    attr.path().segments.last().is_some_and(|s| s.ident == name)
}

fn is_out_key(key: &str) -> bool {
    key.strip_prefix("out").is_some_and(|n| n.bytes().all(|b| b.is_ascii_digit()))
}

/// The value type of a `Result<T, ..>` return type. The check reads the type name only.
fn result_value(ty: &Type) -> Option<&Type> {
    let Type::Path(path) = ty else { return None };
    let last = path.path.segments.last()?;
    let PathArguments::AngleBracketed(args) = &last.arguments else { return None };
    match args.args.first()? {
        GenericArgument::Type(value) if last.ident == "Result" => Some(value),
        _ => None,
    }
}

/// The output pins of a return value type, and the type of each pin.
fn shape(ty: &Type) -> (Output, Vec<String>) {
    match ty {
        Type::Tuple(t) if t.elems.is_empty() => (Output::None, Vec::new()),
        Type::Tuple(t) => (Output::Tuple(t.elems.len()), t.elems.iter().map(type_text).collect()),
        Type::Paren(p) => shape(&p.elem),
        _ => (Output::One, vec![type_text(ty)]),
    }
}

/// The text of a type in the usual Rust form, for example `Vec<u8>` or `(f32, f32)`.
fn type_text(ty: &Type) -> String {
    let raw = ty.to_token_stream().to_string();
    let chars: Vec<char> = raw.chars().collect();
    let word = |c: char| c.is_alphanumeric() || c == '_';
    let mut text = String::with_capacity(raw.len());
    for (i, &c) in chars.iter().enumerate() {
        // The token text has a space between each two tokens. Keep it between two words and after a comma.
        let keep = c != ' '
            || i > 0
                && (matches!(chars[i - 1], ',' | ';')
                    || word(chars[i - 1]) && chars.get(i + 1).is_some_and(|&n| word(n)));
        if keep {
            text.push(c);
        }
    }
    text
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_pins_and_refuses_unsupported_forms() {
        let node = parse(
            "use patina_sdk::node;\n#[node]\npub fn mix(a: &str, b: f64, c: &mut Vec<u8>) \
             -> std::io::Result<(f32, f32)> { todo!() }",
        )
        .unwrap();
        let passes: Vec<_> = node.inputs.iter().map(|i| (i.key.as_str(), i.pass, i.is_str)).collect();
        assert_eq!(passes, [("a", Pass::Ref, true), ("b", Pass::Take, false), ("c", Pass::Mut, false)]);
        assert!(node.fallible);
        assert_eq!(node.output_keys(), ["out0", "out1", "c"]);
        let types: Vec<_> = node.inputs.iter().map(|i| i.ty.as_str()).collect();
        assert_eq!(types, ["&str", "f64", "&mut Vec<u8>"]);
        assert_eq!(node.output_types, ["f32", "f32", "Vec<u8>"]);
        let node = parse("#[node] pub fn f(m: std::collections::HashMap<String, [u8; 4]>) -> &'static str { todo!() }").unwrap();
        assert_eq!(node.inputs[0].ty, "std::collections::HashMap<String, [u8; 4]>");
        assert_eq!(node.output_types, ["&'static str"]);
        assert_eq!((node.domain, parse("#[node(domain = frame)] pub fn f() {}").unwrap().domain), (None, Some(Domain::Frame)));

        let fails = [
            "#[node] fn f() {}",
            "#[node] pub async fn f() {}",
            "#[node] pub fn f<T>(x: T) {}",
            "#[node] pub fn f(x: impl Copy) {}",
            "#[node] pub fn f((a, b): (u8, u8)) {}",
            "#[node] pub fn f(out1: &mut u8) {}",
            "#[node] pub fn f() {} #[node] pub fn g() {}",
            "pub fn f() {}",
            "#[node(domain = audio)] pub fn f() {}",
            "#[node(speed = 1)] pub fn f() {}",
        ];
        for source in fails {
            assert!(parse(source).is_err(), "{source}");
        }
    }

    #[test]
    fn reads_the_pins_of_a_stateful_node_from_process() {
        let counter = "#[node(domain = frame)] pub struct Counter { count: u64 }\n\
                       impl Counter { pub fn init() -> Self { todo!() } }\n\
                       impl Counter { pub fn process(&mut self, step: u64) -> u64 { todo!() } fn other(&self) {} }";
        let node = parse(counter).unwrap();
        assert_eq!((node.func.as_str(), node.state.as_deref(), node.domain), ("process", Some("Counter"), Some(Domain::Frame)));
        assert_eq!((node.inputs.len(), node.inputs[0].key.as_str(), node.output_keys()), (1, "step", vec!["out".to_owned()]));
        let fails = [
            "#[node] struct S; impl S { pub fn init() -> Self { S } pub fn process(&mut self) {} }",
            "#[node] pub struct S<T>(T);",
            "#[node] pub struct S; impl S { pub fn process(&mut self) {} }",
            "#[node] pub struct S; impl S { pub fn init() -> Self { S } }",
            "#[node] pub struct S; impl S { pub fn init(x: u8) -> Self { S } pub fn process(&mut self) {} }",
            "#[node] pub struct S; impl S { pub fn init() -> u8 { 0 } pub fn process(&mut self) {} }",
            "#[node] pub struct S; impl S { pub fn init() -> Self { S } pub fn process(&self) {} }",
            "#[node] pub struct S; impl S { pub fn init() -> Self { S } pub fn process() {} }",
            "#[node] pub struct S; impl S { pub fn init() -> Self { S } fn process(&mut self) {} }",
            "#[node] pub enum E { A }",
        ];
        for source in fails {
            assert!(parse(source).is_err(), "{source}");
        }
    }

    #[test]
    fn reads_the_type_of_a_connection_kind() {
        let kind = parse_kind("use patina_sdk::*;\n#[connection]\npub fn bus(v: &Vec<u8>) -> Lanes { todo!() }").unwrap();
        assert_eq!((kind.func.as_str(), kind.ty.as_str()), ("bus", "Vec<u8>"));
        let fails = [
            "#[connection] fn f(v: &u8) {}",
            "#[connection] pub fn f<'a>(v: &'a u8) {}",
            "#[connection] pub fn f(v: u8) {}",
            "#[connection] pub fn f(v: &mut u8) {}",
            "#[connection] pub fn f(v: &u8, w: &u8) {}",
            "#[connection] pub fn f(v: &u8) {} #[connection] pub fn g(v: &u8) {}",
            "#[node] pub fn f(v: &u8) {}",
        ];
        for source in fails {
            assert!(parse_kind(source).is_err(), "{source}");
        }
    }
}
