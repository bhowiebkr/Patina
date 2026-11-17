//! The outline of a node crate: its code with no function bodies (spec 7.1).

use quote::ToTokens;
use syn::{ImplItem, Item, ReturnType, Signature, Stmt, TraitItem, TraitItemFn};

use super::deep;

/// The code of a Rust file with no function bodies: the part of a node crate that the base
/// library sees (7.1). A `const fn` or a function that gives an `impl` type keeps its body,
/// because the body can change a type. The future of an `async fn` stays in the slot library.
pub fn outline(source: &str) -> Result<String, String> {
    deep(|| {
        let mut file = syn::parse_file(source).map_err(|e| format!("the code does not parse: {e}"))?;
        drop_bodies(&mut file.items);
        Ok(file.to_token_stream().to_string())
    })
}

fn drop_bodies(items: &mut [Item]) {
    for item in items {
        match item {
            Item::Fn(f) => drop_body(&f.sig, &mut f.block.stmts),
            Item::Impl(block) => {
                for item in &mut block.items {
                    if let ImplItem::Fn(f) = item {
                        drop_body(&f.sig, &mut f.block.stmts);
                    }
                }
            }
            Item::Trait(block) => {
                for item in &mut block.items {
                    if let TraitItem::Fn(TraitItemFn { sig, default: Some(body), .. }) = item {
                        drop_body(sig, &mut body.stmts);
                    }
                }
            }
            Item::Mod(module) => {
                if let Some((_, items)) = &mut module.content {
                    drop_bodies(items);
                }
            }
            _ => {}
        }
    }
}

fn drop_body(sig: &Signature, stmts: &mut Vec<Stmt>) {
    let gives_impl = match &sig.output {
        ReturnType::Type(_, ty) => ty.to_token_stream().to_string().split(|c: char| !c.is_alphanumeric() && c != '_').any(|w| w == "impl"),
        ReturnType::Default => false,
    };
    if sig.constness.is_none() && !gives_impl {
        stmts.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_outline_keeps_the_types_and_drops_the_bodies() {
        let base = "type V = f64; struct S { a: V } impl S { fn get(&self) -> V { self.a } } \
                    trait T { fn t(&self) -> u8 { 1 } } const fn n() -> usize { 2 } \
                    fn it() -> impl Iterator<Item = u8> { 0..3 } mod m { pub fn f() -> u8 { 4 } } \
                    async fn w() -> u8 { 6 } #[node] pub fn f(v: &V) -> V { *v }";
        let same = |from: &str, to: &str| {
            assert!(base.contains(from), "{from}");
            outline(base).unwrap() == outline(&base.replacen(from, to, 1)).unwrap()
        };
        assert!(same("{ *v }", "{ *v * 2.0 }"));
        assert!(same("{ self.a }", "{ self.a + 1.0 }"));
        assert!(same("{ 1 }", "{ 5 }"));
        assert!(same("{ 4 }", "{ 5 }"));
        assert!(same("{ 6 }", "{ 7 }"));
        assert!(!same("type V = f64", "type V = f32"));
        assert!(!same("a: V", "a: V, b: u8"));
        assert!(!same("{ 2 }", "{ 3 }"));
        assert!(!same("0..3", "0..5"));
        assert!(outline("fn f( {").is_err());
    }
}
