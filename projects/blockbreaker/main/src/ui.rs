use crate::{
    game::{Game, Phase},
    scene::{
        InstructionWidget, LivesWidget, ObjectiveWidget, ScoreWidget, StatusWidget, TitleWidget,
    },
};
use arcana::{
    ecs::{entity::EntityId, query::Entities, world::World},
    scene::dim2::Rect as SceneRect,
};
use pixie_ui::{
    color::Color,
    draw::Brush,
    font::{Bitmap, Font, FontId, Metrics},
    margin::Margin,
    math::{Pos, Rect, Size, Vec as PixelVec},
    style::{Anchor, Attributes, WidgetPos, WidgetSize},
    text::Text,
    ui::UiState,
    widget::{RootWidget, Widget},
};
use std::collections::HashMap;

const MAX_FONT_SCALE: i32 = 16;

#[derive(arcana::Component)]
struct BlockbreakerUi;

#[derive(Default)]
struct BlockbreakerFonts(HashMap<i32, FontId>);

#[derive(Clone, Copy)]
enum Kind {
    Title,
    Objective,
    Score,
    Lives,
    Status,
    Instruction,
}

struct Spec {
    entity: EntityId,
    kind: Kind,
    size: f32,
    bounds: SceneRect,
}

#[derive(Clone, Copy)]
struct UiData {
    phase: Phase,
    score: u32,
    lives: u32,
}

pub fn update(world: &mut World) {
    let data = world.get_resource::<Game>().map_or(
        UiData {
            phase: Phase::Ready,
            score: 0,
            lives: 3,
        },
        |game| UiData {
            phase: game.phase,
            score: game.score,
            lives: game.lives,
        },
    );

    let mut specs = Vec::new();
    macro_rules! collect {
        ($ty:ty, $kind:expr) => {
            specs.extend(world.view::<(Entities, &$ty, &SceneRect)>().iter().map(
                |(entity, widget, bounds)| Spec {
                    entity: entity.id(),
                    kind: $kind,
                    size: widget.size,
                    bounds: *bounds,
                },
            ));
        };
    }
    collect!(TitleWidget, Kind::Title);
    collect!(ObjectiveWidget, Kind::Objective);
    collect!(ScoreWidget, Kind::Score);
    collect!(LivesWidget, Kind::Lives);
    collect!(StatusWidget, Kind::Status);
    collect!(InstructionWidget, Kind::Instruction);

    let stale: Vec<_> = world
        .view::<(Entities, &BlockbreakerUi)>()
        .iter()
        .filter_map(|(entity, _)| {
            let entity = entity.id();
            (!specs.iter().any(|spec| spec.entity == entity)).then_some(entity)
        })
        .collect();
    for entity in stale {
        let _ = world.remove::<BlockbreakerUi>(entity);
        let _ = world.remove::<RootWidget>(entity);
        let _ = world.remove::<Widget>(entity);
        let _ = world.remove::<Attributes>(entity);
        let _ = world.remove::<Text>(entity);
    }

    let mut fonts = world
        .remove_resource::<BlockbreakerFonts>()
        .unwrap_or_default();
    let mut font_ids = HashMap::new();
    {
        let ui = world.with_resource(UiState::new);
        for spec in &specs {
            let scale = font_scale(spec.size);
            let name = font_name(scale);
            let font = fonts
                .0
                .get(&scale)
                .copied()
                .filter(|&id| ui.font(id).is_some_and(|font| font.name.as_ref() == name));
            let id = font.unwrap_or_else(|| {
                let id = ui.register_font(scaled_mono5x7(scale));
                fonts.0.insert(scale, id);
                id
            });
            font_ids.insert(scale, id);
        }
    }
    world.insert_resource(fonts);

    for spec in specs {
        let scale = font_scale(spec.size);
        let Some(&font) = font_ids.get(&scale) else {
            continue;
        };
        maintain_widget(world, spec, data, font);
    }
}

pub fn prepare_render(world: &mut World) {
    update(world);
    let roots: Vec<_> = world
        .view::<(Entities, &BlockbreakerUi, &arcana::scene::dim2::Transform)>()
        .iter()
        .map(|(entity, _, transform)| {
            let position = transform.position();
            (
                entity.id(),
                PixelVec {
                    x: pixel_position(position.x),
                    y: pixel_position(position.y),
                },
            )
        })
        .collect();
    for (entity, offset) in roots {
        if let Ok(attributes) = world.get::<&mut Attributes>(entity) {
            attributes.position = Some(WidgetPos::Fixed {
                offset,
                anchor: Anchor::TopLeft,
            });
        }
    }
}

fn maintain_widget(world: &mut World, spec: Spec, data: UiData, font: FontId) {
    let text = match spec.kind {
        Kind::Title => "BLOCK BREAKER".to_owned(),
        Kind::Objective => "CLEAR THE WALL".to_owned(),
        Kind::Score => format!("SCORE {:04}", data.score),
        Kind::Lives => format!("LIVES {}", data.lives),
        Kind::Status => match data.phase {
            Phase::Ready => "READY? SPACE TO LAUNCH".to_owned(),
            Phase::Playing => String::new(),
            Phase::Won => format!("WON {:04} R RESTART", data.score),
            Phase::Lost => format!("LOST {:04} R RESTART", data.score),
        },
        Kind::Instruction => "A D / ARROWS  MOVE     SPACE  LAUNCH     R  RESTART".to_owned(),
    };
    let color = match spec.kind {
        Kind::Title | Kind::Lives => Color::from_unmultiplied(31, 217, 224, 255),
        Kind::Objective | Kind::Instruction => Color::from_unmultiplied(87, 117, 153, 255),
        Kind::Score | Kind::Status => Color::from_unmultiplied(214, 240, 255, 255),
    };
    let mut attributes = world
        .get::<&Attributes>(spec.entity)
        .map_or_else(|_| Attributes::default(), |attributes| *attributes);
    attributes.size = Some(WidgetSize::Fixed(Size {
        w: pixel_length(spec.bounds.max.x - spec.bounds.min.x),
        h: pixel_length(spec.bounds.max.y - spec.bounds.min.y),
    }));
    attributes.inner_margin = Some(Margin::new(
        pixel_margin(spec.bounds.min.y),
        pixel_margin(spec.bounds.min.x),
        0,
        0,
    ));
    attributes.fg_brush = Some(Brush::Solid(color));
    attributes.font = Some(font);

    if world.get::<&BlockbreakerUi>(spec.entity).is_err() {
        let _ = world.insert(spec.entity, BlockbreakerUi);
    }
    if world.get::<&RootWidget>(spec.entity).is_err() {
        let _ = world.insert(spec.entity, RootWidget);
    }
    if world
        .get::<&Widget>(spec.entity)
        .map_or(true, |widget| widget.parent.is_some())
    {
        let _ = world.insert(spec.entity, Widget { parent: None });
    }
    let _ = world.insert(spec.entity, attributes);
    if world
        .get::<&Text>(spec.entity)
        .map_or(true, |current| current.string != text)
    {
        let _ = world.insert(spec.entity, Text::new(text));
    }
}

fn pixel_length(value: f64) -> i32 {
    if value.is_finite() {
        value.round().clamp(0.0, i32::MAX as f64) as i32
    } else {
        0
    }
}

fn pixel_margin(value: f64) -> u8 {
    if value.is_finite() {
        value.round().clamp(0.0, u8::MAX as f64) as u8
    } else {
        0
    }
}

fn pixel_position(value: f64) -> i32 {
    if value.is_finite() {
        value.round().clamp(i32::MIN as f64, i32::MAX as f64) as i32
    } else {
        0
    }
}

fn font_scale(size: f32) -> i32 {
    if size.is_finite() {
        size.round().clamp(1.0, MAX_FONT_SCALE as f32) as i32
    } else {
        1
    }
}

fn font_name(scale: i32) -> String {
    format!("blockbreaker mono5x7 {scale}x")
}

fn scaled_mono5x7(scale: i32) -> Font {
    let source = pixie_ui::font::mono5x7();
    let source_width = source.bitmap.glyph_size.w as usize;
    let source_height = source.bitmap.glyph_size.h as usize;
    let scale_usize = scale as usize;
    let width = source_width * scale_usize;
    let height = source_height * scale_usize;
    let source_stride = (source_width * source_height).next_multiple_of(32) / 8;
    let target_stride = (width * height).next_multiple_of(32) / 8;
    let mut bitmap = vec![0u8; source.glyph_metrics.len() * target_stride];
    for glyph in 0..source.glyph_metrics.len() {
        for x in 0..source_width {
            for y in 0..source_height {
                let source_bit = x * source_height + y;
                if source.bitmap.bitmap[glyph * source_stride + source_bit / 8]
                    & (1 << (source_bit % 8))
                    == 0
                {
                    continue;
                }
                for dx in 0..scale_usize {
                    for dy in 0..scale_usize {
                        let target_bit = (x * scale_usize + dx) * height + y * scale_usize + dy;
                        bitmap[glyph * target_stride + target_bit / 8] |= 1 << (target_bit % 8);
                    }
                }
            }
        }
    }
    let scale_size = |size: Size| Size {
        w: size.w * scale,
        h: size.h * scale,
    };
    let scale_pos = |pos: Pos| Pos {
        x: pos.x * scale,
        y: pos.y * scale,
    };
    let scale_vec = |value: PixelVec| PixelVec {
        x: value.x * scale,
        y: value.y * scale,
    };
    let glyph_metrics: Box<[Metrics]> = source
        .glyph_metrics
        .iter()
        .map(|metrics| Metrics {
            advance: scale_size(metrics.advance),
            offset: scale_vec(metrics.offset),
            bbox: Rect {
                lt: scale_pos(metrics.bbox.lt),
                rb: scale_pos(metrics.bbox.rb),
            },
        })
        .collect();
    Font {
        name: font_name(scale).into(),
        size: scale_size(source.size),
        gap: scale_vec(source.gap),
        glyph_metrics,
        bitmap: Bitmap {
            glyph_size: Size {
                w: width as i32,
                h: height as i32,
            },
            bitmap: bitmap.into_boxed_slice(),
        },
        mapping: source.mapping,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use arcana::{athena::Vector2, scene::dim2::Transform};

    #[test]
    fn font_scale_maps_authored_sizes_to_scaled_bitmaps() {
        for (size, scale) in [(2.0, 2), (3.0, 3), (4.0, 4), (3.6, 4)] {
            assert_eq!(font_scale(size), scale);
            let font = scaled_mono5x7(scale);
            assert_eq!(
                font.size,
                Size {
                    w: 5 * scale,
                    h: 7 * scale
                }
            );
            assert_eq!(
                font.bitmap.glyph_size,
                Size {
                    w: 5 * scale,
                    h: 8 * scale
                }
            );
        }
    }

    #[test]
    fn score_ui_is_created_updated_and_removed_with_authored_component() {
        let mut world = World::new();
        let entity = world
            .spawn((
                ScoreWidget { size: 3.0 },
                Transform::new(Vector2::new(10.0, 20.0), 0.0),
                crate::scene::rect(0.0, 0.0, 140.0, 42.0),
            ))
            .id();
        prepare_render(&mut world);
        assert!(world.get::<&Widget>(entity).is_ok());
        assert!(world.get::<&RootWidget>(entity).is_ok());
        assert_eq!(
            world.get::<&Text>(entity).expect("score text").string,
            "SCORE 0000"
        );
        assert_eq!(
            world
                .get::<&Attributes>(entity)
                .expect("attributes")
                .position,
            Some(WidgetPos::Fixed {
                offset: PixelVec { x: 10, y: 20 },
                anchor: Anchor::TopLeft,
            })
        );
        let font_id = world
            .get::<&Attributes>(entity)
            .expect("attributes")
            .font
            .expect("font");
        assert_eq!(
            world
                .expect_resource::<UiState>()
                .font(font_id)
                .expect("registered font")
                .size,
            Size { w: 15, h: 21 }
        );
        world.insert_resource(Game::new());
        world.expect_resource_mut::<Game>().score = 1234;
        update(&mut world);
        assert_eq!(
            world.get::<&Text>(entity).expect("updated score").string,
            "SCORE 1234"
        );
        world
            .remove::<ScoreWidget>(entity)
            .expect("authored score widget");
        update(&mut world);
        assert!(world.is_alive(entity));
        assert!(world.get::<&BlockbreakerUi>(entity).is_err());
        assert!(world.get::<&Widget>(entity).is_err());
        assert!(world.get::<&RootWidget>(entity).is_err());
        assert!(world.get::<&Attributes>(entity).is_err());
        assert!(world.get::<&Text>(entity).is_err());
        assert!(world.get::<&SceneRect>(entity).is_ok());
    }
}
