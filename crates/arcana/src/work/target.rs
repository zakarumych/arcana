use std::{
    any::TypeId,
    hash::Hash,
    mem::{transmute, ManuallyDrop},
};

use hashbrown::HashMap;

use crate::{
    hash::{no_hash_map, NoHashMap},
    make_id,
    stid::HasStid,
    type_id,
};

make_id! {
    /// ID of the render target.
    pub TargetId;
}

pub trait Target: HasStid + 'static {
    type Info: Eq + 'static;

    fn allocate(device: &mev::Device, name: &str, info: &Self::Info) -> Self
    where
        Self: Sized;

    /// Merge two target info instances.
    /// Returns true if the info merged successfully.
    /// If the info is not mergeable, it must return false.
    fn merge_info(_info: &mut Self::Info, _other: &Self::Info) -> bool
    where
        Self: Sized,
    {
        false
    }
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
    pub fn new<V: 'static>() -> Self {
        unsafe {
            AnyHashMap {
                map: transmute(HashMap::<K, V>::new()),
                drop: |map: HashMap<K, u8>| {
                    transmute::<HashMap<K, u8>, HashMap<K, V>>(map);
                },
            }
        }
    }

    pub unsafe fn downcast_ref<V: 'static>(&self) -> &HashMap<K, V> {
        unsafe { transmute(&self.map) }
    }

    pub unsafe fn downcast_mut<V: 'static>(&mut self) -> &mut HashMap<K, V> {
        unsafe { transmute(&mut self.map) }
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

    pub fn plan_create<'a>(&'a mut self, name: &str, device: &mev::Device) -> Option<&'a T::Info> {
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

    pub fn plan_update(&mut self) -> Option<&T::Info> {
        if let Some((_, info)) = &self.external {
            return Some(info);
        }

        self.new_info.as_ref()
    }

    pub fn plan_read(&mut self, info: T::Info) {
        match self.new_info {
            None => {
                self.new_info = Some(info);
            }
            Some(ref mut new_info) => {
                T::merge_info(new_info, &info);
            }
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
    types: NoHashMap<TypeId, AnyHashMap<TargetId>>,
}

impl TargetHub {
    pub const fn new() -> Self {
        TargetHub {
            types: no_hash_map(),
        }
    }

    pub fn data<T: Target>(&self, id: TargetId) -> Option<&TargetData<T>> {
        let any_hub = self.types.get(&type_id::<T>())?;
        let typed_hub = unsafe { any_hub.downcast_ref::<TargetData<T>>() };
        typed_hub.get(&id)
    }

    pub fn data_mut<T: Target>(&mut self, id: TargetId) -> Option<&mut TargetData<T>> {
        let any_hub = self.types.get_mut(&type_id::<T>())?;
        let typed_hub = unsafe { any_hub.downcast_mut::<TargetData<T>>() };
        typed_hub.get_mut(&id)
    }

    pub fn make_data_mut<T: Target>(&mut self, id: TargetId) -> &mut TargetData<T> {
        let any_hub = self
            .types
            .entry(type_id::<T>())
            .or_insert_with(|| AnyHashMap::<TargetId>::new::<TargetData<T>>());
        let typed_hub = unsafe { any_hub.downcast_mut::<TargetData<T>>() };
        typed_hub.entry(id).or_insert_with(|| TargetData::new())
    }

    pub fn plan_create<T: Target>(
        &mut self,
        id: TargetId,
        name: &str,
        device: &mev::Device,
    ) -> Option<&T::Info> {
        let data = self.data_mut::<T>(id)?;
        data.plan_create(name, device)
    }

    pub fn plan_update<T: Target>(&mut self, id: TargetId) -> Option<&T::Info> {
        let data = self.data_mut::<T>(id)?;
        data.plan_update()
    }

    pub fn plan_read<T: Target>(&mut self, id: TargetId, info: T::Info) {
        let data = self.make_data_mut::<T>(id);
        data.plan_read(info);
    }

    pub fn get<T: Target>(&self, id: TargetId) -> Option<&T> {
        self.data::<T>(id)?.instance()
    }

    pub fn external<T: Target>(&mut self, id: TargetId, instance: T, info: T::Info) {
        let data: &mut TargetData<T> = self.make_data_mut(id);
        data.external(instance, info);
    }

    pub fn clear_external<T: Target>(&mut self, id: TargetId) {
        let Some(data) = self.data_mut::<T>(id) else {
            return;
        };
        data.clear_external();
    }

    pub fn clear(&mut self) {
        self.types.clear();
    }
}
