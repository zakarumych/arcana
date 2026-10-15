use std::{
    any::TypeId,
    hash::Hash,
    mem::{ManuallyDrop, transmute},
};

use arcana_hash::{NoHashMap, no_hash_map};
use arcana_id::{HasStid, make_id};
use hashbrown::HashMap;

make_id! {
    /// ID of the render target.
    pub TargetId;
}

/// A type that can be used to define requirement types.
///
/// It uses two variants allowing flexible requirement declarations render jobs.
/// With unambiguous merging rules.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum Req<T> {
    /// Strict requirement.
    /// Contains exact value that will not merge with strict variants unless value is equal.
    Strict(T),

    /// Suggestion will be overwritten by strict requirement.
    Suggest(T),
}

impl<T> Req<T>
where
    T: Eq + Copy,
{
    pub fn merge(&mut self, other: &Self) -> Result<(), MergeError> {
        match (self, other) {
            (Req::Strict(this), Req::Strict(other)) => (*this == *other).ok_or(MergeError),
            (this, Req::Strict(other)) => {
                *this = Req::Strict(*other);
                Ok(())
            }
            _ => Ok(()),
        }
    }

    pub fn get(self) -> T {
        let (Req::Strict(value) | Req::Suggest(value)) = self;
        value
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct MergeError;

pub trait Target: HasStid + 'static {
    /// Requirements from this target consumers.
    /// Merged together using [`Target::merge_req`].
    ///
    /// Used to guide creator's [`Target::Req`]
    type Req: core::fmt::Debug + Eq + 'static;

    fn allocate(device: &mev::Device, name: &str, req: &Self::Req) -> Self
    where
        Self: Sized;

    /// Merge two target req instances.
    /// Returns true if the req merged successfully.
    /// If the req is not mergeable, it must return false.
    fn merge_req(req: &mut Self::Req, other: &Self::Req) -> Result<(), MergeError>
    where
        Self: Sized,
    {
        if *req != *other {
            Err(MergeError)
        } else {
            Ok(())
        }
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
        // # Safety:
        // This relies on layout of HasMap structure to be independent of value type.
        // Operations will be done on references transmuted back to correct type.
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

pub(crate) struct TargetData<T: Target> {
    /// Externally owned target resource.
    external: Option<(T, T::Req)>,

    /// Allocated target instance.
    target: Option<(T, T::Req)>,

    /// Target req.
    new_req: Option<T::Req>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum PlanError {
    MergeError,
    ExternalOverride,
}

impl From<MergeError> for PlanError {
    fn from(_: MergeError) -> Self {
        PlanError::MergeError
    }
}

impl<T> TargetData<T>
where
    T: Target,
{
    fn new() -> Self {
        TargetData {
            external: None,
            target: None,
            new_req: None,
        }
    }

    pub fn external(&mut self, instance: T, desc: T::Req) {
        self.external = Some((instance, desc));
        self.target = None;
        self.new_req = None;
    }

    pub fn clear_external(&mut self) {
        self.external = None;
    }

    pub fn plan_create<'a>(
        &'a mut self,
        name: &str,
        device: &mev::Device,
        own_req: Option<T::Req>,
    ) -> Result<Option<&'a T::Req>, PlanError> {
        if let Some((_, req)) = &self.external {
            if let Some(mut own_req) = own_req {
                T::merge_req(&mut own_req, req)?;
                if own_req != *req {
                    return Err(PlanError::ExternalOverride);
                }
            }
            return Ok(Some(req));
        }

        let Some(mut new_req) = self.new_req.take() else {
            return Ok(None);
        };
        if let Some(own_req) = own_req {
            T::merge_req(&mut new_req, &own_req)?;
        }

        if let Some((_, req)) = &self.target {
            if *req != new_req {
                self.target = None;
            }
        }

        if self.target.is_none() {
            let instance = T::allocate(device, name, &new_req);
            self.target = Some((instance, new_req));
        }

        match &self.target {
            Some((_, req)) => Ok(Some(req)),
            None => unreachable!(),
        }
    }

    pub fn plan_update(&mut self, own_req: Option<T::Req>) -> Result<Option<&T::Req>, PlanError> {
        if let Some((_, req)) = &self.external {
            if let Some(mut own_req) = own_req {
                T::merge_req(&mut own_req, req)?;
                if own_req != *req {
                    return Err(PlanError::ExternalOverride);
                }
            }
            return Ok(Some(req));
        }

        let Some(new_req) = self.new_req.as_mut() else {
            return Ok(None);
        };

        if let Some(own_req) = own_req {
            T::merge_req(new_req, &own_req)?;
        }

        Ok(Some(new_req))
    }

    pub fn plan_read(&mut self, info: T::Req) -> Result<(), PlanError> {
        match self.new_req {
            None => {
                self.new_req = Some(info);
                Ok(())
            }
            Some(ref mut new_req) => {
                T::merge_req(new_req, &info)?;
                Ok(())
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

pub(crate) struct TargetHub {
    types: NoHashMap<TypeId, AnyHashMap<TargetId>>,
}

impl TargetHub {
    pub const fn new() -> Self {
        TargetHub {
            types: no_hash_map(),
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

    pub fn make_data_mut<T: Target>(&mut self, id: TargetId) -> &mut TargetData<T> {
        let any_hub = self
            .types
            .entry(TypeId::of::<T>())
            .or_insert_with(|| AnyHashMap::<TargetId>::new::<TargetData<T>>());
        let typed_hub = unsafe { any_hub.downcast_mut::<TargetData<T>>() };
        typed_hub.entry(id).or_insert_with(|| TargetData::new())
    }

    pub fn plan_create<T: Target>(
        &mut self,
        id: TargetId,
        name: &str,
        device: &mev::Device,
        own_req: Option<T::Req>,
    ) -> Result<Option<&T::Req>, PlanError> {
        let Some(data) = self.data_mut::<T>(id) else {
            return Ok(None);
        };
        data.plan_create(name, device, own_req)
    }

    pub fn plan_update<T: Target>(
        &mut self,
        id: TargetId,
        own_req: Option<T::Req>,
    ) -> Result<Option<&T::Req>, PlanError> {
        let Some(data) = self.data_mut::<T>(id) else {
            return Ok(None);
        };
        data.plan_update(own_req)
    }

    pub fn plan_read<T: Target>(&mut self, id: TargetId, info: T::Req) -> Result<(), PlanError> {
        let data = self.make_data_mut::<T>(id);
        data.plan_read(info)
    }

    pub fn get<T: Target>(&self, id: TargetId) -> Option<&T> {
        self.data::<T>(id)?.instance()
    }

    pub fn external<T: Target>(&mut self, id: TargetId, instance: T, req: T::Req) {
        let data: &mut TargetData<T> = self.make_data_mut(id);
        data.external(instance, req);
    }

    pub fn clear_external<T: Target>(&mut self, id: TargetId) {
        let Some(data) = self.data_mut::<T>(id) else {
            return;
        };
        data.clear_external();
    }

    // pub fn clear(&mut self) {
    //     self.types.clear();
    // }
}
