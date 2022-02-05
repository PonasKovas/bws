use super::GState;
use crate::BwsPlugin;
use abi_stable::{
    external_types::RRwLock,
    std_types::{RSlice, RStr, RString, Tuple2},
};
use repr_c_types::std::SArcOpaque;
use std::path::Path;

#[repr(C)]
pub struct Plugin {
    /// path to the file which provides this plugin
    pub path: RString,
    /// Basically only here to make sure that Library stays in memory
    /// as long as this struct exists
    pub library: SArcOpaque,
    // Valid as long as the library above is valid
    // exposed through a method to make sure the reference doesn't outlive the struct
    root: &'static BwsPlugin,
    // Whether the plugin is enabled
    enabled: RRwLock<bool>,
}

impl Plugin {
    /// Constructs a new Plugin [For internal use only]
    pub fn new(path: RString, library: SArcOpaque, root: &'static BwsPlugin) -> Self {
        Self {
            path,
            library,
            root,
            enabled: RRwLock::new(false),
        }
    }
    /// Returns None if the plugin is not enabled
    pub fn root<'a>(&'a self) -> Option<&'a BwsPlugin> {
        if !*self.enabled.read() {
            return None;
        }
        Some(self.root)
    }
    pub fn name(&self) -> RStr {
        self.root.name
    }
    pub fn version(&self) -> RStr {
        self.root.version
    }
    pub fn dependencies(&self) -> RSlice<Tuple2<RStr, RStr>> {
        self.root.dependencies
    }
    /// Whether the plugin is enabled
    pub fn enabled(&self) -> bool {
        *self.enabled.read()
    }
    /// Enables the plugin. Returns Err if was already enabled
    /// otherwise Ok
    pub fn enable(&self, gstate: &GState) -> Result<(), ()> {
        if *self.enabled.read() {
            return Err(());
        }

        *self.enabled.write() = true;
        (self.root.enable)(gstate);

        Ok(())
    }
    /// Disables the plugin. Returns Err if was already disabled
    /// otherwise Ok
    pub fn disable(&self, gstate: &GState) -> Result<(), ()> {
        if !*self.enabled.read() {
            return Err(());
        }

        *self.enabled.write() = false;
        (self.root.disable)(gstate);

        Ok(())
    }
}
