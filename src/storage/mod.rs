//! Persistence and database subsystem
//!
//! SQLite storage for browser history, bookmarks, user shortcuts, downloads,
//! search engines, and workspaces with strict XDG permissions and async background worker.

pub mod bookmarks;
pub mod database;
pub mod downloads;
pub mod history;
pub mod import_export;
pub mod search_engines;
pub mod shortcuts;
pub mod workspaces;

use database::Database;
use gtk4::glib;
use std::any::Any;
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::sync::mpsc::{Sender, channel};

pub type TaskBox = Box<dyn FnOnce(&mut Database) + Send + 'static>;

thread_local! {
    static PENDING_CALLBACKS: RefCell<HashMap<u64, Box<dyn FnOnce(Box<dyn Any>)>>> =
        RefCell::new(HashMap::new());
    static NEXT_CALLBACK_ID: Cell<u64> = const { Cell::new(1) };
}

/// Asynchronous handle to the browser's persistent storage worker thread.
/// All disk I/O and SQLite operations run on a dedicated background thread,
/// ensuring zero stalls on the GTK main thread.
#[derive(Clone)]
pub struct StorageService {
    tx: Sender<TaskBox>,
}

impl StorageService {
    /// Starts the storage service with an in-memory database (primarily for tests)
    pub fn new_in_memory() -> rusqlite::Result<Self> {
        let db = Database::open_in_memory()?;
        Ok(Self::start(db))
    }

    /// Starts the storage service with the default XDG database path
    pub fn open_default() -> rusqlite::Result<Self> {
        let db = Database::open_default()?;
        Ok(Self::start(db))
    }

    /// Spawns the background worker thread with the given database instance
    pub fn start(mut db: Database) -> Self {
        let (tx, rx) = channel::<TaskBox>();

        std::thread::Builder::new()
            .name("shadhin-storage-worker".into())
            .spawn(move || {
                // Perform any startup maintenance (e.g., mark interrupted downloads)
                let _ = downloads::DownloadRepository::new(db.conn()).mark_interrupted_on_startup();

                while let Ok(task) = rx.recv() {
                    task(&mut db);
                }
            })
            .expect("Failed to spawn persistent storage background thread");

        Self { tx }
    }

    /// Dispatches a fire-and-forget task to the background database worker
    pub fn dispatch<F>(&self, task: F)
    where
        F: FnOnce(&mut Database) + Send + 'static,
    {
        let _ = self.tx.send(Box::new(task));
    }

    /// Executes a query on the background thread and delivers the result to the
    /// GTK main thread using GLib MainContext invocation and thread-local callback dispatch
    pub fn query_async<T, Q, C>(&self, query: Q, on_complete: C)
    where
        T: Send + 'static,
        Q: FnOnce(&mut Database) -> T + Send + 'static,
        C: FnOnce(T) + 'static,
    {
        let callback_id = NEXT_CALLBACK_ID.with(|id_cell| {
            let id = id_cell.get();
            id_cell.set(id.wrapping_add(1));
            id
        });

        PENDING_CALLBACKS.with(|callbacks| {
            let boxed_cb = Box::new(move |any_val: Box<dyn Any>| {
                if let Ok(concrete_val) = any_val.downcast::<T>() {
                    on_complete(*concrete_val);
                }
            });
            callbacks.borrow_mut().insert(callback_id, boxed_cb);
        });

        self.dispatch(move |db| {
            let res = query(db);
            glib::MainContext::default().invoke(move || {
                let cb_opt =
                    PENDING_CALLBACKS.with(|callbacks| callbacks.borrow_mut().remove(&callback_id));
                if let Some(cb) = cb_opt {
                    cb(Box::new(res));
                }
            });
        });
    }

    /// Asynchronously runs SQLite PRAGMA optimize on shutdown or maintenance
    pub fn optimize(&self) {
        self.dispatch(|db| {
            let _ = db.optimize();
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_storage_service_worker_dispatch() {
        let storage = StorageService::new_in_memory().unwrap();
        let (done_tx, done_rx) = std::sync::mpsc::channel();

        storage.dispatch(move |db| {
            let count: i64 = db
                .conn()
                .query_row("SELECT COUNT(*) FROM search_engines;", [], |r| r.get(0))
                .unwrap();
            assert_eq!(count, 6);
            done_tx.send(true).unwrap();
        });

        assert!(done_rx.recv().unwrap());
    }
}
