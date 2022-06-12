#![cfg_attr(not(test), no_std)]

use bitvec::prelude::*;
use embedded_hal::digital::blocking::ToggleableOutputPin;

const BUFFER_SIZE: usize = 24;
type BufferType = BitArr!(for 24*8, in u8, Msb0);

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum Error {
    TooLong,
}

pub struct DccInterruptHandler<P: ToggleableOutputPin> {
    write_buffer: [u8; BUFFER_SIZE],
    write_buffer_len: usize,
    buffer: BufferType,
    buffer_num_bits: usize,
    buffer_position: usize,
    second_half_of_bit: bool,
    one_clocks: usize,
    zero_clocks: usize,
    output_pin: P,
}

impl<P: ToggleableOutputPin> DccInterruptHandler<P> {
    pub fn new(output_pin: P, one_clocks: usize, zero_clocks: usize) -> Self {
        Self {
            write_buffer: [0; BUFFER_SIZE],
            write_buffer_len: 0,
            buffer: BitArray::default(),
            buffer_num_bits: 0,
            buffer_position: 0,
            second_half_of_bit: false,
            one_clocks,
            zero_clocks,
            output_pin,
        }
    }

    /// Run on interrupt; returns the new clock count to set the interrupt to
    #[inline(always)]
    pub fn tick(&mut self) -> Result<usize, P::Error> {
        #[cfg(test)]
        {
            eprintln!("[tick] DCC state:");
            eprintln!(
                "  write_buffer: (len {}) {:?}",
                self.write_buffer_len, self.write_buffer
            );
            eprintln!(
                "  buffer: (len {}, position {}) {:?}",
                self.buffer_num_bits, self.buffer_position, self.buffer
            );
            eprintln!("  second half of bit: {}", self.second_half_of_bit);
            eprintln!(
                "  num clocks: zero={}, one={}",
                self.zero_clocks, self.one_clocks
            );
        }

        // "do nothing" if nothing to send
        if self.buffer_num_bits == 0 {
            if self.write_buffer_len == 0 {
                return Ok(self.one_clocks);
            } else {
                // copy write buffer into internal buffer
                self.buffer
                    .copy_from_bitslice(self.write_buffer.view_bits());
                self.buffer_num_bits = self.write_buffer_len * 8;
            }
        }

        // send one bit
        self.output_pin.toggle().unwrap();

        let new_clock = if *self.buffer.get(self.buffer_position).unwrap() {
            #[cfg(test)]
            eprintln!("ONE");
            self.one_clocks
        } else {
            #[cfg(test)]
            eprintln!("ZERO");
            self.zero_clocks
        };

        if self.second_half_of_bit {
            // advance the current bit by one
            #[cfg(test)]
            eprintln!("Second half: advance position");

            self.buffer_position += 1;
            if self.buffer_position == self.buffer_num_bits {
                self.buffer_position = 0;
            }
        }
        self.second_half_of_bit = !self.second_half_of_bit;

        Ok(new_clock)
    }

    /// Stage a packet for transmission
    pub fn write(&mut self, buf: &[u8]) -> Result<(), Error> {
        if buf.len() > BUFFER_SIZE {
            Err(Error::TooLong)
        } else {
            self.write_buffer[0..buf.len()].copy_from_slice(buf);
            self.write_buffer_len = buf.len();
            #[cfg(test)]
            eprintln!("Written {} bytes to write buffer", buf.len());
            Ok(())
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use embedded_hal::digital::{blocking::*, ErrorType};
    use std::convert::Infallible;

    #[derive(Default)]
    struct MockPin {
        state: bool,
    }

    impl ErrorType for MockPin {
        type Error = Infallible;
    }

    impl OutputPin for MockPin {
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

    impl ToggleableOutputPin for MockPin {
        #[inline(always)]
        fn toggle(&mut self) -> Result<(), Self::Error> {
            self.state = !self.state;
            Ok(())
        }
    }

    #[test]
    fn mock_pin_works() {
        let mut pin = MockPin::default();
        assert!(pin.is_set_low().unwrap());
        pin.set_high().unwrap();
        assert!(pin.is_set_high().unwrap());
        pin.toggle().unwrap();
        assert!(pin.is_set_low().unwrap());
    }

    #[test]
    fn send_a_packet() {
        const ONE: usize = 100;
        const ZERO: usize = 58;
        let pin = MockPin::default();
        let mut dcc = DccInterruptHandler::new(pin, ONE, ZERO);
        let buffer = [0x00, 0xff];
        dcc.write(&buffer).unwrap();

        // output should probably loop over write buffer
        for _ in 0..2 {
            // run 32 ticks to make sure that the clock settings are correct
            // (2 ticks per bit)
            // 16 ticks are one
            for _ in 0..16 {
                let new_delay = dcc.tick().unwrap();
                assert_eq!(new_delay, ZERO);
            }

            // 16 ticks are zero
            for _ in 0..16 {
                let new_delay = dcc.tick().unwrap();
                assert_eq!(new_delay, ONE);
            }
        }
    }
}
