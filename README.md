# todo

- [x] logging
- [x] sentinel token generation
- [x] file cache manager
    - [x] can issue locks for updating
    - [ ] can expire item generation on timeout
    - [ ] can expire items on schedule
    - [ ] can limit total size based on LRU or simple date
- [ ] selectable cloud coverage
- [ ] use cache for slope output tiles
- [ ] docs
- [ ] contours
- [ ] per-user urls

# cache

dequeue vec for order and expiration

for cache send generator to function, then store future for executing that generator, then all threads can block on that future, but still need timeoutgn

send an rx channel with cache request
cache has main loop
check if in cache/pending
return or start thread for getting/waiting

```
fn get_loop(cache, rx) {
    loop {
        request = rx.recv()
        cache.get(request)
    }
}

fn add_loop(cache) {
    loop {
        for key, handle in self.in_progress() {
            if handle.finished() {
                cache.insert(key, handle.join())
            }
        }
        sleep(1)
    }
}

impl Cache {
    fn get(request) {
        lock = self.lock.lock()
        tx, key, getter = request
        if key in self.items {
            tx.send(self.items.get(key))
        } else {
            if not self.in_progress(key) {
                thread::spawn(getter)
            }
            thread::spawn(return_when_ready)
        }
    }
}
```

| cache               | thread 1 | thread 2 |
| ------------------- | -------- | -------- |
|                     | get a    |          |
| no a, return a lock |          |          |
|                     |          | get a    |
| a locked, wait      |          |          |
|                     |          |          |
|                     |          |          |
|                     |          |          |
