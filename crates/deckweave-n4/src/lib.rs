//! N4 transport, independent of desktop/UI and action execution.
pub mod input;
#[cfg(windows)]
mod native;
pub mod packets;
pub mod render;
pub use input::{Input, decode};
#[cfg(windows)]
pub use native::{Candidate, Connection, discover};
pub use render::{KeyVisual, RegionVisual, Renderer};
