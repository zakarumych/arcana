//! Contains logic for the viewports.

use std::{any::TypeId, marker::PhantomData, ptr::NonNull};

use edict::{
    archetype::Archetype,
    query::{Fetch, IntoQuery, WriteAlias},
    Access, Component, Query,
};
use winit::window::Window;

/// Viewport is where game displays its content.
///
/// Viewport can be bound to the UI widget, window, arbitrary texture etc.
/// They are similar to window in usage, but behave differently.
///
/// For one `Viewport` does not own what it is bound to.
/// This allows `Viewport`s to be copyable.
///
/// Most logic should prefer to use `Viewport` instead of a `Window`.
/// When it needs the `Window` behind the `Viewport`, it should be careful to
/// work when `Viewport` is NOT bound to a `Window`.
///
/// If logic is not supposed to work without a `Window`, it should use `Window` directly.
///
/// The type itself is a Query that fetches viewport data from entity.
pub enum Viewport<'a> {
    Window(&'a Window),
    Texture(&'a ViewportTexture),
}

/// Component that holds texture the viewport is bound to.
pub struct ViewportTexture {
    pub image: mev::Image,
}

// pub enum ViewportFetch<'a> {
//     Window(NonNull<Window>, PhantomData<&'a [Window]>),
//     Texture(NonNull<ViewportTexture>, PhantomData<&'a [ViewportTexture]>),
// }

// unsafe impl<'a> Fetch<'a> for ViewportFetch<'a> {
//     type Item = Viewport<'a>;

//     fn dangling() -> Self {
//         Self::Window(NonNull::dangling(), PhantomData)
//     }

//     unsafe fn get_item(&mut self, idx: u32) -> Viewport<'a> {
//         match *self {
//             Self::Window(ptr, _) => Viewport::Window(&*ptr.as_ptr().add(idx as usize)),
//             Self::Texture(ptr, _) => Viewport::Texture(&*ptr.as_ptr().add(idx as usize)),
//         }
//     }
// }

// pub struct ViewportQuery;

// impl Query for ViewportQuery {
//     type Fetch<'a> = ViewportFetch<'a>;
//     type Item<'a> = Viewport<'a>;

//     fn access_archetype(&self, archetype: &Archetype, mut f: impl FnMut(TypeId, Access)) {
//         if archetype.has_component(TypeId::of::<Window>()) {
//             f(TypeId::of::<Window>(), Access::Read);
//         } else if archetype.has_component(TypeId::of::<ViewportTexture>()) {
//             f(TypeId::of::<ViewportTexture>(), Access::Read);
//         }
//     }

//     fn component_type_access(&self, ty: TypeId) -> Result<Option<Access>, WriteAlias> {
//         if ty == TypeId::of::<Window>() {
//             Ok(Some(Access::Read))
//         } else if ty == TypeId::of::<ViewportTexture>() {
//             Ok(Some(Access::Read))
//         } else {
//             Ok(None)
//         }
//     }
// }
