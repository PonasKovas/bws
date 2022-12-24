use std::ptr::null;

/// Allows plugins to define their own API
/// for other plugins to use
#[repr(C)]
pub struct PluginApi {
    /// Cast the pointer to the VTable the plugin gives in their interface library
    ptr: *const (),
}

impl PluginApi {
    /// Constructs a default `PluginApi` without any vtable - a null ptr
    pub const fn new() -> Self {
        Self { ptr: null() }
    }
    /// Boxes the given vtable and constructs `PluginApi` from it
    pub fn from<T>(inner: T) -> Self {
        Self {
            ptr: Box::into_raw(Box::new(inner)) as *mut (),
        }
    }
    /// Casts the pointer into a vtable
    ///
    /// It's up to the user to ensure that it's the correct vtable
    ///
    /// Returns `None` if the pointer was null
    pub unsafe fn into<T>(&self) -> Option<&T> {
        (self.ptr as *const T).as_ref()
    }
}

impl Default for PluginApi {
    fn default() -> Self {
        Self::new()
    }
}

unsafe impl Sync for PluginApi {}
unsafe impl Send for PluginApi {}
