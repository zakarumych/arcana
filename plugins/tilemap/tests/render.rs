use std::{
    collections::HashMap,
    num::NonZeroU64,
    path::PathBuf,
    sync::{Arc, Mutex},
    task::Poll,
    time::Duration,
};

use arcana::{
    assets::{
        AssetBuildContext, AssetData, AssetError, AssetId, Assets, Loader, NotFound,
        import::{ImportContext, Importer, Missing},
    },
    athena::Vector2,
    ecs::world::World,
    graphics::ImageImporter,
    mev,
    model::{ColorModel, ColorValue, Model, Value},
    plugin::PluginsHub,
    render::{CurrentRenderer, NewColorImage},
    scene::dim2::{Camera, Transform},
    tokio,
    work_graph::{Edge, Image2D, Image2DReq, Job, JobIdx, PinId, TargetUpdateDesc, WorkGraph},
};
use futures::future::{BoxFuture, poll_fn};
use tilemap::{Map, TileMap};

fn id(value: u64) -> AssetId {
    AssetId::new(NonZeroU64::new(value).unwrap())
}

#[test]
fn tilemap_job_is_registered_as_an_image_update() {
    let plugin = tilemap::arcana_plugin::get();
    let jobs: Vec<_> = plugin
        .jobs()
        .iter()
        .filter(|info| info.name == arcana::name!(TileMap))
        .collect();
    assert_eq!(jobs.len(), 1);
    let info = jobs[0];
    assert_eq!(info.desc, TileMap::desc());
    assert!(info.desc.params.is_empty());
    assert_eq!(
        info.desc.updates,
        [TargetUpdateDesc::new::<Image2D>(arcana::name!(Target))]
    );
    assert!(info.desc.creates.is_empty());
    assert!(info.desc.reads.is_empty());
    assert!(
        info.location
            .as_ref()
            .is_some_and(|location| location.file.ends_with("render.rs"))
    );
    let mut world = World::new();
    let mut hub = PluginsHub::new();
    plugin.init(&mut world, &mut hub);
    assert!(hub.jobs.contains_key(&info.id));
}

#[derive(Default)]
struct Memory(Mutex<HashMap<AssetId, Vec<u8>>>);
impl Loader for Memory {
    fn load(&self, id: AssetId) -> BoxFuture<'_, Result<AssetData, AssetError>> {
        Box::pin(async move {
            self.0
                .lock()
                .unwrap()
                .get(&id)
                .map(|data| AssetData {
                    bytes: data.clone().into_boxed_slice(),
                    version: 0,
                })
                .ok_or_else(|| AssetError::new(NotFound))
        })
    }
    fn update(&self, _: AssetId, _: u64) -> BoxFuture<'_, Result<Option<AssetData>, AssetError>> {
        Box::pin(async { Ok(None) })
    }
}

struct Fixture(PathBuf);
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn imported_assets() -> (Fixture, Assets) {
    struct Dependencies;
    impl ImportContext for Dependencies {
        fn get_dependency(&mut self, source: &str, _: arcana::Ident) -> Result<AssetId, Missing> {
            match source {
                "tileset.json" => Ok(id(2)),
                "atlas.png" => Ok(id(3)),
                _ => Err(Missing),
            }
        }
        fn get_source(&mut self, _: &str) -> Result<PathBuf, Missing> {
            Err(Missing)
        }
    }
    let fixture =
        Fixture(std::env::temp_dir().join(format!("arcana-tilemap-render-{}", std::process::id())));
    std::fs::create_dir_all(&fixture.0).unwrap();
    let mut atlas = image::RgbaImage::new(8, 4);
    for y in 0..2 {
        for x in 0..2 {
            atlas.put_pixel(
                x,
                y,
                image::Rgba(if y == 0 {
                    [255, 0, 0, 255]
                } else {
                    [255, 255, 0, 255]
                }),
            );
            atlas.put_pixel(x + 2, y, image::Rgba([0, 255, 0, 255]));
            atlas.put_pixel(x + 4, y, image::Rgba([0, 0, 255, 128]));
        }
    }
    atlas.save(fixture.0.join("atlas.png")).unwrap();
    std::fs::write(
        fixture.0.join("map.json"),
        br#"{"tileset":"tileset.json","cells":[[1,0,2],[3,2,1]]}"#,
    )
    .unwrap();
    std::fs::write(fixture.0.join("tileset.json"), br#"{"image":"atlas.png","tile_size":[2,2],"tiles":[{"name":"red-yellow","region":[0,0,2,2]},{"name":"green","region":[2,0,2,2]},{"name":"blue-alpha","region":[4,0,2,2]}]}"#).unwrap();
    let loader = Arc::new(Memory::default());
    for (asset_id, source, importer) in [
        (id(1), "map.json", &tilemap::MapImporter as &dyn Importer),
        (
            id(2),
            "tileset.json",
            &tilemap::TilesetImporter as &dyn Importer,
        ),
        (id(3), "atlas.png", &ImageImporter as &dyn Importer),
    ] {
        let output = fixture.0.join(format!("{source}.asset"));
        importer
            .import(
                &fixture.0.join(source),
                &output,
                Value::Unit,
                &mut Dependencies,
            )
            .unwrap_or_else(|_| panic!("import {source}"));
        loader
            .0
            .lock()
            .unwrap()
            .insert(asset_id, std::fs::read(output).unwrap());
    }
    let assets = Assets::new([loader as Arc<dyn Loader>]);
    (fixture, assets)
}

fn graph(tilemap_id: arcana::work_graph::JobId) -> WorkGraph {
    let Value::Color(color) = Model::Color(ColorModel::Srgba).default_value() else {
        unreachable!()
    };
    let mut color = color.into_srgba();
    color.red = 1.0;
    color.green = 1.0;
    color.blue = 1.0;
    color.alpha = 1.0;
    WorkGraph::new(
        [
            (
                JobIdx(0),
                (
                    NewColorImage::id(),
                    NewColorImage::desc(),
                    [(arcana::name!(Color), Value::Color(ColorValue::Srgba(color)))]
                        .into_iter()
                        .collect(),
                ),
            ),
            (JobIdx(1), (tilemap_id, TileMap::desc(), Default::default())),
        ]
        .into_iter()
        .collect(),
        [Edge {
            from: PinId {
                job: JobIdx(0),
                pin: 0,
            },
            to: PinId {
                job: JobIdx(1),
                pin: 0,
            },
        }]
        .into_iter()
        .collect(),
    )
    .unwrap()
}

fn expected_pixel(
    x: u32,
    y: u32,
    camera: Camera,
    transform: Transform,
    map_transforms: &[Transform],
) -> [u8; 4] {
    let width = camera.size.x.max(camera.size.y * 4.0 / 3.0);
    let height = camera.size.y.max(camera.size.x * 3.0 / 4.0);
    let p = transform.transform_point(Vector2::new(
        camera.center.x + ((x as f64 + 0.5) / 16.0 - 0.5) * width,
        camera.center.y + ((y as f64 + 0.5) / 12.0 - 0.5) * height,
    ));
    let mut result = [255; 4];
    for map_transform in map_transforms {
        let p = map_transform.inverse().transform_point(p);
        if p.x < 0.0 || p.y < 0.0 || p.x >= 6.0 || p.y >= 4.0 {
            continue;
        }
        let cells = [[3, 2, 1], [1, 0, 2]];
        let source = match cells[(p.y / 2.0).floor() as usize][(p.x / 2.0).floor() as usize] {
            0 => continue,
            1 if p.y.rem_euclid(2.0) < 1.0 => [255, 0, 0, 255],
            1 => [255, 255, 0, 255],
            2 => [0, 255, 0, 255],
            3 => [0, 0, 255, 128],
            _ => unreachable!(),
        };
        for c in 0..3 {
            result[c] = ((source[c] as u32 * source[3] as u32
                + result[c] as u32 * (255 - source[3] as u32)
                + 127)
                / 255) as u8;
        }
    }
    result
}

#[test]
#[ignore = "requires a native graphics/compute GPU device and host-readable buffers"]
fn native_tilemap_pixels_follow_renderer_camera() {
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
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_time()
        .build()
        .unwrap();
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let (_fixture, assets) = imported_assets();
        let map = runtime.block_on(async {
            let pending = poll_fn(|cx| assets.poll::<Map>(id(1), cx));
            futures::pin_mut!(pending);
            tokio::time::timeout(Duration::from_secs(30), async {
                loop {
                    if let Poll::Ready(result) = futures::poll!(&mut pending) {
                        break result.unwrap();
                    }
                    context.build_assets(&assets, &mut gpu).unwrap();
                    tokio::task::yield_now().await;
                }
            })
            .await
            .expect("map dependencies must build")
        });
        let _runtime = runtime.enter();
        let mut world = World::new();
        let map_entity = world.spawn((map.clone(),)).id();
        let camera = Camera {
            center: Vector2::new(3.0, 2.0),
            size: Vector2::new(8.0, 6.0),
        };
        let owner = world.spawn((camera, Transform::default())).id();
        let other = world
            .spawn((Camera {
                center: Vector2::new(100.0, 100.0),
                ..camera
            },))
            .id();
        world.insert_resource(CurrentRenderer { entity: owner });
        let plugin = tilemap::arcana_plugin::get();
        let tilemap_id = plugin
            .jobs()
            .iter()
            .find(|info| info.name == arcana::name!(TileMap))
            .unwrap()
            .id;
        let mut hub = PluginsHub::new();
        arcana::render::register_builtin_jobs(&mut hub);
        plugin.init(&mut world, &mut hub);
        // RGBA8 is supported as a color attachment on the native test device.
        let extent = mev::Extent2::new(16, 12);
        let format = mev::PixelFormat::Rgba8Unorm;
        let image = gpu.device.new_image(mev::ImageDesc {
            dims: extent,
            format,
            usage: mev::ImageUsage::TARGET | mev::ImageUsage::TRANSFER_SRC,
            levels: 1,
            name: "TileMap test target",
        });
        let mut readback = gpu.device.new_buffer(mev::BufferDesc {
            size: 16 * 12 * 4,
            usage: mev::BufferUsage::TRANSFER_DST | mev::BufferUsage::HOST_READ,
            name: "TileMap test readback",
        });
        let mut graph = graph(tilemap_id);
        graph.set_sink(
            PinId {
                job: JobIdx(1),
                pin: 0,
            },
            Image2D(image.clone()),
            Image2DReq::from_image(&image),
        );
        let mut check = |graph: &mut WorkGraph,
                         world: &mut World,
                         expected: Option<(Camera, Transform, &[Transform])>,
                         label: &str| {
            graph.run(&mut gpu.queue, world, &mut hub.jobs).unwrap();
            let mut encoder = gpu.queue.new_command_encoder();
            encoder.barrier(mev::PipelineStages::all(), mev::PipelineStages::TRANSFER);
            encoder.copy().copy_image_to_buffer(
                &image,
                &readback,
                mev::TexelShape::new(format, extent),
            );
            gpu.queue.submit_checkpoint([encoder.finish()]).unwrap();
            gpu.queue.wait_idle().unwrap();
            let mut bytes = [0; 16 * 12 * 4];
            readback.read(0, &mut bytes).unwrap();
            for y in 0..12 {
                for x in 0..16 {
                    let reference = expected.map_or([255; 4], |(camera, transform, maps)| {
                        expected_pixel(x, y, camera, transform, maps)
                    });
                    let offset = ((y * 16 + x) * 4) as usize;
                    for c in 0..4 {
                        assert!(
                            (bytes[offset + c] as i16 - reference[c] as i16).abs() <= 1,
                            "{label}: pixel ({x}, {y}), got {:?}, expected {reference:?}",
                            &bytes[offset..offset + 4]
                        );
                    }
                }
            }
        };
        for (label, camera, transform) in [
            ("aspect and rows", camera, Transform::default()),
            ("repeat", camera, Transform::default()),
            (
                "pan",
                Camera {
                    center: Vector2::new(4.0, 3.0),
                    ..camera
                },
                Transform::default(),
            ),
            (
                "zoom",
                Camera {
                    size: Vector2::new(4.0, 3.0),
                    ..camera
                },
                Transform::default(),
            ),
            (
                "rotation",
                camera,
                Transform::new(Vector2::new(5.0, 0.0), std::f64::consts::FRAC_PI_2),
            ),
        ] {
            world.insert(owner, camera).unwrap();
            world.insert(owner, transform).unwrap();
            check(
                &mut graph,
                &mut world,
                Some((camera, transform, &[Transform::default()])),
                label,
            );
        }
        world.insert_resource(CurrentRenderer { entity: other });
        check(&mut graph, &mut world, None, "renderer owner switch");
        world.insert_resource(CurrentRenderer { entity: owner });
        world.remove::<Camera>(owner).unwrap();
        check(&mut graph, &mut world, None, "missing camera");
        world
            .insert(
                owner,
                Camera {
                    size: Vector2::new(0.0, 1.0),
                    ..camera
                },
            )
            .unwrap();
        check(&mut graph, &mut world, None, "invalid camera");
        world.insert(owner, camera).unwrap();
        world.insert(owner, Transform::default()).unwrap();
        world.remove_resource::<CurrentRenderer>();
        check(&mut graph, &mut world, None, "missing renderer");
        world.insert_resource(CurrentRenderer { entity: owner });
        let shifted = Transform::new(Vector2::new(1.0, -1.0), 0.0);
        world.insert(map_entity, shifted).unwrap();
        check(
            &mut graph,
            &mut world,
            Some((camera, Transform::default(), &[shifted])),
            "map transform",
        );
        world.remove::<Transform>(map_entity).unwrap();
        let second_transform = Transform::new(Vector2::new(8.0, 0.0), 0.0);
        let second_map = world.spawn((map.clone(), second_transform)).id();
        let wide_camera = Camera {
            center: Vector2::new(7.0, 2.0),
            size: Vector2::new(16.0, 12.0),
        };
        world.insert(owner, wide_camera).unwrap();
        check(
            &mut graph,
            &mut world,
            Some((
                wide_camera,
                Transform::default(),
                &[Transform::default(), second_transform],
            )),
            "multiple map entities",
        );
        let overlap = Transform::new(Vector2::new(2.0, 0.0), 0.0);
        world.insert(second_map, overlap).unwrap();
        world.insert(owner, camera).unwrap();
        check(
            &mut graph,
            &mut world,
            Some((
                camera,
                Transform::default(),
                &[Transform::default(), overlap],
            )),
            "entity order and overlapping alpha",
        );
        world.despawn(map_entity).unwrap();
        world.despawn(second_map).unwrap();
        check(&mut graph, &mut world, None, "no map entities");
        assets.invalidate_all();
    }));
    let flush = context.flush(&mut gpu);
    drop(context);
    drop(runtime);
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
