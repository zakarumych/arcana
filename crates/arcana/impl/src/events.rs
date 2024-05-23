//! Event system for the Arcana game engine.
//!

use std::any::Any;

use crate::make_id;

make_id! {
    /// Event ID type.
    /// Any emittable event type must have a unique ID.
    /// Listners can listen to specific event IDs.
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

/// Listner for a specific event type.
pub struct EventListener {
    id: EventId,
    last_event_idx: u64,
}

/// Type used events emitting.
pub struct Events {}
