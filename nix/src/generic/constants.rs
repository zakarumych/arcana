use std::{fmt::Debug, mem::size_of};

pub trait Constants {
    type Pod: bytemuck::Pod + Debug;

    fn as_pod(&self) -> Self::Pod;

    const SIZE: usize = size_of::<Self::Pod>();
}

/// Types that can be passed as arguments to shaders.
/// Each element of the enum corresponds to a type that implements [`DataType`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ScalarType {
    Bool,
    Sint8,
    Uint8,
    Sint16,
    Uint16,
    Sint32,
    Uint32,
    Sint64,
    Uint64,
    Float16,
    Float32,
    Float64,
}

impl ScalarType {
    pub const fn size(&self) -> usize {
        match self {
            ScalarType::Bool => 1,
            ScalarType::Sint8 => 1,
            ScalarType::Uint8 => 1,
            ScalarType::Sint16 => 2,
            ScalarType::Uint16 => 2,
            ScalarType::Sint32 => 4,
            ScalarType::Uint32 => 4,
            ScalarType::Sint64 => 8,
            ScalarType::Uint64 => 8,
            ScalarType::Float16 => 2,
            ScalarType::Float32 => 4,
            ScalarType::Float64 => 8,
        }
    }
}

pub trait Scalar: crate::private::Sealed + 'static {
    const TYPE: ScalarType;
}

impl crate::private::Sealed for bool {}

impl Scalar for bool {
    const TYPE: ScalarType = ScalarType::Bool;
}

impl crate::private::Sealed for i8 {}

impl Constants for i8 {
    type Pod = i8;

    #[inline(always)]
    fn as_pod(&self) -> i8 {
        *self
    }
}

impl Scalar for i8 {
    const TYPE: ScalarType = ScalarType::Sint8;
}

impl crate::private::Sealed for u8 {}

impl Constants for u8 {
    type Pod = u8;

    #[inline(always)]
    fn as_pod(&self) -> u8 {
        *self
    }
}

impl Scalar for u8 {
    const TYPE: ScalarType = ScalarType::Uint8;
}

impl crate::private::Sealed for i16 {}

impl Constants for i16 {
    type Pod = i16;

    #[inline(always)]
    fn as_pod(&self) -> i16 {
        *self
    }
}

impl Scalar for i16 {
    const TYPE: ScalarType = ScalarType::Sint16;
}

impl crate::private::Sealed for u16 {}

impl Constants for u16 {
    type Pod = u16;

    #[inline(always)]
    fn as_pod(&self) -> u16 {
        *self
    }
}

impl Scalar for u16 {
    const TYPE: ScalarType = ScalarType::Uint16;
}

impl crate::private::Sealed for i32 {}

impl Constants for i32 {
    type Pod = i32;

    #[inline(always)]
    fn as_pod(&self) -> i32 {
        *self
    }
}

impl Scalar for i32 {
    const TYPE: ScalarType = ScalarType::Sint32;
}

impl crate::private::Sealed for u32 {}

impl Constants for u32 {
    type Pod = u32;

    #[inline(always)]
    fn as_pod(&self) -> u32 {
        *self
    }
}

impl Scalar for u32 {
    const TYPE: ScalarType = ScalarType::Uint32;
}

impl crate::private::Sealed for i64 {}

impl Constants for i64 {
    type Pod = i64;

    #[inline(always)]
    fn as_pod(&self) -> i64 {
        *self
    }
}

impl Scalar for i64 {
    const TYPE: ScalarType = ScalarType::Sint64;
}

impl crate::private::Sealed for u64 {}

impl Constants for u64 {
    type Pod = u64;

    #[inline(always)]
    fn as_pod(&self) -> u64 {
        *self
    }
}

impl Scalar for u64 {
    const TYPE: ScalarType = ScalarType::Uint64;
}

impl crate::private::Sealed for f32 {}

impl Constants for f32 {
    type Pod = f32;

    #[inline(always)]
    fn as_pod(&self) -> f32 {
        *self
    }
}

impl Scalar for f32 {
    const TYPE: ScalarType = ScalarType::Float32;
}

impl crate::private::Sealed for f64 {}

impl Constants for f64 {
    type Pod = f64;

    #[inline(always)]
    fn as_pod(&self) -> f64 {
        *self
    }
}

impl Scalar for f64 {
    const TYPE: ScalarType = ScalarType::Float64;
}

/// Supported sizes of vectors.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum VectorSize {
    One = 1,
    Two = 2,
    Three = 3,
    Four = 4,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct DataType {
    pub scalar: ScalarType,
    pub columns: VectorSize,
    pub rows: VectorSize,
}

/// Values that can be passed as arguments to shaders.
/// This trait is sealed and cannot be implemented in other crates.
pub trait Data: crate::private::Sealed + 'static {
    /// The scalar type of the data type.
    const TYPE: DataType;
}

impl<T> Data for T
where
    T: Scalar,
{
    const TYPE: DataType = DataType {
        scalar: T::TYPE,
        columns: VectorSize::One,
        rows: VectorSize::One,
    };
}

impl<T> crate::private::Sealed for [T; 1] where T: Data {}
impl<T> crate::private::Sealed for [T; 2] where T: Data {}
impl<T> crate::private::Sealed for [T; 3] where T: Data {}
impl<T> crate::private::Sealed for [T; 4] where T: Data {}

#[allow(non_camel_case_types)]
pub type vec<T, const N: usize> = [T; N];

#[allow(non_camel_case_types)]
pub type mat<T, const N: usize, const M: usize> = [vec<T, M>; N];

#[allow(non_camel_case_types)]
pub type vec2<T = f32> = [T; 2];

#[allow(non_camel_case_types)]
pub type vec3<T = f32> = [T; 3];

#[allow(non_camel_case_types)]
pub type vec4<T = f32> = [T; 4];

#[allow(non_camel_case_types)]
pub type mat2<T = f32> = mat<T, 2, 2>;

#[allow(non_camel_case_types)]
pub type mat3<T = f32> = mat<T, 3, 3>;

#[allow(non_camel_case_types)]
pub type mat4<T = f32> = mat<T, 4, 4>;

#[allow(non_camel_case_types)]
pub type mat2x2<T = f32> = mat<T, 2, 2>;

#[allow(non_camel_case_types)]
pub type mat2x3<T = f32> = mat<T, 2, 3>;

#[allow(non_camel_case_types)]
pub type mat2x4<T = f32> = mat<T, 2, 4>;

#[allow(non_camel_case_types)]
pub type mat3x2<T = f32> = mat<T, 3, 2>;

#[allow(non_camel_case_types)]
pub type mat3x3<T = f32> = mat<T, 3, 3>;

#[allow(non_camel_case_types)]
pub type mat3x4<T = f32> = mat<T, 3, 4>;

#[allow(non_camel_case_types)]
pub type mat4x2<T = f32> = mat<T, 4, 2>;

#[allow(non_camel_case_types)]
pub type mat4x3<T = f32> = mat<T, 4, 3>;

#[allow(non_camel_case_types)]
pub type mat4x4<T = f32> = mat<T, 4, 4>;

impl<T> Data for vec2<T>
where
    T: Scalar,
{
    const TYPE: DataType = DataType {
        scalar: T::TYPE,
        columns: VectorSize::One,
        rows: VectorSize::Two,
    };
}

impl<T> Data for vec3<T>
where
    T: Scalar,
{
    const TYPE: DataType = DataType {
        scalar: T::TYPE,
        columns: VectorSize::One,
        rows: VectorSize::Three,
    };
}

impl<T> Data for vec4<T>
where
    T: Scalar,
{
    const TYPE: DataType = DataType {
        scalar: T::TYPE,
        columns: VectorSize::One,
        rows: VectorSize::Four,
    };
}

impl<T> Data for mat2x2<T>
where
    T: Scalar,
{
    const TYPE: DataType = DataType {
        scalar: T::TYPE,
        columns: VectorSize::Two,
        rows: VectorSize::Two,
    };
}

impl<T> Data for mat3x2<T>
where
    T: Scalar,
{
    const TYPE: DataType = DataType {
        scalar: T::TYPE,
        columns: VectorSize::Three,
        rows: VectorSize::Two,
    };
}

impl<T> Data for mat4x2<T>
where
    T: Scalar,
{
    const TYPE: DataType = DataType {
        scalar: T::TYPE,
        columns: VectorSize::Four,
        rows: VectorSize::Two,
    };
}

impl<T> Data for mat2x3<T>
where
    T: Scalar,
{
    const TYPE: DataType = DataType {
        scalar: T::TYPE,
        columns: VectorSize::Two,
        rows: VectorSize::Three,
    };
}

impl<T> Data for mat3x3<T>
where
    T: Scalar,
{
    const TYPE: DataType = DataType {
        scalar: T::TYPE,
        columns: VectorSize::Three,
        rows: VectorSize::Three,
    };
}

impl<T> Data for mat4x3<T>
where
    T: Scalar,
{
    const TYPE: DataType = DataType {
        scalar: T::TYPE,
        columns: VectorSize::Four,
        rows: VectorSize::Three,
    };
}

impl<T> Data for mat2x4<T>
where
    T: Scalar,
{
    const TYPE: DataType = DataType {
        scalar: T::TYPE,
        columns: VectorSize::Two,
        rows: VectorSize::Four,
    };
}

impl<T> Data for mat3x4<T>
where
    T: Scalar,
{
    const TYPE: DataType = DataType {
        scalar: T::TYPE,
        columns: VectorSize::Three,
        rows: VectorSize::Four,
    };
}

impl<T> Data for mat4x4<T>
where
    T: Scalar,
{
    const TYPE: DataType = DataType {
        scalar: T::TYPE,
        columns: VectorSize::Four,
        rows: VectorSize::Four,
    };
}

#[allow(non_camel_case_types)]
pub type bvec2 = vec2<bool>;

#[allow(non_camel_case_types)]
pub type bvec3 = vec3<bool>;

#[allow(non_camel_case_types)]
pub type bvec4 = vec4<bool>;

#[allow(non_camel_case_types)]
pub type bmat2 = mat2<bool>;

#[allow(non_camel_case_types)]
pub type bmat3 = mat3<bool>;

#[allow(non_camel_case_types)]
pub type bmat4 = mat4<bool>;

#[allow(non_camel_case_types)]
pub type bmat2x2 = mat2x2<bool>;

#[allow(non_camel_case_types)]
pub type bmat2x3 = mat2x3<bool>;

#[allow(non_camel_case_types)]
pub type bmat2x4 = mat2x4<bool>;

#[allow(non_camel_case_types)]
pub type bmat3x2 = mat3x2<bool>;

#[allow(non_camel_case_types)]
pub type bmat3x3 = mat3x3<bool>;

#[allow(non_camel_case_types)]
pub type bmat3x4 = mat3x4<bool>;

#[allow(non_camel_case_types)]
pub type bmat4x2 = mat4x2<bool>;

#[allow(non_camel_case_types)]
pub type bmat4x3 = mat4x3<bool>;

#[allow(non_camel_case_types)]
pub type bmat4x4 = mat4x4<bool>;

#[allow(non_camel_case_types)]
pub type ivec2 = vec2<i32>;

#[allow(non_camel_case_types)]
pub type ivec3 = vec3<i32>;

#[allow(non_camel_case_types)]
pub type ivec4 = vec4<i32>;

#[allow(non_camel_case_types)]
pub type imat2 = mat2<i32>;

#[allow(non_camel_case_types)]
pub type imat3 = mat3<i32>;

#[allow(non_camel_case_types)]
pub type imat4 = mat4<i32>;

#[allow(non_camel_case_types)]
pub type imat2x2 = mat2x2<i32>;

#[allow(non_camel_case_types)]
pub type imat2x3 = mat2x3<i32>;

#[allow(non_camel_case_types)]
pub type imat2x4 = mat2x4<i32>;

#[allow(non_camel_case_types)]
pub type imat3x2 = mat3x2<i32>;

#[allow(non_camel_case_types)]
pub type imat3x3 = mat3x3<i32>;

#[allow(non_camel_case_types)]
pub type imat3x4 = mat3x4<i32>;

#[allow(non_camel_case_types)]
pub type imat4x2 = mat4x2<i32>;

#[allow(non_camel_case_types)]
pub type imat4x3 = mat4x3<i32>;

#[allow(non_camel_case_types)]
pub type imat4x4 = mat4x4<i32>;

#[allow(non_camel_case_types)]
pub type uvec2 = vec2<u32>;

#[allow(non_camel_case_types)]
pub type uvec3 = vec3<u32>;

#[allow(non_camel_case_types)]
pub type uvec4 = vec4<u32>;

#[allow(non_camel_case_types)]
pub type umat2 = mat2<u32>;

#[allow(non_camel_case_types)]
pub type umat3 = mat3<u32>;

#[allow(non_camel_case_types)]
pub type umat4 = mat4<u32>;

#[allow(non_camel_case_types)]
pub type umat2x2 = mat2x2<u32>;

#[allow(non_camel_case_types)]
pub type umat2x3 = mat2x3<u32>;

#[allow(non_camel_case_types)]
pub type umat2x4 = mat2x4<u32>;

#[allow(non_camel_case_types)]
pub type umat3x2 = mat3x2<u32>;

#[allow(non_camel_case_types)]
pub type umat3x3 = mat3x3<u32>;

#[allow(non_camel_case_types)]
pub type umat3x4 = mat3x4<u32>;

#[allow(non_camel_case_types)]
pub type umat4x2 = mat4x2<u32>;

#[allow(non_camel_case_types)]
pub type umat4x3 = mat4x3<u32>;

#[allow(non_camel_case_types)]
pub type umat4x4 = mat4x4<u32>;

#[allow(non_camel_case_types)]
pub type dvec2 = vec2<f64>;

#[allow(non_camel_case_types)]
pub type dvec3 = vec3<f64>;

#[allow(non_camel_case_types)]
pub type dvec4 = vec4<f64>;

#[allow(non_camel_case_types)]
pub type dmat2 = mat2<f64>;

#[allow(non_camel_case_types)]
pub type dmat3 = mat3<f64>;

#[allow(non_camel_case_types)]
pub type dmat4 = mat4<f64>;

#[allow(non_camel_case_types)]
pub type dmat2x2 = mat2x2<f64>;

#[allow(non_camel_case_types)]
pub type dmat2x3 = mat2x3<f64>;

#[allow(non_camel_case_types)]
pub type dmat2x4 = mat2x4<f64>;

#[allow(non_camel_case_types)]
pub type dmat3x2 = mat3x2<f64>;

#[allow(non_camel_case_types)]
pub type dmat3x3 = mat3x3<f64>;

#[allow(non_camel_case_types)]
pub type dmat3x4 = mat3x4<f64>;

#[allow(non_camel_case_types)]
pub type dmat4x2 = mat4x2<f64>;

#[allow(non_camel_case_types)]
pub type dmat4x3 = mat4x3<f64>;

#[allow(non_camel_case_types)]
pub type dmat4x4 = mat4x4<f64>;
