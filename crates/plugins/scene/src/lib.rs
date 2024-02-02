#[cfg(feature = "dim2")]
pub mod dim2 {
    use na::{
        Isometry2 as Isometry, Point2 as Point, Translation2 as Translation, Vector2 as Vector,
    };

    pub type Rotation<T> = na::Unit<na::Complex<T>>;
    pub type AngVector<T> = T;

    std::include!("impl.rs");
}

#[cfg(feature = "dim3")]
pub mod dim3 {
    use na::{
        Isometry3 as Isometry, Point3 as Point, Translation3 as Translation, Vector3 as Vector,
    };

    pub type Rotation<T> = na::Unit<na::Quaternion<T>>;
    pub type AngVector<T> = na::Vector3<T>;

    std::include!("impl.rs");
}

#[cfg(all(feature = "dim2", feature = "dim3"))]
arcana::export_arcana_plugin! {
    ScenePlugin {
        components: [dim2::Global, dim3::Global],
        systems: [
            scene_2d: dim2::scene_system,
            scene_3d: dim3::scene_system,
        ],
    }
}

#[cfg(all(feature = "dim2", not(feature = "dim3")))]
arcana::export_arcana_plugin! {
    ScenePlugin {
        components: [dim2::Global],
        systems: [
            scene_2d: dim2::scene_system,
        ],
    }
}

#[cfg(all(feature = "dim3", not(feature = "dim2")))]
arcana::export_arcana_plugin! {
    ScenePlugin {
        components: [dim3::Global],
        systems: [
            scene_3d: dim3::scene_system,
        ],
    }
}
