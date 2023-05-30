use std::{
    path::{Path, PathBuf},
    str::FromStr,
};

use arcana_project::{Dependency, Project};
use clap::{Args, Parser, Subcommand};
use miette::IntoDiagnostic;

#[derive(Debug, Clone, serde::Deserialize)]
struct ArcanaArg {
    arcana: Dependency,
}

impl FromStr for ArcanaArg {
    type Err = toml::de::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let arg = toml::from_str(s)?;
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
    command: Command,
}

fn main() -> miette::Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Command::Init { path, args } => {
            Project::new(path, args.name, map_arcana(args.arcana)?, false)?;
        }
        Command::New { path, args } => {
            Project::new(path, args.name, map_arcana(args.arcana)?, true)?;
        }
        Command::InitWorkspace { path } => {
            let project = Project::find(&path)?;
            project.init_workspace()?;
        }
        Command::Ed { path } => {
            let project = Project::find(&path)?;
            project.init_workspace()?;
            project.run_editor(&path)?;
        }
    }

    Ok(())
}

fn map_arcana(arg: Option<ArcanaArg>) -> miette::Result<Option<Dependency>> {
    match arg {
        Some(ArcanaArg {
            arcana: Dependency::Path { path },
        }) => Ok(Some(Dependency::Path {
            path: realpath_utf8(path.as_ref())?,
        })),
        Some(arg) => Ok(Some(arg.arcana)),
        None => Ok(None),
    }
}

fn realpath_utf8(path: &Path) -> miette::Result<String> {
    let realpath = dunce::canonicalize(path).into_diagnostic()?;
    let string = realpath
        .to_str()
        .ok_or_else(|| miette::miette!("Failed to convert path to string: '{}'", path.display()))?;
    Ok(string.to_owned())
}
