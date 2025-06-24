use std::{
    any::{Any, TypeId},
    mem::{ManuallyDrop, MaybeUninit},
    ptr,
};

pub(crate) const TANY_STORAGE_SIZE: usize = size_of::<[usize; 3]>();
pub(crate) const TANY_STORAGE_ALIGN: usize = 16;

const _: () = const {
    assert!(TANY_STORAGE_SIZE >= size_of::<Box<dyn Any>>());
    assert!(TANY_STORAGE_ALIGN >= align_of::<Box<dyn Any>>());
};

#[repr(C, align(16))]
#[derive(Clone, Copy)]
pub(crate) struct InlineStorage {
    storage: MaybeUninit<[u8; TANY_STORAGE_SIZE]>,
}

impl InlineStorage {
    pub fn new() -> Self {
        InlineStorage {
            storage: MaybeUninit::uninit(),
        }
    }

    pub fn as_ref<T>(&self) -> &MaybeUninit<T> {
        assert!(size_of::<T>() <= TANY_STORAGE_SIZE);
        assert!(align_of::<T>() <= TANY_STORAGE_ALIGN);

        unsafe { &*self.storage.as_ptr().cast() }
    }

    pub fn as_mut<T>(&mut self) -> &mut MaybeUninit<T> {
        assert!(size_of::<T>() <= TANY_STORAGE_SIZE);
        assert!(align_of::<T>() <= TANY_STORAGE_ALIGN);

        unsafe { &mut *self.storage.as_mut_ptr().cast() }
    }
}

unsafe fn type_id_boxed_any(storage: &InlineStorage) -> TypeId {
    unsafe { storage.as_ref::<Box<dyn Any>>().assume_init_ref().type_id() }
}

fn type_id_static<T: ?Sized + 'static>(_: &InlineStorage) -> TypeId {
    TypeId::of::<()>()
}

unsafe fn drop_inlined<T>(storage: &mut InlineStorage) {
    assert!(size_of::<T>() <= TANY_STORAGE_SIZE);
    assert!(align_of::<T>() <= TANY_STORAGE_ALIGN);

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
    assert!(size_of::<T>() <= TANY_STORAGE_SIZE);
    assert!(align_of::<T>() <= TANY_STORAGE_ALIGN);
}

unsafe fn drop_empty_boxed<T: ?Sized>(storage: &mut InlineStorage) {
    unsafe {
        storage.as_mut::<Box<ManuallyDrop<T>>>().assume_init_drop();
    }
}

unsafe fn as_ptr_inlined<T>(storage: &InlineStorage) -> *const u8 {
    assert!(size_of::<T>() <= TANY_STORAGE_SIZE);
    assert!(align_of::<T>() <= TANY_STORAGE_ALIGN);

    let r: &T = unsafe { storage.as_ref::<T>().assume_init_ref() };
    ptr::from_ref(r).cast()
}

unsafe fn as_ptr_boxed<T: ?Sized>(storage: &InlineStorage) -> *const u8 {
    let r: &T = &**unsafe { storage.as_ref::<Box<T>>().assume_init_ref() };
    ptr::from_ref(r).cast()
}

unsafe fn as_mut_inlined<T>(storage: &mut InlineStorage) -> *mut u8 {
    assert!(size_of::<T>() <= TANY_STORAGE_SIZE);
    assert!(align_of::<T>() <= TANY_STORAGE_ALIGN);

    let r: &mut T = unsafe { storage.as_mut::<T>().assume_init_mut() };
    ptr::from_mut(r).cast()
}

unsafe fn as_mut_boxed<T: ?Sized>(storage: &mut InlineStorage) -> *mut u8 {
    let r: &mut T = &mut **unsafe { storage.as_mut::<Box<T>>().assume_init_mut() };
    ptr::from_mut(r).cast()
}

pub(crate) struct VTable {
    pub type_id: unsafe fn(&InlineStorage) -> TypeId,
    pub drop: unsafe fn(&mut InlineStorage),
    pub drop_empty: unsafe fn(&mut InlineStorage),
    pub as_ptr: unsafe fn(&InlineStorage) -> *const u8,
    pub as_mut: unsafe fn(&mut InlineStorage) -> *mut u8,
}

impl VTable {
    pub fn inlined<T: 'static>() -> &'static Self {
        debug_assert!(size_of::<T>() <= TANY_STORAGE_SIZE);
        debug_assert!(align_of::<T>() <= TANY_STORAGE_ALIGN);

        &VTable {
            type_id: type_id_static::<T>,
            drop: drop_inlined::<T>,
            drop_empty: drop_empty_inlined::<T>,
            as_ptr: as_ptr_inlined::<T>,
            as_mut: as_mut_inlined::<T>,
        }
    }

    pub fn boxed<T: ?Sized + 'static>() -> &'static Self {
        debug_assert!(size_of::<Box<T>>() <= TANY_STORAGE_SIZE);
        debug_assert!(align_of::<Box<T>>() <= TANY_STORAGE_ALIGN);

        &VTable {
            type_id: type_id_static::<T>,
            drop: drop_boxed::<T>,
            drop_empty: drop_empty_boxed::<T>,
            as_ptr: as_ptr_boxed::<T>,
            as_mut: as_mut_boxed::<T>,
        }
    }

    pub fn for_any() -> &'static Self {
        &VTable {
            type_id: type_id_boxed_any,
            drop: drop_boxed::<dyn Any>,
            drop_empty: drop_empty_boxed::<dyn Any>,
            as_ptr: as_ptr_boxed::<dyn Any>,
            as_mut: as_mut_boxed::<dyn Any>,
        }
    }
}
