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
    task::{Poll, Waker},
};

use arcana::{
    edict::{flow::FlowEntity, Component},
    export_arcana_plugin,
};

mod client;

pub use self::client::*;

export_arcana_plugin! {
    InputPlugin {
        filters: [input: InputFilter::new()],
        in world => {
            client::init_world(world);
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

    #[inline(never)]
    fn next(&mut self) -> Option<A> {
        self.iter.next()
    }
}

/// Takes next action from the action queue of the entity.
/// Waits if there is no action in the queue.
/// Returns if the entity is not found or it does not have queue of actions `A`.
pub fn next_action<'a, A>(entity: &'a mut FlowEntity<'_>) -> impl Future<Output = Option<A>> + 'a
where
    A: Send + 'static,
{
    entity.try_poll_view_mut::<&mut ActionQueue<A>, _, _>(|queue, cx| {
        if let Some(action) = queue.actions.pop_front() {
            return Poll::Ready(action);
        }
        queue.waker = Some(cx.waker().clone());
        Poll::Pending
    })
}

/// Extension trait for `FlowEntity` to work with input.
#[allow(async_fn_in_trait)]
pub trait FlowEntityExt {
    async fn next_action<A>(&mut self) -> Option<A>
    where
        A: Send + 'static;
}

impl FlowEntityExt for FlowEntity<'_> {
    async fn next_action<'a, A>(&'a mut self) -> Option<A>
    where
        A: Send + 'static,
    {
        next_action::<A>(self).await
    }
}
