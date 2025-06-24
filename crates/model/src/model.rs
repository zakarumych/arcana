//! Data model suitable for handling data in absence of types.
//!
//! It can be used to go from type-less to typed seamlessly with `serde`.
//!

use arcana_intern::Name;
use athena::{Matrix2, Matrix3, Matrix4, Vector2, Vector3, Vector4};

use edict::entity::EntityId;
use gametime::TimeSpan;
use hashbrown::HashMap;
use smol_str::SmolStr;

use crate::value::{ColorValue, Value};

#[derive(
    Copy, Clone, Debug, Default, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize,
)]
pub enum ColorModel {
    Luma,
    Lumaa,
    #[default]
    Srgb,
    Srgba,
    Hsv,
    Hsva,
}

/// Data model compatible with serde but enriched with
/// additional primitives.
///
/// - Color types `Rgb`, `Rgba`, `Hsv`, `Hsva` etc
/// - Vector types `Vec2`, `Vec3`, `Vec4`
/// - Matrix types `Mat2`, `Mat3`, `Mat4`
/// - Entity id
/// - Asset id
/// - Composite types
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum Model {
    /// Type with only single value and thus no data.
    /// Unit, null, void, nothing.
    #[default]
    Unit,

    /// Boolean value.
    Bool,

    /// Integer value.
    Int,

    /// Floating point value.
    Float,

    /// String value.
    String,

    /// Color value.
    /// Any representation of color.
    Color(ColorModel),

    /// Time span
    TimeSpan,

    /// Entity id.
    Entity,

    /// 2 component vector.
    Vec2,

    /// 3 component vector.
    Vec3,

    /// 4 component vector.
    Vec4,

    /// 2x2 matrix.
    Mat2,

    /// 3x3 matrix.
    Mat3,

    /// 4x4 matrix.
    Mat4,

    /// Optional value.
    Option(Option<Box<Model>>),

    /// Array of values with same model and optional length.
    Array {
        elem: Option<Box<Model>>,
        len: Option<usize>,
    },

    /// Map of values with same model with string keys.
    Map(Option<Box<Model>>),

    /// Tuple with unnamed fields.
    Tuple(Vec<Option<Model>>),

    /// Record with named fields.
    Record(Vec<(Name, Option<Model>)>),

    /// Enum with named variants.
    Enum(Vec<(Name, Option<Model>)>),
}

impl Model {
    /// Returns default value that corresponds to the model.
    pub fn default_value(&self) -> Value {
        match *self {
            Model::Unit => Value::Unit,
            Model::Bool => Value::Bool(false),
            Model::Int => Value::Int(0),
            Model::Float => Value::Float(0.0),
            Model::String => Value::String(SmolStr::default()),
            Model::Color(ColorModel::Luma) => {
                Value::Color(ColorValue::Luma(palette::LinLuma::new(0.0)))
            }
            Model::Color(ColorModel::Lumaa) => {
                Value::Color(ColorValue::Lumaa(palette::LinLumaa::new(0.0, 1.0)))
            }
            Model::Color(ColorModel::Srgb) => {
                Value::Color(ColorValue::Srgb(palette::Srgb::new(0.0, 0.0, 0.0)))
            }
            Model::Color(ColorModel::Srgba) => {
                Value::Color(ColorValue::Srgba(palette::Srgba::new(0.0, 0.0, 0.0, 1.0)))
            }
            Model::Color(ColorModel::Hsv) => {
                Value::Color(ColorValue::Hsv(palette::Hsv::new(0.0, 0.0, 0.0)))
            }
            Model::Color(ColorModel::Hsva) => {
                Value::Color(ColorValue::Hsva(palette::Hsva::new(0.0, 0.0, 0.0, 1.0)))
            }
            Model::TimeSpan => Value::TimeSpan(TimeSpan::ZERO),
            Model::Entity => Value::Entity(EntityId::dangling()),
            Model::Vec2 => Value::Vec2(Vector2::new(0.0, 0.0)),
            Model::Vec3 => Value::Vec3(Vector3::new(0.0, 0.0, 0.0)),
            Model::Vec4 => Value::Vec4(Vector4::new(0.0, 0.0, 0.0, 0.0)),
            Model::Mat2 => Value::Mat2(Matrix2::identity()),
            Model::Mat3 => Value::Mat3(Matrix3::identity()),
            Model::Mat4 => Value::Mat4(Matrix4::identity()),
            Model::Option(_) => Value::Option(None),
            Model::Array { ref elem, len } => {
                let len = len.unwrap_or(0);
                Value::Array((0..len).map(|_| default_value(elem.as_deref())).collect())
            }
            Model::Map(_) => Value::Map(HashMap::new()),
            Model::Tuple(ref fields) => {
                Value::Array(fields.iter().map(|f| default_value(f.as_ref())).collect())
            }
            Model::Record(ref fields) => Value::Map(
                fields
                    .iter()
                    .map(|&(k, ref v)| (k.into(), default_value(v.as_ref())))
                    .collect(),
            ),
            Model::Enum(ref variants) if variants.is_empty() => Value::Unit,
            Model::Enum(ref variants) => {
                let v = &variants[0];
                Value::Enum(v.0, Box::new(default_value(v.1.as_ref())))
            }
        }
    }
}

/// Returns default value that corresponds to the model or `Unit` if model is not specified.
pub fn default_value(model: Option<&Model>) -> Value {
    match model {
        None => Value::Unit,
        Some(model) => model.default_value(),
    }
}
