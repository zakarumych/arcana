use std::{
    mem::ManuallyDrop,
    path::{Path, PathBuf},
};
pub use std::{stringify, sync::Arc, vec::Vec};

use rand::Rng;

pub use crate::{
    events::{Event, EventLoop},
    game::Game,
    gametime::{Clock, FrequencyNumExt},
    init_mev,
    mev::{Device, Queue},
    parking_lot::Mutex,
    plugin::{BobPlugin, PluginHub},
    winit::window::WindowId,
};

#[macro_export]
macro_rules! ed_lib {
    ($($plugin_lib:ident),* $(,)?) => {
        #[no_mangle]
        pub unsafe extern "C" fn list_plugins(
        ) -> Vec<(&'static str, &'static [&'static dyn $crate::api::BobPlugin])> {
            vec![$(($crate::api::stringify!($plugin_lib), $plugin_lib::arcana_plugins())),*]
        }

        #[no_mangle]
        pub unsafe extern "C" fn launch(
            events: &$crate::api::EventLoop,
            plugins: &$crate::api::PluginHub,
            enable: &[(&str, &str)],
            device: &$crate::api::Device,
            queue: &$crate::api::Arc<$crate::api::Mutex<$crate::api::Queue>>,
        ) -> Box<dyn $crate::api::GameTrait> {
            #[repr(transparent)]
            struct ThisGame {
                game: $crate::api::Game,
            }

            impl $crate::api::GameTrait for ThisGame {
                fn window_id(&self) -> $crate::api::WindowId {
                    self.game.window_id()
                }

                fn on_event(&mut self, event: $crate::api::Event) -> Option<$crate::api::Event> {
                    self.game.on_event(event)
                }

                fn should_quit(&self) -> bool {
                    self.game.should_quit()
                }

                fn tick(&mut self) {
                    self.game.tick()
                }
            }

            Box::new(ThisGame {
                game: $crate::api::Game::launch(events, plugins, enable, device.clone(), queue.clone()),
            })
        }
    };
}

pub trait GameTrait {
    fn window_id(&self) -> WindowId;
    fn on_event(&mut self, event: Event) -> Option<Event>;
    fn should_quit(&self) -> bool;
    fn tick(&mut self);
}

type ListPluginsFn =
    unsafe extern "C" fn() -> Vec<(&'static str, &'static [&'static dyn BobPlugin])>;

type LaunchFn = unsafe extern "C" fn(
    events: &EventLoop,
    plugins: &PluginHub,
    enable: &[(&str, &str)],
    device: &Device,
    queue: &Arc<Mutex<Queue>>,
) -> Box<dyn GameTrait>;

pub struct ProjectLibrary {
    path: PathBuf,
    games: Vec<Box<dyn GameTrait>>,
    plugins_hub: PluginHub,
    launch_fn: LaunchFn,
    library: ManuallyDrop<libloading::Library>,
}

impl Drop for ProjectLibrary {
    fn drop(&mut self) {
        self.games.clear();

        unsafe {
            ManuallyDrop::drop(&mut self.library);
        }

        if let Err(err) = std::fs::remove_file(&self.path) {
            tracing::error!(
                "Failed to remove library at {}: {}",
                self.path.display(),
                err
            );
        }
    }
}

impl ProjectLibrary {
    pub fn new(name: &str, dir: &Path) -> miette::Result<Self> {
        let bin_name = format!(
            "{}{name}{}",
            std::env::consts::DLL_PREFIX,
            std::env::consts::DLL_SUFFIX
        );
        let bin_path = dir.join(&bin_name);

        let new_bin_path = tmp_bin_path(dir, &bin_path)?;

        // Safety: None
        let library = unsafe { libloading::Library::new(&new_bin_path) }.map_err(|err| {
            miette::miette!("Failed load library {}: {err}", new_bin_path.display())
        })?;

        let list_plugins_fn: ListPluginsFn = *unsafe { library.get(b"list_plugins\0") }
            .map_err(|err| miette::miette!("Failed to get `list_plugins` function: {err}"))?;

        let launch_fn = unsafe { library.get(b"launch\0") }
            .map_err(|err| miette::miette!("Failed to get `launch` function: {err}"))?;

        let mut plugins_hub = PluginHub::new();

        for (lib, plugins) in unsafe { list_plugins_fn() } {
            plugins_hub.add_plugins(lib, plugins);
        }

        Ok(ProjectLibrary {
            path: new_bin_path,
            plugins_hub,
            launch_fn: *launch_fn,
            library: ManuallyDrop::new(library),
            games: Vec::new(),
        })
    }

    pub fn list_plugins(&mut self) -> Vec<(String, Vec<String>)> {
        self.plugins_hub.list()
    }

    pub fn launch<'a>(
        &mut self,
        events: &EventLoop,
        device: &Device,
        queue: &Arc<Mutex<Queue>>,
        enable: impl Iterator<Item = (&'a str, &'a str)>,
    ) {
        let game = unsafe {
            (self.launch_fn)(
                &events,
                &self.plugins_hub,
                &enable.collect::<Vec<_>>(),
                device,
                queue,
            )
        };
        self.games.push(game);
    }

    pub fn on_event(&mut self, event: Event) -> Option<Event> {
        match event {
            Event::WindowEvent { window_id, event } => {
                for game in &mut self.games {
                    if game.window_id() == window_id {
                        return game.on_event(Event::WindowEvent { window_id, event });
                    }
                }
                Some(Event::WindowEvent { window_id, event })
            }
            _ => Some(event),
        }
    }

    pub fn tick(&mut self) {
        self.games.retain(|game| !game.should_quit());
        for game in &mut self.games {
            game.tick();
        }
    }

    pub fn exit(&mut self) {
        self.games.clear();
    }
}

fn tmp_bin_path(dir: &Path, path: &Path) -> miette::Result<PathBuf> {
    let mut rng = rand::thread_rng();
    let filename = path
        .file_name()
        .ok_or_else(|| miette::miette!("Failed to get filename from path: {}", path.display()))?;
    let filename = filename.to_string_lossy();
    loop {
        let r: u128 = rng.gen();
        let filename = format!("{filename}_{r:0X}");

        let tmp_path = dir.join(&filename);
        if tmp_path.exists() {
            continue;
        }

        std::fs::copy(path, &tmp_path).map_err(|err| {
            miette::miette!(
                "Failed to copy library from {} to {}: {err}",
                path.display(),
                tmp_path.display()
            )
        })?;

        return Ok(tmp_path);
    }
}
