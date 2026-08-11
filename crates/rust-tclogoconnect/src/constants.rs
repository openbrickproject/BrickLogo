// TC Logo Connect protocol constants.
//
// The TC Logo Connect is a USB-CDC serial bridge (Seeed XIAO RP2040, stock
// USB identity — no custom VID/PID) to a LEGO Interface A (9750): six on/off
// outputs (ports 0-5) and two touch-sensor inputs (ports 6, 7). It speaks a
// plain ASCII line protocol rather than the BrickInterface framed binary
// protocol.
//
// Protocol reference: https://www.tc-logo.com/protocol/

/// 115200 baud, 8N1, no flow control. NEVER open at 1200 baud — the RP2040
/// firmware treats that as a request to reboot into the bootloader.
pub const BAUD_RATE: u32 = 115_200;

/// How long to wait for a probe (`R` → `CF`) or version (`V`) reply.
pub const PROBE_TIMEOUT_MS: u64 = 500;

/// Only the low 6 bits of a `D` frame are wired to outputs 0-5.
pub const OUTPUT_MASK: u8 = 0x3F;
pub const OUTPUT_COUNT: usize = 6;

/// Input report bit assignments (`Ixx`): bit set = sensor pressed.
pub const INPUT_BIT_PORT6: u8 = 0x40;
pub const INPUT_BIT_PORT7: u8 = 0x80;

/// The device buffers at most 15 characters per line; longer lines are
/// silently truncated. A `D` frame (`Dxx\n`, 4 bytes) is always well under
/// this, so BrickLogo never needs to check it — noted here as a constraint
/// on any future command added to this protocol.
pub const MAX_LINE_CHARS: usize = 15;

/// If the device receives nothing for this long, it forces all outputs off
/// and clears its D-frame replay queue. Any received byte resets the timer.
pub const WATCHDOG_MS: u64 = 500;

/// The device replays queued `D` frames at exactly one per millisecond
/// (1kHz) in a 64-entry queue; sustained throughput above this overflows
/// the queue (oldest entries dropped).
pub const REPLAY_RATE_HZ: u32 = 1000;
pub const REPLAY_QUEUE_LEN: usize = 64;

// ── Host commands ────────────────────────────────
pub const CMD_VERSION: &[u8] = b"V\n";
pub const CMD_STATS: &[u8] = b"T\n";
pub const CMD_BOOT: &[u8] = b"B\n";
pub const CMD_PROBE: &[u8] = b"R\n";

// ── Expected replies ─────────────────────────────
pub const REPLY_PROBE: &str = "CF";
pub const REPLY_BOOT: &str = "BOOT";
