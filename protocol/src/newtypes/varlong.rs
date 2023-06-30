#[derive(Debug, Clone, Copy, PartialEq)]
pub struct VarLong(pub i64);

#[cfg(test)]
mod tests {
    use super::VarLong;
    use crate::{FromBytes, ToBytes};

    #[test]
    fn varlong_read_and_write() {
        let samples: &[(i64, &[u8])] = &[
            (0, &[0x00]),
            (1, &[0x01]),
            (2, &[0x02]),
            (127, &[0x7F]),
            (128, &[0x80, 0x01]),
            (255, &[0xFF, 0x01]),
            (2147483647, &[0xFF, 0xFF, 0xFF, 0xFF, 0x07]),
            (
                9223372036854775807,
                &[0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0x7F],
            ),
            (
                -1,
                &[0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0x01],
            ),
            (
                -2147483648,
                &[0x80, 0x80, 0x80, 0x80, 0xF8, 0xFF, 0xFF, 0xFF, 0xFF, 0x01],
            ),
            (
                -9223372036854775808,
                &[0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x01],
            ),
        ];

        // Writing...
        for sample in samples {
            let mut bytes = Vec::new();
            VarLong(sample.0).write_to(&mut bytes).unwrap();

            assert_eq!(bytes, sample.1);
        }

        // Reading...
        for sample in samples {
            let mut bytes: &[u8] = sample.1;

            assert_eq!(sample.0, VarLong::read_from(&mut bytes).unwrap().0);
        }
    }
}
