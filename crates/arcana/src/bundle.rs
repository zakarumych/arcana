use edict::{query::QueryItem, system::QueryArg, ActionEncoder, Component, QueryRef};

pub trait Bundle: Component + Sync {
    /// Query type for the bundle.
    /// Typical bundle would use query that is matched by any entity.
    /// Otherwise bundle won't unfold for entities that don't match the query.
    type Query: QueryArg;

    // Unfold bundle into the query item.
    // Insert components and spawn new entities with `actions`.
    fn unfold(&self, item: QueryItem<Self::Query>, actions: ActionEncoder);
}

/// System that unfolds bundles.
pub fn bundle_system<B>(mut query: QueryRef<(&B, B::Query)>, mut actions: ActionEncoder)
where
    B: Bundle,
{
    query.for_each(|(bundle, item)| {
        bundle.unfold(item, actions.reborrow());
    })
}

#[test]
fn bundle_system_is_system() {
    use edict::system::IntoSystem;

    #[derive(edict::Component)]
    struct Foo;

    impl Bundle for Foo {
        type Query = ();

        fn unfold(&self, _item: QueryItem<()>, _actions: ActionEncoder) {}
    }

    fn assert_system<T: edict::system::System>(_: T) {}
    assert_system(bundle_system::<Foo>.into_system());
}
