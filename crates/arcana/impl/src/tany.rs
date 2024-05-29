use std::{
    any::{Any, TypeId},
    marker::PhantomData,
    mem::{align_of, size_of, ManuallyDrop, MaybeUninit},
    ptr,
};

use crate::type_id;

const TANY_STORAGE_SIZE: usize = size_of::<[usize; 3]>();
const TANY_STORAGE_ALIGN: usize = 16;

#[repr(C, align(16))]
#[derive(Clone, Copy)]
struct InlineStorage {
    pub storage: MaybeUninit<[u8; TANY_STORAGE_SIZE]>,
}

impl InlineStorage {
    fn new() -> Self {
        InlineStorage {
            storage: MaybeUninit::uninit(),
        }
    }

    fn as_ref<T>(&self) -> &MaybeUninit<T> {
        assert!(size_of::<T>() <= TANY_STORAGE_SIZE);
        assert!(align_of::<T>() <= TANY_STORAGE_ALIGN);

        unsafe { &*self.storage.as_ptr().cast() }
    }

    fn as_mut<T>(&mut self) -> &mut MaybeUninit<T> {
        assert!(size_of::<T>() <= TANY_STORAGE_SIZE);
        assert!(align_of::<T>() <= TANY_STORAGE_ALIGN);

        unsafe { &mut *self.storage.as_mut_ptr().cast() }
    }
}

unsafe fn type_id_boxed_any(storage: &InlineStorage) -> TypeId {
    unsafe { storage.as_ref::<Box<dyn Any>>().assume_init_ref().type_id() }
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

struct VTable {
    type_id: unsafe fn(&InlineStorage) -> TypeId,
    drop: unsafe fn(&mut InlineStorage),
    drop_empty: unsafe fn(&mut InlineStorage),
    as_ptr: unsafe fn(&InlineStorage) -> *const u8,
    as_mut: unsafe fn(&mut InlineStorage) -> *mut u8,
}

/// `dyn Any` with inlined storage for small types.
/// Large types are boxed.
pub struct LTAny {
    vtable: &'static VTable,
    storage: InlineStorage,
    unsend: PhantomData<*mut u8>,
}

impl Drop for LTAny {
    fn drop(&mut self) {
        unsafe {
            (self.vtable.drop)(&mut self.storage);
        }
    }
}

impl LTAny {
    pub fn new<T>(value: T) -> Self
    where
        T: 'static,
    {
        let size_fits = size_of::<T>() <= TANY_STORAGE_SIZE;
        let align_fits = align_of::<T>() <= TANY_STORAGE_ALIGN;

        let mut storage = InlineStorage::new();

        if size_fits && align_fits {
            storage.as_mut().write(value);

            let vtable = &VTable {
                type_id: |_| TypeId::of::<T>(),
                drop: drop_inlined::<T>,
                drop_empty: drop_empty_inlined::<T>,
                as_ptr: as_ptr_inlined::<T>,
                as_mut: as_mut_inlined::<T>,
            };

            LTAny {
                vtable,
                storage,
                unsend: PhantomData,
            }
        } else {
            let boxed = Box::new(value);

            storage.as_mut().write(boxed);

            let vtable = &VTable {
                type_id: |_| TypeId::of::<T>(),
                drop: drop_boxed::<T>,
                drop_empty: drop_empty_boxed::<T>,
                as_ptr: as_ptr_boxed::<T>,
                as_mut: as_mut_boxed::<T>,
            };

            LTAny {
                vtable,
                storage,
                unsend: PhantomData,
            }
        }
    }

    pub fn from_boxed(boxed: Box<dyn Any>) -> Self {
        const {
            assert!(size_of::<Box<dyn Any>>() <= TANY_STORAGE_SIZE);
            assert!(align_of::<Box<dyn Any>>() <= TANY_STORAGE_ALIGN);
        }

        let mut storage = InlineStorage::new();

        storage.as_mut().write(boxed);

        let vtable = &VTable {
            type_id: type_id_boxed_any,
            drop: drop_boxed::<dyn Any>,
            drop_empty: drop_empty_boxed::<dyn Any>,
            as_ptr: as_ptr_boxed::<dyn Any>,
            as_mut: as_mut_boxed::<dyn Any>,
        };

        LTAny {
            vtable,
            storage,
            unsend: PhantomData,
        }
    }

    pub fn type_id(&self) -> TypeId {
        unsafe { (self.vtable.type_id)(&self.storage) }
    }

    pub fn is<T>(&self) -> bool
    where
        T: 'static,
    {
        self.type_id() == type_id::<T>()
    }

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

    pub fn downcast<T>(self) -> Result<T, LTAny>
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
}

/// `dyn Any` with inlined storage for small types.
/// Large types are boxed.
pub struct TAny {
    vtable: &'static VTable,
    storage: InlineStorage,
}

impl Drop for TAny {
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

            let vtable = &VTable {
                type_id: |_| TypeId::of::<T>(),
                drop: drop_inlined::<T>,
                drop_empty: drop_empty_inlined::<T>,
                as_ptr: as_ptr_inlined::<T>,
                as_mut: as_mut_inlined::<T>,
            };

            TAny { vtable, storage }
        } else {
            let boxed = Box::new(value);

            storage.as_mut().write(boxed);

            let vtable = &VTable {
                type_id: |_| TypeId::of::<T>(),
                drop: drop_boxed::<T>,
                drop_empty: drop_empty_boxed::<T>,
                as_ptr: as_ptr_boxed::<T>,
                as_mut: as_mut_boxed::<T>,
            };

            TAny { vtable, storage }
        }
    }

    pub fn from_boxed(boxed: Box<dyn Any + Send + Sync>) -> Self {
        const {
            assert!(size_of::<Box<dyn Any + Send + Sync>>() <= TANY_STORAGE_SIZE);
            assert!(align_of::<Box<dyn Any + Send + Sync>>() <= TANY_STORAGE_ALIGN);
        }

        let mut storage = InlineStorage::new();

        storage.as_mut().write(boxed);

        let vtable = &VTable {
            type_id: type_id_boxed_any,
            drop: drop_boxed::<dyn Any>,
            drop_empty: drop_empty_boxed::<dyn Any>,
            as_ptr: as_ptr_boxed::<dyn Any>,
            as_mut: as_mut_boxed::<dyn Any>,
        };

        TAny { vtable, storage }
    }

    pub fn type_id(&self) -> TypeId {
        unsafe { (self.vtable.type_id)(&self.storage) }
    }

    pub fn is<T>(&self) -> bool
    where
        T: 'static,
    {
        self.type_id() == type_id::<T>()
    }

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
}

fn test_send<T: Send>() {}

fn is_send() {
    test_send::<TAny>();
}
