use std::ops::{Deref, DerefMut};

use arcana::{
    blink_alloc::Blink,
    edict::World,
    events::EventFilter,
    events::{Event, ViewportEvent},
};

arcana::export_arcana_plugin! {
    CursorPlugin {
        resources: [MainCursor(Cursor {
            pos: na::Point2::origin(),
        })],
        filters: [cursor: CursorFilter],
    }
}

/// Value that represents a cursor.
#[derive(Clone, Copy, Debug)]
pub struct Cursor {
    pos: na::Point2<f32>,
}

impl Cursor {
    /// Returns cursor position in world space.
    pub fn position(&self) -> na::Point2<f32> {
        self.pos
    }
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

// /// Resource that contains cursors of active windows.
// pub struct Cursors {
//     windows: HashMap<WindowId, Cursor>,
// }

// impl Cursors {
//     pub fn new() -> Self {
//         Cursors {
//             windows: HashMap::new(),
//         }
//     }
// }

struct CursorFilter;

impl EventFilter for CursorFilter {
    fn filter(&mut self, _blink: &Blink, world: &mut World, event: &Event) -> bool {
        let mut cursor = world.expect_resource_mut::<MainCursor>();

        match *event {
            Event::ViewportEvent { ref event, .. } => match *event {
                ViewportEvent::CursorMoved { x, y, .. } => {
                    let pos = na::Point2::new(x as f32, 1.0 - y as f32);
                    cursor.0.pos = pos;
                }
                _ => {}
            },
            _ => {}
        }
        false
    }
}
