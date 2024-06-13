use std::path::Path;

use super::Ide;

pub struct VSCode;

#[derive(Debug, thiserror::Error)]
pub enum VSCodeError {
    #[error("IO error: {0}")]
    Io(std::io::Error),
    #[error("VSCode exited with code: {0:?}")]
    ExitError(Option<i32>),
}

impl Ide for VSCode {
    fn open(&self, path: &Path, line: Option<u32>) -> bool {
        #[cfg(windows)]
        let mut cmd = std::process::Command::new("cmd");
        #[cfg(windows)]
        cmd.arg("/c");

        #[cfg(unix)]
        let mut cmd = std::process::Command::new("sh");
        #[cfg(unix)]
        cmd.arg("-c");

        cmd.arg("code").arg("--goto").arg(format!(
            "{}:{}:{}",
            path.to_str().unwrap(),
            line.unwrap_or(0),
            0
        ));

        tracing::debug!("Running VSCode: {:#?}", cmd);

        let result = cmd.status();

        let status = try_log_err!(result; false);

        match status.code() {
            Some(0) => true,
            Some(code) => {
                tracing::error!("VSCode exited with exit code: {}", code);
                false
            }
            None => {
                tracing::error!("VSCode exited with no exit code");
                false
            }
        }
    }
}
