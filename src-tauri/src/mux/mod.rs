//! MuxWorker: ein Thread mit niedriger Priorität für alles, was Platte und CPU kostet,
//! aber die Aufnahme nie blockieren darf (Clip schreiben, Thumbnails, Trimmen).

pub mod media;
pub mod mp4;
pub mod recorder;

use crossbeam_channel::{bounded, Sender};

use crate::error::CoreResult;
use crate::util;

type Job = Box<dyn FnOnce() + Send>;

#[derive(Clone)]
pub struct MuxWorker {
    tx: Sender<Job>,
}

impl MuxWorker {
    pub fn start() -> CoreResult<Self> {
        let (tx, rx) = bounded::<Job>(64);
        util::spawn("mux-worker", util::Priority::BelowNormal, move || {
            for job in rx.iter() {
                job();
            }
        })?;
        Ok(MuxWorker { tx })
    }

    /// Reiht eine Aufgabe ein. Ist die Warteschlange voll, wird sie verworfen (und geloggt).
    pub fn submit(&self, job: impl FnOnce() + Send + 'static) -> bool {
        match self.tx.try_send(Box::new(job)) {
            Ok(()) => true,
            Err(_) => {
                tracing::warn!("MuxWorker-Warteschlange voll");
                false
            }
        }
    }
}
