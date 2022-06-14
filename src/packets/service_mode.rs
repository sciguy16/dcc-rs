// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! This module provides types and serialisers for each "service mode"
//! packet type defined by the NMRA standard.
//!
//! <https://www.nmra.org/sites/default/files/standards/sandrp/pdf/S-9.2.3_2012_07.pdf>

use super::{Error, Result, SerialiseBuffer};
use bitvec::prelude::*;

/// Instruction types supported by the `Instruction` packet:
/// * `VerifyByte`: decoder compares its recorded CV value against the provided
/// data byte and responds with an acknowledgement if they match
/// * `WriteCvByte`: decoder writes the provided data byte into the specified
/// CV slot and may respond with an acknowledgement on successful write
/// * `VerifyCvBit`: Compare the given bit with the bit in the specified
/// position within the CV and repond with an acknowledgement if they match
/// * `WriteCvBit`: Write the given bit into the specified position within the
/// specified CV. Decoder may respond with an acknowledgement on success
#[derive(Copy, Clone)]
#[allow(missing_docs)]
pub enum InstructionType {
    WriteCvBit { offset: u8, value: bool },
    VerifyCvBit { offset: u8, value: bool },
    WriteCvByte { value: u8 },
    VerifyCvByte { value: u8 },
}

/// The `Instruction` service-mode packet instructs the decoder to write or
/// verify the specified 10-bit CV address against the provided data byte
pub struct Instruction {
    typ: InstructionType,
    cv_address: u16,
}

impl Instruction {
    /// Create a builder for the Instruction packet
    pub fn builder() -> InstructionBuilder {
        InstructionBuilder::default()
    }

    /// Serialise the Instruction packet into the provided bufffer. Returns the
    /// number of bits written or an `Error::TooLong` if the buffer has
    /// insufficient capacity
    pub fn serialise(&self, buf: &mut SerialiseBuffer) -> Result<usize> {
        // write the first two bits of CV address into this byte now and fill
        // in the packet type later
        let mut type_and_start_of_address = 0x70;
        type_and_start_of_address |= (self.cv_address >> 8) as u8;

        // Pull out the lower 8 bits of the CV address
        let rest_of_address = (self.cv_address & 0x00ff) as u8;

        // Calculate the "data" byte: in "byte" modes this is simply the
        // provided data byte; in "bit" modes this is a combination of offset
        // and value
        #[allow(clippy::unusual_byte_groupings)]
        let data = match self.typ {
            InstructionType::WriteCvBit { offset, value } => {
                type_and_start_of_address |= 0b0000_10_00;
                // padding - 1=write - data - offset
                let mut data = 0b111_1_0000;
                data |= offset;
                data |= (value as u8) << 3;
                data
            }
            InstructionType::VerifyCvBit { offset, value } => {
                type_and_start_of_address |= 0b0000_10_00;
                // padding - 0=verify - data - offset
                #[allow(clippy::unusual_byte_groupings)]
                let mut data = 0b111_0_0000;
                data |= offset;
                data |= (value as u8) << 3;
                data
            }
            InstructionType::WriteCvByte { value } => {
                type_and_start_of_address |= 0b0000_11_00;
                value
            }
            InstructionType::VerifyCvByte { value } => {
                type_and_start_of_address |= 0b0000_01_00;
                value
            }
        };

        super::serialise(
            &[
                type_and_start_of_address,
                rest_of_address,
                data,
                type_and_start_of_address ^ rest_of_address ^ data,
            ],
            buf,
        )
    }
}

/// Builder struct for Instruction packets. Ensures that only valid Instructions
/// are created
#[derive(Default)]
pub struct InstructionBuilder {
    cv_address: Option<u16>,
    typ: Option<InstructionType>,
}

impl InstructionBuilder {
    /// Set the address. Returns `Error::InvalidAddress` if the provided CV
    /// addresss does not fit into 10 bits.
    ///
    /// From the standard: "The configuration variable being addressed is the
    /// provided 10-bit address plus 1", so this method subtracts 1 from the
    /// supplied CV number in order to determine its address.
    pub fn cv_address(&mut self, cv_address: u16) -> Result<&mut Self> {
        if 0 < cv_address && cv_address < 0x0400 {
            self.cv_address = Some(cv_address - 1);
            Ok(self)
        } else {
            Err(Error::InvalidAddress)
        }
    }

    /// Create a `WriteCvByte` packet with the provided byte value
    pub fn write_byte(&mut self, value: u8) -> &mut Self {
        self.typ = Some(InstructionType::WriteCvByte { value });
        self
    }

    /// Create a `VerifyCvByte` packet with the provided byte value
    pub fn verify_byte(&mut self, value: u8) -> &mut Self {
        self.typ = Some(InstructionType::VerifyCvByte { value });
        self
    }

    /// Create a `WriteCvBit` packet with the provided bit offset and value
    pub fn write_bit(&mut self, offset: u8, value: bool) -> Result<&mut Self> {
        if offset < 0x08 {
            self.typ = Some(InstructionType::WriteCvBit { offset, value });
            Ok(self)
        } else {
            Err(Error::InvalidOffset)
        }
    }

    /// Create a `VerifyCvBit` packet with the provided bit offset and value
    pub fn verify_bit(&mut self, offset: u8, value: bool) -> Result<&mut Self> {
        if offset < 0x08 {
            self.typ = Some(InstructionType::VerifyCvBit { offset, value });
            Ok(self)
        } else {
            Err(Error::InvalidOffset)
        }
    }

    /// Validate that all fields are present and return an Instruction packet
    pub fn build(&mut self) -> Result<Instruction> {
        Ok(Instruction {
            typ: self.typ.ok_or(Error::MissingField)?,
            cv_address: self.cv_address.ok_or(Error::MissingField)?,
        })
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::packets::test::print_chunks;

    #[test]
    fn serialise_instruction_packet_write_byte() {
        // [    preamble    ] S      WWAA SAAA A AAA A_S_DD DD_DD DD_S_E EEE_E EEES
        // 1111 1111 1111 111_0 0111 1100 0001 0_111 1_0_10 10_10 10_0_1 111_1 0011
        let pkt = Instruction::builder()
            .cv_address(48)
            .unwrap()
            .write_byte(0xaa)
            .build()
            .unwrap();

        let mut buf = SerialiseBuffer::default();
        let len = pkt.serialise(&mut buf).unwrap();
        assert_eq!(len, 52);

        #[allow(clippy::unusual_byte_groupings)]
        let expected_arr = &[
            0b1111_1111_u8, // PPPP PPPP
            0b1111_111_0,   // PPPP PPPS
            0b0111_11_00,   // 0111 WWAA
            0b0001_0_111,   // SAAA AAAA
            0b1_0_10_10_10, // ASDD DDDD
            0b10_0_1_111_1, // DDSE EEEE
            0b001_1_0000,   // EEES ----
        ]
        .view_bits::<Msb0>()[..len];
        let mut expected = SerialiseBuffer::default();
        expected[..52].copy_from_bitslice(expected_arr);

        println!("Got:");
        print_chunks(&buf, 52);
        println!("Expected:");
        print_chunks(&expected, 52);
        assert_eq!(buf[..len], expected[..52]);
    }

    #[test]
    fn serialise_instruction_packet_verify_bit() {
        // [    preamble    ] S      WWAA SAAA A AAA A_S_DD DK_BO OO_S_E EEE_E EEES
        // 1111 1111 1111 111_0 0111 1001 0001 0_100 1_0_11 10_11 01_0_1 011_1 0111
        let pkt = Instruction::builder()
            .cv_address(298)
            .unwrap()
            .verify_bit(5, true)
            .unwrap()
            .build()
            .unwrap();

        let mut buf = SerialiseBuffer::default();
        let len = pkt.serialise(&mut buf).unwrap();
        assert_eq!(len, 52);

        #[allow(clippy::unusual_byte_groupings)]
        let expected_arr = &[
            0b1111_1111_u8, // PPPP PPPP
            0b1111_111_0,   // PPPP PPPS
            0b0111_10_01,   // 0111 WWAA
            0b0001_0_100,   // SAAA AAAA
            0b1_0_11_10_11, // ASDD DKBO
            0b01_0_1_011_1, // OOSE EEEE
            0b101_1_0000,   // EEES ----
        ]
        .view_bits::<Msb0>()[..len];
        let mut expected = SerialiseBuffer::default();
        expected[..52].copy_from_bitslice(expected_arr);

        println!("Got:");
        print_chunks(&buf, 52);
        println!("Expected:");
        print_chunks(&expected, 52);
        assert_eq!(buf[..len], expected[..52]);
    }
}
