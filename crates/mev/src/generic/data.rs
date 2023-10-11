use std::{
    fmt::Debug,
    mem::{align_of, size_of, MaybeUninit},
};

use bytemuck::{Pod, Zeroable};

const fn max_align(a: usize, b: usize) -> usize {
    if a > b {
        a
    } else {
        b
    }
}

/// Type represetable as a POD type with layout with GPU compatible.
pub trait DeviceRepr {
    type Repr: bytemuck::Pod + Debug;

    fn as_repr(&self) -> Self::Repr;

    fn as_bytes(repr: &Self::Repr) -> &[u8] {
        bytemuck::bytes_of(repr)
    }

    fn as_bytes_array(repr: &[Self::Repr]) -> &[u8] {
        bytemuck::cast_slice(repr)
    }

    const ALIGN: usize;
    const SIZE: usize = size_of::<Self::Repr>();
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

pub trait Scalar: crate::private::Sealed + DeviceRepr + Debug + 'static {
    const TYPE: ScalarType;
}

impl crate::private::Sealed for bool {}

impl Scalar for bool {
    const TYPE: ScalarType = ScalarType::Bool;
}

impl DeviceRepr for bool {
    type Repr = u8;

    #[inline(always)]
    fn as_repr(&self) -> u8 {
        *self as u8
    }

    const ALIGN: usize = align_of::<u8>();
}

impl crate::private::Sealed for i8 {}

impl DeviceRepr for i8 {
    type Repr = i8;

    #[inline(always)]
    fn as_repr(&self) -> i8 {
        *self
    }

    const ALIGN: usize = align_of::<i8>();
}

impl Scalar for i8 {
    const TYPE: ScalarType = ScalarType::Sint8;
}

impl crate::private::Sealed for u8 {}

impl DeviceRepr for u8 {
    type Repr = u8;

    #[inline(always)]
    fn as_repr(&self) -> u8 {
        *self
    }

    const ALIGN: usize = align_of::<u8>();
}

impl Scalar for u8 {
    const TYPE: ScalarType = ScalarType::Uint8;
}

impl crate::private::Sealed for i16 {}

impl DeviceRepr for i16 {
    type Repr = i16;

    #[inline(always)]
    fn as_repr(&self) -> i16 {
        *self
    }

    const ALIGN: usize = align_of::<i16>();
}

impl Scalar for i16 {
    const TYPE: ScalarType = ScalarType::Sint16;
}

impl crate::private::Sealed for u16 {}

impl DeviceRepr for u16 {
    type Repr = u16;

    #[inline(always)]
    fn as_repr(&self) -> u16 {
        *self
    }

    const ALIGN: usize = align_of::<u16>();
}

impl Scalar for u16 {
    const TYPE: ScalarType = ScalarType::Uint16;
}

impl crate::private::Sealed for i32 {}

impl DeviceRepr for i32 {
    type Repr = i32;

    #[inline(always)]
    fn as_repr(&self) -> i32 {
        *self
    }

    const ALIGN: usize = align_of::<i32>();
}

impl Scalar for i32 {
    const TYPE: ScalarType = ScalarType::Sint32;
}

impl crate::private::Sealed for u32 {}

impl DeviceRepr for u32 {
    type Repr = u32;

    #[inline(always)]
    fn as_repr(&self) -> u32 {
        *self
    }

    const ALIGN: usize = align_of::<u32>();
}

impl Scalar for u32 {
    const TYPE: ScalarType = ScalarType::Uint32;
}

impl crate::private::Sealed for i64 {}

impl DeviceRepr for i64 {
    type Repr = i64;

    #[inline(always)]
    fn as_repr(&self) -> i64 {
        *self
    }

    const ALIGN: usize = align_of::<i64>();
}

impl Scalar for i64 {
    const TYPE: ScalarType = ScalarType::Sint64;
}

impl crate::private::Sealed for u64 {}

impl DeviceRepr for u64 {
    type Repr = u64;

    #[inline(always)]
    fn as_repr(&self) -> u64 {
        *self
    }

    const ALIGN: usize = align_of::<u64>();
}

impl Scalar for u64 {
    const TYPE: ScalarType = ScalarType::Uint64;
}

impl crate::private::Sealed for f32 {}

impl DeviceRepr for f32 {
    type Repr = f32;

    #[inline(always)]
    fn as_repr(&self) -> f32 {
        *self
    }

    const ALIGN: usize = align_of::<f32>();
}

impl Scalar for f32 {
    const TYPE: ScalarType = ScalarType::Float32;
}

impl crate::private::Sealed for f64 {}

impl DeviceRepr for f64 {
    type Repr = f64;

    #[inline(always)]
    fn as_repr(&self) -> f64 {
        *self
    }

    const ALIGN: usize = align_of::<f64>();
}

impl Scalar for f64 {
    const TYPE: ScalarType = ScalarType::Float64;
}

// #[derive(Clone, Copy, Debug)]
// #[repr(align(16), C)]
// pub struct Align16<T> {
//     value: T,
//     padding: [u8; 16 - size_of::<T>() % 16],
// }

// unsafe impl<T> Zeroable for Align16<T>
// where
//     T: Zeroable,
// {
//     fn zeroed() -> Self {
//         Align16(T::zeroed())
//     }
// }

// impl<T, const N: usize> DeviceRepr for [T; N]
// where
//     T: DeviceRepr,
// {
//     type Pod = [Align16<T::Pod>; N];

//     #[inline(always)]
//     fn as_pod(&self) -> Self::Pod {
//         let mut pod = [bytemuck::Zeroable::zeroed(); N];
//         for (i, item) in self.iter().enumerate() {
//             pod[i] = Align16(item.as_pod());
//         }
//         pod
//     }
// }

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

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[allow(non_camel_case_types)]
pub struct vec<T, const N: usize>(pub [T; N]);

unsafe impl<T, const N: usize> Zeroable for vec<T, N> where T: Zeroable {}
unsafe impl<T, const N: usize> Pod for vec<T, N> where T: Pod {}

impl<T, const N: usize> From<vec<T, N>> for [T; N] {
    #[inline(always)]
    fn from(v: vec<T, N>) -> Self {
        v.0
    }
}

impl<T, const N: usize> From<[T; N]> for vec<T, N> {
    #[inline(always)]
    fn from(v: [T; N]) -> Self {
        vec(v)
    }
}

impl<T, const N: usize> From<&[T; N]> for vec<T, N>
where
    T: Copy,
{
    #[inline(always)]
    fn from(v: &[T; N]) -> Self {
        vec(*v)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[allow(non_camel_case_types)]
pub struct mat<T, const N: usize, const M: usize>(pub [vec<T, M>; N]);

unsafe impl<T, const N: usize, const M: usize> Zeroable for mat<T, N, M> where T: Zeroable {}
unsafe impl<T, const N: usize, const M: usize> Pod for mat<T, N, M> where T: Pod {}

impl<T, const N: usize, const M: usize> From<mat<T, N, M>> for [[T; M]; N] {
    #[inline(always)]
    fn from(m: mat<T, N, M>) -> Self {
        m.0.map(From::from)
    }
}

impl<T, const N: usize, const M: usize> From<[[T; M]; N]> for mat<T, N, M> {
    #[inline(always)]
    fn from(m: [[T; M]; N]) -> Self {
        mat(m.map(From::from))
    }
}

impl<T, const N: usize, const M: usize> From<&[[T; M]; N]> for mat<T, N, M>
where
    T: Copy,
{
    #[inline(always)]
    fn from(m: &[[T; M]; N]) -> Self {
        mat(m.map(From::from))
    }
}

#[allow(non_camel_case_types)]
pub type vec2<T = f32> = vec<T, 2>;

pub fn vec2<T>(x: T, y: T) -> vec2<T> {
    vec([x, y])
}

#[allow(non_camel_case_types)]
pub type vec3<T = f32> = vec<T, 3>;

pub fn vec3<T>(x: T, y: T, z: T) -> vec3<T> {
    vec([x, y, z])
}

#[allow(non_camel_case_types)]
pub type vec4<T = f32> = vec<T, 4>;

pub fn vec4<T>(x: T, y: T, z: T, w: T) -> vec4<T> {
    vec([x, y, z, w])
}

#[allow(non_camel_case_types)]
pub type mat2<T = f32> = mat<T, 2, 2>;

pub fn mat2<T>(x: vec2<T>, y: vec2<T>) -> mat2<T> {
    mat([x, y])
}

#[allow(non_camel_case_types)]
pub type mat3<T = f32> = mat<T, 3, 3>;

pub fn mat3<T>(x: vec3<T>, y: vec3<T>, z: vec3<T>) -> mat3<T> {
    mat([x, y, z])
}

#[allow(non_camel_case_types)]
pub type mat4<T = f32> = mat<T, 4, 4>;

pub fn mat4<T>(x: vec4<T>, y: vec4<T>, z: vec4<T>, w: vec4<T>) -> mat4<T> {
    mat([x, y, z, w])
}

#[allow(non_camel_case_types)]
pub type mat2x2<T = f32> = mat<T, 2, 2>;

pub fn mat2x2<T>(x: vec2<T>, y: vec2<T>) -> mat2x2<T> {
    mat([x, y])
}

#[allow(non_camel_case_types)]
pub type mat2x3<T = f32> = mat<T, 2, 3>;

pub fn mat2x3<T>(x: vec3<T>, y: vec3<T>) -> mat2x3<T> {
    mat([x, y])
}

#[allow(non_camel_case_types)]
pub type mat2x4<T = f32> = mat<T, 2, 4>;

pub fn mat2x4<T>(x: vec4<T>, y: vec4<T>) -> mat2x4<T> {
    mat([x, y])
}

#[allow(non_camel_case_types)]
pub type mat3x2<T = f32> = mat<T, 3, 2>;

pub fn mat3x2<T>(x: vec2<T>, y: vec2<T>, z: vec2<T>) -> mat3x2<T> {
    mat([x, y, z])
}

#[allow(non_camel_case_types)]
pub type mat3x3<T = f32> = mat<T, 3, 3>;

pub fn mat3x3<T>(x: vec3<T>, y: vec3<T>, z: vec3<T>) -> mat3x3<T> {
    mat([x, y, z])
}

#[allow(non_camel_case_types)]
pub type mat3x4<T = f32> = mat<T, 3, 4>;

pub fn mat3x4<T>(x: vec4<T>, y: vec4<T>, z: vec4<T>) -> mat3x4<T> {
    mat([x, y, z])
}

#[allow(non_camel_case_types)]
pub type mat4x2<T = f32> = mat<T, 4, 2>;

pub fn mat4x2<T>(x: vec2<T>, y: vec2<T>, z: vec2<T>, w: vec2<T>) -> mat4x2<T> {
    mat([x, y, z, w])
}

#[allow(non_camel_case_types)]
pub type mat4x3<T = f32> = mat<T, 4, 3>;

pub fn mat4x3<T>(x: vec3<T>, y: vec3<T>, z: vec3<T>, w: vec3<T>) -> mat4x3<T> {
    mat([x, y, z, w])
}

#[allow(non_camel_case_types)]
pub type mat4x4<T = f32> = mat<T, 4, 4>;

pub fn mat4x4<T>(x: vec4<T>, y: vec4<T>, z: vec4<T>, w: vec4<T>) -> mat4x4<T> {
    mat([x, y, z, w])
}

impl<T> crate::private::Sealed for vec<T, 1> where T: Scalar {}
impl<T> crate::private::Sealed for vec<T, 2> where T: Scalar {}
impl<T> crate::private::Sealed for vec<T, 3> where T: Scalar {}
impl<T> crate::private::Sealed for vec<T, 4> where T: Scalar {}

impl<T, const N: usize> crate::private::Sealed for mat<T, 1, N> where vec<T, N>: Data {}
impl<T, const N: usize> crate::private::Sealed for mat<T, 2, N> where vec<T, N>: Data {}
impl<T, const N: usize> crate::private::Sealed for mat<T, 3, N> where vec<T, N>: Data {}
impl<T, const N: usize> crate::private::Sealed for mat<T, 4, N> where vec<T, N>: Data {}

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

impl<T, const M: usize> Data for mat<T, 2, M>
where
    vec<T, M>: Data,
{
    const TYPE: DataType = DataType {
        scalar: <vec<T, M> as Data>::TYPE.scalar,
        columns: VectorSize::Two,
        rows: <vec<T, M> as Data>::TYPE.rows,
    };
}

impl<T, const M: usize> Data for mat<T, 3, M>
where
    vec<T, M>: Data,
{
    const TYPE: DataType = DataType {
        scalar: <vec<T, M> as Data>::TYPE.scalar,
        columns: VectorSize::Three,
        rows: <vec<T, M> as Data>::TYPE.rows,
    };
}

impl<T, const M: usize> Data for mat<T, 4, M>
where
    vec<T, M>: Data,
{
    const TYPE: DataType = DataType {
        scalar: <vec<T, M> as Data>::TYPE.scalar,
        columns: VectorSize::Four,
        rows: <vec<T, M> as Data>::TYPE.rows,
    };
}

impl<T> DeviceRepr for vec2<T>
where
    T: Scalar,
{
    type Repr = [T::Repr; 2];

    fn as_repr(&self) -> [T::Repr; 2] {
        [self.0[0].as_repr(), self.0[1].as_repr()]
    }

    const ALIGN: usize = max_align(size_of::<[T::Repr; 2]>(), T::ALIGN);
}

impl<T> DeviceRepr for vec3<T>
where
    T: Scalar,
{
    type Repr = [T::Repr; 4];

    fn as_repr(&self) -> [T::Repr; 4] {
        [
            self.0[0].as_repr(),
            self.0[1].as_repr(),
            self.0[2].as_repr(),
            T::Repr::zeroed(),
        ]
    }

    const ALIGN: usize = max_align(size_of::<[T::Repr; 4]>(), T::ALIGN);
}

impl<T> DeviceRepr for vec4<T>
where
    T: Scalar,
{
    type Repr = [T::Repr; 4];

    fn as_repr(&self) -> [T::Repr; 4] {
        [
            self.0[0].as_repr(),
            self.0[1].as_repr(),
            self.0[2].as_repr(),
            self.0[3].as_repr(),
        ]
    }

    const ALIGN: usize = max_align(size_of::<[T::Repr; 4]>(), T::ALIGN);
}

impl<T, const M: usize> DeviceRepr for mat<T, 2, M>
where
    vec<T, M>: DeviceRepr,
{
    type Repr = [<vec<T, M> as DeviceRepr>::Repr; 2];

    fn as_repr(&self) -> [<vec<T, M> as DeviceRepr>::Repr; 2] {
        [self.0[0].as_repr(), self.0[1].as_repr()]
    }

    const ALIGN: usize = <vec<T, M> as DeviceRepr>::ALIGN;
}

impl<T, const M: usize> DeviceRepr for mat<T, 3, M>
where
    vec<T, M>: DeviceRepr,
{
    type Repr = [<vec<T, M> as DeviceRepr>::Repr; 3];

    fn as_repr(&self) -> [<vec<T, M> as DeviceRepr>::Repr; 3] {
        [
            self.0[0].as_repr(),
            self.0[1].as_repr(),
            self.0[2].as_repr(),
        ]
    }

    const ALIGN: usize = <vec<T, M> as DeviceRepr>::ALIGN;
}

impl<T, const M: usize> DeviceRepr for mat<T, 4, M>
where
    vec<T, M>: DeviceRepr,
{
    type Repr = [<vec<T, M> as DeviceRepr>::Repr; 4];

    fn as_repr(&self) -> [<vec<T, M> as DeviceRepr>::Repr; 4] {
        [
            self.0[0].as_repr(),
            self.0[1].as_repr(),
            self.0[2].as_repr(),
            self.0[3].as_repr(),
        ]
    }

    const ALIGN: usize = <vec<T, M> as DeviceRepr>::ALIGN;
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
