pub mod plugins;

/// Wrapper of an Arc pointer to `InnerGlobalState`.
#[repr(C)]
pub struct BwsGlobalState {
    pointer: *const (),
    drop: unsafe extern "C" fn(*const ()),
    get_compression_treshold: unsafe extern "C" fn(*const ()) -> i32,
    get_port: unsafe extern "C" fn(*const ()) -> u16,
}

impl Drop for BwsGlobalState {
    fn drop(&mut self) {
        unsafe {
            (self.drop)(self.pointer);
        }
    }
}

unsafe impl Sync for BwsGlobalState {}
unsafe impl Send for BwsGlobalState {}

impl BwsGlobalState {
    pub unsafe fn new(
        pointer: *const (),
        drop: unsafe extern "C" fn(*const ()),
        get_compression_treshold: unsafe extern "C" fn(*const ()) -> i32,
        get_port: unsafe extern "C" fn(*const ()) -> u16,
    ) -> Self {
        Self {
            pointer,
            drop,
            get_compression_treshold,
            get_port,
        }
    }
    pub fn get_compression_treshold(&self) -> i32 {
        unsafe { (self.get_compression_treshold)(self.pointer) }
    }
    pub fn get_port(&self) -> u16 {
        unsafe { (self.get_port)(self.pointer) }
    }
}
