use std::{
    any::{Any, TypeId},
    fmt::Debug,
    hash::{Hash, Hasher},
    marker::PhantomData,
    mem::{transmute, ManuallyDrop},
    num::NonZeroU64,
    sync::Arc,
};

use hashbrown::HashMap;

use crate::id::{Id, IdGen};

use super::job::JobId;

pub type OutputId<T = ()> = Id<fn() -> T>;
pub type InputId<T = ()> = Id<fn(T)>;
pub type UpdateId<T = ()> = Id<fn(T) -> T>;

pub type TargetId = Id<dyn Target<Info = dyn Any>>;

pub trait Target: 'static {
    type Info: Eq + 'static;

    fn name() -> &'static str
    where
        Self: Sized;

    fn allocate(device: &mev::Device, name: &str, info: &Self::Info) -> Self
    where
        Self: Sized;
}

pub trait TargetInfoMerge: Target {
    fn merge_info(info: &mut Self::Info, other: &Self::Info);
}

struct AnyHashMap<K> {
    map: ManuallyDrop<HashMap<K, u8>>,
    drop: fn(HashMap<K, u8>),
}

impl<K> Drop for AnyHashMap<K> {
    fn drop(&mut self) {
        unsafe {
            (self.drop)(ManuallyDrop::take(&mut self.map));
        }
    }
}

impl<K> AnyHashMap<K> {
    pub fn new<V>() -> Self {
        unsafe {
            AnyHashMap {
                map: transmute(HashMap::<K, V>::new()),
                drop: |map: HashMap<K, u8>| {
                    transmute::<HashMap<K, u8>, HashMap<K, V>>(map);
                },
            }
        }
    }

    pub unsafe fn downcast_ref<V>(&self) -> &HashMap<K, V> {
        transmute(&self.map)
    }

    pub unsafe fn downcast_mut<V>(&mut self) -> &mut HashMap<K, V> {
        transmute(&mut self.map)
    }
}

pub struct TargetData<T: Target> {
    external: Option<(T, T::Info)>,

    /// Allocated target instance.
    target: Option<(T, T::Info)>,

    /// Target info.
    new_info: Option<T::Info>,
}

impl<T> TargetData<T>
where
    T: Target,
{
    fn new() -> Self {
        TargetData {
            external: None,
            target: None,
            new_info: None,
        }
    }

    pub fn external(&mut self, instance: T, info: T::Info) {
        self.external = Some((instance, info));
        self.target = None;
        self.new_info = None;
    }

    pub fn clear_external(&mut self) {
        self.external = None;
    }

    pub fn output<'a>(&'a mut self, name: &str, device: &mev::Device) -> Option<&'a T::Info> {
        if let Some((_, info)) = &self.external {
            return Some(info);
        }

        let new_info = self.new_info.take()?;

        if let Some((_, info)) = &self.target {
            if *info != new_info {
                self.target = None;
            }
        }

        if self.target.is_none() {
            let instance = T::allocate(device, name, &new_info);
            self.target = Some((instance, new_info));
        }

        match &self.target {
            Some((_, info)) => Some(info),
            None => unreachable!(),
        }
    }

    pub fn input(
        &mut self,
        info: T::Info,
        merge_info: &HashMap<TypeId, fn(&mut dyn Any, &dyn Any)>,
    ) {
        match self.new_info {
            None => {
                self.new_info = Some(info);
            }
            Some(ref mut new_info) => match merge_info.get(&TypeId::of::<T>()) {
                None => panic!("Target with non-mergeable info is shared"),
                Some(merge_info) => {
                    merge_info(new_info, &info);
                }
            },
        }
    }

    pub fn instance(&self) -> Option<&T> {
        if let Some((instance, _)) = &self.external {
            return Some(instance);
        }

        if let Some((instance, _)) = &self.target {
            return Some(instance);
        }

        None
    }
}

pub struct TargetHub {
    types: HashMap<TypeId, AnyHashMap<TargetId>>,
}

impl TargetHub {
    pub fn new() -> Self {
        TargetHub {
            types: HashMap::new(),
        }
    }

    pub fn data<T: Target>(&self, id: TargetId) -> Option<&TargetData<T>> {
        let any_hub = self.types.get(&TypeId::of::<T>())?;
        let typed_hub = unsafe { any_hub.downcast_ref::<TargetData<T>>() };
        typed_hub.get(&id)
    }

    pub fn data_mut<T: Target>(&mut self, id: TargetId) -> Option<&mut TargetData<T>> {
        let any_hub = self.types.get_mut(&TypeId::of::<T>())?;
        let typed_hub = unsafe { any_hub.downcast_mut::<TargetData<T>>() };
        typed_hub.get_mut(&id)
    }

    pub fn plan_input<T: Target>(
        &mut self,
        id: TargetId,
        info: T::Info,
        merge_info: &HashMap<TypeId, fn(&mut dyn Any, &dyn Any)>,
    ) {
        let Some(data) = self.data_mut::<T>(id) else {
            return;
        };
        data.input(info, merge_info);
    }

    pub fn plan_output<T: Target>(
        &mut self,
        id: TargetId,
        name: &str,
        device: &mev::Device,
    ) -> Option<&T::Info> {
        let data = self.data_mut::<T>(id)?;
        data.output(name, device)
    }

    pub fn get<T: Target>(&self, id: TargetId) -> Option<&T> {
        self.data::<T>(id)?.instance()
    }

    pub fn external<T: Target>(&mut self, id: TargetId, instance: T, info: T::Info) {
        let data = self.data_mut(id).unwrap();
        data.external(instance, info);
    }

    pub fn clear_external<T: Target>(&mut self, id: TargetId) {
        let data = self.data_mut::<T>(id).unwrap();
        data.clear_external();
    }

    pub fn clear(&mut self) {
        self.types.clear();
    }
}
