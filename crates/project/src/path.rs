use std::{
    borrow::Borrow,
    fmt,
    ops::Deref,
    path::{Component, Path, PathBuf},
};

fn normalized_extend(result: &mut PathBuf, components: std::path::Components) -> bool {
    let mut too_many_parent_dirs = false;

    for comp in components {
        match comp {
            Component::Normal(name) => {
                result.push(name);
            }
            Component::CurDir => {
                let prefix_only = result
                    .components()
                    .any(|c| !matches!(c, Component::Prefix(_)));

                if prefix_only {
                    result.push(Component::CurDir);
                }
            }
            Component::ParentDir => {
                while result.ends_with(Component::CurDir) {
                    result.pop();
                }

                if result.ends_with(Component::ParentDir) {
                    result.push(Component::ParentDir);
                } else if !result.pop() {
                    if result.ends_with(Component::RootDir) {
                        too_many_parent_dirs = true;
                    } else {
                        result.push(Component::ParentDir);
                    }
                }
            }
            Component::RootDir => {
                too_many_parent_dirs = false;

                let tmp = std::mem::take(result);

                match tmp.components().next() {
                    Some(Component::Prefix(prefix)) => {
                        result.clear();
                        result.push(Component::Prefix(prefix));
                    }
                    _ => {}
                }

                result.push(Component::RootDir);
            }
            Component::Prefix(prefix) => {
                too_many_parent_dirs = false;
                result.clear();
                result.push(Component::Prefix(prefix));
            }
        }
    }

    !too_many_parent_dirs
}

pub fn normalized_path(path: &Path) -> Option<PathBuf> {
    normalizing_join(PathBuf::new(), path)
}

pub fn normalizing_join(mut base: PathBuf, path: &Path) -> Option<PathBuf> {
    if normalized_extend(&mut base, path.components()) {
        Some(base)
    } else {
        None
    }
}

/// Returns absolute path.
/// If path is relative, it is resolved relative to current directory.
pub fn real_path(path: &Path) -> Option<PathBuf> {
    if path.is_absolute() {
        return Some(path.to_owned());
    }

    let cd = std::env::current_dir().ok()?;
    if !cd.is_absolute() {
        return None;
    }

    normalizing_join(cd, path)
}

// pub struct RealPathError {
//     path: PathBuf,
// }

// impl fmt::Debug for RealPathError {
//     fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
//         fmt::Display::fmt(self, f)
//     }
// }

// impl fmt::Display for RealPathError {
//     fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
//         write!(f, "Failed to resolve path {}", self.path.display())
//     }
// }

// impl std::error::Error for RealPathError {}

// pub fn real_path(path: &Path) -> Result<PathBuf, RealPathError> {
//     _real_path(path).ok_or_else(|| RealPathError {
//         path: path.to_owned(),
//     })
// }

/// Returns relative path from base to path.
pub fn make_relative(path: &Path, base: &Path) -> PathBuf {
    let mut path_components = path.components();
    let mut base_components = base.components();

    let mut result = PathBuf::new();

    loop {
        match (path_components.next(), base_components.next()) {
            (Some(Component::Prefix(path_prefix)), Some(Component::Prefix(base_prefix))) => {
                if path_prefix != base_prefix {
                    return path.to_owned();
                }
            }
            (Some(Component::Prefix(_)), _) => {
                return path.to_owned();
            }
            (_, Some(Component::Prefix(_))) => {
                panic!("Path must be absolute if base is absolute");
            }
            (Some(Component::RootDir), Some(Component::RootDir)) => {}
            (Some(Component::RootDir), _) => {
                return path.to_owned();
            }
            (_, Some(Component::RootDir)) => {
                panic!("Path must be absolute if base is absolute");
            }
            (Some(path_component), Some(base_component)) => {
                if path_component != base_component {
                    result.push("..");
                    for _ in base_components {
                        result.push("..");
                    }
                    result.push(path_component);
                    break;
                }
            }
            (Some(path_component), None) => {
                result.push(path_component);
                break;
            }
            (None, Some(_)) => {
                result.push("..");
                break;
            }
            (None, None) => return PathBuf::from("."),
        }
    }

    for component in path_components {
        result.push(component);
    }

    result
}
