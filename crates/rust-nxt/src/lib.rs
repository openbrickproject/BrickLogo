//! LEGO Mindstorms NXT (1.0 / 2.0) protocol library.
//!
//! Speaks the brick's stock LCP (LEGO Communication Protocol / Direct
//! Commands) over USB bulk or Bluetooth SPP serial. The transport layer
//! hides the framing difference between the two — USB exchanges raw LCP
//! bytes, Bluetooth wraps every packet in a 2-byte little-endian length
//! prefix — so callers above `Transport` never care.

pub mod constants;
pub mod nxt;
pub mod protocol;
pub mod serial;
pub mod transport;
pub mod usb;
