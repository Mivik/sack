use std::{
    mem,
    sync::{
        Arc,
        atomic::{AtomicU8, Ordering},
    },
    task::Waker,
    time::{Duration, Instant},
};

use criterion::{Criterion, criterion_group, criterion_main};
use parking_lot::Mutex;
use sack::WakerSet;

trait BenchOps: Default {
    fn add(&self, waker: Waker);
    fn wake(&self);
}
impl BenchOps for WakerSet {
    fn add(&self, waker: Waker) {
        self.add(waker);
    }

    fn wake(&self) {
        self.wake_all();
    }
}

#[derive(Default)]
struct LockedVec(Mutex<Vec<Waker>>);
impl BenchOps for LockedVec {
    fn add(&self, waker: Waker) {
        self.0.lock().push(waker);
    }
    fn wake(&self) {
        let vec = mem::take(&mut *self.0.lock());
        for waker in vec {
            waker.wake();
        }
    }
}

fn bench<B: BenchOps>() {
    let b = B::default();
    for _ in 0..16 {
        b.add(Waker::noop().clone());
    }
    b.wake();
}

fn bench_mt<B: BenchOps + Send + Sync>(iters: u64) -> Duration {
    let b = Arc::new(B::default());

    let start = Instant::now();
    crossbeam_utils::thread::scope(|s| {
        let counter = Arc::new(AtomicU8::new(0));
        for _ in 0..4 {
            let b = b.clone();
            let counter = counter.clone();
            s.spawn(move |_| {
                for _ in 0..iters {
                    let count = counter.fetch_add(1, Ordering::Relaxed);
                    if count % 16 == 0 {
                        b.wake();
                    } else {
                        b.add(Waker::noop().clone());
                    }
                }
            });
        }
    })
    .unwrap();

    b.wake();
    start.elapsed()
}

pub fn criterion_benchmark(c: &mut Criterion) {
    c.bench_function("wake set", |b| b.iter(bench::<WakerSet>));
    c.bench_function("locked vec", |b| b.iter(bench::<LockedVec>));

    c.bench_function("wake set mt", |b| b.iter_custom(bench_mt::<WakerSet>));
    c.bench_function("locked vec mt", |b| b.iter_custom(bench_mt::<LockedVec>));
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
