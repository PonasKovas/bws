pub mod login;

use crate::chat_parse;
use crate::datatypes::*;
use crate::internal_communication::{SHBound, WBound, WReceiver, WSender};
use crate::packets::{ClientBound, ServerBound, TitleAction};
use futures::future::FutureExt;
use serde_json::{json, to_string};
use std::{
    thread::Builder,
    time::{Duration, Instant},
};
use tokio::{
    sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender},
    task::unconstrained,
};

// The trait that will be needed to be implemented to create types of worlds (login, lobby, game and so on)
pub trait World: Sized {
    // contains only the most basic methods, will add more later as needed

    // the main method, takes over the thread and runs the world
    fn run(mut self, mut w_receiver: WReceiver) {
        loop {
            let start_of_tick = Instant::now();

            // first - process all WBound messages on the channel
            process_wbound_messages(&mut self, &mut w_receiver);

            // and then simulate the game

            // wait until the next tick, if needed
            std::thread::sleep(
                Duration::from_nanos(1_000_000_000 / 20)
                    .saturating_sub(Instant::now().duration_since(start_of_tick)),
            );
        }
    }
    // should return the name of the world, which doesn't have to be unique
    // but should only contain [a-z0-9/._-] characters
    fn get_world_name(&self) -> &str;
}

pub fn start<W: 'static + World + Send>(world: W) -> WSender {
    let (w_sender, mut w_receiver) = unbounded_channel::<WBound>();

    Builder::new()
        .name(format!("'{}' world thread", world.get_world_name()))
        .spawn(move || {
            world.run(w_receiver);
        })
        .unwrap();

    w_sender
}

fn process_wbound_messages<W: World>(world: &mut W, w_receiver: &mut WReceiver) {
    loop {
        // Tries executing the future exactly once, without forcing it to yield earlier (because non-cooperative multitasking).
        // If it returns Pending, then break the whole loop, because that means there
        // are no more messages queued up at this moment.
        let message = match unconstrained(w_receiver.recv()).now_or_never().flatten() {
            Some(m) => m,
            None => break,
        };

        match message {
            WBound::AddPlayer(username, sh_sender) => {
                // Request to add the player to this world

                let mut dimension = nbt::Blob::new();
                dimension
                    .insert("piglin_safe".to_string(), nbt::Value::Byte(0))
                    .unwrap();
                dimension
                    .insert("natural".to_string(), nbt::Value::Byte(1))
                    .unwrap();
                dimension
                    .insert("ambient_light".to_string(), nbt::Value::Float(1.0))
                    .unwrap();
                dimension
                    .insert("fixed_time".to_string(), nbt::Value::Long(18000))
                    .unwrap();
                dimension
                    .insert("infiniburn".to_string(), nbt::Value::String("".to_string()))
                    .unwrap();
                dimension
                    .insert("respawn_anchor_works".to_string(), nbt::Value::Byte(0))
                    .unwrap();
                dimension
                    .insert("has_skylight".to_string(), nbt::Value::Byte(1))
                    .unwrap();
                dimension
                    .insert("bed_works".to_string(), nbt::Value::Byte(0))
                    .unwrap();
                dimension
                    .insert(
                        "effects".to_string(),
                        nbt::Value::String("minecraft:overworld".to_string()),
                    )
                    .unwrap();
                dimension
                    .insert("has_raids".to_string(), nbt::Value::Byte(0))
                    .unwrap();
                dimension
                    .insert("logical_height".to_string(), nbt::Value::Int(256))
                    .unwrap();
                dimension
                    .insert("coordinate_scale".to_string(), nbt::Value::Float(1.0))
                    .unwrap();
                dimension
                    .insert("ultrawarm".to_string(), nbt::Value::Byte(0))
                    .unwrap();
                dimension
                    .insert("has_ceiling".to_string(), nbt::Value::Byte(0))
                    .unwrap();

                let packet = ClientBound::JoinGame(
                    0,
                    false,
                    0,
                    -1,
                    vec![world.get_world_name().to_string()],
                    dimension,
                    world.get_world_name().to_string(),
                    0,
                    VarInt(20),
                    VarInt(8),
                    false,
                    false,
                    false,
                    true,
                );
                sh_sender.send(SHBound::Packet(packet)).unwrap();
                sh_sender
                    .send(SHBound::Packet(ClientBound::SetBrand("BWS".to_string())))
                    .unwrap();

                sh_sender
                    .send(SHBound::Packet(ClientBound::PlayerPositionAndLook(
                        0.0,
                        0.0,
                        0.0,
                        0.0,
                        0.0,
                        0,
                        VarInt(0),
                    )))
                    .unwrap();

                // this definetely belongs in the login world only TODO (chunk data above too)
                sh_sender
                    .send(SHBound::Packet(ClientBound::Title(TitleAction::SetTitle(
                        to_string(&chat_parse("§bWelcome to §d§lBWS§r§b!".to_string())).unwrap(),
                    ))))
                    .unwrap();
                sh_sender
                    .send(SHBound::Packet(ClientBound::Title(
                        TitleAction::SetActionBar(
                            to_string(&chat_parse(
                                "§aType §6/login §aor §6/register §ato continue".to_string(),
                            ))
                            .unwrap(),
                        ),
                    )))
                    .unwrap();
                sh_sender
                    .send(SHBound::Packet(ClientBound::Title(
                        TitleAction::SetDisplayTime(15, 60, 15),
                    )))
                    .unwrap();
            }
            _ => {}
        }
    }
}
