use crate::processing::ProcessingError;
use anyhow::Result;
use reqwest;
use reqwest::header::ToStrError;
use ringmap;
use std::collections;
use std::fs;
use std::path::{Path, PathBuf};
use std::result::Result as StdResult;
use std::sync::mpsc;
use std::thread;
use std::time;

#[derive(Debug, PartialEq)]
pub enum MaybePath {
    Path(PathBuf),
    InProgress,
    NotAvailable(PathBuf),
}

#[derive(Debug, Clone)]
pub struct GeneratorError {
    message: String,
}

impl GeneratorError {
    pub fn new(message: &str) -> Self {
        GeneratorError {
            message: message.into(),
        }
    }
}

impl std::error::Error for GeneratorError {}
impl std::fmt::Display for GeneratorError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "generator error: {}", self.message)
    }
}
impl From<ProcessingError> for GeneratorError {
    fn from(error: ProcessingError) -> Self {
        GeneratorError {
            message: format!("{}", error),
        }
    }
}
impl From<reqwest::Error> for GeneratorError {
    fn from(error: reqwest::Error) -> Self {
        GeneratorError {
            message: format!("reqwest: {}", error),
        }
    }
}
impl From<std::env::VarError> for GeneratorError {
    fn from(_: std::env::VarError) -> Self {
        GeneratorError {
            message: format!("env"),
        }
    }
}
impl From<ToStrError> for GeneratorError {
    fn from(_: ToStrError) -> Self {
        GeneratorError {
            message: format!("tostr"),
        }
    }
}
impl From<std::io::Error> for GeneratorError {
    fn from(_: std::io::Error) -> Self {
        GeneratorError {
            message: format!("io"),
        }
    }
}

pub type CacheResult = StdResult<PathBuf, GeneratorError>;
type SendBack = mpsc::Sender<CacheResult>;
type GetBack = mpsc::Receiver<CacheResult>;

#[derive(Debug)]
pub struct Request {
    pub key: PathBuf,
    pub send_back: SendBack,
}

#[derive(Debug)]
pub struct Entry {
    path: PathBuf,
    bytes: u64,
    created: time::SystemTime,
}

#[derive(Debug)]
pub struct Cache {
    pub cache: PathBuf,
    items: ringmap::RingMap<PathBuf, Entry>,
    size_bytes: u64,
    max_size_bytes: u64,
    shrink_step_bytes: u64,
    item_timeout: time::Duration,
    in_progress: collections::HashMap<PathBuf, thread::JoinHandle<CacheResult>>,
    to_return: collections::HashMap<PathBuf, Vec<SendBack>>,
}

pub fn get<F>(cache: &mut Cache, key: PathBuf, generator: F) -> CacheResult
where
    F: FnOnce() -> CacheResult + Send + 'static,
{
    let (tx, rx) = mpsc::channel();
    cache.try_get(Request { key, send_back: tx }, generator);
    return rx.recv().unwrap();
}

pub fn run_cache(
    mut cache: Cache,
) -> mpsc::Sender<(Request, Box<dyn FnOnce() -> CacheResult + Send>)> {
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || cache_thread(&mut cache, rx));
    return tx;
}

fn cache_thread(
    cache: &mut Cache,
    rx: mpsc::Receiver<(Request, Box<dyn FnOnce() -> CacheResult + Send>)>,
) {
    loop {
        // TODO also receive the results of getters, then there is no reason to sleep
        if let Ok(todo) = rx.try_recv() {
            log::info!("cache processing {:?}", todo.0);
            cache.try_get(todo.0, todo.1);
        }
        cache.check();
        cache.cleanup();
        thread::sleep(time::Duration::from_millis(1));
    }
}

impl Cache {
    pub fn from_existing_directory(
        path: PathBuf,
        max_size_bytes: u64,
        shrink_step_bytes: u64,
        age_limit_seconds: u64,
    ) -> Result<Self> {
        let mut items = ringmap::RingMap::new();
        log::info!("build cache from {:?}", path);
        if !path.exists() {
            fs::create_dir_all(&path)?;
        }
        let size_bytes = add_all(&path, &Path::new(""), &mut items)?;
        let result = Cache {
            cache: path,
            items,
            size_bytes,
            max_size_bytes,
            shrink_step_bytes,
            item_timeout: time::Duration::from_secs(age_limit_seconds),
            in_progress: collections::HashMap::new(),
            to_return: collections::HashMap::new(),
        };
        log::debug!("generated cache: {:?}", result);
        Ok(result)
    }

    pub fn get(&self, key: &PathBuf) -> Option<&PathBuf> {
        match self.items.get(key) {
            Some(value) => Some(&value.path),
            _ => None,
        }
    }

    pub fn try_get<F>(&mut self, get: Request, generator: F)
    where
        F: FnOnce() -> CacheResult + Send + 'static,
    {
        if let Some(path) = self.get(&get.key) {
            // immediately return item if in cache
            log::info!("return item from cache {:?}", get.key);
            get.send_back.send(Ok(path.clone())).unwrap();
        } else {
            if self.in_progress.get(&get.key).is_none() {
                // execute generator if no one already generating this item
                log::info!("generate item {:?}", get.key);
                self.in_progress
                    .insert(get.key.clone(), thread::spawn(generator));
            }
            // add caller to list of askers waiting for result
            log::info!("wait for generating item {:?}", get.key);
            if let Some(vector) = self.to_return.get_mut(&get.key) {
                vector.push(get.send_back);
            } else {
                self.to_return.insert(get.key, vec![get.send_back]);
            }
        }
    }

    pub fn check(&mut self) {
        // get all finished generators
        let mut finished = Vec::new();
        for (key, handle) in self.in_progress.iter() {
            if handle.is_finished() {
                finished.push(key.clone());
            }
        }
        for key in finished {
            // send generator result to each waiting asker
            let handle = self.in_progress.remove(&key).unwrap();
            let result = handle.join().unwrap();
            for sender in self.to_return.remove(&key).unwrap() {
                sender.send(result.clone()).unwrap();
            }
            if let Ok(value) = result {
                let metadata = fs::metadata(&value).unwrap();
                let bytes = metadata.len();
                self.size_bytes += bytes;
                self.items.insert(
                    key.clone(),
                    Entry {
                        path: value,
                        bytes,
                        created: metadata.created().unwrap(),
                    },
                );
            }
        }
    }

    pub fn cleanup(&mut self) {
        self.shrink();
        self.expire();
    }

    /// keep total size of self below max size
    ///
    /// removes up to step_bytes worth of items to avoid thrashing
    fn shrink(&mut self) {
        if self.size_bytes > self.max_size_bytes {
            while self.size_bytes > self.max_size_bytes - self.shrink_step_bytes {
                self.remove_oldest();
            }
        }
    }

    fn expire(&mut self) {
        // TODO remove unwraps
        loop {
            if let Some((_key, oldest)) = self.items.get_index(0) {
                if oldest.created.elapsed().unwrap() > self.item_timeout {
                    self.remove_oldest();
                } else {
                    break;
                }
            } else {
                break;
            }
        }
    }

    fn remove_oldest(&mut self) {
        // TODO remove unwraps
        let (key, entry) = self.items.pop_front().unwrap();
        log::debug!("removing cache item {:?}", key);
        self.size_bytes -= entry.bytes;
        std::fs::remove_file(entry.path).unwrap();
    }
}

fn add_all(
    cache: &Path,
    subpath: &Path,
    items: &mut ringmap::RingMap<PathBuf, Entry>,
) -> Result<u64> {
    let mut size = 0;
    for maybe_entry in fs::read_dir(cache.join(subpath))? {
        let entry = maybe_entry?;
        let entry_path = entry.path();
        if entry_path.is_dir() {
            add_all(cache, &entry_path.strip_prefix(cache)?, items)?;
        } else {
            let key = entry_path.strip_prefix(cache)?;
            let metadata = entry.metadata()?;
            size += metadata.len();
            items.insert(
                key.to_path_buf(),
                Entry {
                    path: entry_path,
                    bytes: metadata.len(),
                    created: metadata.created()?,
                },
            );
        }
    }
    Ok(size)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile;

    fn check_until(cache: &mut Cache, rx: GetBack) -> CacheResult {
        loop {
            cache.check();
            if let Ok(item) = rx.try_recv() {
                return item;
            }
            thread::sleep(time::Duration::from_millis(5));
        }
    }

    fn insert_item(cache: &mut Cache, path: &str, filename_override: &str) -> GetBack {
        let (tx, rx) = mpsc::channel();
        let cache_dir = cache.cache.clone();
        let filename = filename_override.to_string();
        let generator = move || {
            thread::sleep(time::Duration::from_millis(100));
            let full_path = cache_dir.join(&filename);
            fs::write(&full_path, "item").unwrap();
            return Ok(full_path);
        };
        cache.try_get(
            Request {
                key: path.into(),
                send_back: tx,
            },
            generator,
        );
        return rx;
    }

    #[test]
    fn test_load_dir() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("a"), "a").unwrap();
        fs::write(dir.path().join("b"), "b").unwrap();
        fs::create_dir(dir.path().join("c")).unwrap();
        fs::write(dir.path().join("c").join("d"), "d").unwrap();
        let mut cache =
            Cache::from_existing_directory(dir.path().to_path_buf(), 10_000, 100, 600).unwrap();
        println!("{:?}", cache);
        assert!(cache.get(&"a".into()).unwrap().ends_with("a"));
        assert!(cache.get(&"b".into()).unwrap().ends_with("b"));
        assert!(cache
            .get(&Path::new("c").join("d"))
            .unwrap()
            .ends_with("c/d"));
    }

    #[test]
    fn test_add_to_cache() {
        let dir = tempfile::tempdir().unwrap();
        let mut cache =
            Cache::from_existing_directory(dir.path().to_path_buf(), 10_000, 100, 600).unwrap();
        assert!(cache.get(&"a".into()).is_none());

        let cache_dir = cache.cache.clone();
        let generator = move || {
            let full_path = cache_dir.join("a");
            fs::write(&full_path, "a").unwrap();
            return Ok(full_path);
        };
        let (tx, rx) = mpsc::channel();
        cache.try_get(
            Request {
                key: "a".into(),
                send_back: tx,
            },
            generator,
        );
        println!("cache is now {:?}", cache);
        let result = check_until(&mut cache, rx).unwrap();
        println!("cache is now {:?}", cache);
        assert!(result.ends_with("a"));
        assert!(cache.cache.join("a").exists());
    }

    #[test]
    fn test_wait_for_cache_add() {
        let dir = tempfile::tempdir().unwrap();
        let mut cache =
            Cache::from_existing_directory(dir.path().to_path_buf(), 10_000, 100, 600).unwrap();
        let rx = insert_item(&mut cache, "a", "a");
        assert!(rx.try_recv().is_err());
        let result = check_until(&mut cache, rx).unwrap();
        assert!(result.ends_with("a"));
        assert!(cache.cache.join("a").exists());
    }

    #[test]
    fn test_run_generator_only_once() {
        let dir = tempfile::tempdir().unwrap();
        let mut cache =
            Cache::from_existing_directory(dir.path().to_path_buf(), 10_000, 100, 600).unwrap();
        let rx_a = insert_item(&mut cache, "a", "x");
        let rx_b = insert_item(&mut cache, "a", "y");
        let rx_c = insert_item(&mut cache, "a", "z");
        // all the cache calls should return the result of the first generator
        let result_a = check_until(&mut cache, rx_a).unwrap();
        assert!(result_a.ends_with("x"));
        let result_b = rx_b.recv().unwrap().unwrap();
        assert!(result_b.ends_with("x"));
        let result_c = rx_c.recv().unwrap().unwrap();
        assert!(result_c.ends_with("x"));

        assert!(cache.cache.join("x").exists());
        // the other generators should never generate their files
        assert!(!cache.cache.join("y").exists());
        assert!(!cache.cache.join("z").exists());
    }

    #[test]
    fn test_expire_cache_items_after_age_limit() {
        let dir = tempfile::tempdir().unwrap();
        let mut cache =
            Cache::from_existing_directory(dir.path().to_path_buf(), 10_000, 100, 3).unwrap();
        let rx_a = insert_item(&mut cache, "a", "a");
        let _ = check_until(&mut cache, rx_a).unwrap();
        thread::sleep(time::Duration::from_secs(2));
        let rx_b = insert_item(&mut cache, "b", "b");
        let _ = check_until(&mut cache, rx_b).unwrap();
        thread::sleep(time::Duration::from_secs(2));
        cache.cleanup();
        assert!(cache.get(&"a".into()).is_none());
        assert!(cache.get(&"b".into()).is_some());
        thread::sleep(time::Duration::from_secs(2));
        cache.cleanup();
        assert!(cache.get(&"a".into()).is_none());
        assert!(cache.get(&"b".into()).is_none());
    }

    #[test]
    fn test_keep_cache_size_below_limit() {
        let dir = tempfile::tempdir().unwrap();
        let mut cache =
            Cache::from_existing_directory(dir.path().to_path_buf(), 15, 6, 600).unwrap();
        for item in vec!["a", "b", "c", "d"] {
            let rx = insert_item(&mut cache, item, item);
            let _ = check_until(&mut cache, rx).unwrap();
        }
        cache.cleanup();
        assert!(cache.get(&"a".into()).is_none());
        assert!(cache.get(&"b".into()).is_none());
        assert!(cache.get(&"c".into()).is_some());
        assert!(cache.get(&"d".into()).is_some());
    }
}
