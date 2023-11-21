use std::{
    hash::{BuildHasher, Hash, Hasher},
    iter::FusedIterator,
};

use hashbrown::{HashMap, HashSet};
use winit::{
    error::OsError,
    window::{Window, WindowBuilder, WindowId},
};

use crate::events::EventLoop;

/// Resource containing all windows of the application.
///
/// This resource must be added by all multi-window applications.
/// Single-window applications should add the [`Window`] as resource instead.
///
/// The code that should work for both single- and multi-window applications
/// would check either for [`Window`] or [`Windows`] resource.
pub struct Windows {
    pub windows: HashMap<WindowId, Window>,
}

impl Windows {
    pub fn new() -> Self {
        Windows {
            windows: HashMap::new(),
        }
    }

    pub fn get(&self, id: WindowId) -> Option<&Window> {
        self.windows.get(&id)
    }

    pub fn build(
        &mut self,
        builder: WindowBuilder,
        events: &EventLoop,
    ) -> Result<&Window, OsError> {
        let window = builder.build(events)?;
        Ok(self.windows.entry(window.id()).insert(window).into_mut())
    }

    pub fn open(&mut self, events: &EventLoop) -> Result<&Window, OsError> {
        self.build(WindowBuilder::new(), events)
    }

    pub fn close(&mut self, id: WindowId) -> bool {
        self.windows.remove(&id).is_some()
    }

    pub fn is_empty(&self) -> bool {
        self.windows.is_empty()
    }

    pub fn is_single(&self, id: WindowId) -> bool {
        self.windows.len() == 1 && self.windows.contains_key(&id)
    }

    pub fn iter(&self) -> WindowsIter<'_> {
        WindowsIter {
            iter: self.windows.values(),
        }
    }
}

impl<'a> IntoIterator for &'a Windows {
    type Item = &'a Window;
    type IntoIter = WindowsIter<'a>;

    fn into_iter(self) -> WindowsIter<'a> {
        self.iter()
    }
}

#[derive(Clone)]
pub struct WindowsIter<'a> {
    iter: hashbrown::hash_map::Values<'a, WindowId, Window>,
}

impl<'a> Iterator for WindowsIter<'a> {
    type Item = &'a Window;

    fn next(&mut self) -> Option<Self::Item> {
        self.iter.next()
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.iter.size_hint()
    }
}

impl<'a> ExactSizeIterator for WindowsIter<'a> {
    fn len(&self) -> usize {
        self.iter.len()
    }
}

impl<'a> FusedIterator for WindowsIter<'a> {}
