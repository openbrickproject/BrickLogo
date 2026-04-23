//! STM32 DfuSe parser and USB DFU 1.1a + DfuSe class client.
//!
//! Two layers:
//!
//!   - [`dfuse`] — parse (and optionally gunzip) an ST "DfuSe" firmware file.
//!   - [`dfu`]   — drive a device in the STM32 DFU bootloader (VID `0x0483`
//!     PID `0xDF11`) through erase + write + manifest. Hardware access goes
//!     through a small [`dfu::DfuTransport`] trait so the state machine is
//!     unit-testable without USB.

pub mod dfu;
pub mod dfuse;

use std::fmt;

#[derive(Debug)]
pub enum Error {
    Io(std::io::Error),
    Usb(rusb::Error),
    Parse(String),
    /// The device reported a DFU error status. `status` is the `bStatus` byte
    /// from `DFU_GETSTATUS`; `state` is the `bState` byte.
    DfuStatus { status: u8, state: u8 },
    Timeout,
    NotFound,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Io(e) => write!(f, "i/o: {}", e),
            Error::Usb(e) => write!(f, "usb: {}", e),
            Error::Parse(msg) => write!(f, "parse: {}", msg),
            Error::DfuStatus { status, state } => {
                write!(f, "dfu error (status={:#04x}, state={:#04x})", status, state)
            }
            Error::Timeout => write!(f, "timeout"),
            Error::NotFound => write!(f, "device not found"),
        }
    }
}

impl std::error::Error for Error {}

impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Self { Error::Io(e) }
}

impl From<rusb::Error> for Error {
    fn from(e: rusb::Error) -> Self { Error::Usb(e) }
}

pub type Result<T> = std::result::Result<T, Error>;

/// Progress callback. Arguments are `(done, total, phase)` where `phase` is a
/// short word like "erasing" or "writing".
pub type ProgressFn = Box<dyn Fn(usize, usize, &str) + Send>;
