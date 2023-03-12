pub struct Queue {
    queue: metal::CommandQueue,
}

impl Queue {
    pub(super) fn new(queue: metal::CommandQueue) -> Self {
        Queue { queue }
    }
}
