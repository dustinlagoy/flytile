use anyhow::Result;
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
struct GeneratorError;

type SendBack = mpsc::Sender<StdResult<PathBuf, GeneratorError>>;
type GetBack = mpsc::Receiver<StdResult<PathBuf, GeneratorError>>;

// type Generator = FnOnce() -> StdResult<PathBuf, GeneratorError> + Send + 'static;

// #[derive(Debug)]
// pub struct GeneratingRequest {
//     request: Request,
//     generator: Generator,
// }

#[derive(Debug)]
pub struct Request {
    key: PathBuf,
    send_back: SendBack,
}

#[derive(Debug)]
pub struct Cache {
    pub cache: PathBuf,
    items: collections::HashMap<PathBuf, PathBuf>,
    max_size_bytes: usize,
    item_timeout: time::Duration,
    in_progress: collections::HashMap<
        PathBuf,
        // time::Instant,
        thread::JoinHandle<StdResult<PathBuf, GeneratorError>>,
    >,
    progress_lock: std::sync::Mutex<u8>,
    to_return: collections::HashMap<PathBuf, Vec<SendBack>>,
}

impl Cache {
    pub fn from_existing_directory(path: PathBuf) -> Result<Self> {
        let mut items = collections::HashMap::new();
        add_all(&path, &Path::new(""), &mut items)?;
        Ok(Cache {
            cache: path,
            items,
            max_size_bytes: 10_000_000_000,
            item_timeout: time::Duration::from_secs(86400),
            in_progress: collections::HashMap::new(),
            progress_lock: std::sync::Mutex::new(0),
            to_return: collections::HashMap::new(),
        })
    }

    pub fn get(&self, key: &PathBuf) -> Option<&PathBuf> {
        return self.items.get(key);
    }

    pub fn try_get<F>(&mut self, get: Request, generator: F)
    where
        F: FnOnce() -> StdResult<PathBuf, GeneratorError> + Send + 'static,
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

    // pub fn get_or_set<F>(&mut self, key: &PathBuf, generator: F) -> Result<PathBuf>
    // where
    //     F: FnOnce() -> Result<PathBuf> + Send + 'static,
    // {
    //     let (tx, rx) = mpsc::channel();
    //     let mut generating_item = false;
    //     {
    //         let _guard = self.progress_lock.lock().unwrap();
    //         if let Some(path) = self.items.get(key) {
    //             return Ok(path.clone());
    //         }
    //         // item not in cache
    //         if let Some(_instant) = self.in_progress.get(key) {
    //             // someone currently generating item
    //             // TODO: we must release lock and then join
    //         } else {
    //             // no one currently generating item, generate it now
    //             generating_item = true;
    //             thread::spawn(move || tx.send(generator()));
    //             self.in_progress
    //                 .insert(key.to_path_buf(), time::Instant::now());
    //         }
    //     }
    //     // match self.in_progress.get(key).unwrap().join() {
    //     if generating_item {
    //         // wait for the item to be generated and add it to the cache
    //         match rx.recv().unwrap() {
    //             Ok(item) => {
    //                 let _guard = self.progress_lock.lock().unwrap();
    //                 self.items.insert(key.clone(), item);
    //                 self.in_progress.remove(key);
    //                 if let Some(path) = self.items.get(key) {
    //                     return Ok(path.clone());
    //                 }
    //                 Err(anyhow!("oops"))
    //             }
    //             Err(x) => {
    //                 let _guard = self.progress_lock.lock().unwrap();
    //                 self.in_progress.remove(key);
    //                 Err(anyhow!("{:?}", x))
    //             }
    //         }
    //     } else {
    //         // someone should be generating the item, so wait until it is complete
    //         loop {
    //             if let Some(path) = self.items.get(key) {
    //                 return Ok(path.clone());
    //             }
    //             thread::sleep(time::Duration::from_millis(5));
    //         }
    //     }
    // }

    // pub fn get_or_set<F>(&mut self, key: &PathBuf, generator: F) -> Result<PathBuf>
    // where
    //     F: FnOnce() -> Result<PathBuf>,
    // {
    //     {
    //         let _guard = self.progress_lock.lock().unwrap();
    //         if let Some(path) = self.items.get(key) {
    //             return Ok(path.clone());
    //         }
    //         // item not in cache
    //         if let Some(instant) = self.in_progress.get(key) {
    //             if instant.elapsed() < self.progress_timeout {
    //                 // someone currently generating item
    //                 // wait?
    //             }
    //         }
    //         // no one currently generating item
    //         // or previous generation timed out
    //         self.in_progress
    //             .insert(key.to_path_buf(), time::Instant::now());
    //     }
    //     match generator() {
    //         Ok(item) => {
    //             let _guard = self.progress_lock.lock().unwrap();
    //             self.items.insert(key.clone(), item);
    //             self.in_progress.remove(key);
    //             if let Some(path) = self.items.get(key) {
    //                 return Ok(path.clone());
    //             }
    //             Err(anyhow!("oops"))
    //         }
    //         Err(x) => {
    //             let _guard = self.progress_lock.lock().unwrap();
    //             self.in_progress.remove(key);
    //             Err(x)
    //         }
    //     }
    // }

    fn is_expired(&self, full_path: PathBuf) -> bool {
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
        elapsed > self.item_timeout
    }
}

fn run_cache<F>(cache: &mut Cache, rx: mpsc::Receiver<(Request, F)>)
where
    F: FnOnce() -> StdResult<PathBuf, GeneratorError> + Send + 'static,
{
    loop {
        // can also receive the results of getters
        if let Ok(todo) = rx.try_recv() {
            println!("{:?}", todo.0);
            cache.try_get(todo.0, todo.1);
        }
        cache.check();
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

    fn check_until(cache: &mut Cache, rx: GetBack) -> StdResult<PathBuf, GeneratorError> {
        loop {
            cache.check();
            if let Ok(item) = rx.try_recv() {
                return item;
            }
            thread::sleep(time::Duration::from_millis(5));
        }
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
        // let result = cache
        //     .get_or_set(&"a".into(), move || {
        //         let full_path = cache_dir.join("a");
        //         fs::write(&full_path, "a").unwrap();
        //         return Ok(full_path);
        //     })
        //     .unwrap();
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
        let (tx, rx) = mpsc::channel();
        let cache_dir = cache.cache.clone();
        let generator = move || {
            thread::sleep(time::Duration::from_millis(100));
            let full_path = cache_dir.join("a");
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
        assert!(rx.try_recv().is_err());
        let result = check_until(&mut cache, rx).unwrap();
        assert!(result.ends_with("a"));
        assert!(cache.cache.join("a").exists());
    }

    // #[test]
    // fn test_wait_for_cache_add() {
    //     let dir = tempfile::tempdir().unwrap();
    //     let mut cache = Arc::new(Cache::from_existing_directory(dir.path().to_path_buf()).unwrap());
    //     let a_cache = cache.clone();
    //     thread::spawn(move || {
    //         let cache_dir = a_cache.cache.clone();
    //         a_cache
    //             .get_or_set(&"a".into(), move || {
    //                 thread::sleep(time::Duration::from_secs(1));
    //                 let full_path = cache_dir.join("a");
    //                 fs::write(&full_path, "a").unwrap();
    //                 return Ok(full_path);
    //             })
    //             .unwrap()
    //     });
    //     assert!(cache.get(&"a".into()).is_none());
    //     // assert_eq!(
    //     //     cache.get("a".into()),
    //     //     MaybePath::NotAvailable(dir.path().to_path_buf())
    //     // );
    //     // assert_eq!(cache.get("a".into()), MaybePath::InProgress);
    // }
}
