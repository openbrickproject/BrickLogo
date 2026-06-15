//! Wire-level constants for the LEGO Dimensions ToyPad (USB HID).
//!
//! Ported from the reference implementation `node-toypad` (`src/constants.ts`).
//! See `dev/SOURCES.md` for the protocol references and decisions.

/// USB vendor id of the LEGO Dimensions ToyPad (Gen 1).
pub const VENDOR_ID: u16 = 0x0E6F;
/// USB product id of the LEGO Dimensions ToyPad (Gen 1).
pub const PRODUCT_ID: u16 = 0x0241;

/// Every ToyPad packet, command or event, is exactly 32 bytes.
pub const PACKET_LENGTH: usize = 32;

/// Init/handshake packet. The ToyPad ignores all commands until this is
/// written once after the HID device is opened (analogous to the BrickInterface
/// DTR assertion). The ASCII tail reads `(c) LEGO 2014`.
pub const WAKE_SEQUENCE: [u8; PACKET_LENGTH] = [
    0x55, 0x0f, 0xb0, 0x01, 0x28, 0x63, 0x29, 0x20, 0x4c, 0x45, 0x47, 0x4f, 0x20, 0x32, 0x30, 0x31,
    0x34, 0xf7, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
];

/// First byte of an incoming frame: command response vs. unsolicited event.
pub const MSG_RESPONSE: u8 = 0x55;
pub const MSG_EVENT: u8 = 0x56;

/// Second byte of an event frame identifies a tag add/remove action.
pub const CMD_ACTION: u8 = 0x0b;

/// Outgoing command ids (request type).
pub const REQ_SET_COLOR: u8 = 0xc0;
pub const REQ_GET_COLOR: u8 = 0xc1;
pub const REQ_FADE: u8 = 0xc2;
pub const REQ_FLASH: u8 = 0xc3;
pub const REQ_FADE_ALL: u8 = 0xc6;
pub const REQ_FLASH_ALL: u8 = 0xc7;
pub const REQ_SET_COLOR_ALL: u8 = 0xc8;
pub const REQ_LIST_TAGS: u8 = 0xd0;
pub const REQ_READ_TAG: u8 = 0xd2;
pub const REQ_WRITE_TAG: u8 = 0xd3;

/// The NFC page holding the character/vehicle identity record.
pub const PAGE_IDENTITY: u8 = 0x24;

/// Pad positions. `All` (0) is only meaningful for output (set-all commands);
/// tag events always carry a specific pad.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum Panel {
    All = 0,
    Center = 1,
    Left = 2,
    Right = 3,
}

impl Panel {
    /// Map a raw panel byte to a `Panel`, defaulting unknown values to `All`
    /// (mirrors node-toypad's `normalizePanel`).
    pub fn from_byte(value: u8) -> Panel {
        match value {
            1 => Panel::Center,
            2 => Panel::Left,
            3 => Panel::Right,
            _ => Panel::All,
        }
    }
}

/// Tag placement action carried by an event frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Action {
    Add = 0,
    Remove = 1,
}

impl Action {
    pub fn from_byte(value: u8) -> Action {
        if value == 1 { Action::Remove } else { Action::Add }
    }
}
