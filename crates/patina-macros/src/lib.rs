//! The `#[node]` attribute (spec 4.1). The engine reads the node signature with `syn` (4.1), so
//! this macro only prepares the function for the cluster that calls it.

use proc_macro::{Delimiter, TokenStream, TokenTree};

/// Marks the function of a node. The function gets `#[inline]`, so small nodes inline into
/// each other inside one cluster (spec 8.3).
#[proc_macro_attribute]
pub fn node(attr: TokenStream, item: TokenStream) -> TokenStream {
    if !attr.is_empty() {
        return error("#[node] takes no arguments");
    }
    if !is_function(&item) {
        // shortcut: only function nodes, stateful nodes (4.2) come at milestone 5.
        return error("Patina supports only function nodes (spec 4.1) at this time");
    }
    let mut out: TokenStream = "#[inline]".parse().expect("valid attribute");
    out.extend(item);
    out
}

/// Returns true when the item has the `fn` keyword before its body.
fn is_function(item: &TokenStream) -> bool {
    for tree in item.clone() {
        match tree {
            TokenTree::Ident(ident) if ident.to_string() == "fn" => return true,
            TokenTree::Ident(ident) if ["struct", "enum", "impl"].contains(&&*ident.to_string()) => {
                return false;
            }
            TokenTree::Group(group) if group.delimiter() == Delimiter::Brace => return false,
            _ => {}
        }
    }
    false
}

fn error(message: &str) -> TokenStream {
    format!("compile_error!({message:?});").parse().expect("valid compile_error")
}
