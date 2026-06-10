// ── Host → device commands ────────────────────────────────────────────────────

pub const CMD_PING: u8 = 0x01;
pub const CMD_GET_VERSION: u8 = 0x02;
pub const CMD_GET_CAPABILITIES: u8 = 0x03;
pub const CMD_RESET_STATE: u8 = 0x04;
pub const CMD_ENTER_BOOTLOADER: u8 = 0x05;

pub const CMD_IFACE_SET_OUTPUTS: u8 = 0x10;
pub const CMD_IFACE_GET_INPUTS: u8 = 0x11;
pub const CMD_IFACE_GET_COUNTS: u8 = 0x12;
pub const CMD_IFACE_RESET_COUNT: u8 = 0x13;

pub const CMD_PF_SEND: u8 = 0x20;
pub const PF_MODE_COMBO_DIRECT: u8 = 0x00;
pub const PF_MODE_SINGLE_PWM: u8 = 0x01;
pub const PF_MODE_SINGLE_CST: u8 = 0x02;
pub const PF_MODE_COMBO_PWM: u8 = 0x03;
pub const CMD_LEGACY_SEND: u8 = 0x30;
pub const CMD_IR_ABORT_ALL: u8 = 0x40;

// ── Device → host replies ─────────────────────────────────────────────────────

pub const REPLY_PONG: u8 = 0x81;
pub const REPLY_VERSION: u8 = 0x82;
pub const REPLY_CAPABILITIES: u8 = 0x83;
pub const REPLY_OK: u8 = 0x84;
pub const REPLY_IFACE_INPUTS: u8 = 0x90;
pub const REPLY_IFACE_COUNTS: u8 = 0x91;
pub const REPLY_IR_ACCEPTED: u8 = 0xA0;
pub const REPLY_IR_DONE: u8 = 0xA1;
pub const REPLY_ERROR: u8 = 0xE0;

// ── Capability bits ───────────────────────────────────────────────────────────

pub const CAP_INTERFACE_A: u16 = 0x0001;
pub const CAP_PF_IR: u16 = 0x0002;
pub const CAP_LEGACY_IR: u16 = 0x0004;
pub const CAP_IR_DONE_EVENTS: u16 = 0x0010;
