// use std::mem::size_of_val;

// use crate::generic::{ArgumentKind, ArgumentLayout, DataType};

// use super::{Buffer, Image, Sampler};

// pub(super) enum MetalArgument<'a> {
//     Buffer(&'a [Buffer]),
//     Image(&'a [Image]),
//     Sampler(&'a [Sampler]),
//     Constant(&'a [u8], DataType),
// }

// impl dyn AnyArgument + '_ {
//     pub(super) fn metal(&self, layout: ArgumentLayout) -> MetalArgument {
//         // Safety:
//         // `Argument` trait is sealed and
//         // implemented only for `Buffer`, `Image`, `Sampler`, byte and arrays of those.
//         // For each of these cases the cast below is valid,
//         // since slice `[T]` of length N can be created from pointer to `[T; N]`,
//         // and `T` and `[T; 1]` have the same layout.
//         unsafe {
//             match layout.kind {
//                 ArgumentKind::Buffer => MetalArgument::Buffer(std::slice::from_raw_parts(
//                     self as *const _ as *const Buffer,
//                     self.len(),
//                 )),
//                 ArgumentKind::Image => MetalArgument::Image(std::slice::from_raw_parts(
//                     self as *const _ as *const Image,
//                     self.len(),
//                 )),
//                 ArgumentKind::Sampler => MetalArgument::Sampler(std::slice::from_raw_parts(
//                     self as *const _ as *const Sampler,
//                     self.len(),
//                 )),
//                 ArgumentKind::Constant(data_type) => {
//                     let ptr = self as *const _ as *const u8;
//                     let size = size_of_val(self);
//                     debug_assert_eq!(self.len() * data_type.size(), size);
//                     MetalArgument::Constant(std::slice::from_raw_parts(ptr, size), data_type)
//                 }
//             }
//         }
//     }
// }
