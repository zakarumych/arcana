use std::{
    fs::{self, File, OpenOptions},
    io::{self, Write},
    path::{Path, PathBuf},
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};

use parking_lot::{Mutex, MutexGuard};
use tracing_subscriber::fmt::MakeWriter;

const MAX_FILE_BYTES: u64 = 1_000_000_000;
const PREFIX: &str = "arcana-editor-";

#[derive(Clone)]
pub(crate) struct LogWriter(Arc<Mutex<State>>);

struct State {
    directory: PathBuf,
    run: String,
    _run_lock: File,
    segment: u64,
    file: Option<File>,
    path: PathBuf,
    bytes: u64,
    max_bytes: u64,
    keep: usize,
    reported_error: bool,
}

impl LogWriter {
    pub(crate) fn current_path(&self) -> PathBuf {
        self.0.lock().path.clone()
    }

    pub(crate) fn files(&self) -> io::Result<Vec<PathBuf>> {
        let directory = self.0.lock().directory.clone();
        let mut files = Vec::new();
        for entry in fs::read_dir(directory)? {
            let entry = entry?;
            if !entry.file_name().to_str().is_some_and(is_log_name) || !entry.file_type()?.is_file()
            {
                continue;
            }
            let metadata = entry.metadata()?;
            let created = metadata.created().or_else(|_| metadata.modified())?;
            files.push((created, entry.path()));
        }
        files.sort_unstable_by(|a, b| b.cmp(a));
        Ok(files.into_iter().map(|(_, path)| path).collect())
    }

    pub(crate) fn new(keep: usize) -> io::Result<Self> {
        let directory = match dirs::data_local_dir() {
            Some(path) => path.join("Arcana Engine").join("Logs"),
            None => std::env::current_exe()?
                .parent()
                .ok_or_else(|| io::Error::other("Executable has no parent directory"))?
                .join("Logs"),
        };
        Self::in_directory(directory, keep, MAX_FILE_BYTES)
    }

    fn in_directory(directory: PathBuf, keep: usize, max_bytes: u64) -> io::Result<Self> {
        assert!(max_bytes > 0);
        fs::create_dir_all(&directory)?;
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(io::Error::other)?
            .as_nanos();
        let mut collision = 0u32;
        let (run, run_lock) = loop {
            let run = format!(
                "{PREFIX}{timestamp:039}-{:010}-{collision:010}",
                std::process::id()
            );
            let lock_path = directory.join(format!("{run}.lock"));
            match OpenOptions::new()
                .read(true)
                .write(true)
                .create_new(true)
                .open(lock_path)
            {
                Ok(file) => {
                    file.try_lock().map_err(io::Error::other)?;
                    break (run, file);
                }
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
                    collision = collision.checked_add(1).ok_or(error)?;
                }
                Err(error) => return Err(error),
            }
        };
        let path = directory.join(format!("{run}-0000000000.log"));
        let file = match create_log(&path) {
            Ok(file) => file,
            Err(error) => {
                let _ = fs::remove_file(directory.join(format!("{run}.lock")));
                return Err(error);
            }
        };
        let mut state = State {
            directory,
            run,
            _run_lock: run_lock,
            segment: 0,
            file: Some(file),
            path,
            bytes: 0,
            max_bytes,
            keep: keep.max(1),
            reported_error: false,
        };
        state.cleanup();
        Ok(Self(Arc::new(Mutex::new(state))))
    }
}

fn create_log(path: &Path) -> io::Result<File> {
    OpenOptions::new().write(true).create_new(true).open(path)
}

fn is_log_name(name: &str) -> bool {
    let Some(name) = name
        .strip_prefix(PREFIX)
        .and_then(|name| name.strip_suffix(".log"))
    else {
        return false;
    };
    let mut parts = name.split('-');
    for width in [39, 10, 10, 10] {
        let Some(part) = parts.next() else {
            return false;
        };
        if part.len() != width || !part.bytes().all(|byte| byte.is_ascii_digit()) {
            return false;
        }
    }
    parts.next().is_none()
}

impl State {
    fn report(&mut self, error: &io::Error) {
        if !self.reported_error {
            eprintln!("Editor log file error: {error}");
            self.reported_error = true;
        }
    }

    fn cleanup(&mut self) {
        if let Err(error) = self.prune() {
            self.report(&error);
        }
    }

    fn prune(&self) -> io::Result<()> {
        let mut files = Vec::new();
        let mut first_error = None;
        for entry in fs::read_dir(&self.directory)? {
            let entry = match entry {
                Ok(entry) => entry,
                Err(error) => {
                    first_error.get_or_insert(error);
                    continue;
                }
            };
            if entry.file_name().to_str().is_some_and(is_log_name) {
                match entry.file_type() {
                    Ok(kind) if kind.is_file() => match entry.metadata() {
                        Ok(metadata) => match metadata.created().or_else(|_| metadata.modified()) {
                            Ok(created) => files.push((created, entry.path())),
                            Err(error) => {
                                first_error.get_or_insert(error);
                            }
                        },
                        Err(error) => {
                            first_error.get_or_insert(error);
                        }
                    },
                    Ok(_) => {}
                    Err(error) => {
                        first_error.get_or_insert(error);
                    }
                }
            }
        }
        files.sort_unstable();
        let remove = files.len().saturating_sub(self.keep);
        for (_, path) in files.into_iter().take(remove) {
            if path == self.path {
                continue;
            }
            let name = path.file_name().unwrap().to_str().unwrap();
            let (run, _) = name.rsplit_once('-').unwrap();
            let lock_path = self.directory.join(format!("{run}.lock"));
            let _lock = if run == self.run {
                None
            } else {
                match OpenOptions::new().read(true).write(true).open(&lock_path) {
                    Ok(file) => match file.try_lock() {
                        Ok(()) => Some(file),
                        Err(std::fs::TryLockError::WouldBlock) => continue,
                        Err(std::fs::TryLockError::Error(error)) => {
                            first_error.get_or_insert(error);
                            continue;
                        }
                    },
                    Err(error) if error.kind() == io::ErrorKind::NotFound => None,
                    Err(error) => {
                        first_error.get_or_insert(error);
                        continue;
                    }
                }
            };
            if let Err(error) = fs::remove_file(&path) {
                if error.kind() != io::ErrorKind::NotFound {
                    first_error.get_or_insert(error);
                }
                continue;
            }
            // Run IDs are reserved before any segments exist and are never reused.
            let remaining = fs::read_dir(&self.directory)?.any(|entry| {
                entry.map_or(true, |entry| {
                    entry.file_name().to_str().is_some_and(|name| {
                        is_log_name(name)
                            && name.rsplit_once('-').is_some_and(|(other, _)| other == run)
                    })
                })
            });
            if !remaining {
                if let Err(error) = fs::remove_file(lock_path) {
                    if error.kind() != io::ErrorKind::NotFound {
                        first_error.get_or_insert(error);
                    }
                }
            }
        }
        match first_error {
            Some(error) => Err(error),
            None => Ok(()),
        }
    }

    fn rotate(&mut self) -> io::Result<()> {
        let segment = self
            .segment
            .checked_add(1)
            .filter(|segment| *segment <= 9_999_999_999)
            .ok_or_else(|| io::Error::other("Editor log segment limit reached"))?;
        let path = self
            .directory
            .join(format!("{}-{segment:010}.log", self.run));
        let file = create_log(&path)?;
        self.file = Some(file);
        self.segment = segment;
        self.path = path;
        self.bytes = 0;
        self.cleanup();
        Ok(())
    }

    fn write_all(&mut self, mut bytes: &[u8]) -> io::Result<()> {
        while !bytes.is_empty() {
            if self.bytes == self.max_bytes {
                self.rotate()?;
            }
            let count = usize::try_from(self.max_bytes - self.bytes)
                .unwrap_or(usize::MAX)
                .min(bytes.len());
            self.file
                .as_mut()
                .expect("Active log file")
                .write_all(&bytes[..count])?;
            self.bytes += count as u64;
            bytes = &bytes[count..];
        }
        Ok(())
    }
}

pub(crate) struct EventWriter<'a>(MutexGuard<'a, State>);

impl<'a> MakeWriter<'a> for LogWriter {
    type Writer = EventWriter<'a>;

    fn make_writer(&'a self) -> Self::Writer {
        EventWriter(self.0.lock())
    }
}

impl Write for EventWriter<'_> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if self.0.file.is_some() {
            if let Err(error) = self.0.write_all(bytes) {
                self.0.report(&error);
                self.0.file = None;
            }
        }
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        if let Some(file) = &mut self.0.file {
            if let Err(error) = file.flush() {
                self.0.report(&error);
                self.0.file = None;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};
    use tracing_subscriber::prelude::*;

    struct Directory(PathBuf);

    impl Directory {
        fn new() -> Self {
            static NEXT: AtomicU64 = AtomicU64::new(0);
            let path = std::env::temp_dir().join(format!(
                "arcana-logging-test-{}-{}-{}",
                std::process::id(),
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_nanos(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir_all(&path).unwrap();
            Self(path)
        }

        fn logs(&self) -> Vec<PathBuf> {
            let mut paths: Vec<_> = fs::read_dir(&self.0)
                .unwrap()
                .map(|entry| entry.unwrap().path())
                .filter(|path| path.file_name().unwrap().to_str().is_some_and(is_log_name))
                .collect();
            paths.sort();
            paths
        }

        fn writer(&self, keep: usize, cap: u64) -> LogWriter {
            LogWriter::in_directory(self.0.clone(), keep, cap).unwrap()
        }
    }

    impl Drop for Directory {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.0).unwrap();
        }
    }

    #[test]
    fn fresh_runs_and_exact_rotation() {
        let directory = Directory::new();
        let first = directory.writer(10, 4);
        first.make_writer().write_all(b"abcdefghij").unwrap();
        let paths = directory.logs();
        drop(first);
        assert_eq!(paths.len(), 3);
        assert_eq!(fs::read(&paths[0]).unwrap(), b"abcd");
        assert_eq!(fs::read(&paths[1]).unwrap(), b"efgh");
        assert_eq!(fs::read(&paths[2]).unwrap(), b"ij");
        let second = directory.writer(10, 4);
        second.make_writer().write_all(b"klmn").unwrap();
        assert_eq!(directory.logs().len(), 4);
        assert_eq!(fs::read(&paths[2]).unwrap(), b"ij");
        // Reaching the cap does not create an empty trailing segment.
        second.make_writer().write_all(b"o").unwrap();
        assert_eq!(directory.logs().len(), 5);
    }

    #[test]
    fn retention_applies_at_startup_and_rotation() {
        let directory = Directory::new();
        for _ in 0..4 {
            drop(directory.writer(10, 4));
        }
        let writer = directory.writer(2, 4);
        assert_eq!(directory.logs().len(), 2);
        writer.make_writer().write_all(b"123456789").unwrap();
        let paths = directory.logs();
        drop(writer);
        assert_eq!(paths.len(), 2);
        assert_eq!(fs::read(&paths[0]).unwrap(), b"5678");
        assert_eq!(fs::read(&paths[1]).unwrap(), b"9");
        assert_eq!(fs::read_dir(&directory.0).unwrap().count(), 3);
    }

    #[test]
    fn retention_preserves_active_files_and_newest_n() {
        let directory = Directory::new();
        let active = directory.writer(10, 4);
        let active_path = active.0.lock().path.clone();
        drop(directory.writer(10, 4));
        drop(directory.writer(10, 4));
        let newest_previous = directory.logs().pop().unwrap();
        let _current = directory.writer(2, 4);
        let paths = directory.logs();
        assert_eq!(paths.len(), 3);
        assert!(paths.contains(&active_path));
        assert!(paths.contains(&newest_previous));
        drop(active);
        let _next = directory.writer(2, 4);
        assert!(!active_path.exists());
    }

    #[test]
    fn retention_only_deletes_exact_log_names() {
        let directory = Directory::new();
        for name in [
            "notes.log",
            "arcana-editor-1-2-3-4.log",
            "arcana-editor-invalid.log",
        ] {
            fs::write(directory.0.join(name), b"preserve").unwrap();
        }
        let writer = directory.writer(0, 4);
        writer.make_writer().write_all(b"123456789").unwrap();
        assert_eq!(directory.logs().len(), 1);
        for name in [
            "notes.log",
            "arcana-editor-1-2-3-4.log",
            "arcana-editor-invalid.log",
        ] {
            assert_eq!(fs::read(directory.0.join(name)).unwrap(), b"preserve");
        }
    }

    #[test]
    fn retention_orders_segments_by_creation_across_overlapping_runs() {
        let directory = Directory::new();
        let older_run = directory.writer(2, 4);
        older_run.make_writer().write_all(b"1234").unwrap();
        std::thread::sleep(std::time::Duration::from_millis(10));
        drop(directory.writer(10, 4));
        std::thread::sleep(std::time::Duration::from_millis(10));
        older_run.make_writer().write_all(b"56789").unwrap();
        let paths = directory.logs();
        drop(older_run);
        assert_eq!(paths.len(), 2);
        let mut contents: Vec<_> = paths.iter().map(|path| fs::read(path).unwrap()).collect();
        contents.sort();
        assert_eq!(contents, [b"5678".to_vec(), b"9".to_vec()]);
    }

    #[test]
    fn tracing_formats_plain_text_and_filters_events() {
        let directory = Directory::new();
        let writer = directory.writer(10, 1000);
        let subscriber = tracing_subscriber::registry()
            .with(
                tracing_subscriber::fmt::layer()
                    .with_ansi(false)
                    .with_writer(writer.clone()),
            )
            .with(tracing_subscriber::EnvFilter::new("info"));
        tracing::subscriber::with_default(subscriber, || {
            tracing::info!(answer = 42, "editor message");
            tracing::debug!("filtered message");
        });
        let text = fs::read_to_string(&directory.logs()[0]).unwrap();
        assert!(text.contains("editor message"));
        assert!(text.contains("answer=42"));
        assert!(!text.contains("filtered message"));
        assert!(!text.contains('\u{1b}'));
    }

    #[test]
    fn log_listing_excludes_unrelated_files_and_orders_newest_first() {
        let directory = Directory::new();
        let first = directory.writer(10, 4);
        let first_path = first.current_path();
        std::thread::sleep(std::time::Duration::from_millis(10));
        let second = directory.writer(10, 4);
        let current = second.current_path();
        fs::write(directory.0.join("notes.log"), b"unrelated").unwrap();
        fs::write(directory.0.join("arcana-editor-invalid.log"), b"unrelated").unwrap();
        let directory_name = format!("{}-0000000001.log", second.0.lock().run);
        fs::create_dir(directory.0.join(directory_name)).unwrap();
        assert_eq!(second.files().unwrap(), [current.clone(), first_path]);
        second.make_writer().write_all(b"live").unwrap();
        assert_eq!(fs::read(current).unwrap(), b"live");
    }

    #[test]
    fn current_log_path_follows_rotation() {
        let directory = Directory::new();
        let writer = directory.writer(10, 4);
        let initial = writer.current_path();
        writer.make_writer().write_all(b"12345").unwrap();
        let current = writer.current_path();
        assert_ne!(initial, current);
        assert_eq!(writer.files().unwrap()[0], current);
        assert_eq!(fs::read(current).unwrap(), b"5");
    }

    #[test]
    fn logging_preferences_accept_old_config_and_round_trip() {
        let cfg: crate::app::AppConfig = serde_json::from_str(r#"{"ide":null}"#).unwrap();
        let json = serde_json::to_value(&cfg).unwrap();
        assert_eq!(json["log_files_to_keep"], 10);
        let cfg: crate::app::AppConfig =
            serde_json::from_str(r#"{"ide":null,"log_files_to_keep":25}"#).unwrap();
        let cfg =
            serde_json::from_value::<crate::app::AppConfig>(serde_json::to_value(cfg).unwrap())
                .unwrap();
        assert_eq!(serde_json::to_value(cfg).unwrap()["log_files_to_keep"], 25);
    }
}
