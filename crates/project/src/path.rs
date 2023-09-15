use std::{
    borrow::Borrow,
    fmt,
    ops::Deref,
    path::{Component, Path, PathBuf},
};

/// Contains absolute resolved path.
/// It does not contain any symlinks, current directory and parent directory components.
/// It may not exist.
#[derive(Clone)]
#[repr(transparent)]
pub struct RealPathBuf(PathBuf);

impl RealPathBuf {
    pub fn as_path(&self) -> &Path {
        &self.0
    }

    pub fn into_path(self) -> PathBuf {
        self.0
    }

    pub fn pop(&mut self) -> bool {
        self.0.pop()
    }
}

impl Borrow<Path> for RealPathBuf {
    fn borrow(&self) -> &Path {
        &self.0
    }
}

impl Borrow<RealPath> for RealPathBuf {
    fn borrow(&self) -> &RealPath {
        self
    }
}

impl Deref for RealPathBuf {
    type Target = RealPath;

    fn deref(&self) -> &Self::Target {
        RealPath::wrap(&self.0)
    }
}

impl fmt::Debug for RealPathBuf {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        <Path as fmt::Debug>::fmt(self.as_ref(), f)
    }
}

impl AsRef<Path> for RealPathBuf {
    fn as_ref(&self) -> &Path {
        &self.0
    }
}

impl AsRef<RealPath> for RealPathBuf {
    fn as_ref(&self) -> &RealPath {
        self
    }
}

/// Contains absolute resolved path.
/// It does not contain any symlinks, current directory and parent directory components.
/// It may not exist.
#[repr(transparent)]
pub struct RealPath(Path);

impl RealPath {
    pub fn wrap(path: &Path) -> &Self {
        unsafe { &*(path as *const Path as *const RealPath) }
    }

    pub fn as_path(&self) -> &Path {
        &self.0
    }

    pub fn join(&self, path: impl AsRef<Path>) -> PathBuf {
        self.0.join(path)
    }

    pub fn metadata(&self) -> std::io::Result<std::fs::Metadata> {
        std::fs::metadata(&self.0)
    }

    pub fn display(&self) -> std::path::Display<'_> {
        self.0.display()
    }

    pub fn file_name(&self) -> Option<&std::ffi::OsStr> {
        self.0.file_name()
    }

    pub fn parent(&self) -> Option<&Self> {
        self.0.parent().map(Self::wrap)
    }

    pub fn ancestors(&self) -> impl Iterator<Item = &Self> {
        self.0.ancestors().map(Self::wrap)
    }

    pub fn exists(&self) -> bool {
        self.0.exists()
    }
}

impl ToOwned for RealPath {
    type Owned = RealPathBuf;

    fn to_owned(&self) -> Self::Owned {
        RealPathBuf(self.0.to_owned())
    }
}

impl fmt::Debug for RealPath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        <Path as fmt::Debug>::fmt(self.as_ref(), f)
    }
}

impl AsRef<Path> for RealPath {
    fn as_ref(&self) -> &Path {
        &self.0
    }
}

fn _join(mut base: PathBuf, path: &Path) -> Option<PathBuf> {
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
pub fn _real_path(path: &Path) -> Option<RealPathBuf> {
    for a in path.ancestors() {
        if let Ok(base) = dunce::canonicalize(a) {
            let tail = path.strip_prefix(a).unwrap();
            let path = _join(base, tail)?;

            // Base was canonicalized
            // Tail does not exist and _join normalized result.
            return Some(RealPathBuf(path));
        }
    }

    let cd = std::env::current_dir().ok()?;
    let cd = dunce::canonicalize(cd).ok()?;
    let path = _join(cd, path)?;

    // Current directory was canonicalized
    // Path does not exist and _join normalized result.
    Some(RealPathBuf(path))
}

pub struct RealPathError {
    path: PathBuf,
}

impl fmt::Debug for RealPathError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}

impl fmt::Display for RealPathError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Failed to resolve path {}", self.path.display())
    }
}

impl std::error::Error for RealPathError {}

pub fn real_path(path: &Path) -> Result<RealPathBuf, RealPathError> {
    _real_path(path).ok_or_else(|| RealPathError {
        path: path.to_owned(),
    })
}

/// Returns relative path from base to path.
pub fn make_relative(path: &RealPath, base: &RealPath) -> PathBuf {
    let mut path_components = path.0.components().peekable();
    let mut base_components = base.0.components().peekable();

    let mut result = PathBuf::new();

    match (path_components.peek(), base_components.peek()) {
        (Some(Component::Prefix(path_prefix)), Some(Component::Prefix(base_prefix))) => {
            if path_prefix != base_prefix {
                return path.as_path().to_owned();
            }
            path_components.next();
            base_components.next();
        }
        (Some(Component::Prefix(_)), _) | (_, Some(Component::Prefix(_))) => {
            return path.as_path().to_owned();
        }
        _ => {}
    }

    loop {
        match (path_components.next(), base_components.next()) {
            (Some(path_component), Some(base_component)) => {
                if path_component != base_component {
                    result.push("..");
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

    for _ in base_components {
        result.push("..");
    }

    for component in path_components {
        result.push(component);
    }

    result
}
