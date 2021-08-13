pub mod plugins;

use crate::*;
use plugins::BwsPlugins;
use std::{intrinsics::transmute, marker::PhantomData};

#[repr(C)]
pub struct GlobalStateVTable {
    pub drop: unsafe extern "C" fn(*const ()),
    pub get_compression_treshold: unsafe extern "C" fn(*const ()) -> i32,
    pub get_port: unsafe extern "C" fn(*const ()) -> u16,
    pub get_plugins: unsafe extern "C" fn(*const ()) -> Tuple2<*const (), plugins::PluginsVTable>,
}

/// Wrapper of an Arc pointer to `InnerGlobalState`.
pub struct BwsGlobalState {
    pointer: *const (),
    vtable: GlobalStateVTable,
}

impl Drop for BwsGlobalState {
    fn drop(&mut self) {
        unsafe {
            (self.vtable.drop)(self.pointer);
        }
    }
}

unsafe impl Sync for BwsGlobalState {}
unsafe impl Send for BwsGlobalState {}

impl BwsGlobalState {
    pub unsafe fn new(pointer: *const (), vtable: GlobalStateVTable) -> Self {
        Self { pointer, vtable }
    }
    pub fn get_compression_treshold(&self) -> i32 {
        unsafe { (self.vtable.get_compression_treshold)(self.pointer) }
    }
    pub fn get_port(&self) -> u16 {
        unsafe { (self.vtable.get_port)(self.pointer) }
    }
    pub fn get_plugins<'a>(&'a self) -> BwsPlugins<'a> {
        let Tuple2(pointer, vtable) = unsafe { (self.vtable.get_plugins)(self.pointer) };

        BwsPlugins {
            pointer,
            vtable,
            phantom: PhantomData,
        }
    }
}
