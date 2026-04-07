use std::sync::Mutex;
use std::time::{Duration, Instant};

const TICK_INTERVAL: Duration = Duration::from_nanos(1_000_000_000 / 60); // ~16.6ms

/// Trait for a device serviced by the shared driver thread.
pub trait DeviceSlot: Send {
    /// Called once per tick (~60hz). Read sensors, drain command queue, write.
    fn tick(&mut self);
    /// Return false if the device has been disconnected and should be removed.
    fn is_alive(&self) -> bool;
}

struct DriverInner {
    slots: Vec<(usize, Box<dyn DeviceSlot>)>,
    next_id: usize,
    thread_running: bool,
}

impl DriverInner {
    fn new() -> Self {
        DriverInner {
            slots: Vec::new(),
            next_id: 0,
            thread_running: false,
        }
    }
}

lazy_static::lazy_static! {
    static ref DRIVER: Mutex<DriverInner> = Mutex::new(DriverInner::new());
}

/// Register a device slot with the shared driver. Returns a slot ID for deregistration.
pub fn register(slot: Box<dyn DeviceSlot>) -> usize {
    let mut driver = DRIVER.lock().unwrap();
    let id = driver.next_id;
    driver.next_id += 1;
    driver.slots.push((id, slot));

    if !driver.thread_running {
        driver.thread_running = true;
        std::thread::spawn(driver_loop);
    }

    id
}

/// Remove a device slot by ID.
pub fn deregister(slot_id: usize) {
    let mut driver = DRIVER.lock().unwrap();
    driver.slots.retain(|(id, _)| *id != slot_id);
}

fn driver_loop() {
    loop {
        let tick_start = Instant::now();

        {
            let mut driver = DRIVER.lock().unwrap();

            // Remove dead slots
            driver.slots.retain(|(_, slot)| slot.is_alive());

            if driver.slots.is_empty() {
                driver.thread_running = false;
                return;
            }

            for (_, slot) in &mut driver.slots {
                slot.tick();
            }
        }

        // Sleep until next tick boundary
        let elapsed = tick_start.elapsed();
        if elapsed < TICK_INTERVAL {
            std::thread::sleep(TICK_INTERVAL - elapsed);
        }
    }
}
