//! Reads the type of a connection kind from its `#[connection]` function (spec 6.3).

use syn::{Attribute, FnArg, Item, LitInt, LitStr, Meta, Token, Type, Visibility, punctuated::Punctuated};

use super::{deep, marked, named, type_text};

/// The function of a connection kind (6.3).
#[derive(Debug, Clone)]
pub struct Kind {
    pub func: String,
    /// The type of the values that the kind shows, as the code writes it, for example `u8`.
    pub ty: String,
    /// The lane names of `lanes = ...`, lane 0 first, or none (6.3).
    pub lanes: Vec<String>,
    pub label: Option<String>,
    /// The color family of the lanes.
    pub color: Option<String>,
}

const FAMILIES: [&str; 5] = ["green", "teal", "violet", "pink", "copper"];

/// Reads `lanes = <count>` or `lanes = ["name", ...]`, `label = "..."` and `color = "<family>"` of
/// `#[connection(...)]` (6.3).
fn read_kind_attr(attr: &Attribute, kind: &mut Kind) -> syn::Result<()> {
    if matches!(attr.meta, Meta::Path(_)) {
        return Ok(());
    }
    attr.parse_nested_meta(|meta| {
        let value = meta.value()?;
        if meta.path.is_ident("lanes") {
            kind.lanes = if value.peek(LitInt) {
                let count: usize = value.parse::<LitInt>()?.base10_parse()?;
                (0..count).rev().map(|bit| bit.to_string()).collect()
            } else {
                let list;
                syn::bracketed!(list in value);
                let names = Punctuated::<LitStr, Token![,]>::parse_terminated(&list)?;
                let names: Vec<String> = names.iter().map(LitStr::value).collect();
                if let Some(bad) = names.iter().find(|n| n.is_empty() || n.len() > 16 || n.chars().any(char::is_control)) {
                    return Err(meta.error(format!("the lane name {bad:?} is empty, longer than 16 bytes or has a control character")));
                }
                names
            };
            if !(1..=64).contains(&kind.lanes.len()) {
                return Err(meta.error("a connection kind has 1 to 64 lanes"));
            }
        } else if meta.path.is_ident("label") {
            let label = value.parse::<LitStr>()?.value();
            if label.is_empty() || label.len() > 64 {
                return Err(meta.error("the label has 1 to 64 bytes"));
            }
            kind.label = Some(label);
        } else if meta.path.is_ident("color") {
            let color = value.parse::<LitStr>()?.value();
            if !FAMILIES.contains(&color.as_str()) {
                return Err(meta.error(format!("the color is one of {}", FAMILIES.join(", "))));
            }
            kind.color = Some(color);
        } else {
            return Err(meta.error("#[connection] takes only `lanes`, `label` and `color`"));
        }
        Ok(())
    })
}

/// Finds the one `#[connection]` function of a connection kind crate and reads its value type.
pub fn parse_kind(source: &str) -> Result<Kind, String> {
    deep(|| kind_of(source))
}

fn kind_of(source: &str) -> Result<Kind, String> {
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
        let mut kind = Kind { func: sig.ident.to_string(), ty: type_text(&r.elem), lanes: Vec::new(), label: None, color: None };
        for attr in func.attrs.iter().filter(|a| named(a, "connection")) {
            read_kind_attr(attr, &mut kind).map_err(|e| format!("#[connection]: {e}"))?;
        }
        return Ok(kind);
    }
    Err("a connection function takes one input, a `&T` of the type that it shows".to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

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
            "#[connection(lanes = 0)] pub fn f(v: &u8) {}",
            "#[connection(lanes = 65)] pub fn f(v: &u8) {}",
            "#[connection(lanes = [\"\"])] pub fn f(v: &u8) {}",
            "#[connection(lanes = [\"a name of 17 bytes\"])] pub fn f(v: &u8) {}",
            "#[connection(color = \"red\")] pub fn f(v: &u8) {}",
            "#[connection(label = \"\")] pub fn f(v: &u8) {}",
            "#[connection(size = 3)] pub fn f(v: &u8) {}",
        ];
        for source in fails {
            assert!(parse_kind(source).is_err(), "{source}");
        }
    }

    #[test]
    fn reads_the_lanes_label_and_color_of_a_kind() {
        let kind = parse_kind("#[connection(lanes = 4, label = \"data\", color = \"teal\")] pub fn f(v: &u8) -> Lanes { todo!() }").unwrap();
        assert_eq!((kind.lanes, kind.label.as_deref(), kind.color.as_deref()), (vec!["3".into(), "2".into(), "1".into(), "0".into()], Some("data"), Some("teal")));
        let kind = parse_kind("#[connection(lanes = [\"HLT\", \"MI\"])] pub fn f(v: &u16) -> Lanes { todo!() }").unwrap();
        assert_eq!(kind.lanes, ["HLT", "MI"]);
    }
}
