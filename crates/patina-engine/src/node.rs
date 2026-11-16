//! Reads the pins of a node from the `#[node]` function in its source (spec 4.1), or from the
//! `process` method of a `#[node]` struct (4.2). Reads the type of a connection kind from its
//! `#[connection]` function (6.3).

use quote::ToTokens;
use syn::{
    Attribute, FnArg, GenericArgument, GenericParam, Generics, Ident, ImplItem, Item, ItemStruct, LitStr, Meta, Pat, PathArguments, ReturnType, Signature, Type,
    Visibility,
};

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
    /// The type of the value that the wire brings: `ty` without `&` or `&mut`.
    pub value: String,
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
    Audio,
}

impl Domain {
    /// Reads a domain name, as `#[node(domain = ...)]` and the graph file write it.
    pub fn parse(name: &str) -> Result<Domain, String> {
        match name {
            "batch" => Ok(Domain::Batch),
            "frame" => Ok(Domain::Frame),
            "audio" => Ok(Domain::Audio),
            "event" => Err("the event domain comes at milestone 12".to_owned()),
            _ => Err(format!("{name:?} is not a domain. Use batch, frame or audio")),
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Domain::Batch => "batch",
            Domain::Frame => "frame",
            Domain::Audio => "audio",
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
    /// The type parameters of a generic node, in order (4.5). The wires give their types.
    pub generics: Vec<String>,
    pub inputs: Vec<Input>,
    pub output: Output,
    /// The function returns a `Result`. An `Err` stops the node (4.7).
    pub fallible: bool,
    /// The value type of each output pin, in the order of `output_keys`.
    pub output_types: Vec<String>,
    /// The domain of `#[node(domain = ...)]`.
    pub domain: Option<Domain>,
    /// The state struct has `pub fn ui(&mut self, ui: &mut Ui)` (4.3).
    pub ui: bool,
    /// `#[node(full_gui)]`: the GUI has a full size (4.3).
    pub full: bool,
    /// `#[node(color = "#rrggbb")]`: the header color (15.3).
    pub color: Option<String>,
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
    let (domain, full, color) = read_attr(attr)?;
    let (sig, state, generics, ui) = match item {
        Item::Fn(func) if matches!(func.vis, Visibility::Public(_)) => (&func.sig, None, type_params(&func.sig.generics)?, false),
        Item::Fn(func) => return Err(format!("make the node function `{}` pub", func.sig.ident)),
        Item::Struct(s) if !matches!(s.vis, Visibility::Public(_)) => return Err(format!("make the node struct `{}` pub", s.ident)),
        Item::Struct(s) => {
            let generics = type_params(&s.generics)?;
            let (sig, ui) = process(&file, s, &generics)?;
            (sig, Some(s.ident.to_string()), generics, ui)
        }
        _ => return Err("put #[node] on a function (spec 4.1) or a struct (spec 4.2)".to_owned()),
    };
    plain(sig, state.is_none())?;
    if sig.inputs.len() > MAX_ARGS {
        return Err(format!("a node function takes at most {MAX_ARGS} arguments, with `&mut self` (spec 4.1)"));
    }
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
        let ty = type_text(&arg.ty);
        if ty.split(|c: char| !c.is_alphanumeric()).any(|word| word == "impl") {
            return Err(format!("give the input `{key}` a type parameter, not `impl` (spec 4.5)"));
        }
        let (pass, is_str, value) = match &*arg.ty {
            Type::Reference(r) if r.mutability.is_some() => {
                mut_types.push(type_text(&r.elem));
                (Pass::Mut, false, type_text(&r.elem))
            }
            Type::Reference(r) => (Pass::Ref, matches!(&*r.elem, Type::Path(p) if p.path.is_ident("str")), type_text(&r.elem)),
            _ => (Pass::Take, false, ty.clone()),
        };
        if pass == Pass::Mut && is_out_key(&key) {
            return Err(format!("rename the input `{key}`: its output key is used by the return value"));
        }
        inputs.push(Input { key, pass, is_str, ty, value });
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
    if full && !ui {
        return Err("a node with `full_gui` needs `pub fn ui(&mut self, ui: &mut Ui)` (spec 4.3)".to_owned());
    }
    Ok(Node { func: sig.ident.to_string(), state, generics, inputs, output, fallible, output_types, domain, ui, full, color })
}

/// The most arguments of a node call: the cluster calls nodes through tuples (generate.rs).
const MAX_ARGS: usize = 16;

const PROCESS: &str = "`process` takes `&mut self` and then the inputs";

/// The built-in delay node (9.2). Its output is the value of its input in the previous frame.
pub fn delay() -> Node {
    let t = "T".to_owned();
    let input = Input { key: "value".to_owned(), pass: Pass::Take, is_str: false, ty: t.clone(), value: t.clone() };
    let (generics, output_types) = (vec![t.clone()], vec![t]);
    Node { func: "delay".to_owned(), state: None, generics, inputs: vec![input], output: Output::One, fallible: false, output_types, domain: Some(Domain::Frame), ui: false, full: false, color: None }
}

/// The names of the type parameters. A node has no lifetime or const parameters (4.5).
fn type_params(generics: &Generics) -> Result<Vec<String>, String> {
    let name = |param: &GenericParam| match param {
        GenericParam::Type(t) => Ok(t.ident.to_string()),
        _ => Err("a generic node takes only type parameters (spec 4.5)".to_owned()),
    };
    generics.params.iter().map(name).collect()
}

/// Checks that `sig` is a plain safe function. A method of a stateful node has no generics of its own.
fn plain(sig: &Signature, own_generics: bool) -> Result<(), String> {
    if sig.asyncness.is_some() {
        return Err("async nodes come at milestone 12".to_owned());
    }
    if !own_generics && !sig.generics.params.is_empty() {
        return Err(format!("put the type parameters of `{}` on the node struct (spec 4.5)", sig.ident));
    }
    if sig.unsafety.is_some() || sig.variadic.is_some() || sig.abi.is_some() {
        return Err(format!("`{}` must be a plain safe Rust function", sig.ident));
    }
    Ok(())
}

/// Finds the `process` method of stateful node `node`, and checks its `init` method (4.2) and its
/// `ui` method (4.3). The `impl` block names the type parameters as the struct does (4.5).
fn process<'a>(file: &'a syn::File, node: &ItemStruct, generics: &[String]) -> Result<(&'a Signature, bool), String> {
    let name = &node.ident;
    let this = if generics.is_empty() { name.to_string() } else { format!("{name}<{}>", generics.join(", ")) };
    let head = if generics.is_empty() { format!("impl {this}") } else { format!("impl<{}> {this}", generics.join(", ")) };
    let (mut init, mut process, mut ui) = (None, None, None);
    for item in &file.items {
        let Item::Impl(block) = item else { continue };
        if block.trait_.is_some() || type_text(&block.self_ty) != this {
            continue;
        }
        for func in block.items.iter().filter_map(|i| if let ImplItem::Fn(f) = i { Some(f) } else { None }) {
            let slot = match func.sig.ident.to_string().as_str() {
                "init" => &mut init,
                "process" => &mut process,
                "ui" => &mut ui,
                _ => continue,
            };
            if !matches!(func.vis, Visibility::Public(_)) {
                return Err(format!("make `{name}::{}` pub", func.sig.ident));
            }
            *slot = Some(&func.sig);
        }
    }
    let init = init.ok_or(format!("add `pub fn init() -> Self` to `{head}`"))?;
    plain(init, false)?;
    let is_self = |ty: &Type| matches!(&*type_text(ty), "Self") || type_text(ty) == this;
    if !init.inputs.is_empty() || !matches!(&init.output, ReturnType::Type(_, ty) if is_self(ty)) {
        return Err(format!("`{name}::init` takes no inputs and returns `Self`"));
    }
    if let Some(ui) = ui {
        plain(ui, false)?;
        let is_mut = |arg: Option<&FnArg>| matches!(arg, Some(FnArg::Receiver(r)) if r.reference.is_some() && r.mutability.is_some());
        let is_ui = |arg: Option<&FnArg>| matches!(arg, Some(FnArg::Typed(t)) if { let ty = type_text(&t.ty).replace(' ', ""); ty.starts_with("&mut") && (ty == "&mutUi" || ty.ends_with("::Ui")) });
        if ui.inputs.len() != 2 || !is_mut(ui.inputs.first()) || !is_ui(ui.inputs.get(1)) || !matches!(ui.output, ReturnType::Default) {
            return Err(format!("`{name}::ui` takes `&mut self` and `ui: &mut Ui` (spec 4.3)"));
        }
    }
    let process = process.ok_or(format!("add `pub fn process(&mut self, ...)` to `{head}`"))?;
    Ok((process, ui.is_some()))
}

/// Reads `domain = <name>` (9.1), `full_gui` (4.3) and `color = "#rrggbb"` (15.3) of `#[node(...)]`.
fn read_attr(attr: &Attribute) -> Result<(Option<Domain>, bool, Option<String>), String> {
    if matches!(attr.meta, Meta::Path(_)) {
        return Ok((None, false, None));
    }
    let (mut domain, mut full, mut color) = (None, false, None);
    attr.parse_nested_meta(|meta| {
        if meta.path.is_ident("full_gui") {
            full = true;
            return Ok(());
        }
        if meta.path.is_ident("color") {
            let text = meta.value()?.parse::<LitStr>()?.value();
            if !text.strip_prefix('#').is_some_and(|h| h.len() == 6 && h.bytes().all(|b| b.is_ascii_hexdigit())) {
                return Err(meta.error("write the node color as \"#rrggbb\", for example \"#175c59\" (spec 15.3)"));
            }
            color = Some(text.to_ascii_lowercase());
            return Ok(());
        }
        if !meta.path.is_ident("domain") {
            return Err(meta.error("#[node] takes only `domain = <name>`, `full_gui` and `color = \"#rrggbb\"`"));
        }
        let name: Ident = meta.value()?.parse()?;
        domain = Some(Domain::parse(&name.to_string()).map_err(|e| meta.error(e))?);
        Ok(())
    })
    .map_err(|e| e.to_string())?;
    Ok((domain, full, color))
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
            "#[node] pub fn f<'a>(x: &'a u8) {}",
            "#[node] pub fn f<const N: usize>() {}",
            "#[node] pub fn f(x: impl Copy) {}",
            "#[node] pub fn f<T>(x: &impl Into<T>) {}",
            "#[node] pub fn f((a, b): (u8, u8)) {}",
            "#[node] pub fn f(out1: &mut u8) {}",
            "#[node] pub fn f() {} #[node] pub fn g() {}",
            "pub fn f() {}",
            "#[node(domain = event)] pub fn f() {}",
            "#[node(speed = 1)] pub fn f() {}",
        ];
        for source in fails {
            assert!(parse(source).is_err(), "{source}");
        }
        let args = |count: usize| (0..count).map(|i| format!("a{i}: u8")).collect::<Vec<_>>().join(", ");
        assert!(parse(&format!("#[node] pub fn f({}) {{}}", args(MAX_ARGS))).is_ok());
        assert!(parse(&format!("#[node] pub fn f({}) {{}}", args(MAX_ARGS + 1))).is_err());
    }

    #[test]
    fn reads_the_pins_of_a_stateful_node_from_process() {
        let counter = "#[node(domain = frame)] pub struct Counter { count: u64 }\n\
                       impl Counter { pub fn init() -> Self { todo!() } }\n\
                       impl Counter { pub fn process(&mut self, step: u64) -> u64 { todo!() } fn other(&self) {} }";
        let node = parse(counter).unwrap();
        assert_eq!((node.func.as_str(), node.state.as_deref(), node.domain), ("process", Some("Counter"), Some(Domain::Frame)));
        assert_eq!((node.inputs.len(), node.inputs[0].key.as_str(), node.output_keys()), (1, "step", vec!["out".to_owned()]));
        assert!(!node.ui);
        let gui = counter.replace("fn other(&self) {}", "pub fn ui(&mut self, ui: &mut patina_sdk::Ui) {}");
        assert!(parse(&gui).unwrap().ui);
        let full = gui.replace("#[node(domain = frame)]", "#[node(domain = frame, full_gui)]");
        assert!(parse(&full).unwrap().full);
        let colored = counter.replace("#[node(domain = frame)]", "#[node(domain = frame, color = \"#175C59\")]");
        assert_eq!(parse(&colored).unwrap().color.as_deref(), Some("#175c59"));
        assert!(parse(&colored.replace("#175C59", "175c59")).is_err());
        assert!(parse(&colored.replace("#175C59", "#17c5g9")).is_err());
        assert!(parse(&counter.replace("#[node(domain = frame)]", "#[node(full_gui)]")).is_err());
        let hold = "#[node] pub struct Hold<T, U> { v: Option<(T, U)> }\n\
                    impl<T: Clone, U> Hold<T, U> { pub fn init() -> Hold<T, U> { todo!() } }\n\
                    impl<T, U> Hold<T, U> where T: Copy { pub fn process(&mut self, x: T, y: &mut Vec<U>) -> Option<T> { todo!() } }";
        let node = parse(hold).unwrap();
        assert_eq!((node.generics, node.output_types), (vec!["T".to_owned(), "U".to_owned()], vec!["Option<T>".to_owned(), "Vec<U>".to_owned()]));
        let values: Vec<_> = node.inputs.iter().map(|i| i.value.as_str()).collect();
        assert_eq!(values, ["T", "Vec<U>"]);
        let fails = [
            "#[node] struct S; impl S { pub fn init() -> Self { S } pub fn process(&mut self) {} }",
            "#[node] pub struct S<T>(T);",
            "#[node] pub struct S<T>(T); impl<U> S<U> { pub fn init() -> Self { todo!() } pub fn process(&mut self) {} }",
            "#[node] pub struct S<T>(T); impl<T> S<T> { pub fn init() -> Self { todo!() } pub fn process<V>(&mut self, v: V) {} }",
            "#[node] pub struct S<'a>(&'a u8);",
            "#[node] pub struct S; impl S { pub fn process(&mut self) {} }",
            "#[node] pub struct S; impl S { pub fn init() -> Self { S } }",
            "#[node] pub struct S; impl S { pub fn init(x: u8) -> Self { S } pub fn process(&mut self) {} }",
            "#[node] pub struct S; impl S { pub fn init() -> u8 { 0 } pub fn process(&mut self) {} }",
            "#[node] pub struct S; impl S { pub fn init() -> Self { S } pub fn process(&self) {} }",
            "#[node] pub struct S; impl S { pub fn init() -> Self { S } pub fn process() {} }",
            "#[node] pub struct S; impl S { pub fn init() -> Self { S } fn process(&mut self) {} }",
            "#[node] pub enum E { A }",
            "#[node] pub struct S; impl S { pub fn init() -> Self { S } pub fn process(&mut self) {} pub fn ui(&self, ui: &mut Ui) {} }",
            "#[node] pub struct S; impl S { pub fn init() -> Self { S } pub fn process(&mut self) {} pub fn ui(&mut self, ui: &Ui) {} }",
            "#[node] pub struct S; impl S { pub fn init() -> Self { S } pub fn process(&mut self) {} fn ui(&mut self, ui: &mut Ui) {} }",
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
