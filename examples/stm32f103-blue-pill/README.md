# STM32F103 "Blue Pill" example
This is an example DCC base station running on an STM32F103 "Blue Pill" board.
It sends speed control packets addressed to loco 2 to a motor shield connected
to GPIO A0. Any GPIO pin can be used, as this implementation is a simple "bit-
banging" one.

To run this example, connect your blue pill board with an STLink programmer,
install `probe-run`, and run with `cargo run --release`:
```bash
cargo install probe-run
cargo run --release
```

The `run` command is configured in `.cargo/config` to use `probe-run`.
Diagnostic information is relayed to the host computer with `defmt`.