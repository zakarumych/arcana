use std::{path::PathBuf, str::FromStr};

use arcana_launcher::Start;
use arcana_names::Ident;
use arcana_project::{Dependency, Profile};
use clap::{builder::TypedValueParser, Parser, Subcommand};

#[derive(Debug, Clone, serde::Deserialize)]
struct ArcanaArg {
    arcana: Dependency,
}

impl FromStr for ArcanaArg {
    type Err = toml::de::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let arg: ArcanaArg = toml::from_str(&format!("arcana = {s}"))?;
        Ok(arg)
    }
}

#[derive(Clone, Copy)]
struct IdentValueParser;

impl TypedValueParser for IdentValueParser {
    type Value = Ident;

    fn parse_ref(
        &self,
        _cmd: &clap::Command,
        _arg: Option<&clap::Arg>,
        value: &std::ffi::OsStr,
    ) -> Result<Self::Value, clap::Error> {
        let Some(s) = value.to_str() else {
            return Err(clap::Error::raw(
                clap::error::ErrorKind::InvalidUtf8,
                "Identifier is not UTF-8",
            ));
        };
        match Ident::from_str(s) {
            Ok(ident) => Ok(ident.to_owned()),
            Err(err) => Err(clap::Error::raw(clap::error::ErrorKind::InvalidValue, err)),
        }
    }
}

#[derive(Debug, Subcommand)]
#[command(rename_all = "kebab-case")]
enum Command {
    /// Initializes new project in an existing directory.
    Init {
        /// Path to the project directory.
        /// It may be either absolute or relative to the current directory.
        /// The directory may or may not exist.
        /// If it does exist, it must not already contain an Arcana Project.
        /// If it does not exist, it will be created.
        /// The directory must not be part of the cargo workspace.
        #[arg(value_name = "path", default_value = ".")]
        path: PathBuf,

        /// Name of the project.
        /// If not specified, the name of the project will be inferred from the directory name.
        #[arg(
            long = "name",
            value_name = "name",
            value_parser = IdentValueParser
        )]
        name: Option<Ident>,

        /// Arcana dependency.
        /// If not specified, the version of this CLI crate will be used.
        /// If specified this must be a string with valid toml syntax for a dependency.
        #[arg(long = "arcana", value_name = "arcana-dependency")]
        arcana: Option<ArcanaArg>,
    },
    /// Creates new project.
    New {
        /// Path to the project directory.
        /// It may be either absolute or relative to the current directory.
        /// The directory must not exist, it will be created.
        /// The directory must not be part of the cargo workspace.
        #[arg(value_name = "path")]
        path: PathBuf,

        /// Name of the project.
        /// If not specified, the name of the project will be inferred from the directory name.
        #[arg(long = "name", value_name = "name", value_parser = IdentValueParser)]
        name: Option<Ident>,

        /// Arcana dependency.
        /// If not specified, the version of this CLI crate will be used.
        /// If specified this must be a string with valid toml syntax for a dependency.
        #[arg(long = "arcana", value_name = "arcana-dependency")]
        arcana: Option<ArcanaArg>,
    },
    /// Initializes cargo workspace for an existing project.
    InitWorkspace {
        /// Path to the project directory.
        #[arg(value_name = "path", default_value = ".")]
        path: PathBuf,
    },
    /// Run Arcana Ed with the project.
    Ed {
        /// Path to the project directory.
        #[arg(value_name = "path", default_value = ".")]
        path: PathBuf,

        #[arg(value_name = "release")]
        release: bool,
    },
    /// Creates new plugin.
    NewPlugin {
        /// Path to the plugin directory.
        #[arg(value_name = "path", default_value = ".")]
        path: PathBuf,

        /// Name of the plugin.
        /// If not specified, the name of the plugin will be inferred from the directory name.
        #[arg(long = "name", value_name = "name", value_parser = IdentValueParser)]
        name: Option<Ident>,

        /// Arcana dependency.
        /// If not specified, the version of this CLI crate will be used.
        /// If specified this must be a string with valid toml syntax for a dependency.
        #[arg(long = "arcana", value_name = "arcana-dependency")]
        arcana: Option<ArcanaArg>,
    },
    /// Runs the game.
    Game {
        /// Path to the project directory.
        #[arg(value_name = "path", default_value = ".")]
        path: PathBuf,

        #[arg(value_name = "release")]
        release: bool,
    },
    /// Cooks game together with assets and all binaries.
    Cook {
        /// Path to the project directory.
        #[arg(value_name = "path", default_value = ".")]
        path: PathBuf,
    },
}

#[derive(Debug, Parser)]
#[command(name = "arcn")]
#[command(about = "Arcana game engine CLI")]
#[command(rename_all = "kebab-case")]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,
}

pub fn run_cli() -> miette::Result<()> {
    let cli = Cli::parse();
    let start = Start::new();

    match cli.command.unwrap_or_else(|| Command::Ed {
        path: PathBuf::from("."),
        release: false,
    }) {
        Command::Init { path, name, arcana } => {
            start.init(&path, name, pick_engine_version(&start, arcana), false)?;
        }
        Command::New { path, name, arcana } => {
            start.init(&path, name, pick_engine_version(&start, arcana), true)?;
        }
        Command::InitWorkspace { path } => {
            start.init_workspace(&path)?;
        }
        Command::Ed { path, release } => {
            start.run_ed(
                &path,
                if release {
                    Profile::Release
                } else {
                    Profile::Debug
                },
            )?;
        }
        Command::NewPlugin { path, name, arcana } => {
            start.new_plugin(&path, name, pick_engine_version(&start, arcana))?;
        }
        Command::Game { path, release } => {
            start.run_game(
                &path,
                if release {
                    Profile::Release
                } else {
                    Profile::Debug
                },
            )?;
        }
        Command::Cook { .. } => {
            unimplemented!()
            //     let path = start.build_game(&path)?;

            //     if run {
            //         tracing::info!("Game binary: {}", path.display());
            //         match std::process::Command::new(path).status() {
            //             Ok(status) => {
            //                 if !status.success() {
            //                     std::process::exit(status.code().unwrap_or(1));
            //                 }
            //             }
            //             Err(err) => {
            //                 eprintln!("Failed to run game: {}", err);
            //                 std::process::exit(1);
            //             }
            //         }
            //     } else {
            //         println!("Game binary");
            //         println!("{}", path.display());
            //     }
        }
    }

    Ok(())
}

fn pick_engine_version(start: &Start, arcana: Option<ArcanaArg>) -> Dependency {
    match arcana {
        None => match start.list_engine_versions() {
            [] => Dependency::Crates(env!("CARGO_PKG_VERSION").to_owned()),
            [first, ..] => first.clone(),
        },
        Some(ArcanaArg { arcana }) => arcana,
    }
}
