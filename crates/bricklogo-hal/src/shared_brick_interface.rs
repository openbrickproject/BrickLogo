use std::sync::{Arc, Mutex, atomic::{AtomicBool, Ordering}};
use rust_brickinterface::BrickInterface;

pub struct SharedBrickInterface {
    pub device: Mutex<BrickInterface>,
    pub interfacea_active: AtomicBool,
    pub powerfunctions_active: AtomicBool,
    pub capabilities: u16,
    pub path: String,
}

impl SharedBrickInterface {
    pub fn new(device: BrickInterface) -> Arc<Self> {
        Arc::new(SharedBrickInterface {
            device: Mutex::new(device),
            interfacea_active: AtomicBool::new(false),
            powerfunctions_active: AtomicBool::new(false),
            capabilities: u16::MAX,
            path: String::new(),
        })
    }

    pub fn open(serial_path: &str) -> Result<Arc<Self>, String> {
        let mut device = BrickInterface::open(serial_path)?;
        let capabilities = device.get_capabilities()
            .map_err(|e| format!("Could not query capabilities on {}: {}", serial_path, e))?;
        Ok(Arc::new(SharedBrickInterface {
            device: Mutex::new(device),
            interfacea_active: AtomicBool::new(false),
            powerfunctions_active: AtomicBool::new(false),
            capabilities,
            path: serial_path.to_string(),
        }))
    }

    pub fn release_interfacea(&self) {
        self.interfacea_active.store(false, Ordering::SeqCst);
    }

    pub fn release_powerfunctions(&self) {
        self.powerfunctions_active.store(false, Ordering::SeqCst);
    }
}
