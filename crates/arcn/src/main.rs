use std::{path::PathBuf, str::FromStr};

use arcana_project::Dependency;
use clap::{Args, Parser, Subcommand};

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

#[derive(Debug, Args)]
struct InitArgs {
    /// Name of the project.
    /// If not specified, the name of the project will be inferred from the directory name.
    #[arg(long = "name", value_name = "name")]
    name: Option<String>,

    /// Arcana dependency.
    /// If not specified, the version of this CLI crate will be used.
    /// If specified this must be a string with valid toml syntax for a dependency.
    #[arg(long = "arcana", value_name = "arcana-dependency")]
    arcana: Option<ArcanaArg>,
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

        #[command(flatten)]
        args: InitArgs,
    },
    /// Creates new project.
    New {
        /// Path to the project directory.
        /// It may be either absolute or relative to the current directory.
        /// The directory must not exist, it will be created.
        /// The directory must not be part of the cargo workspace.
        #[arg(value_name = "path")]
        path: PathBuf,

        #[command(flatten)]
        args: InitArgs,
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

fn main() -> miette::Result<()> {
    install_tracing_subscriber();

    let cli = Cli::parse();
    let arcn = arcn::Arcn::new()?;

    match cli.command.unwrap_or_else(|| Command::Ed {
        path: PathBuf::from("."),
    }) {
        Command::Init { path, args } => {
            arcn.init(
                &path,
                args.name.as_deref(),
                false,
                args.arcana.as_ref().map(|a| &a.arcana),
            )?;
        }
        Command::New { path, args } => {
            arcn.init(
                &path,
                args.name.as_deref(),
                true,
                args.arcana.as_ref().map(|a| &a.arcana),
            )?;
        }
        Command::InitWorkspace { path } => {
            arcn.init_workspace(&path)?;
        }
        Command::Ed { path } => {
            arcn.run_ed(&path)?;
        }
    }

    Ok(())
}

fn install_tracing_subscriber() {
    use tracing_subscriber::layer::SubscriberExt as _;
    if let Err(err) = tracing::subscriber::set_global_default(
        tracing_subscriber::fmt()
            .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
            .finish()
            .with(tracing_error::ErrorLayer::default()),
    ) {
        panic!("Failed to install tracing subscriber: {}", err);
    }
}
