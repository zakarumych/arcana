//! Hashing stuff.

mod noop;
mod sha2;
mod stable;

use core::fmt;

use crate::base58::base58_enc_fmt;

pub use self::{
    noop::{no_hash_map, NoHashBuilder, NoHashMap, NoHasher},
    sha2::{sha256, sha256_file, sha256_io, sha512, sha512_file, sha512_io},
    stable::{
        hue_hash, mix_hash_with_string, rgb_hash, rgba_hash, rgba_premultiplied_hash, stable_hash,
        stable_hash_file, stable_hash_map, stable_hash_read, stable_hasher, StableHashBuilder,
        StableHashMap,
    },
};

/// 64-bit hash value.
#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Hash64(pub [u8; 8]);

impl fmt::Debug for Hash64 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if f.alternate() {
            write!(f, "{:016x}", self.as_u64())
        } else {
            base58_enc_fmt(&self.0, f)
        }
    }
}

impl fmt::Display for Hash64 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        base58_enc_fmt(&self.0, f)
    }
}

#[cfg(target_endian = "big")]
impl Hash64 {
    pub const fn as_u16(&self) -> [u16; 4] {
        let [a0, a1, b0, b1, c0, c1, d0, d1] = self.0;

        let a = u16::from_le_bytes([a0, a1]);
        let b = u16::from_le_bytes([b0, b1]);
        let c = u16::from_le_bytes([c0, c1]);
        let d = u16::from_le_bytes([d0, d1]);

        [a, b, c, d]
    }

    pub const fn as_u32(&self) -> [u32; 2] {
        let [a0, a1, a2, a3, b0, b1, b2, b3] = self.0;

        let a = u32::from_le_bytes([a0, a1, a2, a3]);
        let b = u32::from_le_bytes([b0, b1, b2, b3]);

        [a, b]
    }

    pub const fn as_u64(&self) -> u64 {
        u64::from_le_bytes(self.0)
    }
}

#[cfg(target_endian = "little")]
impl Hash64 {
    pub const fn as_u16(&self) -> [u16; 4] {
        unsafe { core::mem::transmute_copy(&self.0) }
    }

    pub const fn as_u32(&self) -> [u32; 2] {
        unsafe { core::mem::transmute_copy(&self.0) }
    }

    #[inline]
    pub const fn as_u64(&self) -> u64 {
        unsafe { core::mem::transmute_copy(&self.0) }
    }
}

/// 128-bit hash value.
#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Hash128(pub [u8; 16]);

impl fmt::Debug for Hash128 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if f.alternate() {
            let [a, b] = self.as_u64();
            write!(f, "{:016x}{:016x}", a, b)
        } else {
            base58_enc_fmt(&self.0, f)
        }
    }
}

impl fmt::Display for Hash128 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        base58_enc_fmt(&self.0, f)
    }
}

#[cfg(target_endian = "big")]
impl Hash128 {
    pub const fn as_u16(&self) -> [u16; 8] {
        let [a0, a1, b0, b1, c0, c1, d0, d1, e0, e1, f0, f1, g0, g1, h0, h1] = self.0;

        let a = u16::from_le_bytes([a0, a1]);
        let b = u16::from_le_bytes([b0, b1]);
        let c = u16::from_le_bytes([c0, c1]);
        let d = u16::from_le_bytes([d0, d1]);
        let e = u16::from_le_bytes([e0, e1]);
        let f = u16::from_le_bytes([f0, f1]);
        let g = u16::from_le_bytes([g0, g1]);
        let h = u16::from_le_bytes([h0, h1]);

        [a, b, c, d, e, f, g, h]
    }

    pub const fn as_u32(&self) -> [u32; 4] {
        let [a0, a1, a2, a3, b0, b1, b2, b3, c0, c1, c2, c3, d0, d1, d2, d3] = self.0;

        let a = u32::from_le_bytes([a0, a1, a2, a3]);
        let b = u32::from_le_bytes([b0, b1, b2, b3]);
        let c = u32::from_le_bytes([c0, c1, c2, c3]);
        let d = u32::from_le_bytes([d0, d1, d2, d3]);

        [a, b, c, d]
    }

    #[inline]
    pub const fn as_u64(&self) -> [u64; 2] {
        let [a0, a1, a2, a3, a4, a5, a6, a7, b0, b1, b2, b3, b4, b5, b6, b7] = self.0;

        let a = u64::from_le_bytes([a0, a1, a2, a3, a4, a5, a6, a7]);
        let b = u64::from_le_bytes([b0, b1, b2, b3, b4, b5, b6, b7]);

        [a, b]
    }

    #[inline]
    pub const fn as_u128(&self) -> u128 {
        u128::from_le_bytes(self.0)
    }
}

#[cfg(target_endian = "little")]
impl Hash128 {
    pub const fn as_u16(&self) -> [u16; 8] {
        unsafe { core::mem::transmute_copy(&self.0) }
    }

    pub const fn as_u32(&self) -> [u32; 4] {
        unsafe { core::mem::transmute_copy(&self.0) }
    }

    #[inline]
    pub const fn as_u64(&self) -> [u64; 2] {
        unsafe { core::mem::transmute_copy(&self.0) }
    }

    #[inline]
    pub const fn as_u128(&self) -> u128 {
        unsafe { core::mem::transmute_copy(&self.0) }
    }
}

/// 256-bit hash value.
#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Hash256(pub [u8; 32]);

impl fmt::Debug for Hash256 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if f.alternate() {
            let [a, b, c, d] = self.as_u64();
            write!(f, "{:016x}{:016x}{:016x}{:016x}", a, b, c, d)
        } else {
            base58_enc_fmt(&self.0, f)
        }
    }
}

impl fmt::Display for Hash256 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        base58_enc_fmt(&self.0, f)
    }
}

#[cfg(target_endian = "big")]
impl Hash256 {
    pub const fn as_u16(&self) -> [u16; 16] {
        let [a0, a1, b0, b1, c0, c1, d0, d1, e0, e1, f0, f1, g0, g1, h0, h1, i0, i1, j0, j1, k0, k1, l0, l1, m0, m1, n0, n1, o0, o1, p0, p1] =
            self.0;

        let a = u16::from_le_bytes([a0, a1]);
        let b = u16::from_le_bytes([b0, b1]);
        let c = u16::from_le_bytes([c0, c1]);
        let d = u16::from_le_bytes([d0, d1]);
        let e = u16::from_le_bytes([e0, e1]);
        let f = u16::from_le_bytes([f0, f1]);
        let g = u16::from_le_bytes([g0, g1]);
        let h = u16::from_le_bytes([h0, h1]);
        let i = u16::from_le_bytes([i0, i1]);
        let j = u16::from_le_bytes([j0, j1]);
        let k = u16::from_le_bytes([k0, k1]);
        let l = u16::from_le_bytes([l0, l1]);
        let m = u16::from_le_bytes([m0, m1]);
        let n = u16::from_le_bytes([n0, n1]);
        let o = u16::from_le_bytes([o0, o1]);
        let p = u16::from_le_bytes([p0, p1]);

        [a, b, c, d, e, f, g, h, i, j, k, l, m, n, o, p]
    }

    pub const fn as_u32(&self) -> [u32; 8] {
        let [a0, a1, a2, a3, b0, b1, b2, b3, c0, c1, c2, c3, d0, d1, d2, d3, e0, e1, e2, e3, f0, f1, f2, f3, g0, g1, g2, g3, h0, h1, h2, h3] =
            self.0;

        let a = u32::from_le_bytes([a0, a1, a2, a3]);
        let b = u32::from_le_bytes([b0, b1, b2, b3]);
        let c = u32::from_le_bytes([c0, c1, c2, c3]);
        let d = u32::from_le_bytes([d0, d1, d2, d3]);
        let e = u32::from_le_bytes([e0, e1, e2, e3]);
        let f = u32::from_le_bytes([f0, f1, f2, f3]);
        let g = u32::from_le_bytes([g0, g1, g2, g3]);
        let h = u32::from_le_bytes([h0, h1, h2, h3]);

        [a, b, c, d, e, f, g, h]
    }

    #[inline]
    pub const fn as_u64(&self) -> [u64; 4] {
        let [a0, a1, a2, a3, a4, a5, a6, a7, b0, b1, b2, b3, b4, b5, b6, b7, c0, c1, c2, c3, c4, c5, c6, c7, d0, d1, d2, d3, d4, d5, d6, d7] =
            self.0;

        let a = u64::from_le_bytes([a0, a1, a2, a3, a4, a5, a6, a7]);
        let b = u64::from_le_bytes([b0, b1, b2, b3, b4, b5, b6, b7]);
        let c = u64::from_le_bytes([c0, c1, c2, c3, c4, c5, c6, c7]);
        let d = u64::from_le_bytes([d0, d1, d2, d3, d4, d5, d6, d7]);

        [a, b, c, d]
    }

    #[inline]
    pub const fn as_u128(&self) -> [u128; 2] {
        let [a0, a1, a2, a3, a4, a5, a6, a7, a8, a9, a10, a11, a12, a13, a14, a15, b0, b1, b2, b3, b4, b5, b6, b7, b8, b9, b10, b11, b12, b13, b14, b15] =
            self.0;

        let a = u128::from_le_bytes([
            a0, a1, a2, a3, a4, a5, a6, a7, a8, a9, a10, a11, a12, a13, a14, a15,
        ]);
        let b = u128::from_le_bytes([
            b0, b1, b2, b3, b4, b5, b6, b7, b8, b9, b10, b11, b12, b13, b14, b15,
        ]);

        [a, b]
    }
}

#[cfg(target_endian = "little")]
impl Hash256 {
    pub const fn as_u16(&self) -> [u16; 16] {
        unsafe { core::mem::transmute_copy(&self.0) }
    }

    pub const fn as_u32(&self) -> [u32; 8] {
        unsafe { core::mem::transmute_copy(&self.0) }
    }

    #[inline]
    pub const fn as_u64(&self) -> [u64; 4] {
        unsafe { core::mem::transmute_copy(&self.0) }
    }

    #[inline]
    pub const fn as_u128(&self) -> [u128; 2] {
        unsafe { core::mem::transmute_copy(&self.0) }
    }
}

/// 512-bit hash value.
#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Hash512(pub [u8; 64]);

impl fmt::Debug for Hash512 {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        if fmt.alternate() {
            let [a, b, c, d, e, f, g, h] = self.as_u64();
            write!(
                fmt,
                "{:016x}{:016x}{:016x}{:016x}{:016x}{:016x}{:016x}{:016x}",
                a, b, c, d, e, f, g, h
            )
        } else {
            base58_enc_fmt(&self.0, fmt)
        }
    }
}

impl fmt::Display for Hash512 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        base58_enc_fmt(&self.0, f)
    }
}

#[cfg(target_endian = "big")]
impl Hash512 {
    pub const fn as_u16(&self) -> [u16; 32] {
        let [a0, a1, a2, a3, b0, b1, b2, b3, c0, c1, c2, c3, d0, d1, d2, d3, e0, e1, e2, e3, f0, f1, f2, f3, g0, g1, g2, g3, h0, h1, h2, h3, i0, i1, i2, i3, j0, j1, j2, j3, k0, k1, k2, k3, l0, l1, l2, l3, m0, m1, m2, m3, n0, n1, n2, n3, o0, o1, o2, o3, p0, p1, p2, p3] =
            self.0;

        let a0 = u16::from_le_bytes([a0, a1]);
        let a1 = u16::from_le_bytes([a2, a3]);
        let b0 = u16::from_le_bytes([b0, b1]);
        let b1 = u16::from_le_bytes([b2, b3]);
        let c0 = u16::from_le_bytes([c0, c1]);
        let c1 = u16::from_le_bytes([c2, c3]);
        let d0 = u16::from_le_bytes([d0, d1]);
        let d1 = u16::from_le_bytes([d2, d3]);
        let e0 = u16::from_le_bytes([e0, e1]);
        let e1 = u16::from_le_bytes([e2, e3]);
        let f0 = u16::from_le_bytes([f0, f1]);
        let f1 = u16::from_le_bytes([f2, f3]);
        let g0 = u16::from_le_bytes([g0, g1]);
        let g1 = u16::from_le_bytes([g2, g3]);
        let h0 = u16::from_le_bytes([h0, h1]);
        let h1 = u16::from_le_bytes([h2, h3]);
        let i0 = u16::from_le_bytes([i0, i1]);
        let i1 = u16::from_le_bytes([i2, i3]);
        let j0 = u16::from_le_bytes([j0, j1]);
        let j1 = u16::from_le_bytes([j2, j3]);
        let k0 = u16::from_le_bytes([k0, k1]);
        let k1 = u16::from_le_bytes([k2, k3]);
        let l0 = u16::from_le_bytes([l0, l1]);
        let l1 = u16::from_le_bytes([l2, l3]);
        let m0 = u16::from_le_bytes([m0, m1]);
        let m1 = u16::from_le_bytes([m2, m3]);
        let n0 = u16::from_le_bytes([n0, n1]);
        let n1 = u16::from_le_bytes([n2, n3]);
        let o0 = u16::from_le_bytes([o0, o1]);
        let o1 = u16::from_le_bytes([o2, o3]);
        let p0 = u16::from_le_bytes([p0, p1]);
        let p1 = u16::from_le_bytes([p2, p3]);

        [
            a0, a1, b0, b1, c0, c1, d0, d1, e0, e1, f0, f1, g0, g1, h0, h1, i0, i1, j0, j1, k0, k1,
            l0, l1, m0, m1, n0, n1, o0, o1, p0, p1,
        ]
    }

    pub const fn as_u32(&self) -> [u32; 16] {
        let [a0, a1, a2, a3, b0, b1, b2, b3, c0, c1, c2, c3, d0, d1, d2, d3, e0, e1, e2, e3, f0, f1, f2, f3, g0, g1, g2, g3, h0, h1, h2, h3, i0, i1, i2, i3, j0, j1, j2, j3, k0, k1, k2, k3, l0, l1, l2, l3, m0, m1, m2, m3, n0, n1, n2, n3, o0, o1, o2, o3, p0, p1, p2, p3] =
            self.0;

        let a = u32::from_le_bytes([a0, a1, a2, a3]);
        let b = u32::from_le_bytes([b0, b1, b2, b3]);
        let c = u32::from_le_bytes([c0, c1, c2, c3]);
        let d = u32::from_le_bytes([d0, d1, d2, d3]);
        let e = u32::from_le_bytes([e0, e1, e2, e3]);
        let f = u32::from_le_bytes([f0, f1, f2, f3]);
        let g = u32::from_le_bytes([g0, g1, g2, g3]);
        let h = u32::from_le_bytes([h0, h1, h2, h3]);
        let i = u32::from_le_bytes([i0, i1, i2, i3]);
        let j = u32::from_le_bytes([j0, j1, j2, j3]);
        let k = u32::from_le_bytes([k0, k1, k2, k3]);
        let l = u32::from_le_bytes([l0, l1, l2, l3]);
        let m = u32::from_le_bytes([m0, m1, m2, m3]);
        let n = u32::from_le_bytes([n0, n1, n2, n3]);
        let o = u32::from_le_bytes([o0, o1, o2, o3]);
        let p = u32::from_le_bytes([p0, p1, p2, p3]);

        [a, b, c, d, e, f, g, h, i, j, k, l, m, n, o, p]
    }

    #[inline]
    pub const fn as_u64(&self) -> [u64; 8] {
        let [a0, a1, a2, a3, a4, a5, a6, a7, b0, b1, b2, b3, b4, b5, b6, b7, c0, c1, c2, c3, c4, c5, c6, c7, d0, d1, d2, d3, d4, d5, d6, d7, e0, e1, e2, e3, e4, e5, e6, e7, f0, f1, f2, f3, f4, f5, f6, f7, g0, g1, g2, g3, g4, g5, g6, g7, h0, h1, h2, h3, h4, h5, h6, h7] =
            self.0;

        let a = u64::from_le_bytes([a0, a1, a2, a3, a4, a5, a6, a7]);
        let b = u64::from_le_bytes([b0, b1, b2, b3, b4, b5, b6, b7]);
        let c = u64::from_le_bytes([c0, c1, c2, c3, c4, c5, c6, c7]);
        let d = u64::from_le_bytes([d0, d1, d2, d3, d4, d5, d6, d7]);
        let e = u64::from_le_bytes([e0, e1, e2, e3, e4, e5, e6, e7]);
        let f = u64::from_le_bytes([f0, f1, f2, f3, f4, f5, f6, f7]);
        let g = u64::from_le_bytes([g0, g1, g2, g3, g4, g5, g6, g7]);
        let h = u64::from_le_bytes([h0, h1, h2, h3, h4, h5, h6, h7]);

        [a, b, c, d, e, f, g, h]
    }

    #[inline]
    pub const fn as_u128(&self) -> [u128; 4] {
        let [a0, a1, a2, a3, a4, a5, a6, a7, a8, a9, a10, a11, a12, a13, a14, a15, b0, b1, b2, b3, b4, b5, b6, b7, b8, b9, b10, b11, b12, b13, b14, b15, c0, c1, c2, c3, c4, c5, c6, c7, c8, c9, c10, c11, c12, c13, c14, c15, d0, d1, d2, d3, d4, d5, d6, d7, d8, d9, d10, d11, d12, d13, d14, d15] =
            self.0;

        let a = u128::from_le_bytes([
            a0, a1, a2, a3, a4, a5, a6, a7, a8, a9, a10, a11, a12, a13, a14, a15,
        ]);
        let b = u128::from_le_bytes([
            b0, b1, b2, b3, b4, b5, b6, b7, b8, b9, b10, b11, b12, b13, b14, b15,
        ]);
        let c = u128::from_le_bytes([
            c0, c1, c2, c3, c4, c5, c6, c7, c8, c9, c10, c11, c12, c13, c14, c15,
        ]);
        let d = u128::from_le_bytes([
            d0, d1, d2, d3, d4, d5, d6, d7, d8, d9, d10, d11, d12, d13, d14, d15,
        ]);

        [a, b, c, d]
    }
}

#[cfg(target_endian = "little")]
impl Hash512 {
    pub const fn as_u16(&self) -> [u16; 32] {
        unsafe { core::mem::transmute_copy(&self.0) }
    }

    pub const fn as_u32(&self) -> [u32; 16] {
        unsafe { core::mem::transmute_copy(&self.0) }
    }

    #[inline]
    pub const fn as_u64(&self) -> [u64; 8] {
        unsafe { core::mem::transmute_copy(&self.0) }
    }

    #[inline]
    pub const fn as_u128(&self) -> [u128; 4] {
        unsafe { core::mem::transmute_copy(&self.0) }
    }
}
