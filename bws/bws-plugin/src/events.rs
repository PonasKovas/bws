use std::marker::PhantomData;

use crate::prelude::*;

#[repr(C)]
#[derive(Debug, Clone, PartialEq)]
pub struct BwsEvent<'a> {
    pub id: u32,
    pub data: *mut (),
    phantom: PhantomData<&'a ()>,
}

unsafe impl<'a> Sync for BwsEvent<'a> {}
unsafe impl<'a> Send for BwsEvent<'a> {}

impl BwsEvent<'static> {
    pub fn new(id: u32, data: *mut ()) -> Self {
        Self {
            id,
            data,
            phantom: PhantomData,
        }
    }
}
