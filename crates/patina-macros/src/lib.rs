//! The `#[node]` and `#[connection]` attributes (spec 4.1, 6.3). The engine reads the signatures
//! with `syn`, so these macros only check the item and prepare it for the cluster.

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

/// Marks the function of a connection kind (6.3). The function takes `&T` and returns
/// `patina_sdk::Lanes`. The editor calls it for each live value of type `T`.
#[proc_macro_attribute]
pub fn connection(attr: TokenStream, item: TokenStream) -> TokenStream {
    if !attr.is_empty() {
        return error("#[connection] takes no arguments");
    }
    if !is_function(&item) {
        return error("a connection kind is a function (spec 6.3)");
    }
    item
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
