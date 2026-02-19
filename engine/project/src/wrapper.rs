//! This module runs cargo commands to build and run arcana project.

use std::{
    env::consts::{DLL_PREFIX, DLL_SUFFIX, EXE_SUFFIX},
    fmt,
    io::Read,
    path::{Path, PathBuf},
    process::{Child, Command, ExitStatus, Stdio},
};

use arcana_error::Error;

use crate::{WORKSPACE_DIR_NAME, path::make_relative};

use super::Dependency;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Profile {
    Release,
    Debug,
}

/// Construct a command to run ed for arcana project.
pub fn run_editor(root_path: &Path, manifest_path: &Path, profile: Profile) -> Command {
    let workspace = root_path.join(WORKSPACE_DIR_NAME);
    let mut cmd = Command::new("cargo");
    cmd.arg("+nightly").arg("run").arg("--package=ed");
    match profile {
        Profile::Release => {
            cmd.arg("--release");
            cmd.env("ARCANA_PROFILE", "release");
        }
        Profile::Debug => {
            cmd.env("ARCANA_PROFILE", "debug");
        }
    }

    cmd.arg("--");
    cmd.arg(manifest_path.as_os_str());

    // cmd.arg("--verbose")
    cmd.env("RUSTFLAGS", "-Zshare-generics=off -Cprefer-dynamic=yes")
        .current_dir(&workspace);

    cmd
}

/// Construct a command to run ed for arcana project.
pub fn build_editor(root: &Path, profile: Profile) -> Command {
    let workspace = root.join(WORKSPACE_DIR_NAME);
    let mut cmd = Command::new("cargo");
    cmd.arg("+nightly").arg("build").arg("--package=ed");
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
    cmd.arg("+nightly")
        .arg("run")
        .arg("--package=game")
        .arg("--features=arcana/ed");
    if profile == Profile::Release {
        cmd.arg("--release");
    }
    cmd.env("RUSTFLAGS", "-Zshare-generics=off")
        .current_dir(&workspace);

    cmd.stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());

    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x08000000); // CREATE_NO_WINDOW
    }

    cmd
}

/// Construct a command to run ed for arcana project.
pub fn build_game(root: &Path, profile: Profile) -> Command {
    let workspace = root.join(WORKSPACE_DIR_NAME);
    let mut cmd = Command::new("cargo");
    cmd.arg("+nightly").arg("build").arg("--package=game");
    if profile == Profile::Release {
        cmd.arg("--release");
    }
    cmd.env("RUSTFLAGS", "-Zshare-generics=off")
        .current_dir(&workspace);

    cmd.stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x08000000); // CREATE_NO_WINDOW
    }

    cmd
}

/// Spawn async plugins building process.
/// Returns BuildProcess that can be used to determine expected shared lib artefact
/// and poll build completion.
pub fn build_plugins(root: &Path, profile: Profile) -> Result<BuildProcess, Error> {
    let workspace = root.join(WORKSPACE_DIR_NAME);

    // let mut cargo_tree = Command::new("cargo")
    //     .arg("+nightly")
    //     .arg("tree")
    //     .arg("--edges=features,build,normal")
    //     .arg("--package=arcana")
    //     .spawn()
    //     .unwrap();

    // cargo_tree.wait().unwrap();

    // let mut output = String::new();
    // cargo_tree
    //     .stderr
    //     .take()
    //     .unwrap()
    //     .read_to_string(&mut output)
    //     .unwrap();

    // tracing::error!("Cargo tree output:\n{output}");

    let mut cmd = Command::new("cargo");
    cmd.arg("+nightly").arg("build").arg("--package=plugins");

    // cmd.arg("--message-format=json-diagnostic-rendered-ansi");

    if profile == Profile::Release {
        cmd.arg("--release");
    }

    cmd.env("RUSTFLAGS", "-Zshare-generics=off -Cprefer-dynamic=yes")
        .current_dir(&workspace);

    cmd.stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x08000000); // CREATE_NO_WINDOW
    }

    let child = cmd.spawn().map_err(|error| {
        Error::msg(format!(
            "Failed to start building plugins '{}'. {error:?}",
            workspace.display()
        ))
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

struct BuildError {
    status: ExitStatus,
    stderr: String,
}

impl fmt::Debug for BuildError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Build process failed: '{status}'\n\n{stderr}",
            status = self.status,
            stderr = self.stderr
        )
    }
}

impl fmt::Display for BuildError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if f.alternate() {
            write!(
                f,
                "Build process failed: '{status}'\n\n{stderr}",
                status = self.status,
                stderr = self.stderr
            )
        } else {
            write!(f, "Build process failed: '{status}'", status = self.status)
        }
    }
}

impl std::error::Error for BuildError {}

impl BuildError {
    pub fn status(&self) -> ExitStatus {
        self.status
    }

    pub fn stderr(&self) -> &str {
        &self.stderr
    }
}

impl BuildProcess {
    /// Checks if build process has finished and returns result.
    ///
    /// Returns [`None`] if process is still running,
    /// [`Ok`] if process finished successfully
    /// and [`Err`] if process finished with error or failed to wait.
    pub fn finished(&mut self) -> Option<Result<(), Error>> {
        match self.child.try_wait() {
            Err(error) => Some(Err(Error::msg(format!(
                "Failed to wait for build process to finish. {error:?}"
            )))),
            Ok(None) => None,
            Ok(Some(status)) if status.success() => Some(Ok(())),
            Ok(Some(status)) => {
                let stderr = match self.child.stderr.take() {
                    None => format!("<STDERR NOT CAPTURED>"),
                    Some(mut stderr) => {
                        let mut buf = String::new();
                        match stderr.read_to_string(&mut buf) {
                            Ok(_) => buf,
                            Err(error) => format!("<FAILED TO READ STDERR: {error:?}>"),
                        }
                    }
                };

                Some(Err(Error::wrap(BuildError { status, stderr })))
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
