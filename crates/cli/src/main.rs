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
    #[arg(long = "name", value_name = "name")]
    name: Option<String>,

    #[arg(long = "arcana", value_name = "ARCANA_DEPENDENCY")]
    arcana: Option<ArcanaArg>,
}

#[derive(Debug, Subcommand)]
#[command(rename_all = "kebab-case")]
enum Command {
    Init {
        #[arg(value_name = "path", default_value = ".")]
        path: PathBuf,

        #[command(flatten)]
        args: InitArgs,
    },
    New {
        #[arg(value_name = "path")]
        path: PathBuf,

        #[command(flatten)]
        args: InitArgs,
    },
    InitWorkspace {
        #[arg(value_name = "path", default_value = ".")]
        path: PathBuf,
    },
}

#[derive(Debug, Parser)]
#[command(name = "arcana")]
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
    let realpath = dunce::realpath(path).into_diagnostic()?;
    let string = realpath
        .to_str()
        .ok_or_else(|| miette::miette!("Failed to convert path to string: '{}'", path.display()))?;
    Ok(string.to_owned())
}
