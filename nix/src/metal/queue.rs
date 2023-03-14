use crate::generic::{OutOfMemory, QueueError};

use super::{CommandBuffer, CommandEncoder, SurfaceImage};

pub struct Queue {
    queue: metal::CommandQueue,
}

unsafe impl Send for Queue {}
unsafe impl Sync for Queue {}

impl Queue {
    pub(super) fn new(queue: metal::CommandQueue) -> Self {
        Queue { queue }
    }
}

#[hidden_trait::expose]
impl crate::traits::Queue for Queue {
    fn new_command_encoder(&mut self) -> Result<CommandEncoder, OutOfMemory> {
        Ok(CommandEncoder::new(
            self.queue.new_command_buffer().to_owned(),
        ))
    }

    fn submit<I>(&mut self, command_buffers: I) -> Result<(), QueueError>
    where
        I: IntoIterator<Item = CommandBuffer>,
    {
        for buffer in command_buffers {
            buffer.commit();
        }
        Ok(())
    }

    fn present(&mut self, surface: SurfaceImage) -> Result<(), QueueError> {
        surface.present();
        Ok(())
    }
}
