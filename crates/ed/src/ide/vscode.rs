use std::path::Path;

use crate::subprocess;

use super::Ide;

pub struct VSCode;

pub struct VSCodeError(std::io::Error);

impl Ide for VSCode {
    type Error = VSCodeError;

    fn open(&self, path: &Path, line: Option<u32>) -> Result<(), VSCodeError> {
        let child = std::process::Command::new("code")
            .arg("--goto")
            .arg(format!(
                "{}:{}:{}",
                path.to_str().unwrap(),
                line.unwrap_or(0),
                0
            ))
            .spawn()
            .map_err(VSCodeError)?;

        SUBPROCESSES.lock().push(child);
        Ok(())
    }
}
