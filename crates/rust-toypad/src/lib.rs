//! LEGO Dimensions ToyPad protocol library (USB HID).
//!
//! Pure protocol: framing/decoding ([`protocol`]), tag identity decryption
//! ([`tag`]), and a thin device wrapper ([`toypad`]). No dependency on the
//! BrickLogo language or HAL. See `dev/SOURCES.md` for protocol references.

pub mod constants;
pub mod protocol;
pub mod tag;
pub mod toypad;

pub use constants::{Action, Panel};
pub use protocol::{
    create_fade, create_fade_all, create_list_tags, create_read_tag, create_set_color,
    create_set_color_all, decode_list_tags, decode_message, encode_command, format_uid, Command,
    FadeSpec, Incoming, ListEntry, TagEvent,
};
pub use tag::{detect_tag_type, get_character_id, get_vehicle_id, identify, Identity, TagType};
pub use toypad::{toypad_usb_present, ToyPad, ToyPadDeviceInfo};
