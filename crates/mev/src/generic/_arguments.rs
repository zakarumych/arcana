use crate::traits::Argument;

use self::data_types::DataType;

use super::ShaderStages;

/// Kind of the argument.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ArgumentKind {
    /// Buffer argument.
    Buffer,
    /// Image argument.
    Image,
    /// Sampler argument.
    Sampler,
    /// Constant argument.
    Constant(DataType),
}

/// Layout of the argument
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ArgumentLayout {
    pub kind: ArgumentKind,
    pub size: Option<usize>,
}

/// Layout of the argument
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ArgumentGroupLayout<'a> {
    pub stages: ShaderStages,
    pub arguments: &'a [ArgumentLayout],
}

/// Arguments are data that can be passed from the app into shaders.
/// Images, buffers, and samplers are all arguments.
///
/// This trait should be implemented for types that contain arguments,
/// and is implemented for individual argument types.
///
/// Can be derived for a structure using `#[derive(Arguments)]`.
/// Derive macro requires all fields to implement [`Arguments`] trait.
///
/// Automatically implemented for all types that implement [`Argument`] trait.
pub unsafe trait Arguments: 'static {
    const LAYOUT: &'static [ArgumentLayout];

    fn raw_len(&self) -> usize;
    fn fill_raw(&self, raw: &mut [u8]);
}

unsafe impl<A> Arguments for A
where
    A: Argument,
{
    const LAYOUT: &'static [ArgumentLayout] = &[A::LAYOUT];

    fn raw_len(&self) -> usize {
        self.len()
    }

    fn fill_raw(&self, raw: &mut [u8]) {
        unsafe { std::ptr::copy_nonoverlapping(self.as_ptr().cast(), raw.as_mut_ptr(), self.len()) }
    }
}

pub mod data_types {
    use super::*;

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

    impl DataType {
        pub const fn size(&self) -> usize {
            self.columns as usize * self.rows as usize * self.scalar.size()
        }
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

    impl<T> Argument for T
    where
        T: Data,
    {
        const LAYOUT: ArgumentLayout = ArgumentLayout {
            kind: ArgumentKind::Constant(T::TYPE),
            size: Some(std::mem::size_of::<T>()),
        };

        #[inline(always)]
        fn len(&self) -> usize {
            std::mem::size_of::<T>()
        }
    }

    impl<T> crate::private::Sealed for [T; 1] where T: Scalar {}
    impl<T> crate::private::Sealed for [T; 2] where T: Scalar {}
    impl<T> crate::private::Sealed for [T; 3] where T: Scalar {}
    impl<T> crate::private::Sealed for [T; 4] where T: Scalar {}

    #[repr(C, align(16))]
    struct Align16;

    impl<T> crate::private::Sealed for [(T, Align16); 1] where T: Data {}
    impl<T> crate::private::Sealed for [(T, Align16); 2] where T: Data {}
    impl<T> crate::private::Sealed for [(T, Align16); 3] where T: Data {}
    impl<T> crate::private::Sealed for [(T, Align16); 4] where T: Data {}

    #[allow(non_camel_case_types)]
    pub type vec<T, const N: usize> = [T; N];

    #[allow(non_camel_case_types)]
    pub type mat<T, const N: usize, const M: usize> = [(vec<T, M>, Align16); N];

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
}
