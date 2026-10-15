use arcana::{
    athena::Vector2,
    ecs::{entity::EntityId, query::Entities, world::World},
    scene::dim2::{self, Transform},
};
use pixie_ui::{
    math::Vec as PixelVec,
    style::{Anchor, Attributes, WidgetPos},
    widget::{RootWidget, Widget},
};
use std::collections::HashMap;
#[derive(Default)]
struct Baselines(HashMap<EntityId, (PixelVec, Vector2<f64>, f64)>);
/// Synchronizes root fixed offsets and world translations, adding concurrent edits.
/// First observation takes Arcana's position. Anchors and rotation are preserved;
/// fractional Arcana coordinates remain intact while Pixie offsets are rounded.
#[arcana::system]
pub fn sync_root_transforms(world: &mut World) {
    pixie_ui::widget::sync_widget_parents(world);
    let mut baselines = world.remove_resource::<Baselines>().unwrap_or_default();
    let roots: Vec<_> = world
        .view::<(Entities, &Widget, &Attributes, &Transform)>()
        .with::<RootWidget>()
        .iter()
        .filter(|(_, widget, _, _)| widget.parent.is_none())
        .map(|(entity, _, attributes, transform)| (entity.id(), *attributes, *transform))
        .collect();
    let mut next = HashMap::with_capacity(roots.len());
    for (entity, mut attributes, transform) in roots {
        let position = transform.position();
        let (offset, anchor) = match attributes.position {
            Some(WidgetPos::Fixed { offset, anchor }) => (Some(offset), anchor),
            _ => (None, Anchor::TopLeft),
        };
        let position = match (offset, baselines.0.get(&entity)) {
            (Some(offset), Some(last)) => Vector2::new(
                position.x + f64::from(offset.x) - f64::from(last.0.x),
                position.y + f64::from(offset.y) - f64::from(last.0.y),
            ),
            _ => position,
        };
        if !position.x.is_finite() || !position.y.is_finite() {
            continue;
        }
        let written = PixelVec {
            x: position.x.round() as i32,
            y: position.y.round() as i32,
        };
        let arcana_changed = baselines
            .0
            .get(&entity)
            .is_none_or(|last| last.1 != transform.position() || last.2 != transform.angle());
        if (position != transform.position() || arcana_changed)
            && dim2::set_world_transform(world, entity, Transform::new(position, transform.angle()))
                .is_err()
        {
            if let Some(last) = baselines.0.remove(&entity) {
                next.insert(entity, last);
            }
            continue;
        }
        let fixed = Some(WidgetPos::Fixed {
            offset: written,
            anchor,
        });
        if attributes.position != fixed {
            attributes.position = fixed;
            if world.insert(entity, attributes).is_err() {
                continue;
            }
        }
        let actual = world
            .get::<&Transform>(entity)
            .expect("Root transform remains present");
        next.insert(entity, (written, actual.position(), actual.angle()));
    }
    world.insert_resource(Baselines(next));
}

#[cfg(test)]
mod tests {
    use super::*;
    use arcana::scene::dim2::LocalTransform;
    fn root(world: &mut World, x: f64) -> EntityId {
        world
            .spawn((
                RootWidget,
                Widget { parent: None },
                Attributes::default(),
                Transform::new(Vector2::new(x, 0.0), 0.4),
            ))
            .id()
    }
    fn pixie(world: &mut World, entity: EntityId, x: i32) {
        world
            .get::<&mut Attributes>(entity)
            .expect("attributes")
            .position = Some(WidgetPos::Fixed {
            offset: PixelVec { x, y: 0 },
            anchor: Anchor::Center,
        });
    }
    fn x(world: &mut World, e: EntityId) -> f64 {
        world.get::<&Transform>(e).expect("transform").position().x
    }
    #[test]
    fn merges_edits_preserves_fraction_rotation_and_anchor() {
        let mut world = World::new();
        let e = root(&mut world, 10.25);
        sync_root_transforms(&mut world);
        pixie(&mut world, e, 12);
        sync_root_transforms(&mut world);
        assert!((x(&mut world, e) - 12.25).abs() < 1e-9);
        world
            .insert(e, Transform::new(Vector2::new(15.75, 0.0), 0.7))
            .expect("entity");
        pixie(&mut world, e, 14);
        sync_root_transforms(&mut world);
        assert!((x(&mut world, e) - 17.75).abs() < 1e-9);
        assert!((world.get::<&Transform>(e).expect("transform").angle() - 0.7).abs() < 1e-9);
        assert_eq!(
            world.get::<&Attributes>(e).expect("attributes").position,
            Some(WidgetPos::Fixed {
                offset: PixelVec { x: 18, y: 0 },
                anchor: Anchor::Center
            })
        );
        sync_root_transforms(&mut world);
        assert!((x(&mut world, e) - 17.75).abs() < 1e-9);
        world
            .insert(e, Transform::new(Vector2::new(20.25, 0.0), 0.7))
            .expect("entity");
        sync_root_transforms(&mut world);
        assert!((x(&mut world, e) - 20.25).abs() < 1e-9);
    }
    #[test]
    fn direct_arcana_edits_update_local_transform() {
        let mut world = World::new();
        let e = root(&mut world, 3.0);
        world.insert(e, LocalTransform::default()).expect("entity");
        sync_root_transforms(&mut world);
        world
            .insert(e, Transform::new(Vector2::new(8.0, 0.0), 0.8))
            .expect("entity");
        sync_root_transforms(&mut world);
        arcana::scene::update_transforms(&mut world).expect("valid hierarchy");
        assert!((x(&mut world, e) - 8.0).abs() < 1e-9);
        pixie(&mut world, e, 11);
        sync_root_transforms(&mut world);
        arcana::scene::update_transforms(&mut world).expect("valid hierarchy");
        assert!((x(&mut world, e) - 11.0).abs() < 1e-9);
    }
    #[test]
    fn excludes_children_and_prunes_removed_components() {
        let mut world = World::new();
        let e = root(&mut world, 4.0);
        sync_root_transforms(&mut world);
        world.remove::<RootWidget>(e).expect("entity");
        pixie(&mut world, e, 20);
        sync_root_transforms(&mut world);
        assert_eq!(
            world
                .get_resource::<Baselines>()
                .expect("baselines")
                .0
                .len(),
            0
        );
        assert!((x(&mut world, e) - 4.0).abs() < 1e-9);
        world.insert(e, RootWidget).expect("entity");
        sync_root_transforms(&mut world);
        assert!((x(&mut world, e) - 4.0).abs() < 1e-9);
        let parent = world
            .spawn((pixie_ui::widget::Container { children: vec![e] },))
            .id();
        sync_root_transforms(&mut world);
        assert_eq!(
            world.get::<&Widget>(e).expect("widget").parent,
            Some(parent)
        );
        assert_eq!(
            world
                .get_resource::<Baselines>()
                .expect("baselines")
                .0
                .len(),
            0
        );
    }
}
