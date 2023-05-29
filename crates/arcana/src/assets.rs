use std::path::Path;

pub use argosy::{
    proc::{Asset, AssetField},
    AssetId,
};
use argosy::{AssetDriver, AssetFuture, DriveAsset, LoadedAssetDriver};

pub trait Asset: argosy::Asset + for<'a> argosy::AssetBuild<BobBuilder<'a>> {}
impl<A> Asset for A where A: argosy::Asset + for<'a> argosy::AssetBuild<BobBuilder<'a>> {}

pub struct Assets {
    loader: argosy::Loader,

    load_queue: Vec<AssetDriver<Bob>>,
    build_queue: Vec<LoadedAssetDriver<Bob>>,
}

/// Builder for a Bob asset.
/// This is required to build graphics assets.
#[cfg(feature = "client")]
pub struct BobBuilder<'a> {
    pub device: &'a mev::Device,
    pub encoder: mev::CopyCommandEncoder<'a>,
}

/// Builder for a Bob asset.
/// This is required to build graphics assets.
#[cfg(not(feature = "client"))]
pub struct BobBuilder<'a> {
    marker: std::marker::PhantomData<&'a ()>,
}

struct Bob;

impl DriveAsset for Bob {
    type Builder<'a> = BobBuilder<'a>;
}

impl Assets {
    pub fn new() -> Self {
        // let store_path = Path::new(env!("CARGO_MANIFEST_DIR"))
        //     .parent()
        //     .unwrap()
        //     .join("argosy.toml");

        // if !store_path.exists() {
        //     let info = argosy_store::StoreInfo {
        //         artifacts: Some(Path::new("target/artifacts").to_owned()),
        //         external: Some(Path::new("target/external").to_owned()),
        //         temp: Some(Path::new("target/temp").to_owned()),
        //         importers: vec![Path::new("target/debug/arcana_importers").to_owned()],
        //     };
        //     info.write(&store_path).unwrap();
        // }

        // let store = argosy_store::Store::find(&store_path).unwrap();

        let loader = argosy::Loader::builder()
            // .with(store)
            .build();

        Assets {
            loader,
            load_queue: Vec::new(),
            build_queue: Vec::new(),
        }
    }

    #[cfg(feature = "client")]
    pub fn build(
        &mut self,
        device: &mev::Device,
        queue: &mut mev::Queue,
    ) -> Result<(), mev::QueueError> {
        self.load_queue
            .retain_mut(|driver| match driver.poll_loaded() {
                None => true,
                Some(loaded) => {
                    self.build_queue.push(loaded);
                    false
                }
            });

        if self.build_queue.is_empty() {
            return Ok(());
        }

        let mut encoder = queue.new_command_encoder()?;
        let mut builder = BobBuilder {
            device,
            encoder: encoder.copy(),
        };

        for loaded in self.build_queue.drain(..) {
            loaded.build(&mut builder);
        }

        queue.submit(Some(encoder.finish()?), false)?;

        Ok(())
    }

    #[cfg(not(feature = "client"))]
    pub fn build(&mut self) -> Result<(), std::convert::Infallible> {
        self.load_queue
            .retain_mut(|driver| match driver.poll_loaded() {
                None => true,
                Some(loaded) => {
                    self.build_queue.push(loaded);
                    false
                }
            });

        if self.build_queue.is_empty() {
            return Ok(());
        }

        for loaded in self.build_queue.drain(..) {
            loaded.build(&mut BobBuilder {
                marker: std::marker::PhantomData,
            });
        }

        Ok(())
    }

    pub fn load_with_id<A>(&mut self, id: AssetId) -> AssetFuture<A>
    where
        A: Asset,
    {
        let handle = self.loader.load(id);
        let driver = handle.clone().driver::<Bob>();
        self.load_queue.push(driver);

        handle.ready()
    }
}
