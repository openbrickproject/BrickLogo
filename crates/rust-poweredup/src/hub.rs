use std::collections::HashMap;
use crate::constants::*;
use crate::devices::{self, SensorReading, build_mode_lookup};
use crate::protocol::*;

/// A device attached to a hub port.
#[derive(Debug, Clone)]
pub struct AttachedDevice {
    pub port_id: u8,
    pub device_type: DeviceType,
    pub mode_lookup: HashMap<String, u8>,
    pub current_mode: Option<u8>,
    pub last_reading: Option<SensorReading>,
}

impl AttachedDevice {
    pub fn new(port_id: u8, device_type: DeviceType) -> Self {
        AttachedDevice {
            port_id,
            device_type,
            mode_lookup: build_mode_lookup(device_type),
            current_mode: None,
            last_reading: None,
        }
    }
}

/// Hub state — tracks connected devices, properties, and cached sensor values.
pub struct Hub {
    pub hub_type: HubType,
    pub name: String,
    pub battery: u8,
    devices: HashMap<u8, AttachedDevice>,
    connected: bool,
    port_names: HashMap<String, u8>,
}

impl Hub {
    pub fn new(hub_type: HubType) -> Self {
        let port_names = default_port_names(hub_type);
        Hub {
            hub_type,
            name: String::new(),
            battery: 0,
            devices: HashMap::new(),
            connected: false,
            port_names,
        }
    }

    pub fn on_connected(&mut self) {
        self.connected = true;
    }

    pub fn on_disconnected(&mut self) {
        self.connected = false;
        self.devices.clear();
    }

    pub fn is_connected(&self) -> bool {
        self.connected
    }

    /// Process a complete LWP3 message. Returns any events that resulted.
    pub fn process_message(&mut self, msg: &[u8]) -> Vec<HubEvent> {
        let mut events = Vec::new();
        let msg_type = match message_type(msg) {
            Some(t) => t,
            None => return events,
        };

        match msg_type {
            MessageType::HubAttachedIo => {
                if let Some(io_event) = parse_attached_io(msg) {
                    match io_event {
                        AttachedIoEvent::Attached { port_id, device_type } => {
                            let device = AttachedDevice::new(port_id, device_type);
                            self.devices.insert(port_id, device);
                            events.push(HubEvent::DeviceAttached { port_id, device_type });
                        }
                        AttachedIoEvent::Detached { port_id } => {
                            if let Some(device) = self.devices.remove(&port_id) {
                                events.push(HubEvent::DeviceDetached { port_id, device_type: device.device_type });
                            }
                        }
                        AttachedIoEvent::AttachedVirtual { port_id, device_type, first_port, second_port } => {
                            let device = AttachedDevice::new(port_id, device_type);
                            self.devices.insert(port_id, device);
                            events.push(HubEvent::VirtualDeviceAttached {
                                port_id, device_type, first_port, second_port,
                            });
                        }
                    }
                }
            }

            MessageType::PortValueSingle => {
                if let Some((port_id, data)) = parse_port_value(msg) {
                    if let Some(device) = self.devices.get_mut(&port_id) {
                        if let Some(mode) = device.current_mode {
                            let reading = devices::parse_sensor_data(
                                device.device_type, mode, data, self.hub_type.is_wedo2()
                            );
                            if let Some(ref r) = reading {
                                device.last_reading = Some(r.clone());
                                events.push(HubEvent::SensorValue {
                                    port_id,
                                    device_type: device.device_type,
                                    reading: r.clone(),
                                });
                            }
                        }
                    }
                }
            }

            MessageType::PortOutputCommandFeedback => {
                for fb in parse_port_feedback(msg) {
                    events.push(HubEvent::CommandFeedback {
                        port_id: fb.port_id,
                        completed: fb.is_completed(),
                        discarded: fb.is_discarded(),
                    });
                }
            }

            MessageType::HubProperties => {
                if let Some(prop) = parse_hub_property(msg) {
                    match prop {
                        HubPropertyValue::BatteryVoltage(v) => { self.battery = v; }
                        HubPropertyValue::Name(ref n) => { self.name = n.clone(); }
                        _ => {}
                    }
                    events.push(HubEvent::PropertyUpdate(prop));
                }
            }

            _ => {}
        }

        events
    }

    /// Process a WeDo 2.0 port type message (attach/detach).
    pub fn process_wedo2_port_type(&mut self, msg: &[u8]) -> Vec<HubEvent> {
        let mut events = Vec::new();
        if msg.len() < 2 { return events; }

        let port_id = msg[0];
        let event = msg[1];

        match event {
            0x01 => { // attached
                if msg.len() >= 4 {
                    let device_type = DeviceType::from_u16(msg[3] as u16);
                    let device = AttachedDevice::new(port_id, device_type);
                    self.devices.insert(port_id, device);
                    events.push(HubEvent::DeviceAttached { port_id, device_type });
                }
            }
            0x00 => { // detached
                if let Some(device) = self.devices.remove(&port_id) {
                    events.push(HubEvent::DeviceDetached { port_id, device_type: device.device_type });
                }
            }
            _ => {}
        }

        events
    }

    /// Process a WeDo 2.0 sensor value message.
    pub fn process_wedo2_sensor_value(&mut self, msg: &[u8]) -> Vec<HubEvent> {
        let mut events = Vec::new();
        if let Some((port_id, data)) = parse_wedo2_sensor_value(msg) {
            if let Some(device) = self.devices.get_mut(&port_id) {
                if let Some(mode) = device.current_mode {
                    let reading = devices::parse_sensor_data(
                        device.device_type, mode, data, true
                    );
                    if let Some(ref r) = reading {
                        device.last_reading = Some(r.clone());
                        events.push(HubEvent::SensorValue {
                            port_id,
                            device_type: device.device_type,
                            reading: r.clone(),
                        });
                    }
                }
            }
        }
        events
    }

    // ── Device queries ──────────────────────────

    pub fn get_device(&self, port_id: u8) -> Option<&AttachedDevice> {
        self.devices.get(&port_id)
    }

    pub fn get_device_mut(&mut self, port_id: u8) -> Option<&mut AttachedDevice> {
        self.devices.get_mut(&port_id)
    }

    pub fn get_attached_devices(&self) -> Vec<&AttachedDevice> {
        self.devices.values().collect()
    }

    /// Get port ID by name (e.g., "A" → 0, "B" → 1).
    pub fn port_id_by_name(&self, name: &str) -> Option<u8> {
        self.port_names.get(&name.to_uppercase()).copied()
    }

    /// Get the device on a named port.
    pub fn get_device_at_port(&self, port_name: &str) -> Option<&AttachedDevice> {
        self.port_id_by_name(port_name).and_then(|id| self.devices.get(&id))
    }

    /// Find the first device matching a type.
    pub fn find_device_by_type(&self, device_type: DeviceType) -> Option<&AttachedDevice> {
        self.devices.values().find(|d| d.device_type == device_type)
    }

    // ── Sensor subscription tracking ────────────

    /// Record that we've subscribed to a mode on a port.
    pub fn set_subscribed_mode(&mut self, port_id: u8, mode: u8) {
        if let Some(device) = self.devices.get_mut(&port_id) {
            device.current_mode = Some(mode);
        }
    }

    /// Get the mode number for an event name on a port.
    pub fn mode_for_event(&self, port_id: u8, event: &str) -> Option<u8> {
        self.devices.get(&port_id)
            .and_then(|d| d.mode_lookup.get(event).copied())
    }

    /// Get the last reading from a port.
    pub fn last_reading(&self, port_id: u8) -> Option<&SensorReading> {
        self.devices.get(&port_id).and_then(|d| d.last_reading.as_ref())
    }
}

/// Events produced by processing hub messages.
#[derive(Debug, Clone, PartialEq)]
pub enum HubEvent {
    DeviceAttached { port_id: u8, device_type: DeviceType },
    DeviceDetached { port_id: u8, device_type: DeviceType },
    VirtualDeviceAttached { port_id: u8, device_type: DeviceType, first_port: u8, second_port: u8 },
    SensorValue { port_id: u8, device_type: DeviceType, reading: SensorReading },
    CommandFeedback { port_id: u8, completed: bool, discarded: bool },
    PropertyUpdate(HubPropertyValue),
}

/// Default port name → port ID mapping per hub type.
fn default_port_names(hub_type: HubType) -> HashMap<String, u8> {
    let mut map = HashMap::new();
    match hub_type {
        HubType::WeDo2SmartHub => {
            map.insert("A".into(), 1);
            map.insert("B".into(), 2);
        }
        HubType::MoveHub => {
            map.insert("A".into(), 0);
            map.insert("B".into(), 1);
            map.insert("C".into(), 2);
            map.insert("D".into(), 3);
        }
        HubType::Hub => {
            map.insert("A".into(), 0);
            map.insert("B".into(), 1);
        }
        HubType::RemoteControl => {
            map.insert("A".into(), 0); // LEFT
            map.insert("B".into(), 1); // RIGHT
        }
        HubType::DuploTrainBase => {
            map.insert("A".into(), 0);
        }
        HubType::TechnicMediumHub => {
            map.insert("A".into(), 0);
            map.insert("B".into(), 1);
            map.insert("C".into(), 2);
            map.insert("D".into(), 3);
        }
        HubType::TechnicSmallHub => {
            map.insert("A".into(), 0);
            map.insert("B".into(), 1);
        }
        HubType::Unknown => {}
    }
    map
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hub_new() {
        let hub = Hub::new(HubType::TechnicMediumHub);
        assert_eq!(hub.hub_type, HubType::TechnicMediumHub);
        assert!(!hub.is_connected());
        assert!(hub.get_attached_devices().is_empty());
    }

    #[test]
    fn test_hub_connect_disconnect() {
        let mut hub = Hub::new(HubType::Hub);
        hub.on_connected();
        assert!(hub.is_connected());
        hub.on_disconnected();
        assert!(!hub.is_connected());
    }

    #[test]
    fn test_process_attach() {
        let mut hub = Hub::new(HubType::TechnicMediumHub);
        hub.on_connected();

        // Attach a Technic Color Sensor on port 0
        let msg = vec![15, 0x00, 0x04, 0x00, 0x01, 0x3d, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00];
        let events = hub.process_message(&msg);

        assert_eq!(events.len(), 1);
        assert_eq!(events[0], HubEvent::DeviceAttached {
            port_id: 0,
            device_type: DeviceType::TechnicColorSensor,
        });

        let device = hub.get_device(0).unwrap();
        assert_eq!(device.device_type, DeviceType::TechnicColorSensor);
        assert!(device.mode_lookup.contains_key("color"));
    }

    #[test]
    fn test_process_detach() {
        let mut hub = Hub::new(HubType::Hub);
        hub.on_connected();

        // Attach
        let msg = vec![15, 0x00, 0x04, 0x00, 0x01, 0x3d, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00];
        hub.process_message(&msg);
        assert!(hub.get_device(0).is_some());

        // Detach
        let msg = vec![5, 0x00, 0x04, 0x00, 0x00];
        let events = hub.process_message(&msg);
        assert_eq!(events.len(), 1);
        assert!(matches!(events[0], HubEvent::DeviceDetached { port_id: 0, .. }));
        assert!(hub.get_device(0).is_none());
    }

    #[test]
    fn test_process_sensor_value() {
        let mut hub = Hub::new(HubType::TechnicMediumHub);
        hub.on_connected();

        // Attach color sensor on port 0
        let attach = vec![15, 0x00, 0x04, 0x00, 0x01, 0x3d, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00];
        hub.process_message(&attach);

        // Subscribe to color mode
        hub.set_subscribed_mode(0, 0x00);

        // Sensor value: color = 3 (blue)
        let msg = vec![6, 0x00, 0x45, 0x00, 0x03];
        let events = hub.process_message(&msg);

        assert_eq!(events.len(), 1);
        if let HubEvent::SensorValue { port_id, reading, .. } = &events[0] {
            assert_eq!(*port_id, 0);
            assert_eq!(*reading, SensorReading::Number(3.0));
        } else {
            panic!("Expected SensorValue event");
        }

        // Check cached reading
        assert_eq!(hub.last_reading(0), Some(&SensorReading::Number(3.0)));
    }

    #[test]
    fn test_process_motor_rotation() {
        let mut hub = Hub::new(HubType::TechnicMediumHub);
        hub.on_connected();

        // Attach angular motor on port 0
        let attach = vec![15, 0x00, 0x04, 0x00, 0x01, 0x30, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00];
        hub.process_message(&attach);

        // Subscribe to rotation mode
        hub.set_subscribed_mode(0, 0x02);

        // Sensor value: 720 degrees (0x000002D0 LE)
        let deg_bytes = 720_i32.to_le_bytes();
        let msg = vec![9, 0x00, 0x45, 0x00, deg_bytes[0], deg_bytes[1], deg_bytes[2], deg_bytes[3]];
        let events = hub.process_message(&msg);

        assert_eq!(events.len(), 1);
        if let HubEvent::SensorValue { reading, .. } = &events[0] {
            assert_eq!(*reading, SensorReading::Number(720.0));
        }
    }

    #[test]
    fn test_process_command_feedback() {
        let mut hub = Hub::new(HubType::Hub);
        hub.on_connected();

        let msg = vec![5, 0x00, 0x82, 0x00, 0x0a]; // port 0, feedback 0x0a (completed + buffer empty)
        let events = hub.process_message(&msg);

        assert_eq!(events.len(), 1);
        if let HubEvent::CommandFeedback { port_id, completed, discarded } = &events[0] {
            assert_eq!(*port_id, 0);
            assert!(*completed);
            assert!(!*discarded);
        }
    }

    #[test]
    fn test_process_battery() {
        let mut hub = Hub::new(HubType::Hub);
        let msg = vec![6, 0x00, 0x01, 0x06, 0x06, 75];
        hub.process_message(&msg);
        assert_eq!(hub.battery, 75);
    }

    #[test]
    fn test_port_name_mapping() {
        let hub = Hub::new(HubType::TechnicMediumHub);
        assert_eq!(hub.port_id_by_name("A"), Some(0));
        assert_eq!(hub.port_id_by_name("B"), Some(1));
        assert_eq!(hub.port_id_by_name("C"), Some(2));
        assert_eq!(hub.port_id_by_name("D"), Some(3));
        assert_eq!(hub.port_id_by_name("a"), Some(0)); // case insensitive
        assert_eq!(hub.port_id_by_name("E"), None);
    }

    #[test]
    fn test_port_name_wedo2() {
        let hub = Hub::new(HubType::WeDo2SmartHub);
        assert_eq!(hub.port_id_by_name("A"), Some(1)); // WeDo2 ports start at 1
        assert_eq!(hub.port_id_by_name("B"), Some(2));
    }

    #[test]
    fn test_mode_for_event() {
        let mut hub = Hub::new(HubType::Hub);
        hub.on_connected();

        let attach = vec![15, 0x00, 0x04, 0x00, 0x01, 0x3d, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00];
        hub.process_message(&attach);

        assert_eq!(hub.mode_for_event(0, "color"), Some(0x00));
        assert_eq!(hub.mode_for_event(0, "reflect"), Some(0x01));
        assert_eq!(hub.mode_for_event(0, "nonexistent"), None);
        assert_eq!(hub.mode_for_event(99, "color"), None); // no device on port 99
    }

    #[test]
    fn test_find_device_by_type() {
        let mut hub = Hub::new(HubType::TechnicMediumHub);
        hub.on_connected();

        // Attach motor on port 0
        let motor_attach = vec![15, 0x00, 0x04, 0x00, 0x01, 0x30, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00];
        hub.process_message(&motor_attach);

        // Attach color sensor on port 2
        let color_attach = vec![15, 0x00, 0x04, 0x02, 0x01, 0x3d, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00];
        hub.process_message(&color_attach);

        let motor = hub.find_device_by_type(DeviceType::TechnicMediumAngularMotor);
        assert!(motor.is_some());
        assert_eq!(motor.unwrap().port_id, 0);

        let color = hub.find_device_by_type(DeviceType::TechnicColorSensor);
        assert!(color.is_some());
        assert_eq!(color.unwrap().port_id, 2);

        assert!(hub.find_device_by_type(DeviceType::TechnicForceSensor).is_none());
    }

    #[test]
    fn test_get_device_at_port() {
        let mut hub = Hub::new(HubType::TechnicMediumHub);
        hub.on_connected();

        let attach = vec![15, 0x00, 0x04, 0x02, 0x01, 0x3d, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00];
        hub.process_message(&attach);

        let device = hub.get_device_at_port("C");
        assert!(device.is_some());
        assert_eq!(device.unwrap().device_type, DeviceType::TechnicColorSensor);

        assert!(hub.get_device_at_port("A").is_none());
    }

    #[test]
    fn test_wedo2_port_type() {
        let mut hub = Hub::new(HubType::WeDo2SmartHub);
        hub.on_connected();

        // Attach tilt sensor on port 1
        let msg = vec![1, 0x01, 0x00, 34]; // port 1, attached, device type 34 (TiltSensor)
        let events = hub.process_wedo2_port_type(&msg);

        assert_eq!(events.len(), 1);
        assert_eq!(events[0], HubEvent::DeviceAttached {
            port_id: 1,
            device_type: DeviceType::TiltSensor,
        });
    }

    #[test]
    fn test_no_sensor_value_without_subscription() {
        let mut hub = Hub::new(HubType::TechnicMediumHub);
        hub.on_connected();

        let attach = vec![15, 0x00, 0x04, 0x00, 0x01, 0x3d, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00];
        hub.process_message(&attach);

        // Don't subscribe — sensor value should produce no events
        let msg = vec![6, 0x00, 0x45, 0x00, 0x03];
        let events = hub.process_message(&msg);
        assert!(events.is_empty());
    }

    #[test]
    fn test_virtual_port_attach() {
        let mut hub = Hub::new(HubType::TechnicMediumHub);
        hub.on_connected();

        let msg = vec![9, 0x00, 0x04, 0x10, 0x02, 0x30, 0x00, 0x00, 0x01];
        let events = hub.process_message(&msg);

        assert_eq!(events.len(), 1);
        if let HubEvent::VirtualDeviceAttached { port_id, first_port, second_port, .. } = &events[0] {
            assert_eq!(*port_id, 0x10);
            assert_eq!(*first_port, 0x00);
            assert_eq!(*second_port, 0x01);
        }
    }
}
