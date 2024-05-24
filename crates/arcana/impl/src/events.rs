//! Event system for the Arcana game engine.
//!

use std::{
    any::{Any, TypeId},
    collections::VecDeque,
};

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
    value: T,
}

impl<T> Event<T> {
    pub fn new(value: T) -> Self {
        Event { value }
    }
}

impl Event {
    pub fn get<T: 'static>(&self) -> &T {
        self.value.downcast_ref().unwrap()
    }
}

/// Listener for a specific event type.
pub struct EventListener {
    id: EventId,
    last_event_idx: u64,
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
    offset: u64,
    events: VecDeque<Event<T>>,
}

impl<T> AnyEvents for TypedEvents<T>
where
    T: 'static,
{
    fn next_idx(&self) -> u64 {
        self.offset + self.events.len() as u64
    }
}

impl<T> TypedEvents<T> {
    pub fn new(offset: u64) -> Self {
        TypedEvents {
            offset: 0,
            events: VecDeque::new(),
        }
    }

    pub fn emit(&mut self, value: T) {
        while self.events.len() >= 1000 {
            self.events.pop_front();
            self.offset += 1;
        }
        self.events.push_back(Event { value });
    }
}

/// Events container.
pub struct Events {
    map: HashMap<EventId, Box<dyn AnyEvents>>,
}

impl Events {
    /// Emit event with value payload.
    pub fn emit<T: 'static>(&mut self, event: EventId, value: T) {
        let events = self
            .map
            .entry(event)
            .or_insert_with(|| Box::new(TypedEvents::<T>::new(0)));
        if !events.is::<T>() {
            let offset = events.next_idx();
            *events = Box::new(TypedEvents::<T>::new(offset))
        }
        let typed_events = unsafe { events.downcast_mut::<T>() };
        typed_events.emit(value);
    }
}
