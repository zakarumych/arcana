//! Functionality for arcn and arcn-gui.

use std::{
    cmp::Ordering,
    path::{Path, PathBuf},
};

use arcana_project::{new_plugin_crate, real_path, Dependency, Ident, IdentBuf, Project};
use figa::Figa;

#[derive(Default, serde::Serialize, serde::Deserialize, figa::Figa)]
struct Config {
    // Recently created and opened projects.
    #[figa(append)]
    recent: Vec<PathBuf>,

    // Configured variants of arcana dependency.
    #[figa(append)]
    engines: Vec<Dependency>,

    // Known plugins.
    #[figa(append)]
    plugins: Vec<Dependency>,
}

pub struct Start {
    config: Config,
}

fn dependency_sort(a: &Dependency, b: &Dependency) -> Ordering {
    match (a, b) {
        (Dependency::Crates(a), Dependency::Crates(b)) => a.cmp(b),
        (Dependency::Crates(_), _) => std::cmp::Ordering::Less,
        (_, Dependency::Crates(_)) => std::cmp::Ordering::Greater,
        (
            Dependency::Git {
                git: a_git,
                branch: a_branch,
            },
            Dependency::Git {
                git: b_git,
                branch: b_branch,
            },
        ) => a_git.cmp(b_git).then(a_branch.cmp(b_branch)),
        (Dependency::Git { .. }, _) => std::cmp::Ordering::Less,
        (_, Dependency::Git { .. }) => std::cmp::Ordering::Greater,
        (Dependency::Path { path: a }, Dependency::Path { path: b }) => a.cmp(b),
    }
}

fn update_config_from_path(config: &mut Config, path: &Path) {
    let s = match std::fs::read_to_string(path) {
        Ok(s) => s,
        Err(err) => {
            if err.kind() == std::io::ErrorKind::NotFound {
                tracing::debug!("No config found at {}", path.display());
            } else {
                tracing::warn!("Failed to read config from {}: {}", path.display(), err);
            }
            return;
        }
    };

    if let Err(err) = config.update(toml::Deserializer::new(&s)) {
        tracing::warn!("Failed to update config from {}: {}", path.display(), err);
    }
}

fn save_config_to_path(config: &Config, path: &Path) {
    let s = match toml::to_string_pretty(config) {
        Ok(s) => s,
        Err(err) => {
            tracing::warn!("Failed to serialize config to {}: {}", path.display(), err);
            return;
        }
    };

    if let Err(err) = std::fs::write(path, s) {
        tracing::warn!("Failed to write config to {}: {}", path.display(), err);
    }
}

fn update_config_from_env(config: &mut Config) {
    let de = denvars::Deserializer::from_prefixed_env_vars("ARCANA_")
        .with_options(denvars::Options::toml());

    if let Err(err) = config.update(de) {
        tracing::warn!("Failed to update config from environment: {}", err);
    }
}

impl Start {
    pub fn new() -> Self {
        let mut config = Config::default();
        if let Some(dir) = dirs::config_local_dir() {
            update_config_from_path(&mut config, &dir.join("Arcana/config.toml"));
        }
        update_config_from_env(&mut config);

        config.engines.sort_by(dependency_sort);
        config.engines.dedup();

        Start { config }
    }

    pub fn list_engine_versions(&self) -> &[Dependency] {
        &self.config.engines
    }

    pub fn init(
        &self,
        path: &Path,
        name: Option<&Ident>,
        engine: Dependency,
        new: bool,
    ) -> miette::Result<Project> {
        let (path, name) = process_path_name(path, name)?;
        Project::new(name, engine, path, new)
    }

    pub fn open(&self, path: &Path) -> miette::Result<Project> {
        Project::open(path)
    }

    pub fn init_workspace(&self, path: &Path) -> miette::Result<()> {
        Project::find(&path)?.init_workspace()
    }

    pub fn run_ed(&self, path: &Path) -> miette::Result<()> {
        let p = Project::find(&path)?;
        p.run_editor()
    }

    pub fn new_plugin(
        &self,
        path: &Path,
        name: Option<&Ident>,
        engine: Dependency,
    ) -> miette::Result<()> {
        let (path, name) = process_path_name(path, name)?;
        new_plugin_crate(&name, &path, engine)
    }

    pub fn build_game(&self, path: &Path) -> miette::Result<PathBuf> {
        let p = Project::find(&path)?;
        p.init_workspace()?;
        p.build_game()
    }

    pub fn run_game(&self, path: &Path) -> miette::Result<()> {
        let p = Project::find(&path)?;
        p.init_workspace()?;
        p.run_game()
    }

    pub fn recent<'a>(&'a self) -> impl ExactSizeIterator<Item = &'a Path> + 'a {
        self.config.recent.iter().rev().map(|p| &**p)
    }

    pub fn add_engine(&mut self, engine: Dependency) {
        if !self.config.engines.contains(&engine) {
            self.config.engines.push(engine);
            self.config.engines.sort_by(dependency_sort);
            self.config.engines.dedup();
        }

        if let Some(dir) = dirs::config_local_dir() {
            save_config_to_path(&self.config, &dir.join("Arcana/config.toml"));
        }
    }

    pub fn add_recent(&mut self, project_path: PathBuf) {
        if let Some(idx) = self.config.recent.iter().position(|p| **p == project_path) {
            self.config.recent.remove(idx);
        }
        self.config.recent.push(project_path);

        if let Some(dir) = dirs::config_local_dir() {
            save_config_to_path(&self.config, &dir.join("Arcana/config.toml"));
        }
    }

    pub fn remove_recent(&mut self, project_path: &Path) {
        if let Some(idx) = self.config.recent.iter().position(|p| **p == *project_path) {
            self.config.recent.remove(idx);
        }

        if let Some(dir) = dirs::config_local_dir() {
            save_config_to_path(&self.config, &dir.join("Arcana/config.toml"));
        }
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

fn process_path_name(path: &Path, name: Option<&Ident>) -> miette::Result<(PathBuf, IdentBuf)> {
    let path = match real_path(&path) {
        Some(path) => path,
        None => miette::bail!(
            "Failed to get project destination path from {}",
            path.display()
        ),
    };

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

            let Ok(file_name) = Ident::from_str(file_name) else {
                miette::bail!(
                    "Project's directory name cannot be used as project name is it is not valid identifier. Specify name manually"
                );
            };

            file_name.to_owned()
        }
        Some(name) => name.to_owned(),
    };

    Ok((path, name))
}
