use std::{
    alloc::Layout,
    cell::{Cell, RefCell},
    marker::PhantomData,
    mem,
    ptr::NonNull,
};

use crate::alloc_guard;

/// Very simple typed arena.
/// Is able to hold values of single type.
/// User may put new values and keep mutable references to them.
/// Arena uses interior mutability to allow putting values via shared reference,
/// so it is allowed to put new values while references to previously put values are alive.
///
/// With exclusive access to arena user may drain all put values or drop them.
///
/// Note that arena may grow while references to contained values are alive.
/// This is due to the fact that growth does not move existing values.
/// Instead buffers are chained into a list where all buffers except the root are exhausted -
/// contain maximum number of values.
///
/// On reset all exhausted buffers are deallocated and the root buffer is reset.
/// All values are dropped unless drained.
pub struct Arena<T> {
    head: Head<T>,
    tail: RefCell<Vec<Exhausted<T>>>,
    count: Cell<usize>,
}

unsafe impl<T> Send for Arena<T> where T: Send {}

struct Exhausted<T> {
    ptr: NonNull<T>,
    len: usize,
}

impl<T> Drop for Exhausted<T> {
    fn drop(&mut self) {
        unsafe {
            let slice = std::ptr::slice_from_raw_parts_mut(self.ptr.as_ptr(), self.len);
            std::ptr::drop_in_place::<[T]>(slice);
        }
    }
}

impl<T> Exhausted<T> {
    fn is_empty(&self) -> bool {
        self.len == 0
    }

    fn drain(mut self) -> ExhaustedDrain<T> {
        let len = self.len;

        // Prevent dropping elements that are being drained.
        self.len = 0;

        ExhaustedDrain {
            ptr: self.ptr,
            idx: 0,
            len,
        }
    }
}

struct Head<T> {
    /// Pointer to an array of values.
    ptr: Cell<NonNull<T>>,

    /// Capacity of the array.
    cap: Cell<usize>,

    /// Number of values put into the array.
    /// Indices between `len` and `cap` may be uninitialized.
    len: Cell<usize>,
}

impl<T> Drop for Head<T> {
    fn drop(&mut self) {
        // Drop stored elements.
        let len = self.len.get();
        unsafe {
            let slice = std::ptr::slice_from_raw_parts_mut(self.ptr.get().as_ptr(), len);
            std::ptr::drop_in_place::<[T]>(slice);
        }

        if self.cap.get() != 0 {
            // Deallocate array.
            let layout = Layout::array::<T>(self.cap.get()).unwrap();

            unsafe {
                std::alloc::dealloc(self.ptr.get().as_ptr() as *mut u8, layout);
            }
        }
    }
}

impl<T> Head<T> {
    /// Creates new empty head with zero capacity.
    const fn empty() -> Self {
        Head {
            ptr: Cell::new(NonNull::dangling()),
            cap: Cell::new(0),
            len: Cell::new(0),
        }
    }

    /// Put a value into the head.
    /// Returns mutable reference to the value
    /// bound to the shared head reference lifetime.
    fn put(&self, value: T) -> &mut T {
        debug_assert!(self.len.get() < self.cap.get());

        let len = self.len.get();
        let ptr = unsafe { self.ptr.get().as_ptr().add(len) };
        unsafe { ptr.write(value) };
        self.len.set(len + 1);

        // SAFETY: Value was just written.
        unsafe { &mut *ptr }
    }

    /// Check if the head is exhausted.
    fn is_exhausted(&self) -> bool {
        self.len.get() == self.cap.get()
    }

    /// Reallocate the head with new capacity.
    /// Array owned by the head is returned.
    ///
    /// Can only be called if the head is exhausted.
    unsafe fn reallocate(&self, new_cap: usize) -> Exhausted<T> {
        debug_assert_eq!(
            self.len.get(),
            self.cap.get(),
            "Only exhausted `Head` can be reallocated"
        );

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

    /// Reset the head, dropping all stored values.
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

    /// Drain all stored values.
    /// Head is reset and may be reused when returned iterator is dropped.
    fn drain(&mut self) -> HeadDrain<'_, T> {
        let len = self.len.get();

        // Reset length to zero before draining elements.
        // If draining panics, we'll leak elements instead of draining them twice.
        self.len.set(0);

        // Iterator reads values from the array.
        // So it can't be replaced by a slice.
        HeadDrain {
            ptr: self.ptr.get(),
            idx: 0,
            len,
            marker: PhantomData,
        }
    }
}

impl<T> Arena<T> {
    /// Minimum non-zero capacity for the head depends on the element size.
    const MIN_NON_ZERO_CAP: usize = if mem::size_of::<T>() == 1 {
        8
    } else if mem::size_of::<T>() <= 1024 {
        4
    } else {
        1
    };

    /// Creates new empty arena.
    pub const fn new() -> Self {
        Arena {
            head: Head::empty(),
            tail: RefCell::new(Vec::new()),
            count: Cell::new(0),
        }
    }

    /// Put a value into the arena.
    /// Returns mutable reference to the value.
    ///
    /// Value will be dropped when arena is reset.
    /// Or returned if arena is drained.
    pub fn put(&self, value: T) -> &mut T {
        if self.head.is_exhausted() {
            // Find new capacity for the head.
            let new_cap = self
                .count
                .get()
                .next_power_of_two()
                .min(Self::MIN_NON_ZERO_CAP);

            alloc_guard(new_cap);

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

    /// Drains all elements from the Arena.
    /// Returned iterator drops all remaining elements when dropped.
    /// Arena is left empty with last allocated buffer reused.
    pub fn drain(&mut self) -> impl Iterator<Item = T> + '_ {
        self.count.set(0);

        let tail = self.tail.get_mut();

        tail.drain(..)
            .flat_map(|e| e.drain())
            .chain(self.head.drain())
    }
}

struct ExhaustedDrain<T> {
    ptr: NonNull<T>,
    idx: usize,
    len: usize,
}

impl<T> Drop for ExhaustedDrain<T> {
    fn drop(&mut self) {
        let remaining = self.len - self.idx;
        unsafe {
            let slice = std::ptr::slice_from_raw_parts_mut(self.ptr.as_ptr(), remaining);
            std::ptr::drop_in_place::<[T]>(slice);
        }

        if self.len != 0 {
            let layout = Layout::array::<T>(self.len).unwrap();
            unsafe {
                std::alloc::dealloc(self.ptr.as_ptr() as *mut u8, layout);
            }
        }
    }
}

impl<T> Iterator for ExhaustedDrain<T> {
    type Item = T;

    #[cfg_attr(inline_more, inline)]
    fn next(&mut self) -> Option<Self::Item> {
        if self.idx == self.len {
            return None;
        }

        let ptr = unsafe { self.ptr.as_ptr().add(self.idx) };
        self.idx += 1;

        // SAFETY: Index is in bounds.
        Some(unsafe { ptr.read() })
    }

    #[cfg_attr(inline_more, inline)]
    fn nth(&mut self, n: usize) -> Option<Self::Item> {
        // Limit number of elements to skip.
        let n = n.min(self.len - self.idx);

        // Drop skipped elements.
        unsafe {
            let ptr = self.ptr.as_ptr().add(self.idx);

            // Move index.
            self.idx += n;

            let slice = std::ptr::slice_from_raw_parts_mut(ptr, n);
            std::ptr::drop_in_place::<[T]>(slice);
        }

        // Return next element.
        self.next()
    }

    #[cfg_attr(inline_more, inline)]
    fn count(self) -> usize {
        let n = self.len - self.idx;

        // Drop remaining elements.
        unsafe {
            let ptr = self.ptr.as_ptr().add(self.idx);
            let slice = std::ptr::slice_from_raw_parts_mut(ptr, n);
            std::ptr::drop_in_place::<[T]>(slice);
        }

        n
    }
}

struct HeadDrain<'a, T: 'a> {
    ptr: NonNull<T>,
    idx: usize,
    len: usize,
    marker: PhantomData<&'a T>,
}

impl<'a, T: 'a> Drop for HeadDrain<'a, T> {
    fn drop(&mut self) {
        let remaining = self.len - self.idx;
        unsafe {
            let slice = std::ptr::slice_from_raw_parts_mut(self.ptr.as_ptr(), remaining);
            std::ptr::drop_in_place::<[T]>(slice);
        }
    }
}

impl<'a, T: 'a> Iterator for HeadDrain<'a, T> {
    type Item = T;

    #[cfg_attr(inline_more, inline)]
    fn next(&mut self) -> Option<Self::Item> {
        if self.idx == self.len {
            return None;
        }

        let ptr = unsafe { self.ptr.as_ptr().add(self.idx) };
        self.idx += 1;

        // SAFETY: Index is in bounds.
        Some(unsafe { ptr.read() })
    }

    #[cfg_attr(inline_more, inline)]
    fn nth(&mut self, n: usize) -> Option<Self::Item> {
        // Limit number of elements to skip.
        let n = n.min(self.len - self.idx);

        // Drop skipped elements.
        unsafe {
            let ptr = self.ptr.as_ptr().add(self.idx);

            // Move index.
            self.idx += n;

            let slice = std::ptr::slice_from_raw_parts_mut(ptr, n);
            std::ptr::drop_in_place::<[T]>(slice);
        }

        // Return next element.
        self.next()
    }

    #[cfg_attr(inline_more, inline)]
    fn count(self) -> usize {
        let n = self.len - self.idx;

        // Drop remaining elements.
        unsafe {
            let ptr = self.ptr.as_ptr().add(self.idx);
            let slice = std::ptr::slice_from_raw_parts_mut(ptr, n);
            std::ptr::drop_in_place::<[T]>(slice);
        }

        n
    }
}
