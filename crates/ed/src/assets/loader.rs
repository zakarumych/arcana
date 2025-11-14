use std::{
    future::Future,
    pin::Pin,
    task::{Context, Poll},
};

use arcana::assets::{AssetData, AssetError, AssetId, Loader, NotFound};
use futures::future::BoxFuture;

use crate::task::{TaskQueue, WakerArray};

struct AssetDataRequest {
    wakers: WakerArray,
    data: Option<AssetData>,
}

impl Future for AssetDataRequest {
    type Output = AssetData;

    fn poll(self: Pin<&mut Self>, cx: &mut Context) -> Poll<AssetData> {
        let me = self.get_mut();
        if let Some(data) = me.data.take() {
            return Poll::Ready(data);
        }

        me.wakers.register(cx.waker());
        Poll::Pending
    }
}

pub struct RepositoryAssetLoader {
    task_queue: TaskQueue<AssetRequest, Result<Option<AssetData>, AssetError>>,
}

pub struct AssetRequest {
    id: AssetId,
    min_version: u64,
}

impl Loader for RepositoryAssetLoader {
    fn load<'a>(&'a self, id: AssetId) -> BoxFuture<'a, Result<AssetData, AssetError>> {
        let response = self.task_queue.push(AssetRequest { id, min_version: 0 });
        Box::pin(async move {
            match response.await {
                Ok(None) => Err(AssetError::new(NotFound)), // Shouldn't happen for Load requests.
                Ok(Some(data)) => Ok(data),
                Err(e) => Err(e),
            }
        })
    }

    fn update<'a>(
        &'a self,
        id: AssetId,
        version: u64,
    ) -> BoxFuture<'a, Result<Option<AssetData>, AssetError>> {
        let response = self.task_queue.push(AssetRequest {
            id,
            min_version: version.wrapping_add(1),
        });
        Box::pin(response)
    }
}

pub struct RepositoryAssetProvider {
    task_queue: TaskQueue<AssetRequest, Result<Option<AssetData>, AssetError>>,
}

impl RepositoryAssetProvider {
    pub fn new() -> Self {
        let task_queue = TaskQueue::new();
        Self { task_queue }
    }

    pub fn provide(
        &mut self,
        f: impl FnMut(AssetRequest) -> Result<Option<AssetData>, AssetError>,
    ) {
        self.task_queue.process_all(f);
    }

    pub fn loader(&self) -> RepositoryAssetLoader {
        RepositoryAssetLoader {
            task_queue: self.task_queue.clone(),
        }
    }
}
