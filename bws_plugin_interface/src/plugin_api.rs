use std::ptr::null;

/// Allows plugins to define their own API
/// for other plugins to use
#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct PluginApi {
    /// Cast the pointer to the VTable the plugin gives in their interface library
    vtable: *const (),
}

impl PluginApi {
    /// Constructs a default `PluginApi` without any vtable - a null ptr
    pub const fn new() -> Self {
        Self { vtable: null() }
    }
    /// Takes a static reference and constructs a `PluginApi` from it
    pub const fn from<T>(inner: &'static T) -> Self {
        Self {
            vtable: inner as *const _ as *const (),
        }
    }
    /// Casts the pointer into a vtable
    ///
    /// It's up to the user to ensure that it's the correct vtable
    ///
    /// Returns `None` if the pointer was null
    pub unsafe fn into_vtable<T>(&self) -> Option<&T> {
        (self.vtable as *const T).as_ref()
    }
}

impl Default for PluginApi {
    fn default() -> Self {
        Self::new()
    }
}

unsafe impl Sync for PluginApi {}
unsafe impl Send for PluginApi {}
