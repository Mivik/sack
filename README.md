# sack

[![crates.io](https://img.shields.io/crates/v/sack.svg)](https://crates.io/crates/sack)
[![docs.rs](https://docs.rs/sack/badge.svg)](https://docs.rs/sack)

A lock-free sack data structure and a `WakerSet` for waking multiple tasks at once.

## Overview

This crate provides two data structures:

- `Sack<T>`: A concurrent, lock-free sack that supports adding and draining items.
- `WakerSet`: A set of wakers that can be woken all at once.

`Sack<T>` is implemented as a lock-free, singly-linked list, and `WakerSet` is a wrapper around a `Sack<Waker>`.

Generally this provides better performance than `Mutex<Vec<T>>` for small numbers of entries. However it can vary depending on the specific use case and access patterns. Always benchmark your code to find the best solution for your particular scenario.

## Usage

Add this to your `Cargo.toml`:

```toml
[dependencies]
sack = "0.1.0"
```

### Sack

Here is an example of how to use `Sack` in a multi-producer, single-consumer scenario:

```rust
use sack::Sack;
use std::sync::Arc;
use std::thread;

let sack = Arc::new(Sack::new());

// Spawn a producer thread.
let producer = {
    let sack = Arc::clone(&sack);
    thread::spawn(move || {
        for i in 0..10 {
            sack.add(i);
        }
    })
};

// Wait for the producer to finish.
producer.join().unwrap();

// Drain the sack and collect the items.
let mut items: Vec<_> = sack.drain().collect();
items.sort();

assert_eq!(items, (0..10).collect::<Vec<_>>());
```

### WakerSet

Here is an example of how to use `WakerSet` to wake up multiple tasks:

```rust
use sack::WakerSet;
use std::sync::Arc;
use std::task::{Wake, Waker};

// Create a new WakeSet.
let wake_set = Arc::new(WakerSet::new());

// Add the waker to the set.
wake_set.add_by_ref(Waker::noop());

// Wake all wakers in the set.
assert_eq!(wake_set.wake_all(), 1);
```

## API

The API is documented on [docs.rs](https://docs.rs/sack).

## License

This project is licensed under the MIT license.
