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

pub struct AssetsLoader {
    task_queue: TaskQueue<AssetRequest, Result<Option<AssetData>, AssetError>>,
}

enum AssetRequest {
    Load { id: AssetId },
    Update { id: AssetId, version: u64 },
}

impl Loader for AssetsLoader {
    fn load<'a>(&'a self, id: AssetId) -> BoxFuture<'a, Result<AssetData, AssetError>> {
        let response = self.task_queue.push(AssetRequest::Load { id });
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
        let response = self.task_queue.push(AssetRequest::Update { id, version });
        Box::pin(response)
    }
}
