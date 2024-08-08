use std::{
    any::{Any, TypeId},
    fmt,
    sync::Arc,
};

/// Error type for asset loading.
#[derive(Clone)]
pub struct Error(Arc<dyn AssetError>);

trait AssetError: std::error::Error + Any + Send + Sync {}

impl<E> AssetError for E where E: std::error::Error + Any + Send + Sync {}

impl fmt::Debug for Error {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&*self.0, f)
    }
}

impl fmt::Display for Error {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&*self.0, f)
    }
}

impl std::error::Error for Error {
    #[inline]
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        self.0.source()
    }
}

struct Message {
    msg: &'static str,
}

impl fmt::Debug for Message {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(self.msg, f)
    }
}

impl fmt::Display for Message {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self.msg, f)
    }
}

impl std::error::Error for Message {}

impl Error {
    pub fn new<E>(error: E) -> Self
    where
        E: std::error::Error + Any + Send + Sync,
    {
        Error(Arc::new(error))
    }

    pub fn msg(msg: &'static str) -> Self {
        Error(Arc::new(Message { msg }))
    }

    #[inline]
    pub fn type_id(&self) -> TypeId {
        self.0.type_id()
    }

    #[inline]
    pub fn is<T>(&self) -> bool
    where
        T: Any,
    {
        self.0.type_id() == TypeId::of::<T>()
    }
}

/// Error type for when an asset is not found.
#[derive(Clone, Copy, Debug, thiserror::Error)]
#[error("Asset not found")]
pub struct NotFound;
