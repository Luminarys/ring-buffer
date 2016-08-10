use std::sync::atomic::{AtomicUsize, Ordering};
use std::cell::UnsafeCell;
use std::mem;
use std::thread;
use std::time::Duration;

/// Lockfree SPSC fixed size ring buffer.
pub struct RingBuffer<T> {
    size: usize,
    items: UnsafeCell<Vec<Option<T>>>,
    write_pos: AtomicUsize,
    read_pos: AtomicUsize,
}

impl<T> RingBuffer<T> {
    pub fn new(size: usize) -> RingBuffer<T> {
        let items = (0..size).map(|_| None).collect();
        RingBuffer {
            size: size,
            items: UnsafeCell::new(items),
            write_pos: AtomicUsize::new(0),
            read_pos: AtomicUsize::new(0),
        }
    }

    pub fn push(&self, item: T) {
        let write_pos = self.write_pos.load(Ordering::Acquire);
        loop {
            let read_pos = self.read_pos.load(Ordering::Acquire);
            if write_pos - read_pos != self.size {
                break;
            } else {
                thread::park_timeout(Duration::from_millis(10));
            }
        }

        unsafe {
            let mut items = &mut *self.items.get();
            mem::replace(&mut items[write_pos % self.size], Some(item));
        }
        self.write_pos.store(write_pos + 1, Ordering::Release);
    }

    pub fn pop(&self) -> T {
        let read_pos = self.read_pos.load(Ordering::Acquire);
        loop {
            let write_pos = self.write_pos.load(Ordering::Acquire);
            if write_pos != read_pos {
                break;
            } else {
                thread::park_timeout(Duration::from_millis(10));
            }
        }

        let item = unsafe {
            let mut items = &mut *self.items.get();
            mem::replace(&mut items[read_pos % self.size], None)
        };
        self.read_pos.store(read_pos + 1, Ordering::Release);
        item.unwrap()
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
    }
}
