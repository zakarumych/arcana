use arcana::{
    athena::{Matrix4, Vector2, Vector3},
    ecs::{entity::EntityId, query::Entities, world::World},
    error::Error,
    id::HasStid,
    plugin::SceneComponents,
    scene::{dim2, dim3},
};
use transform_gizmo_egui::{
    Gizmo, GizmoConfig, GizmoInteraction, GizmoMode,
    math::{DMat4, DQuat, DVec3, Transform},
};

#[derive(Default)]
pub(crate) struct SceneViewport {
    gizmo: Gizmo,
    drag: Option<(EntityId, Pose)>,
    cancelled: bool,
}

#[derive(Clone, Copy)]
enum Pose {
    Two(dim2::Transform),
    Three(dim3::Transform),
}
impl Pose {
    fn gizmo(self) -> Transform {
        let matrix = match self {
            Self::Two(t) => matrix(t.matrix()),
            Self::Three(t) => matrix(t.matrix()),
        };
        let (_, rotation, translation) = matrix.to_scale_rotation_translation();
        Transform::from_scale_rotation_translation(DVec3::ONE, rotation, translation)
    }
    fn apply(self, world: &mut World, entity: EntityId) -> Result<(), Error> {
        match self {
            Self::Two(t) => dim2::set_world_transform(world, entity, t),
            Self::Three(t) => dim3::set_world_transform(world, entity, t),
        }
        .map_err(Error::wrap)
    }
}

fn matrix(value: Matrix4<f64>) -> DMat4 {
    DMat4::from_cols_array_2d(value.arrays())
}
fn pose(world: &World, entity: EntityId, two: bool) -> Option<Pose> {
    let marker = world.view::<&SceneComponents>();
    let marker = marker.try_get(entity).ok()?;
    if two {
        if !marker.components.contains(&dim2::LocalTransform::stid())
            && !marker.components.contains(&dim2::Transform::stid())
        {
            return None;
        }
        world
            .view::<&dim2::Transform>()
            .try_get(entity)
            .copied()
            .ok()
            .map(Pose::Two)
    } else {
        if !marker.components.contains(&dim3::LocalTransform::stid())
            && !marker.components.contains(&dim3::Transform::stid())
        {
            return None;
        }
        world
            .view::<&dim3::Transform>()
            .try_get(entity)
            .copied()
            .ok()
            .map(Pose::Three)
    }
}

fn ray(vp: DMat4, rect: egui::Rect, pointer: egui::Pos2) -> Option<(DVec3, DVec3)> {
    if rect.width() <= 0.0 || rect.height() <= 0.0 {
        return None;
    }
    let inverse = vp.inverse();
    if !inverse.is_finite() {
        return None;
    }
    let x = 2.0 * f64::from((pointer.x - rect.left()) / rect.width()) - 1.0;
    let y = 1.0 - 2.0 * f64::from((pointer.y - rect.top()) / rect.height());
    let near = inverse.project_point3(DVec3::new(x, y, 0.0));
    let far = inverse.project_point3(DVec3::new(x, y, 1.0));
    Some((near, far - near))
}

fn slab(origin: DVec3, direction: DVec3, min: DVec3, max: DVec3) -> Option<f64> {
    if min.cmpgt(max).any() {
        return None;
    }
    let mut enter = 0.0f64;
    let mut exit = 1.0f64;
    for axis in 0..3 {
        if direction[axis].abs() < 1e-12 {
            if origin[axis] < min[axis] || origin[axis] > max[axis] {
                return None;
            }
        } else {
            let a = (min[axis] - origin[axis]) / direction[axis];
            let b = (max[axis] - origin[axis]) / direction[axis];
            enter = enter.max(a.min(b));
            exit = exit.min(a.max(b));
            if enter > exit {
                return None;
            }
        }
    }
    Some(enter)
}

fn pick(
    world: &World,
    two: bool,
    origin: DVec3,
    direction: DVec3,
    selected: Option<EntityId>,
) -> Option<EntityId> {
    let mut hits = Vec::new();
    if two {
        if direction.z.abs() < 1e-12 {
            return None;
        }
        let distance = -origin.z / direction.z;
        if !(0.0..=1.0).contains(&distance) {
            return None;
        }
        let point = origin + direction * distance;
        for (entity, _, transform, bounds) in world
            .view::<(Entities, &SceneComponents, &dim2::Transform, &dim2::Rect)>()
            .iter()
        {
            if bounds.contains(
                transform
                    .inverse()
                    .transform_point(Vector2::new(point.x, point.y)),
            ) {
                hits.push((distance, entity.id()));
            }
        }
    } else {
        for (entity, _, transform, bounds) in world
            .view::<(Entities, &SceneComponents, &dim3::Transform, &dim3::Cuboid)>()
            .iter()
        {
            let inverse = matrix(transform.matrix()).inverse();
            if let Some(distance) = slab(
                inverse.transform_point3(origin),
                inverse.transform_vector3(direction),
                DVec3::new(bounds.min.x, bounds.min.y, bounds.min.z),
                DVec3::new(bounds.max.x, bounds.max.y, bounds.max.z),
            ) {
                hits.push((distance, entity.id()));
            }
        }
    }
    hits.sort_unstable_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
    if two {
        if let Some(index) = hits.iter().position(|h| Some(h.1) == selected) {
            return Some(hits[(index + 1) % hits.len()].1);
        }
    }
    hits.first().map(|h| h.1)
}

fn draw_bounds(
    world: &World,
    entity: EntityId,
    two: bool,
    vp: DMat4,
    rect: egui::Rect,
    ui: &egui::Ui,
) {
    let (model, min, max) = if two {
        let transforms = world.view::<&dim2::Transform>();
        let bounds = world.view::<&dim2::Rect>();
        let (Ok(t), Ok(b)) = (transforms.try_get(entity), bounds.try_get(entity)) else {
            return;
        };
        (
            matrix(t.matrix()),
            DVec3::new(b.min.x, b.min.y, 0.0),
            DVec3::new(b.max.x, b.max.y, 0.0),
        )
    } else {
        let transforms = world.view::<&dim3::Transform>();
        let bounds = world.view::<&dim3::Cuboid>();
        let (Ok(t), Ok(b)) = (transforms.try_get(entity), bounds.try_get(entity)) else {
            return;
        };
        (
            matrix(t.matrix()),
            DVec3::new(b.min.x, b.min.y, b.min.z),
            DVec3::new(b.max.x, b.max.y, b.max.z),
        )
    };
    let points: Vec<_> = (0..8)
        .map(|i| {
            let p = vp
                * model
                * DVec3::new(
                    if i & 1 == 0 { min.x } else { max.x },
                    if i & 2 == 0 { min.y } else { max.y },
                    if i & 4 == 0 { min.z } else { max.z },
                )
                .extend(1.0);
            if p.w <= 0.0 {
                return None;
            }
            let p = p.truncate() / p.w;
            Some(egui::pos2(
                rect.left() + (p.x as f32 + 1.0) * rect.width() * 0.5,
                rect.top() + (1.0 - p.y as f32) * rect.height() * 0.5,
            ))
        })
        .collect();
    let painter = ui.painter().with_clip_rect(rect);
    for i in 0..if two { 4 } else { 8 } {
        for bit in [1, 2, 4] {
            let j = i ^ bit;
            if j > i && j < points.len() && (!two || bit != 4) {
                if let (Some(a), Some(b)) = (points[i], points[j]) {
                    painter.line_segment([a, b], egui::Stroke::new(1.5, egui::Color32::YELLOW));
                }
            }
        }
    }
}

impl SceneViewport {
    pub(crate) fn show(
        &mut self,
        world: &mut World,
        renderer: EntityId,
        selected: &mut Option<EntityId>,
        response: &egui::Response,
        extent: arcana::mev::Extent2,
        ui: &mut egui::Ui,
    ) -> Result<(), Error> {
        let rect = response.rect;
        if extent.height() == 0 {
            return Ok(());
        }
        let aspect = f64::from(extent.width()) / f64::from(extent.height());
        let (camera, two) = if let Some(camera) = dim2::camera_matrices(world, renderer, aspect) {
            (camera, true)
        } else if let Some(camera) = dim3::camera_matrices(world, renderer, aspect) {
            (camera, false)
        } else {
            return Ok(());
        };
        let view = matrix(camera.view);
        let projection = matrix(camera.projection);
        let vp = projection * view;
        let down = ui.input(|i| i.pointer.primary_down());
        if !down {
            self.cancelled = false;
        }
        if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
            if let Some((entity, pose)) = self.drag.take() {
                pose.apply(world, entity)?;
                self.gizmo = Gizmo::default();
                self.cancelled = true;
            }
        }
        let mut consumed = self.drag.is_some() || self.cancelled;
        if let Some(entity) = *selected {
            draw_bounds(world, entity, two, vp, rect, ui);
            if let Some(current) = pose(world, entity, two) {
                self.gizmo.update_config(GizmoConfig {
                    view_matrix: view.into(),
                    projection_matrix: projection.into(),
                    viewport: rect,
                    pixels_per_point: ui.ctx().pixels_per_point(),
                    modes: if two {
                        GizmoMode::TranslateX
                            | GizmoMode::TranslateY
                            | GizmoMode::TranslateXY
                            | GizmoMode::RotateZ
                    } else {
                        GizmoMode::all_translate() | GizmoMode::all_rotate()
                    },
                    ..Default::default()
                });
                if !self.cancelled {
                    let pointer = ui.input(|i| i.pointer.hover_pos()).unwrap_or_default();
                    let result = self.gizmo.update(
                        GizmoInteraction {
                            cursor_pos: (pointer.x, pointer.y),
                            hovered: response.hovered(),
                            drag_started: response.hovered()
                                && ui.input(|i| i.pointer.primary_pressed()),
                            dragging: down,
                        },
                        &[current.gizmo()],
                    );
                    let draw = self.gizmo.draw();
                    ui.painter().with_clip_rect(rect).add(egui::Mesh {
                        indices: draw.indices,
                        vertices: draw
                            .vertices
                            .into_iter()
                            .zip(draw.colors)
                            .map(|(pos, [r, g, b, a])| egui::epaint::Vertex {
                                pos: pos.into(),
                                uv: egui::Pos2::ZERO,
                                color: egui::Rgba::from_rgba_premultiplied(r, g, b, a).into(),
                            })
                            .collect(),
                        ..Default::default()
                    });
                    if let Some((_, transforms)) = result {
                        if self.drag.is_none() {
                            self.drag = Some((entity, current));
                        }
                        consumed = true;
                        if let Some(t) = transforms.first() {
                            let p = DVec3::from(t.translation);
                            let q = DQuat::from(t.rotation);
                            let updated = if two {
                                let x = q * DVec3::X;
                                Pose::Two(dim2::Transform::new(
                                    Vector2::new(p.x, p.y),
                                    x.y.atan2(x.x),
                                ))
                            } else {
                                let r = q.to_scaled_axis();
                                Pose::Three(dim3::Transform::new(
                                    Vector3::new(p.x, p.y, p.z),
                                    Vector3::new(r.x, r.y, r.z),
                                ))
                            };
                            updated.apply(world, entity)?;
                        }
                    }
                    consumed |= self.gizmo.is_focused();
                }
            }
        }
        if !down {
            self.drag = None;
        }
        if !consumed && response.hovered() && ui.input(|i| i.pointer.primary_clicked()) {
            if let Some(pointer) = ui
                .input(|i| i.pointer.interact_pos())
                .filter(|p| rect.contains(*p))
            {
                if let Some((origin, direction)) = ray(vp, rect, pointer) {
                    *selected = pick(world, two, origin, direction, *selected);
                    self.gizmo = Gizmo::default();
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selected_gizmo_allows_inspector_clicks_axis_drag_and_other_selection() {
        let mut world = World::new();
        let camera = world
            .spawn_one(dim2::Camera {
                center: Vector2::new(50.0, 50.0),
                size: Vector2::new(100.0, 100.0),
            })
            .id();
        let first = world
            .spawn((
                SceneComponents::default().with::<dim2::LocalTransform>(),
                dim2::LocalTransform::new(Vector2::new(50.0, 50.0), 0.0),
                dim2::Rect::default(),
            ))
            .id();
        let second = world
            .spawn((
                SceneComponents::default().with::<dim2::LocalTransform>(),
                dim2::LocalTransform::new(Vector2::new(20.0, 80.0), 0.0),
                dim2::Rect {
                    min: Vector2::new(-5.0, -5.0),
                    max: Vector2::new(5.0, 5.0),
                },
            ))
            .id();
        arcana::scene::update_transforms(&mut world).expect("transforms");
        let ctx = egui::Context::default();
        let mut viewport = SceneViewport::default();
        let mut selected = Some(first);
        let mut clicked = false;
        let button = egui::pos2(50.0, 30.0);
        let handle = egui::pos2(490.0, 150.0);
        let moved = egui::pos2(520.0, 150.0);
        let other = egui::pos2(360.0, 240.0);
        let steps = [
            (button, None),
            (button, None),
            (button, Some(true)),
            (button, Some(false)),
            (handle, None),
            (handle, None),
            (handle, Some(true)),
            (moved, None),
            (moved, Some(false)),
            (other, None),
            (other, None),
            (other, Some(true)),
            (other, Some(false)),
        ];
        for (pointer, pressed) in steps {
            let mut input = egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(700.0, 400.0),
                )),
                ..Default::default()
            };
            input.events.push(egui::Event::PointerMoved(pointer));
            if let Some(pressed) = pressed {
                input.events.push(egui::Event::PointerButton {
                    pos: pointer,
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: Default::default(),
                });
            }
            let mut output = ctx.run_ui(input, |ui| {
                let inspector = ui.put(
                    egui::Rect::from_min_size(egui::pos2(10.0, 10.0), egui::vec2(100.0, 40.0)),
                    egui::Button::new("Inspector"),
                );
                clicked |= inspector.clicked();
                let rect =
                    egui::Rect::from_min_size(egui::pos2(300.0, 0.0), egui::vec2(300.0, 300.0));
                let response = ui.interact(rect, ui.id().with("image"), egui::Sense::click());
                viewport
                    .show(
                        &mut world,
                        camera,
                        &mut selected,
                        &response,
                        arcana::mev::Extent2::new(300, 300),
                        ui,
                    )
                    .expect("viewport");
            });
            output.textures_delta.clear();
        }
        assert!(clicked, "selected gizmo must not capture inspector input");
        assert!(
            world
                .get::<&dim2::LocalTransform>(first)
                .expect("authored transform")
                .position()
                .x
                > 55.0,
            "axis drag must edit authored transform"
        );
        assert_eq!(
            selected,
            Some(second),
            "gizmo must allow clicking other objects"
        );
    }

    #[test]
    fn picks_rotated_offset_ranges_and_cycles_overlap() {
        let mut world = World::new();
        let transform = dim2::Transform::new(Vector2::new(20.0, 30.0), std::f64::consts::FRAC_PI_2);
        let bounds = dim2::Rect {
            min: Vector2::new(2.0, 3.0),
            max: Vector2::new(8.0, 9.0),
        };
        let first = world
            .spawn((SceneComponents::default(), transform, bounds))
            .id();
        let second = world
            .spawn((SceneComponents::default(), transform, bounds))
            .id();
        let p = transform.transform_point(Vector2::new(5.0, 6.0));
        let origin = DVec3::new(p.x, p.y, -1.0);
        let direction = DVec3::new(0.0, 0.0, 2.0);
        assert_eq!(
            pick(&world, true, origin, direction, None),
            Some(first.min(second))
        );
        assert_eq!(
            pick(&world, true, origin, direction, Some(first)),
            Some(second)
        );
        assert_eq!(
            pick(&world, true, DVec3::new(20.0, 30.0, -1.0), direction, None),
            None
        );
    }

    #[test]
    fn cuboid_slab_handles_parallel_rays_and_rejects_invalid_ranges() {
        let min = DVec3::new(2.0, 3.0, 4.0);
        let max = DVec3::new(5.0, 6.0, 7.0);
        assert_eq!(
            slab(
                DVec3::new(3.0, 4.0, 0.0),
                DVec3::new(0.0, 0.0, 10.0),
                min,
                max
            ),
            Some(0.4)
        );
        assert_eq!(slab(DVec3::ZERO, DVec3::Z * 10.0, min, max), None);
        assert_eq!(slab(DVec3::ZERO, DVec3::Z * 10.0, max, min), None);
    }

    #[test]
    fn image_rect_and_physical_aspect_define_pointer_projection() {
        let camera = dim2::Camera {
            center: Vector2::new(100.0, 50.0),
            size: Vector2::new(200.0, 100.0),
        };
        let matrices = camera
            .matrices(dim2::Transform::default(), 2.0)
            .expect("valid camera");
        let rect = egui::Rect::from_min_size(egui::pos2(30.0, 80.0), egui::vec2(400.0, 200.0));
        let (origin, direction) = ray(matrix(matrices.view_projection()), rect, rect.center())
            .expect("invertible projection");
        let point = origin + direction * (-origin.z / direction.z);
        assert!((point - DVec3::new(100.0, 50.0, 0.0)).length() < 1e-9);
    }

    #[test]
    fn egui_pointer_selects_visible_entity() {
        let mut world = World::new();
        let camera = world
            .spawn_one(dim2::Camera {
                center: Vector2::new(50.0, 50.0),
                size: Vector2::new(100.0, 100.0),
            })
            .id();
        let object = world
            .spawn((
                SceneComponents::default(),
                dim2::Transform::new(Vector2::new(50.0, 50.0), 0.0),
                dim2::Rect {
                    min: Vector2::new(-10.0, -10.0),
                    max: Vector2::new(10.0, 10.0),
                },
            ))
            .id();
        let ctx = egui::Context::default();
        let mut viewport = SceneViewport::default();
        let mut selected = None;
        let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(500.0, 400.0));
        let mut center = egui::Pos2::ZERO;
        for frame in 0..4 {
            let mut input = egui::RawInput {
                screen_rect: Some(screen),
                ..Default::default()
            };
            if frame > 0 {
                input.events.push(egui::Event::PointerMoved(center));
            }
            if frame == 2 || frame == 3 {
                input.events.push(egui::Event::PointerButton {
                    pos: center,
                    button: egui::PointerButton::Primary,
                    pressed: frame == 2,
                    modifiers: Default::default(),
                });
            }
            let mut output = ctx.run_ui(input, |ui| {
                let (_, response) =
                    ui.allocate_exact_size(egui::vec2(200.0, 200.0), egui::Sense::click());
                center = response.rect.center();
                viewport
                    .show(
                        &mut world,
                        camera,
                        &mut selected,
                        &response,
                        arcana::mev::Extent2::new(400, 400),
                        ui,
                    )
                    .expect("valid scene");
            });
            output.textures_delta.clear();
        }
        assert_eq!(selected, Some(object));
    }
}
