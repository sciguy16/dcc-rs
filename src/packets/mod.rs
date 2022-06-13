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

/// Buffer long enough to serialise any common DCC packet into
pub type SerialiseBuffer = BitArr!(for 43, in u8, Msb0);

/// TODO use this method for all serialisations. Should be less error-prone
/// than all of the manual bit offsets we implemented in baseline.
fn serialise(data: &[u8], buf: &mut SerialiseBuffer) -> Result<usize> {
    todo!()
}
