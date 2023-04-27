use super::out_of_bounds;

#[derive(Clone)]
pub struct Buffer {
    buffer: metal::Buffer,
}

impl Buffer {
    pub(super) fn new(buffer: metal::Buffer) -> Self {
        Buffer { buffer }
    }

    pub(super) fn metal(&self) -> &metal::Buffer {
        &self.buffer
    }
}

unsafe impl Send for Buffer {}

#[hidden_trait::expose]
impl crate::traits::Buffer for Buffer {
    #[inline(always)]
    unsafe fn write_unchecked(&self, offset: u64, data: &[u8]) {
        let length = self.buffer.length();
        let fits = match u64::try_from(data.len()) {
            Ok(len) => match offset.checked_add(len) {
                Some(end) => end <= length,
                None => false,
            },
            Err(_) => false,
        };
        if !fits {
            out_of_bounds();
        }
        unsafe {
            let ptr = self.buffer.contents().add(offset as usize);
            ptr.cast::<u8>()
                .copy_from_nonoverlapping(data.as_ptr(), data.len());
            self.buffer.did_modify_range(metal::NSRange {
                location: offset as _,
                length: data.len() as _,
            })
        }
    }
}

/// Buffer device id.
/// In shader code it can be used to access buffer's contents.
#[repr(transparent)]
pub struct BufferId(u64);

impl crate::private::Sealed for BufferId {}
impl crate::traits::Argument for BufferId {
    const KIND: crate::generic::ArgumentKind = crate::generic::ArgumentKind::Buffer;
}

impl<const N: usize> crate::private::Sealed for [BufferId; N] {}
impl<const N: usize> crate::traits::Argument for [BufferId; N] {
    const KIND: crate::generic::ArgumentKind = crate::generic::ArgumentKind::Buffer;
}

impl crate::private::Sealed for [BufferId] {}
impl crate::traits::Argument for [BufferId] {
    const KIND: crate::generic::ArgumentKind = crate::generic::ArgumentKind::Buffer;
}
