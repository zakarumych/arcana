arcana::declare_plugin!();

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
