use super::*;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// Counts its own ticks on an `AtomicUsize`, so tests can wait until a
/// known number of ticks have elapsed without relying on wall-clock
/// sleeps. `is_alive` returns whatever the shared flag says, letting tests
/// retire the slot from outside.
struct CountingSlot {
    ticks: Arc<AtomicUsize>,
    alive: Arc<AtomicBool>,
}

impl DeviceSlot for CountingSlot {
    fn tick(&mut self) {
        self.ticks.fetch_add(1, Ordering::SeqCst);
    }
    fn is_alive(&self) -> bool {
        self.alive.load(Ordering::SeqCst)
    }
}

fn wait_for<F: Fn() -> bool>(cond: F, timeout: Duration) -> bool {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        if cond() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    cond()
}

// ── Slot registration ───────────────────────────

#[test]
fn test_register_slot_returns_unique_ids() {
    let alive = Arc::new(AtomicBool::new(true));
    let ticks = Arc::new(AtomicUsize::new(0));
    let slot_a = Box::new(CountingSlot { ticks: ticks.clone(), alive: alive.clone() });
    let slot_b = Box::new(CountingSlot { ticks: ticks.clone(), alive: alive.clone() });

    let id_a = register_slot(slot_a);
    let id_b = register_slot(slot_b);
    assert_ne!(id_a, id_b, "register_slot must return unique IDs");

    // Cleanup.
    alive.store(false, Ordering::SeqCst);
    deregister_slot(id_a);
    deregister_slot(id_b);
}

#[test]
fn test_registered_slot_gets_ticked() {
    let alive = Arc::new(AtomicBool::new(true));
    let ticks = Arc::new(AtomicUsize::new(0));
    let slot = Box::new(CountingSlot { ticks: ticks.clone(), alive: alive.clone() });

    let id = register_slot(slot);
    // The scheduler ticks at 60 Hz; at least one tick should land within
    // a generous 500 ms window.
    let ok = wait_for(|| ticks.load(Ordering::SeqCst) > 0, Duration::from_millis(500));
    assert!(ok, "slot was never ticked");

    alive.store(false, Ordering::SeqCst);
    deregister_slot(id);
}

#[test]
fn test_deregister_slot_stops_ticks() {
    let alive = Arc::new(AtomicBool::new(true));
    let ticks = Arc::new(AtomicUsize::new(0));
    let slot = Box::new(CountingSlot { ticks: ticks.clone(), alive: alive.clone() });

    let id = register_slot(slot);
    wait_for(|| ticks.load(Ordering::SeqCst) > 0, Duration::from_millis(500));
    deregister_slot(id);

    // Give any in-flight tick a moment to finish, then snapshot.
    std::thread::sleep(Duration::from_millis(50));
    let after_dereg = ticks.load(Ordering::SeqCst);
    std::thread::sleep(Duration::from_millis(100));
    let later = ticks.load(Ordering::SeqCst);
    assert_eq!(
        after_dereg, later,
        "slot continued ticking after deregister"
    );

    alive.store(false, Ordering::SeqCst);
}

#[test]
fn test_slot_with_is_alive_false_is_reaped() {
    let alive = Arc::new(AtomicBool::new(true));
    let ticks = Arc::new(AtomicUsize::new(0));
    let slot = Box::new(CountingSlot { ticks: ticks.clone(), alive: alive.clone() });

    register_slot(slot);
    wait_for(|| ticks.load(Ordering::SeqCst) > 0, Duration::from_millis(500));
    alive.store(false, Ordering::SeqCst);

    // Next tick must notice is_alive=false and drop the slot.
    std::thread::sleep(Duration::from_millis(50));
    let snapshot = ticks.load(Ordering::SeqCst);
    std::thread::sleep(Duration::from_millis(100));
    let later = ticks.load(Ordering::SeqCst);
    assert_eq!(
        snapshot, later,
        "slot that reported is_alive=false kept getting ticked"
    );
}

#[test]
fn test_deregister_unknown_id_is_noop() {
    // Deregistering an ID that never existed must not panic or poison
    // internal state. Subsequent registrations should still work.
    deregister_slot(usize::MAX);

    let alive = Arc::new(AtomicBool::new(true));
    let ticks = Arc::new(AtomicUsize::new(0));
    let slot = Box::new(CountingSlot { ticks: ticks.clone(), alive: alive.clone() });
    let id = register_slot(slot);

    assert!(wait_for(
        || ticks.load(Ordering::SeqCst) > 0,
        Duration::from_millis(500)
    ));
    alive.store(false, Ordering::SeqCst);
    deregister_slot(id);
}

// ── Periodic tasks ──────────────────────────────

#[test]
fn test_periodic_task_runs_and_retires() {
    let runs = Arc::new(AtomicUsize::new(0));
    let runs_inner = runs.clone();

    register_task(Box::new(move || {
        let n = runs_inner.fetch_add(1, Ordering::SeqCst) + 1;
        // Retire after three runs.
        n < 3
    }));

    let ok = wait_for(|| runs.load(Ordering::SeqCst) >= 3, Duration::from_millis(500));
    assert!(ok, "task did not reach three runs");

    std::thread::sleep(Duration::from_millis(100));
    let snap = runs.load(Ordering::SeqCst);
    std::thread::sleep(Duration::from_millis(100));
    assert_eq!(
        snap,
        runs.load(Ordering::SeqCst),
        "task kept running after returning false"
    );
}

#[test]
fn test_task_can_register_another_task_mid_tick() {
    // This exercises the reentrancy path in `scheduler_loop`: tasks are
    // run outside the scheduler lock and can call `register_task`. The
    // loop must merge the newly-registered task before the next iteration.
    let chain = Arc::new(Mutex::new(Vec::<&'static str>::new()));
    let chain_a = chain.clone();
    let chain_b_outer = chain.clone();

    register_task(Box::new(move || {
        chain_a.lock().unwrap().push("a");
        let chain_b = chain_b_outer.clone();
        // On first run, schedule a second task.
        if chain_a.lock().unwrap().len() == 1 {
            register_task(Box::new(move || {
                chain_b.lock().unwrap().push("b");
                false // retire
            }));
        }
        false // retire
    }));

    let ok = wait_for(
        || {
            let v = chain.lock().unwrap();
            v.contains(&"a") && v.contains(&"b")
        },
        Duration::from_millis(500),
    );
    assert!(ok, "nested task was not picked up by the scheduler");
}

// ── Concurrent registration ─────────────────────

#[test]
fn test_concurrent_registrations_all_tick() {
    // Spawn several threads that each register a slot and wait for it
    // to tick at least once. Exercises the scheduler's registration lock
    // under contention and confirms every slot really does run.
    let handles: Vec<_> = (0..8)
        .map(|_| {
            std::thread::spawn(|| {
                let alive = Arc::new(AtomicBool::new(true));
                let ticks = Arc::new(AtomicUsize::new(0));
                let slot = Box::new(CountingSlot {
                    ticks: ticks.clone(),
                    alive: alive.clone(),
                });
                let id = register_slot(slot);
                let ok = wait_for(
                    || ticks.load(Ordering::SeqCst) > 0,
                    Duration::from_millis(500),
                );
                alive.store(false, Ordering::SeqCst);
                deregister_slot(id);
                ok
            })
        })
        .collect();

    for h in handles {
        assert!(
            h.join().unwrap(),
            "one of the concurrent slots never ticked"
        );
    }
}
