//! Resource requests for unfolding callbacks.

use edict::{
    resources::{Res, ResMut},
    world::World,
};

mod sealed {
    pub trait Sealed {}
}

/// A resource request made by an unfolding component.
///
/// Supported requests are `()`, shared and mutable references, optional
/// references, and tuples of requests up to twelve elements. Required resources
/// must exist before the component query advances its modification checkpoint.
/// Conflicting resource borrows panic through the world's borrow checks.
/// This trait is sealed so requests cannot borrow entity components.
pub trait ResourceQuery: sealed::Sealed + 'static {
    /// Guards retaining resource borrows for one typed unfolding pass.
    type Guard<'w>;
    /// Short-lived references passed to an individual callback.
    type Item<'a>;

    /// Fetch resources, returning `None` when a required resource is absent.
    fn fetch(world: &World) -> Option<Self::Guard<'_>>;

    /// Borrow the retained guards for an individual callback.
    fn reborrow<'a, 'w>(guard: &'a mut Self::Guard<'w>) -> Self::Item<'a>
    where
        'w: 'a;
}

/// The callback item for a resource request.
pub type ResourceItem<'a, R> = <R as ResourceQuery>::Item<'a>;

impl sealed::Sealed for () {}
impl ResourceQuery for () {
    type Guard<'w> = ();
    type Item<'a> = ();
    fn fetch(_: &World) -> Option<()> {
        Some(())
    }
    fn reborrow<'a, 'w>(_: &'a mut ())
    where
        'w: 'a,
    {
    }
}

impl<T: Sync + 'static> sealed::Sealed for &'static T {}
impl<T: Sync + 'static> ResourceQuery for &'static T {
    type Guard<'w> = Res<'w, T>;
    type Item<'a> = &'a T;
    fn fetch(world: &World) -> Option<Self::Guard<'_>> {
        world.get_resource::<T>()
    }
    fn reborrow<'a, 'w>(guard: &'a mut Self::Guard<'w>) -> &'a T
    where
        'w: 'a,
    {
        guard
    }
}

impl<T: Send + 'static> sealed::Sealed for &'static mut T {}
impl<T: Send + 'static> ResourceQuery for &'static mut T {
    type Guard<'w> = ResMut<'w, T>;
    type Item<'a> = &'a mut T;
    fn fetch(world: &World) -> Option<Self::Guard<'_>> {
        world.get_resource_mut::<T>()
    }
    fn reborrow<'a, 'w>(guard: &'a mut Self::Guard<'w>) -> &'a mut T
    where
        'w: 'a,
    {
        guard
    }
}

impl<T: Sync + 'static> sealed::Sealed for Option<&'static T> {}
impl<T: Sync + 'static> ResourceQuery for Option<&'static T> {
    type Guard<'w> = Option<Res<'w, T>>;
    type Item<'a> = Option<&'a T>;
    fn fetch(world: &World) -> Option<Self::Guard<'_>> {
        Some(world.get_resource::<T>())
    }
    fn reborrow<'a, 'w>(guard: &'a mut Self::Guard<'w>) -> Self::Item<'a>
    where
        'w: 'a,
    {
        guard.as_deref()
    }
}

impl<T: Send + 'static> sealed::Sealed for Option<&'static mut T> {}
impl<T: Send + 'static> ResourceQuery for Option<&'static mut T> {
    type Guard<'w> = Option<ResMut<'w, T>>;
    type Item<'a> = Option<&'a mut T>;
    fn fetch(world: &World) -> Option<Self::Guard<'_>> {
        Some(world.get_resource_mut::<T>())
    }
    fn reborrow<'a, 'w>(guard: &'a mut Self::Guard<'w>) -> Self::Item<'a>
    where
        'w: 'a,
    {
        guard.as_deref_mut()
    }
}

macro_rules! tuple {
    ($($ty:ident:$idx:tt),+) => {
        impl<$($ty: ResourceQuery),+> sealed::Sealed for ($($ty,)+) {}
        impl<$($ty: ResourceQuery),+> ResourceQuery for ($($ty,)+) {
            type Guard<'w> = ($($ty::Guard<'w>,)+);
            type Item<'a> = ($($ty::Item<'a>,)+);
            fn fetch(world: &World) -> Option<Self::Guard<'_>> {
                Some(($($ty::fetch(world)?,)+))
            }
            fn reborrow<'a, 'w>(guard: &'a mut Self::Guard<'w>) -> Self::Item<'a>
            where 'w: 'a {
                ($($ty::reborrow(&mut guard.$idx),)+)
            }
        }
    };
}
tuple!(A:0);
tuple!(A:0, B:1);
tuple!(A:0, B:1, C:2);
tuple!(A:0, B:1, C:2, D:3);
tuple!(A:0, B:1, C:2, D:3, E:4);
tuple!(A:0, B:1, C:2, D:3, E:4, F:5);
tuple!(A:0, B:1, C:2, D:3, E:4, F:5, G:6);
tuple!(A:0, B:1, C:2, D:3, E:4, F:5, G:6, H:7);
tuple!(A:0, B:1, C:2, D:3, E:4, F:5, G:6, H:7, I:8);
tuple!(A:0, B:1, C:2, D:3, E:4, F:5, G:6, H:7, I:8, J:9);
tuple!(A:0, B:1, C:2, D:3, E:4, F:5, G:6, H:7, I:8, J:9, K:10);
tuple!(A:0, B:1, C:2, D:3, E:4, F:5, G:6, H:7, I:8, J:9, K:10, L:11);
