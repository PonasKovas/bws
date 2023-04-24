use quote::quote;
use syn::{parse_macro_input, DeriveInput};

/// Implements `PluginApi` for a type
///
/// Uses the package name and version for safety checks, so make sure to use semantic versioning correctly
/// in your `Cargo.toml`. Not doing so will most likely result in UB at some point somewhere.
///
/// # Usage
///
/// ```
/// # use bws_plugin_api_derive::PluginApi;
/// # use safe_types::MaybePanicked;
/// #[repr(C)]
/// #[derive(PluginApi)]
/// pub struct MyPluginApi {
///     pub some_function: extern "C" fn(arg: usize) -> MaybePanicked<bool>,
/// }
/// ```
#[proc_macro_derive(PluginApi)]
pub fn derive_plugin_api(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let input = parse_macro_input!(input as DeriveInput);

    let name = input.ident;

    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();

    let pkg_name = std::env::var("CARGO_PKG_NAME").unwrap();
    let pkg_version = std::env::var("CARGO_PKG_VERSION").unwrap();

    let expanded = quote! {
        impl #impl_generics ::bws_plugin_interface::PluginApi for #name #ty_generics #where_clause {
            const PKG_NAME: &'static str = #pkg_name;
            const PKG_VERSION: &'static str = #pkg_version;
        }
    };

    proc_macro::TokenStream::from(expanded)
}
