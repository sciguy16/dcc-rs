use crate::Error;
use bitvec::prelude::*;

pub type Result<T> = core::result::Result<T, Error>;

pub struct Preamble(BitArr!(for 14, in u8, Msb0));
pub type SerialiseBuffer = BitArr!(for 42, in u8, Msb0);

impl Default for Preamble {
    fn default() -> Self {
        Self(BitArray::from([0xff, 0xff]))
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum Direction {
    Forward,
    Backward,
}

impl Default for Direction {
    fn default() -> Self {
        Self::Forward
    }
}

pub struct SpeedAndDirection {
    address: u8,
    instruction: u8,
    ecc: u8,
}

impl SpeedAndDirection {
    pub fn builder() -> SpeedAndDirectionBuilder {
        SpeedAndDirectionBuilder::default()
    }

    pub fn serialise(&self, buf: &mut SerialiseBuffer) -> Result<usize> {
        buf[0..16].copy_from_bitslice([0xff, 0xfe].view_bits::<Msb0>()); // preamble
        buf.set(15, false); // start bit
        buf[16..24].copy_from_bitslice([self.address].view_bits::<Msb0>());
        buf.set(24, false); // data start bit
        buf[25..33].copy_from_bitslice([self.instruction].view_bits::<Msb0>());
        buf.set(33, false); // crc start bit
        buf[34..42].copy_from_bitslice([self.ecc].view_bits::<Msb0>());

        buf.set(42, true); // stop bit

        Ok(42)
    }
}

#[derive(Default)]
pub struct SpeedAndDirectionBuilder {
    address: Option<u8>,
    speed: Option<u8>,
    headlight: Option<bool>,
    direction: Option<Direction>,
}

impl SpeedAndDirectionBuilder {
    pub fn address(&mut self, address: u8) -> Result<&mut Self> {
        if address == 0 || address > 0x7f {
            Err(Error::InvalidAddress)
        } else {
            self.address = Some(address);
            Ok(self)
        }
    }

    pub fn speed(&mut self, speed: u8) -> Result<&mut Self> {
        if speed > 0x0f {
            Err(Error::InvalidSpeed)
        } else {
            self.speed = Some(speed);
            Ok(self)
        }
    }

    pub fn direction(&mut self, direction: Direction) -> &mut Self {
        self.direction = Some(direction);
        self
    }

    pub fn headlight(&mut self, headlight: bool) -> &mut Self {
        self.headlight = Some(headlight);
        self
    }

    pub fn build(&mut self) -> SpeedAndDirection {
        let address = self.address.unwrap_or(3);
        let mut instruction = 0b0100_0000; // packet type
        if let Direction::Forward = self.direction.unwrap_or_default() {
            instruction |= 0b0010_0000;
        }
        if self.headlight.unwrap_or_default() {
            instruction |= 0b0001_0000;
        }
        // speed is only in the bottom 4 bits, enforced by builder
        instruction |= self.speed.unwrap_or_default();
        let ecc = address ^ instruction;
        SpeedAndDirection {
            address,
            instruction,
            ecc,
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    fn display_serialise_buffer(buf: &SerialiseBuffer) {
        println!("{buf:?}");
        //        15              1 8        1 8        1 8        1
        //        15              16 24      25 33      34 42      43
        println!("ppppppppppppppp s aaaaaaaa s 01dcvvvv s cccccccc s");
        println!(
            "{} {} {} {} {} {} {} {}",
            buf[..15]
                .iter()
                .map(|b| if *b { "1" } else { "0" })
                .collect::<Vec<_>>()
                .join(""),
            if *buf.get(15).unwrap() { "1" } else { "0" },
            buf[16..24]
                .iter()
                .map(|b| if *b { "1" } else { "0" })
                .collect::<Vec<_>>()
                .join(""),
            if *buf.get(24).unwrap() { "1" } else { "0" },
            buf[25..33]
                .iter()
                .map(|b| if *b { "1" } else { "0" })
                .collect::<Vec<_>>()
                .join(""),
            if *buf.get(33).unwrap() { "1" } else { "0" },
            buf[34..42]
                .iter()
                .map(|b| if *b { "1" } else { "0" })
                .collect::<Vec<_>>()
                .join(""),
            if *buf.get(42).unwrap() { "1" } else { "0" },
        );
    }

    #[test]
    fn make_speed_and_direction() -> Result<()> {
        let pkt = SpeedAndDirection::builder()
            .address(35)?
            .speed(14)?
            .direction(Direction::Forward)
            .build();
        assert_eq!(pkt.address, 35);
        assert_eq!(pkt.instruction, 0b0110_1110);
        assert_eq!(pkt.ecc, 0x4d);

        Ok(())
    }

    #[test]
    fn serialise_speed_and_direction() -> Result<()> {
        let pkt = SpeedAndDirection::builder()
            .address(35)?
            .speed(14)?
            .direction(Direction::Forward)
            .build();
        let mut buf = SerialiseBuffer::default();
        let len = pkt.serialise(&mut buf)?;
        #[allow(clippy::unusual_byte_groupings)]
        let expected_arr = [
            0xff_u8,      // preamble
            0b1111_1110,  // preamble + start
            35,           // address
            0b0_0110_111, // start + instr[..7]
            0b0_0_010011, // instr[7] + start + crc[..6]
            0b01_1_00000, // crc[6..] + stop + 5 zeroes
        ];
        let mut expected = SerialiseBuffer::default();
        expected[..43].copy_from_bitslice(&expected_arr.view_bits::<Msb0>()[..43]);
        println!("got:");
        display_serialise_buffer(&buf);
        println!("expected:");
        display_serialise_buffer(&expected);
        assert_eq!(len, 42);
        assert_eq!(buf[..len], expected[..42]);
        Ok(())
    }
}
