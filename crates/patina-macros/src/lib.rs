//! The `#[node]`, `#[connection]` and `#[view]` attributes (spec 4.1, 6.3, 15.8). The engine reads the signatures
//! with `syn`, so these macros only check the item and prepare it for the cluster.

use proc_macro::{Delimiter, TokenStream, TokenTree};

/// Marks the function of a node, or the state struct of a stateful node (spec 4.2). A function
/// gets `#[inline]`, so small nodes inline into each other inside one cluster (8.3).
/// `#[node(domain = frame)]` sets the clock domain (9.1), and `#[node(full_gui)]` gives the GUI a
/// full size (4.3). The engine reads these arguments.
#[proc_macro_attribute]
pub fn node(attr: TokenStream, item: TokenStream) -> TokenStream {
    let args: Vec<String> = attr.into_iter().map(|t| t.to_string()).collect();
    if !node_args(&args) {
        return error("#[node] takes only `domain = batch`, `frame`, `audio` or `event`, and `full_gui`");
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

/// True when the tokens of `#[node(...)]` are a list of `domain = <name>` and `full_gui`.
fn node_args(args: &[String]) -> bool {
    let domains = ["batch", "frame", "audio", "event"];
    args.is_empty()
        || args.split(|a| a == ",").all(|arg| match arg {
            [full] => full == "full_gui",
            [key, eq, name] => key == "domain" && eq == "=" && domains.contains(&name.as_str()),
            _ => false,
        })
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

/// Marks the state struct of a view (15.8). The struct has `pub fn init() -> Self` and
/// `pub fn ui(&mut self, ui: &mut Ui, graph: &mut Graph)`. The attribute writes the exports of the
/// view library, so a view crate has one view.
#[proc_macro_attribute]
pub fn view(attr: TokenStream, item: TokenStream) -> TokenStream {
    if !attr.is_empty() {
        return error("#[view] takes no arguments");
    }
    let Some(name) = struct_name(item.clone().into_iter().map(|t| t.to_string())) else {
        return error("put #[view] on a struct with no generic parameters (spec 15.8)");
    };
    let exports = VIEW_EXPORTS.replace("NAME", &name).parse::<TokenStream>().expect("valid exports");
    let mut out = item;
    out.extend(exports);
    out
}

const VIEW_EXPORTS: &str = "
#[doc(hidden)]
#[unsafe(no_mangle)]
pub extern \"C\" fn patina_abi_version() -> u32 { ::patina_sdk::glue::ABI_VERSION }
#[doc(hidden)]
#[unsafe(no_mangle)]
pub unsafe extern \"C\" fn patina_view_host(share: ::patina_sdk::glue::ShareFn, root: *const u8, len: usize) {
    // SAFETY: the engine gives `len` bytes at `root`.
    unsafe { ::patina_sdk::glue::set_host(share, root, len) }
}
#[doc(hidden)]
#[unsafe(no_mangle)]
pub extern \"C\" fn patina_view_new() -> *mut ::core::ffi::c_void { ::patina_sdk::glue::view_new(NAME::init) }
#[doc(hidden)]
#[unsafe(no_mangle)]
pub unsafe extern \"C\" fn patina_view_ui(view: *mut ::core::ffi::c_void, mode: u32, events: *const u8, len: usize, graph: *const u8, graph_len: usize, rep: *const ::patina_sdk::glue::Reporter, reader: *const ::patina_sdk::glue::Reader) -> bool {
    // SAFETY: the engine keeps the rules of `view_ui`.
    unsafe { ::patina_sdk::glue::view_ui(view, mode, (events, len), (graph, graph_len), rep, reader, NAME::ui) }
}
#[doc(hidden)]
#[unsafe(no_mangle)]
pub unsafe extern \"C\" fn patina_view_free(view: *mut ::core::ffi::c_void) {
    // SAFETY: the engine keeps the rules of `view_free`.
    unsafe { ::patina_sdk::glue::view_free::<NAME>(view) }
}
";

/// The name of the struct of the item, from the texts of its tokens. None when the item is not a
/// struct or has generic parameters.
fn struct_name(mut tokens: impl Iterator<Item = String>) -> Option<String> {
    tokens.find(|t| t == "struct")?;
    let name = tokens.next().filter(|n| n.chars().all(|c| c.is_alphanumeric() || c == '_'))?;
    (tokens.next().as_deref() != Some("<")).then_some(name)
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

#[cfg(test)]
mod tests {
    #[test]
    fn node_takes_a_domain_and_full_gui() {
        let args = |text: &str| super::node_args(&text.split(' ').filter(|t| !t.is_empty()).map(str::to_owned).collect::<Vec<_>>());
        assert!(args("") && args("full_gui") && args("domain = frame , full_gui"));
        assert!(!args("domain = gpu") && !args("full") && !args("full_gui ,") && !args("domain frame"));
    }

    #[test]
    fn view_needs_a_plain_struct() {
        let name = |text: &str| super::struct_name(text.split(' ').map(str::to_owned));
        assert_eq!(name("#[derive(Default)] pub struct Timeline {zoom:f32}").as_deref(), Some("Timeline"));
        assert_eq!(name("pub struct Plot < T > {values:T}"), None);
        assert_eq!(name("pub fn plot() {}"), None);
    }
}
