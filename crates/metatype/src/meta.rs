use std::{
    mem::{ManuallyDrop, MaybeUninit},
    ptr,
};

use crate::{MetaType, MetaTypeInfo, Stid};

pub(crate) const META_STORAGE_SIZE: usize = size_of::<[usize; 3]>();
pub(crate) const META_STORAGE_ALIGN: usize = 16;

#[repr(C, align(16))]
#[derive(Clone, Copy)]
pub(crate) struct InlineStorage {
    storage: MaybeUninit<[u8; META_STORAGE_SIZE]>,
}

impl InlineStorage {
    pub fn new() -> Self {
        InlineStorage {
            storage: MaybeUninit::uninit(),
        }
    }

    pub fn as_ref<T>(&self) -> &MaybeUninit<T> {
        assert!(size_of::<T>() <= META_STORAGE_SIZE);
        assert!(align_of::<T>() <= META_STORAGE_ALIGN);

        unsafe { &*self.storage.as_ptr().cast() }
    }

    pub fn as_mut<T>(&mut self) -> &mut MaybeUninit<T> {
        assert!(size_of::<T>() <= META_STORAGE_SIZE);
        assert!(align_of::<T>() <= META_STORAGE_ALIGN);

        unsafe { &mut *self.storage.as_mut_ptr().cast() }
    }
}

unsafe fn drop_inlined<T>(storage: &mut InlineStorage) {
    assert!(size_of::<T>() <= META_STORAGE_SIZE);
    assert!(align_of::<T>() <= META_STORAGE_ALIGN);

    unsafe {
        storage.as_mut::<T>().assume_init_drop();
    }
}

unsafe fn drop_boxed<T: ?Sized>(storage: &mut InlineStorage) {
    unsafe {
        storage.as_mut::<Box<T>>().assume_init_drop();
    }
}

unsafe fn drop_empty_inlined<T>(_storage: &mut InlineStorage) {
    assert!(size_of::<T>() <= META_STORAGE_SIZE);
    assert!(align_of::<T>() <= META_STORAGE_ALIGN);
}

unsafe fn drop_empty_boxed<T: ?Sized>(storage: &mut InlineStorage) {
    unsafe {
        storage.as_mut::<Box<ManuallyDrop<T>>>().assume_init_drop();
    }
}

unsafe fn as_ptr_inlined<T>(storage: &InlineStorage) -> *const u8 {
    assert!(size_of::<T>() <= META_STORAGE_SIZE);
    assert!(align_of::<T>() <= META_STORAGE_ALIGN);

    let r: &T = unsafe { storage.as_ref::<T>().assume_init_ref() };
    ptr::from_ref(r).cast()
}

unsafe fn as_ptr_boxed<T: ?Sized>(storage: &InlineStorage) -> *const u8 {
    let r: &T = &**unsafe { storage.as_ref::<Box<T>>().assume_init_ref() };
    ptr::from_ref(r).cast()
}

unsafe fn as_mut_inlined<T>(storage: &mut InlineStorage) -> *mut u8 {
    assert!(size_of::<T>() <= META_STORAGE_SIZE);
    assert!(align_of::<T>() <= META_STORAGE_ALIGN);

    let r: &mut T = unsafe { storage.as_mut::<T>().assume_init_mut() };
    ptr::from_mut(r).cast()
}

unsafe fn as_mut_boxed<T: ?Sized>(storage: &mut InlineStorage) -> *mut u8 {
    let r: &mut T = &mut **unsafe { storage.as_mut::<Box<T>>().assume_init_mut() };
    ptr::from_mut(r).cast()
}

pub(crate) struct VTable {
    pub meta: MetaTypeInfo,
    pub drop: unsafe fn(&mut InlineStorage),
    pub drop_empty: unsafe fn(&mut InlineStorage),
    pub as_ptr: unsafe fn(&InlineStorage) -> *const u8,
    pub as_mut: unsafe fn(&mut InlineStorage) -> *mut u8,
}

impl VTable {
    pub fn inlined<T: MetaType>() -> &'static Self {
        debug_assert!(size_of::<T>() <= META_STORAGE_SIZE);
        debug_assert!(align_of::<T>() <= META_STORAGE_ALIGN);

        &VTable {
            meta: T::META,
            drop: drop_inlined::<T>,
            drop_empty: drop_empty_inlined::<T>,
            as_ptr: as_ptr_inlined::<T>,
            as_mut: as_mut_inlined::<T>,
        }
    }

    pub fn boxed<T: MetaType + ?Sized>() -> &'static Self {
        debug_assert!(size_of::<Box<T>>() <= META_STORAGE_SIZE);
        debug_assert!(align_of::<Box<T>>() <= META_STORAGE_ALIGN);

        &VTable {
            meta: T::META,
            drop: drop_boxed::<T>,
            drop_empty: drop_empty_boxed::<T>,
            as_ptr: as_ptr_boxed::<T>,
            as_mut: as_mut_boxed::<T>,
        }
    }
}

/// `dyn Any` with fixed-size inlined storage.
/// Types that fit in the storage are stored without allocation.
/// Types that are larger than 24 bytes or have alignment greater than 16 bytes are boxed.
/// Requires `MetaType`, `Send` and `Sync`.
pub struct Meta {
    vtable: &'static VTable,
    storage: InlineStorage,
}

impl Drop for Meta {
    #[inline]
    fn drop(&mut self) {
        unsafe {
            (self.vtable.drop)(&mut self.storage);
        }
    }
}

impl Meta {
    pub fn new<T>(value: T) -> Self
    where
        T: MetaType + Send + Sync,
    {
        let size_fits = size_of::<T>() <= META_STORAGE_SIZE;
        let align_fits = align_of::<T>() <= META_STORAGE_ALIGN;

        let mut storage = InlineStorage::new();

        if size_fits && align_fits {
            storage.as_mut().write(value);

            Meta {
                vtable: VTable::inlined::<T>(),
                storage,
            }
        } else {
            let boxed = Box::new(value);

            storage.as_mut().write(boxed);

            Meta {
                vtable: VTable::boxed::<T>(),
                storage,
            }
        }
    }

    #[inline]
    pub fn meta_type(&self) -> &MetaTypeInfo {
        &self.vtable.meta
    }

    #[inline]
    pub fn stid(&self) -> Stid {
        self.vtable.meta.stid
    }

    #[inline]
    pub fn name(&self) -> &'static str {
        self.vtable.meta.name
    }

    #[inline]
    pub fn is<T>(&self) -> bool
    where
        T: MetaType,
    {
        self.stid() == Stid::of::<T>()
    }

    #[inline]
    pub fn downcast_ref<T>(&self) -> Option<&T>
    where
        T: MetaType,
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
        T: MetaType,
    {
        if self.is::<T>() {
            let ptr = unsafe { (self.vtable.as_mut)(&mut self.storage) };
            Some(unsafe { &mut *ptr.cast() })
        } else {
            None
        }
    }

    #[inline]
    pub fn downcast<T>(self) -> Result<T, Meta>
    where
        T: MetaType,
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
        T: MetaType + Send + Sync,
    {
        match self.downcast_mut() {
            Some(slot) => *slot = value,
            None => *self = Meta::new(value),
        }
    }

    #[inline]
    pub fn clone_from<T>(&mut self, value: &T)
    where
        T: MetaType + Clone + Send + Sync,
    {
        match self.downcast_mut::<T>() {
            Some(slot) => slot.clone_from(value),
            None => *self = Meta::new(value.clone()),
        }
    }

    #[inline]
    pub fn from_borrowed<T>(&mut self, value: &(impl ToOwned<Owned = T> + ?Sized))
    where
        T: MetaType + Send + Sync,
    {
        match self.downcast_mut::<T>() {
            Some(slot) => value.clone_into(slot),
            None => *self = Meta::new(value.to_owned()),
        }
    }

    #[cfg(feature = "probe")]
    pub fn probe(&mut self) -> Option<&mut dyn egui_probe::EguiProbe> {
        self.vtable.meta.probe.map(|f| (f)(self))
    }
}
