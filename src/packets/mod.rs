// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Modules containing packet definitions

pub mod baseline;
pub mod extended;
pub mod service_mode;

pub use baseline::*;
pub use extended::*;
pub use service_mode::*;

use crate::Error;
use bitvec::prelude::*;

/// Convenient Result wrapper
pub type Result<T> = core::result::Result<T, Error>;

struct Preamble(BitArr!(for 14, in u8, Msb0));

const MAX_BITS: usize = 43;
/// Buffer long enough to serialise any common DCC packet into
pub type SerialiseBuffer = BitArr!(for MAX_BITS, in u8, Msb0);

/// TODO use this method for all serialisations. Should be less error-prone
/// than all of the manual bit offsets we implemented in baseline.
fn serialise(data: &[u8], buf: &mut SerialiseBuffer) -> Result<usize> {
    // check that the provided data will fit into the buffer
    let required_bits = 15 + data.len() * 9 + 1;
    if required_bits > MAX_BITS {
        return Err(Error::TooLong);
    }

    buf[0..16].copy_from_bitslice([0xff, 0xfe].view_bits::<Msb0>()); // preamble

    let mut pos: usize = 15;
    for byte in data {
        buf.set(pos, false); // start bit
        pos += 1;
        buf[pos..pos + 8].copy_from_bitslice([*byte].view_bits::<Msb0>());
        pos += 8;
    }

    buf.set(pos, true); // stop bit
    pos += 1;

    Ok(pos)
}
