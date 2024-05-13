//! This module defines simple data model suitable for handling data in absense of types.
//!
//! It can be used to go from typeless to typed seemlessly with `serde`.
//!
//!

use edict::EntityId;
use hashbrown::HashMap;
use palette::IntoColor;

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
///
/// and others.
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

    /// Entity id.
    Entity,

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
    Record(Vec<(String, Option<Model>)>),
}

pub fn default_value(model: Option<&Model>) -> Value {
    match model {
        None => Value::Unit,
        Some(model) => model.default_value(),
    }
}

impl Model {
    pub fn default_value(&self) -> Value {
        match *self {
            Model::Unit => Value::Unit,
            Model::Bool => Value::Bool(false),
            Model::Int => Value::Int(0),
            Model::Float => Value::Float(0.0),
            Model::String => Value::String(String::new()),
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
            Model::Vec2 => Value::Vec2(na::Vector2::new(0.0, 0.0)),
            Model::Vec3 => Value::Vec3(na::Vector3::new(0.0, 0.0, 0.0)),
            Model::Vec4 => Value::Vec4(na::Vector4::new(0.0, 0.0, 0.0, 0.0)),
            Model::Mat2 => Value::Mat2(na::Matrix2::identity()),
            Model::Mat3 => Value::Mat3(na::Matrix3::identity()),
            Model::Mat4 => Value::Mat4(na::Matrix4::identity()),
            Model::Entity => Value::Entity(EntityId::dangling()),
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
                    .map(|(k, v)| (k.clone(), default_value(v.as_ref())))
                    .collect(),
            ),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum ColorValue {
    Luma(palette::LinLuma),
    Lumaa(palette::LinLumaa),
    Srgb(palette::Srgb),
    Srgba(palette::Srgba),
    Hsv(palette::Hsv),
    Hsva(palette::Hsva),
}

impl ColorValue {
    pub fn kind(&self) -> &str {
        match self {
            ColorValue::Luma(_) => "Luma",
            ColorValue::Lumaa(_) => "Lumaa",
            ColorValue::Srgb(_) => "Srgb",
            ColorValue::Srgba(_) => "Srgba",
            ColorValue::Hsv(_) => "Hsv",
            ColorValue::Hsva(_) => "Hsva",
        }
    }

    pub fn into_luma(self) -> palette::LinLuma {
        match self {
            ColorValue::Luma(luma) => luma,
            ColorValue::Lumaa(lumaa) => lumaa.into_color(),
            ColorValue::Srgb(srgb) => srgb.into_color(),
            ColorValue::Srgba(srgba) => srgba.into_color(),
            ColorValue::Hsv(hsv) => hsv.into_color(),
            ColorValue::Hsva(hsva) => hsva.into_color(),
        }
    }

    pub fn into_lumaa(self) -> palette::LinLumaa {
        match self {
            ColorValue::Luma(luma) => luma.into_color(),
            ColorValue::Lumaa(lumaa) => lumaa,
            ColorValue::Srgb(srgb) => srgb.into_color(),
            ColorValue::Srgba(srgba) => srgba.into_color(),
            ColorValue::Hsv(hsv) => hsv.into_color(),
            ColorValue::Hsva(hsva) => hsva.into_color(),
        }
    }

    pub fn into_srgb(self) -> palette::Srgb {
        match self {
            ColorValue::Luma(luma) => luma.into_color(),
            ColorValue::Lumaa(lumaa) => lumaa.into_color(),
            ColorValue::Srgb(srgb) => srgb,
            ColorValue::Srgba(srgba) => srgba.into_color(),
            ColorValue::Hsv(hsv) => hsv.into_color(),
            ColorValue::Hsva(hsva) => hsva.into_color(),
        }
    }

    pub fn into_srgba(self) -> palette::Srgba {
        match self {
            ColorValue::Luma(luma) => luma.into_color(),
            ColorValue::Lumaa(lumaa) => lumaa.into_color(),
            ColorValue::Srgb(srgb) => srgb.into_color(),
            ColorValue::Srgba(srgba) => srgba,
            ColorValue::Hsv(hsv) => hsv.into_color(),
            ColorValue::Hsva(hsva) => hsva.into_color(),
        }
    }

    pub fn into_hsv(self) -> palette::Hsv {
        match self {
            ColorValue::Luma(luma) => luma.into_color(),
            ColorValue::Lumaa(lumaa) => lumaa.into_color(),
            ColorValue::Srgb(srgb) => srgb.into_color(),
            ColorValue::Srgba(srgba) => srgba.into_color(),
            ColorValue::Hsv(hsv) => hsv,
            ColorValue::Hsva(hsva) => hsva.into_color(),
        }
    }

    pub fn into_hsva(self) -> palette::Hsva {
        match self {
            ColorValue::Luma(luma) => luma.into_color(),
            ColorValue::Lumaa(lumaa) => lumaa.into_color(),
            ColorValue::Srgb(srgb) => srgb.into_color(),
            ColorValue::Srgba(srgba) => srgba.into_color(),
            ColorValue::Hsv(hsv) => hsv.into_color(),
            ColorValue::Hsva(hsva) => hsva,
        }
    }
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum Value {
    Unit,
    Bool(bool),
    Int(i64),
    Float(f64),
    String(String),
    Color(ColorValue),
    Vec2(na::Vector2<f64>),
    Vec3(na::Vector3<f64>),
    Vec4(na::Vector4<f64>),
    Mat2(na::Matrix2<f64>),
    Mat3(na::Matrix3<f64>),
    Mat4(na::Matrix4<f64>),
    Option(Option<Box<Value>>),
    Entity(EntityId),
    Array(Vec<Value>),
    Map(HashMap<String, Value>),
}

impl Default for Value {
    #[inline(always)]
    fn default() -> Self {
        Value::Unit
    }
}

impl Value {
    pub fn take(&mut self) -> Value {
        std::mem::replace(self, Value::Unit)
    }

    pub fn kind(&self) -> &str {
        match self {
            Value::Unit => "Unit",
            Value::Bool(_) => "Bool",
            Value::Int(_) => "Int",
            Value::Float(_) => "Float",
            Value::String(_) => "String",
            Value::Color(_) => "Color",
            Value::Vec2(_) => "Vec2",
            Value::Vec3(_) => "Vec3",
            Value::Vec4(_) => "Vec4",
            Value::Mat2(_) => "Mat2",
            Value::Mat3(_) => "Mat3",
            Value::Mat4(_) => "Mat4",
            Value::Option(_) => "Option",
            Value::Entity(_) => "Entity",
            Value::Array(_) => "Array",
            Value::Map(_) => "Map",
        }
    }
}
