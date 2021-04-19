use crate::datatypes::*;
use std::io::{self, Cursor};

// Sent from the client to the server
#[derive(Debug, Clone)]
pub enum ServerBound {
    Handshake(VarInt, MString, u16, VarInt), // protocol, address, port, next state
    StatusRequest,
    StatusPing(i64), // random number
    // LoginStart(MString), // username
    // KeepAlive(i64),
    // ChatMessage(MString), // the raw message, up to 256 characters
    // ClientStatus(VarInt), // 0 - respawn, 1 - request statistics
    // InteractEntity(VarInt, VarInt, bool), // entity id, [0 - interact, 1 - attack, 2 - interact at (not supported)], whether sneaking
    // PlayerPositionAndRotation(f64, f64, f64, f32, f32, bool), // x, y, z, yaw, pitch, whether on ground
    // Animation(VarInt),                                        // 0 - main hand, 1 - off hand
    // TeleportConfirm(VarInt),                                  // teleport id
    // EntityAction(VarInt, VarInt, VarInt), // player's entity id, action (see https://wiki.vg/index.php?title=Protocol&oldid=16091#Entity_Action), jump boost (only for jumping with horse)
    // HeldItemChange(i16),                  // slot id 0-8
    // UseItem(VarInt),                      // 0 - main hand, 1 - off hand
    // PlayerDigging(VarInt, i64, i8),       // action [0-6], position, face
    Unknown(VarInt), // the packet id of the unknown packet
}

// Sent from the server to the client
#[derive(Debug, Clone)]
pub enum ClientBound {
    StatusResponse(MString),
    StatusPong(i64), // the same random number
    LoginDisconnect(MString),
    // SetCompression(VarInt),      // treshold
    // LoginSuccess(u128, MString), // UUID and Username
    // KeepAlive(i64),              // some random number that the client must respond with
    // PlayDisconnect(MString),
    // UpdateHealth(f32, VarInt, f32), // health, food, saturation
    // PlayerPositionAndLook(f64, f64, f64, f32, f32, u8, VarInt), // x, y, z, yaw, pitch, flags, tp id
    // SpawnLivingEntity(
    //     VarInt,
    //     u128,
    //     VarInt,
    //     f64,
    //     f64,
    //     f64,
    //     u8,
    //     u8,
    //     u8,
    //     i16,
    //     i16,
    //     i16,
    // ), // entity id, uuid, type, x, y, z, yaw, pitch, head pitch, velocity: x, y, z
    // EntityTeleport(VarInt, f64, f64, f64, u8, u8, bool), // entity id, x, y, z, yaw, pitch, whether on ground
    // EntityPosition(VarInt, i16, i16, i16, bool), // entity id, delta x, y ,z, whether on ground
    // DestroyEntities(Vec<VarInt>),                // Array of entity IDs to destroy
    // JoinGame(i32), // this has lots of other data, but we're reading only the entity id
    // SetSlot(i8, i16, Slot), // window id, slot id, slot data
    // Statistics(Vec<(VarInt, VarInt, VarInt)>), // Category, id, value
    //
}

impl ServerBound {
    pub fn deserialize(input: &mut Cursor<&Vec<u8>>, status: i64) -> io::Result<Self> {
        let packet_id = VarInt::deserialize(input)?.0;

        let result = match status {
            0 => {
                // Handshake
                match packet_id {
                    0x00 => Ok(Self::Handshake(
                        VarInt::deserialize(input)?,
                        MString::deserialize(input)?,
                        u16::deserialize(input)?,
                        VarInt::deserialize(input)?,
                    )),
                    _ => Ok(Self::Unknown(VarInt(packet_id))),
                }
            }
            1 => {
                // Status
                match packet_id {
                    0x00 => Ok(Self::StatusRequest),
                    0x01 => Ok(Self::StatusPing(i64::deserialize(input)?)),
                    _ => Ok(Self::Unknown(VarInt(packet_id))),
                }
            }
            _ => Ok(Self::Unknown(VarInt(packet_id))),
        };

        result
    }
}

impl ClientBound {
    pub fn serialize(self, output: &mut Vec<u8>) {
        match self {
            Self::StatusResponse(json) => {
                VarInt(0x00).serialize(output);

                json.serialize(output);
            }
            Self::StatusPong(number) => {
                VarInt(0x01).serialize(output);

                number.serialize(output);
            }
            Self::LoginDisconnect(reason) => {
                VarInt(0x00).serialize(output);

                reason.serialize(output);
            }
        }
    }
}
