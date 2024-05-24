use std::collections::VecDeque;

use arcana::edict::{
    self,
    query::{Not, With},
    relation::FilterRelates,
    Component, Entities, Related, RelatesExclusive, Relation, View,
};

#[derive(Clone, Copy, Debug, Component)]
#[repr(transparent)]
pub struct Global {
    pub iso: Isometry<f32>,
}

impl Global {
    pub fn identity() -> Self {
        Global {
            iso: Isometry::identity(),
        }
    }

    pub fn new(iso: Isometry<f32>) -> Self {
        Global { iso }
    }

    pub fn from_position(position: Point<f32>) -> Self {
        Global {
            iso: Isometry {
                rotation: Rotation::identity(),
                translation: Translation {
                    vector: position.coords,
                },
            },
        }
    }

    pub fn from_position_rotation(position: Point<f32>, rotation: AngVector<f32>) -> Self {
        Global {
            iso: Isometry {
                rotation: Rotation::new(rotation),
                translation: na::Translation {
                    vector: position.coords,
                },
            },
        }
    }

    pub fn translate(&mut self, v: Vector<f32>) -> &mut Self {
        self.iso.translation.vector += v;
        self
    }

    pub fn rotate(&mut self, angle: AngVector<f32>) -> &mut Self {
        self.iso.rotation *= Rotation::new(angle);
        self
    }

    pub fn translated(mut self, v: Vector<f32>) -> Self {
        self.translate(v);
        self
    }

    pub fn rotated(mut self, angle: AngVector<f32>) -> Self {
        self.rotate(angle);
        self
    }
}

#[derive(Clone, Copy, Debug, Relation)]
#[edict(owned, exclusive)]
#[repr(transparent)]
pub struct Local {
    pub iso: Isometry<f32>,
}

impl Local {
    pub fn identity() -> Self {
        Local {
            iso: Isometry::identity(),
        }
    }

    pub fn translate(&mut self, v: Vector<f32>) {
        self.iso.translation.vector += v;
    }

    pub fn rotate(&mut self, angle: AngVector<f32>) {
        self.iso.rotation *= Rotation::new(angle);
    }
}

pub fn scene_system(
    root: View<(Entities, Related<Local>), (Not<FilterRelates<Local>>, With<Global>)>,
    kids: View<(RelatesExclusive<&Local>, Option<Related<Local>>), With<Global>>,
    mut global: View<&mut Global>,
) {
    let mut queue = VecDeque::new();

    for (parent, children) in root {
        let global = *global.get_mut(parent).unwrap();
        queue.push_back((global, children));
    }

    while let Some((parent_global, children)) = queue.pop_front() {
        for &child in children {
            if let Some(((local, _), children)) = kids.get(child) {
                let global = global.get_mut(child).unwrap();
                global.iso = parent_global.iso * local.iso;
                if let Some(children) = children {
                    queue.push_back((*global, children));
                }
            }
        }
    }
}
