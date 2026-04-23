//! A single 60 Hz background thread that drives all periodic hardware work
//! in BrickLogo.
//!
//! Two things run here:
//!
//!   - **Device slots.** Slot-based adapters (Build HAT, Control Lab,
//!     WeDo 1.0, RCX) register a `DeviceSlot` whose `tick()` is called
//!     every ~16.6 ms. The slot reads sensors, drains its command queue,
//!     writes PWM/serial, and handles keep-alives.
//!   - **Periodic tasks.** Anything that wants to happen at a roughly-fixed
//!     cadence (today: `flash` timers) registers a closure. The closure is
//!     invoked every tick and returns `true` to keep running or `false` to
//!     retire itself.
//!
//! The thread is spawned lazily on first registration and exits when both
//! lists empty out.

use std::sync::Mutex;
use std::time::{Duration, Instant};

const TICK_INTERVAL: Duration = Duration::from_nanos(1_000_000_000 / 60); // ~16.6 ms

/// Trait for a device serviced by the scheduler.
pub trait DeviceSlot: Send {
    /// Called once per tick. Read sensors, drain command queue, write.
    fn tick(&mut self);
    /// Return false when the adapter has disconnected — the scheduler will
    /// then remove this slot on its next iteration.
    fn is_alive(&self) -> bool;
}

/// A closure invoked every tick. Returns `true` to keep running, `false`
/// to self-retire (the scheduler drops the task on its next iteration).
pub type PeriodicTask = Box<dyn FnMut() -> bool + Send>;

struct SchedulerInner {
    slots: Vec<(usize, Box<dyn DeviceSlot>)>,
    tasks: Vec<(usize, PeriodicTask)>,
    next_id: usize,
    thread_running: bool,
}

impl SchedulerInner {
    fn new() -> Self {
        SchedulerInner {
            slots: Vec::new(),
            tasks: Vec::new(),
            next_id: 0,
            thread_running: false,
        }
    }
}

lazy_static::lazy_static! {
    static ref SCHEDULER: Mutex<SchedulerInner> = Mutex::new(SchedulerInner::new());
}

/// Register a device slot. Returns an ID for later deregistration.
pub fn register_slot(slot: Box<dyn DeviceSlot>) -> usize {
    let mut sched = SCHEDULER.lock().unwrap();
    let id = sched.next_id;
    sched.next_id += 1;
    sched.slots.push((id, slot));
    ensure_running(&mut sched);
    id
}

/// Remove a slot by ID.
pub fn deregister_slot(slot_id: usize) {
    let mut sched = SCHEDULER.lock().unwrap();
    sched.slots.retain(|(id, _)| *id != slot_id);
}

/// Register a periodic task. The closure is invoked every tick and returns
/// `true` to stay scheduled or `false` to retire. Cancellation of a task
/// from outside the closure is typically done via a shared `AtomicBool`
/// flag that the closure checks — see `port_manager::flash`.
pub fn register_task(task: PeriodicTask) {
    let mut sched = SCHEDULER.lock().unwrap();
    let id = sched.next_id;
    sched.next_id += 1;
    sched.tasks.push((id, task));
    ensure_running(&mut sched);
}

fn ensure_running(sched: &mut SchedulerInner) {
    if !sched.thread_running {
        sched.thread_running = true;
        std::thread::spawn(scheduler_loop);
    }
}

fn scheduler_loop() {
    loop {
        let tick_start = Instant::now();

        // Reap dead slots, then tick survivors. Slot ticks do hardware I/O
        // only and don't reach back into the scheduler, so holding the lock
        // across them is safe.
        {
            let mut sched = SCHEDULER.lock().unwrap();
            sched.slots.retain(|(_, slot)| slot.is_alive());
            for (_, slot) in &mut sched.slots {
                slot.tick();
            }
        }

        // Tasks may lock `port_manager`, and `port_manager::flash` calls
        // `scheduler::register_task` under the `port_manager` lock — so the
        // scheduler lock must NOT be held while a task runs, or we risk
        // deadlock. Move the task list out, run outside the lock, then
        // merge any tasks that were registered mid-tick.
        let mut tasks = {
            let mut sched = SCHEDULER.lock().unwrap();
            std::mem::take(&mut sched.tasks)
        };
        tasks.retain_mut(|(_, task)| task());

        let should_exit = {
            let mut sched = SCHEDULER.lock().unwrap();
            let added_during_tick = std::mem::take(&mut sched.tasks);
            sched.tasks = tasks;
            sched.tasks.extend(added_during_tick);

            if sched.slots.is_empty() && sched.tasks.is_empty() {
                sched.thread_running = false;
                true
            } else {
                false
            }
        };
        if should_exit {
            return;
        }

        // Sleep until next tick boundary.
        let elapsed = tick_start.elapsed();
        if elapsed < TICK_INTERVAL {
            std::thread::sleep(TICK_INTERVAL - elapsed);
        }
    }
}

#[cfg(test)]
#[path = "tests/scheduler.rs"]
mod tests;
