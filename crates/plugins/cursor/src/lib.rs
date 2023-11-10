use std::ops::{Deref, DerefMut};

use arcana::{
    blink_alloc::Blink,
    edict::World,
    events::{Event, WindowEvent},
    funnel::EventFilter,
    plugin::{ArcanaPlugin, PluginInit},
    plugin_init,
    project::{ident, Ident},
    winit::window::Window,
};

arcana::export_arcana_plugin!(CursorPlugin);

pub struct CursorPlugin;

impl ArcanaPlugin for CursorPlugin {
    fn init(&self, world: &mut World) -> PluginInit {
        world.insert_resource(MainCursor(Cursor {
            pos: na::Point2::origin(),
        }));

        plugin_init!()
    }

    fn event_filters(&self) -> Vec<&Ident> {
        vec![ident!(CursorFilter)]
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
    fn filter(&mut self, _blink: &Blink, world: &mut World, event: Event) -> Option<Event> {
        let mut cursor = world.expect_resource_mut::<MainCursor>();
        let window = world.expect_resource::<Window>();

        match event {
            Event::WindowEvent { event, window_id } => {
                if window_id == window.id() {
                    match event {
                        WindowEvent::CursorMoved { position, .. } => {
                            let inner_size = window.inner_size();
                            let pos = na::Point2::new(
                                position.x as f32 / inner_size.width as f32 * 2.0 - 1.0,
                                1.0 - position.y as f32 / inner_size.height as f32 * 2.0,
                            );

                            cursor.0.pos = pos;
                            return None;
                        }
                        _ => {}
                    }
                }
                Some(Event::WindowEvent { window_id, event })
            }
            _ => Some(event),
        }
    }
}
