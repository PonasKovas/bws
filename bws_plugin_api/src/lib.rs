/// A trait for marking Plugin API types
///
/// Use `#[derive(PluginApi)]` to implement
pub trait PluginApi {
    // the name and version of the package defining the API
    // these are used to make sure other plugins cast the API vtable pointer
    // to a compatible API vtable struct
    //
    // This whole trait is only useful if semantic versioning is used correctly
    // on the plugin interface crates.
    //
    // <major>.<minor>.<patch>
    // - bump patch for fully compatible internal changes or bug fixes,
    // - bump minor for forward-compatible, but backwards-incompatible changes
    // (for example introducing new functionality without affecting the existing API)
    // - bump major for forward-incompatible changes (otherwise known as breaking changes)
    //
    // for more info search for "semver spec" on the internet lol
    #[doc(hidden)]
    const PKG_NAME: &'static str;
    #[doc(hidden)]
    const PKG_VERSION: &'static str;
}

#[cfg(feature = "macro")]
pub use bws_plugin_api_derive::PluginApi;
