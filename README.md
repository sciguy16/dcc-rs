# dcc-rs
[![Crates.io](https://img.shields.io/crates/v/dcc-rs)](https://crates.io/dcc-rs)
[![docs.rs](https://img.shields.io/docsrs/dcc-rs)](https://docs.rs/dcc-rs)

Pure-Rust implementation of NMRA Digital Command Control

This crate implements the NMRA DCC protocol, intended for embedded use on e.g.
custom base stations for computer control. It is fully `no_std`-compatible,
with zero allocations and uses the `embedded_hal` traits for driving output
pins.

To work around the lack of standardised interrupt support, this crate provides
a `DccInterruptHandler` struct which may be owned by a `static` within an
interrupt handler, and the `DccInterruptHandler::tick` method performs all of
the necessary processing, returning the number of microseconds to wait before
it should be called again. This timing is critical to the correct functioning
of this crate as it is used to time the output pin transitions.

Getting new data packets into the `DccInterruptHandler` is left as an exercise
for the implementor. In the provided example code, it is done via a `Mutex`
holding a `RefCell<Option<_>>`, which allows external code to pop new serialised
packets in for the interrupt handler to retrieve at its leisure.

## Status
This crate currently only implements the base station (transmitter) side, and
only the "baseline" and "service mode" packets. "Extended format" DCC packets
are a work in progress.

DCC receiving is under future work, once the main packet types have been
implemented.

## Example implementations
* [examples/stm32f103-blue-pill](examples/stm32f103-blue-pill) Single-channel
speed controller

## Contributing
Contributions are welcome, in the form of code improvements, testing reports,
more example implementations, more platform implementations, etc. I'm
particularly interested in getting this running on an arduino uno/nano once
the loose ends in Rust's AVR support have been tidied up. A drop-in replacement
`DCC++`-compatible firmware would also be neat.

## Licence
This crate is available under the terms of the Mozilla Public Licence Version
2.0.
