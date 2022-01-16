use crate::LinearSearch;
use bws_plugin::prelude::*;
use lazy_static::lazy_static;
use std::{cmp::PartialOrd, mem::ManuallyDrop};
use tokio::sync::{mpsc::UnboundedSender, oneshot, RwLock};

type EventData = BwsTuple3<usize, SendMutPtr<()>, SendPtr<()>>;

lazy_static! {
    static ref EVENTS: RwLock<Vec<(EventName, Vec<(f32, UnboundedSender<EventData>)>)>> =
        RwLock::new(Vec::new());
}

pub struct EventName {
    // Usually the name of the plugin that the event originates from
    // or 'core' if it's built-in
    pub namespace: String,
    pub name: String,
}

pub async fn get_event_id(namespace: &str, name: &str) -> usize {
    let already_exists =
        EVENTS.read().await.iter().position(|event_name| {
            &event_name.0.namespace == namespace && &event_name.0.name == name
        });

    match already_exists {
        Some(id) => id,
        None => {
            // add the event
            EVENTS.write().await.push((
                EventName {
                    namespace: namespace.to_owned(),
                    name: name.to_owned(),
                },
                Vec::new(),
            ));

            EVENTS.read().await.len() - 1
        }
    }
}

/// Panics if invalid ID
pub async fn subscribe_to_event(id: usize, priority: f32, sender: UnboundedSender<EventData>) {
    EVENTS.write().await[id].1.push((priority, sender));
    EVENTS.write().await[id]
        .1
        .sort_unstable_by(|v1, v2| (v1.0).partial_cmp(&v2.0).unwrap());
}

/// Panics if invalid ID
pub async fn fire_event(id: usize, data: SendMutPtr<()>) -> SendMutPtr<()> {
    for (_priority, sender) in &EVENTS.read().await[id].1 {
        let (oneshot_sender, oneshot_receiver) = oneshot::channel::<bool>();
        let oneshot_sender = ManuallyDrop::new(oneshot_sender);
        sender
            .send(BwsTuple3(
                id,
                data,
                SendPtr(&*oneshot_sender as *const _ as *const ()),
            ))
            .unwrap();

        if oneshot_receiver.await.unwrap() {
            // true means further handling is not wanted
            break;
        }
    }

    data
}
