//! Joke is for laugh

use std::{
    any::{Any, TypeId},
    error::Error,
    future::Future,
    num::NonZeroU64,
    sync::Arc,
};

use hashbrown::HashMap;
use parking_lot::Mutex;
pub use tokio::io::AsyncRead;

/// Somewhat unique asset id.
///
/// The loader uses this id to identify assets.
/// Users should treat this id as opaque.
/// Asset sources should use this id to identify asset data.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[repr(transparent)]
pub struct AssetId(NonZeroU64);

impl AssetId {
    /// Create asset id instance.
    pub fn new(id: u64) -> Option<Self> {
        NonZeroU64::new(id).map(AssetId)
    }
}

/// Assets are values constructible from data.
///
/// On construction asset takes asynchronous bytes stream,
/// optionally requests other assets
/// and asynchronously produces a value that can be built into asset type
/// using build context.
pub trait Asset {
    /// Error type that represents any error that may occur during
    /// asset loading and building.
    type Error;

    /// Type that represents loaded but not yet built asset.
    type Loaded;

    /// Future produced by [`load`](Asset::load) method.
    /// Awaiting this future produces result with
    /// [`Loaded`](Asset::Loaded) value ok [`Error`](Asset::Error)
    type Load: Future<Output = Result<Self::Loaded, Self::Error>>;

    /// Named kind of the asset.
    /// Use this name for displaying asset type to the user.
    #[must_use]
    fn kind() -> &'static str;

    /// Load asset from asynchronous byte stream.
    #[must_use]
    fn load<R>(data: R, loader: Loader) -> Self::Load
    where
        R: AsyncRead;

    /// Build loaded asset using build-context.
    #[must_use]
    #[inline]
    fn build<B>(loaded: Self::Loaded, context: &mut B) -> Self
    where
        Self: Sized,
        B: BuildContext<Self>,
    {
        <B as BuildContext<Self>>::build(context, loaded)
    }
}

/// Build context trait for specific asset type.
pub trait BuildContext<A: Asset> {
    #[must_use]
    fn build(&mut self, loaded: A::Loaded) -> A;
}

/// Stateless build context when no real build context is needed.
pub struct NoBuildContext;

pub struct AssetInfo {
    pub id: AssetId,
}

/// Interface for various asset sources.
/// [`Loader`] uses implementations of this trait to search for assets.
pub trait Source {
    type Error: Error;
    type Stream: AsyncRead;
    type Load: Future<Output = Result<Self::Stream, Self::Error>>;

    fn load(&self, id: AssetId) -> Self::Load;
}

trait AnySource {
    fn load(
        &self,
        id: AssetId,
    ) -> Box<dyn Future<Output = Result<Box<dyn AsyncRead>, Box<dyn Error>>>>;
}

impl<S> AnySource for S
where
    S: Source,
    S::Error: 'static,
    S::Stream: 'static,
    S::Load: 'static,
{
    fn load(
        &self,
        id: AssetId,
    ) -> Box<dyn Future<Output = Result<Box<dyn AsyncRead>, Box<dyn Error>>>> {
        let load = self.load(id);
        Box::new(async move {
            let stream = load.await?;
            Ok(Box::new(stream) as Box<dyn AsyncRead>)
        })
    }
}

/// Asset loader contains asset sources,
/// searches for assets, drives loading and building process.
pub struct Loader {
    inner: Arc<LoaderInner>,
}

pub struct Handle<A> {}

enum AssetCache<A> {
    Loaded(A),
}

struct LoaderInner {
    /// List of available sources.
    sources: Vec<Box<dyn AnySource>>,

    // Any is `AssetCache<A>` where `TypeId::of::<A>()` is in the key.
    // Index is chosen by key's hash.
    cached: Vec<Mutex<HashMap<(AssetId, TypeId), Box<dyn Any>>>>,
}
