use std::{
    path::{Path, PathBuf},
    str::FromStr,
};

use arcana_project::{
    path::{make_relative, real_path, RealPath},
    Dependency, Ident, Project,
};
use camino::Utf8PathBuf;
use clap::{Args, Parser, Subcommand};
use miette::{Context, IntoDiagnostic};

#[derive(Debug, Clone, serde::Deserialize)]
struct ArcanaArg {
    arcana: Dependency,
}

impl FromStr for ArcanaArg {
    type Err = toml::de::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let arg = toml::from_str(&format!("arcana = {s}"))?;
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

    match cli.command.unwrap_or_else(|| Command::Ed {
        path: PathBuf::from("."),
    }) {
        Command::Init { path, args } => {
            let path = real_path(&path).into_diagnostic()?;
            let arcana = map_arcana(args.arcana, &path)?;
            let name = args
                .name
                .map(Ident::from_string)
                .transpose()
                .context("Invalid project name provided")?;
            Project::new(path, name, arcana, false)?;
        }
        Command::New { path, args } => {
            let path = real_path(&path).into_diagnostic()?;
            let arcana = map_arcana(args.arcana, &path)?;
            let name = args
                .name
                .map(Ident::from_string)
                .transpose()
                .context("Invalid project name provided")?;
            Project::new(path, name, arcana, true)?;
        }
        Command::InitWorkspace { path } => {
            let project = Project::find(&path)?;
            project.init_workspace()?;
        }
        Command::Ed { path } => {
            let project = Project::find(&path)?;
            project.init_workspace()?;
            project.run_editor()?;
        }
    }

    Ok(())
}

fn map_arcana(arg: Option<ArcanaArg>, base: &RealPath) -> miette::Result<Option<Dependency>> {
    match arg {
        Some(ArcanaArg {
            arcana: Dependency::Path { path },
        }) => Ok(Some(Dependency::Path {
            path: rebase_dep_path(path.as_ref(), base)?,
        })),
        Some(arg) => Ok(Some(arg.arcana)),
        None => Ok(None),
    }
}

fn rebase_dep_path(path: &Path, base: &RealPath) -> miette::Result<Utf8PathBuf> {
    let path = real_path(path).into_diagnostic()?;
    let path = make_relative(&path, &base);
    let path = Utf8PathBuf::try_from(path).into_diagnostic()?;
    Ok(path)
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
