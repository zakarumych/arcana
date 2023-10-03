//! This module generates the wrapper libs for plugins libraries.

use std::{
    env::consts::{DLL_PREFIX, DLL_SUFFIX, EXE_SUFFIX},
    fmt,
    path::{Path, PathBuf},
    process::{Child, Command},
};

use crate::{path::RealPath, Plugin};

use super::Dependency;

const WORKSPACE_DIR_NAME: &'static str = "crates";

fn github_autogen_issue_template(file: &str) -> String {
    format!("https://github.com/zakarumych/nothing/issues/new?body=%3C%21--%20Please%2C%20provide%20your%20reason%20to%20edit%20auto-generated%20{file}%20in%20Arcana%20project%20--%3E")
}

/// Writes content to a file.
/// If new content is the same as old content the file is not modified.
fn write_file(path: impl AsRef<Path>, content: impl AsRef<[u8]>) -> std::io::Result<()> {
    match std::fs::read(path.as_ref()) {
        Ok(old_content) if old_content == content.as_ref() => {
            return Ok(());
        }
        _ => {}
    }
    std::fs::write(path, content)
}

struct ArcanaDependency<'a>(Option<&'a Dependency>);

impl fmt::Display for ArcanaDependency<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.0 {
            None => write!(f, "\"{}\"", env!("CARGO_PKG_VERSION")),
            Some(Dependency::Crates(version)) => write!(f, "\"{}\"", version),
            Some(Dependency::Git { git, branch }) => {
                if let Some(branch) = branch {
                    write!(f, "{{ git = \"{git}\", branch = \"{branch}\" }}")
                } else {
                    write!(f, "{{ git = \"{git}\" }}")
                }
            }
            Some(Dependency::Path { path }) => {
                if path.is_absolute() {
                    write!(f, "{{ path = \"{}\" }}", path.as_str().escape_default())
                } else {
                    // Workspace is currently hardcoded to be one directory down from the root.
                    // Switch to arcana-dyn crate
                    write!(f, "{{ path = \"../{}\" }}", path.as_str().escape_default())
                }
            }
        }
    }
}

struct ArcanaDynDependency<'a>(Option<&'a Dependency>);

impl fmt::Display for ArcanaDynDependency<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.0 {
            None => write!(f, "\"{}\"", env!("CARGO_PKG_VERSION")),
            Some(Dependency::Crates(version)) => write!(f, "\"{}\"", version),
            Some(Dependency::Git { git, branch }) => {
                if let Some(branch) = branch {
                    write!(f, "{{ git = \"{git}\", branch = \"{branch}\" }}")
                } else {
                    write!(f, "{{ git = \"{git}\" }}")
                }
            }
            Some(Dependency::Path { path }) => {
                // Switch to arcana-dyn crate
                if path.is_absolute() {
                    if let Some(parent) = path.parent() {
                        write!(
                            f,
                            "{{ path = \"{}/dyn\" }}",
                            parent.as_str().escape_default()
                        )
                    } else {
                        // Try like that
                        write!(
                            f,
                            "{{ path = \"{}/../dyn\" }}",
                            path.as_str().escape_default()
                        )
                    }
                } else {
                    // Workspace is currently hardcoded to be one directory down from the root.
                    if let Some(parent) = path.parent() {
                        write!(
                            f,
                            "{{ path = \"../{}/dyn\" }}",
                            parent.as_str().escape_default()
                        )
                    } else {
                        // Try like that
                        write!(
                            f,
                            "{{ path = \"../{}/../dyn\" }}",
                            path.as_str().escape_default()
                        )
                    }
                }
            }
        }
    }
}

struct PluginDependency<'a> {
    dep: &'a Dependency,
}

impl fmt::Display for PluginDependency<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.dep {
            Dependency::Crates(version) => write!(f, "\"{}\"", version),
            Dependency::Git { git, branch } => {
                if let Some(branch) = branch {
                    write!(f, "{{ git = \"{git}\", branch = \"{branch}\" }}")
                } else {
                    write!(f, "{{ git = \"{git}\" }}")
                }
            }
            Dependency::Path { path } => {
                // Plugins are added to be in the 'crates/plugins' or 'crates/game' crate.
                write!(
                    f,
                    "{{ path = \"../../{}\" }}",
                    path.as_str().escape_default()
                )
            }
        }
    }
}

/// Generates workspace.
pub fn init_workspace(
    name: &str,
    engine: Option<&Dependency>,
    root: &RealPath,
    plugins: &[Plugin],
) -> miette::Result<()> {
    let workspace = root.join(WORKSPACE_DIR_NAME);
    std::fs::create_dir_all(&*workspace).map_err(|err| {
        miette::miette!(
            "Failed to create project workspace directory: '{}'. {err}",
            workspace.display()
        )
    })?;

    let cargo_toml = format!(
        r#"# This file is automatically generated for Arcana Project.
# It should not require manual editing.
# If manual editing is required, consider posting your motivation in new GitHub issue
# [{gh_issue}]

[workspace]
resolver = "2"
members = ["plugins", "ed", "game"]

[workspace.dependencies]
arcana = {arcana}
arcana-dyn = {arcana_dyn}
"#,
        gh_issue = github_autogen_issue_template("workspace Cargo.toml"),
        arcana = ArcanaDependency(engine),
        arcana_dyn = ArcanaDynDependency(engine)
    );

    let cargo_toml_path = workspace.join("Cargo.toml");
    write_file(&cargo_toml_path, cargo_toml).map_err(|err| {
        miette::miette!(
            "Failed to create project workspace Cargo.toml: '{}'. {err}",
            cargo_toml_path.display()
        )
    })?;

    let rust_toolchain = r#"[toolchain]
channel = "nightly"
    "#;

    let rust_toolchain_path = workspace.join("rust-toolchain.toml");
    write_file(&rust_toolchain_path, rust_toolchain).map_err(|err| {
        miette::miette!(
            "Failed to create project workspace rust-toolchain.toml: '{}'. {err}",
            rust_toolchain_path.display()
        )
    })?;

    make_ed_crate(&workspace)?;
    make_plugins_crate(&workspace, plugins)?;
    make_game_crate(name, &workspace, plugins)?;

    Ok(())
}

/// Generates ed crate
fn make_ed_crate(workspace: &Path) -> miette::Result<()> {
    let ed_path = workspace.join("ed");

    std::fs::create_dir_all(&ed_path).map_err(|err| {
        miette::miette!(
            "Failed to create project ed crate directory: '{}'. {err}",
            ed_path.display()
        )
    })?;

    let cargo_toml = format!(
        r#"# This file is automatically generated for Arcana Project.
# It should not require manual editing.
# If manual editing is required, consider posting your motivation in new GitHub issue
# [{gh_issue}]
[package]
name = "ed"
version = "0.0.0"
publish = false
edition = "2021"

[dependencies]
arcana-dyn = {{ workspace = true }}
"#,
        gh_issue = github_autogen_issue_template("ed/Cargo.toml")
    );

    let cargo_toml_path = ed_path.join("Cargo.toml");
    write_file(&cargo_toml_path, cargo_toml).map_err(|err| {
        miette::miette!(
            "Failed to create project ed crate Cargo.toml '{}'. {err}",
            cargo_toml_path.display()
        )
    })?;

    let src_path = ed_path.join("src");
    std::fs::create_dir_all(&src_path).map_err(|err| {
        miette::miette!(
            "Failed to create project ed crate src directory: '{}'. {err}",
            src_path.display()
        )
    })?;

    let main_rs = format!(
        r#"//! This file is automatically generated for Arcana Project.
//! It should not require manual editing.
//! If manual editing is required, consider posting your motivation in new GitHub issue
//! [{gh_issue}]

fn main() {{
    arcana_dyn::ed::run(env!("CARGO_MANIFEST_DIR").as_ref());
}}
"#,
        gh_issue = github_autogen_issue_template("ed/src/main.rs")
    );

    let main_rs_path = src_path.join("main.rs");
    write_file(&main_rs_path, main_rs).map_err(|err| {
        miette::miette!(
            "Failed to create project ed crate source: '{}'. {err}",
            main_rs_path.display()
        )
    })?;

    Ok(())
}

/// Generates plugins crate
fn make_plugins_crate(workspace: &Path, plugins: &[Plugin]) -> miette::Result<()> {
    let plugins_path = workspace.join("plugins");

    std::fs::create_dir_all(&plugins_path).map_err(|err| {
        miette::miette!(
            "Failed to create project plugins crate directory: '{}'. {err}",
            plugins_path.display()
        )
    })?;

    let mut cargo_toml = format!(
        r#"# This file is automatically generated for Arcana Project.
# It should not require manual editing.
# If manual editing is required, consider posting your motivation in new GitHub issue
# [{gh_issue}]
[package]
name = "plugins"
version = "0.0.0"
publish = false
edition = "2021"

[lib]
crate-type = ["cdylib"]

[dependencies]
arcana-dyn = {{ workspace = true }}
"#,
        gh_issue = github_autogen_issue_template("plugins/Cargo.toml")
    );

    for plugin in plugins {
        cargo_toml.push_str(&format!(
            "{name} = {dependency}\n",
            name = &plugin.name,
            dependency = PluginDependency { dep: &plugin.dep }
        ));
    }

    let cargo_toml_path = plugins_path.join("Cargo.toml");
    write_file(&cargo_toml_path, &cargo_toml).map_err(|err| {
        miette::miette!(
            "Failed to create project plugins crate Cargo.toml '{}'. {err}",
            cargo_toml_path.display()
        )
    })?;

    let src_path = plugins_path.join("src");
    std::fs::create_dir_all(&src_path).map_err(|err| {
        miette::miette!(
            "Failed to create project plugins crate src directory: '{}'. {err}",
            src_path.display()
        )
    })?;

    let mut lib_rs = format!(
        r#"//! This file is automatically generated for Arcana Project.
//! It should not require manual editing.
//! If manual editing is required, consider posting your motivation in new GitHub issue
//! [{gh_issue}]

#[no_mangle]
pub fn arcana_plugins() -> Vec<&'static dyn arcana_dyn::plugin::ArcanaPlugin> {{
    vec!["#,
        gh_issue = github_autogen_issue_template("plugins/src/lib.rs")
    );

    if !plugins.is_empty() {
        lib_rs.push('\n');

        for plugin in plugins {
            lib_rs.push_str(&format!(
                "        {name}::__arcana_plugin(),\n",
                name = &plugin.name
            ));
        }
    }

    lib_rs.push_str(
        r#"    ]
}"#,
    );

    let lib_rs_path = src_path.join("lib.rs");
    write_file(&lib_rs_path, lib_rs).map_err(|err| {
        miette::miette!(
            "Failed to create project plugins crate source: '{}'. {err}",
            lib_rs_path.display()
        )
    })?;

    Ok(())
}

/// Generates game crate
fn make_game_crate(name: &str, workspace: &Path, plugins: &[Plugin]) -> miette::Result<()> {
    let game_path = workspace.join("game");

    std::fs::create_dir_all(&game_path).map_err(|err| {
        miette::miette!(
            "Failed to create project plugins crate directory: '{}'. {err}",
            game_path.display()
        )
    })?;

    let mut cargo_toml = format!(
        r#"# This file is automatically generated for Arcana Project.
# It should not require manual editing.
# If manual editing is required, consider posting your motivation in new GitHub issue
# [{gh_issue}]
[package]
name = "game"
version = "0.0.0"
publish = false
edition = "2021"

[[bin]]
name = "{name}"
path = "src/main.rs"

[dependencies]
arcana = {{ workspace = true }}
"#,
        gh_issue = github_autogen_issue_template("game/Cargo.toml")
    );

    for plugin in plugins {
        cargo_toml.push_str(&format!(
            "{name} = {dependency}\n",
            name = &plugin.name,
            dependency = PluginDependency { dep: &plugin.dep }
        ));
    }

    let cargo_toml_path = game_path.join("Cargo.toml");
    write_file(&cargo_toml_path, cargo_toml).map_err(|err| {
        miette::miette!(
            "Failed to create project game crate Cargo.toml '{}'. {err}",
            cargo_toml_path.display()
        )
    })?;

    let src_path = game_path.join("src");
    std::fs::create_dir_all(&src_path).map_err(|err| {
        miette::miette!(
            "Failed to create project game crate src directory: '{}'. {err}",
            src_path.display()
        )
    })?;

    let main_rs = format!(
        r#"//! This file is automatically generated for Arcana Project.
//! It should not require manual editing.
//! If manual editing is required, consider posting your motivation in new GitHub issue
//! [{gh_issue}]

fn main() {{ todo!() }}
"#,
        gh_issue = github_autogen_issue_template("game/src/main.rs")
    );

    let main_rs_path = src_path.join("main.rs");
    write_file(&main_rs_path, main_rs).map_err(|err| {
        miette::miette!(
            "Failed to create project plugins crate source: '{}'. {err}",
            main_rs_path.display()
        )
    })?;

    Ok(())
}

/// Construct a command to run ed for arcana project.
pub fn run_editor(root: &RealPath) -> Command {
    let workspace = root.join(WORKSPACE_DIR_NAME);
    let mut cmd = Command::new("cargo");
    cmd.arg("run")
        .arg("--package=ed")
        .env("RUSTFLAGS", "-Zshare-generics=off -Cprefer-dynamic=yes")
        .current_dir(&workspace);
    cmd
}

/// Spawn async plugins building process.
/// Returns BuildProcess that can be used to determine expected shared lib artefact
/// and poll build completion.
pub fn build_plugins(root: &RealPath) -> miette::Result<BuildProcess> {
    let workspace = root.join(WORKSPACE_DIR_NAME);

    let child = Command::new("cargo")
        .arg("build")
        .arg("--package=plugins")
        .env("RUSTFLAGS", "-Zshare-generics=off -Cprefer-dynamic=yes")
        .current_dir(&workspace)
        .spawn()
        .map_err(|err| {
            miette::miette!(
                "Failed to start building plugins '{}'. {err}",
                workspace.display()
            )
        })?;

    let artifact = plugins_lib_path(&workspace);

    Ok(BuildProcess { child, artifact })
}

/// Construct expected plugin build artifact path.
fn plugins_lib_path(workspace: &Path) -> PathBuf {
    let mut lib_path = workspace.join("target");
    lib_path.push("debug"); // Hardcoded for now.
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

/// Spawn game building process.
/// Returns path to the game binary.
pub fn build_game(name: &str, root: &RealPath) -> miette::Result<PathBuf> {
    let workspace = root.join(WORKSPACE_DIR_NAME);

    let result = Command::new("cargo")
        .arg("build")
        .arg("--package=game")
        .current_dir(&workspace)
        .status();

    match result {
        Err(err) => {
            miette::bail!("Failed to wait for build process to finish. {err}",);
        }
        Ok(status) if status.success() => {}
        Ok(status) => {
            miette::bail!(
                "Build process failed with status '{status}'.",
                status = status
            );
        }
    };

    Ok(game_bin_path(name, &workspace))
}

/// Construct expected plugin build artifact path.
fn game_bin_path(name: &str, workspace: &Path) -> PathBuf {
    let mut lib_path = workspace.join("target");
    lib_path.push("debug"); // Hardcoded for now.
    lib_path.push(format!("{name}{EXE_SUFFIX}"));
    lib_path
}
