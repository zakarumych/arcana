use std::ops::{Deref, DerefMut};

use arcana::{
    blink_alloc::Blink,
    edict::World,
    input::{Input, InputFilter, ViewInput},
};

arcana::export_arcana_plugin! {
    CursorPlugin {
        resources: [MainCursor(Cursor {
            x: 0.0,
            y: 0.0,
        })],
        filters: [cursor: CursorFilter],
    }
}

/// Value that represents a cursor.
#[derive(Clone, Copy, Debug)]
pub struct Cursor {
    pub x: f32,
    pub y: f32,
}

pub struct MainCursor(Cursor);

impl Deref for MainCursor {
    type Target = Cursor;
    fn deref(&self) -> &Cursor {
        &self.0
    }
}

impl DerefMut for MainCursor {
    fn deref_mut(&mut self) -> &mut Cursor {
        &mut self.0
    }
}

struct CursorFilter;

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
