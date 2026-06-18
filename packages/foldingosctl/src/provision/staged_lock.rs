use std::fs::{self, File};

use nix::fcntl::{Flock, FlockArg};

use crate::paths::AppliancePaths;

pub fn with_staged_update_lock<F>(paths: &AppliancePaths, operation: F) -> Result<(), String>
where
    F: FnOnce() -> Result<(), String>,
{
    if let Some(parent) = paths.staged_update_lock.parent() {
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    let lock = File::options()
        .read(true)
        .write(true)
        .create(true)
        .open(&paths.staged_update_lock)
        .map_err(|error| format!("open staged update lock: {error}"))?;
    let _guard = Flock::lock(lock, FlockArg::LockExclusive)
        .map_err(|(_, errno)| format!("acquire staged update lock: {errno}"))?;
    operation()
}
