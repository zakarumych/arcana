use arcana::{
    ecs::{entity::EntityId, query::Entities, relation::ChildOf, world::World},
    error::Error,
    plugin::{PluginsHub, SceneComponents},
    scene,
};
use egui_ltreeview::{Action, TreeView, TreeViewBuilder, TreeViewState};

pub(crate) fn reparent(
    world: &mut World,
    entity: EntityId,
    target: Option<EntityId>,
) -> Result<(), Error> {
    world.get::<&SceneComponents>(entity).map_err(Error::wrap)?;
    let mut ancestor = target;
    let mut seen = arcana::hash::HashSet::default();
    while let Some(parent) = ancestor {
        if parent == entity || !seen.insert(parent) {
            return Err(Error::msg("Cannot create a hierarchy cycle".into()));
        }
        world.get::<&SceneComponents>(parent).map_err(Error::wrap)?;
        ancestor = scene::parent(world, parent);
    }
    if let Some(target) = target {
        if world
            .view::<&scene::dim2::LocalTransform>()
            .try_get(entity)
            .is_ok()
            && world
                .view::<&scene::dim2::Transform>()
                .try_get(target)
                .is_err()
            || world
                .view::<&scene::dim3::LocalTransform>()
                .try_get(entity)
                .is_ok()
                && world
                    .view::<&scene::dim3::Transform>()
                    .try_get(target)
                    .is_err()
        {
            return Err(Error::msg("Parent needs a matching Transform".into()));
        }
        world
            .insert_relation(entity, ChildOf, target)
            .map_err(Error::wrap)?;
    } else if let Some(parent) = scene::parent(world, entity) {
        world
            .remove_relation::<ChildOf>(entity, parent)
            .map_err(Error::wrap)?;
    }
    Ok(())
}

pub(crate) fn show(
    world: &mut World,
    hub: &PluginsHub,
    state: &mut TreeViewState<EntityId>,
    selected: &mut Option<EntityId>,
    ui: &mut egui::Ui,
) -> Result<(), Error> {
    let mut nodes: Vec<_> = world
        .view::<(Entities, &SceneComponents)>()
        .iter()
        .map(|(e, marker)| {
            let id = e.id();
            let label = world
                .view::<&scene::Name>()
                .try_get(id)
                .ok()
                .map(|name| name.0.to_string())
                .unwrap_or_else(|| {
                    let mut names: Vec<_> = marker
                        .components
                        .iter()
                        .filter_map(|id| hub.components.get(id).map(|d| d.name.as_str()))
                        .collect();
                    names.sort_unstable();
                    if names.is_empty() {
                        format!("Entity {id}")
                    } else {
                        format!("{} ({id})", names.join(", "))
                    }
                });
            (id, scene::parent(world, id), label)
        })
        .collect();
    nodes.sort_unstable_by_key(|n| n.0);
    state.set_selected(selected.iter().copied().collect());
    let mut result = Ok(());
    ui.weak("Drag to reparent (keeps local pose).");
    egui::ScrollArea::vertical()
        .id_salt("scene-entities")
        .max_height(180.0)
        .show(ui, |ui| {
            fn children(
                builder: &mut TreeViewBuilder<'_, EntityId>,
                nodes: &[(EntityId, Option<EntityId>, String)],
                parent: Option<EntityId>,
                visited: &mut arcana::hash::HashSet<EntityId>,
            ) {
                for (id, _, label) in nodes.iter().filter(|n| n.1 == parent) {
                    if !visited.insert(*id) {
                        continue;
                    }
                    builder.dir(*id, label);
                    children(builder, nodes, Some(*id), visited);
                    builder.close_dir();
                }
            }
            let (_, actions) = TreeView::new(ui.make_persistent_id("scene-tree"))
                .allow_multi_selection(false)
                .show_state(ui, state, |builder| {
                    children(builder, &nodes, None, &mut Default::default());
                });
            for action in actions {
                match action {
                    Action::SetSelected(ids) => *selected = ids.first().copied(),
                    Action::Move(moved) => {
                        for id in moved.source {
                            if let Err(error) = reparent(world, id, Some(moved.target)) {
                                result = Err(error);
                            }
                        }
                    }
                    _ => {}
                }
            }
        });
    if let Some(id) = *selected {
        if scene::parent(world, id).is_some()
            && ui
                .small_button("Move to scene root")
                .on_hover_text("Keeps the local pose; world pose follows the new parent.")
                .clicked()
        {
            result = reparent(world, id, None);
        }
    }
    result
}
