use crate::safe_types::SStr;
use bws_plugin_api::PluginApi;
use std::ptr::NonNull;

/// Allows plugins to define their own API for other plugins to use
#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct PluginApiPtr {
    vtable: NonNull<()>,
    // from PluginApi trait
    pkg_name: SStr<'static>,
    pkg_version: SStr<'static>,
}

impl PluginApiPtr {
    /// Converts a `PluginApi` reference to to a `PluginApiPtr`
    pub const fn new<T: PluginApi>(plugin_api: &'static T) -> Self {
        Self {
            vtable: unsafe { NonNull::new_unchecked(plugin_api as *const _ as *mut ()) },
            pkg_name: SStr::from_str(<T as PluginApi>::PKG_NAME),
            pkg_version: SStr::from_str(<T as PluginApi>::PKG_VERSION),
        }
    }
    /// Converts the pointer to a static reference to the plugin API vtable
    ///
    /// # Safety
    ///
    /// Might result in UB if `PluginApi` is implemented incorrectly
    /// or the plugin interface crate does not use semantic versioning correctly.
    ///
    /// For most cases should be safe.
    ///
    /// # Panic
    ///
    /// Will panic if attempted to cast to an incompatible type.
    pub unsafe fn get<T: PluginApi>(&self) -> &'static T {
        // check if T is compatible with the original
        assert_eq!(
            <T as PluginApi>::PKG_NAME,
            self.pkg_name.into_str(),
            "plugin api pkg names dont match"
        );

        fn split(version: &str) -> Option<(u64, u64, u64)> {
            let mut s = version.split('.');

            let result = (
                s.next()?.parse().ok()?,
                s.next()?.parse().ok()?,
                s.next()?.parse().ok()?,
            );

            // make sure theres no fourth .
            if s.next().is_some() {
                return None;
            }

            Some(result)
        }

        let (my_major, my_minor, _my_patch) =
            split(<T as PluginApi>::PKG_VERSION).expect("incorrect PKG_VERSION format");
        let (og_major, og_minor, _og_patch) =
            split(self.pkg_version.into_str()).expect("incorrect PKG_VERSION format");

        if og_major == 0 {
            assert_eq!(
                <T as PluginApi>::PKG_VERSION,
                self.pkg_version.into_str(),
                "plugin api pkg versions are not compatible"
            );
        } else {
            assert!(
                my_major == og_major && my_minor <= og_minor,
                "plugin api pkg versions are not compatible"
            );
        }

        unsafe { self.vtable.cast::<T>().as_ref() }
    }
}

unsafe impl Sync for PluginApiPtr {}
unsafe impl Send for PluginApiPtr {}
