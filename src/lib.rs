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

unsafe impl<T> Send for RingBuffer<T>{ }
unsafe impl<T> Sync for RingBuffer<T>{ }

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

    pub fn try_push(&self, item: T) -> Option<()> {
        let write_pos = self.write_pos.load(Ordering::Acquire);
        let read_pos = self.read_pos.load(Ordering::Acquire);
        if write_pos - read_pos == self.size {
            return None;
        }
        unsafe {
            let mut items = &mut *self.items.get();
            mem::replace(&mut items[write_pos % self.size], Some(item));
        }
        self.write_pos.store(write_pos + 1, Ordering::Release);
        Some(())
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

    pub fn try_pop(&self) -> Option<T> {
        let read_pos = self.read_pos.load(Ordering::Acquire);
        let write_pos = self.write_pos.load(Ordering::Acquire);
        if write_pos == read_pos {
            return None;
        }
        let item = unsafe {
            let mut items = &mut *self.items.get();
            mem::replace(&mut items[read_pos % self.size], None)
        };
        self.read_pos.store(read_pos + 1, Ordering::Release);
        Some(item.unwrap())
    }

    pub fn write(&self, buffer: &[T]) where T: Clone {
        for item in buffer {
            self.push(item.clone());
        }
    }

    pub fn try_write(&self, buffer: &[T]) -> usize where T: Clone {
        let mut counter = 0;
        for item in buffer {
            if let None = self.try_push(item.clone()) {
                return counter;
            }
            counter += 1;
         }
        counter
    }

    pub fn read(&self, size: usize) -> Vec<T> {
        let mut v = Vec::with_capacity(size);
        for _ in 0..size {
            v.push(self.pop());
        }
        v
    }

    pub fn try_read(&self, size: usize) -> Vec<T> {
        let mut v = Vec::with_capacity(size);
        for _ in 0..size {
            if let Some(i) = self.try_pop() {
                v.push(i);
            } else {
                return v;
            }
        }
        v
    }

    pub fn len(&self) -> usize {
        let read_pos = self.read_pos.load(Ordering::Acquire);
        let write_pos = self.write_pos.load(Ordering::Acquire);
        write_pos - read_pos
    }
}

#[cfg(test)]
mod tests {
    use ::RingBuffer;
    #[test]
    fn push_pop() {
        let rb = RingBuffer::new(1);
        rb.push(1);
        assert_eq!(rb.len(), 1);
        assert_eq!(rb.pop(), 1);
        assert_eq!(rb.len(), 0);
    }

    #[test]
    fn try_push_pop() {
        let rb = RingBuffer::new(2);
        assert_eq!(Some(()), rb.try_push(1));
        assert_eq!(Some(()), rb.try_push(2));
        assert_eq!(None, rb.try_push(3));
        assert_eq!(Some(1), rb.try_pop());
        assert_eq!(Some(2), rb.try_pop());
        assert_eq!(None, rb.try_pop());
    }

    #[test]
    fn read() {
        let rb = RingBuffer::new(3);
        rb.push(1);
        rb.push(2);
        rb.push(3);
        assert_eq!(rb.read(3), vec![1,2,3]);
    }

    #[test]
    fn write() {
        let rb = RingBuffer::new(3);
        rb.write(&vec![1,2,3]);
        assert_eq!(rb.read(3), vec![1,2,3]);
    }

    #[test]
    fn try_read() {
        let rb = RingBuffer::new(3);
        rb.push(1);
        rb.push(2);
        rb.push(3);
        assert_eq!(rb.try_read(4), vec![1,2,3]);
    }

    #[test]
    fn try_write() {
        let rb = RingBuffer::new(3);
        rb.try_write(&vec![1,2,3,4]);
        assert_eq!(rb.read(3), vec![1,2,3]);
    }
}
