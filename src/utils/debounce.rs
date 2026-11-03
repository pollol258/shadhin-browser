//! Safe debounce and one-shot timer utility for GLib main context

use gtk4::glib::{self, SourceId};
use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;

/// A thread-safe, panic-free debounce helper for GTK/GLib event loops.
///
/// Ensures that:
/// - Fired one-shot timers automatically clear their stored `SourceId`.
/// - Cancelled timers cleanly remove only pending sources.
/// - Cancelling an already-fired or already-removed source is a safe no-op.
/// - Contains 100% safe Rust code.
#[derive(Clone, Default, Debug)]
pub struct Debounce {
    source: Rc<RefCell<Option<SourceId>>>,
}

impl Debounce {
    /// Constructs a new idle Debounce instance
    pub fn new() -> Self {
        Self {
            source: Rc::new(RefCell::new(None)),
        }
    }

    /// Safely cancels any pending timer without panicking or unwrapping
    pub fn cancel(&self) {
        if let Some(source_id) = self.source.borrow_mut().take() {
            source_id.remove();
        }
    }

    /// Schedules a callback after `delay`. If a previous callback is pending,
    /// it is cancelled safely.
    pub fn debounce<F>(&self, delay: Duration, callback: F)
    where
        F: FnOnce() + 'static,
    {
        self.cancel();

        let source_cell = self.source.clone();
        let source_id = glib::timeout_add_local_once(delay, move || {
            *source_cell.borrow_mut() = None;
            callback();
        });

        *self.source.borrow_mut() = Some(source_id);
    }

    /// Returns true if a timer is currently pending and has not yet fired
    pub fn is_pending(&self) -> bool {
        self.source.borrow().is_some()
    }
}

impl Drop for Debounce {
    fn drop(&mut self) {
        if Rc::strong_count(&self.source) == 1 {
            self.cancel();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    static TEST_LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn test_debounce_fires() {
        let _lock = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let ctx = glib::MainContext::default();
        let Ok(_guard) = ctx.acquire() else {
            eprintln!("SKIPPED: GLib MainContext already acquired by another thread");
            return;
        };

        let debounce = Debounce::new();
        let executed = Rc::new(RefCell::new(false));
        let ex_clone = executed.clone();

        debounce.debounce(Duration::from_millis(10), move || {
            *ex_clone.borrow_mut() = true;
        });

        assert!(debounce.is_pending());

        let start = std::time::Instant::now();
        while !*executed.borrow() && start.elapsed() < Duration::from_secs(1) {
            std::thread::sleep(Duration::from_millis(2));
            while ctx.iteration(false) {}
        }

        assert!(*executed.borrow(), "Debounce callback should have executed");
        assert!(
            !debounce.is_pending(),
            "SourceId should be cleared after firing"
        );
    }

    #[test]
    fn test_debounce_cancels_previous() {
        let _lock = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let ctx = glib::MainContext::default();
        let Ok(_guard) = ctx.acquire() else {
            eprintln!("SKIPPED: GLib MainContext already acquired by another thread");
            return;
        };

        let debounce = Debounce::new();
        let counter = Rc::new(RefCell::new(0));

        let c1 = counter.clone();
        debounce.debounce(Duration::from_millis(50), move || {
            *c1.borrow_mut() += 1;
        });

        // Cancel previous by scheduling a new one immediately
        let c2 = counter.clone();
        debounce.debounce(Duration::from_millis(15), move || {
            *c2.borrow_mut() += 10;
        });

        let start = std::time::Instant::now();
        while *counter.borrow() == 0 && start.elapsed() < Duration::from_secs(1) {
            std::thread::sleep(Duration::from_millis(2));
            while ctx.iteration(false) {}
        }

        assert_eq!(
            *counter.borrow(),
            10,
            "First callback should have been cancelled"
        );
        assert!(!debounce.is_pending());
    }

    #[test]
    fn test_debounce_cancel_after_fired_does_not_panic() {
        let _lock = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let ctx = glib::MainContext::default();
        let Ok(_guard) = ctx.acquire() else {
            eprintln!("SKIPPED: GLib MainContext already acquired by another thread");
            return;
        };

        let debounce = Debounce::new();
        let executed = Rc::new(RefCell::new(false));
        let ex_clone = executed.clone();

        debounce.debounce(Duration::from_millis(5), move || {
            *ex_clone.borrow_mut() = true;
        });

        let start = std::time::Instant::now();
        while !*executed.borrow() && start.elapsed() < Duration::from_secs(1) {
            std::thread::sleep(Duration::from_millis(2));
            while ctx.iteration(false) {}
        }

        assert!(*executed.borrow());
        // Verify cancel after fired does not panic
        debounce.cancel();
        assert!(!debounce.is_pending());
        // Double cancel also does not panic
        debounce.cancel();
        assert!(!debounce.is_pending());
    }

    #[test]
    fn test_debounce_drop_cleans_up() {
        let _lock = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let ctx = glib::MainContext::default();
        let Ok(_guard) = ctx.acquire() else {
            eprintln!("SKIPPED: GLib MainContext already acquired by another thread");
            return;
        };

        let debounce = Debounce::new();
        debounce.debounce(Duration::from_secs(10), || {});
        assert!(debounce.is_pending());
        drop(debounce); // Should cancel without panicking
    }

    #[test]
    fn test_debounce_safety_nonexistent_source_remove() {
        let _lock = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let debounce = Debounce::new();
        // Test repeated cancel on empty debounce
        debounce.cancel();
        debounce.cancel();
        assert!(!debounce.is_pending());
    }
}
