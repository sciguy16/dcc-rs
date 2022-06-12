// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
#![doc = include_str!("../README.md")]
#![cfg_attr(not(test), no_std)]
#![deny(missing_docs)]

pub use bitvec;
use bitvec::prelude::*;
use embedded_hal::digital::v2::OutputPin;

pub mod packets;

const BUFFER_SIZE: usize = 24 * 8;
type BufferType = BitArr!(for 24*8, in u8, Msb0);
const ZERO_MICROS: u32 = 100;
const ONE_MICROS: u32 = 58;

/// Error types returned by this crate
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum Error {
    /// Data packet was too long for the internal buffer
    TooLong,
    /// Not a valid short-mode DCC address (must be in range 1-126)
    InvalidAddress,
    /// Not a valid short-mode DCC speed (must be in range 0-16)
    InvalidSpeed,
}

#[derive(Debug)]
enum TxState {
    Idle {
        second_half_of_bit: bool,
    },
    Transmitting {
        offset: usize,
        second_half_of_bit: bool,
    },
}

/// The main interrupt handler. Calling the `tick` method advances the
/// internal state and toggles the provided output pin to control the
/// track polarity
pub struct DccInterruptHandler<P: OutputPin> {
    write_buffer: BufferType,
    write_buffer_len: usize,
    buffer: BufferType,
    buffer_num_bits: usize,
    state: TxState,
    output_pin: P,
}

impl<P: OutputPin> DccInterruptHandler<P> {
    /// Initialise the interrupt handler. `output_pin` is the GPIO pin
    /// connected to e.g. a motor shield's `direction` pin to control the
    /// track polarity.
    pub fn new(output_pin: P) -> Self {
        Self {
            write_buffer: BitArray::default(),
            write_buffer_len: 0,
            buffer: BitArray::default(),
            buffer_num_bits: 0,
            state: TxState::Idle {
                second_half_of_bit: false,
            },
            output_pin,
        }
    }

    /// Run on interrupt; returns the new clock count to set the interrupt to
    #[inline(always)]
    pub fn tick(&mut self) -> Result<u32, P::Error> {
        #[cfg(test)]
        {
            eprintln!("[tick] DCC state:");
            eprintln!(
                "  write_buffer: (len {}) {:?}",
                self.write_buffer_len,
                &self.write_buffer[..self.write_buffer_len]
            );
            eprintln!("  state {:?}", self.state,);
        }

        let new_clock;
        self.state = match self.state {
            TxState::Idle { second_half_of_bit } => {
                // transmit a zero
                if second_half_of_bit {
                    self.output_pin.set_high()?;
                } else {
                    self.output_pin.set_low()?;
                }
                new_clock = ZERO_MICROS;

                if second_half_of_bit && self.write_buffer_len != 0 {
                    // copy write buffer into internal buffer
                    self.buffer.copy_from_bitslice(&self.write_buffer);
                    self.buffer_num_bits = self.write_buffer_len;
                    self.write_buffer_len = 0;
                    #[cfg(test)]
                    eprintln!("Loaded new data into tx buffer");

                    TxState::Transmitting {
                        offset: 0,
                        second_half_of_bit: false,
                    }
                } else {
                    TxState::Idle {
                        second_half_of_bit: !second_half_of_bit,
                    }
                }
            }
            TxState::Transmitting {
                mut offset,
                second_half_of_bit,
            } => {
                // transmit the next bit-half in the sequence
                let current_bit = *self.buffer.get(offset).unwrap();

                new_clock = if current_bit { ONE_MICROS } else { ZERO_MICROS };

                if second_half_of_bit {
                    self.output_pin.set_high()?;
                    // increment offset
                    offset += 1;
                } else {
                    self.output_pin.set_low()?;
                }

                // if there is remaining data then continue transmitting,
                // otherwise go back to Idle mode
                if offset < self.buffer_num_bits {
                    TxState::Transmitting {
                        offset,
                        second_half_of_bit: !second_half_of_bit,
                    }
                } else {
                    TxState::Idle {
                        second_half_of_bit: false,
                    }
                }
            }
        };

        Ok(new_clock)
    }

    /// Stage a packet for transmission
    pub fn write(&mut self, buf: &BitSlice<u8, Msb0>) -> Result<(), Error> {
        if buf.len() > BUFFER_SIZE {
            Err(Error::TooLong)
        } else {
            self.write_buffer[0..buf.len()].copy_from_bitslice(buf);
            self.write_buffer_len = buf.len();
            #[cfg(test)]
            eprintln!("Written {} bits to write buffer", buf.len());
            Ok(())
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use embedded_hal::digital::v2::*;
    use std::convert::Infallible;

    #[derive(Default)]
    struct MockPin {
        state: bool,
    }

    impl OutputPin for MockPin {
        type Error = Infallible;

        #[inline(always)]
        fn set_high(&mut self) -> Result<(), Self::Error> {
            self.state = true;
            Ok(())
        }

        #[inline(always)]
        fn set_low(&mut self) -> Result<(), Self::Error> {
            self.state = false;
            Ok(())
        }
    }

    impl StatefulOutputPin for MockPin {
        #[inline(always)]
        fn is_set_high(&self) -> Result<bool, Self::Error> {
            Ok(self.state)
        }

        #[inline(always)]
        fn is_set_low(&self) -> Result<bool, Self::Error> {
            Ok(!self.state)
        }
    }

    #[test]
    fn mock_pin_works() {
        let mut pin = MockPin::default();
        assert!(pin.is_set_low().unwrap());
        pin.set_high().unwrap();
        assert!(pin.is_set_high().unwrap());
        pin.set_low().unwrap();
        assert!(pin.is_set_low().unwrap());
    }

    #[test]
    fn send_a_packet() {
        const ONE: u32 = 100;
        const ZERO: u32 = 58;
        let pin = MockPin::default();
        let mut dcc = DccInterruptHandler::new(pin, ZERO, ONE);
        let buffer = [0x00, 0xff].view_bits();
        dcc.write(buffer).unwrap();

        // first two ticks are idle
        for _ in 0..2 {
            let new_delay = dcc.tick().unwrap();
            eprintln!("new delay: {new_delay}");
            assert_eq!(new_delay, 500);
        }

        // run 32 ticks to make sure that the clock settings are correct
        // (2 ticks per bit)
        // 16 ticks are one
        for _ in 0..16 {
            let new_delay = dcc.tick().unwrap();
            eprintln!("new delay: {new_delay}");
            assert_eq!(new_delay, ZERO);
        }

        // 16 ticks are zero
        for _ in 0..16 {
            let new_delay = dcc.tick().unwrap();
            eprintln!("new delay: {new_delay}");
            assert_eq!(new_delay, ONE);
        }

        // after packet is finished we just have idle zeroes
        for _ in 0..8 {
            let new_delay = dcc.tick().unwrap();
            eprintln!("new delay: {new_delay}");
            assert_eq!(new_delay, 500);
        }
    }
}
