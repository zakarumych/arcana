//! Event system for the Arcana game engine.
//!

use std::{
    any::{Any, TypeId},
    collections::VecDeque,
    mem::size_of,
};

use edict::{Entity, EntityId, World};
use hashbrown::HashMap;

use crate::{make_id, static_assert, type_id, Slot};

const MAX_EVENTS_BY_TYPE: usize = 65536;

static_assert!(
    size_of::<usize>() <= size_of::<u64>(),
    "Unchecked cast from usize to u64 is performed in this module"
);

make_id! {
    /// Event ID type.
    /// Any event type must have a unique ID.
    /// Listeners can listen to specific event IDs.
    ///
    /// Events with same ID should be emitted with the same payload type.
    pub EventId
}

pub struct Event<T: ?Sized = dyn Any> {
    pub id: EventId,
    pub entity: EntityId,
    pub payload: T,
}

impl<T> Event<T> {
    #[inline(always)]
    pub fn new(id: EventId, entity: impl Entity, payload: T) -> Self {
        Event {
            id,
            entity: entity.id(),
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

// trait AnyEvents: Any {
//     fn next_idx(&self) -> u64;

//     fn evict(&mut self, keep: usize);

//     fn get_event(&self, idx: u64) -> Option<(EntityId, &dyn Any)>;
// }

// impl dyn AnyEvents {
//     fn is<T: 'static>(&self) -> bool {
//         self.type_id() == type_id::<TypedEvents<T>>()
//     }

//     unsafe fn downcast_ref<T: 'static>(&self) -> &TypedEvents<T> {
//         debug_assert!(self.is::<T>());
//         unsafe { &*(self as *const dyn AnyEvents as *const TypedEvents<T>) }
//     }

//     unsafe fn downcast_mut<T: 'static>(&mut self) -> &mut TypedEvents<T> {
//         debug_assert!(self.is::<T>());
//         unsafe { &mut *(self as *mut dyn AnyEvents as *mut TypedEvents<T>) }
//     }
// }

// pub struct TypedEvents<T> {
//     /// Entity index offset.
//     offset: u64,

//     /// Events queue for all entities.
//     events: VecDeque<(EntityId, T)>,

//     /// Indices of events for particular entities.
//     for_entities: HashMap<EntityId, VecDeque<u64>>,
// }

// impl<T> AnyEvents for TypedEvents<T>
// where
//     T: 'static,
// {
//     #[inline(always)]
//     fn next_idx(&self) -> u64 {
//         self.next_idx()
//     }

//     #[inline(always)]
//     fn evict(&mut self, keep: usize) {
//         self.evict(keep);
//     }

//     #[inline(always)]
//     fn get_event(&self, idx: u64) -> Option<(EntityId, &dyn Any)> {
//         let pos = idx.checked_sub(self.offset)?;
//         let pos = pos as usize;

//         let (entity, payload) = self.events.get(pos)?;
//         Some((*entity, payload as &dyn Any))
//     }
// }

// impl<T> TypedEvents<T> {
//     fn new(offset: u64) -> Self {
//         TypedEvents {
//             offset,
//             events: VecDeque::new(),
//             for_entities: HashMap::new(),
//         }
//     }

//     /// Evict old events keeping only `keep` most recent ones.
//     fn evict(&mut self, keep: usize) {
//         if self.events.len() <= keep {
//             return;
//         }

//         let evict_count = self.events.len() - keep;
//         self.events.truncate(keep);

//         debug_assert!(u64::try_from(evict_count).is_ok());
//         debug_assert!(self.offset.checked_add(evict_count as u64).is_some());

//         self.offset += evict_count as u64;

//         for queue in self.for_entities.values_mut() {
//             match queue.binary_search_by(|&idx| self.offset.cmp(&idx)) {
//                 Ok(pos) => queue.truncate(pos + 1),
//                 Err(pos) => queue.truncate(pos),
//             };
//         }
//     }

//     /// Emit an event, adding it to the queue.
//     fn emit(&mut self, event: Event<T>) {
//         // Never store too many events.
//         self.evict(MAX_EVENTS_BY_TYPE - 1);

//         let next_id = self.next_idx();

//         self.for_entities
//             .entry(event.entity)
//             .or_default()
//             .push_front(next_id);

//         self.events.push_front((event.entity, event.payload));
//     }

//     /// Iterate over events starting from `start` index.
//     ///
//     /// Returns the number of skipped events (that was evicted) and an iterator over events.
//     fn iter_events(&self, start: u64) -> (u64, std::collections::vec_deque::Iter<(EntityId, T)>) {
//         let start = start.saturating_sub(self.offset);
//         let skipped = self.offset.saturating_sub(start);

//         let len = self.events.len();

//         let Ok(start) = usize::try_from(start) else {
//             return (skipped, self.events.range(..0));
//         };

//         if self.events.len() <= start {
//             return (skipped, self.events.range(..0));
//         }

//         let end = len - start;

//         (skipped, self.events.range(..end))
//     }

//     /// Iterate over events for a specific entity starting from `start` index.
//     fn iter_entity_events(
//         &self,
//         entity: EntityId,
//         start: u64,
//     ) -> std::collections::vec_deque::Iter<u64> {
//         let start = start.saturating_sub(self.offset);

//         let Some(queue) = self.for_entities.get(&entity) else {
//             return const { &VecDeque::new() }.iter();
//         };

//         match queue.binary_search_by(|idx| start.cmp(&idx)) {
//             Ok(pos) => queue.range(..=pos),
//             Err(pos) => queue.range(..pos),
//         }
//     }

//     fn next_idx(&self) -> u64 {
//         debug_assert!(u64::try_from(self.events.len()).is_ok());
//         debug_assert!(self.offset.checked_add(self.events.len() as u64).is_some());

//         self.offset + self.events.len() as u64
//     }
// }

struct AnyEvent {
    id: EventId,
    entity: EntityId,
    payload_id: TypeId,
    payload_idx: u64,
}

pub trait AnyPayload: Any + Send {
    fn len(&self) -> usize;
    fn get(&self, idx: usize) -> &(dyn Any + Send);
    fn get_mut(&mut self, idx: usize) -> &mut (dyn Any + Send);
    fn clone_to(&self, idx: usize, slot: &mut Slot);
}

macro_rules! any_payload {
    () => {
        impl AnyPayload for () {
            fn len(&self) -> usize {
                0
            }

            fn get(&self, _idx: usize) -> &(dyn Any + Send) {
                unreachable!()
            }

            fn get_mut(&mut self, _idx: usize) -> &mut (dyn Any + Send) {
                unreachable!()
            }

            fn clone_to(&self, _idx: usize, _slot: &mut Slot) {
                unreachable!()
            }
        }
    };
    (A) => {
        impl<A> AnyPayload for (A,)
        where
            A: Any + Clone + Send,
        {
            fn len(&self) -> usize {
                1
            }

            fn get(&self, idx: usize) -> &(dyn Any + Send) {
                match idx {
                    0 => &self.0,
                    _ => unreachable!(),
                }
            }

            fn get_mut(&mut self, idx: usize) -> &mut (dyn Any + Send) {
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
    (A B) => {
        impl<A, B> AnyPayload for (A, B)
        where
            A: Any + Clone + Send,
            B: Any + Clone + Send,
        {
            fn len(&self) -> usize {
                2
            }

            fn get(&self, idx: usize) -> &(dyn Any + Send) {
                match idx {
                    0 => &self.0,
                    1 => &self.1,
                    _ => unreachable!(),
                }
            }

            fn get_mut(&mut self, idx: usize) -> &mut (dyn Any + Send) {
                match idx {
                    0 => &mut self.0,
                    1 => &mut self.1,
                    _ => unreachable!(),
                }
            }

            fn clone_to(&self, idx: usize, slot: &mut Box<dyn Any>) {
                match idx {
                    0 => slot.set(self.0.clone()),
                    1 => slot.set(self.1.clone()),
                    _ => unreachable!(),
                }
            }
        }
    };
    (A B C) => {
        impl<A, B, C> AnyPayload for (A, B, C)
        where
            A: Any + Clone + Send,
            B: Any + Clone + Send,
            C: Any + Clone + Send,
        {
            fn len(&self) -> usize {
                3
            }

            fn get(&self, idx: usize) -> &(dyn Any + Send) {
                match idx {
                    0 => &self.0,
                    1 => &self.1,
                    2 => &self.2,
                    _ => unreachable!(),
                }
            }

            fn get_mut(&mut self, idx: usize) -> &mut (dyn Any + Send) {
                match idx {
                    0 => &mut self.0,
                    1 => &mut self.1,
                    2 => &mut self.2,
                    _ => unreachable!(),
                }
            }

            fn clone_to(&self, idx: usize, slot: &mut Box<dyn Any>) {
                match idx {
                    0 => slot.set(self.0.clone()),
                    1 => slot.set(self.1.clone()),
                    2 => slot.set(self.2.clone()),
                    _ => unreachable!(),
                }
            }
        }
    };
    (A B C D) => {
        impl<A, B, C, D> AnyPayload for (A, B, C, D)
        where
            A: Any + Clone + Send,
            B: Any + Clone + Send,
            C: Any + Clone + Send,
            D: Any + Clone + Send,
        {
            fn len(&self) -> usize {
                4
            }

            fn get(&self, idx: usize) -> &(dyn Any + Send) {
                match idx {
                    0 => &self.0,
                    1 => &self.1,
                    2 => &self.2,
                    3 => &self.3,
                    _ => unreachable!(),
                }
            }

            fn get_mut(&mut self, idx: usize) -> &mut (dyn Any + Send) {
                match idx {
                    0 => &mut self.0,
                    1 => &mut self.1,
                    2 => &mut self.2,
                    3 => &mut self.3,
                    _ => unreachable!(),
                }
            }

            fn clone_to(&self, idx: usize, slot: &mut Box<dyn Any>) {
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
            $($a: Any + Clone + Send,)+
        {
            fn len(&self) -> usize {
                let mut count = 0;
                $(
                    let _: $a;
                    count += 1;
                )+
                count
            }

            fn get(&self, idx: usize) -> &(dyn Any + Send) {
                #![allow(unused)]

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

            fn get_mut(&mut self, idx: usize) -> &mut (dyn Any + Send) {
                #![allow(unused)]

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
                #![allow(unused)]

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

trait AnyPayloadStorage: Any {
    fn evict_before(&mut self, idx: u64);
    fn get(&self, idx: u64) -> &dyn AnyPayload;
}

impl dyn AnyPayloadStorage {
    fn is<T: AnyPayload>(&self) -> bool {
        type_id::<T>() == type_id::<TypedPayloadStorage<T>>()
    }

    fn downcast_ref<T: AnyPayload>(&self) -> &TypedPayloadStorage<T> {
        debug_assert!(self.is::<T>());
        unsafe { &*(self as *const dyn AnyPayloadStorage as *const TypedPayloadStorage<T>) }
    }

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
    storages: HashMap<TypeId, Box<dyn AnyPayloadStorage>>,
}

impl Events {
    pub fn new() -> Self {
        Events {
            offset: 0,
            events: VecDeque::new(),
            storages: HashMap::new(),
        }
    }

    /// Emit an event.
    pub fn emit<T>(&mut self, event: Event<T>)
    where
        T: AnyPayload,
    {
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

        let event = self.events.get(pos)?;
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

        let idx = idx as usize;
        let event = &self.events[idx];
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
    storages: &'a HashMap<TypeId, Box<dyn AnyPayloadStorage>>,
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

// /// Listener for a specific event type.
// pub struct EventListener {
//     id: EventId,
//     next_event_idx: u64,
// }

// impl EventListener {
//     pub const fn new(id: EventId) -> Self {
//         EventListener {
//             id,
//             next_event_idx: 0,
//         }
//     }

//     pub const fn with_next_idx(id: EventId, next_idx: u64) -> Self {
//         EventListener {
//             id,
//             next_event_idx: next_idx,
//         }
//     }

//     pub fn iter<'a, T: 'static>(&mut self, events: &'a Events) -> EventIter<'a, '_, T> {
//         events.iter_events(self.id, &mut self.next_event_idx)
//     }
// }

// pub struct EventIter<'a, 'b, T> {
//     iter: std::collections::vec_deque::Iter<'a, (EntityId, T)>,
//     next_event_idx: &'b mut u64,
// }

// impl<'a, 'b, T> Iterator for EventIter<'a, 'b, T> {
//     type Item = (EntityId, &'a T);

//     #[inline(always)]
//     fn size_hint(&self) -> (usize, Option<usize>) {
//         self.iter.size_hint()
//     }

//     #[inline(always)]
//     fn next(&mut self) -> Option<(EntityId, &'a T)> {
//         let &(entity, ref payload) = self.iter.next_back()?;
//         *self.next_event_idx += 1;
//         Some((entity, payload))
//     }

//     #[inline]
//     fn nth(&mut self, n: usize) -> Option<(EntityId, &'a T)> {
//         let len = self.iter.len();

//         match self.iter.nth_back(n) {
//             None => {
//                 debug_assert!(u64::try_from(len).is_ok());
//                 *self.next_event_idx += len as u64;
//                 return None;
//             }
//             Some(&(entity, ref payload)) => {
//                 debug_assert!(u64::try_from(n).is_ok());
//                 *self.next_event_idx += n as u64;
//                 return Some((entity, payload));
//             }
//         }
//     }

//     #[inline(always)]
//     fn fold<B, F>(self, init: B, mut f: F) -> B
//     where
//         Self: Sized,
//         F: FnMut(B, (EntityId, &'a T)) -> B,
//     {
//         self.iter.rfold(init, move |acc, &(entity, ref payload)| {
//             *self.next_event_idx += 1;
//             f(acc, (entity, payload))
//         })
//     }
// }

// /// Listener for a specific event type.
// pub struct EntityEventListener {
//     entity: EntityId,
//     id: EventId,
//     next_event_idx: u64,
// }

// impl EntityEventListener {
//     pub const fn new(entity: EntityId, id: EventId) -> Self {
//         EntityEventListener {
//             entity,
//             id,
//             next_event_idx: 0,
//         }
//     }

//     pub const fn with_next_idx(entity: EntityId, id: EventId, next_idx: u64) -> Self {
//         EntityEventListener {
//             entity,
//             id,
//             next_event_idx: next_idx,
//         }
//     }

//     pub fn iter<'a, T: 'static>(&mut self, events: &'a Events) -> EntityEventIter<'a, '_, T> {
//         events.iter_entity_events(self.entity, self.id, &mut self.next_event_idx)
//     }
// }

// pub struct EntityEventIter<'a, 'b, T> {
//     #[cfg(debug_assertions)]
//     entity: EntityId,
//     iter: std::collections::vec_deque::Iter<'a, u64>,
//     events: &'a VecDeque<(EntityId, T)>,
//     offset: u64,
//     next_event_idx: &'b mut u64,
// }

// impl<'a, 'b, T> EntityEventIter<'a, 'b, T>
// where
//     T: 'a,
// {
//     fn _get(&mut self, idx: u64) -> &'a T {
//         debug_assert!(idx >= self.offset);
//         debug_assert!(usize::try_from(idx - self.offset).is_ok());

//         let pos = self.events.len() - 1 - (idx - self.offset) as usize;
//         let &(_entity, ref payload) = &self.events[pos];

//         #[cfg(debug_assertions)]
//         assert_eq!(_entity, self.entity);

//         debug_assert!(*self.next_event_idx <= idx);
//         *self.next_event_idx = idx + 1;

//         payload
//     }
// }

// impl<'a, 'b, T> Iterator for EntityEventIter<'a, 'b, T>
// where
//     T: 'a,
// {
//     type Item = &'a T;

//     #[inline(always)]
//     fn size_hint(&self) -> (usize, Option<usize>) {
//         self.iter.size_hint()
//     }

//     #[inline(always)]
//     fn next(&mut self) -> Option<&'a T> {
//         let idx = *self.iter.next_back()?;
//         Some(self._get(idx))
//     }

//     #[inline]
//     fn nth(&mut self, n: usize) -> Option<&'a T> {
//         let Some(&last_idx) = self.iter.clone().next() else {
//             return None;
//         };

//         match self.iter.nth_back(n) {
//             None => {
//                 *self.next_event_idx = last_idx;
//                 return None;
//             }
//             Some(&idx) => Some(self._get(idx)),
//         }
//     }

//     #[inline(always)]
//     fn fold<B, F>(mut self, init: B, mut f: F) -> B
//     where
//         Self: Sized,
//         F: FnMut(B, &'a T) -> B,
//     {
//         self.iter
//             .clone()
//             .rfold(init, move |acc, &idx| f(acc, self._get(idx)))
//     }
// }

// #[test]
// fn test_events_emit() {
//     const TEST_EVENT: EventId = crate::local_name_hash_id!(TEST_EVENT => EventId);
//     type TestEvent = u32;

//     let test_entity1 = EntityId::from_bits(1).unwrap();
//     let test_entity2 = EntityId::from_bits(2).unwrap();

//     let mut events = Events::new();

//     let mut listener = EventListener::new(TEST_EVENT);

//     assert_eq!(listener.iter::<TestEvent>(&events).count(), 0, "no events");

//     events.emit(Event {
//         id: TEST_EVENT,
//         entity: test_entity1,
//         payload: 1 as TestEvent,
//     });

//     assert_eq!(
//         listener.iter::<TestEvent>(&events).collect::<Vec<_>>(),
//         vec![(test_entity1, &1)],
//         "One event with payload"
//     );

//     let mut entity_listener = EntityEventListener::new(test_entity2, TEST_EVENT);

//     assert_eq!(
//         entity_listener.iter::<TestEvent>(&events).count(),
//         0,
//         "no events"
//     );

//     events.emit(Event {
//         id: TEST_EVENT,
//         entity: test_entity2,
//         payload: 2 as TestEvent,
//     });

//     assert_eq!(
//         entity_listener
//             .iter::<TestEvent>(&events)
//             .collect::<Vec<_>>(),
//         vec![&2],
//         "One event with payload"
//     );

//     let mut listener = EventListener::new(TEST_EVENT);

//     assert_eq!(
//         listener.iter::<TestEvent>(&events).collect::<Vec<_>>(),
//         vec![(test_entity1, &1), (test_entity2, &2)],
//         "Two events"
//     );
// }

pub fn init_events(world: &mut World) {
    world.insert_resource(Events::new());
}
