use protocol::{datatypes::*, packets::*, Deserializable, Serializable};
use std::io::Cursor;

#[test]
fn first() {
    let mut buffer: Vec<u8> = Vec::new();

    let original = PlayServerBound::ClientSettings(ClientSettings {
        locale: "locale".into(),
        view_distance: 11,
        chat_mode: ChatMode::CommandsOnly,
        chat_colors: true,
        displayed_skin_parts: SkinParts::JACKET | SkinParts::HAT,
        main_hand: Hand::Left,
    });

    original.to_writer(&mut buffer).unwrap();
    println!("{:x?}", buffer);
    let parsed = PlayServerBound::from_reader(&mut Cursor::new(buffer)).unwrap();

    assert_eq!(original, parsed);
}

#[cfg(feature = "ffi_safe")]
#[test]
#[deny(improper_ctypes_definitions)]
fn ffi_safety() {
    extern "C" fn _test(_: protocol::packets::ServerBound, _: protocol::packets::ClientBound) {}
}
