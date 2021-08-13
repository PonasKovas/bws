use crate::*;
use std::iter::IntoIterator;
use std::marker::PhantomData;

type _f_PluginsIterNext<'a> =
    unsafe extern "C" fn(*const ()) -> BwsOption<Tuple2<BwsStr<'a>, BwsPlugin<'a>>>;

#[repr(C)]
pub struct PluginsVTable {
    pub get: unsafe extern "C" fn(*const (), BwsStr) -> BwsOption<Tuple2<*const (), PluginVTable>>,
    pub iter: unsafe extern "C" fn(*const ()) -> Tuple2<*const (), PluginsIterVTable>,
}

#[repr(C)]
pub struct PluginVTable {}

#[repr(C)]
pub struct PluginsIterVTable {
    pub next: unsafe extern "C" fn(*const ()) -> BwsOption<Tuple2<*const (), PluginVTable>>,
}

/// Wrapper of a reference to the plugins hashmap in global state.
pub struct BwsPlugins<'a> {
    pub(crate) pointer: *const (),
    pub(crate) vtable: PluginsVTable,
    // Bound to the lifetime of the global state this was obtained from
    pub(crate) phantom: PhantomData<&'a ()>,
}

// #[repr(C)]
// pub struct BwsPluginsIter<'a> {
//     pointer: *const (), // pointer to a unstable boxed iterator
//     next: _f_PluginsIterNext<'a>,
// }

/// Wrapper of a reference to a `RwLock<Plugin>`
#[repr(C)]
pub struct BwsPlugin<'a> {
    pub(crate) pointer: *const (),
    pub(crate) vtable: PluginVTable,
    // Bound to the lifetime of the `BwsPlugins` this was obtained from
    pub(crate) phantom: PhantomData<&'a ()>,
}

impl<'a> BwsPlugins<'a> {
    pub fn get(&self, name: BwsStr) -> Option<BwsPlugin<'a>> {
        unsafe { (self.vtable.get)(self.pointer, name) }
            .into_option()
            .map(|Tuple2(pointer, vtable)| BwsPlugin {
                pointer,
                vtable,
                phantom: PhantomData,
            })
    }
    // /// **Please note** that this allocates, so if there is another way, better avoid this method.
    // pub fn iter(&self) -> BwsPluginsIter<'a> {
    //     IntoIterator::into_iter(self)
    // }
}

// impl<'a> IntoIterator for &BwsPlugins<'a> {
//     type Item = <BwsPluginsIter<'a> as Iterator>::Item;
//     type IntoIter = BwsPluginsIter<'a>;

//     /// **Please note** that this allocates, so if there is another way, better avoid this method.
//     fn into_iter(self) -> Self::IntoIter {
//         let Tuple2(pointer, next) = unsafe { (self.iter)(self.pointer) };
//         BwsPluginsIter { pointer, next }
//     }
// }

// impl<'a> Iterator for BwsPluginsIter<'a> {
//     type Item = Tuple2<BwsStr<'a>, BwsPlugin<'a>>;

//     fn next(&mut self) -> Option<Self::Item> {
//         unsafe { (self.next)(self.pointer).into_option() }
//     }
// }
