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

use std::{
    collections::VecDeque,
    future::Future,
    task::{Context, Poll, Waker},
};

use arcana::{
    edict::{flow::FlowEntity, Component, EntityError, World},
    export_arcana_plugin,
    plugin::{ArcanaPlugin, PluginInit},
};

arcana::feature_client! {
    mod client;

    pub use self::client::*;
}

export_arcana_plugin!(ThePlugin);

pub struct ThePlugin;

impl ArcanaPlugin for ThePlugin {
    fn init(&self, world: &mut World) -> PluginInit {
        arcana::feature_client! {
            client::init_world(world);
        }
        let mut init = PluginInit::new();
        arcana::feature_client! {
            init.filters.push(client::init_event_filter());
        }
        init
    }

    arcana::feature_client! {
        fn event_filters(&self) -> Vec<&arcana::project::Ident> {
            vec![client::event_filter()]
        }
    }
}

pub struct ActionQueue<A> {
    actions: VecDeque<A>,
    waker: Option<Waker>,
}

impl<A> Component for ActionQueue<A>
where
    A: 'static,
{
    fn name() -> &'static str {
        "ActionQueue"
    }
}

impl<A> ActionQueue<A> {
    pub fn drain(&mut self) -> ActionQueueIter<A> {
        ActionQueueIter {
            iter: self.actions.drain(..),
        }
    }
}

pub struct ActionQueueIter<'a, A> {
    iter: std::collections::vec_deque::Drain<'a, A>,
}

impl<'a, A> Iterator for ActionQueueIter<'a, A> {
    type Item = A;

    #[inline(always)]
    fn next(&mut self) -> Option<A> {
        self.iter.next()
    }
}

/// Takes next action from the action queue of the entity.
/// Waits if there is no action in the queue.
/// Returns if the entity is not found or it does not have queue of actions `A`.
pub fn next_action<A>(entity: FlowEntity<'_>) -> impl Future<Output = Result<A, EntityError>> + '_
where
    A: Send + 'static,
{
    std::future::poll_fn(move |cx: &mut Context| {
        let queue = unsafe { entity.fetch_mut::<ActionQueue<A>>()? };
        if let Some(action) = queue.actions.pop_front() {
            return Poll::Ready(Ok(action));
        }
        queue.waker = Some(cx.waker().clone());
        Poll::Pending
    })
}

/// Extension trait for `FlowEntity` to work with input.
pub trait FlowEntityExt {
    async fn next_action<A>(&self) -> Result<A, EntityError>
    where
        A: Send + 'static;
}

impl FlowEntityExt for FlowEntity<'_> {
    async fn next_action<A>(&self) -> Result<A, EntityError>
    where
        A: Send + 'static,
    {
        next_action(*self).await
    }
}
