use std::fmt;

use ash::vk;

use crate::generic::{OutOfMemory, QueueError, QueueFlags};

use super::{
    device::Device, handle_host_oom, refs::Refs, unexpected_error, CommandBuffer, CommandEncoder,
};

pub struct Queue {
    device: Device,
    handle: vk::Queue,
    family: u32,
    flags: QueueFlags,
    pool: vk::CommandPool,

    // Waits to add into next submission
    wait_semaphores: Vec<vk::Semaphore>,
    wait_stages: Vec<vk::PipelineStageFlags>,

    // epochs: Vec<(ash::vk::Fence, Vec<Refs>)>,
    next_epoch: Vec<Refs>,
    free_refs: Vec<Refs>,
}

impl fmt::Debug for Queue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Queue({:p}@{:?})", self.handle, self.device)
    }
}

impl Queue {
    pub(super) fn new(device: Device, handle: vk::Queue, flags: QueueFlags, family: u32) -> Self {
        Queue {
            device,
            handle,
            flags,
            family,
            pool: vk::CommandPool::null(),
            wait_semaphores: Vec::new(),
            wait_stages: Vec::new(),

            // epochs: Vec::new(),
            next_epoch: Vec::new(),
            free_refs: Vec::new(),
        }
    }

    pub(super) fn add_wait(&mut self, semaphores: vk::Semaphore, stages: vk::PipelineStageFlags) {
        self.wait_semaphores.push(semaphores);
        self.wait_stages.push(stages);
    }

    #[cold]
    #[inline(never)]
    fn init_pool(&mut self) -> Result<(), OutOfMemory> {
        let result = unsafe {
            self.device.ash().create_command_pool(
                &vk::CommandPoolCreateInfo::builder().flags(vk::CommandPoolCreateFlags::TRANSIENT),
                None,
            )
        };

        let pool = result.map_err(|err| match err {
            vk::Result::ERROR_OUT_OF_HOST_MEMORY => handle_host_oom(),
            vk::Result::ERROR_OUT_OF_DEVICE_MEMORY => OutOfMemory,
            _ => unexpected_error(err),
        })?;
        self.pool = pool;
        Ok(())
    }
}

#[hidden_trait::expose]
impl crate::traits::Queue for Queue {
    /// Get the queue family index.
    fn family(&self) -> u32 {
        self.family
    }

    fn new_command_encoder(&mut self) -> Result<CommandEncoder, OutOfMemory> {
        if self.pool == vk::CommandPool::null() {
            self.init_pool()?;
        }

        let mut cbuf = vk::CommandBuffer::null();

        let result = unsafe {
            (self.device.ash().fp_v1_0().allocate_command_buffers)(
                self.device.ash().handle(),
                &vk::CommandBufferAllocateInfo::builder()
                    .command_pool(self.pool)
                    .level(vk::CommandBufferLevel::PRIMARY)
                    .command_buffer_count(1)
                    .build(),
                &mut cbuf,
            )
        };

        match result {
            vk::Result::SUCCESS => {}
            vk::Result::ERROR_OUT_OF_HOST_MEMORY => handle_host_oom(),
            vk::Result::ERROR_OUT_OF_DEVICE_MEMORY => return Err(OutOfMemory),
            err => unexpected_error(err),
        }

        let result = unsafe {
            self.device.ash().begin_command_buffer(
                cbuf,
                &vk::CommandBufferBeginInfo::builder()
                    .flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT),
            )
        };

        result.map_err(|err| match err {
            vk::Result::ERROR_OUT_OF_HOST_MEMORY => handle_host_oom(),
            vk::Result::ERROR_OUT_OF_DEVICE_MEMORY => OutOfMemory,
            _ => unexpected_error(err),
        })?;

        Ok(CommandEncoder::new(
            self.device.clone(),
            cbuf,
            self.free_refs.pop().unwrap_or_else(Refs::new),
        ))
    }

    fn submit<I>(&mut self, command_buffers: I) -> Result<(), QueueError>
    where
        I: IntoIterator<Item = CommandBuffer>,
    {
        let mut command_buffer_handles = Vec::new();
        let mut present_semaphores = Vec::new();
        let mut present_swapchains = Vec::new();
        let mut present_indices = Vec::new();
        let mut do_present = false;

        for cbuf in command_buffers {
            let (handle, present, refs) = cbuf.deconstruct();
            self.next_epoch.push(refs);
            command_buffer_handles.push(handle);

            for frame in present {
                present_semaphores.push(frame.present());
                present_swapchains.push(frame.swapchain());
                present_indices.push(frame.image_idx());
                do_present = true;
            }
        }

        let result = unsafe {
            self.device.ash().queue_submit(
                self.handle,
                &[vk::SubmitInfo::builder()
                    .wait_semaphores(&self.wait_semaphores)
                    .wait_dst_stage_mask(&self.wait_stages)
                    .signal_semaphores(&present_semaphores)
                    .command_buffers(&command_buffer_handles)
                    .build()],
                vk::Fence::null(),
            )
        };

        self.wait_semaphores.clear();
        self.wait_stages.clear();

        result.map_err(|err| match err {
            vk::Result::ERROR_OUT_OF_HOST_MEMORY => handle_host_oom(),
            vk::Result::ERROR_OUT_OF_DEVICE_MEMORY => QueueError::OutOfMemory,
            vk::Result::ERROR_DEVICE_LOST => QueueError::DeviceLost,
            _ => unexpected_error(err),
        })?;

        if do_present {
            let result = unsafe {
                self.device.swapchain().queue_present(
                    self.handle,
                    &vk::PresentInfoKHR::builder()
                        .wait_semaphores(&present_semaphores)
                        .swapchains(&present_swapchains)
                        .image_indices(&present_indices),
                )
            };

            match result {
                Ok(_) => {}
                Err(vk::Result::ERROR_OUT_OF_HOST_MEMORY) => handle_host_oom(),
                Err(vk::Result::ERROR_OUT_OF_DEVICE_MEMORY) => return Err(QueueError::OutOfMemory),
                Err(vk::Result::ERROR_DEVICE_LOST) => return Err(QueueError::DeviceLost),
                Err(
                    vk::Result::ERROR_OUT_OF_DATE_KHR
                    | vk::Result::ERROR_SURFACE_LOST_KHR
                    | vk::Result::ERROR_FULL_SCREEN_EXCLUSIVE_MODE_LOST_EXT,
                ) => {}
                Err(err) => unexpected_error(err),
            };
        }
        Ok(())
    }

    fn check_point(&mut self) {
        // TODO: This is not the best way to do this.
        // Better insert fences and check them.
        unsafe {
            self.device.ash().queue_wait_idle(self.handle).unwrap();
        }

        self.next_epoch.iter_mut().for_each(Refs::clear);
        self.free_refs.append(&mut self.next_epoch);
    }
}
