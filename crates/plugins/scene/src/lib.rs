use std::collections::VecDeque;

use arcana::{
    edict::{
        self,
        query::{Not, With},
        relation::FilterRelates,
        Component, Entities, Related, RelatesExclusive, Relation, Scheduler, View, World,
    },
    plugin::ArcanaPlugin,
};

arcana::export_arcana_plugin!(ScenePlugin);

pub struct ScenePlugin;

impl ArcanaPlugin for ScenePlugin {
    fn name(&self) -> &'static str {
        "scene"
    }

    fn init(&self, _world: &mut World, scheduler: &mut Scheduler) {
        scheduler.add_system(scene_system2);
        scheduler.add_system(scene_system3);
    }
}

#[derive(Clone, Copy, Debug, Component)]
#[repr(transparent)]
pub struct Global2 {
    pub iso: na::Isometry2<f32>,
}

impl Global2 {
    pub fn identity() -> Self {
        Global2 {
            iso: na::Isometry2::identity(),
        }
    }

    pub fn new(iso: na::Isometry2<f32>) -> Self {
        Global2 { iso }
    }

    pub fn from_position(position: na::Point2<f32>) -> Self {
        Global2 {
            iso: na::Isometry2 {
                rotation: na::UnitComplex::identity(),
                translation: na::Translation2 {
                    vector: position.coords,
                },
            },
        }
    }

    pub fn from_position_rotation(position: na::Point2<f32>, rotation: f32) -> Self {
        Global2 {
            iso: na::Isometry2 {
                rotation: na::UnitComplex::new(rotation),
                translation: na::Translation2 {
                    vector: position.coords,
                },
            },
        }
    }

    pub fn translate(&mut self, v: na::Vector2<f32>) -> &mut Self {
        self.iso.translation.vector += v;
        self
    }

    pub fn rotate(&mut self, angle: f32) -> &mut Self {
        self.iso.rotation *= na::UnitComplex::new(angle);
        self
    }

    pub fn translated(mut self, v: na::Vector2<f32>) -> Self {
        self.translate(v);
        self
    }

    pub fn rotated(mut self, angle: f32) -> Self {
        self.rotate(angle);
        self
    }
}

#[derive(Clone, Copy, Debug, Component)]
#[repr(transparent)]
pub struct Global3 {
    pub iso: na::Isometry3<f32>,
}

impl Global3 {
    pub fn identity() -> Self {
        Global3 {
            iso: na::Isometry3::identity(),
        }
    }

    pub fn translate(&mut self, v: na::Vector3<f32>) {
        self.iso.translation.vector += v;
    }

    pub fn rotate(&mut self, q: na::UnitQuaternion<f32>) {
        self.iso.rotation *= q;
    }
}

#[derive(Clone, Copy, Debug, Relation)]
#[edict(owned, exclusive)]
#[repr(transparent)]
pub struct Local2 {
    pub iso: na::Isometry2<f32>,
}

impl Local2 {
    pub fn identity() -> Self {
        Local2 {
            iso: na::Isometry2::identity(),
        }
    }

    pub fn translate(&mut self, v: na::Vector2<f32>) {
        self.iso.translation.vector += v;
    }

    pub fn rotate(&mut self, angle: f32) {
        self.iso.rotation *= na::UnitComplex::new(angle);
    }
}

#[derive(Clone, Copy, Debug, Relation)]
#[edict(owned, exclusive)]
#[repr(transparent)]
pub struct Local3 {
    pub iso: na::Isometry3<f32>,
}

impl Local3 {
    pub fn identity() -> Self {
        Local3 {
            iso: na::Isometry3::identity(),
        }
    }

    pub fn translate(&mut self, v: na::Vector3<f32>) {
        self.iso.translation.vector += v;
    }

    pub fn rotate(&mut self, q: na::UnitQuaternion<f32>) {
        self.iso.rotation *= q;
    }
}

fn scene_system2(
    root: View<(Entities, Related<Local2>), (Not<FilterRelates<Local2>>, With<Global2>)>,
    kid: View<(RelatesExclusive<&Local2>, Option<Related<Local2>>), With<Global2>>,
    mut global: View<&mut Global2>,
) {
    let mut queue = VecDeque::new();

    for (parent, children) in root {
        let global = *global.get_mut(parent).unwrap();
        queue.push_back((global, children));
    }

    while let Some((parent_global, children)) = queue.pop_front() {
        for &child in children {
            if let Some(((local, _), children)) = kid.get(child) {
                let global = global.get_mut(child).unwrap();
                global.iso = parent_global.iso * local.iso;
                if let Some(children) = children {
                    queue.push_back((*global, children));
                }
            }
        }
    }
}

fn scene_system3(
    root: View<(Entities, Related<Local3>), (Not<FilterRelates<Local3>>, With<Global3>)>,
    kid: View<(RelatesExclusive<&Local3>, Option<Related<Local3>>), With<Global3>>,
    mut global: View<&mut Global3>,
) {
    let mut queue = VecDeque::new();

    for (parent, children) in root {
        let global = *global.get_mut(parent).unwrap();
        queue.push_back((global, children));
    }

    while let Some((parent_global, children)) = queue.pop_front() {
        for &child in children {
            if let Some(((local, _), children)) = kid.get(child) {
                let global = global.get_mut(child).unwrap();
                global.iso = parent_global.iso * local.iso;
                if let Some(children) = children {
                    queue.push_back((*global, children));
                }
            }
        }
    }
}
