#![cfg_attr(not(test), no_std)]
#![doc = include_str!("../README.md")]

//! A lock-free data structure.
//!
//! This crate provides a `Sack<T>` type, which is a concurrent, lock-free
//! collection that supports adding and draining items. See [`Sack<T>`] for more
//! details.
//!
//! This crate also provides a `WakerSet` type, which is a set of wakers that can
//! be woken all at once. This is useful for implementing synchronization
//! primitives that need to wake up multiple tasks.

extern crate alloc;

use core::{
    ptr,
    sync::atomic::{AtomicPtr, Ordering},
};

use alloc::boxed::Box;

#[cfg(feature = "waker")]
mod waker;
#[cfg(feature = "waker")]
pub use waker::*;

/// A single entry in the sack.
struct Entry<T> {
    /// The item stored in the entry.
    item: T,
    /// A pointer to the next entry in the sack.
    next: *mut Entry<T>,
}

/// A lock-free sack data structure.
///
/// A sack is a concurrent data structure that allows adding items and draining
/// them in a lock-free manner. It is implemented as a singly-linked list where
/// the head is an atomic pointer. This allows multiple producers to add items
/// concurrently without locks.
///
/// ## How it works
///
/// The `Sack` is essentially a LIFO (last-in, first-out) stack. When an item is
/// added, it is pushed to the front of the list. When the sack is drained, the
/// entire list is atomically swapped with an empty list, and the old list is
/// returned as a draining iterator.
///
/// This design has the following properties:
///
/// * **Lock-free:** Adding and draining items are lock-free operations, which
///   means they don't require mutual exclusion. This makes them very fast and
///   scalable.
/// * **Concurrent producers:** Multiple threads can add items to the sack
///   concurrently.
/// * **Single consumer:** Only one thread can drain the sack at a time. This is
///   enforced by the `&self` receiver on the `drain` method.
///
/// ## Example
///
/// ```
/// use sack::Sack;
/// use std::sync::Arc;
/// use std::thread;
///
/// let sack = Arc::new(Sack::new());
///
/// // Spawn a producer thread.
/// let producer = {
///     let sack = Arc::clone(&sack);
///     thread::spawn(move || {
///         for i in 0..10 {
///             sack.add(i);
///         }
///     })
/// };
///
/// // Wait for the producer to finish.
/// producer.join().unwrap();
///
/// // Drain the sack and collect the items.
/// let mut items: Vec<_> = sack.drain().collect();
/// items.sort();
///
/// assert_eq!(items, (0..10).collect::<Vec<_>>());
/// ```
pub struct Sack<T> {
    head: AtomicPtr<Entry<T>>,
}

impl<T> Default for Sack<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T> Sack<T> {
    /// Creates a new, empty sack.
    pub const fn new() -> Self {
        Self {
            head: AtomicPtr::new(ptr::null_mut()),
        }
    }

    /// Adds an item to the sack.
    ///
    /// This operation is lock-free and can be called by multiple threads concurrently.
    pub fn add(&self, item: T) {
        let entry = Box::leak(Box::new(Entry {
            item,
            next: ptr::null_mut(),
        }));

        entry.next = self.head.load(Ordering::Acquire);
        loop {
            match self.head.compare_exchange_weak(
                entry.next,
                entry,
                Ordering::Release,
                Ordering::Acquire,
            ) {
                Ok(_) => break,
                Err(current) => entry.next = current,
            }
        }
    }

    /// Drains all items from the sack.
    ///
    /// This operation is lock-free and returns a draining iterator over the items in the sack.
    pub fn drain(&self) -> Drain<T> {
        let head = self.head.swap(ptr::null_mut(), Ordering::AcqRel);
        Drain::new(head)
    }

    /// Checks if the sack is empty.
    ///
    /// This operation is lock-free.
    pub fn is_empty(&self) -> bool {
        self.head.load(Ordering::Acquire).is_null()
    }
}

/// A draining iterator for [`Sack<T>`].
///
/// This struct is created by [`Sack<T>::drain`]. See its documentation for more.
pub struct Drain<T>(Option<Box<Entry<T>>>);

impl<T> Drain<T> {
    /// Creates a new draining iterator from a pointer to the head of the sack.
    fn new(ptr: *mut Entry<T>) -> Self {
        let head = if ptr.is_null() {
            None
        } else {
            Some(unsafe { Box::from_raw(ptr) })
        };
        Self(head)
    }
}
impl<T> Iterator for Drain<T> {
    type Item = T;

    fn next(&mut self) -> Option<Self::Item> {
        let entry = self.0.take()?;
        *self = Self::new(entry.next);
        Some(entry.item)
    }
}
impl<T> Drop for Drain<T> {
    fn drop(&mut self) {
        while let Some(entry) = self.0.take() {
            *self = Self::new(entry.next);
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{
        sync::{
            Arc,
            atomic::{AtomicUsize, Ordering},
        },
        task::{Wake, Waker},
        thread, vec,
        vec::Vec,
    };

    use super::*;

    struct CountingWaker {
        count: AtomicUsize,
    }

    impl Wake for CountingWaker {
        fn wake(self: Arc<Self>) {
            self.count.fetch_add(1, Ordering::SeqCst);
        }
    }

    #[test]
    fn test_waker_set() {
        let waker = Arc::new(CountingWaker {
            count: AtomicUsize::new(0),
        });

        let wake_set = WakerSet::new();
        wake_set.add(Waker::from(waker.clone()));
        wake_set.add(Waker::from(waker.clone()));

        assert_eq!(wake_set.wake_all(), 2);
        assert_eq!(waker.count.load(Ordering::SeqCst), 2);
    }

    #[test]
    fn test_sack_add_drain() {
        let sack = Sack::new();
        sack.add(1);
        sack.add(2);
        sack.add(3);

        let mut drained: Vec<_> = sack.drain().collect();
        drained.sort();
        assert_eq!(drained, vec![1, 2, 3]);
    }

    #[test]
    fn test_sack_is_empty() {
        let sack = Sack::new();
        assert!(sack.is_empty());
        sack.add(1);
        assert!(!sack.is_empty());
        let _ = sack.drain();
        assert!(sack.is_empty());
    }

    #[test]
    fn test_sack_concurrent_add() {
        let sack = Arc::new(Sack::new());
        let mut handles = vec![];

        for i in 0..10 {
            let sack = Arc::clone(&sack);
            handles.push(thread::spawn(move || {
                for j in 0..100 {
                    sack.add(i * 100 + j);
                }
            }));
        }

        for handle in handles {
            handle.join().unwrap();
        }

        let mut drained: Vec<_> = sack.drain().collect();
        assert_eq!(drained.len(), 1000);
        drained.sort();
        for (i, item) in drained.into_iter().enumerate() {
            assert_eq!(item, i);
        }
    }
}
