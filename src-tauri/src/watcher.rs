use std::{
    path::{Path, PathBuf},
    sync::{mpsc::{self, Receiver, Sender}, Arc, Mutex},
    time::Duration,
};

use notify::{
    Event as NotifyEvent, EventKind as NotifyEventKind, RecommendedWatcher, RecursiveMode, Watcher,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum WatchEvent {
    Create,
    Modify,
    Delete,
    Rename,
}

impl From<&NotifyEventKind> for WatchEvent {
    fn from(value: &NotifyEventKind) -> Self {
        match value {
            NotifyEventKind::Create(_) => WatchEvent::Create,
            NotifyEventKind::Modify(_) => WatchEvent::Modify,
            NotifyEventKind::Remove(_) => WatchEvent::Delete,
            NotifyEventKind::Rename(_, _) => WatchEvent::Rename,
            _ => WatchEvent::Modify,
        }
    }
}

#[derive(Debug, Clone)]
pub struct VaultWatchEvent {
    pub path: PathBuf,
    pub event: WatchEvent,
}

#[derive(Debug)]
pub enum WatcherError {
    PathNotAbsolute,
    WatcherClosed,
    PathNotFound,
}

impl std::fmt::Display for WatcherError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            WatcherError::PathNotAbsolute => write!(f, "watcher path is not absolute"),
            WatcherError::WatcherClosed => write!(f, "watcher has been closed"),
            WatcherError::PathNotFound => write!(f, "watch path does not exist"),
        }
    }
}

impl std::error::Error for WatcherError {}

pub struct VaultWatcher {
    _watcher: RecommendedWatcher,
    receiver: Receiver<VaultWatchEvent>,
    _sender: Arc<Mutex<Sender<VaultWatchEvent>>>,
}

impl VaultWatcher {
    /// Start watching a single vault directory recursively.
    pub fn watch(roots: &[PathBuf]) -> Result<Self, WatcherError> {
        if roots.is_empty() {
            return Err(WatcherError::PathNotFound);
        }

        let (sender, receiver) = mpsc::channel();

        for root in roots {
            if !root.is_absolute() {
                return Err(WatcherError::PathNotAbsolute);
            }
            if !root.exists() {
                return Err(WatcherError::PathNotFound);
            }
        }

        let shared_sender = Arc::new(Mutex::new(sender));
        let guard = shared_sender.lock().unwrap();

        let event_sender = guard.clone();
        drop(guard);

        let mut watcher: RecommendedWatcher = RecommendedWatcher::new(
            move |result: Result<NotifyEvent, notify::Error>| match result {
                Ok(NotifyEvent { kind, paths, .. }) => {
                    let event = WatchEvent::from(&kind);
                    for path in paths {
                        if path.as_os_str().is_empty() {
                            continue;
                        }

                        let payload = VaultWatchEvent {
                            path,
                            event,
                        };

                        if event_sender.lock().unwrap().send(payload).is_err() {
                            return;
                        }
                    }
                }
                Err(_) => {}
            },
            notify::Config::default().with_poll_interval(Duration::from_millis(50)),
        )
        .map_err(|_| WatcherError::WatcherClosed)?;

        for root in roots {
            watcher
                .watch(root.as_ref(), RecursiveMode::Recursive)
                .map_err(|_| WatcherError::PathNotFound)?;
        }

        Ok(Self {
            _watcher: watcher,
            receiver,
            _sender: shared_sender,
        })
    }

    pub fn events(&self) -> Vec<VaultWatchEvent> {
        self.receiver.try_iter().collect()
    }

    pub fn recv(&self) -> Result<VaultWatchEvent, mpsc::RecvError> {
        self.receiver.recv()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::{self, File};

    #[test]
    fn empty_roots_rejected() {
        let err = VaultWatcher::watch(&[]).unwrap_err();
        assert!(matches!(err, WatcherError::PathNotFound));
    }

    #[test]
    fn detects_file_creation() {
        let root = std::env::temp_dir().join("nabu-watcher-create-test");
        fs::create_dir_all(&root).unwrap();

        let watcher = VaultWatcher::watch(&[root.clone()]).unwrap();
        let path = root.join("created.md");
        File::create(&path).unwrap();
        std::thread::sleep(std::time::Duration::from_millis(200));

        let captured = watcher.events().iter().any(|event| event.path == path);
        let _ = fs::remove_file(&path);
        let _ = fs::remove_dir_all(&root);

        assert!(captured);
    }

    #[test]
    fn detects_file_modification() {
        let root = std::env::temp_dir().join("nabu-watcher-modify-test");
        fs::create_dir_all(&root).unwrap();
        let path = root.join("modified.md");
        File::create(&path).unwrap();

        let watcher = VaultWatcher::watch(&[root.clone()]).unwrap();
        fs::write(&path, b"updated").unwrap();
        std::thread::sleep(std::time::Duration::from_millis(200));

        let captured = watcher.events().iter().any(|event| event.path == path);
        let _ = fs::remove_file(&path);
        let _ = fs::remove_dir_all(&root);

        assert!(captured);
    }
}
