#![cfg_attr(not(test), no_std)]

use bitvec::prelude::*;
use embedded_hal::digital::blocking::ToggleableOutputPin;

const BUFFER_SIZE: usize = 24;
type BufferType = BitArr!(for 24*8, in usize, Msb0);

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum Error {
    TooLong,
}

pub struct DccInterruptHandler<P: ToggleableOutputPin> {
    write_buffer: [u8; BUFFER_SIZE],
    write_buffer_len: usize,
    buffer: BufferType,
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
            one_clocks,
            zero_clocks,
            output_pin,
        }
    }

    /// Run on interrupt; returns the new clock count to set the interrupt to
    #[inline(always)]
    pub fn tick(&mut self) -> usize {
        self.one_clocks
    }

    /// Stage a packet for transmission
    pub fn write(&mut self, buf: &[u8]) -> Result<(), Error> {
        if buf.len() > BUFFER_SIZE {
            Err(Error::TooLong)
        } else {
            self.write_buffer[0..buf.len()].copy_from_slice(buf);
            self.write_buffer_len = buf.len();
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
        const ONE: usize = 1;
        const ZERO: usize = 2;
        let pin = MockPin::default();
        let mut dcc = DccInterruptHandler::new(pin, ONE, ZERO);
        let buffer = [0x00, 0xff];
        dcc.write(&buffer).unwrap();

        // run 32 ticks to make sure that the clock settings are correct
        // (2 ticks per bit)
        // 16 ticks are one
        for _ in 0..16 {
            let new_delay = dcc.tick();
            assert_eq!(new_delay, ZERO);
        }

        // 16 ticks are zero
        for _ in 0..16 {
            let new_delay = dcc.tick();
            assert_eq!(new_delay, ONE);
        }
    }
}
