use notify::{Config, RecommendedWatcher, RecursiveMode, Watcher as NotifyWatcher};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::mpsc;

pub enum WatchEvent {
    Changed(PathBuf),
    Removed(PathBuf),
    Created(PathBuf),
}

pub struct Watcher {
    _watcher: RecommendedWatcher,
}

impl Watcher {
    pub fn new(path: PathBuf, tx: mpsc::Sender<WatchEvent>) -> anyhow::Result<Self> {
        let watcher_tx = tx.clone();
        
        let mut watcher = RecommendedWatcher::new(
            move |res: notify::Result<notify::Event>| {
                if let Ok(event) = res {
                    for path in event.paths {
                        let watch_event = match event.kind {
                            notify::EventKind::Create(_) => Some(WatchEvent::Created(path)),
                            notify::EventKind::Modify(_) => Some(WatchEvent::Changed(path)),
                            notify::EventKind::Remove(_) => Some(WatchEvent::Removed(path)),
                            _ => None,
                        };
                        
                        if let Some(e) = watch_event {
                            let _ = watcher_tx.try_send(e);
                        }
                    }
                }
            },
            Config::default(),
        )?;

        watcher.watch(&path, RecursiveMode::Recursive)?;

        Ok(Self { _watcher: watcher })
    }
}
