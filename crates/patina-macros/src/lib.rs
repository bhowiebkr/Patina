//! The `#[node]` and `#[connection]` attributes (spec 4.1, 6.3). The engine reads the signatures
//! with `syn`, so these macros only check the item and prepare it for the cluster.

use proc_macro::{Delimiter, TokenStream, TokenTree};

/// Marks the function of a node, or the state struct of a stateful node (spec 4.2). A function
/// gets `#[inline]`, so small nodes inline into each other inside one cluster (8.3).
/// `#[node(domain = frame)]` sets the clock domain (9.1). The engine reads the domain.
#[proc_macro_attribute]
pub fn node(attr: TokenStream, item: TokenStream) -> TokenStream {
    let args: Vec<String> = attr.into_iter().map(|t| t.to_string()).collect();
    let domains = ["batch", "frame", "audio", "event"];
    if !(args.is_empty() || args.len() == 3 && args[0] == "domain" && args[1] == "=" && domains.contains(&&*args[2])) {
        return error("#[node] takes only `domain = batch`, `frame`, `audio` or `event`");
    }
    match keyword(&item).as_deref() {
        Some("fn") => {}
        Some("struct") => return item,
        _ => return error("put #[node] on a function (spec 4.1) or a struct (spec 4.2)"),
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
    if keyword(&item).as_deref() != Some("fn") {
        return error("a connection kind is a function (spec 6.3)");
    }
    item
}

/// The first item keyword of the item, for example `fn` or `struct`.
fn keyword(item: &TokenStream) -> Option<String> {
    for tree in item.clone() {
        match tree {
            TokenTree::Ident(ident) if ["fn", "struct", "enum", "impl", "trait", "mod"].contains(&&*ident.to_string()) => {
                return Some(ident.to_string());
            }
            TokenTree::Group(group) if group.delimiter() == Delimiter::Brace => return None,
            _ => {}
        }
    }
    None
}

fn error(message: &str) -> TokenStream {
    format!("compile_error!({message:?});").parse().expect("valid compile_error")
}
