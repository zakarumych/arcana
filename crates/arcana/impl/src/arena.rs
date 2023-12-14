use std::{
    alloc::Layout,
    boxed,
    cell::{Cell, RefCell, RefMut, UnsafeCell},
    mem,
    ptr::NonNull,
};

/// Very simple typed arena.
pub struct Arena<T> {
    head: Head<T>,
    tail: RefCell<Vec<Exhausted<T>>>,
    count: Cell<usize>,
}

struct Exhausted<T> {
    ptr: NonNull<T>,
    len: usize,
}

impl<T> Exhausted<T> {
    fn is_empty(&self) -> bool {
        self.len == 0
    }

    fn drain(mut self) -> impl Iterator<Item = T> {
        let len = self.len;

        // Prevent dropping elements that are being drained.
        self.len = 0;

        (0..len).map(move |i| {
            let ptr = unsafe { self.ptr.as_ptr().add(i) };
            unsafe { ptr.read() }
        })
    }
}

impl<T> Drop for Exhausted<T> {
    fn drop(&mut self) {
        unsafe {
            let slice = std::ptr::slice_from_raw_parts_mut(self.ptr.as_ptr(), self.len);
            std::ptr::drop_in_place::<[T]>(slice);
        }
    }
}

struct Head<T> {
    ptr: Cell<NonNull<T>>,
    cap: Cell<usize>,
    len: Cell<usize>,
}

impl<T> Drop for Head<T> {
    fn drop(&mut self) {
        let len = self.len.get();
        unsafe {
            let slice = std::ptr::slice_from_raw_parts_mut(self.ptr.get().as_ptr(), len);
            std::ptr::drop_in_place::<[T]>(slice);
        }
    }
}

impl<T> Head<T> {
    const fn empty() -> Self {
        Head {
            ptr: Cell::new(NonNull::dangling()),
            cap: Cell::new(0),
            len: Cell::new(0),
        }
    }

    fn new(cap: usize) -> Self {
        if cap == 0 {
            return Head::empty();
        }

        let layout = Layout::array::<T>(cap).unwrap();
        let ptr = unsafe { std::alloc::alloc(layout) } as *mut T;

        Head {
            ptr: Cell::new(NonNull::new(ptr).unwrap()),
            cap: Cell::new(cap),
            len: Cell::new(0),
        }
    }

    fn put(&self, value: T) -> &mut T {
        debug_assert!(self.len.get() < self.cap.get());

        let len = self.len.get();
        let ptr = unsafe { self.ptr.get().as_ptr().add(len) };
        unsafe { ptr.write(value) };
        self.len.set(len + 1);

        // SAFETY: Value was just written.
        unsafe { &mut *ptr }
    }

    fn is_exhausted(&self) -> bool {
        self.len.get() == self.cap.get()
    }

    unsafe fn reallocate(&self, new_cap: usize) -> Exhausted<T> {
        debug_assert_eq!(self.len.get(), self.cap.get());

        let ptr = self.ptr.get();
        let len = self.len.get();
        self.ptr.set(NonNull::dangling());
        self.cap.set(0);
        self.len.set(0);

        let exhausted = Exhausted { ptr, len };

        let layout = Layout::array::<T>(new_cap).unwrap();
        let ptr = unsafe { std::alloc::alloc(layout) } as *mut T;

        self.ptr.set(NonNull::new(ptr).unwrap());
        self.cap.set(new_cap);

        exhausted
    }

    fn reset(&mut self) {
        let len = self.len.get();

        // Reset length to zero before dropping elements.
        // If dropping panics, we'll leak elements instead of dropping them twice.
        self.len.set(0);

        unsafe {
            let slice = std::ptr::slice_from_raw_parts_mut(self.ptr.get().as_ptr(), len);
            std::ptr::drop_in_place::<[T]>(slice);
        }
    }

    fn drain(&mut self) -> impl Iterator<Item = T> + '_ {
        let len = self.len.get();

        // Reset length to zero before draining elements.
        // If draining panics, we'll leak elements instead of draining them twice.
        self.len.set(0);

        (0..len).map(move |i| {
            let ptr = unsafe { self.ptr.get().as_ptr().add(i) };
            unsafe { ptr.read() }
        })
    }
}

impl<T> Arena<T> {
    const MIN_NON_ZERO_CAP: usize = if mem::size_of::<T>() == 1 {
        8
    } else if mem::size_of::<T>() <= 1024 {
        4
    } else {
        1
    };

    pub const fn new() -> Self {
        Arena {
            head: Head::empty(),
            tail: RefCell::new(Vec::new()),
            count: Cell::new(0),
        }
    }

    pub fn put(&self, value: T) -> &mut T {
        if self.head.is_exhausted() {
            let new_cap = self
                .count
                .get()
                .next_power_of_two()
                .min(Self::MIN_NON_ZERO_CAP);

            let exhausted = unsafe { self.head.reallocate(new_cap) };

            if !exhausted.is_empty() {
                self.tail.borrow_mut().push(exhausted);
            }
        }

        debug_assert!(!self.head.is_exhausted());
        self.count.set(self.count.get() + 1);
        self.head.put(value)
    }

    /// Reset the arena, dropping all allocated values.
    pub fn reset(&mut self) {
        self.head.reset();
        self.tail.borrow_mut().clear();
        self.count.set(0);
    }

    /// Reset the arena, draining all allocated values.
    pub fn drain(&mut self) -> impl Iterator<Item = T> + '_ {
        self.count.set(0);

        let tail = self.tail.get_mut();

        DropExhaust {
            iter: tail
                .drain(..)
                .flat_map(|e| e.drain())
                .chain(self.head.drain()),
        }
    }
}

struct DropExhaust<I: Iterator> {
    iter: I,
}

impl<I> Drop for DropExhaust<I>
where
    I: Iterator,
{
    fn drop(&mut self) {
        for _ in &mut self.iter {}
    }
}

impl<I> Iterator for DropExhaust<I>
where
    I: Iterator,
{
    type Item = I::Item;

    #[inline(always)]
    fn next(&mut self) -> Option<Self::Item> {
        self.iter.next()
    }
}
