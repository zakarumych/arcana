//! Event system for the Arcana game engine.
//!

use std::{
    any::{Any, TypeId},
    collections::VecDeque,
};

use edict::EntityId;
use hashbrown::HashMap;

use crate::make_id;

make_id! {
    /// Event ID type.
    /// Any event type must have a unique ID.
    /// Listeners can listen to specific event IDs.
    ///
    /// Events with same ID should be emitted with the same payload type.
    pub EventId
}

pub struct Event<T: ?Sized = dyn Any> {
    id: EventId,
    entity: EntityId,
    value: T,
}

impl<T> Event<T> {
    pub fn new(id: EventId, entity: EntityId, value: T) -> Self {
        Event { id, entity, value }
    }
}

impl Event {
    pub fn get<T: 'static>(&self) -> &T {
        self.value.downcast_ref().unwrap()
    }
}

trait AnyEvents: Any {
    fn next_idx(&self) -> u64;
}

impl dyn AnyEvents {
    fn is<T: 'static>(&self) -> bool {
        self.type_id() == TypeId::of::<TypedEvents<T>>()
    }

    unsafe fn downcast_ref<T: 'static>(&self) -> &TypedEvents<T> {
        debug_assert!(self.is::<T>());
        unsafe { &*(self as *const dyn AnyEvents as *const TypedEvents<T>) }
    }

    unsafe fn downcast_mut<T: 'static>(&mut self) -> &mut TypedEvents<T> {
        debug_assert!(self.is::<T>());
        unsafe { &mut *(self as *mut dyn AnyEvents as *mut TypedEvents<T>) }
    }
}

pub struct TypedEvents<T> {
    /// Entity index offset.
    offset: u64,

    /// Events queue for all entities.
    events: VecDeque<(EntityId, T)>,

    /// Indices of events for particular entities.
    for_entities: HashMap<EntityId, VecDeque<u64>>,
}

impl<T> AnyEvents for TypedEvents<T>
where
    T: 'static,
{
    #[inline(always)]
    fn next_idx(&self) -> u64 {
        self.next_idx()
    }
}

impl<T> TypedEvents<T> {
    fn new(offset: u64) -> Self {
        TypedEvents {
            offset,
            events: VecDeque::new(),
            for_entities: HashMap::new(),
        }
    }

    fn emit(&mut self, event: Event<T>) {
        while self.events.len() >= 1000 {
            self.events.pop_front();
            self.offset += 1;
        }

        let next_id = self.next_idx();

        self.for_entities
            .entry(event.entity)
            .or_default()
            .push_back(next_id);

        self.events.push_back((event.entity, event.value));
    }

    fn iter_events(&self, start: u64) -> std::collections::vec_deque::Iter<(EntityId, T)> {
        let start = start.saturating_sub(self.offset);

        let len = self.events.len();

        let Ok(start) = usize::try_from(start) else {
            return self.events.range(len..);
        };

        if self.events.len() <= start {
            return self.events.range(len..);
        }

        self.events.range(start..)
    }

    fn next_idx(&self) -> u64 {
        debug_assert!(u64::try_from(self.events.len()).is_ok());
        debug_assert!(self.offset.checked_add(self.events.len() as u64).is_some());

        self.offset + self.events.len() as u64
    }
}

/// Events container.
pub struct Events {
    map: HashMap<EventId, Box<dyn AnyEvents>>,
}

impl Events {
    /// Emit an event.
    pub fn emit<T: 'static>(&mut self, event: Event<T>) {
        let events = self
            .map
            .entry(event.id)
            .or_insert_with(|| Box::new(TypedEvents::<T>::new(0)));

        if !events.is::<T>() {
            // Reset queue if value type changes.
            let offset = events.next_idx();
            *events = Box::new(TypedEvents::<T>::new(offset))
        }

        let typed_events = unsafe { events.downcast_mut::<T>() };
        typed_events.emit(event);
    }

    fn iter_events<T: 'static>(
        &self,
        id: EventId,
        start: u64,
    ) -> std::collections::vec_deque::Iter<'_, (EntityId, T)> {
        let Some(events) = self.map.get(&id) else {
            return const { &VecDeque::new() }.iter();
        };

        if !events.is::<T>() {
            return const { &VecDeque::new() }.iter();
        }

        let typed_events = unsafe { events.downcast_ref::<T>() };
        typed_events.iter_events(start)
    }
}

/// Listener for a specific event type.
pub struct EventListener {
    id: EventId,
    last_event_idx: u64,
}

impl EventListener {
    pub const fn new(id: EventId) -> Self {
        EventListener {
            id,
            last_event_idx: 0,
        }
    }

    pub fn iter_events<'a, T: 'static>(
        &'a mut self,
        events: &'a Events,
    ) -> EventListenerIter<'a, T> {
        let iter = events.iter_events::<T>(self.id, self.last_event_idx);
        EventListenerIter {
            iter,
            last_event_idx: &mut self.last_event_idx,
        }
    }
}

pub struct EventListenerIter<'a, T> {
    iter: std::collections::vec_deque::Iter<'a, (EntityId, T)>,
    last_event_idx: &'a mut u64,
}
