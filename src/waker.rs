use core::task::Waker;

use alloc::{sync::Arc, task::Wake};

use crate::Sack;

/// A set of wakers that can be woken all at once.
///
/// This is useful for implementing synchronization primitives that need to wake up multiple tasks.
#[derive(Default)]
pub struct WakerSet(Sack<Waker>);

impl WakerSet {
    /// Creates a new, empty `WakerSet`.
    pub const fn new() -> Self {
        Self(Sack::new())
    }

    /// Adds a waker to the set.
    pub fn add(&self, waker: Waker) {
        self.0.add(waker);
    }

    /// Adds a waker to the set by reference.
    pub fn add_by_ref(&self, waker: &Waker) {
        self.0.add(waker.clone());
    }

    /// Wakes all wakers in the set.
    ///
    /// Returns the number of wakers that were woken.
    pub fn wake_all(&self) -> usize {
        let mut count = 0;
        for waker in self.0.drain() {
            waker.wake();
            count += 1;
        }
        count
    }

    /// Clears all wakers from the set without waking them.
    ///
    /// Returns the number of wakers that were cleared.
    pub fn clear(&self) -> usize {
        self.0.drain().count()
    }

    /// Checks if the set is empty.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl Drop for WakerSet {
    fn drop(&mut self) {
        self.wake_all();
    }
}

impl Wake for WakerSet {
    fn wake(self: Arc<Self>) {
        self.wake_all();
    }

    fn wake_by_ref(self: &Arc<Self>) {
        self.wake_all();
    }
}
