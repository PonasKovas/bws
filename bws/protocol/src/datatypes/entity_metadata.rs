use crate::datatypes::*;
use crate::{Deserializable, Serializable};
use bitflags::bitflags;

use crate as protocol;

#[derive(PartialEq, Clone, Debug)]
pub enum Entity<'a> {
    Flags(EntityBitflags),
    AirTicks(VarInt),
    CustomName(Option<Chat<'a>>),
    IsCustomNameVisible(bool),
    IsSilent(bool),
    HasNoGravity(bool),
    Pose(Pose),
}

#[derive(PartialEq, Clone, Debug)]
pub enum ThrownEgg<'a> {
    Entity(Entity<'a>),
    Item(Slot),
}

bitflags! {
    pub struct EntityBitflags: u8 {
        const IS_ON_FIRE = 0x01;
        const IS_CROUCHING = 0x02;
        const IS_SPRINTING = 0x08;
        const IS_SWIMMING = 0x10;
        const IS_INVISIBLE = 0x20;
        const IS_GLOWING = 0x40;
        const IS_FLYING_WITH_ELYTRA = 0x80;
    }
}
