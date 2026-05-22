use std::sync::{Arc, Mutex, atomic::{AtomicBool, Ordering}};
use rust_brickinterface::BrickInterface;

pub struct SharedBrickInterface {
    pub device: Mutex<BrickInterface>,
    pub interfacea_active: AtomicBool,
    pub powerfunctions_active: AtomicBool,
}

impl SharedBrickInterface {
    pub fn new(device: BrickInterface) -> Arc<Self> {
        Arc::new(SharedBrickInterface {
            device: Mutex::new(device),
            interfacea_active: AtomicBool::new(false),
            powerfunctions_active: AtomicBool::new(false),
        })
    }

    pub fn open(serial_path: &str) -> Result<Arc<Self>, String> {
        let device = BrickInterface::open(serial_path)?;
        Ok(Self::new(device))
    }

    pub fn release_interfacea(&self) {
        self.interfacea_active.store(false, Ordering::SeqCst);
    }

    pub fn release_powerfunctions(&self) {
        self.powerfunctions_active.store(false, Ordering::SeqCst);
    }
}
