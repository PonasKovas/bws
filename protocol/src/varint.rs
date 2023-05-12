use crate::{FromBytes, ToBytes};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct VarInt(pub i32);

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct VarLong(pub i64);

impl ToBytes for VarInt {
    fn write_to<W: std::io::Write>(&self, write: &mut W) -> std::io::Result<usize> {
        let mut i = 0;
        let mut value = self.0;
        loop {
            // Take the 7 lower bits of the value
            let mut temp = (value & 0b0111_1111) as u8;

            // Shift the value 7 bits to the right.
            value = ((value as u32) >> 7) as i32;

            // If there is more data to write, set the high bit
            if value != 0 {
                temp |= 0b1000_0000;
            }

            write.write_all(&[temp])?;
            i += 1;

            // If there is no more data to write, exit the loop
            if value == 0 {
                break;
            }
        }

        Ok(i)
    }
}

impl FromBytes for VarInt {
    fn read_from<R: std::io::Read>(read: &mut R) -> std::io::Result<Self>
    where
        Self: Sized,
    {
        let mut num_read = 0; // Count of bytes that have been read
        let mut result = 0i32; // The VarInt being constructed

        loop {
            // VarInts are at most 5 bytes long.
            if num_read == 5 {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "VarInt is too big",
                ));
            }

            // Read a byte
            let mut byte = 0u8;
            read.read_exact(std::slice::from_mut(&mut byte))?;

            // Extract the 7 lower bits (the data bits) and cast to i32
            let value = (byte & 0b0111_1111) as i32;

            // Shift the data bits to the correct position and add them to the result
            result |= value << (7 * num_read);

            num_read += 1;

            // If the high bit is not set, this was the last byte in the VarInt
            if (byte & 0b1000_0000) == 0 {
                break;
            }
        }

        Ok(Self(result))
    }
}

impl ToBytes for VarLong {
    fn write_to<W: std::io::Write>(&self, write: &mut W) -> std::io::Result<usize> {
        let mut i = 0;

        let mut value = self.0;
        loop {
            // Take the 7 lower bits of the value
            let mut temp = (value & 0b0111_1111) as u8;

            // Shift the value 7 bits to the right.
            value = ((value as u64) >> 7) as i64;

            // If there is more data to write, set the high bit
            if value != 0 {
                temp |= 0b1000_0000;
            }

            write.write_all(&[temp])?;
            i += 1;

            // If there is no more data to write, exit the loop
            if value == 0 {
                break;
            }
        }

        Ok(i)
    }
}

impl FromBytes for VarLong {
    fn read_from<R: std::io::Read>(read: &mut R) -> std::io::Result<Self>
    where
        Self: Sized,
    {
        let mut num_read = 0; // Count of bytes that have been read
        let mut result = 0i64; // The VarInt being constructed

        loop {
            // VarLongs are at most 10 bytes long.
            if num_read > 10 {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "VarLong is too big",
                ));
            }

            // Read a byte
            let mut byte = 0u8;
            read.read_exact(std::slice::from_mut(&mut byte))?;

            // Extract the 7 lower bits (the data bits) and cast to i32
            let value = (byte & 0b0111_1111) as i64;

            // Shift the data bits to the correct position and add them to the result
            result |= value << (7 * num_read);

            num_read += 1;

            // If the high bit is not set, this was the last byte in the VarInt
            if (byte & 0b1000_0000) == 0 {
                break;
            }
        }

        Ok(Self(result))
    }
}

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
    use crate::{varint::VarLong, FromBytes, ToBytes, VarInt};

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
