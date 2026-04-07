// ── BLE Service UUIDs ────────────────────────────

pub const LPF2_SERVICE_UUID: &str = "00001623-1212-efde-1623-785feabcd123";
pub const WEDO2_SERVICE_UUID: &str = "00001523-1212-efde-1523-785feabcd123";

// ── BLE Characteristic UUIDs ─────────────────────

pub const LPF2_CHARACTERISTIC_UUID: &str = "00001624-1212-efde-1623-785feabcd123";

pub const WEDO2_PORT_TYPE_UUID: &str = "00001527-1212-efde-1523-785feabcd123";
pub const WEDO2_SENSOR_VALUE_UUID: &str = "00001560-1212-efde-1523-785feabcd123";
pub const WEDO2_VALUE_FORMAT_UUID: &str = "00001561-1212-efde-1523-785feabcd123";
pub const WEDO2_PORT_TYPE_WRITE_UUID: &str = "00001563-1212-efde-1523-785feabcd123";
pub const WEDO2_MOTOR_VALUE_WRITE_UUID: &str = "00001565-1212-efde-1523-785feabcd123";
pub const WEDO2_BUTTON_UUID: &str = "00001526-1212-efde-1523-785feabcd123";
pub const WEDO2_BATTERY_UUID: &str = "2a19";
pub const WEDO2_DISCONNECT_UUID: &str = "0000152b-1212-efde-1523-785feabcd123";
pub const WEDO2_NAME_UUID: &str = "00001524-1212-efde-1523-785feabcd123";

// ── Hub Types ────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HubType {
    Unknown,
    WeDo2SmartHub,
    MoveHub,
    Hub,
    RemoteControl,
    DuploTrainBase,
    TechnicMediumHub,
    TechnicSmallHub,
}

impl HubType {
    pub fn display_name(&self) -> &str {
        match self {
            HubType::Unknown => "Unknown Hub",
            HubType::WeDo2SmartHub => "WeDo 2.0 Smart Hub",
            HubType::MoveHub => "Move Hub",
            HubType::Hub => "Powered UP Hub",
            HubType::RemoteControl => "Remote Control",
            HubType::DuploTrainBase => "Duplo Train Base",
            HubType::TechnicMediumHub => "Technic Medium Hub",
            HubType::TechnicSmallHub => "Technic Small Hub",
        }
    }

    pub fn is_wedo2(&self) -> bool {
        *self == HubType::WeDo2SmartHub
    }
}

// ── BLE Manufacturer Data (hub identification) ──

pub const DUPLO_TRAIN_BASE_ID: u8 = 32;
pub const MOVE_HUB_ID: u8 = 64;
pub const HUB_ID: u8 = 65;
pub const REMOTE_CONTROL_ID: u8 = 66;
pub const TECHNIC_MEDIUM_HUB_ID: u8 = 128;
pub const TECHNIC_SMALL_HUB_ID: u8 = 131;

pub fn hub_type_from_manufacturer_byte(byte: u8) -> HubType {
    match byte {
        DUPLO_TRAIN_BASE_ID => HubType::DuploTrainBase,
        MOVE_HUB_ID => HubType::MoveHub,
        HUB_ID => HubType::Hub,
        REMOTE_CONTROL_ID => HubType::RemoteControl,
        TECHNIC_MEDIUM_HUB_ID => HubType::TechnicMediumHub,
        TECHNIC_SMALL_HUB_ID => HubType::TechnicSmallHub,
        _ => HubType::Unknown,
    }
}

// ── Device Types ─────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u16)]
pub enum DeviceType {
    Unknown = 0,
    SimpleMediumLinearMotor = 1,
    TrainMotor = 2,
    Light = 8,
    VoltageSensor = 20,
    CurrentSensor = 21,
    PiezoBuzzer = 22,
    HubLed = 23,
    TiltSensor = 34,
    MotionSensor = 35,
    ColorDistanceSensor = 37,
    MediumLinearMotor = 38,
    MoveHubMediumLinearMotor = 39,
    MoveHubTiltSensor = 40,
    DuploTrainBaseMotor = 41,
    DuploTrainBaseSpeaker = 42,
    DuploTrainBaseColorSensor = 43,
    DuploTrainBaseSpeedometer = 44,
    TechnicLargeLinearMotor = 46,
    TechnicXLargeLinearMotor = 47,
    TechnicMediumAngularMotor = 48,
    TechnicLargeAngularMotor = 49,
    TechnicMediumHubGestSensor = 54,
    RemoteControlButton = 55,
    RemoteControlRssi = 56,
    TechnicMediumHubAccelerometer = 57,
    TechnicMediumHubGyroSensor = 58,
    TechnicMediumHubTiltSensor = 59,
    TechnicMediumHubTemperatureSensor = 60,
    TechnicColorSensor = 61,
    TechnicDistanceSensor = 62,
    TechnicForceSensor = 63,
    Technic3x3ColorLightMatrix = 64,
    TechnicSmallAngularMotor = 65,
    TechnicMediumAngularMotorGrey = 75,
    TechnicLargeAngularMotorGrey = 76,
}

impl DeviceType {
    pub fn from_u16(val: u16) -> Self {
        match val {
            1 => DeviceType::SimpleMediumLinearMotor,
            2 => DeviceType::TrainMotor,
            8 => DeviceType::Light,
            20 => DeviceType::VoltageSensor,
            21 => DeviceType::CurrentSensor,
            22 => DeviceType::PiezoBuzzer,
            23 => DeviceType::HubLed,
            34 => DeviceType::TiltSensor,
            35 => DeviceType::MotionSensor,
            37 => DeviceType::ColorDistanceSensor,
            38 => DeviceType::MediumLinearMotor,
            39 => DeviceType::MoveHubMediumLinearMotor,
            40 => DeviceType::MoveHubTiltSensor,
            41 => DeviceType::DuploTrainBaseMotor,
            42 => DeviceType::DuploTrainBaseSpeaker,
            43 => DeviceType::DuploTrainBaseColorSensor,
            44 => DeviceType::DuploTrainBaseSpeedometer,
            46 => DeviceType::TechnicLargeLinearMotor,
            47 => DeviceType::TechnicXLargeLinearMotor,
            48 => DeviceType::TechnicMediumAngularMotor,
            49 => DeviceType::TechnicLargeAngularMotor,
            54 => DeviceType::TechnicMediumHubGestSensor,
            55 => DeviceType::RemoteControlButton,
            56 => DeviceType::RemoteControlRssi,
            57 => DeviceType::TechnicMediumHubAccelerometer,
            58 => DeviceType::TechnicMediumHubGyroSensor,
            59 => DeviceType::TechnicMediumHubTiltSensor,
            60 => DeviceType::TechnicMediumHubTemperatureSensor,
            61 => DeviceType::TechnicColorSensor,
            62 => DeviceType::TechnicDistanceSensor,
            63 => DeviceType::TechnicForceSensor,
            64 => DeviceType::Technic3x3ColorLightMatrix,
            65 => DeviceType::TechnicSmallAngularMotor,
            75 => DeviceType::TechnicMediumAngularMotorGrey,
            76 => DeviceType::TechnicLargeAngularMotorGrey,
            _ => DeviceType::Unknown,
        }
    }

    /// Returns true if this device type has tacho (rotation) feedback.
    pub fn is_tacho_motor(&self) -> bool {
        matches!(self,
            DeviceType::MediumLinearMotor |
            DeviceType::MoveHubMediumLinearMotor |
            DeviceType::TechnicLargeLinearMotor |
            DeviceType::TechnicXLargeLinearMotor |
            DeviceType::TechnicMediumAngularMotor |
            DeviceType::TechnicLargeAngularMotor |
            DeviceType::TechnicSmallAngularMotor |
            DeviceType::TechnicMediumAngularMotorGrey |
            DeviceType::TechnicLargeAngularMotorGrey
        )
    }

    /// Returns true if this device type has absolute position feedback.
    pub fn is_absolute_motor(&self) -> bool {
        matches!(self,
            DeviceType::TechnicMediumAngularMotor |
            DeviceType::TechnicLargeAngularMotor |
            DeviceType::TechnicSmallAngularMotor |
            DeviceType::TechnicMediumAngularMotorGrey |
            DeviceType::TechnicLargeAngularMotorGrey
        )
    }

    /// Returns true if this device type is any kind of motor.
    pub fn is_motor(&self) -> bool {
        matches!(self,
            DeviceType::SimpleMediumLinearMotor |
            DeviceType::TrainMotor |
            DeviceType::DuploTrainBaseMotor |
            DeviceType::MediumLinearMotor |
            DeviceType::MoveHubMediumLinearMotor |
            DeviceType::TechnicLargeLinearMotor |
            DeviceType::TechnicXLargeLinearMotor |
            DeviceType::TechnicMediumAngularMotor |
            DeviceType::TechnicLargeAngularMotor |
            DeviceType::TechnicSmallAngularMotor |
            DeviceType::TechnicMediumAngularMotorGrey |
            DeviceType::TechnicLargeAngularMotorGrey
        )
    }
}

// ── LWP3 Message Types ───────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum MessageType {
    HubProperties = 0x01,
    HubActions = 0x02,
    HubAlerts = 0x03,
    HubAttachedIo = 0x04,
    GenericErrorMessages = 0x05,
    PortInformationRequest = 0x21,
    PortModeInformationRequest = 0x22,
    PortInputFormatSetupSingle = 0x41,
    PortInputFormatSetupCombinedMode = 0x42,
    PortInformation = 0x43,
    PortModeInformation = 0x44,
    PortValueSingle = 0x45,
    PortValueCombinedMode = 0x46,
    PortInputFormatSingle = 0x47,
    PortInputFormatCombinedMode = 0x48,
    VirtualPortSetup = 0x61,
    PortOutputCommand = 0x81,
    PortOutputCommandFeedback = 0x82,
}

impl MessageType {
    pub fn from_u8(val: u8) -> Option<Self> {
        match val {
            0x01 => Some(MessageType::HubProperties),
            0x02 => Some(MessageType::HubActions),
            0x03 => Some(MessageType::HubAlerts),
            0x04 => Some(MessageType::HubAttachedIo),
            0x05 => Some(MessageType::GenericErrorMessages),
            0x21 => Some(MessageType::PortInformationRequest),
            0x22 => Some(MessageType::PortModeInformationRequest),
            0x41 => Some(MessageType::PortInputFormatSetupSingle),
            0x42 => Some(MessageType::PortInputFormatSetupCombinedMode),
            0x43 => Some(MessageType::PortInformation),
            0x44 => Some(MessageType::PortModeInformation),
            0x45 => Some(MessageType::PortValueSingle),
            0x46 => Some(MessageType::PortValueCombinedMode),
            0x47 => Some(MessageType::PortInputFormatSingle),
            0x48 => Some(MessageType::PortInputFormatCombinedMode),
            0x61 => Some(MessageType::VirtualPortSetup),
            0x81 => Some(MessageType::PortOutputCommand),
            0x82 => Some(MessageType::PortOutputCommandFeedback),
            _ => None,
        }
    }
}

// ── Hub Attached IO Events ───────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum IoEvent {
    DetachedIo = 0x00,
    AttachedIo = 0x01,
    AttachedVirtualIo = 0x02,
}

// ── Hub Property References ──────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum HubProperty {
    AdvertisingName = 0x01,
    Button = 0x02,
    FwVersion = 0x03,
    HwVersion = 0x04,
    Rssi = 0x05,
    BatteryVoltage = 0x06,
    BatteryType = 0x07,
    ManufacturerName = 0x08,
    RadioFirmwareVersion = 0x09,
    LwpProtocolVersion = 0x0A,
    SystemTypeId = 0x0B,
    HwNetworkId = 0x0C,
    PrimaryMacAddress = 0x0D,
    SecondaryMacAddress = 0x0E,
    HardwareNetworkFamily = 0x0F,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum HubPropertyOperation {
    Set = 0x01,
    EnableUpdates = 0x02,
    DisableUpdates = 0x03,
    Reset = 0x04,
    RequestUpdate = 0x05,
    Update = 0x06,
}

// ── Motor Command Constants ──────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum BrakingStyle {
    Float = 0,
    Hold = 126,
    Brake = 127,
}

/// Port output sub-command IDs
pub const SUBCMD_SET_ACC_TIME: u8 = 0x05;
pub const SUBCMD_SET_DEC_TIME: u8 = 0x06;
pub const SUBCMD_START_SPEED: u8 = 0x07;
pub const SUBCMD_START_SPEED_DUAL: u8 = 0x08;
pub const SUBCMD_START_SPEED_FOR_TIME: u8 = 0x09;
pub const SUBCMD_START_SPEED_FOR_TIME_DUAL: u8 = 0x0a;
pub const SUBCMD_START_SPEED_FOR_DEGREES: u8 = 0x0b;
pub const SUBCMD_START_SPEED_FOR_DEGREES_DUAL: u8 = 0x0c;
pub const SUBCMD_GOTO_ABSOLUTE: u8 = 0x0d;
pub const SUBCMD_GOTO_ABSOLUTE_DUAL: u8 = 0x0e;
pub const SUBCMD_WRITE_DIRECT_MODE: u8 = 0x51;

// ── Command Feedback ─────────────────────────────

pub const FEEDBACK_BUFFER_EMPTY: u8 = 0x08;
pub const FEEDBACK_BUFFER_FREE: u8 = 0x01;
pub const FEEDBACK_COMPLETED: u8 = 0x02;
pub const FEEDBACK_DISCARDED: u8 = 0x04;
pub const FEEDBACK_HOLDING_TWO: u8 = 0x10;

// ── Colors ───────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Color {
    Black = 0,
    Pink = 1,
    Purple = 2,
    Blue = 3,
    LightBlue = 4,
    Cyan = 5,
    Green = 6,
    Yellow = 7,
    Orange = 8,
    Red = 9,
    White = 10,
    None = 255,
}

// ── Button States ────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum ButtonState {
    Released = 0,
    Up = 1,
    Pressed = 2,
    Stop = 127,
    Down = 255,
}

// ── WeDo 2.0 Port Mapping ───────────────────────

pub const WEDO2_PORT_A: u8 = 1;
pub const WEDO2_PORT_B: u8 = 2;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hub_type_from_manufacturer_byte() {
        assert_eq!(hub_type_from_manufacturer_byte(64), HubType::MoveHub);
        assert_eq!(hub_type_from_manufacturer_byte(65), HubType::Hub);
        assert_eq!(hub_type_from_manufacturer_byte(66), HubType::RemoteControl);
        assert_eq!(hub_type_from_manufacturer_byte(128), HubType::TechnicMediumHub);
        assert_eq!(hub_type_from_manufacturer_byte(131), HubType::TechnicSmallHub);
        assert_eq!(hub_type_from_manufacturer_byte(32), HubType::DuploTrainBase);
        assert_eq!(hub_type_from_manufacturer_byte(99), HubType::Unknown);
    }

    #[test]
    fn test_device_type_from_u16() {
        assert_eq!(DeviceType::from_u16(1), DeviceType::SimpleMediumLinearMotor);
        assert_eq!(DeviceType::from_u16(2), DeviceType::TrainMotor);
        assert_eq!(DeviceType::from_u16(61), DeviceType::TechnicColorSensor);
        assert_eq!(DeviceType::from_u16(63), DeviceType::TechnicForceSensor);
        assert_eq!(DeviceType::from_u16(9999), DeviceType::Unknown);
    }

    #[test]
    fn test_device_type_motor_classification() {
        assert!(DeviceType::TrainMotor.is_motor());
        assert!(DeviceType::TechnicMediumAngularMotor.is_motor());
        assert!(!DeviceType::TechnicColorSensor.is_motor());

        assert!(DeviceType::TechnicMediumAngularMotor.is_tacho_motor());
        assert!(!DeviceType::TrainMotor.is_tacho_motor());

        assert!(DeviceType::TechnicMediumAngularMotor.is_absolute_motor());
        assert!(!DeviceType::MediumLinearMotor.is_absolute_motor());
    }

    #[test]
    fn test_message_type_from_u8() {
        assert_eq!(MessageType::from_u8(0x01), Some(MessageType::HubProperties));
        assert_eq!(MessageType::from_u8(0x04), Some(MessageType::HubAttachedIo));
        assert_eq!(MessageType::from_u8(0x45), Some(MessageType::PortValueSingle));
        assert_eq!(MessageType::from_u8(0x81), Some(MessageType::PortOutputCommand));
        assert_eq!(MessageType::from_u8(0x82), Some(MessageType::PortOutputCommandFeedback));
        assert_eq!(MessageType::from_u8(0xFF), None);
    }

    #[test]
    fn test_hub_type_wedo2() {
        assert!(HubType::WeDo2SmartHub.is_wedo2());
        assert!(!HubType::Hub.is_wedo2());
        assert!(!HubType::TechnicMediumHub.is_wedo2());
    }
}
