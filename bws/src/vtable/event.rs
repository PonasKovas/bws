use crate::LinearSearch;
use bws_plugin_interface::{
    safe_types::*,
    vtable::{EventFn, LogLevel, VTable},
};
use once_cell::sync::{Lazy, OnceCell};
use std::{
    collections::BTreeMap,
    sync::{Mutex, RwLock},
};

pub struct Callback {
    plugin_name: String,
    callback: EventFn,
    priority: f64,
}

/// (event name, Vector of callbacks)
pub type EventsMap = Vec<(String, Vec<Callback>)>;

/// The map of all events
pub static EVENTS: Lazy<RwLock<EventsMap>> = Lazy::new(|| {
    RwLock::new(vec![(
        "start".to_string(),
        vec![Callback {
            plugin_name: "".to_string(),
            callback: {
                use std::sync::atomic::{AtomicBool, Ordering};

                static STARTED: AtomicBool = AtomicBool::new(false);
                extern "C" fn start_once(vtable: &VTable, _: *const ()) -> bool {
                    // make sure the "start" event isnt fired more than once
                    if STARTED.load(Ordering::SeqCst) {
                        bws_plugin_interface::error!(
                            vtable,
                            "\"start\" event was already fired. Ignoring."
                        );
                        return false;
                    }
                    STARTED.store(true, Ordering::SeqCst);
                    true
                }
                start_once
            },
            priority: f64::NEG_INFINITY,
        }],
    )])
});

pub extern "C" fn get_event_id(event_id: SStr) -> usize {
    let pos = EVENTS
        .read()
        .unwrap()
        .iter()
        .position(|x| x.0 == event_id.as_str());
    match pos {
        Some(p) => p,
        None => {
            // Add the event name and return the position of it
            let mut events_lock = EVENTS.write().unwrap();

            events_lock.push((event_id.into_str().to_owned(), Vec::new()));

            events_lock.len() - 1
        }
    }
}

pub extern "C" fn add_event_callback(
    event_id: usize,
    plugin_name: SStr,
    callback: EventFn,
    priority: f64,
) {
    let mut events_lock = EVENTS.write().unwrap();

    // Callbacks with "" as plugin_name are reserved for BWS itself, not plugins
    // for example the "start" event has a hardcoded callback that prevents it from
    // being fired more than once, and it uses "" as the plugin_name.
    // This is important so that the special callback would get executed before all others
    if plugin_name.is_empty() {
        bws_plugin_interface::error!(
            super::VTABLE,
            "Attempted to register an event ({:?}, id: {}) callback without plugin name.",
            if event_id >= events_lock.len() {
                "{{invalid event}}"
            } else {
                &events_lock[event_id].0
            },
            event_id
        );
        return;
    }

    // Make sure event ID is valid
    if event_id >= events_lock.len() {
        bws_plugin_interface::error!(
            super::VTABLE,
            "Plugin {} tried to add an event callback for event {}, but no event with such ID exists.", plugin_name, event_id
        );
        return;
    }

    // Make sure the same callback is not already registered
    for c in &events_lock[event_id].1 {
        if c.callback == callback {
            bws_plugin_interface::error!(
                super::VTABLE,
                "Plugin {plugin_name} tried to add an event callback for event \"{}\" (id: {event_id}), but an identical callback already exists: (plugin_name: {:?}, callback: {:?}, priority: {}).",
                events_lock[event_id].0,
                c.plugin_name,
                c.callback,
                c.priority
            );
            return;
        }
    }

    events_lock[event_id].1.push(Callback {
        plugin_name: plugin_name.into_str().to_owned(),
        callback,
        priority,
    });

    // sort the callbacks according to their priorities
    events_lock[event_id].1.sort_by(|a, b| {
        match a.priority.total_cmp(&b.priority) {
            std::cmp::Ordering::Equal => {
                // Make sure the ordering is deterministic every time
                a.plugin_name.cmp(&b.plugin_name)
            }
            ord => ord,
        }
    });
}

pub extern "C" fn remove_event_callback(event_id: usize, callback: EventFn) {
    let mut events_lock = EVENTS.write().unwrap();

    // Make sure event ID is valid
    if event_id >= events_lock.len() {
        bws_plugin_interface::error!(
            super::VTABLE,
            "Attempted to remove callback ({callback:?}) from event {event_id}, but no event with such ID exists."
        );
        return;
    }

    let index = match events_lock[event_id]
        .1
        .iter()
        .position(|c| c.callback == callback)
    {
        Some(n) => n,
        None => {
            bws_plugin_interface::error!(
                super::VTABLE,
                "Attempted to remove callback ({callback:?}) from event {} (id: {event_id}), but the callback was not found.",
                events_lock[event_id].0
            );
            return;
        }
    };
    events_lock[event_id].1.remove(index);
}

pub extern "C" fn fire_event(event_id: usize, data: *const ()) -> bool {
    let events_lock = EVENTS.read().unwrap();

    if event_id >= events_lock.len() {
        bws_plugin_interface::error!(
            super::VTABLE,
            "Attempted to fire event {}, but no event with such ID exists.",
            event_id
        );
        return false;
    }

    for callback in &events_lock[event_id].1 {
        if !(callback.callback)(&super::VTABLE, data) {
            return false;
        }
    }

    true
}
