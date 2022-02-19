use std::io::Cursor;

use protocol::{datatypes::*, packets::*, Deserializable, Serializable};

#[test]
fn first() {
    let mut buffer: Vec<u8> = Vec::new();

    let original = PlayServerBound::ClientSettings {
        locale: "locale".into(),
        view_distance: 11,
        chat_mode: ChatMode::CommandsOnly,
        chat_colors: true,
        displayed_skin_parts: SkinParts::JACKET | SkinParts::HAT,
        main_hand: Hand::Left,
    };

    original.to_writer(&mut buffer).unwrap();
    println!("{:x?}", buffer);
    let parsed = PlayServerBound::from_reader(&mut Cursor::new(buffer)).unwrap();

    assert_eq!(original, parsed);
}
