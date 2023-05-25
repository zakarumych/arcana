use alkahest::alkahest;
pub use bob::{
    events::{Event, EventLoop},
    game::Game,
    gametime::{Clock, FrequencyNumExt},
    init_nix,
    parking_lot::Mutex,
    plugin::{BobPlugin, PluginHub},
    // tokio,
    winit::window::WindowId,
};
// use tokio::io::{AsyncReadExt, AsyncWriteExt};

use std::{
    io::{BufRead, BufReader, Read, Write},
    net::{Ipv4Addr, TcpListener, TcpStream},
    path::Path,
    process::Child,
    sync::Arc,
};
pub use std::{stringify, vec::Vec};

/// Version of the Ed-API library.
pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

#[alkahest(Formula, Serialize, Deserialize)]
enum EdMessage {
    ListPlugins,
    Launch { enable: Vec<(String, String)> },
    Step,
    Exit,
}

#[alkahest(Formula, Serialize, Deserialize)]
struct ListPluginsResponse {
    plugins: Vec<(String, Vec<String>)>,
}

pub fn run_ed_game(hub: PluginHub) {
    let (device, queue) = init_nix();
    let queue = Arc::new(Mutex::new(queue));
    let mut games: Vec<Game> = Vec::new();

    EventLoop::run(|events| async move {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();

        let local_addr = listener.local_addr().unwrap();
        println!("PORT={}", local_addr.port());

        let (stream, _) = listener.accept().unwrap();

        let clocks = Clock::new();
        let ticker = 120u32.hz().ticker(clocks.now());

        let mut stream = BufReader::new(stream);
        let mut buffer = [0; 4096];

        loop {
            let new_events = events
                .next(ticker.next_tick().map(|t| clocks.stamp_instant(t)))
                .await;

            for event in new_events {
                match event {
                    Event::WindowEvent { window_id, event } => {
                        for game in &mut games {
                            if game.window_id() == window_id {
                                game.on_event(Event::WindowEvent { window_id, event });
                                break;
                            }
                        }
                    }
                    _ => {}
                }
            }

            stream.read_exact(&mut buffer[..4]).unwrap();
            let packet_size = alkahest::read_packet_size::<EdMessage>(&buffer[..4]).unwrap();
            stream
                .read_exact(&mut buffer[4..][..packet_size - 4])
                .unwrap();

            let (message, _) =
                alkahest::read_packet::<EdMessage, EdMessage>(&buffer[..packet_size]).unwrap();

            match message {
                EdMessage::ListPlugins => {
                    let response = ListPluginsResponse {
                        plugins: hub.list(),
                    };
                    let packet_size = alkahest::write_packet_unchecked::<ListPluginsResponse, _>(
                        response,
                        &mut buffer,
                    );
                    stream.get_mut().write_all(&buffer[..packet_size]).unwrap();
                }
                EdMessage::Exit => {
                    return;
                }
                EdMessage::Launch { enable } => {
                    let game = Game::launch(&events, &hub, &enable, device.clone(), queue.clone());
                    games.push(game);
                }
                EdMessage::Step => {
                    for game in &mut games {
                        game.tick();
                    }
                }
            }

            games.retain(|game| !game.should_quit());
        }
    });
}

#[macro_export]
macro_rules! ed_main {
    ($($plugin_lib:ident),* $(,)?) => {
        fn main() {
            let mut hub = $crate::PluginHub::new();
            $(
                hub.add_plugins($crate::stringify!($plugin_lib), $plugin_lib::bob_plugins());
            )*

            $crate::run_ed_game(hub);
        }
    };
}

pub struct ProjectBinary {
    child: Child,
    stream: std::io::BufReader<TcpStream>,
}

impl ProjectBinary {
    pub fn new(path: &Path) -> Self {
        let mut child = std::process::Command::new(path)
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::inherit())
            .spawn()
            .unwrap();

        let mut stdout = std::io::BufReader::new(child.stdout.as_mut().unwrap());

        let port = loop {
            let mut line = String::new();
            stdout.read_line(&mut line).unwrap();

            if line.is_empty() {
                panic!("Failed to read port from game binary");
            }

            if let Some(port) = line.trim_end().strip_prefix("PORT=") {
                break u16::from_str_radix(port, 10).unwrap();
            }
        };

        let stream = std::net::TcpStream::connect((std::net::Ipv4Addr::LOCALHOST, port)).unwrap();
        let stream = std::io::BufReader::new(stream);

        ProjectBinary { child, stream }
    }

    pub fn list_plugins(&mut self) -> Vec<(String, Vec<String>)> {
        let mut buffer = [0; 4096];
        let packet_size =
            alkahest::write_packet_unchecked::<EdMessage, _>(EdMessage::ListPlugins, &mut buffer);

        self.stream
            .get_mut()
            .write_all(&buffer[..packet_size])
            .unwrap();

        self.stream.read_exact(&mut buffer[..4]).unwrap();
        let packet_size = alkahest::read_packet_size::<ListPluginsResponse>(&buffer[..4]).unwrap();

        self.stream
            .read_exact(&mut buffer[4..][..packet_size - 4])
            .unwrap();

        let (response, _) = alkahest::read_packet::<ListPluginsResponse, ListPluginsResponse>(
            &buffer[..packet_size],
        )
        .unwrap();
        response.plugins
    }

    pub fn launch<'a>(&mut self, enable: impl Iterator<Item = (&'a str, &'a str)>) {
        let mut buffer = [0; 4096];
        let packet_size = alkahest::write_packet_unchecked::<EdMessage, _>(
            EdMessage::Launch {
                enable: enable
                    .map(|(name, version)| (name.to_owned(), version.to_owned()))
                    .collect(),
            },
            &mut buffer,
        );

        self.stream
            .get_mut()
            .write_all(&buffer[..packet_size])
            .unwrap();
    }

    pub fn tick(&mut self) {
        let mut buffer = [0; 4096];
        let packet_size =
            alkahest::write_packet_unchecked::<EdMessage, _>(EdMessage::Step, &mut buffer);

        self.stream
            .get_mut()
            .write_all(&buffer[..packet_size])
            .unwrap();
    }

    pub fn exit(&mut self) {
        let mut buffer = [0; 4096];
        let packet_size =
            alkahest::write_packet_unchecked::<EdMessage, _>(EdMessage::Exit, &mut buffer);

        self.stream
            .get_mut()
            .write_all(&buffer[..packet_size])
            .unwrap();

        self.child.wait().unwrap();
    }
}
