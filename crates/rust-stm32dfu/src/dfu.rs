//! STM32 DFU 1.1a + DfuSe class client.
//!
//! All hardware access goes through [`DfuTransport`], so the download state
//! machine is unit-testable without USB. [`RusbTransport`] is the real
//! implementation.
//!
//! DFU class requests use bmRequestType `0x21` (host→device, class, interface)
//! for downloads and `0xA1` for status reads.
//!
//! STM32's DfuSe extension repurposes `DFU_DNLOAD wBlock=0` as a command
//! channel:
//!   - `[0x21, addr_u32_le]` — set the next write address.
//!   - `[0x41, addr_u32_le]` — erase the page at `addr`.
//!
//! Normal data downloads use `wBlock >= 2`, incrementing per chunk.

use std::time::Duration;

use rusb::{DeviceHandle, GlobalContext};

use crate::dfuse::DfuSeFile;
use crate::{Error, ProgressFn, Result};

// ── USB identity ────────────────────────────────

/// LEGO System A/S USB vendor id. SPIKE Prime / MINDSTORMS Robot Inventor
/// hubs in DFU bootloader mode appear as "LEGO Technic Large Hub in DFU
/// Mode" under this VID (not the stock STMicro `0x0483:0xDF11`). BrickLogo
/// only flashes LEGO hubs — generic STM32 devices in DFU mode are ignored.
pub const LEGO_VID: u16 = 0x0694;

// ── DFU class requests ──────────────────────────

const DFU_DNLOAD: u8 = 1;
const DFU_GETSTATUS: u8 = 3;
const DFU_CLRSTATUS: u8 = 4;
const DFU_ABORT: u8 = 6;

const REQ_OUT: u8 = 0x21;
const REQ_IN: u8 = 0xA1;

// ── DFU states ──────────────────────────────────

const DFU_STATE_IDLE: u8 = 2;
const DFU_STATE_DNBUSY: u8 = 4;
const DFU_STATE_DNLOAD_IDLE: u8 = 5;
const DFU_STATE_MANIFEST_SYNC: u8 = 6;
const DFU_STATE_MANIFEST: u8 = 7;
const DFU_STATE_MANIFEST_WAIT_RESET: u8 = 8;
const DFU_STATE_ERROR: u8 = 10;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DfuStatus {
    pub status: u8,
    pub poll_timeout_ms: u32,
    pub state: u8,
}

impl DfuStatus {
    fn parse(buf: &[u8]) -> Result<Self> {
        if buf.len() < 6 {
            return Err(Error::Parse("GET_STATUS returned fewer than 6 bytes".into()));
        }
        Ok(DfuStatus {
            status: buf[0],
            poll_timeout_ms: (buf[1] as u32) | ((buf[2] as u32) << 8) | ((buf[3] as u32) << 16),
            state: buf[4],
        })
    }
}

// ── Transport seam ──────────────────────────────

/// Minimal USB control-transfer surface. Production: [`RusbTransport`].
/// Tests: scripted mock.
pub trait DfuTransport {
    fn control_out(&mut self, request: u8, value: u16, index: u16, data: &[u8]) -> Result<usize>;
    fn control_in(&mut self, request: u8, value: u16, index: u16, length: u16) -> Result<Vec<u8>>;
}

// ── Memory layout parser ────────────────────────

/// Contiguous run of same-size sectors.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SectorRange {
    pub count: u32,
    pub size: u32,
    /// Flags letter from the interface string: `a` = read-only, `b`–`g` =
    /// various combinations of erase/write permissions. Only `writable`
    /// sectors are candidates for erase/download.
    pub writable: bool,
}

/// Flash memory map parsed from the DFU interface string descriptor.
///
/// STM32 bootloaders expose their flash layout in the form
/// `@Name/0x08000000/04*016Kg,01*064Kg,07*128Kg`. LEGO's custom bootloader
/// adds multiple regions at different base addresses and marks the
/// bootloader sectors read-only (`a` suffix):
/// `@LEGO LES HUB/0x08000000/02*016Ka,02*016Kg,01*064Kg,07*128Kg/0x10000000/01*1Ma/...`.
///
/// We parse every region and preserve the read-only flag so the downloader
/// can skip protected sectors (erasing them would either fail with a USB
/// stall or, worse, brick the hub).
#[derive(Debug, Clone, PartialEq)]
pub struct MemoryLayout {
    pub start: u32,
    pub regions: Vec<MemoryRegion>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MemoryRegion {
    pub start: u32,
    pub ranges: Vec<SectorRange>,
}

impl MemoryLayout {
    pub fn parse(desc: &str) -> Result<Self> {
        // Drop the "@<name>" prefix up to the first '/'.
        let (_, body) = desc
            .split_once('/')
            .ok_or_else(|| Error::Parse(format!("no '/' in memory layout: {:?}", desc)))?;
        // Body is a sequence of "/addr/spec" pairs. Tokens alternate: an
        // address token starts with "0x", a spec token contains '*'.
        let tokens: Vec<&str> = body.split('/').map(str::trim).filter(|s| !s.is_empty()).collect();

        let mut regions = Vec::new();
        let mut i = 0;
        while i < tokens.len() {
            let addr_tok = tokens[i];
            let hex = addr_tok
                .strip_prefix("0x")
                .or_else(|| addr_tok.strip_prefix("0X"))
                .ok_or_else(|| {
                    Error::Parse(format!("expected hex address, got {:?}", addr_tok))
                })?;
            let start = u32::from_str_radix(hex, 16)
                .map_err(|e| Error::Parse(format!("bad address {:?}: {}", addr_tok, e)))?;
            i += 1;
            let spec_tok = tokens.get(i).copied().unwrap_or("");
            i += 1;
            let ranges = parse_sector_spec(spec_tok)?;
            regions.push(MemoryRegion { start, ranges });
        }
        if regions.is_empty() {
            return Err(Error::Parse(format!("no regions in layout: {:?}", desc)));
        }
        let start = regions[0].start;
        Ok(MemoryLayout { start, regions })
    }

    /// Return the start addresses of every **writable** sector that overlaps
    /// the byte range `[start, end)`. Read-only sectors (e.g. LEGO's protected
    /// bootloader) are skipped.
    pub fn pages_in(&self, start: u32, end: u32) -> Vec<u32> {
        let mut out = Vec::new();
        for region in &self.regions {
            let mut cursor = region.start;
            for range in &region.ranges {
                for _ in 0..range.count {
                    let sector_start = cursor;
                    let sector_end = cursor.saturating_add(range.size);
                    if range.writable && sector_end > start && sector_start < end {
                        out.push(sector_start);
                    }
                    cursor = sector_end;
                }
            }
        }
        out
    }
}

fn parse_sector_spec(body: &str) -> Result<Vec<SectorRange>> {
    let mut ranges = Vec::new();
    for spec in body.split(',') {
        let spec = spec.trim();
        if spec.is_empty() {
            continue;
        }
        // Form: "<count>*<size><unit><flag>" e.g. "04*016Kg" or "02*016Ka".
        let (count_str, rest) = spec
            .split_once('*')
            .ok_or_else(|| Error::Parse(format!("missing '*' in sector spec: {:?}", spec)))?;
        let count: u32 = count_str
            .parse()
            .map_err(|e| Error::Parse(format!("bad count {:?}: {}", count_str, e)))?;
        let digits_end = rest
            .char_indices()
            .find(|(_, c)| !c.is_ascii_digit())
            .map(|(i, _)| i)
            .unwrap_or(rest.len());
        let (size_str, unit_rest) = rest.split_at(digits_end);
        let size_base: u32 = size_str
            .parse()
            .map_err(|e| Error::Parse(format!("bad size {:?}: {}", size_str, e)))?;
        let mut chars = unit_rest.chars();
        let unit = chars.next().unwrap_or(' ');
        let flag = chars.next().unwrap_or(' ');
        let multiplier = match unit {
            ' ' | '\0' => 1,
            'B' | 'b' => 1,
            'K' | 'k' => 1024,
            'M' | 'm' => 1024 * 1024,
            _ => return Err(Error::Parse(format!("unknown size unit {:?}", unit))),
        };
        // STM32 DFU flag letters: 'a' = read-only, others allow writes.
        let writable = !matches!(flag, 'a' | 'A');
        ranges.push(SectorRange { count, size: size_base * multiplier, writable });
    }
    Ok(ranges)
}

// ── DFU device ──────────────────────────────────

pub struct DfuDevice<T: DfuTransport> {
    transport: T,
    iface: u16,
    transfer_size: u16,
    layout: MemoryLayout,
}

impl<T: DfuTransport> DfuDevice<T> {
    pub fn new(transport: T, iface: u16, transfer_size: u16, layout: MemoryLayout) -> Self {
        DfuDevice { transport, iface, transfer_size, layout }
    }

    fn get_status(&mut self) -> Result<DfuStatus> {
        let bytes = self.transport.control_in(DFU_GETSTATUS, 0, self.iface, 6)?;
        DfuStatus::parse(&bytes)
    }

    fn clear_status(&mut self) -> Result<()> {
        self.transport.control_out(DFU_CLRSTATUS, 0, self.iface, &[])?;
        Ok(())
    }

    fn abort(&mut self) -> Result<()> {
        self.transport.control_out(DFU_ABORT, 0, self.iface, &[])?;
        Ok(())
    }

    /// Poll GET_STATUS until the device leaves a busy state, then verify it
    /// is in `expected` (or in `DFU_STATE_DNBUSY` which transitions on its
    /// own after `poll_timeout_ms`).
    fn wait_until(&mut self, expected: u8) -> Result<()> {
        for _ in 0..128 {
            let status = self.get_status()?;
            if status.status != 0 {
                return Err(Error::DfuStatus { status: status.status, state: status.state });
            }
            if status.state == expected {
                return Ok(());
            }
            if status.state == DFU_STATE_DNBUSY || status.state == DFU_STATE_MANIFEST {
                let wait = status.poll_timeout_ms.max(1);
                std::thread::sleep(Duration::from_millis(wait as u64));
                continue;
            }
            if status.state == DFU_STATE_ERROR {
                self.clear_status().ok();
                return Err(Error::DfuStatus { status: status.status, state: status.state });
            }
            // Some intermediate state — one more status poll.
            std::thread::sleep(Duration::from_millis(5));
        }
        Err(Error::Timeout)
    }

    fn set_address(&mut self, addr: u32) -> Result<()> {
        let mut cmd = Vec::with_capacity(5);
        cmd.push(0x21);
        cmd.extend_from_slice(&addr.to_le_bytes());
        self.transport.control_out(DFU_DNLOAD, 0, self.iface, &cmd)?;
        self.wait_until(DFU_STATE_DNLOAD_IDLE)?;
        Ok(())
    }

    fn erase_page(&mut self, addr: u32) -> Result<()> {
        let mut cmd = Vec::with_capacity(5);
        cmd.push(0x41);
        cmd.extend_from_slice(&addr.to_le_bytes());
        self.transport.control_out(DFU_DNLOAD, 0, self.iface, &cmd)?;
        self.wait_until(DFU_STATE_DNLOAD_IDLE)?;
        Ok(())
    }

    fn download_block(&mut self, block: u16, data: &[u8]) -> Result<()> {
        self.transport.control_out(DFU_DNLOAD, block, self.iface, data)?;
        self.wait_until(DFU_STATE_DNLOAD_IDLE)?;
        Ok(())
    }

    /// Issue the final zero-length DNLOAD that triggers the bootloader's
    /// manifest phase. After a successful manifest the bootloader resets
    /// the device — the ensuing USB error from the GET_STATUS poll is
    /// expected and silently absorbed.
    fn leave(&mut self) -> Result<()> {
        self.transport.control_out(DFU_DNLOAD, 0, self.iface, &[])?;
        // Manifest progresses on its own; we don't wait past it because the
        // device will reset and USB transfers will fail — that's success.
        match self.get_status() {
            Ok(status) if status.state == DFU_STATE_MANIFEST
                || status.state == DFU_STATE_MANIFEST_SYNC
                || status.state == DFU_STATE_MANIFEST_WAIT_RESET => Ok(()),
            Ok(_) | Err(Error::Usb(_)) => Ok(()),
            Err(e) => Err(e),
        }
    }

    /// Erase, write, and leave. Calls `progress(done, total, phase)` with
    /// phase `"erasing"` and `"writing"`.
    pub fn download(&mut self, file: &DfuSeFile, progress: &ProgressFn) -> Result<()> {
        // Drop any stray error state from a prior aborted attempt.
        if let Ok(status) = self.get_status() {
            if status.state == DFU_STATE_ERROR {
                self.clear_status().ok();
            } else if status.state != DFU_STATE_IDLE {
                self.abort().ok();
            }
        }

        // Flatten all targets into a single element list. SPIKE images only
        // populate one target ("Internal Flash"), but a DfuSe file may have
        // several.
        let elements: Vec<_> = file
            .targets
            .iter()
            .flat_map(|t| t.elements.iter())
            .collect();

        // Erase phase — every sector overlapping any element.
        let mut pages: Vec<u32> = Vec::new();
        for el in &elements {
            let end = el.address.saturating_add(el.data.len() as u32);
            for p in self.layout.pages_in(el.address, end) {
                if !pages.contains(&p) {
                    pages.push(p);
                }
            }
        }
        let total_pages = pages.len();
        for (i, addr) in pages.iter().enumerate() {
            self.erase_page(*addr)?;
            progress(i + 1, total_pages, "erasing");
        }

        // Write phase. LEGO's mboot-based bootloader (like MicroPython's
        // pydfu, which pybricksdev uses) requires `set_address` before
        // EVERY chunk; the usual DfuSe trick of incrementing `wBlock` to
        // auto-advance the address stalls after a few dozen KB on this
        // bootloader. Every data download uses `wBlock=2`.
        let total_bytes: usize = elements.iter().map(|e| e.data.len()).sum();
        let mut written: usize = 0;
        let chunk_size = self.transfer_size as usize;
        for el in &elements {
            let mut offset = 0usize;
            while offset < el.data.len() {
                let end = (offset + chunk_size).min(el.data.len());
                let chunk = &el.data[offset..end];
                let addr = el.address + offset as u32;
                self.set_address(addr)?;
                self.download_block(2, chunk).map_err(|e| match e {
                    Error::Usb(usb_e) => Error::Parse(format!(
                        "write failed at 0x{:08x} ({} bytes): {}",
                        addr,
                        chunk.len(),
                        usb_e,
                    )),
                    other => other,
                })?;
                offset = end;
                written += chunk.len();
                progress(written, total_bytes, "writing");
            }
        }

        self.leave()?;
        Ok(())
    }
}

// ── Real rusb-backed transport ──────────────────

pub struct RusbTransport {
    handle: DeviceHandle<GlobalContext>,
    timeout: Duration,
}

impl RusbTransport {
    pub fn new(handle: DeviceHandle<GlobalContext>) -> Self {
        RusbTransport { handle, timeout: Duration::from_secs(5) }
    }
}

impl DfuTransport for RusbTransport {
    fn control_out(&mut self, request: u8, value: u16, index: u16, data: &[u8]) -> Result<usize> {
        let n = self.handle.write_control(REQ_OUT, request, value, index, data, self.timeout)?;
        Ok(n)
    }
    fn control_in(&mut self, request: u8, value: u16, index: u16, length: u16) -> Result<Vec<u8>> {
        let mut buf = vec![0u8; length as usize];
        let n = self.handle.read_control(REQ_IN, request, value, index, &mut buf, self.timeout)?;
        buf.truncate(n);
        Ok(buf)
    }
}

/// Discovered bootloader. `iproduct` is the USB product string (e.g.
/// `"STM32F413"` or `"STM32H562"`), useful for picking the right firmware
/// image before connecting.
pub struct BootloaderInfo {
    pub device: rusb::Device<GlobalContext>,
    pub iproduct: String,
    pub iface: u16,
    pub alt: u8,
    pub transfer_size: u16,
    pub interface_desc: String,
}

/// Scan USB for a LEGO hub in DFU bootloader mode. Matches only LEGO's VID
/// `0x0694`; the interface-class filter below restricts further to devices
/// exposing a DFU-class interface (so normal-mode hubs, which are CDC, are
/// skipped).
pub fn find_stm32_bootloader() -> Result<Option<BootloaderInfo>> {
    for device in rusb::devices()?.iter() {
        let desc = device.device_descriptor()?;
        if desc.vendor_id() != LEGO_VID {
            continue;
        }
        // We need an open handle to read string descriptors.
        let handle = device.open()?;
        let lang = match handle.read_languages(Duration::from_secs(1))?.into_iter().next() {
            Some(l) => l,
            None => continue,
        };
        let iproduct = handle
            .read_product_string(lang, &desc, Duration::from_secs(1))
            .unwrap_or_default();

        // Locate the DFU interface with the largest alternate setting (its
        // `iInterface` string encodes the flash layout for that bank).
        let config = device.active_config_descriptor()?;
        let mut chosen: Option<(u16, u8, u16, String)> = None;
        for iface in config.interfaces() {
            for alt in iface.descriptors() {
                if alt.class_code() != 0xFE || alt.sub_class_code() != 0x01 {
                    continue; // not DFU application-specific
                }
                let iface_num = alt.interface_number() as u16;
                let alt_setting = alt.setting_number();
                let iface_str = handle
                    .read_interface_string(lang, &alt, Duration::from_secs(1))
                    .unwrap_or_default();
                // Transfer size lives in the DFU functional descriptor
                // (bDescriptorType = 0x21, 9 bytes). `alt.extra()` is the raw
                // byte stream of class-specific descriptors; walk it by
                // bLength so we find the DFU descriptor even when other
                // class-specific descriptors precede it.
                let mut transfer_size = 64u16;
                let extra = alt.extra();
                let mut i = 0;
                while i + 2 <= extra.len() {
                    let len = extra[i] as usize;
                    if len == 0 || i + len > extra.len() {
                        break;
                    }
                    if len >= 7 && extra[i + 1] == 0x21 {
                        transfer_size = u16::from_le_bytes([extra[i + 5], extra[i + 6]]);
                        break;
                    }
                    i += len;
                }
                // Prefer the "Internal Flash" alternate setting.
                let prefer = iface_str.contains("Internal Flash");
                if chosen.is_none() || prefer {
                    chosen = Some((iface_num, alt_setting, transfer_size, iface_str));
                    if prefer { break; }
                }
            }
        }

        let (iface, alt, transfer_size, interface_desc) = match chosen {
            Some(c) => c,
            None => continue,
        };

        // Detach kernel driver (Linux) if present, claim interface.
        #[cfg(target_os = "linux")]
        {
            if handle.kernel_driver_active(iface as u8).unwrap_or(false) {
                let _ = handle.detach_kernel_driver(iface as u8);
            }
        }
        handle.claim_interface(iface as u8)?;
        handle.set_alternate_setting(iface as u8, alt)?;

        // Release the handle we used for discovery — caller will open its own.
        drop(handle);

        return Ok(Some(BootloaderInfo {
            device,
            iproduct,
            iface,
            alt,
            transfer_size,
            interface_desc,
        }));
    }
    Ok(None)
}

/// Convenience: open a `DfuDevice` directly from a discovered bootloader.
pub fn open(info: &BootloaderInfo) -> Result<DfuDevice<RusbTransport>> {
    let handle = info.device.open()?;
    #[cfg(target_os = "linux")]
    {
        if handle.kernel_driver_active(info.iface as u8).unwrap_or(false) {
            let _ = handle.detach_kernel_driver(info.iface as u8);
        }
    }
    handle.claim_interface(info.iface as u8)?;
    handle.set_alternate_setting(info.iface as u8, info.alt)?;
    let layout = MemoryLayout::parse(&info.interface_desc)?;
    Ok(DfuDevice::new(
        RusbTransport::new(handle),
        info.iface,
        info.transfer_size,
        layout,
    ))
}

#[cfg(test)]
#[path = "tests/dfu.rs"]
mod tests;
