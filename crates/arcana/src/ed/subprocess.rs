use std::process::Child;

use parking_lot::Mutex;

static SUBPROCESSES: Mutex<Vec<Child>> = Mutex::new(Vec::new());

pub fn kill_subprocesses() {
    let subprocesses = std::mem::take(&mut *SUBPROCESSES.lock());
    for mut child in subprocesses {
        let _ = child.kill();
    }
}

pub fn filter_subprocesses() {
    let mut subprocesses = SUBPROCESSES.lock();
    subprocesses.retain_mut(|child| match child.try_wait() {
        Ok(Some(_)) => false,
        Err(_) => false,
        _ => true,
    });
}
