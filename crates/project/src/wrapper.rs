//! This module generates the wrapper libs for plugins libraries.

use std::{
    env::consts::{DLL_PREFIX, DLL_SUFFIX, EXE_SUFFIX},
    fmt,
    path::{Path, PathBuf},
    process::{Child, Command},
};

use crate::{path::make_relative, Plugin};

use super::Dependency;

const WORKSPACE_DIR_NAME: &'static str = "crates";

fn github_autogen_issue_template(file: &str) -> String {
    format!("https://github.com/zakarumych/nothing/issues/new?body=%3C%21--%20Please%2C%20provide%20your%20reason%20to%20edit%20auto-generated%20{file}%20in%20Arcana%20project%20--%3E")
}

#[derive(Clone, Copy)]
enum Profile {
    Client,
    Server,
    ClientServer,
    Ed,
}

struct WithFeatures(Profile);

impl fmt::Display for WithFeatures {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.0 {
            Profile::Client => write!(f, ", features = [\"client\"]"),
            Profile::Server => write!(f, ", features = [\"server\"]"),
            Profile::ClientServer => write!(f, ", features = [\"client\", \"server\"]"),
            Profile::Ed => write!(f, ", features = [\"ed\"]"),
        }
    }
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
                write!(f, "{{ path = \"{}\" }}", path.as_str().escape_default())
            }
        }
    }
}

// struct ArcanaDynDependency<'a>(Option<&'a Dependency>);

// impl fmt::Display for ArcanaDynDependency<'_> {
//     fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
//         match self.0 {
//             None => write!(f, "\"{}\"", env!("CARGO_PKG_VERSION")),
//             Some(Dependency::Crates(version)) => write!(f, "\"{}\"", version),
//             Some(Dependency::Git { git, branch }) => {
//                 if let Some(branch) = branch {
//                     write!(f, "{{ git = \"{git}\", branch = \"{branch}\" }}")
//                 } else {
//                     write!(f, "{{ git = \"{git}\" }}")
//                 }
//             }
//             Some(Dependency::Path { path }) => {
//                 // Switch to arcana-dyn crate
//                 if path.is_absolute() {
//                     if let Some(parent) = path.parent() {
//                         write!(
//                             f,
//                             "{{ path = \"{}/dyn\" }}",
//                             parent.as_str().escape_default()
//                         )
//                     } else {
//                         // Try like that
//                         write!(
//                             f,
//                             "{{ path = \"{}/../dyn\" }}",
//                             path.as_str().escape_default()
//                         )
//                     }
//                 } else {
//                     // Workspace is currently hardcoded to be one directory down from the root.
//                     if let Some(parent) = path.parent() {
//                         write!(
//                             f,
//                             "{{ path = \"{}/dyn\" }}",
//                             parent.as_str().escape_default()
//                         )
//                     } else {
//                         // Try like that
//                         write!(
//                             f,
//                             "{{ path = \"{}/../dyn\" }}",
//                             path.as_str().escape_default()
//                         )
//                     }
//                 }
//             }
//         }
//     }
// }

struct PluginDependency<'a> {
    dep: &'a Dependency,
}

impl fmt::Display for PluginDependency<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.dep {
            Dependency::Crates(version) => write!(f, "\"{}\"", version),
            Dependency::Git { git, branch } => {
                if let Some(branch) = branch {
                    write!(f, "{{ git = \"{git}\", branch = \"{branch}\" }}",)
                } else {
                    write!(f, "{{ git = \"{git}\" }}")
                }
            }
            Dependency::Path { path } => {
                write!(f, "{{ path = \"{}\" }}", path.as_str().escape_default(),)
            }
        }
    }
}

/// Generates workspace.
pub fn init_workspace(
    root: &Path,
    name: &str,
    engine: Option<&Dependency>,
    plugins: &[Plugin],
) -> miette::Result<()> {
    let workspace = root.join(WORKSPACE_DIR_NAME);
    std::fs::create_dir_all(&*workspace).map_err(|err| {
        miette::miette!(
            "Failed to create project workspace directory: '{}'. {err}",
            workspace.display()
        )
    })?;

    let engine = engine.map(|d| d.clone().make_relative(WORKSPACE_DIR_NAME));

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
"#,
        gh_issue = github_autogen_issue_template("workspace Cargo.toml"),
        arcana = ArcanaDependency(engine.as_ref()),
        // arcana_dyn = ArcanaDynDependency(engine.as_ref())
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

    make_ed_crate(root, &workspace)?;
    make_plugins_crate(root, &workspace, plugins)?;
    make_game_crate(root, &workspace, name, plugins)?;

    Ok(())
}

/// Generates ed crate
fn make_ed_crate(root: &Path, workspace: &Path) -> miette::Result<()> {
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
arcana = {{ workspace = true, features = ["client", "server", "ed"] }}
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
    arcana::ed::run(env!("CARGO_MANIFEST_DIR").as_ref());
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
fn make_plugins_crate(root: &Path, workspace: &Path, plugins: &[Plugin]) -> miette::Result<()> {
    let plugins_path = workspace.join("plugins");
    let plugins_path_from_root = make_relative(&plugins_path, root);

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
arcana = {{ workspace = true, features = ["client", "server", "ed"] }}
"#,
        gh_issue = github_autogen_issue_template("plugins/Cargo.toml")
    );

    for plugin in plugins {
        let dep = plugin.dep.clone().make_relative(&plugins_path_from_root);

        cargo_toml.push_str(&format!(
            "{name} = {dependency}\n",
            name = &plugin.name,
            dependency = PluginDependency { dep: &dep }
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
pub fn arcana_plugins() -> &'static [&'static dyn arcana::plugin::ArcanaPlugin] {{
    const PLUGINS: [&'static dyn arcana::plugin::ArcanaPlugin; {plugins_count}] = ["#,
        gh_issue = github_autogen_issue_template("plugins/src/lib.rs"),
        plugins_count = plugins.len(),
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
        r#"    ];
    &PLUGINS
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
fn make_game_crate(
    root: &Path,
    workspace: &Path,
    name: &str,
    plugins: &[Plugin],
) -> miette::Result<()> {
    let game_path = workspace.join("game");
    let game_path_from_root = make_relative(&game_path, root);

    std::fs::create_dir_all(&game_path).map_err(|err| {
        miette::miette!(
            "Failed to create project game crate directory: '{}'. {err}",
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
arcana = {{ workspace = true, features = ["client"] }}
"#,
        gh_issue = github_autogen_issue_template("game/Cargo.toml")
    );

    for plugin in plugins {
        let dep = plugin.dep.clone().make_relative(&game_path_from_root);

        cargo_toml.push_str(&format!(
            "{name} = {dependency}\n",
            name = &plugin.name,
            dependency = PluginDependency { dep: &dep }
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

    let mut main_rs = format!(
        r#"//! This file is automatically generated for Arcana Project.
//! It should not require manual editing.
//! If manual editing is required, consider posting your motivation in new GitHub issue
//! [{gh_issue}]

fn main() {{
    const PLUGINS: [&'static dyn arcana::plugin::ArcanaPlugin; {plugins_count}] = ["#,
        gh_issue = github_autogen_issue_template("game/src/main.rs"),
        plugins_count = plugins.len(),
    );

    if !plugins.is_empty() {
        main_rs.push('\n');

        for plugin in plugins {
            main_rs.push_str(&format!(
                "        {name}::__arcana_plugin(),\n",
                name = &plugin.name
            ));
        }
    }

    main_rs.push_str(
        r#"    ];
    arcana::game::run(&PLUGINS);
}"#,
    );

    let main_rs_path = src_path.join("main.rs");
    write_file(&main_rs_path, main_rs).map_err(|err| {
        miette::miette!(
            "Failed to create project game crate source: '{}'. {err}",
            main_rs_path.display()
        )
    })?;

    Ok(())
}

/// Generates new plugin crate
pub fn new_plugin_crate(
    name: &str,
    path: &Path,
    engine: Option<&Dependency>,
) -> miette::Result<()> {
    std::fs::create_dir_all(&path).map_err(|err| {
        miette::miette!(
            "Failed to create project plugin crate directory: '{}'. {err}",
            path.display()
        )
    })?;

    let mut cargo_toml = format!(
        r#"[package]
version = "0.0.0"
name = "{name}"
edition = "2021"
publish = false

[dependencies]
arcana = {engine}
"#,
        engine = ArcanaDependency(engine)
    );

    let cargo_toml_path = path.join("Cargo.toml");
    write_file(&cargo_toml_path, cargo_toml).map_err(|err| {
        miette::miette!(
            "Failed to create project game crate Cargo.toml '{}'. {err}",
            cargo_toml_path.display()
        )
    })?;

    let src_path = path.join("src");
    std::fs::create_dir_all(&src_path).map_err(|err| {
        miette::miette!(
            "Failed to create project game crate src directory: '{}'. {err}",
            src_path.display()
        )
    })?;

    let lib_rs = format!(
        r#"
use arcana::{{
    edict::{{Scheduler, World}},
    plugin::ArcanaPlugin,
    export_arcana_plugin,
}};

export_arcana_plugin!(ThePlugin);

pub struct ThePlugin;

impl ArcanaPlugin for ThePlugin {{
    fn name(&self) -> &'static str {{
        "{name}"
    }}

    fn init(&self, _world: &mut World, _scheduler: &mut Scheduler) {{
        unimplemented!()
    }}
}}
"#
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

/// Construct a command to run ed for arcana project.
pub fn run_editor(root: &Path) -> Command {
    let workspace = root.join(WORKSPACE_DIR_NAME);
    let mut cmd = Command::new("cargo");
    cmd.arg("run")
        .arg("--package=ed")
        // .arg("--release")
        .env("RUSTFLAGS", "-Zshare-generics=off -Cprefer-dynamic=yes")
        .current_dir(&workspace);
    cmd
}

/// Construct a command to run ed for arcana project.
pub fn build_game(root: &Path) -> Command {
    let workspace = root.join(WORKSPACE_DIR_NAME);
    let mut cmd = Command::new("cargo");
    cmd.arg("build")
        .arg("--package=game")
        // .arg("--release")
        .env("RUSTFLAGS", "-Zshare-generics=off")
        .current_dir(&workspace);
    cmd
}

/// Spawn async plugins building process.
/// Returns BuildProcess that can be used to determine expected shared lib artefact
/// and poll build completion.
pub fn build_plugins(root: &Path) -> miette::Result<BuildProcess> {
    let workspace = root.join(WORKSPACE_DIR_NAME);

    let child = Command::new("cargo")
        .arg("build")
        // .arg("--verbose")
        // .arg("--release")
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

/// Construct expected plugin build artifact path.
pub fn game_bin_path(name: &str, workspace: &Path) -> PathBuf {
    let mut bin_path = workspace.join(WORKSPACE_DIR_NAME);
    bin_path.push("target");
    bin_path.push("debug"); // Hardcoded for now.
    bin_path.push(format!("{name}{EXE_SUFFIX}"));
    bin_path
}
