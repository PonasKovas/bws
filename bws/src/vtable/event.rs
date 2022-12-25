use crate::LinearSearch;
use bws_plugin_interface::{
    safe_types::*,
    vtable::{EventFn, LogLevel, VTable},
};
use log::error;
use once_cell::sync::{Lazy, OnceCell};
use std::{
    collections::BTreeMap,
    sync::{Mutex, RwLock},
};

pub struct Callback {
    plugin_name: String,
    callback: EventFn,
    before_reqs: Vec<String>,
}

/// event id -> Vector of callbacks
pub type EventsMap = Vec<(String, Vec<Callback>)>;

/// The map of all events
pub static EVENTS: Lazy<RwLock<EventsMap>> = Lazy::new(|| RwLock::new(Vec::new()));

/// Returns the numerical ID of the event, which will to stay the same until next launch
pub extern "C" fn get_event_id(event_id: SStr) -> usize {
    let pos = EVENTS
        .read()
        .unwrap()
        .iter()
        .position(|x| x.0 == event_id.as_str());
    match pos {
        Some(p) => p,
        None => {
            // Add the event name and return the position of that
            let mut events_lock = EVENTS.write().unwrap();

            events_lock.push((event_id.into_str().to_owned(), Vec::new()));

            events_lock.len() - 1
        }
    }
}

/// Registers a callback for an event
///
///  - `event_id` - the numerical ID of the event (can be obtained with `get_event_id`)
///  - `plugin_name` - the name of the plugin which is registering the callback
///  - `callback` - the callback function pointer
///  - `before_reqs` - an optional list of plugins that must handle the event after this plugin
pub extern "C" fn add_event_callback(
    event_id: usize,
    plugin_name: SStr,
    callback: EventFn,
    before_reqs: SSlice<SStr>,
) {
    let mut events_lock = EVENTS.write().unwrap();

    // Make sure event ID is valid
    if event_id >= events_lock.len() {
        error!("Plugin {} tried to add an event callback for event {}, but no event with such ID exists.", plugin_name, event_id);
        // todo: this error probably needs to be fatal and result in a shutdown
        return;
    }

    // Find the right position to insert the new callback that meets the requirements
    let mut minimum_idx = 0;
    let mut maximum_idx = events_lock[event_id].1.len();
    // check requirements given by the plugin thats registering right now
    for req in before_reqs {
        for (i, callback) in events_lock[event_id].1.iter().enumerate().rev() {
            if callback.plugin_name == req.as_str() {
                maximum_idx = i;
            }
        }
    }
    // check requirements given by all other plugins in this event
    for (i, callback) in events_lock[event_id].1.iter().enumerate() {
        for req in &callback.before_reqs {
            if req == plugin_name.as_str() {
                minimum_idx = i + 1;
            }
        }
    }

    if minimum_idx > maximum_idx {
        // There is no possible way to satisfy all requirements
        error!(
            "{} failed to add callback for event {} ({}): it wants to come before plugin {}, but plugin {} wants to come before {}",
            plugin_name,
            events_lock[event_id].0,
            event_id,
            (events_lock[event_id].1)[maximum_idx].plugin_name,
            (events_lock[event_id].1)[minimum_idx-1].plugin_name,
            plugin_name,
        );
        // todo: this error probably needs to be fatal and result in a shutdown
        return;
    }

    events_lock[event_id].1.insert(
        minimum_idx,
        Callback {
            plugin_name: plugin_name.into_str().to_owned(),
            callback,
            before_reqs: before_reqs
                .iter()
                .map(|x| x.into_str().to_owned())
                .collect(),
        },
    );
}

/// Fires an event and executes the callbacks associated
///
///  - `event_id` - the numerical ID of the event (can be obtained with `get_event_id`)
///  - `data` - a pointer to arbitrary data that event handlers will have access to
///
/// Returns `false` if the event handling was ended by a callback, `true` otherwise.
pub extern "C" fn fire_event(event_id: usize, data: *const ()) -> bool {
    for callback in &EVENTS.read().unwrap()[event_id].1 {
        if !(callback.callback)(&super::VTABLE, data) {
            return false;
        }
    }

    true
}
