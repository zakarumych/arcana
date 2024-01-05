//! This module runs cargo commands to build and run arcana project.

use std::{
    env::consts::{DLL_PREFIX, DLL_SUFFIX, EXE_SUFFIX},
    fmt,
    path::{Path, PathBuf},
    process::{Child, Command},
};

use crate::{path::make_relative, WORKSPACE_DIR_NAME};

use super::Dependency;

// #[derive(Clone, Copy)]
// enum Profile {
//     Client,
//     Server,
//     ClientServer,
//     Ed,
// }

// struct WithFeatures(Profile);

// impl fmt::Display for WithFeatures {
//     fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
//         match self.0 {
//             Profile::Client => write!(f, ", features = [\"client\"]"),
//             Profile::Server => write!(f, ", features = [\"server\"]"),
//             Profile::ClientServer => write!(f, ", features = [\"client\", \"server\"]"),
//             Profile::Ed => write!(f, ", features = [\"ed\"]"),
//         }
//     }
// }

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Profile {
    Release,
    Debug,
}

/// Construct a command to run ed for arcana project.
pub fn run_editor(root: &Path, profile: Profile) -> Command {
    let workspace = root.join(WORKSPACE_DIR_NAME);
    let mut cmd = Command::new("cargo");
    cmd.arg("run").arg("--package=ed");
    match profile {
        Profile::Release => {
            cmd.arg("--release");
            cmd.env("ARCANA_PROFILE", "release");
        }
        Profile::Debug => {
            cmd.env("ARCANA_PROFILE", "debug");
        }
    }
    // cmd.arg("--verbose")
    cmd.env("RUSTFLAGS", "-Zshare-generics=off -Cprefer-dynamic=yes")
        .current_dir(&workspace);
    cmd
}

/// Construct a command to run ed for arcana project.
pub fn build_editor(root: &Path, profile: Profile) -> Command {
    let workspace = root.join(WORKSPACE_DIR_NAME);
    let mut cmd = Command::new("cargo");
    cmd.arg("build").arg("--package=ed");
    if profile == Profile::Release {
        cmd.arg("--release");
    }
    // cmd.arg("--verbose")
    cmd.env("RUSTFLAGS", "-Zshare-generics=off -Cprefer-dynamic=yes")
        .current_dir(&workspace);
    cmd
}

/// Construct a command to run ed for arcana project.
pub fn run_game(root: &Path, profile: Profile) -> Command {
    let workspace = root.join(WORKSPACE_DIR_NAME);
    let mut cmd = Command::new("cargo");
    cmd.arg("run")
        .arg("--package=game")
        .arg("--features=arcana/ed");
    if profile == Profile::Release {
        cmd.arg("--release");
    }
    cmd.env("RUSTFLAGS", "-Zshare-generics=off -Cprefer-dynamic=yes")
        .current_dir(&workspace);
    cmd
}

/// Construct a command to run ed for arcana project.
pub fn build_game(root: &Path, profile: Profile) -> Command {
    let workspace = root.join(WORKSPACE_DIR_NAME);
    let mut cmd = Command::new("cargo");
    cmd.arg("build").arg("--package=game");
    if profile == Profile::Release {
        cmd.arg("--release");
    }
    cmd.env("RUSTFLAGS", "-Zshare-generics=off")
        .current_dir(&workspace);
    cmd
}

/// Spawn async plugins building process.
/// Returns BuildProcess that can be used to determine expected shared lib artefact
/// and poll build completion.
pub fn build_plugins(root: &Path, profile: Profile) -> miette::Result<BuildProcess> {
    let workspace = root.join(WORKSPACE_DIR_NAME);

    let mut cmd = Command::new("cargo");
    cmd.arg("build").arg("--package=plugins");

    if profile == Profile::Release {
        cmd.arg("--release");
    }

    let child = cmd
        .env("RUSTFLAGS", "-Zshare-generics=off -Cprefer-dynamic=yes")
        .current_dir(&workspace)
        .spawn()
        .map_err(|err| {
            miette::miette!(
                "Failed to start building plugins '{}'. {err}",
                workspace.display()
            )
        })?;

    let artifact = plugins_lib_path(&workspace, profile);

    Ok(BuildProcess { child, artifact })
}

/// Construct expected plugin build artifact path.
fn plugins_lib_path(workspace: &Path, profile: Profile) -> PathBuf {
    let mut lib_path = workspace.join("target");
    lib_path.push(match profile {
        Profile::Release => "release",
        Profile::Debug => "debug",
    }); // Hardcoded for now.
    lib_path.push(format!("{DLL_PREFIX}plugins{DLL_SUFFIX}"));
    lib_path
}

pub struct BuildProcess {
    child: Child,
    artifact: PathBuf,
}

impl Drop for BuildProcess {
    fn drop(&mut self) {
        let _ = self.child.kill();
    }
}

impl BuildProcess {
    /// Checks if process has finished.
    /// Returns error if process exit unsuccessfully.
    /// Returns Ok(true) if process is complete.
    /// Returns Ok(false) if process is still running.
    pub fn finished(&mut self) -> miette::Result<bool> {
        match self.child.try_wait() {
            Err(err) => {
                miette::bail!("Failed to wait for build process to finish. {err}",);
            }
            Ok(None) => Ok(false),
            Ok(Some(status)) if status.success() => Ok(true),
            Ok(Some(status)) => {
                miette::bail!(
                    "Build process failed with status '{status}'.",
                    status = status
                );
            }
        }
    }

    /// Returns expected build artifact path.
    pub fn artifact(&self) -> &Path {
        &self.artifact
    }
}

/// Construct expected plugin build artifact path.
pub fn game_bin_path(name: &str, root: &Path) -> PathBuf {
    let mut bin_path = root.join(WORKSPACE_DIR_NAME);
    bin_path.push("target");
    bin_path.push("debug"); // Hardcoded for now.
    bin_path.push(format!("{name}{EXE_SUFFIX}"));
    bin_path
}
