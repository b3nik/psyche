use std::sync::{Arc, Condvar, Mutex};
use std::time::Duration;

/// Error returned when attempting to wait on a cancelled barrier
#[derive(Debug, Clone, Copy)]
pub struct CancelledBarrier {}

/// A synchronization primitive that allows multiple threads to wait at a point until
/// enough threads have arrived or the barrier is cancelled
#[derive(Debug)]
pub struct CancellableBarrier {
    mutex: Mutex<BarrierState>,
    condvar: Condvar,
}

#[derive(Debug)]
struct BarrierState {
    count: usize,
    total: usize,
    generation: usize,
    cancelled: bool,
}

impl CancellableBarrier {
    /// Creates a new barrier that can be used by `n` threads
    #[must_use]
    pub fn new(n: usize) -> Arc<Self> {
        assert!(n > 0, "Barrier size must be greater than 0");
        Arc::new(CancellableBarrier {
            mutex: Mutex::new(BarrierState {
                count: 0,
                total: n,
                generation: 0,
                cancelled: false,
            }),
            condvar: Condvar::new(),
        })
    }

    /// Waits until all threads have reached the barrier or the barrier is cancelled
    pub fn wait(&self) -> Result<usize, CancelledBarrier> {
        let mut state = self.mutex.lock().unwrap();

        if state.cancelled {
            return Err(CancelledBarrier {});
        }

        let generation = state.generation;
        state.count += 1;

        if state.count < state.total {
            // Not all threads have arrived yet
            while state.count < state.total && state.generation == generation && !state.cancelled {
                state = self.condvar.wait(state).unwrap();
            }

            if state.cancelled {
                return Err(CancelledBarrier {});
            }
        } else {
            // Last thread to arrive
            state.count = 0;
            state.generation += 1;
            self.condvar.notify_all();
        }

        Ok(generation)
    }

    /// Cancels the barrier, causing all waiting threads to return with an error
    pub fn cancel(&self) {
        let mut state = self.mutex.lock().unwrap();
        state.cancelled = true;
        self.condvar.notify_all();
    }

    /// Resets the barrier to its initial state
    pub fn reset(&self) {
        let mut state = self.mutex.lock().unwrap();
        state.cancelled = false;
        state.count = 0;
        state.generation += 1;
    }

    /// Returns true if the barrier is currently cancelled
    #[must_use]
    pub fn is_cancelled(&self) -> bool {
        self.mutex.lock().unwrap().cancelled
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;

    const TEST_SLEEP_DURATION: Duration = Duration::from_millis(100);

    #[test]
    fn test_basic_barrier() {
        let barrier = CancellableBarrier::new(3);
        let barrier_clone1 = barrier.clone();
        let barrier_clone2 = barrier.clone();

        let t1 = thread::spawn(move || {
            barrier.wait().unwrap();
        });

        let t2 = thread::spawn(move || {
            barrier_clone1.wait().unwrap();
        });

        let t3 = thread::spawn(move || {
            barrier_clone2.wait().unwrap();
        });

        t1.join().unwrap();
        t2.join().unwrap();
        t3.join().unwrap();
    }

    #[test]
    fn test_cancel_barrier() {
        let barrier = CancellableBarrier::new(3);
        let barrier_clone1 = barrier.clone();
        let barrier_clone2 = barrier.clone();

        let t1 = thread::spawn(move || {
            thread::sleep(TEST_SLEEP_DURATION);
            barrier.wait()
        });

        let t2 = thread::spawn(move || {
            thread::sleep(TEST_SLEEP_DURATION);
            barrier_clone1.wait()
        });

        let t3 = thread::spawn(move || {
            barrier_clone2.cancel();
            barrier_clone2.wait()
        });

        assert!(t1.join().unwrap().is_err());
        assert!(t2.join().unwrap().is_err());
        assert!(t3.join().unwrap().is_err());
    }

    #[test]
    fn test_reset_barrier() {
        let barrier = CancellableBarrier::new(2);
        let barrier_clone1 = barrier.clone();

        // First, cancel the barrier
        barrier.cancel();
        assert!(barrier.wait().is_err());

        // Reset the barrier
        barrier.reset();

        // Now it should work again
        let t1 = thread::spawn(move || {
            barrier.wait().unwrap();
        });

        let t2 = thread::spawn(move || {
            barrier_clone1.wait().unwrap();
        });

        t1.join().unwrap();
        t2.join().unwrap();
    }
}