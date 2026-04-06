pub const CORAL_SERVICE_UUID: &str = "0000fd02-0000-1000-8000-00805f9b34fb";
pub const CORAL_WRITE_CHAR_UUID: &str = "0000fd02-0001-1000-8000-00805f9b34fb";
pub const CORAL_NOTIFY_CHAR_UUID: &str = "0000fd02-0002-1000-8000-00805f9b34fb";
pub const CORAL_SERVICE_SHORT: u16 = 0xfd02;
pub const DEFAULT_NOTIFICATION_INTERVAL_MS: u16 = 50;
pub const LEGO_COMPANY_ID: u16 = 0x0397;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CoralDeviceKind {
    SingleMotor = 0,
    DoubleMotor = 1,
    ColorSensor = 2,
    Controller = 3,
}

impl CoralDeviceKind {
    pub fn from_hardware_byte(byte: u8) -> Option<Self> {
        match byte {
            0 => Some(CoralDeviceKind::SingleMotor),
            1 => Some(CoralDeviceKind::DoubleMotor),
            2 => Some(CoralDeviceKind::ColorSensor),
            3 => Some(CoralDeviceKind::Controller),
            _ => None,
        }
    }

    pub fn display_name(&self) -> &str {
        match self {
            CoralDeviceKind::SingleMotor => "LEGO Education Science Single Motor",
            CoralDeviceKind::DoubleMotor => "LEGO Education Science Double Motor",
            CoralDeviceKind::ColorSensor => "LEGO Education Science Color Sensor",
            CoralDeviceKind::Controller => "LEGO Education Science Controller",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MotorBits {
    Left = 1,
    Right = 2,
    Both = 3,
}

impl MotorBits {
    pub fn from_u8(v: u8) -> Option<Self> {
        match v {
            1 => Some(MotorBits::Left),
            2 => Some(MotorBits::Right),
            3 => Some(MotorBits::Both),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MotorDirection {
    Clockwise = 0,
    Counterclockwise = 1,
    Shortest = 2,
    Longest = 3,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MotorEndState {
    Coast = 0,
    Brake = 1,
    Hold = 2,
    Continue = 3,
    SmartCoast = 4,
    SmartBrake = 5,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MotorState {
    Ready = 0,
    Running = 1,
    Stalled = 2,
    CmdAborted = 3,
    RegulationError = 4,
    MotorDisconnected = 5,
    Holding = 6,
    DcRunning = 7,
    NotAllowedToRun = 8,
}

impl MotorState {
    pub fn from_u8(v: u8) -> Self {
        match v {
            0 => MotorState::Ready,
            1 => MotorState::Running,
            2 => MotorState::Stalled,
            3 => MotorState::CmdAborted,
            4 => MotorState::RegulationError,
            5 => MotorState::MotorDisconnected,
            6 => MotorState::Holding,
            7 => MotorState::DcRunning,
            8 => MotorState::NotAllowedToRun,
            _ => MotorState::Ready,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MotionGesture {
    NoGesture = -1,
    Tapped = 0,
    DoubleTapped = 1,
    Collision = 2,
    Shake = 3,
    Freefall = 4,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MotorGesture {
    NoGesture = -1,
    SlowClockwise = 1,
    FastClockwise = 2,
    SlowCounterclockwise = 3,
    FastCounterclockwise = 4,
    Wiggled = 5,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LegoColor {
    None = -1,
    Black = 0,
    Magenta = 1,
    Purple = 2,
    Blue = 3,
    Azure = 4,
    Turquoise = 5,
    Green = 6,
    Yellow = 7,
    Orange = 8,
    Red = 9,
    White = 10,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CommandStatus {
    Completed = 0,
    Interrupted = 1,
    Nack = 2,
}

impl CommandStatus {
    pub fn from_u8(v: u8) -> Self {
        match v {
            0 => CommandStatus::Completed,
            1 => CommandStatus::Interrupted,
            _ => CommandStatus::Nack,
        }
    }
}
