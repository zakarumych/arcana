use std::{collections::VecDeque, fmt, sync::Arc};

use ash::vk;
use parking_lot::Mutex;

use crate::generic::{OutOfMemory, PipelineStages, QueueError, QueueFlags};

use super::{
    device::Device, from::IntoAsh, handle_host_oom, refs::Refs, unexpected_error, CommandBuffer,
    CommandEncoder,
};

#[derive(Clone)]
pub(super) struct PendingEpochs {
    queue: Arc<Mutex<VecDeque<(ash::vk::Fence, Vec<Refs>)>>>,
}

impl PendingEpochs {
    pub fn new() -> Self {
        PendingEpochs {
            queue: Arc::new(Mutex::new(VecDeque::new())),
        }
    }

    pub(super) fn clear(&self, device: &ash::Device) {
        self.queue.lock().drain(..).for_each(|(fence, _)| unsafe {
            device.destroy_fence(fence, None);
        })
    }
}

pub struct Queue {
    device: Device,
    handle: vk::Queue,
    family: u32,
    flags: QueueFlags,
    pool: vk::CommandPool,

    // Waits to add into next submission
    wait_semaphores: Vec<vk::Semaphore>,
    wait_stages: Vec<vk::PipelineStageFlags>,

    next_epoch: Vec<Refs>,
    free_refs: Vec<Refs>,

    epochs: PendingEpochs,
}

impl Drop for Queue {
    fn drop(&mut self) {
        unsafe {
            self.device.ash().queue_wait_idle(self.handle).unwrap();
        }

        self.epochs.clear(self.device.ash());

        unsafe {
            self.device.ash().destroy_command_pool(self.pool, None);
        }
    }
}

impl fmt::Debug for Queue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Queue({:p}@{:?})", self.handle, self.device)
    }
}

impl Queue {
    pub(super) fn new(
        device: Device,
        handle: vk::Queue,
        flags: QueueFlags,
        family: u32,
        epochs: PendingEpochs,
    ) -> Self {
        Queue {
            device,
            handle,
            flags,
            family,
            pool: vk::CommandPool::null(),
            wait_semaphores: Vec::new(),
            wait_stages: Vec::new(),

            next_epoch: Vec::new(),
            free_refs: Vec::new(),

            epochs,
        }
    }

    pub(super) fn add_wait(&mut self, semaphores: vk::Semaphore, before: PipelineStages) {
        self.wait_semaphores.push(semaphores);
        self.wait_stages
            .push(ash::vk::PipelineStageFlags::TOP_OF_PIPE | before.into_ash());
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

    fn next_check_point(&mut self) -> ash::vk::Fence {
        let mut epochs = self.epochs.queue.lock();
        if epochs.len() >= 3 {
            let (fence, mut refs) = epochs.pop_front().unwrap();

            unsafe {
                self.device
                    .ash()
                    .wait_for_fences(&[fence], true, !0)
                    .unwrap();

                self.device.ash().reset_fences(&[fence]).unwrap();
            }

            refs.iter_mut().for_each(Refs::clear);
            self.free_refs.append(&mut refs);

            let next_epoch = std::mem::take(&mut self.next_epoch);
            epochs.push_back((fence, next_epoch));
            fence
        } else {
            let fence = unsafe {
                self.device
                    .ash()
                    .create_fence(&ash::vk::FenceCreateInfo::builder(), None)
            }
            .unwrap();

            let next_epoch = std::mem::take(&mut self.next_epoch);
            epochs.push_back((fence, next_epoch));
            fence
        }
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

    fn submit<I>(&mut self, command_buffers: I, check_point: bool) -> Result<(), QueueError>
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

        let fence = if check_point {
            self.next_check_point()
        } else {
            ash::vk::Fence::null()
        };

        let result = unsafe {
            self.device.ash().queue_submit(
                self.handle,
                &[vk::SubmitInfo::builder()
                    .wait_semaphores(&self.wait_semaphores)
                    .wait_dst_stage_mask(&self.wait_stages)
                    .signal_semaphores(&present_semaphores)
                    .command_buffers(&command_buffer_handles)
                    .build()],
                fence,
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
}
