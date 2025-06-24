use std::{
    any::{Any, TypeId},
    mem::ManuallyDrop,
};

use crate::storage::{InlineStorage, VTable, TANY_STORAGE_ALIGN, TANY_STORAGE_SIZE};

/// `dyn Any` with fixed-size inlined storage.
/// Types that fit in the storage are stored without allocation.
/// Types that are larger than 24 bytes or have alignment greater than 16 bytes are boxed.
/// Requires `Send` or `Sync`.
/// For thread-local version see [`LTAny`].
pub struct TAny {
    vtable: &'static VTable,
    storage: InlineStorage,
}

impl Drop for TAny {
    #[inline]
    fn drop(&mut self) {
        unsafe {
            (self.vtable.drop)(&mut self.storage);
        }
    }
}

impl TAny {
    pub fn new<T>(value: T) -> Self
    where
        T: Send + Sync + 'static,
    {
        let size_fits = size_of::<T>() <= TANY_STORAGE_SIZE;
        let align_fits = align_of::<T>() <= TANY_STORAGE_ALIGN;

        let mut storage = InlineStorage::new();

        if size_fits && align_fits {
            storage.as_mut().write(value);

            TAny {
                vtable: VTable::inlined::<T>(),
                storage,
            }
        } else {
            let boxed = Box::new(value);

            storage.as_mut().write(boxed);

            TAny {
                vtable: VTable::boxed::<T>(),
                storage,
            }
        }
    }

    #[inline]
    pub fn from_boxed(boxed: Box<dyn Any + Send + Sync>) -> Self {
        const {
            assert!(size_of::<Box<dyn Any + Send + Sync>>() <= TANY_STORAGE_SIZE);
            assert!(align_of::<Box<dyn Any + Send + Sync>>() <= TANY_STORAGE_ALIGN);
        }

        let mut storage = InlineStorage::new();

        storage.as_mut().write(boxed);

        TAny {
            vtable: VTable::for_any(),
            storage,
        }
    }

    #[inline]
    pub fn type_id(&self) -> TypeId {
        unsafe { (self.vtable.type_id)(&self.storage) }
    }

    #[inline]
    pub fn is<T>(&self) -> bool
    where
        T: 'static,
    {
        self.type_id() == TypeId::of::<T>()
    }

    #[inline]
    pub fn downcast_ref<T>(&self) -> Option<&T>
    where
        T: 'static,
    {
        if self.is::<T>() {
            let ptr = unsafe { (self.vtable.as_ptr)(&self.storage) };
            Some(unsafe { &*ptr.cast() })
        } else {
            None
        }
    }

    #[inline]
    pub fn downcast_mut<T>(&mut self) -> Option<&mut T>
    where
        T: 'static,
    {
        if self.is::<T>() {
            let ptr = unsafe { (self.vtable.as_mut)(&mut self.storage) };
            Some(unsafe { &mut *ptr.cast() })
        } else {
            None
        }
    }

    #[inline]
    pub fn downcast<T>(self) -> Result<T, TAny>
    where
        T: 'static,
    {
        if self.is::<T>() {
            let mut me = ManuallyDrop::new(self);
            let ptr = unsafe { (me.vtable.as_ptr)(&me.storage) };
            let value = unsafe { ptr.cast::<T>().read() };
            unsafe {
                (me.vtable.drop_empty)(&mut me.storage);
            }
            Ok(value)
        } else {
            Err(self)
        }
    }

    #[inline]
    pub fn set<T>(&mut self, value: T)
    where
        T: Send + Sync + 'static,
    {
        match self.downcast_mut() {
            Some(slot) => *slot = value,
            None => *self = TAny::new(value),
        }
    }

    #[inline]
    pub fn clone_from<T>(&mut self, value: &T)
    where
        T: Clone + Send + Sync + 'static,
    {
        match self.downcast_mut::<T>() {
            Some(slot) => slot.clone_from(value),
            None => *self = TAny::new(value.clone()),
        }
    }

    #[inline]
    pub fn from_borrowed<T>(&mut self, value: &(impl ToOwned<Owned = T> + ?Sized))
    where
        T: Send + Sync + 'static,
    {
        match self.downcast_mut::<T>() {
            Some(slot) => value.clone_into(slot),
            None => *self = TAny::new(value.to_owned()),
        }
    }
}
