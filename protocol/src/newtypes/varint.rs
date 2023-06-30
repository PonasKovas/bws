#[derive(Debug, Clone, Copy, PartialEq)]
pub struct VarInt(pub i32);

impl From<i32> for VarInt {
    fn from(value: i32) -> Self {
        Self(value)
    }
}

impl From<VarInt> for i32 {
    fn from(value: VarInt) -> i32 {
        value.0
    }
}

#[cfg(test)]
mod tests {
    use super::VarInt;
    use crate::{FromBytes, ToBytes};

    #[test]
    fn varint_read_and_write() {
        let samples: &[(i32, &[u8])] = &[
            (0, &[0x00]),
            (1, &[0x01]),
            (2, &[0x02]),
            (127, &[0x7F]),
            (128, &[0x80, 0x01]),
            (255, &[0xFF, 0x01]),
            (25565, &[0xDD, 0xC7, 0x01]),
            (2097151, &[0xFF, 0xFF, 0x7F]),
            (2147483647, &[0xFF, 0xFF, 0xFF, 0xFF, 0x07]),
            (-1, &[0xFF, 0xFF, 0xFF, 0xFF, 0x0F]),
            (-2147483648, &[0x80, 0x80, 0x80, 0x80, 0x08]),
        ];

        // Writing...
        for sample in samples {
            let mut bytes = Vec::new();
            VarInt(sample.0).write_to(&mut bytes).unwrap();

            assert_eq!(bytes, sample.1);
        }

        // Reading...
        for sample in samples {
            let mut bytes: &[u8] = sample.1;

            assert_eq!(sample.0, VarInt::read_from(&mut bytes).unwrap().0);
        }
    }
}
