use edict::World;

use crate::events::Event;

pub trait Filter {
    fn filter(&mut self, world: &mut World, event: Event) -> Option<Event>;
}

pub struct Funnel {
    pub filters: Vec<Box<dyn Filter>>,
}

impl Funnel {
    pub fn new() -> Self {
        Funnel {
            filters: Vec::new(),
        }
    }

    pub fn add<F>(&mut self, funnel: F)
    where
        F: Filter + 'static,
    {
        self.filters.push(Box::new(funnel));
    }

    #[inline]
    pub fn filter(&mut self, world: &mut World, mut event: Event) -> Option<Event> {
        for filter in self.filters.iter_mut() {
            event = filter.filter(world, event)?;
        }
        Some(event)
    }
}

impl Filter for Funnel {
    #[inline]
    fn filter(&mut self, world: &mut World, event: Event) -> Option<Event> {
        self.filter(world, event)
    }
}
