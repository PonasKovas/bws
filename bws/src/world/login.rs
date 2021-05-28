use std::{
    process::Command,
    thread::{sleep, Builder},
    time::Duration,
};

use crate::chat_parse;
use crate::datatypes::*;
use crate::internal_communication::{SHBound, SHSender, WBound, WSender};
use crate::packets::{ClientBound, ServerBound, TitleAction};
use crate::world::World;
use serde_json::to_string;
use tokio::sync::mpsc::{unbounded_channel, UnboundedSender};

pub struct LoginWorld {}

impl World for LoginWorld {
    fn get_world_name(&self) -> &str {
        "authentication"
    }
    fn dimension(&self) -> nbt::Blob {
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

        dimension
    }
    fn add_player(&mut self, username: String, sh_sender: SHSender) {
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

        // declare commands
        sh_sender
            .send(SHBound::Packet(ClientBound::DeclareCommands(
                vec![
                    CommandNode::Root(vec![VarInt(1), VarInt(4)]),
                    CommandNode::Literal(false, vec![VarInt(2)], None, "register".to_string()),
                    CommandNode::Argument(
                        false,
                        vec![VarInt(3)],
                        None,
                        "password".to_string(),
                        Parser::String(VarInt(0)),
                        false,
                    ),
                    CommandNode::Argument(
                        true,
                        Vec::new(),
                        None,
                        "password".to_string(),
                        Parser::String(VarInt(0)),
                        false,
                    ),
                    CommandNode::Literal(false, vec![VarInt(5)], None, "login".to_string()),
                    CommandNode::Argument(
                        true,
                        Vec::new(),
                        None,
                        "password".to_string(),
                        Parser::String(VarInt(0)),
                        false,
                    ),
                ],
                VarInt(0),
            )))
            .unwrap();

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
}

pub fn new() -> LoginWorld {
    LoginWorld {}
}
