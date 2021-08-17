use crate::*;
use std::marker::PhantomData;

/// Wrapper of an Arc pointer to `InnerGlobalState`.
#[repr(C)]
pub struct BwsGlobalState {
    pointer: *const (),
}

impl Drop for BwsGlobalState {
    fn drop(&mut self) {
        unsafe {
            (VTABLE.drop_global_state)(self.pointer);
        }
    }
}

unsafe impl Sync for BwsGlobalState {}
unsafe impl Send for BwsGlobalState {}

impl BwsGlobalState {
    pub unsafe fn new(pointer: *const ()) -> Self {
        Self { pointer }
    }
    pub fn get_compression_treshold(&self) -> i32 {
        unsafe { (VTABLE.gs_get_compression_treshold)(self.pointer) }
    }
    pub fn get_port(&self) -> u16 {
        unsafe { (VTABLE.gs_get_port)(self.pointer) }
    }
}
