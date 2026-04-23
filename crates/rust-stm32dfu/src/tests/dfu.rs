use super::*;
use crate::dfuse::{DfuSeFile, Element, Target};
use std::sync::{Arc, Mutex};

/// Scripted transport for state-machine tests. Records every control_out
/// call; always replies to control_in with a canned `DFU_GETSTATUS` byte
/// stream that walks the device through DNBUSY → DNLOAD_IDLE.
#[derive(Default)]
struct MockTransport {
    writes: Arc<Mutex<Vec<(u8, u16, u16, Vec<u8>)>>>,
    status_state: Arc<Mutex<u8>>,
}

impl MockTransport {
    fn new() -> Self {
        let mock = MockTransport::default();
        *mock.status_state.lock().unwrap() = DFU_STATE_IDLE;
        mock
    }
    fn writes(&self) -> Vec<(u8, u16, u16, Vec<u8>)> {
        self.writes.lock().unwrap().clone()
    }
}

impl DfuTransport for MockTransport {
    fn control_out(&mut self, request: u8, value: u16, index: u16, data: &[u8]) -> Result<usize> {
        self.writes
            .lock()
            .unwrap()
            .push((request, value, index, data.to_vec()));
        // A DNLOAD transitions the state to DNLOAD_SYNC (or IDLE→SYNC for
        // leave()). We advance directly to DNLOAD_IDLE so wait_until returns
        // on its first poll.
        if request == DFU_DNLOAD {
            *self.status_state.lock().unwrap() = DFU_STATE_DNLOAD_IDLE;
        }
        Ok(data.len())
    }

    fn control_in(&mut self, request: u8, _value: u16, _index: u16, length: u16) -> Result<Vec<u8>> {
        assert_eq!(request, DFU_GETSTATUS);
        assert!(length >= 6);
        let state = *self.status_state.lock().unwrap();
        Ok(vec![0, 0, 0, 0, state, 0])
    }
}

fn mock_device() -> DfuDevice<MockTransport> {
    // 8 KiB uniform sectors starting at 0x0800_0000, matching H5 layout.
    let layout = MemoryLayout {
        start: 0x0800_0000,
        regions: vec![MemoryRegion {
            start: 0x0800_0000,
            ranges: vec![SectorRange { count: 8, size: 8 * 1024, writable: true }],
        }],
    };
    DfuDevice::new(MockTransport::new(), 0, 1024, layout)
}

#[test]
fn test_memory_layout_parse_simple() {
    let layout = MemoryLayout::parse("@Internal Flash  /0x08000000/4*016Kg,01*064Kg,07*128Kg").unwrap();
    assert_eq!(layout.start, 0x0800_0000);
    assert_eq!(layout.regions.len(), 1);
    let r = &layout.regions[0].ranges;
    assert_eq!(r.len(), 3);
    assert_eq!(r[0], SectorRange { count: 4, size: 16 * 1024, writable: true });
    assert_eq!(r[1], SectorRange { count: 1, size: 64 * 1024, writable: true });
    assert_eq!(r[2], SectorRange { count: 7, size: 128 * 1024, writable: true });
}

#[test]
fn test_memory_layout_h5() {
    let layout = MemoryLayout::parse("@Internal Flash   /0x08000000/128*008Kg").unwrap();
    assert_eq!(
        layout.regions[0].ranges[0],
        SectorRange { count: 128, size: 8 * 1024, writable: true }
    );
}

#[test]
fn test_memory_layout_parse_rejects_bad_unit() {
    assert!(MemoryLayout::parse("@x/0x08000000/4*016Qg").is_err());
}

#[test]
fn test_memory_layout_multi_region_lego() {
    let layout = MemoryLayout::parse(
        "@LEGO LES HUB /0x08000000/02*016Ka,02*016Kg,01*064Kg,07*128Kg/0x10000000/01*1Ma/0x10100000/31*1Ma"
    ).unwrap();
    assert_eq!(layout.regions.len(), 3);
    // First region: LEGO bootloader (32K read-only) then LEGO-writable firmware area.
    let r0 = &layout.regions[0].ranges;
    assert_eq!(r0[0], SectorRange { count: 2, size: 16 * 1024, writable: false }); // 'a' = read-only
    assert_eq!(r0[1], SectorRange { count: 2, size: 16 * 1024, writable: true });
    // Other regions: all read-only (`a`).
    assert!(!layout.regions[1].ranges[0].writable);
    assert!(!layout.regions[2].ranges[0].writable);
}

#[test]
fn test_pages_in_h5_style() {
    let layout = MemoryLayout {
        start: 0x0800_0000,
        regions: vec![MemoryRegion {
            start: 0x0800_0000,
            ranges: vec![SectorRange { count: 4, size: 8 * 1024, writable: true }],
        }],
    };
    let pages = layout.pages_in(0x0800_1000, 0x0800_3000);
    assert_eq!(pages, vec![0x0800_0000, 0x0800_2000]);
}

#[test]
fn test_pages_in_f4_mixed() {
    let layout = MemoryLayout {
        start: 0x0800_0000,
        regions: vec![MemoryRegion {
            start: 0x0800_0000,
            ranges: vec![
                SectorRange { count: 4, size: 16 * 1024, writable: true },
                SectorRange { count: 1, size: 64 * 1024, writable: true },
            ],
        }],
    };
    let pages = layout.pages_in(0x0800_8000, 0x0801_3000);
    assert_eq!(pages, vec![0x0800_8000, 0x0800_C000, 0x0801_0000]);
}

#[test]
fn test_pages_in_skips_readonly_bootloader() {
    // LEGO layout: 2×16K read-only bootloader, then 2×16K writable, 1×64K, 7×128K.
    let layout = MemoryLayout::parse(
        "@LEGO LES HUB/0x08000000/02*016Ka,02*016Kg,01*064Kg,07*128Kg"
    ).unwrap();
    // An element that starts at 0x08000000 (inside the bootloader) and
    // spans into writable flash should only return writable sector starts.
    let pages = layout.pages_in(0x0800_0000, 0x0800_C000);
    assert_eq!(pages, vec![0x0800_8000]); // only the writable 16K sector
}

#[test]
fn test_download_executes_erase_and_write() {
    let mut dev = mock_device();
    let file = DfuSeFile {
        vendor: 0x0483,
        product: 0xDF11,
        device: 0x2200,
        targets: vec![Target {
            alt: 0,
            name: "STM32".into(),
            elements: vec![Element {
                address: 0x0800_0000,
                data: vec![0xAA; 4096], // fits in one 8K sector, spans 4 chunks of 1024
            }],
        }],
    };
    let progress: ProgressFn = Box::new(|_, _, _| {});
    dev.download(&file, &progress).unwrap();

    let writes = dev.transport.writes();
    // Pybricks-style: set_address BEFORE every chunk, then data download
    // with wBlock=2. 1 erase + 4 set_addr + 4 data + 1 leave.
    let erase_count = writes
        .iter()
        .filter(|(req, value, _, data)| *req == DFU_DNLOAD && *value == 0 && data.first() == Some(&0x41))
        .count();
    let setaddr_count = writes
        .iter()
        .filter(|(req, value, _, data)| *req == DFU_DNLOAD && *value == 0 && data.first() == Some(&0x21))
        .count();
    let data_count = writes
        .iter()
        .filter(|(req, value, _, _)| *req == DFU_DNLOAD && *value == 2)
        .count();
    let leave_count = writes
        .iter()
        .filter(|(req, value, _, data)| *req == DFU_DNLOAD && *value == 0 && data.is_empty())
        .count();
    assert_eq!(erase_count, 1, "writes={:?}", writes);
    assert_eq!(setaddr_count, 4); // one per chunk
    assert_eq!(data_count, 4);
    assert_eq!(leave_count, 1);
}

#[test]
fn test_download_progress_callbacks() {
    let mut dev = mock_device();
    let file = DfuSeFile {
        vendor: 0, product: 0, device: 0,
        targets: vec![Target {
            alt: 0, name: "".into(),
            elements: vec![Element { address: 0x0800_0000, data: vec![0; 2048] }],
        }],
    };
    let phases = Arc::new(Mutex::new(Vec::<String>::new()));
    let phases_inner = phases.clone();
    let progress: ProgressFn = Box::new(move |_, _, phase| {
        phases_inner.lock().unwrap().push(phase.to_string());
    });
    dev.download(&file, &progress).unwrap();
    let observed = phases.lock().unwrap().clone();
    assert!(observed.iter().any(|p| p == "erasing"));
    assert!(observed.iter().any(|p| p == "writing"));
}
