#![cfg_attr(not(test), no_std)]

use bitvec::prelude::*;
use embedded_hal::digital::v2::OutputPin;

pub mod packets;

const BUFFER_SIZE: usize = 24 * 8;
type BufferType = BitArr!(for 24*8, in u8, Msb0);

const IDLE_MICROS: u32 = 500;

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum Error {
    TooLong,
    InvalidAddress,
    InvalidSpeed,
}

pub struct DccInterruptHandler<P: OutputPin> {
    write_buffer: BufferType,
    write_buffer_len: usize,
    buffer: BufferType,
    buffer_num_bits: usize,
    buffer_position: usize,
    second_half_of_bit: bool,
    one_micros: u32,
    zero_micros: u32,
    output_pin: P,
}

impl<P: OutputPin> DccInterruptHandler<P> {
    pub fn new(output_pin: P, zero_micros: u32, one_micros: u32) -> Self {
        Self {
            write_buffer: BitArray::default(),
            write_buffer_len: 0,
            buffer: BitArray::default(),
            buffer_num_bits: 0,
            buffer_position: 0,
            second_half_of_bit: false,
            one_micros,
            zero_micros,
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
                self.write_buffer_len, self.write_buffer
            );
            eprintln!(
                "  buffer: (len {}, position {}) {:?}",
                self.buffer_num_bits, self.buffer_position, self.buffer
            );
            eprintln!("  second half of bit: {}", self.second_half_of_bit);
            eprintln!(
                "  num clocks: zero={}, one={}",
                self.zero_micros, self.one_micros
            );
        }

        // "do nothing" if nothing to send
        if self.buffer_num_bits == 0 {
            if self.write_buffer_len == 0 {
                return Ok(IDLE_MICROS);
            } else {
                // copy write buffer into internal buffer
                self.buffer.copy_from_bitslice(&self.write_buffer);
                self.buffer_num_bits = self.write_buffer_len * 8;
            }
        }

        // send one bit
        if self.second_half_of_bit {
            self.output_pin.set_high()?;
        } else {
            self.output_pin.set_low()?;
        }

        let mut new_clock = if *self.buffer.get(self.buffer_position).unwrap() {
            #[cfg(test)]
            eprintln!("ONE");
            self.one_micros
        } else {
            #[cfg(test)]
            eprintln!("ZERO");
            self.zero_micros
        };

        if self.second_half_of_bit {
            // advance the current bit by one
            #[cfg(test)]
            eprintln!("Second half: advance position");

            self.buffer_position += 1;
            if self.buffer_position == self.buffer_num_bits {
                self.buffer_position = 0;
                // if end of packet then wait for longer time
                new_clock = IDLE_MICROS;
            }
        }
        self.second_half_of_bit = !self.second_half_of_bit;

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
            eprintln!("Written {} bytes to write buffer", buf.len());
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
        let mut dcc = DccInterruptHandler::new(pin, ONE, ZERO);
        let buffer = [0x00, 0xff].view_bits();
        dcc.write(buffer).unwrap();

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
            for _ in 0..15 {
                let new_delay = dcc.tick().unwrap();
                assert_eq!(new_delay, ONE);
            }

            // final delay is a bit longer for a gap between packets
            let new_delay = dcc.tick().unwrap();
            assert_eq!(new_delay, 5000);
        }
    }
}
