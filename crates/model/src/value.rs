use core::fmt;

use arcana_base_encoding::base58;
use arcana_intern::Name;
use athena::{Matrix2, Matrix3, Matrix4, Vector2, Vector3, Vector4};
use gametime::TimeSpan;
use hashbrown::HashMap;
use palette::IntoColor;
use smol_str::{SmolStr, SmolStrBuilder};

use crate::model::{ColorModel, Model};

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

    pub fn model(&self) -> ColorModel {
        match self {
            ColorValue::Luma(_) => ColorModel::Luma,
            ColorValue::Lumaa(_) => ColorModel::Lumaa,
            ColorValue::Srgb(_) => ColorModel::Srgb,
            ColorValue::Srgba(_) => ColorModel::Srgba,
            ColorValue::Hsv(_) => ColorModel::Hsv,
            ColorValue::Hsva(_) => ColorModel::Hsva,
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
    String(SmolStr),
    Color(ColorValue),
    TimeSpan(TimeSpan),
    Vec2(Vector2<f64>),
    Vec3(Vector3<f64>),
    Vec4(Vector4<f64>),
    Mat2(Matrix2<f64>),
    Mat3(Matrix3<f64>),
    Mat4(Matrix4<f64>),
    Option(Option<Box<Value>>),
    Array(Vec<Value>),
    Map(HashMap<SmolStr, Value>),
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
            Value::TimeSpan(_) => "TimeSpan",
            Value::Vec2(_) => "Vec2",
            Value::Vec3(_) => "Vec3",
            Value::Vec4(_) => "Vec4",
            Value::Mat2(_) => "Mat2",
            Value::Mat3(_) => "Mat3",
            Value::Mat4(_) => "Mat4",
            Value::Option(_) => "Option",
            Value::Array(_) => "Array",
            Value::Map(_) => "Map",
            Value::Enum(_, _) => "Enum",
        }
    }

    pub fn model(&self) -> Model {
        match self {
            Value::Unit => Model::Unit,
            Value::Bool(_) => Model::Bool,
            Value::Int(_) => Model::Int,
            Value::Float(_) => Model::Float,
            Value::String(_) => Model::String,
            Value::Color(color) => Model::Color(color.model()),
            Value::TimeSpan(_) => Model::TimeSpan,
            Value::Vec2(_) => Model::Vec2,
            Value::Vec3(_) => Model::Vec3,
            Value::Vec4(_) => Model::Vec4,
            Value::Mat2(_) => Model::Mat2,
            Value::Mat3(_) => Model::Mat3,
            Value::Mat4(_) => Model::Mat4,
            Value::Option(_) => Model::Option(None),
            Value::Array(_) => Model::Array {
                elem: None,
                len: None,
            },
            Value::Map(_) => Model::Map(None),
            Value::Enum(_, _) => Model::Enum(Vec::new()),
        }
    }
}

#[derive(Clone, Debug, thiserror::Error)]
pub enum ValueError {
    Custom(String),
}

impl fmt::Display for ValueError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ValueError::Custom(msg) => write!(f, "{}", msg),
        }
    }
}

impl serde::ser::Error for ValueError {
    fn custom<T>(msg: T) -> Self
    where
        T: std::fmt::Display,
    {
        ValueError::Custom(msg.to_string())
    }
}

impl serde::de::Error for ValueError {
    fn custom<T>(msg: T) -> Self
    where
        T: std::fmt::Display,
    {
        ValueError::Custom(msg.to_string())
    }
}

impl<'de> serde::de::VariantAccess<'de> for ColorValue {
    type Error = ValueError;

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
    type Error = ValueError;
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

impl<'de> serde::de::IntoDeserializer<'de, ValueError> for Value {
    type Deserializer = Self;

    #[inline]
    fn into_deserializer(self) -> Self {
        self
    }
}

struct EnumAccess {
    name: Name,
    value: Box<Value>,
}

impl<'de> serde::de::EnumAccess<'de> for EnumAccess {
    type Error = ValueError;
    type Variant = Value;

    fn variant_seed<V>(self, seed: V) -> Result<(V::Value, Value), ValueError>
    where
        V: serde::de::DeserializeSeed<'de>,
    {
        let variant = V::deserialize(seed, serde::de::value::StrDeserializer::new(&self.name))?;
        Ok((variant, *self.value))
    }
}

impl<'de> serde::de::VariantAccess<'de> for Value {
    type Error = ValueError;

    fn unit_variant(self) -> Result<(), ValueError> {
        match self {
            Value::Unit => Ok(()),
            _ => Err(serde::de::Error::custom("expected unit")),
        }
    }

    fn newtype_variant_seed<T>(self, seed: T) -> Result<T::Value, ValueError>
    where
        T: serde::de::DeserializeSeed<'de>,
    {
        T::deserialize(seed, self)
    }

    fn tuple_variant<V>(self, _len: usize, visitor: V) -> Result<V::Value, ValueError>
    where
        V: serde::de::Visitor<'de>,
    {
        serde::de::Deserializer::deserialize_any(self, visitor)
    }

    fn struct_variant<V>(
        self,
        _fields: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, ValueError>
    where
        V: serde::de::Visitor<'de>,
    {
        serde::de::Deserializer::deserialize_any(self, visitor)
    }
}

struct MapAccess {
    iter: hashbrown::hash_map::IntoIter<SmolStr, Value>,
    next_value: Option<Value>,
}

impl<'de> serde::de::MapAccess<'de> for MapAccess {
    type Error = ValueError;

    fn next_key_seed<K>(&mut self, seed: K) -> Result<Option<K::Value>, ValueError>
    where
        K: serde::de::DeserializeSeed<'de>,
    {
        match self.iter.next() {
            None => return Ok(None),
            Some((key, value)) => {
                self.next_value = Some(value);
                seed.deserialize(serde::de::value::StrDeserializer::new(&key))
                    .map(Some)
            }
        }
    }

    fn next_value_seed<V>(&mut self, seed: V) -> Result<V::Value, ValueError>
    where
        V: serde::de::DeserializeSeed<'de>,
    {
        let value = self
            .next_value
            .take()
            .expect("next_key or next_key_seed should be called first");

        seed.deserialize(value)
    }

    fn next_entry_seed<K, V>(
        &mut self,
        kseed: K,
        vseed: V,
    ) -> Result<Option<(K::Value, V::Value)>, ValueError>
    where
        K: serde::de::DeserializeSeed<'de>,
        V: serde::de::DeserializeSeed<'de>,
    {
        match self.iter.next() {
            None => return Ok(None),
            Some((key, value)) => {
                self.next_value = Some(value);
                let key = kseed.deserialize(serde::de::value::StrDeserializer::new(&key))?;
                let value = vseed.deserialize(self.next_value.take().unwrap())?;
                Ok(Some((key, value)))
            }
        }
    }
}

impl<'de> serde::de::Deserializer<'de> for Value {
    type Error = ValueError;

    fn deserialize_any<V>(self, visitor: V) -> Result<V::Value, ValueError>
    where
        V: serde::de::Visitor<'de>,
    {
        match self {
            Value::Unit => visitor.visit_unit(),
            Value::Bool(value) => visitor.visit_bool(value),
            Value::Int(value) => visitor.visit_i64(value),
            Value::Float(value) => visitor.visit_f64(value),
            Value::String(value) => visitor.visit_str(&value),
            Value::Color(color) => visitor.visit_enum(color),
            Value::TimeSpan(span) => visitor.visit_i64(span.as_nanos()),
            Value::Vec2(vec) => visitor.visit_seq(serde::de::value::SeqDeserializer::new(
                vec.into_array().into_iter(),
            )),
            Value::Vec3(vec) => visitor.visit_seq(serde::de::value::SeqDeserializer::new(
                vec.into_array().into_iter(),
            )),
            Value::Vec4(vec) => visitor.visit_seq(serde::de::value::SeqDeserializer::new(
                vec.into_array().into_iter(),
            )),
            Value::Mat2(mat) => visitor.visit_seq(serde::de::value::SeqDeserializer::new(
                mat.arrays()
                    .iter()
                    .map(|v| serde::de::value::SeqDeserializer::new((*v).into_iter())),
            )),
            Value::Mat3(mat) => visitor.visit_seq(serde::de::value::SeqDeserializer::new(
                mat.arrays()
                    .iter()
                    .map(|v| serde::de::value::SeqDeserializer::new((*v).into_iter())),
            )),
            Value::Mat4(mat) => visitor.visit_seq(serde::de::value::SeqDeserializer::new(
                mat.arrays()
                    .iter()
                    .map(|v| serde::de::value::SeqDeserializer::new((*v).into_iter())),
            )),
            Value::Option(None) => visitor.visit_none(),
            Value::Option(Some(value)) => visitor.visit_some(*value),
            Value::Array(array) => {
                visitor.visit_seq(serde::de::value::SeqDeserializer::new(array.into_iter()))
            }
            Value::Map(map) => visitor.visit_map(MapAccess {
                iter: map.into_iter(),
                next_value: None,
            }),
            Value::Enum(name, value) => visitor.visit_enum(EnumAccess { name, value: value }),
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

    fn deserialize_bytes<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        match self {
            Value::String(value) => {
                let mut bytes = Vec::new();
                if let Err(err) = base58::decode_to_vec(value.as_bytes(), &mut bytes) {
                    return Err(ValueError::Custom(err.to_string()));
                }
                visitor.visit_byte_buf(bytes)
            }
            _ => todo!(),
        }
    }

    #[inline]
    fn deserialize_byte_buf<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        self.deserialize_bytes(visitor)
    }

    serde::forward_to_deserialize_any! {
        bool i8 i16 i32 i64 i128 u8 u16 u32 u64 u128 f32 f64 char str
        string option unit unit_struct newtype_struct seq
        tuple tuple_struct map enum identifier ignored_any
    }
}

pub struct ValueSerializerOk {
    _private: (),
}

struct StringSerializer;

impl<'a> serde::ser::Serializer for StringSerializer {
    type Ok = SmolStr;
    type Error = ValueError;

    type SerializeSeq = serde::ser::Impossible<SmolStr, ValueError>;
    type SerializeTuple = serde::ser::Impossible<SmolStr, ValueError>;
    type SerializeTupleStruct = serde::ser::Impossible<SmolStr, ValueError>;
    type SerializeTupleVariant = serde::ser::Impossible<SmolStr, ValueError>;
    type SerializeMap = serde::ser::Impossible<SmolStr, ValueError>;
    type SerializeStruct = serde::ser::Impossible<SmolStr, ValueError>;
    type SerializeStructVariant = serde::ser::Impossible<SmolStr, ValueError>;

    fn serialize_str(self, v: &str) -> Result<SmolStr, Self::Error> {
        Ok(v.into())
    }

    fn serialize_bool(self, _v: bool) -> Result<Self::Ok, Self::Error> {
        Err(ValueError::Custom("Cannot serialize bool".to_string()))
    }

    fn serialize_i8(self, _v: i8) -> Result<Self::Ok, Self::Error> {
        Err(ValueError::Custom("Cannot serialize i8".to_string()))
    }

    fn serialize_i16(self, _v: i16) -> Result<Self::Ok, Self::Error> {
        Err(ValueError::Custom("Cannot serialize i16".to_string()))
    }

    fn serialize_i32(self, _v: i32) -> Result<Self::Ok, Self::Error> {
        Err(ValueError::Custom("Cannot serialize i32".to_string()))
    }

    fn serialize_i64(self, _v: i64) -> Result<Self::Ok, Self::Error> {
        Err(ValueError::Custom("Cannot serialize i64".to_string()))
    }

    fn serialize_i128(self, _v: i128) -> Result<Self::Ok, Self::Error> {
        Err(ValueError::Custom("Cannot serialize i128".to_string()))
    }

    fn serialize_u8(self, _v: u8) -> Result<Self::Ok, Self::Error> {
        Err(ValueError::Custom("Cannot serialize u8".to_string()))
    }

    fn serialize_u16(self, _v: u16) -> Result<Self::Ok, Self::Error> {
        Err(ValueError::Custom("Cannot serialize u16".to_string()))
    }

    fn serialize_u32(self, _v: u32) -> Result<Self::Ok, Self::Error> {
        Err(ValueError::Custom("Cannot serialize u32".to_string()))
    }

    fn serialize_u64(self, _v: u64) -> Result<Self::Ok, Self::Error> {
        Err(ValueError::Custom("Cannot serialize u64".to_string()))
    }

    fn serialize_u128(self, _v: u128) -> Result<Self::Ok, Self::Error> {
        Err(ValueError::Custom("Cannot serialize u128".to_string()))
    }

    fn serialize_f32(self, _v: f32) -> Result<Self::Ok, Self::Error> {
        Err(ValueError::Custom("Cannot serialize f32".to_string()))
    }

    fn serialize_f64(self, _v: f64) -> Result<Self::Ok, Self::Error> {
        Err(ValueError::Custom("Cannot serialize f64".to_string()))
    }

    fn serialize_char(self, _v: char) -> Result<Self::Ok, Self::Error> {
        Err(ValueError::Custom("Cannot serialize char".to_string()))
    }

    fn serialize_bytes(self, _v: &[u8]) -> Result<Self::Ok, Self::Error> {
        Err(ValueError::Custom("Cannot serialize bytes".to_string()))
    }

    fn serialize_none(self) -> Result<Self::Ok, Self::Error> {
        Err(ValueError::Custom("Cannot serialize None".to_string()))
    }

    fn serialize_some<T>(self, _value: &T) -> Result<Self::Ok, Self::Error>
    where
        T: ?Sized + serde::ser::Serialize,
    {
        Err(ValueError::Custom("Cannot serialize Some(_)".to_owned()))
    }

    fn serialize_unit(self) -> Result<Self::Ok, Self::Error> {
        Err(ValueError::Custom("Cannot serialize unit".to_string()))
    }

    fn serialize_unit_struct(self, _name: &'static str) -> Result<Self::Ok, Self::Error> {
        Err(ValueError::Custom(
            "Cannot serialize unit struct".to_string(),
        ))
    }

    fn serialize_unit_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        _variant: &'static str,
    ) -> Result<Self::Ok, Self::Error> {
        Err(ValueError::Custom(
            "Cannot serialize unit variant".to_string(),
        ))
    }

    fn serialize_newtype_struct<T>(
        self,
        _name: &'static str,
        _value: &T,
    ) -> Result<Self::Ok, Self::Error>
    where
        T: ?Sized + serde::Serialize,
    {
        Err(ValueError::Custom(
            "Cannot serialize newtype struct".to_string(),
        ))
    }

    fn serialize_newtype_variant<T>(
        self,
        _name: &'static str,
        _variant_index: u32,
        _variant: &'static str,
        _value: &T,
    ) -> Result<Self::Ok, Self::Error>
    where
        T: ?Sized + serde::Serialize,
    {
        Err(ValueError::Custom(
            "Cannot serialize newtype variant".to_string(),
        ))
    }

    fn serialize_seq(self, _len: Option<usize>) -> Result<Self::SerializeSeq, Self::Error> {
        Err(ValueError::Custom("Cannot serialize sequence".to_string()))
    }

    fn serialize_tuple(self, _len: usize) -> Result<Self::SerializeTuple, Self::Error> {
        Err(ValueError::Custom("Cannot serialize tuple".to_string()))
    }

    fn serialize_tuple_struct(
        self,
        _name: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeTupleStruct, Self::Error> {
        Err(ValueError::Custom(
            "Cannot serialize tuple struct".to_string(),
        ))
    }

    fn serialize_tuple_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        _variant: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeTupleVariant, Self::Error> {
        Err(ValueError::Custom(
            "Cannot serialize tuple variant".to_string(),
        ))
    }

    fn serialize_map(self, _len: Option<usize>) -> Result<Self::SerializeMap, Self::Error> {
        Err(ValueError::Custom("Cannot serialize map".to_string()))
    }

    fn serialize_struct(
        self,
        _name: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeStruct, Self::Error> {
        Err(ValueError::Custom("Cannot serialize struct".to_string()))
    }

    fn serialize_struct_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        _variant: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeStructVariant, Self::Error> {
        Err(ValueError::Custom(
            "Cannot serialize struct variant".to_string(),
        ))
    }

    fn is_human_readable(&self) -> bool {
        false
    }
}

pub struct ValueSerializer<'a>(&'a mut Value);

impl<'a> ValueSerializer<'a> {
    fn array(self, len: Option<usize>) -> ValueArraySerializer<'a> {
        let array = Vec::with_capacity(len.unwrap_or(0));
        *self.0 = Value::Array(array);
        match self.0 {
            Value::Array(ref mut array) => ValueArraySerializer(array),
            _ => unreachable!(),
        }
    }

    fn map(self, len: Option<usize>) -> ValueMapSerializer<'a> {
        let map = HashMap::with_capacity(len.unwrap_or(0));
        *self.0 = Value::Map(map);
        match self.0 {
            Value::Map(ref mut map) => ValueMapSerializer { map, new_key: None },
            _ => unreachable!(),
        }
    }
}

pub struct ValueArraySerializer<'a>(&'a mut Vec<Value>);

impl<'a> ValueArraySerializer<'a> {
    fn push<T>(&mut self, value: &T) -> Result<(), ValueError>
    where
        T: ?Sized + serde::ser::Serialize,
    {
        let mut v = Value::Unit;
        value.serialize(ValueSerializer(&mut v))?;
        self.0.push(v);
        Ok(())
    }
}

pub struct ValueMapSerializer<'a> {
    map: &'a mut HashMap<SmolStr, Value>,
    new_key: Option<SmolStr>,
}

impl<'a> serde::ser::Serializer for ValueSerializer<'a> {
    type Ok = ValueSerializerOk;
    type Error = ValueError;

    type SerializeSeq = ValueArraySerializer<'a>;
    type SerializeTuple = ValueArraySerializer<'a>;
    type SerializeTupleStruct = ValueArraySerializer<'a>;
    type SerializeTupleVariant = ValueArraySerializer<'a>;
    type SerializeMap = ValueMapSerializer<'a>;
    type SerializeStruct = ValueMapSerializer<'a>;
    type SerializeStructVariant = ValueMapSerializer<'a>;

    fn serialize_bool(self, v: bool) -> Result<Self::Ok, ValueError> {
        *self.0 = Value::Bool(v);
        Ok(ValueSerializerOk { _private: () })
    }

    fn serialize_i8(self, v: i8) -> Result<Self::Ok, Self::Error> {
        *self.0 = Value::Int(v as i64);
        Ok(ValueSerializerOk { _private: () })
    }

    fn serialize_i16(self, v: i16) -> Result<Self::Ok, Self::Error> {
        *self.0 = Value::Int(v as i64);
        Ok(ValueSerializerOk { _private: () })
    }

    fn serialize_i32(self, v: i32) -> Result<Self::Ok, Self::Error> {
        *self.0 = Value::Int(v as i64);
        Ok(ValueSerializerOk { _private: () })
    }

    fn serialize_i64(self, v: i64) -> Result<Self::Ok, Self::Error> {
        *self.0 = Value::Int(v);
        Ok(ValueSerializerOk { _private: () })
    }

    fn serialize_u8(self, v: u8) -> Result<Self::Ok, Self::Error> {
        *self.0 = Value::Int(v as i64);
        Ok(ValueSerializerOk { _private: () })
    }

    fn serialize_u16(self, v: u16) -> Result<Self::Ok, Self::Error> {
        *self.0 = Value::Int(v as i64);
        Ok(ValueSerializerOk { _private: () })
    }

    fn serialize_u32(self, v: u32) -> Result<Self::Ok, Self::Error> {
        *self.0 = Value::Int(v as i64);
        Ok(ValueSerializerOk { _private: () })
    }

    fn serialize_u64(self, v: u64) -> Result<Self::Ok, Self::Error> {
        if v > i64::MAX as u64 {
            return Err(ValueError::Custom("Integer overflow".to_string()));
        }
        *self.0 = Value::Int(v as i64);
        Ok(ValueSerializerOk { _private: () })
    }

    fn serialize_f32(self, v: f32) -> Result<Self::Ok, Self::Error> {
        *self.0 = Value::Float(v as f64);
        Ok(ValueSerializerOk { _private: () })
    }

    fn serialize_f64(self, v: f64) -> Result<Self::Ok, Self::Error> {
        *self.0 = Value::Float(v);
        Ok(ValueSerializerOk { _private: () })
    }

    fn serialize_char(self, v: char) -> Result<Self::Ok, Self::Error> {
        let mut s = SmolStrBuilder::new();
        s.push(v);
        *self.0 = Value::String(s.finish());
        Ok(ValueSerializerOk { _private: () })
    }

    fn serialize_str(self, v: &str) -> Result<Self::Ok, Self::Error> {
        *self.0 = Value::String(v.into());
        Ok(ValueSerializerOk { _private: () })
    }

    fn serialize_bytes(self, v: &[u8]) -> Result<Self::Ok, Self::Error> {
        let mut s = SmolStrBuilder::new();
        base58::encode_to_fmt(v, &mut s).expect("SmolStrBuilder fmt::Write should not fail");
        *self.0 = Value::String(s.finish());
        Ok(ValueSerializerOk { _private: () })
    }

    fn serialize_none(self) -> Result<Self::Ok, Self::Error> {
        *self.0 = Value::Option(None);
        Ok(ValueSerializerOk { _private: () })
    }

    fn serialize_some<T>(self, value: &T) -> Result<Self::Ok, Self::Error>
    where
        T: ?Sized + serde::ser::Serialize,
    {
        let mut v = Value::Unit;
        value.serialize(ValueSerializer(&mut v))?;
        *self.0 = Value::Option(Some(Box::new(v)));
        Ok(ValueSerializerOk { _private: () })
    }

    fn serialize_unit(self) -> Result<Self::Ok, Self::Error> {
        *self.0 = Value::Unit;
        Ok(ValueSerializerOk { _private: () })
    }

    fn serialize_unit_struct(self, _name: &'static str) -> Result<Self::Ok, Self::Error> {
        *self.0 = Value::Unit;
        Ok(ValueSerializerOk { _private: () })
    }

    fn serialize_unit_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        _variant: &'static str,
    ) -> Result<Self::Ok, Self::Error> {
        *self.0 = Value::Unit;
        Ok(ValueSerializerOk { _private: () })
    }

    fn serialize_newtype_struct<T>(
        self,
        _name: &'static str,
        value: &T,
    ) -> Result<Self::Ok, Self::Error>
    where
        T: ?Sized + serde::Serialize,
    {
        let mut v = Value::Unit;
        value.serialize(ValueSerializer(&mut v))?;
        *self.0 = v;
        Ok(ValueSerializerOk { _private: () })
    }

    fn serialize_newtype_variant<T>(
        self,
        _name: &'static str,
        _variant_index: u32,
        _variant: &'static str,
        value: &T,
    ) -> Result<Self::Ok, Self::Error>
    where
        T: ?Sized + serde::Serialize,
    {
        let mut v = Value::Unit;
        value.serialize(ValueSerializer(&mut v))?;
        *self.0 = v;
        Ok(ValueSerializerOk { _private: () })
    }

    fn serialize_seq(self, len: Option<usize>) -> Result<Self::SerializeSeq, Self::Error> {
        Ok(self.array(len))
    }

    fn serialize_tuple(self, len: usize) -> Result<Self::SerializeTuple, Self::Error> {
        Ok(self.array(Some(len)))
    }

    fn serialize_tuple_struct(
        self,
        _name: &'static str,
        len: usize,
    ) -> Result<Self::SerializeTupleStruct, Self::Error> {
        Ok(self.array(Some(len)))
    }

    fn serialize_tuple_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        _variant: &'static str,
        len: usize,
    ) -> Result<Self::SerializeTupleVariant, Self::Error> {
        Ok(self.array(Some(len)))
    }

    fn serialize_map(self, len: Option<usize>) -> Result<Self::SerializeMap, Self::Error> {
        Ok(self.map(len))
    }

    fn serialize_struct(
        self,
        _name: &'static str,
        len: usize,
    ) -> Result<Self::SerializeStruct, Self::Error> {
        Ok(self.map(Some(len)))
    }

    fn serialize_struct_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        _variant: &'static str,
        len: usize,
    ) -> Result<Self::SerializeStructVariant, Self::Error> {
        Ok(self.map(Some(len)))
    }

    fn is_human_readable(&self) -> bool {
        false
    }
}

impl<'a> serde::ser::SerializeSeq for ValueArraySerializer<'a> {
    type Ok = ValueSerializerOk;
    type Error = ValueError;

    fn serialize_element<T>(&mut self, value: &T) -> Result<(), Self::Error>
    where
        T: ?Sized + serde::ser::Serialize,
    {
        self.push(value)
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        Ok(ValueSerializerOk { _private: () })
    }
}

impl<'a> serde::ser::SerializeTuple for ValueArraySerializer<'a> {
    type Ok = ValueSerializerOk;
    type Error = ValueError;

    fn serialize_element<T>(&mut self, value: &T) -> Result<(), Self::Error>
    where
        T: ?Sized + serde::ser::Serialize,
    {
        self.push(value)
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        Ok(ValueSerializerOk { _private: () })
    }
}

impl<'a> serde::ser::SerializeTupleStruct for ValueArraySerializer<'a> {
    type Ok = ValueSerializerOk;
    type Error = ValueError;

    fn serialize_field<T>(&mut self, value: &T) -> Result<(), Self::Error>
    where
        T: ?Sized + serde::ser::Serialize,
    {
        self.push(value)
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        Ok(ValueSerializerOk { _private: () })
    }
}

impl<'a> serde::ser::SerializeTupleVariant for ValueArraySerializer<'a> {
    type Ok = ValueSerializerOk;
    type Error = ValueError;

    fn serialize_field<T>(&mut self, value: &T) -> Result<(), Self::Error>
    where
        T: ?Sized + serde::ser::Serialize,
    {
        self.push(value)
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        Ok(ValueSerializerOk { _private: () })
    }
}

impl<'a> serde::ser::SerializeMap for ValueMapSerializer<'a> {
    type Ok = ValueSerializerOk;
    type Error = ValueError;

    fn serialize_key<T>(&mut self, key: &T) -> Result<(), Self::Error>
    where
        T: ?Sized + serde::ser::Serialize,
    {
        let s = key.serialize(StringSerializer)?;
        self.new_key = Some(s);
        Ok(())
    }

    fn serialize_value<T>(&mut self, value: &T) -> Result<(), Self::Error>
    where
        T: ?Sized + serde::Serialize,
    {
        let key = self
            .new_key
            .take()
            .expect("serialize_key should be called first");

        let mut v = Value::Unit;
        value.serialize(ValueSerializer(&mut v))?;
        self.map.insert(key, v);

        Ok(())
    }

    fn serialize_entry<K, V>(&mut self, key: &K, value: &V) -> Result<(), Self::Error>
    where
        K: ?Sized + serde::Serialize,
        V: ?Sized + serde::Serialize,
    {
        let s = key.serialize(StringSerializer)?;
        let mut v = Value::Unit;
        value.serialize(ValueSerializer(&mut v))?;
        self.map.insert(s, v);
        Ok(())
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        Ok(ValueSerializerOk { _private: () })
    }
}

impl<'a> serde::ser::SerializeStruct for ValueMapSerializer<'a> {
    type Ok = ValueSerializerOk;
    type Error = ValueError;

    fn serialize_field<T>(&mut self, key: &'static str, value: &T) -> Result<(), Self::Error>
    where
        T: ?Sized + serde::ser::Serialize,
    {
        let mut v = Value::Unit;
        value.serialize(ValueSerializer(&mut v))?;
        self.map.insert(key.into(), v);
        Ok(())
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        Ok(ValueSerializerOk { _private: () })
    }
}

impl<'a> serde::ser::SerializeStructVariant for ValueMapSerializer<'a> {
    type Ok = ValueSerializerOk;
    type Error = ValueError;

    fn serialize_field<T>(&mut self, key: &'static str, value: &T) -> Result<(), Self::Error>
    where
        T: ?Sized + serde::ser::Serialize,
    {
        let mut v = Value::Unit;
        value.serialize(ValueSerializer(&mut v))?;
        self.map.insert(key.into(), v);
        Ok(())
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        Ok(ValueSerializerOk { _private: () })
    }
}
