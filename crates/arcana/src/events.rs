//! Event system for the Arcana game engine.
//!

use std::{
    any::{Any, TypeId},
    collections::VecDeque,
};

use edict::{
    entity::{Entity, EntityId},
    world::World,
};

use crate::{
    hash::{no_hash_map, NoHashMap},
    make_uid, type_id, Slot,
};

const MAX_EVENTS: usize = 65536;

make_uid! {
    /// Event ID type.
    /// Any event type must have a unique ID.
    /// Listeners can listen to specific event IDs.
    ///
    /// Events with same ID should be emitted with the same payload type.
    pub EventId;
}

pub struct Event<T: ?Sized = dyn Any> {
    pub id: EventId,
    pub entity: EntityId,
    pub payload: T,
}

impl Event<()> {
    #[inline(always)]
    pub fn new(id: EventId, entity: impl Entity) -> Self {
        Event {
            id,
            entity: entity.id(),
            payload: (),
        }
    }

    pub fn with_payload<T>(self, payload: T) -> Event<T> {
        Event {
            id: self.id,
            entity: self.entity,
            payload,
        }
    }
}

impl Event {
    #[inline(always)]
    pub fn get<T: 'static>(&self) -> &T {
        self.payload.downcast_ref().unwrap()
    }
}

struct AnyEvent {
    id: EventId,
    entity: EntityId,
    payload_id: TypeId,
    payload_idx: u64,
}

pub trait AnyPayload: Any + Send + Sync {
    fn len(&self) -> usize;
    fn get(&self, idx: usize) -> &(dyn Any + Send + Sync);
    fn get_mut(&mut self, idx: usize) -> &mut (dyn Any + Send + Sync);
    fn clone_to(&self, idx: usize, slot: &mut Slot);
}

macro_rules! any_payload {
    () => {
        impl AnyPayload for () {
            fn len(&self) -> usize {
                0
            }

            fn get(&self, _idx: usize) -> &(dyn Any + Send + Sync) {
                unreachable!()
            }

            fn get_mut(&mut self, _idx: usize) -> &mut (dyn Any + Send + Sync) {
                unreachable!()
            }

            fn clone_to(&self, _idx: usize, _slot: &mut Slot) {
                unreachable!()
            }
        }
    };
    (P) => {
        impl<P> AnyPayload for (P,)
        where
            P: Any + Clone + Send + Sync,
        {
            fn len(&self) -> usize {
                1
            }

            fn get(&self, idx: usize) -> &(dyn Any + Send + Sync) {
                match idx {
                    0 => &self.0,
                    _ => unreachable!(),
                }
            }

            fn get_mut(&mut self, idx: usize) -> &mut (dyn Any + Send + Sync) {
                match idx {
                    0 => &mut self.0,
                    _ => unreachable!(),
                }
            }

            fn clone_to(&self, idx: usize, slot: &mut Slot) {
                match idx {
                    0 => slot.set(self.0.clone()),
                    _ => unreachable!(),
                }
            }
        }
    };
    (O P) => {
        impl<O, P> AnyPayload for (O, P)
        where
            O: Any + Clone + Send + Sync,
            P: Any + Clone + Send + Sync,
        {
            fn len(&self) -> usize {
                2
            }

            fn get(&self, idx: usize) -> &(dyn Any + Send + Sync) {
                match idx {
                    0 => &self.0,
                    1 => &self.1,
                    _ => unreachable!(),
                }
            }

            fn get_mut(&mut self, idx: usize) -> &mut (dyn Any + Send + Sync) {
                match idx {
                    0 => &mut self.0,
                    1 => &mut self.1,
                    _ => unreachable!(),
                }
            }

            fn clone_to(&self, idx: usize, slot: &mut Slot) {
                match idx {
                    0 => slot.set(self.0.clone()),
                    1 => slot.set(self.1.clone()),
                    _ => unreachable!(),
                }
            }
        }
    };
    (N O P) => {
        impl<N, O, P> AnyPayload for (N, O, P)
        where
            N: Any + Clone + Send + Sync,
            O: Any + Clone + Send + Sync,
            P: Any + Clone + Send + Sync,
        {
            fn len(&self) -> usize {
                3
            }

            fn get(&self, idx: usize) -> &(dyn Any + Send + Sync) {
                match idx {
                    0 => &self.0,
                    1 => &self.1,
                    2 => &self.2,
                    _ => unreachable!(),
                }
            }

            fn get_mut(&mut self, idx: usize) -> &mut (dyn Any + Send + Sync) {
                match idx {
                    0 => &mut self.0,
                    1 => &mut self.1,
                    2 => &mut self.2,
                    _ => unreachable!(),
                }
            }

            fn clone_to(&self, idx: usize, slot: &mut Slot) {
                match idx {
                    0 => slot.set(self.0.clone()),
                    1 => slot.set(self.1.clone()),
                    2 => slot.set(self.2.clone()),
                    _ => unreachable!(),
                }
            }
        }
    };
    (M N O P) => {
        impl<M, N, O, P> AnyPayload for (M, N, O, P)
        where
            M: Any + Clone + Send + Sync,
            N: Any + Clone + Send + Sync,
            O: Any + Clone + Send + Sync,
            P: Any + Clone + Send + Sync,
        {
            fn len(&self) -> usize {
                4
            }

            fn get(&self, idx: usize) -> &(dyn Any + Send + Sync) {
                match idx {
                    0 => &self.0,
                    1 => &self.1,
                    2 => &self.2,
                    3 => &self.3,
                    _ => unreachable!(),
                }
            }

            fn get_mut(&mut self, idx: usize) -> &mut (dyn Any + Send + Sync) {
                match idx {
                    0 => &mut self.0,
                    1 => &mut self.1,
                    2 => &mut self.2,
                    3 => &mut self.3,
                    _ => unreachable!(),
                }
            }

            fn clone_to(&self, idx: usize, slot: &mut Slot) {
                match idx {
                    0 => slot.set(self.0.clone()),
                    1 => slot.set(self.1.clone()),
                    2 => slot.set(self.2.clone()),
                    3 => slot.set(self.3.clone()),
                    _ => unreachable!(),
                }
            }
        }
    };
    ($($a:ident)+) => {
        impl<$($a),+> AnyPayload for ($($a,)+)
        where
            $($a: Clone + Send + Sync + 'static,)+
        {
            fn len(&self) -> usize {
                let mut count = 0;
                $(
                    let _: $a;
                    count += 1;
                )+
                count
            }

            fn get(&self, idx: usize) -> &(dyn Any + Send + Sync) {
                #![allow(unused, non_snake_case)]

                let ($($a,)+) = self;

                let mut i = idx;
                $(
                    if i == 0 {
                        return $a;
                    }
                    i -= 1;
                )+

                unreachable!()
            }

            fn get_mut(&mut self, idx: usize) -> &mut (dyn Any + Send + Sync) {
                #![allow(unused, non_snake_case)]

                let ($($a,)+) = self;

                let mut i = idx;
                $(
                    if i == 0 {
                        return $a;
                    }
                    i -= 1;
                )+

                unreachable!()
            }

            fn clone_to(&self, idx: usize, slot: &mut Slot) {
                #![allow(unused, non_snake_case)]

                let ($($a,)+) = self;

                let mut i = idx;
                $(
                    if i == 0 {
                        slot.set($a.clone());
                        return;
                    }
                    i -= 1;
                )+

                unreachable!()
            }
        }
    };
}

for_tuple!(any_payload);

trait AnyPayloadStorage: Any + Send {
    fn evict_before(&mut self, idx: u64);
    fn get(&self, idx: u64) -> &dyn AnyPayload;
}

impl dyn AnyPayloadStorage {
    fn is<T: AnyPayload>(&self) -> bool {
        self.type_id() == type_id::<TypedPayloadStorage<T>>()
    }

    // fn downcast_ref<T: AnyPayload>(&self) -> &TypedPayloadStorage<T> {
    //     debug_assert!(self.is::<T>());
    //     unsafe { &*(self as *const dyn AnyPayloadStorage as *const TypedPayloadStorage<T>) }
    // }

    fn downcast_mut<T: AnyPayload>(&mut self) -> &mut TypedPayloadStorage<T> {
        debug_assert!(self.is::<T>());
        unsafe { &mut *(self as *mut dyn AnyPayloadStorage as *mut TypedPayloadStorage<T>) }
    }
}

struct TypedPayloadStorage<T> {
    offset: u64,
    events: VecDeque<T>,
}

impl<T> TypedPayloadStorage<T> {
    fn new(offset: u64) -> Self {
        TypedPayloadStorage {
            offset,
            events: VecDeque::new(),
        }
    }

    fn add(&mut self, payload: T) -> u64 {
        let idx = self.offset + self.events.len() as u64;
        self.events.push_front(payload);
        idx
    }
}

impl<T> AnyPayloadStorage for TypedPayloadStorage<T>
where
    T: AnyPayload,
{
    fn evict_before(&mut self, idx: u64) {
        if idx <= self.offset {
            return;
        }

        let remove_count = idx - self.offset;

        match usize::try_from(remove_count) {
            Ok(remove_count) => {
                if remove_count >= self.events.len() {
                    self.events.clear();
                } else {
                    let new_len = self.events.len() - remove_count;
                    self.events.truncate(new_len);
                }
            }
            Err(_) => {
                self.events.clear();
            }
        }

        self.offset = idx;
    }

    fn get(&self, idx: u64) -> &dyn AnyPayload {
        debug_assert!(idx >= self.offset);
        let pos = idx - self.offset;

        debug_assert!(usize::try_from(pos).is_ok());
        let pos = pos as usize;

        &self.events[pos]
    }
}

/// Events container.
pub struct Events {
    offset: u64,
    events: VecDeque<AnyEvent>,
    storages: NoHashMap<TypeId, Box<dyn AnyPayloadStorage>>,
}

impl Events {
    pub const fn new() -> Self {
        Events {
            offset: 0,
            events: VecDeque::new(),
            storages: no_hash_map(),
        }
    }

    /// Emit an event.
    pub fn emit<T>(&mut self, event: Event<T>)
    where
        T: AnyPayload,
    {
        self.evict(MAX_EVENTS - 1);

        let storage = self
            .storages
            .entry(type_id::<T>())
            .or_insert_with(|| Box::new(TypedPayloadStorage::<T>::new(0)));

        assert!(storage.is::<T>());

        let idx = storage.downcast_mut::<T>().add(event.payload);

        self.events.push_front(AnyEvent {
            id: event.id,
            entity: event.entity,
            payload_id: type_id::<T>(),
            payload_idx: idx,
        });
    }

    pub fn evict(&mut self, keep: usize) {
        if keep >= self.events.len() {
            return;
        }

        // Keeping only `keep` most recent events.
        // Recent events are in the front of the queue.

        for event in self.events.drain(..keep) {
            let storage = self.storages.get_mut(&event.payload_id).unwrap();
            storage.evict_before(event.payload_idx);
        }
    }

    pub fn get(&self, idx: u64) -> Option<Event<&dyn AnyPayload>> {
        if idx < self.offset {
            return None;
        }

        let idx = idx - self.offset;
        if idx >= self.events.len() as u64 {
            return None;
        }

        let pos = self.events.len() - idx as usize - 1;

        let event = &self.events[pos];
        let payload = self.storages[&event.payload_id].get(event.payload_idx);
        Some(Event {
            id: event.id,
            entity: event.entity,
            payload,
        })
    }

    pub fn iter_events(&self, start: u64) -> EventIter {
        let start = start.saturating_sub(self.offset);
        if start >= self.events.len() as u64 {
            return EventIter {
                iter: self.events.range(..0),
                storages: &self.storages,
            };
        }

        let end = self.events.len() - start as usize;
        EventIter {
            iter: self.events.range(..end),
            storages: &self.storages,
        }
    }

    /// First event index stored.
    pub fn start(&self) -> u64 {
        self.offset
    }

    /// One past the last event index.
    pub fn end(&self) -> u64 {
        self.offset + self.events.len() as u64
    }

    /// Fetch next event.
    /// Update start index.
    pub fn next(&self, start: &mut u64) -> Option<Event<&dyn AnyPayload>> {
        if *start < self.offset {
            *start = self.offset;
        }

        let idx = *start - self.offset;
        if idx >= self.events.len() as u64 {
            return None;
        }

        let pos = self.events.len() - idx as usize - 1;

        let event = &self.events[pos];
        let payload = self.storages[&event.payload_id].get(event.payload_idx);
        *start += 1;
        Some(Event {
            id: event.id,
            entity: event.entity,
            payload,
        })
    }
}

pub struct EventIter<'a> {
    iter: std::collections::vec_deque::Iter<'a, AnyEvent>,
    storages: &'a NoHashMap<TypeId, Box<dyn AnyPayloadStorage>>,
}

impl<'a> Iterator for EventIter<'a> {
    type Item = Event<&'a dyn AnyPayload>;

    fn next(&mut self) -> Option<Event<&'a dyn AnyPayload>> {
        let event = self.iter.next_back()?;
        let payload = self.storages[&event.payload_id].get(event.payload_idx);
        Some(Event {
            id: event.id,
            entity: event.entity,
            payload,
        })
    }
}

pub fn init_events(world: &mut World) {
    world.insert_resource(Events::new());
}

pub fn emit_event<T>(world: &World, event: Event<T>)
where
    T: AnyPayload,
{
    let mut events = world.get_resource_mut::<Events>().unwrap();
    events.emit(event);
}
