use rocksdb::{DB, Error as RocksDBError};
use std::thread::sleep;
use std::time::Duration;
use fs2::free_space;

pub fn with_retry<F, T>(db: &DB, mut operation: F) -> Result<T, RocksDBError>
where
    F: FnMut(&DB) -> Result<T, RocksDBError>,
{
    loop {
        match operation(db) {
            Ok(result) => return Ok(result),
            Err(e) => {
                if e.as_ref().starts_with("IO error: No space left on device") {
                    let path = db.path();
                    let free = free_space(path).unwrap_or(0);
                    log::info!(
                        "Out of space writing to {}. Free space: {}. Retrying in 20s...",
                        path.display(),
                        free
                    );
                    sleep(Duration::from_secs(20));
                    continue;
                }
                return Err(e);
            }
        }
    }
}