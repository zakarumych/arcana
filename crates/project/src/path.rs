use std::{
    borrow::Borrow,
    fmt,
    ops::Deref,
    path::{Component, Path, PathBuf},
};

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

/// Checks if path is available for creating a file.
///
/// This doesn't check permissions for file creation,
/// but checks that path is not occupied,
/// and no ancestor is a file or inaccessible,
/// and that some first existing ancestor is a directory.
pub fn is_available(path: &Path) -> bool {
    if path.exists() {
        return false;
    }

    let Some(parent) = path.parent() else {
        return false;
    };

    for ancestor in parent.ancestors() {
        match ancestor.metadata() {
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
                continue;
            }
            Err(_) => {
                return false;
            }
            Ok(metadata) => {
                if metadata.is_dir() {
                    return true;
                } else {
                    return false;
                }
            }
        }
    }

    false
}
