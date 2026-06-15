use super::*;
use std::collections::VecDeque;

// A character tag from node-toypad's crypto vector: this UID + this 8-byte
// encrypted record decrypts to character id 16.
const CHAR_UID: [u8; 7] = [0x04, 0x47, 0x37, 0xe2, 0x48, 0x3f, 0x80];
const CHAR_ENC: [u8; 8] = [0x5c, 0xf7, 0x1c, 0xde, 0x29, 0xad, 0xea, 0x08];

#[derive(Default)]
struct MockState {
    reads: VecDeque<[u8; PACKET_LENGTH]>,
    writes: Vec<[u8; PACKET_LENGTH]>,
}

struct MockTransport {
    state: Arc<Mutex<MockState>>,
}

impl ToyPadTransport for MockTransport {
    fn read_frame(&mut self) -> Result<Option<[u8; PACKET_LENGTH]>, String> {
        Ok(self.state.lock().unwrap().reads.pop_front())
    }
    fn write(&mut self, data: &[u8]) -> Result<(), String> {
        let mut frame = [0u8; PACKET_LENGTH];
        let n = data.len().min(PACKET_LENGTH);
        frame[..n].copy_from_slice(&data[..n]);
        self.state.lock().unwrap().writes.push(frame);
        Ok(())
    }
}

/// Adapter + slot wired together, driven by manual `slot.tick()` calls (no
/// scheduler) for deterministic read-path tests.
fn make_manual(seeded: bool) -> (ToyPadAdapter, ToyPadSlot, Arc<Mutex<MockState>>) {
    let mock = Arc::new(Mutex::new(MockState::default()));
    let (tx, rx) = mpsc::channel();
    let shared = Arc::new(Mutex::new(ToyPadShared::default()));
    let slot = ToyPadSlot {
        device: Box::new(MockTransport { state: mock.clone() }),
        rx,
        shared: shared.clone(),
        pending: HashMap::new(),
        request_id: 0,
        seeded,
        alive: true,
    };
    let mut adapter = ToyPadAdapter::new(None);
    adapter.tx = Some(tx);
    adapter.shared = shared;
    (adapter, slot, mock)
}

/// Adapter with its slot registered on the scheduler, for color-output tests
/// (whose `set_rgb` blocks waiting for the slot to process the command).
fn make_scheduled() -> (ToyPadAdapter, Arc<Mutex<MockState>>) {
    let mock = Arc::new(Mutex::new(MockState::default()));
    let (tx, rx) = mpsc::channel();
    let shared = Arc::new(Mutex::new(ToyPadShared::default()));
    let slot = ToyPadSlot {
        device: Box::new(MockTransport { state: mock.clone() }),
        rx,
        shared: shared.clone(),
        pending: HashMap::new(),
        request_id: 0,
        seeded: true,
        alive: true,
    };
    let slot_id = scheduler::register_slot(Box::new(slot));
    let mut adapter = ToyPadAdapter::new(None);
    adapter.tx = Some(tx);
    adapter.shared = shared;
    adapter.slot_id = Some(slot_id);
    (adapter, mock)
}

fn add_event(panel: u8, index: u8, uid: [u8; 7]) -> [u8; PACKET_LENGTH] {
    let mut f = [0u8; PACKET_LENGTH];
    f[..6].copy_from_slice(&[0x56, 0x0b, panel, 0x00, index, 0x00]); // action 0 = Add
    f[6..13].copy_from_slice(&uid);
    f
}

fn remove_event(panel: u8, index: u8, uid: [u8; 7]) -> [u8; PACKET_LENGTH] {
    let mut f = add_event(panel, index, uid);
    f[5] = 0x01; // action 1 = Remove
    f
}

/// Build a ReadTag response carrying a 16-byte page for the given request id.
fn read_response(request_id: u8, page: [u8; 16]) -> [u8; PACKET_LENGTH] {
    let mut f = [0u8; PACKET_LENGTH];
    f[0] = 0x55;
    f[1] = 18; // length covering requestId + error + 16 page bytes
    f[2] = request_id;
    f[3] = 0x00; // error code OK
    f[4..20].copy_from_slice(&page);
    f
}

fn character_page() -> [u8; 16] {
    let mut page = [0u8; 16];
    page[..8].copy_from_slice(&CHAR_ENC);
    page[8..12].copy_from_slice(&[0xaa, 0xbb, 0xcc, 0xdd]); // not the vehicle marker
    page
}

fn vehicle_page(id: u16) -> [u8; 16] {
    let mut page = [0u8; 16];
    page[0] = (id & 0xff) as u8;
    page[1] = (id >> 8) as u8;
    page[8..12].copy_from_slice(&[0x00, 0x01, 0x00, 0x00]); // vehicle marker
    page
}

/// Request id of the most recent ReadTag write for the given page.
fn last_read_tag_req(mock: &Arc<Mutex<MockState>>, page: u8) -> u8 {
    let writes = &mock.lock().unwrap().writes;
    writes
        .iter()
        .rev()
        .find(|f| f[2] == 0xd2 && f[5] == page)
        .map(|f| f[3])
        .expect("expected a ReadTag write")
}

fn list_len(v: &LogoValue) -> usize {
    match v {
        LogoValue::List(items) => items.len(),
        _ => panic!("expected a list, got {:?}", v),
    }
}

#[test]
fn add_makes_uid_visible_immediately_with_pending_identity() {
    let (mut adapter, mut slot, mock) = make_manual(true);
    mock.lock().unwrap().reads.push_back(add_event(2, 1, CHAR_UID)); // panel Left
    slot.tick();

    // Presence true and UID present right away.
    assert_eq!(adapter.read_sensor("left", None).unwrap(), Some(LogoValue::Number(1.0)));
    let tags = adapter.read_sensor("left", Some("tags")).unwrap().unwrap();
    assert_eq!(list_len(&tags), 1);
    // Identity not yet resolved.
    assert_eq!(list_len(&adapter.read_sensor("left", Some("characters")).unwrap().unwrap()), 0);
    // And the slot issued the identity read.
    let _ = last_read_tag_req(&mock, 0x24);
}

#[test]
fn character_identity_backfills_after_response() {
    let (mut adapter, mut slot, mock) = make_manual(true);
    mock.lock().unwrap().reads.push_back(add_event(2, 1, CHAR_UID));
    slot.tick();
    let req = last_read_tag_req(&mock, 0x24);

    mock.lock().unwrap().reads.push_back(read_response(req, character_page()));
    slot.tick();

    let chars = adapter.read_sensor("left", Some("characters")).unwrap().unwrap();
    assert_eq!(chars, LogoValue::List(vec![LogoValue::Number(16.0)]));
    // UID still listed; not a vehicle.
    assert_eq!(list_len(&adapter.read_sensor("left", Some("tags")).unwrap().unwrap()), 1);
    assert_eq!(list_len(&adapter.read_sensor("left", Some("vehicles")).unwrap().unwrap()), 0);
}

#[test]
fn vehicle_identity_backfills_after_response() {
    let (mut adapter, mut slot, mock) = make_manual(true);
    let uid = [0x04, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66];
    mock.lock().unwrap().reads.push_back(add_event(3, 2, uid)); // panel Right
    slot.tick();
    let req = last_read_tag_req(&mock, 0x24);

    mock.lock().unwrap().reads.push_back(read_response(req, vehicle_page(0x0463)));
    slot.tick();

    let vehicles = adapter.read_sensor("right", Some("vehicles")).unwrap().unwrap();
    assert_eq!(vehicles, LogoValue::List(vec![LogoValue::Number(1123.0)])); // 0x0463
    assert_eq!(list_len(&adapter.read_sensor("right", Some("characters")).unwrap().unwrap()), 0);
}

#[test]
fn undecryptable_tag_stays_in_tags_only() {
    let (mut adapter, mut slot, mock) = make_manual(true);
    mock.lock().unwrap().reads.push_back(add_event(2, 1, CHAR_UID));
    slot.tick();
    let req = last_read_tag_req(&mock, 0x24);

    // Garbage identity page: not a vehicle, won't decrypt.
    let mut page = [0u8; 16];
    page[..8].copy_from_slice(&[0x00, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77]);
    mock.lock().unwrap().reads.push_back(read_response(req, page));
    slot.tick();

    assert_eq!(adapter.read_sensor("left", None).unwrap(), Some(LogoValue::Number(1.0)));
    assert_eq!(list_len(&adapter.read_sensor("left", Some("tags")).unwrap().unwrap()), 1);
    assert_eq!(list_len(&adapter.read_sensor("left", Some("characters")).unwrap().unwrap()), 0);
    assert_eq!(list_len(&adapter.read_sensor("left", Some("vehicles")).unwrap().unwrap()), 0);
}

#[test]
fn remove_drops_the_tag() {
    let (mut adapter, mut slot, mock) = make_manual(true);
    mock.lock().unwrap().reads.push_back(add_event(2, 1, CHAR_UID));
    slot.tick();
    mock.lock().unwrap().reads.push_back(remove_event(2, 1, CHAR_UID));
    slot.tick();

    assert_eq!(adapter.read_sensor("left", None).unwrap(), Some(LogoValue::Number(0.0)));
    assert_eq!(list_len(&adapter.read_sensor("left", Some("tags")).unwrap().unwrap()), 0);
}

#[test]
fn identity_response_for_lifted_tag_is_ignored() {
    let (mut adapter, mut slot, mock) = make_manual(true);
    mock.lock().unwrap().reads.push_back(add_event(2, 1, CHAR_UID));
    slot.tick();
    let req = last_read_tag_req(&mock, 0x24);
    // Tag lifted before the identity read returns.
    mock.lock().unwrap().reads.push_back(remove_event(2, 1, CHAR_UID));
    slot.tick();
    // Late response arrives — must not resurrect the tag or panic.
    mock.lock().unwrap().reads.push_back(read_response(req, character_page()));
    slot.tick();

    assert_eq!(adapter.read_sensor("left", None).unwrap(), Some(LogoValue::Number(0.0)));
    assert_eq!(list_len(&adapter.read_sensor("left", Some("characters")).unwrap().unwrap()), 0);
}

#[test]
fn seeding_lists_and_identifies_present_tags() {
    let (mut adapter, mut slot, mock) = make_manual(false);
    // First tick sends ListTags.
    slot.tick();
    let list_req = {
        let writes = &mock.lock().unwrap().writes;
        writes.iter().find(|f| f[2] == 0xd0).map(|f| f[3]).expect("ListTags sent")
    };
    // Respond: one tag on Left (panel 2), index 1.
    let mut list_resp = [0u8; PACKET_LENGTH];
    list_resp[..5].copy_from_slice(&[0x55, 3, list_req, 0x21, 0x00]);
    mock.lock().unwrap().reads.push_back(list_resp);
    slot.tick();

    // Slot now reads page 0 for the UID.
    let uid_req = last_read_tag_req(&mock, 0x00);
    let mut page0 = [0u8; 16];
    page0[0] = CHAR_UID[0];
    page0[1] = CHAR_UID[1];
    page0[2] = CHAR_UID[2];
    page0[4] = CHAR_UID[3];
    page0[5] = CHAR_UID[4];
    page0[6] = CHAR_UID[5];
    page0[7] = CHAR_UID[6];
    mock.lock().unwrap().reads.push_back(read_response(uid_req, page0));
    slot.tick();

    // UID now visible; slot reads identity page.
    assert_eq!(list_len(&adapter.read_sensor("left", Some("tags")).unwrap().unwrap()), 1);
    let id_req = last_read_tag_req(&mock, 0x24);
    mock.lock().unwrap().reads.push_back(read_response(id_req, character_page()));
    slot.tick();

    assert_eq!(
        adapter.read_sensor("left", Some("characters")).unwrap().unwrap(),
        LogoValue::List(vec![LogoValue::Number(16.0)])
    );
}

#[test]
fn unknown_pad_and_mode_error() {
    let (mut adapter, _slot, _mock) = make_manual(true);
    assert!(adapter.read_sensor("middle", None).is_err());
    assert!(adapter.read_sensor("left", Some("bogus")).is_err());
    assert!(adapter.validate_sensor_port("left", Some("characters")).is_ok());
}

#[test]
fn set_color_is_not_supported_with_setrgb_hint() {
    let (mut adapter, _mock) = make_scheduled();
    let err = adapter.set_color("left", 9).unwrap_err();
    adapter.disconnect();
    assert!(err.contains("setrgb"), "expected setrgb hint, got: {}", err);
}

#[test]
fn set_rgb_writes_set_color() {
    let (mut adapter, mock) = make_scheduled();
    adapter.set_rgb("left", (0x12, 0x34, 0x56)).unwrap();
    adapter.disconnect();
    let writes = mock.lock().unwrap().writes.clone();
    let frame = writes.iter().find(|f| f[2] == 0xc0).expect("a SetColor write");
    assert_eq!(frame[4], 2); // Panel::Left
    assert_eq!(&frame[5..8], &[0x12, 0x34, 0x56]);
}

#[test]
fn set_rgb_ports_multi_uses_set_color_all() {
    let (mut adapter, mock) = make_scheduled();
    adapter
        .set_rgb_ports(&[("left", (0xff, 0, 0)), ("right", (0, 0, 0xff))])
        .unwrap();
    adapter.disconnect();
    let writes = mock.lock().unwrap().writes.clone();
    let frame = writes.iter().find(|f| f[2] == 0xc8).expect("a SetColorAll write");
    // center off, left red, right blue
    assert_eq!(&frame[4..16], &[0, 0, 0, 0, 1, 0xff, 0, 0, 1, 0, 0, 0xff]);
}
