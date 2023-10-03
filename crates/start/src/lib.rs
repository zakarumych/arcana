//! Functionality for arcn and arcn-gui.

use std::path::{Path, PathBuf};

use arcana_project::{
    path::{make_relative, real_path, RealPath},
    Dependency, Ident, Project,
};
use camino::Utf8PathBuf;
use figa::Figa;
use miette::{Context, IntoDiagnostic};

#[derive(Default, serde::Serialize, serde::Deserialize, figa::Figa)]
struct Config {
    // Configured arcana dependency.
    #[figa(append)]
    engine: Option<Dependency>,

    // Recently created and opened projects.
    #[figa(append)]
    recent: Vec<PathBuf>,
}

pub struct Start {
    config: Config,
}

fn update_config_from_path(config: &mut Config, path: &Path) -> miette::Result<()> {
    let mut r = std::fs::read_to_string(path);

    if path.extension().is_none() {
        if let Err(err) = &r {
            if err.kind() == std::io::ErrorKind::NotFound {
                r = std::fs::read_to_string(path.with_extension("toml"));
            }
        }
    }

    if let Err(err) = &r {
        if err.kind() == std::io::ErrorKind::NotFound {
            tracing::debug!("No config found at {}", path.display());
            return Ok(());
        }
    }

    let s = r
        .into_diagnostic()
        .with_context(|| format!("Failed to read config from {}", path.display()))?;

    config
        .update(toml::Deserializer::new(&s))
        .into_diagnostic()
        .with_context(|| format!("Failed to update config from {}", path.display()))
}

fn update_config_from_env(config: &mut Config) -> miette::Result<()> {
    config
        .update(
            denvars::Deserializer::from_prefixed_env_vars("ARCANA_")
                .with_options(denvars::Options::toml()),
        )
        .into_diagnostic()
        .context("Failed to update config from environment variables")
}

impl Start {
    pub fn new() -> miette::Result<Self> {
        let mut config = Config::default();
        if let Some(dir) = dirs::config_local_dir() {
            update_config_from_path(&mut config, &dir.join("Arcana/config"))?;
        }
        update_config_from_env(&mut config)?;

        Ok(Start { config })
    }

    pub fn init(
        &self,
        path: &Path,
        name: Option<&str>,
        new: bool,
        engine: Option<&Dependency>,
    ) -> miette::Result<Project> {
        let path = real_path(&path).into_diagnostic()?;
        let engine = map_engine_dep(engine.or(self.config.engine.as_ref()), &path)?;
        let name: Option<Ident> = name
            .map(Ident::from_str)
            .transpose()
            .context("Invalid project name provided")?;
        Project::new(path, name, engine, new)
    }

    pub fn init_workspace(&self, path: &Path) -> miette::Result<()> {
        Project::find(&path)?.init_workspace()
    }

    pub fn run_ed(&self, path: &Path) -> miette::Result<()> {
        let p = Project::find(&path)?;
        p.init_workspace()?;
        p.run_editor()
    }
}

fn map_engine_dep(dep: Option<&Dependency>, base: &RealPath) -> miette::Result<Option<Dependency>> {
    match dep {
        Some(Dependency::Path { path }) if path.is_absolute() => {
            Ok(Some(Dependency::Path { path: path.clone() }))
        }
        Some(Dependency::Path { path }) => Ok(Some(Dependency::Path {
            path: rebase_dep_path(path.as_ref(), base)?,
        })),

        Some(arcana) => Ok(Some(arcana.clone())),
        None => Ok(None),
    }
}

fn rebase_dep_path(path: &Path, base: &RealPath) -> miette::Result<Utf8PathBuf> {
    let path = real_path(path).into_diagnostic()?;
    let path = make_relative(&path, &base);
    let path = Utf8PathBuf::try_from(path).into_diagnostic()?;
    Ok(path)
}
