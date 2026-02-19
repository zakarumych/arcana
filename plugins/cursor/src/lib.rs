use std::ops::{Deref, DerefMut};

use arcana::{
    blink_alloc::Blink,
    edict::World,
    input::{Input, InputFilter, ViewInput},
};

arcana::declare_plugin!();

/// Value that represents a cursor.
#[derive(Clone, Copy, Debug)]
pub struct Cursor {
    pub x: f32,
    pub y: f32,
}

/// A resource that contains main cursor.
/// Main cursor receives mouse moving events and updates its position.
pub struct MainCursor(Cursor);

impl Deref for MainCursor {
    type Target = Cursor;

    #[inline]
    fn deref(&self) -> &Cursor {
        &self.0
    }
}

impl DerefMut for MainCursor {
    #[inline]
    fn deref_mut(&mut self) -> &mut Cursor {
        &mut self.0
    }
}

pub struct CursorFilter;

impl InputFilter for CursorFilter {
    fn filter(&mut self, _blink: &Blink, world: &mut World, event: &Input) -> bool {
        let mut cursor = world.expect_resource_mut::<MainCursor>();

        match *event {
            Input::ViewInput { ref input } => match *input {
                ViewInput::CursorMoved { x, y, .. } => {
                    cursor.x = x as f32;
                    cursor.y = y as f32;
                }
                _ => {}
            },
            _ => {}
        }
        false
    }
}
