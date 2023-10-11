//! This plugin provides the InputHandler to grab events
//! and direct them to the user-defined controller.
//!
//! Controller consumes events.
//! Out-of-the-box controller is Commander which
//! translates events to commands and sends them to the command queue
//! of associated entity.
//!
//! User may find Translator useful to grab raw events and translate them
//! to user-defined actions using mapping.
//!
//! The crate also provides few preset Translators and Commanders.

use std::collections::VecDeque;

use arcana::{
    edict::{Component, Scheduler, World},
    export_arcana_plugin,
    plugin::ArcanaPlugin,
};

arcana::feature_client! {
    mod client;

    pub use self::client::*;
}

export_arcana_plugin!(ThePlugin);

pub struct ThePlugin;

impl ArcanaPlugin for ThePlugin {
    fn name(&self) -> &'static str {
        "input"
    }

    fn init(&self, world: &mut World, scheduler: &mut Scheduler) {
        arcana::feature_client! {
            client::init(world, scheduler);
        }
    }

    arcana::feature_client! {
        fn init_funnel(&self, funnel: &mut arcana::funnel::EventFunnel) {
            client::init_funnel(funnel);
        }
    }
}

pub struct CommandQueue<A> {
    actions: VecDeque<A>,
}

impl<A> Component for CommandQueue<A>
where
    A: 'static,
{
    fn name() -> &'static str {
        "CommandQueue"
    }
}

impl<A> CommandQueue<A> {
    pub fn drain(&mut self) -> CommandQueueIter<A> {
        CommandQueueIter {
            iter: self.actions.drain(..),
        }
    }
}

pub struct CommandQueueIter<'a, A> {
    iter: std::collections::vec_deque::Drain<'a, A>,
}

impl<'a, A> Iterator for CommandQueueIter<'a, A> {
    type Item = A;

    #[inline(always)]
    fn next(&mut self) -> Option<A> {
        self.iter.next()
    }
}
