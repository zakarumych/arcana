use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::Duration,
};

use arcana::{
    assets::{Asset, AssetData, AssetError, AssetId, Assets, Loader, NotFound},
    tokio,
};
use futures::future::{BoxFuture, poll_fn};
use tilemap::{Map, Tileset};

fn id(value: u64) -> AssetId {
    AssetId::new(std::num::NonZeroU64::new(value).unwrap())
}

#[derive(Default)]
struct Memory {
    data: Mutex<HashMap<AssetId, Vec<u8>>>,
    reads: Mutex<Vec<AssetId>>,
}

impl Memory {
    fn put(&self, id: AssetId, data: impl Into<Vec<u8>>) {
        self.data.lock().unwrap().insert(id, data.into());
    }
}

impl Loader for Memory {
    fn load(&self, id: AssetId) -> BoxFuture<'_, Result<AssetData, AssetError>> {
        Box::pin(async move {
            self.reads.lock().unwrap().push(id);
            self.data
                .lock()
                .unwrap()
                .get(&id)
                .map(|bytes| AssetData {
                    bytes: bytes.clone().into_boxed_slice(),
                    version: 0,
                })
                .ok_or_else(|| AssetError::new(NotFound))
        })
    }

    fn update(&self, _: AssetId, _: u64) -> BoxFuture<'_, Result<Option<AssetData>, AssetError>> {
        Box::pin(async { Ok(None) })
    }
}

fn runtime() -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_current_thread()
        .enable_time()
        .build()
        .unwrap()
}

const TILESET: &[u8] =
    br#"{"image":3,"tile_size":[16,16],"tiles":[{"name":"ground","region":[0,0,16,16]}]}"#;
const MAP: &[u8] = br#"{"tileset":2,"cells":[[0,1]]}"#;

#[test]
fn runtime_rejects_malformed_imported_artifacts_before_dependencies() {
    runtime().block_on(async {
        let loader = Arc::new(Memory::default());
        let assets = Assets::new([loader.clone() as Arc<dyn Loader>]);
        for bytes in [
            br#"{"tileset":"tileset.json","cells":[[1]]}"#.as_slice(),
            br#"{"tileset":0,"cells":[[1]]}"#,
            br#"{"tileset":2,"cells":[]}"#,
            br#"{"tileset":2,"cells":[[]]}"#,
            br#"{"tileset":2,"cells":[[1],[1,2]]}"#,
            br#"{"tileset":2,"cells":[[-1]]}"#,
            br#"{"tileset":2,"cells":[[4294967296]]}"#,
            br#"{"tileset":2,"cells":[[1]],"extra":1}"#,
        ] {
            assert!(Map::load(bytes.into(), &assets).await.is_err());
        }
        for bytes in [
            br#"{"image":"atlas.png","tile_size":[16,16],"tiles":[]}"#.as_slice(),
            br#"{"image":3,"tile_size":[16,16],"tiles":[]}"#,
            br#"{"image":3,"tile_size":[0,16],"tiles":[{"name":"a","region":[0,0,1,1]}]}"#,
            br#"{"image":3,"tile_size":[1e100,16],"tiles":[{"name":"a","region":[0,0,1,1]}]}"#,
            br#"{"image":3,"tile_size":[16,16],"tiles":[{"name":" ","region":[0,0,1,1]}]}"#,
            br#"{"image":3,"tile_size":[16,16],"tiles":[{"name":"a","region":[0,0,0,1]}]}"#,
            br#"{"image":3,"tile_size":[16,16],"tiles":[{"name":"a","region":[4294967295,0,1,1]}]}"#,
            br#"{"image":3,"tile_size":[16,16],"tiles":[{"name":"a","region":[0,0,1,1]},{"name":"a","region":[0,0,1,1]}]}"#,
            br#"{"image":3,"tile_size":[16,16],"tiles":[{"name":"a","region":[0,0,1,1]}],"extra":1}"#,
        ] {
            assert!(Tileset::load(bytes.into(), &assets).await.is_err());
        }
        assert!(loader.reads.lock().unwrap().is_empty());
    });
}

#[test]
fn runtime_dependency_errors_wake_map_waiters_and_remain_cached() {
    runtime().block_on(async {
        // Missing tileset, malformed tileset, missing texture, malformed texture.
        for (tileset, texture, expected_reads, message) in [
            (None, None, vec![id(1), id(2)], "Asset not found"),
            (Some(b"{".as_slice()), None, vec![id(1), id(2)], "EOF"),
            (
                Some(TILESET),
                None,
                vec![id(1), id(2), id(3)],
                "Asset not found",
            ),
            (
                Some(TILESET),
                Some(b"invalid JKLI".as_slice()),
                vec![id(1), id(2), id(3)],
                "Invalid JKLI",
            ),
        ] {
            let loader = Arc::new(Memory::default());
            loader.put(id(1), MAP);
            if let Some(bytes) = tileset {
                loader.put(id(2), bytes);
            }
            if let Some(bytes) = texture {
                loader.put(id(3), bytes);
            }
            let assets = Assets::new([loader.clone() as Arc<dyn Loader>]);
            for _ in 0..2 {
                let result = tokio::time::timeout(
                    Duration::from_secs(5),
                    poll_fn(|cx| assets.poll::<Map>(id(1), cx)),
                )
                .await
                .expect("map waiter must wake");
                let error = result.err().expect("dependency must fail");
                assert!(error.to_string().contains(message), "{error}");
            }
            assert_eq!(*loader.reads.lock().unwrap(), expected_reads);
        }
    });
}

#[test]
#[ignore = "requires a native GPU device"]
fn native_imported_map_resolves_built_dependencies_and_reloads() {
    use arcana::{
        assets::{
            AssetBuildContext,
            import::{ImportContext, Importer, Missing},
        },
        graphics::{ImageImporter, texture::Texture},
        mev,
        model::Value,
    };
    use std::{
        path::{Path, PathBuf},
        task::Poll,
    };

    struct Dependencies;
    impl ImportContext for Dependencies {
        fn get_dependency(
            &mut self,
            source: &str,
            target: arcana::Ident,
        ) -> Result<AssetId, Missing> {
            match (source, target) {
                ("tileset.json", t) if t == arcana::ident!(Tileset) => Ok(id(2)),
                ("atlas.png", t) if t == arcana::ident!(Texture) => Ok(id(3)),
                _ => Err(Missing),
            }
        }
        fn get_source(&mut self, _: &str) -> Result<PathBuf, Missing> {
            Err(Missing)
        }
    }
    struct Temp(PathBuf);
    impl Drop for Temp {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    let temp =
        Temp(std::env::temp_dir().join(format!("arcana-tilemap-runtime-{}", std::process::id())));
    std::fs::create_dir_all(&temp.0).unwrap();
    let sources = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/tilemap");
    let map_path = temp.0.join("map.asset");
    let tileset_path = temp.0.join("tileset.asset");
    let texture_path = temp.0.join("texture.asset");
    tilemap::MapImporter
        .import(
            &sources.join("map.json"),
            &map_path,
            Value::Unit,
            &mut Dependencies,
        )
        .unwrap_or_else(|_| panic!("sample map import"));
    tilemap::TilesetImporter
        .import(
            &sources.join("tileset.json"),
            &tileset_path,
            Value::Unit,
            &mut Dependencies,
        )
        .unwrap_or_else(|_| panic!("sample tileset import"));
    ImageImporter
        .import(
            &sources.join("atlas.png"),
            &texture_path,
            Value::Unit,
            &mut Dependencies,
        )
        .unwrap_or_else(|_| panic!("sample image import"));
    let loader = Arc::new(Memory::default());
    let map_bytes = std::fs::read(map_path).unwrap();
    let tileset_bytes = std::fs::read(tileset_path).unwrap();
    loader.put(id(1), map_bytes.clone());
    loader.put(id(2), tileset_bytes.clone());
    loader.put(id(3), std::fs::read(texture_path).unwrap());
    let assets = Assets::new([loader.clone() as Arc<dyn Loader>]);
    let instance = mev::Instance::load().unwrap();
    let (idx, family) = instance
        .capabilities()
        .devices
        .iter()
        .enumerate()
        .find_map(|(idx, device)| {
            device
                .families
                .iter()
                .position(|family| {
                    family.queue_flags.contains(
                        mev::QueueFlags::GRAPHICS
                            | mev::QueueFlags::COMPUTE
                            | mev::QueueFlags::TRANSFER,
                    )
                })
                .map(|family| (idx, family as u32))
        })
        .expect("graphics/compute/transfer queue");
    let (_, mut queues) = instance
        .new_device(mev::DeviceDesc {
            idx,
            queues: &[family],
            features: mev::Features::BUFFER_MAP | mev::Features::BUFFER_MAP_AT_CREATION,
        })
        .unwrap();
    let mut gpu = arcana::Gpu::new(queues.pop().unwrap()).unwrap();
    let mut context = AssetBuildContext::new();
    let runtime = runtime();

    async fn request<A: Asset>(
        assets: &Assets,
        context: &mut AssetBuildContext,
        gpu: &mut arcana::Gpu,
    ) -> Result<A, arcana::error::Error> {
        let pending = poll_fn(|cx| assets.poll::<A>(id(1), cx));
        futures::pin_mut!(pending);
        tokio::time::timeout(Duration::from_secs(30), async {
            loop {
                if let Poll::Ready(result) = futures::poll!(&mut pending) {
                    return result;
                }
                context.build_assets(assets, gpu).unwrap();
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("asset chain must finish")
    }

    // Complete GPU cleanup even when an assertion or asset request fails.
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        runtime.block_on(async {
            let map = request::<Map>(&assets, &mut context, &mut gpu)
                .await
                .unwrap();
            let expected: tilemap::MapData = serde_json::from_slice(&map_bytes).unwrap();
            assert_eq!(map.data(), &expected);
            assert_eq!(map.data().tileset, id(2));
            assert_eq!(map.tileset().data().image, id(3));
            assert_eq!(map.tileset().data().tile_size, [16.0, 16.0]);
            assert_eq!(map.tileset().data().tiles[1].region, [16, 0, 16, 16]);
            assert!(map.tileset().texture().image.extent().width() >= 32);
            let Poll::Ready(Ok(tileset)) = assets.get::<Tileset>(id(2)) else {
                panic!("built tileset cached")
            };
            let Poll::Ready(Ok(texture)) = assets.get::<Texture>(id(3)) else {
                panic!("built texture cached")
            };
            assert!(std::ptr::eq(map.tileset().data(), tileset.data()));
            assert_eq!(map.tileset().texture().image, texture.image);
            let cached = request::<Map>(&assets, &mut context, &mut gpu)
                .await
                .unwrap();
            assert!(std::ptr::eq(map.data(), cached.data()));
            assert_eq!(*loader.reads.lock().unwrap(), [id(1), id(2), id(3)]);

            loader.put(id(1), br#"{"tileset":2,"cells":[[3]]}"#);
            assets.invalidate_all();
            let error = request::<Map>(&assets, &mut context, &mut gpu)
                .await
                .err()
                .expect("invalid tile index");
            assert!(error.to_string().contains("cell index"), "{error}");

            loader.put(id(1), map_bytes.clone());
            let mut bad: tilemap::TilesetData = serde_json::from_slice(&tileset_bytes).unwrap();
            bad.tiles[0].region[0] = texture.image.extent().width();
            loader.put(id(2), serde_json::to_vec(&bad).unwrap());
            assets.invalidate_all();
            let error = request::<Map>(&assets, &mut context, &mut gpu)
                .await
                .err()
                .expect("invalid atlas bounds");
            assert!(error.to_string().contains("texture bounds"), "{error}");

            loader.put(id(2), tileset_bytes.clone());
            assets.invalidate_all();
            let reloaded = request::<Map>(&assets, &mut context, &mut gpu)
                .await
                .unwrap();
            assert_eq!(reloaded.data(), map.data());
            assert!(!std::ptr::eq(reloaded.data(), map.data()));
            assert_eq!(map.tileset().texture().image, texture.image);
        })
    }));
    assets.invalidate_all();
    drop(assets);
    drop(runtime);
    let flush = context.flush(&mut gpu);
    drop(context);
    let arcana::Gpu {
        mut queue,
        uploader,
        cvt,
        device,
    } = gpu;
    drop(uploader);
    drop(cvt);
    let checkpoint = queue.submit_checkpoint([]);
    let idle = queue.wait_idle();
    drop(queue);
    drop(device);
    flush.unwrap();
    checkpoint.unwrap();
    idle.unwrap();
    if let Err(panic) = result {
        std::panic::resume_unwind(panic);
    }
}
