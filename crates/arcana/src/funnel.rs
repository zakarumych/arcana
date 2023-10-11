use blink_alloc::Blink;
use edict::World;

use crate::events::Event;

pub trait EventFilter {
    fn filter(&mut self, blink: &Blink, world: &mut World, event: Event) -> Option<Event>;
}

pub struct EventFunnel {
    pub filters: Vec<Box<dyn EventFilter>>,
}

impl EventFunnel {
    pub const fn new() -> Self {
        EventFunnel {
            filters: Vec::new(),
        }
    }

    #[inline]
    pub fn add<F>(&mut self, filter: F)
    where
        F: EventFilter + 'static,
    {
        self.filters.push(Box::new(filter));
    }

    #[inline]
    pub fn filter(&mut self, blink: &Blink, world: &mut World, mut event: Event) -> Option<Event> {
        for filter in self.filters.iter_mut() {
            event = filter.filter(blink, world, event)?;
        }
        Some(event)
    }
}

impl EventFilter for EventFunnel {
    #[inline]
    fn filter(&mut self, blink: &Blink, world: &mut World, event: Event) -> Option<Event> {
        self.filter(blink, world, event)
    }
}
