use crate::processing::ProcessingError;
use anyhow::Result;
use reqwest;
use reqwest::header::ToStrError;
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

#[derive(Debug, Clone, Copy)]
pub struct GeneratorError;

impl std::error::Error for GeneratorError {}
impl std::fmt::Display for GeneratorError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "generator error")
    }
}
impl From<ProcessingError> for GeneratorError {
    fn from(_: ProcessingError) -> Self {
        GeneratorError
    }
}
impl From<reqwest::Error> for GeneratorError {
    fn from(_: reqwest::Error) -> Self {
        GeneratorError
    }
}
impl From<std::env::VarError> for GeneratorError {
    fn from(_: std::env::VarError) -> Self {
        GeneratorError
    }
}
impl From<ToStrError> for GeneratorError {
    fn from(_: ToStrError) -> Self {
        GeneratorError
    }
}
impl From<std::io::Error> for GeneratorError {
    fn from(_: std::io::Error) -> Self {
        GeneratorError
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
pub struct Cache {
    pub cache: PathBuf,
    items: collections::HashMap<PathBuf, PathBuf>,
    _max_size_bytes: usize,
    _item_timeout: time::Duration,
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
        // can also receive the results of getters
        if let Ok(todo) = rx.try_recv() {
            println!("{:?}", todo.0);
            cache.try_get(todo.0, todo.1);
        }
        cache.check();
    }
}

impl Cache {
    pub fn from_existing_directory(path: PathBuf) -> Result<Self> {
        let mut items = collections::HashMap::new();
        add_all(&path, &Path::new(""), &mut items)?;
        Ok(Cache {
            cache: path,
            items,
            _max_size_bytes: 10_000_000_000,
            _item_timeout: time::Duration::from_secs(86400),
            in_progress: collections::HashMap::new(),
            to_return: collections::HashMap::new(),
        })
    }

    pub fn get(&self, key: &PathBuf) -> Option<&PathBuf> {
        return self.items.get(key);
    }

    pub fn try_get<F>(&mut self, get: Request, generator: F)
    where
        F: FnOnce() -> CacheResult + Send + 'static,
    {
        if let Some(path) = self.items.get(&get.key) {
            // immediately return item if in cache
            get.send_back.send(Ok(path.clone())).unwrap();
        } else {
            if self.in_progress.get(&get.key).is_none() {
                // execute generator if no one already generating this item
                self.in_progress
                    .insert(get.key.clone(), thread::spawn(move || generator()));
            }
            // add caller to list of askers waiting for result
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
                self.items.insert(key.clone(), value);
            }
        }
    }

    fn _is_expired(&self, full_path: PathBuf) -> bool {
        let metadata = match fs::metadata(&full_path) {
            Ok(x) => x,
            _ => return true,
        };
        let created = match metadata.created() {
            Ok(x) => x,
            _ => return true,
        };
        let elapsed = match created.elapsed() {
            Ok(x) => x,
            _ => return false,
        };
        elapsed > self._item_timeout
    }
}

fn add_all(
    cache: &Path,
    subpath: &Path,
    items: &mut collections::HashMap<PathBuf, PathBuf>,
) -> Result<()> {
    for entry in fs::read_dir(cache.join(subpath))? {
        let entry_path = entry?.path();
        if entry_path.is_dir() {
            add_all(cache, &entry_path.strip_prefix(cache)?, items)?;
        } else {
            let key = entry_path.strip_prefix(cache)?;
            items.insert(key.to_path_buf(), entry_path);
        }
    }
    Ok(())
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

    fn insert_a(cache: &mut Cache, filename_override: &str) -> GetBack {
        let (tx, rx) = mpsc::channel();
        let cache_dir = cache.cache.clone();
        let filename = filename_override.to_string();
        let generator = move || {
            thread::sleep(time::Duration::from_millis(100));
            let full_path = cache_dir.join(&filename);
            fs::write(&full_path, "a").unwrap();
            return Ok(full_path);
        };
        cache.try_get(
            Request {
                key: "a".into(),
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
        let cache = Cache::from_existing_directory(dir.path().to_path_buf()).unwrap();
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
        let mut cache = Cache::from_existing_directory(dir.path().to_path_buf()).unwrap();
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
        let mut cache = Cache::from_existing_directory(dir.path().to_path_buf()).unwrap();
        let rx = insert_a(&mut cache, "a");
        assert!(rx.try_recv().is_err());
        let result = check_until(&mut cache, rx).unwrap();
        assert!(result.ends_with("a"));
        assert!(cache.cache.join("a").exists());
    }

    #[test]
    fn test_run_generator_only_once() {
        let dir = tempfile::tempdir().unwrap();
        let mut cache = Cache::from_existing_directory(dir.path().to_path_buf()).unwrap();
        let rx_a = insert_a(&mut cache, "x");
        let rx_b = insert_a(&mut cache, "y");
        let rx_c = insert_a(&mut cache, "z");
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
}
