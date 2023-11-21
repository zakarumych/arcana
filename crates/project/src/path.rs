use std::{
    borrow::Borrow,
    fmt,
    ops::Deref,
    path::{Component, Path, PathBuf},
};

pub fn normalizing_join(mut base: PathBuf, path: &Path) -> Option<PathBuf> {
    let mut comps = path.components();
    while let Some(comp) = comps.next() {
        match comp {
            Component::Normal(name) => {
                base.push(name);
            }
            Component::Prefix(_) | Component::RootDir => {
                base.clear();
                base.push(comp);
                for comp in comps {
                    base.push(comp);
                }
                return Some(base);
            }
            Component::CurDir => {}
            Component::ParentDir => {
                if base.pop() {
                    return None;
                }
            }
        }
    }
    Some(base)
}

/// Returns absolute resolved path.
/// If path is relative, it is resolved relative to current directory.
pub fn real_path(path: &Path) -> Option<PathBuf> {
    for a in path.ancestors() {
        if let Ok(base) = dunce::canonicalize(a) {
            let tail = path.strip_prefix(a).unwrap();
            let path = normalizing_join(base, tail)?;

            // Base was canonicalized
            // Tail does not exist and normalizing_join normalized result.
            return Some(path);
        }
    }

    if path.is_absolute() {
        return Some(path.to_owned());
    }

    let cd = std::env::current_dir().ok()?;
    let path = normalizing_join(cd, path)?;

    // Current directory was canonicalized
    // Path does not exist and normalizing_join normalized result.
    Some(path)
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
            (Some(Component::Prefix(_)), _) | (_, Some(Component::Prefix(_))) => {
                return path.to_owned();
            }
            (Some(Component::RootDir), Some(Component::RootDir)) => {}
            (Some(Component::RootDir), _) | (_, Some(Component::RootDir)) => {
                return path.to_owned();
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
