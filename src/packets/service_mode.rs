// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! This module provides types and serialisers for each "service mode"
//! packet type defined by the NMRA standard.
//!
//! <https://www.nmra.org/sites/default/files/standards/sandrp/pdf/S-9.2.3_2012_07.pdf>

use super::{Error, Result, SerialiseBuffer};

#[derive(Copy, Clone)]
#[allow(missing_docs)]
pub enum Operation {
    Verify,
    Write,
}

/// "A packet sequence sent to guarantee the contents of the page register"
pub struct PagePreset;

impl PagePreset {
    /// Serialise the Instruction packet into the provided bufffer. Returns the
    /// number of bits written or an `Error::TooLong` if the buffer has
    /// insufficient capacity
    pub fn serialise(&self, buf: &mut SerialiseBuffer) -> Result<usize> {
        super::serialise(&[0b01111101, 0b00000001, 0b01111100], buf)
    }
}

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

/// `AddressOnly` instructs the decoder to set its short-mode address to the
/// provided value and to clear its extended addressing and consist CVs
#[allow(missing_docs)]
pub enum AddressOnly {
    Write { address: u8 },
    Verify { address: u8 },
}

impl AddressOnly {
    /// Create a packet instructing the decoder to write the specified address
    /// into CV1. The decoder may respond with an acknowledgement on success.
    pub fn write(address: u8) -> Result<AddressOnly> {
        if address < 0x7f {
            Ok(AddressOnly::Write { address })
        } else {
            Err(Error::InvalidAddress)
        }
    }

    /// Create a packet instructing the decoder to verify the address stored in
    /// CV1. The decoder must respond with an acknowledgement if they match.
    pub fn verify(address: u8) -> Result<AddressOnly> {
        if address < 0x7f {
            Ok(AddressOnly::Verify { address })
        } else {
            Err(Error::InvalidAddress)
        }
    }

    /// Serialise the Instruction packet into the provided bufffer. Returns the
    /// number of bits written or an `Error::TooLong` if the buffer has
    /// insufficient capacity
    pub fn serialise(&self, buf: &mut SerialiseBuffer) -> Result<usize> {
        let mut instr = 0b0111_0000;
        let address = match self {
            AddressOnly::Write { address } => {
                instr |= 0b0000_1000;
                *address
            }
            AddressOnly::Verify { address } => *address,
        };
        super::serialise(&[instr, address, instr ^ address], buf)
    }
}

/// The `PhysicalRegister` operation instructs the decoder to update or verify
/// the value stored in each of the eight "physical registers". These correspond
/// to various CV slots depending on whether it is a locomotove or an accessory
/// decoder.
pub struct PhysicalRegister {
    operation: Operation,
    register: u8,
    value: u8,
}

impl PhysicalRegister {
    /// Address (CV 1)
    pub const ADDRESS: u8 = 1;
    /// Start Voltage (CV 2)
    pub const START_VOLTAGE: u8 = 2;
    /// Acceleration (CV 3)
    pub const ACCELERATION: u8 = 3;
    /// Deceleration (CV 4)
    pub const DECELERATION: u8 = 4;
    /// Basic configuration register (CV 29)
    pub const BASIC_CONFIGURATION_REGISTER: u8 = 5;
    /// Reserved for page register
    pub const RESERVED_FOR_PAGE_REGISTER: u8 = 6;
    /// Version number (CV 7)
    pub const VERSION_NUMBER: u8 = 7;
    /// Manufacturer ID (CV 8)
    pub const MANUFACTURER_ID: u8 = 8;

    /// Builder for `PhysicalRegister`
    pub fn builder() -> PhysicalRegisterBuilder {
        PhysicalRegisterBuilder::default()
    }

    /// Serialise the PhysicalRegister packet into the provided bufffer. Returns
    /// the number of bits written or an `Error::TooLong` if the buffer has
    /// insufficient capacity
    pub fn serialise(&self, buf: &mut SerialiseBuffer) -> Result<usize> {
        let mut instr = 0b0111_0000;

        if let Operation::Write = self.operation {
            instr |= 0b0000_1000;
        }

        instr |= self.register;

        super::serialise(&[instr, self.value, instr ^ self.value], buf)
    }
}

/// Builder struct for the `PhysicalRegister` packet
#[derive(Default)]
pub struct PhysicalRegisterBuilder {
    operation: Option<Operation>,
    register: Option<u8>,
    value: Option<u8>,
}

impl PhysicalRegisterBuilder {
    /// Sets the `Operation` (verify/write mode) to be performed on the register
    pub fn operation(&mut self, operation: Operation) -> &mut Self {
        self.operation = Some(operation);
        self
    }

    /// Sets the register address. Valid registers are numbered 1-8,
    /// corresponding to raw addresses 0-7. Returns `Error::InvalidAddress` for
    /// values outside this range
    pub fn register(&mut self, register: u8) -> Result<&mut Self> {
        if 1 < register && register <= 8 {
            self.register = Some(register - 1);
            Ok(self)
        } else {
            Err(Error::InvalidAddress)
        }
    }

    /// The 8-bit value to use for the operation
    pub fn value(&mut self, value: u8) -> &mut Self {
        self.value = Some(value);
        self
    }

    /// Build a `PhysicalRegister` packet, returning `Error::MissingField` if
    /// any of the required fields are missing
    pub fn build(&mut self) -> Result<PhysicalRegister> {
        Ok(PhysicalRegister {
            operation: self.operation.ok_or(Error::MissingField)?,
            register: self.register.ok_or(Error::MissingField)?,
            value: self.value.ok_or(Error::MissingField)?,
        })
    }
}

/// Reset decoder to factory-default condition
pub struct FactoryReset;

impl FactoryReset {
    /// Serialise the PhysicalRegister packet into the provided bufffer. Returns
    /// the number of bits written or an `Error::TooLong` if the buffer has
    /// insufficient capacity
    pub fn serialise(&self, buf: &mut SerialiseBuffer) -> Result<usize> {
        super::serialise(&[0b01111111, 0b00001000, 0b01110111], buf)
    }
}

/// Query an older decoder to verify its address
pub struct AddressQuery {
    address: u8,
}

impl AddressQuery {
    /// Create an `AddressQuery` packet for the given address
    pub fn address(address: u8) -> AddressQuery {
        AddressQuery { address }
    }

    /// Serialise the PhysicalRegister packet into the provided bufffer. Returns
    /// the number of bits written or an `Error::TooLong` if the buffer has
    /// insufficient capacity
    pub fn serialise(&self, buf: &mut SerialiseBuffer) -> Result<usize> {
        let instr = 0b11111001;
        super::serialise(&[self.address, instr, self.address ^ instr], buf)
    }
}

/// Instruct any decoder not matching the given address to ignore any subsequent
/// service-mode packets
pub struct DecoderLock {
    address: u8,
}

impl DecoderLock {
    /// Builder for DecoderLock packet
    pub fn builder() -> DecoderLockBuilder {
        DecoderLockBuilder::default()
    }

    /// Serialise the PhysicalRegister packet into the provided bufffer. Returns
    /// the number of bits written or an `Error::TooLong` if the buffer has
    /// insufficient capacity
    pub fn serialise(&self, buf: &mut SerialiseBuffer) -> Result<usize> {
        let instr = 0b11111001;
        super::serialise(&[0, instr, self.address, self.address ^ instr], buf)
    }
}

/// Builder for DecoderLock packet
#[derive(Default)]
pub struct DecoderLockBuilder {
    address: Option<u8>,
}

impl DecoderLockBuilder {
    /// Set the address of the DecoderLock packet. Only short-mode (i.e. 7-bit)
    /// addresses are supported. Returns an `Error::InvalidAddress` if the
    /// supplied address does not fit into 7 bits.
    pub fn address(&mut self, address: u8) -> Result<&mut Self> {
        if address < 0x7f {
            self.address = Some(address);
            Ok(self)
        } else {
            Err(Error::InvalidAddress)
        }
    }

    /// Build the DecoderLock packet. Returns `Error::MissingField` if the
    /// address has not been set
    pub fn build(&mut self) -> Result<DecoderLock> {
        Ok(DecoderLock {
            address: self.address.ok_or(Error::MissingField)?,
        })
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::packets::test::print_chunks;
    use bitvec::prelude::*;

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

    #[test]
    fn serialise_address_only_packet() {
        // [    preamble    ] S 0111 C000 S 0DDD DDDD S EEEE EEEE S
        // 1111 1111 1111 111_0 0111 1000 0 0011 1011 0 0100 0011 1
        let pkt = AddressOnly::write(59).unwrap();

        let mut buf = SerialiseBuffer::default();
        let len = pkt.serialise(&mut buf).unwrap();
        assert_eq!(len, 43);

        #[allow(clippy::unusual_byte_groupings)]
        let expected_arr = &[
            0b1111_1111_u8, // PPPP PPPP
            0b1111_111_0,   // PPPP PPPS
            0b0111_1000,    // 0111 C000
            0b0_0011_101,   // S0DD DDDD
            0b1_0_01_0000,  // DSEE EEEE
            0b11_1_0_0000,  // EES- ----
        ]
        .view_bits::<Msb0>()[..len];
        let mut expected = SerialiseBuffer::default();
        expected[..43].copy_from_bitslice(expected_arr);

        println!("Got:");
        print_chunks(&buf, 43);
        println!("Expected:");
        print_chunks(&expected, 43);
        assert_eq!(buf[..len], expected[..43]);
    }

    #[test]
    fn serialise_physical_register_packet() {
        // [    preamble    ] S 0111 CRRR S DDDD DDDD S EEEE EEEE S
        // 1111 1111 1111 111_0 0111 1101 0 1010 1010 0 1101 0111 1
        let pkt = PhysicalRegister::builder()
            .operation(Operation::Write)
            .register(6)
            .unwrap()
            .value(0xaa)
            .build()
            .unwrap();

        let mut buf = SerialiseBuffer::default();
        let len = pkt.serialise(&mut buf).unwrap();
        assert_eq!(len, 43);

        #[allow(clippy::unusual_byte_groupings)]
        let expected_arr = &[
            0b1111_1111_u8, // PPPP PPPP
            0b1111_111_0,   // PPPP PPPS
            0b0111_1_101,   // 0111 CRRR
            0b0_1010_101,   // SDDD DDDD
            0b0_0_11_0101,  // DSEE EEEE
            0b11_1_0_0000,  // EES- ----
        ]
        .view_bits::<Msb0>()[..len];
        let mut expected = SerialiseBuffer::default();
        expected[..43].copy_from_bitslice(expected_arr);

        println!("Got:");
        print_chunks(&buf, 43);
        println!("Expected:");
        print_chunks(&expected, 43);
        assert_eq!(buf[..len], expected[..43]);
    }
}
