//! Functionality for arcn and arcn-gui.

use std::path::{Path, PathBuf};

use arcana_project::{path::real_path, Dependency, Ident, Project};
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
        engine: Option<Dependency>,
    ) -> miette::Result<Project> {
        let engine = engine
            .or_else(|| self.config.engine.clone())
            .map(|d| d.make_relative(path));

        let (path, name) = process_path_name(path, name)?;
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

    pub fn new_plugin(
        &self,
        path: &Path,
        name: Option<&str>,
        engine: Option<Dependency>,
    ) -> miette::Result<()> {
        let engine = engine
            .or_else(|| self.config.engine.clone())
            .map(|d| d.make_relative(path));

        let (path, name) = process_path_name(path, name)?;
        Project::new_plugin_crate(&path, &name, engine.as_ref())
    }
}

// fn map_engine_dep(dep: Option<&Dependency>, base: &Path) -> miette::Result<Option<Dependency>> {
//     match dep {
//         Some(Dependency::Path { path }) if !path.is_absolute() => Ok(Some(Dependency::Path {
//             path: rebase_dep_path(path.as_ref(), base)?,
//         })),
//         Some(arcana) => Ok(Some(arcana.clone())),
//         None => Ok(None),
//     }
// }

// fn rebase_dep_path(path: &Path, base: &Path) -> miette::Result<Utf8PathBuf> {
//     let path = real_path(path).into_diagnostic()?;
//     let path = make_relative(&path, &base);
//     let path = Utf8PathBuf::try_from(path).into_diagnostic()?;
//     Ok(path)
// }

fn process_path_name(path: &Path, name: Option<&str>) -> miette::Result<(PathBuf, Ident)> {
    let path = real_path(&path).into_diagnostic()?;

    let name = match name {
        None => {
            let Some(file_name) = path.file_name() else {
                miette::bail!("Failed to get project name destination path");
            };

            if file_name.is_empty() || file_name == "." || file_name == ".." {
                miette::bail!("Failed to get project name destination path");
            }

            let Some(file_name) = file_name.to_str() else {
                miette::bail!("Failed to get project name destination path");
            };

            Ident::from_str(file_name).context("Failed to derive project name from path")?
        }
        Some(name) => Ident::from_str(name).context("Invalid project name provided")?,
    };

    Ok((path, name))
}
