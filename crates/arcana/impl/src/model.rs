//! Data model suitable for handling data in absense of types.
//!
//! It can be used to go from typeless to typed seemlessly with `serde`.
//!

use std::fmt;

use arcana_names::Name;
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
    Record(Vec<(Name, Option<Model>)>),

    // Enum with named variants.
    Enum(Vec<(Name, Option<Model>)>),
}

/// Returns default value that correspons to the model or `Unit` if model is not specified.
pub fn default_value(model: Option<&Model>) -> Value {
    match model {
        None => Value::Unit,
        Some(model) => model.default_value(),
    }
}

impl Model {
    /// Returns default value that correspons to the model.
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
                    .map(|(k, v)| (k.to_string(), default_value(v.as_ref())))
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

/// Data value compatible with `Model` description.
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
    Enum(Name, Box<Value>),
}

impl Default for Value {
    #[inline(always)]
    fn default() -> Self {
        Value::Unit
    }
}

impl Value {
    // pub fn take(&mut self) -> Value {
    //     std::mem::replace(self, Value::Unit)
    // }

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
            Value::Enum(_, _) => "Enum",
        }
    }
}

#[derive(Clone, Debug, thiserror::Error)]
pub enum DeValueError {
    Custom(String),
}

impl fmt::Display for DeValueError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DeValueError::Custom(msg) => write!(f, "{}", msg),
        }
    }
}

impl serde::de::Error for DeValueError {
    fn custom<T>(msg: T) -> Self
    where
        T: std::fmt::Display,
    {
        DeValueError::Custom(msg.to_string())
    }
}

impl<'de> serde::de::VariantAccess<'de> for ColorValue {
    type Error = DeValueError;

    fn unit_variant(self) -> Result<(), Self::Error> {
        Ok(())
    }

    fn newtype_variant_seed<T>(self, seed: T) -> Result<T::Value, Self::Error>
    where
        T: serde::de::DeserializeSeed<'de>,
    {
        match self {
            ColorValue::Luma(luma) => {
                seed.deserialize(serde::de::value::F32Deserializer::new(luma.luma))
            }
            ColorValue::Lumaa(lumaa) => seed.deserialize(serde::de::value::SeqDeserializer::new(
                [lumaa.luma, lumaa.alpha].into_iter(),
            )),

            ColorValue::Hsv(hsv) => seed.deserialize(serde::de::value::SeqDeserializer::new(
                [hsv.hue.into_inner(), hsv.saturation, hsv.value].into_iter(),
            )),
            ColorValue::Hsva(hsva) => seed.deserialize(serde::de::value::SeqDeserializer::new(
                [
                    hsva.hue.into_inner(),
                    hsva.saturation,
                    hsva.value,
                    hsva.alpha,
                ]
                .into_iter(),
            )),
            ColorValue::Srgb(srgb) => seed.deserialize(serde::de::value::SeqDeserializer::new(
                [srgb.red, srgb.green, srgb.blue].into_iter(),
            )),
            ColorValue::Srgba(srgb) => seed.deserialize(serde::de::value::SeqDeserializer::new(
                [srgb.red, srgb.green, srgb.blue, srgb.alpha].into_iter(),
            )),
        }
    }

    fn tuple_variant<V>(self, _len: usize, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        match self {
            ColorValue::Luma(luma) => visitor.visit_f32(luma.luma),
            ColorValue::Lumaa(lumaa) => visitor.visit_seq(serde::de::value::SeqDeserializer::new(
                [lumaa.luma, lumaa.alpha].into_iter(),
            )),

            ColorValue::Hsv(hsv) => visitor.visit_seq(serde::de::value::SeqDeserializer::new(
                [hsv.hue.into_inner(), hsv.saturation, hsv.value].into_iter(),
            )),
            ColorValue::Hsva(hsva) => visitor.visit_seq(serde::de::value::SeqDeserializer::new(
                [
                    hsva.hue.into_inner(),
                    hsva.saturation,
                    hsva.value,
                    hsva.alpha,
                ]
                .into_iter(),
            )),
            ColorValue::Srgb(srgb) => visitor.visit_seq(serde::de::value::SeqDeserializer::new(
                [srgb.red, srgb.green, srgb.blue].into_iter(),
            )),
            ColorValue::Srgba(srgb) => visitor.visit_seq(serde::de::value::SeqDeserializer::new(
                [srgb.red, srgb.green, srgb.blue, srgb.alpha].into_iter(),
            )),
        }
    }

    fn struct_variant<V>(
        self,
        _fields: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        match self {
            ColorValue::Luma(luma) => visitor.visit_map(serde::de::value::MapDeserializer::new(
                [("luma", luma.luma)].into_iter(),
            )),
            ColorValue::Lumaa(lumaa) => visitor.visit_seq(serde::de::value::MapDeserializer::new(
                [("luma", lumaa.luma), ("alpha", lumaa.alpha)].into_iter(),
            )),
            ColorValue::Hsv(hsv) => visitor.visit_seq(serde::de::value::MapDeserializer::new(
                [
                    ("hue", hsv.hue.into_inner()),
                    ("saturation", hsv.saturation),
                    ("value", hsv.value),
                ]
                .into_iter(),
            )),
            ColorValue::Hsva(hsva) => visitor.visit_seq(serde::de::value::MapDeserializer::new(
                [
                    ("hue", hsva.hue.into_inner()),
                    ("saturation", hsva.saturation),
                    ("value", hsva.value),
                    ("alpha", hsva.alpha),
                ]
                .into_iter(),
            )),
            ColorValue::Srgb(srgb) => visitor.visit_seq(serde::de::value::MapDeserializer::new(
                [
                    ("red", srgb.red),
                    ("green", srgb.green),
                    ("blue", srgb.blue),
                ]
                .into_iter(),
            )),
            ColorValue::Srgba(srgb) => visitor.visit_seq(serde::de::value::MapDeserializer::new(
                [
                    ("red", srgb.red),
                    ("green", srgb.green),
                    ("blue", srgb.blue),
                    ("alpha", srgb.alpha),
                ]
                .into_iter(),
            )),
        }
    }
}

impl<'de> serde::de::EnumAccess<'de> for ColorValue {
    type Error = DeValueError;
    type Variant = Self;

    fn variant_seed<V>(self, seed: V) -> Result<(V::Value, Self::Variant), Self::Error>
    where
        V: serde::de::DeserializeSeed<'de>,
    {
        let value = match self {
            ColorValue::Luma(_) => seed.deserialize(serde::de::value::StrDeserializer::new("Luma")),
            ColorValue::Lumaa(_) => {
                seed.deserialize(serde::de::value::StrDeserializer::new("Lumaa"))
            }
            ColorValue::Hsv(_) => seed.deserialize(serde::de::value::StrDeserializer::new("Hsv")),
            ColorValue::Hsva(_) => seed.deserialize(serde::de::value::StrDeserializer::new("Hsva")),
            ColorValue::Srgb(_) => seed.deserialize(serde::de::value::StrDeserializer::new("Srgb")),
            ColorValue::Srgba(_) => {
                seed.deserialize(serde::de::value::StrDeserializer::new("Srgba"))
            }
        }?;

        Ok((value, self))
    }
}

impl<'de> serde::de::Deserializer<'de> for Value {
    type Error = DeValueError;

    fn deserialize_any<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        match self {
            Value::Unit => visitor.visit_unit(),
            Value::Bool(value) => visitor.visit_bool(value),
            Value::Int(value) => visitor.visit_i64(value),
            Value::Float(value) => visitor.visit_f64(value),
            Value::String(value) => visitor.visit_string(value),
            Value::Color(color) => visitor.visit_enum(color),
            Value::Vec2(vec) => visitor.visit_seq(serde::de::value::SeqDeserializer::new(
                [vec.x, vec.y].into_iter(),
            )),
            Value::Vec3(vec) => visitor.visit_seq(serde::de::value::SeqDeserializer::new(
                [vec.x, vec.y, vec.z].into_iter(),
            )),
            Value::Vec4(vec) => visitor.visit_seq(serde::de::value::SeqDeserializer::new(
                [vec.x, vec.y, vec.z, vec.w].into_iter(),
            )),
            _ => todo!(),
        }
    }

    fn deserialize_struct<V>(
        self,
        name: &'static str,
        fields: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        match self {
            Value::Color(color) => match (name, fields, color) {
                ("Luma", &["luma"], ColorValue::Luma(luma)) => {
                    return visitor.visit_seq(serde::de::value::SeqDeserializer::new(
                        [luma.luma].into_iter(),
                    ))
                }
                ("Lumaa", &["luma", "alpha"], ColorValue::Lumaa(lumaa)) => {
                    return visitor.visit_seq(serde::de::value::SeqDeserializer::new(
                        [lumaa.luma, lumaa.alpha].into_iter(),
                    ))
                }
                _ => {}
            },
            _ => {}
        }
        self.deserialize_any(visitor)
    }

    serde::forward_to_deserialize_any! {
        bool i8 i16 i32 i64 i128 u8 u16 u32 u64 u128 f32 f64 char str
        string bytes byte_buf option unit unit_struct newtype_struct seq
        tuple tuple_struct map enum identifier ignored_any
    }
}

impl<'de> serde::de::Deserializer<'de> for &'de Value {
    type Error = DeValueError;

    fn deserialize_any<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        match *self {
            Value::Unit => visitor.visit_unit(),
            Value::Bool(value) => visitor.visit_bool(value),
            Value::Int(value) => visitor.visit_i64(value),
            Value::Float(value) => visitor.visit_f64(value),
            Value::String(ref value) => visitor.visit_str(value),
            Value::Color(color) => visitor.visit_enum(color),
            _ => todo!(),
        }
    }

    serde::forward_to_deserialize_any! {
        bool i8 i16 i32 i64 i128 u8 u16 u32 u64 u128 f32 f64 char str
        string bytes byte_buf option unit unit_struct newtype_struct seq
        tuple tuple_struct map struct enum identifier ignored_any
    }
}

/// Trait for types that matches some data model.
pub trait TypeModel {
    /// Returns data model that describes the type.
    fn model() -> Model
    where
        Self: Sized;

    /// Returns data model that describes the type.
    fn model_dyn(&self) -> Model;
}
