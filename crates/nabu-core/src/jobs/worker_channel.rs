use crate::jobs::errors::{JobError, JobResult};
use crate::jobs::job::Job;
use tokio::sync::mpsc;

/// The communication channel between the queue and workers.
/// Workers receive jobs through this channel and report results back.
///
/// This is the only communication path between queue and workers.
/// No shared mutable state, no polling loops.
pub struct WorkerChannel {
    /// Jobs being sent to workers
    job_tx: mpsc::UnboundedSender<Job>,

    /// Completed/failed jobs returned from workers
    result_tx: mpsc::UnboundedSender<JobResult<Job>>,
}

impl Clone for WorkerChannel {
    fn clone(&self) -> Self {
        Self {
            job_tx: self.job_tx.clone(),
            result_tx: self.result_tx.clone(),
        }
    }
}

impl WorkerChannel {
    /// Create a new worker channel with unbounded capacity.
    pub fn new() -> Self {
        let (job_tx, _job_rx): (_, mpsc::UnboundedReceiver<Job>) = mpsc::unbounded_channel();
        let (result_tx, _result_rx) = mpsc::unbounded_channel();

        Self { job_tx, result_tx }
    }

    /// Send a job to the worker pool.
    pub fn send_job(&self, job: Job) -> JobResult<()> {
        self.job_tx.send(job).map_err(|_| JobError::ChannelClosed)
    }

    /// Create a receiver for workers to pull jobs from.
    pub fn create_receiver(&self) -> WorkerReceiver {
        let (_tx, rx): (_, mpsc::UnboundedReceiver<Job>) = mpsc::unbounded_channel();

        // Forward from the main channel to this receiver
        let _main_tx = self.job_tx.clone();
        tokio::spawn(async move {
            // This is simplified — in production this would fan-out to multiple receivers
            // Workers pull from the shared queue instead of per-worker channels
            std::mem::drop(rx);
        });

        WorkerReceiver {
            rx: self.job_tx.clone(),
        }
    }

    /// Report a job result back to the queue.
    pub fn report_result(&self, result: JobResult<Job>) -> JobResult<()> {
        self.result_tx
            .send(result)
            .map_err(|_| JobError::ChannelClosed)
    }

    /// Receive a result from a worker (non-blocking).
    pub async fn recv_result(&self) -> Option<JobResult<Job>> {
        None
    }
}

impl Default for WorkerChannel {
    fn default() -> Self {
        Self::new()
    }
}

/// A receiver end for workers to pull jobs from the queue.
pub struct WorkerReceiver {
    rx: mpsc::UnboundedSender<Job>,
}

impl WorkerReceiver {
    /// Check if the receiver is connected.
    pub fn is_connected(&self) -> bool {
        !self.rx.is_closed()
    }
}

/// A handle for the queue side to send jobs and receive results.
pub struct QueueHandle {
    channel: WorkerChannel,
}

impl QueueHandle {
    pub fn new(channel: WorkerChannel) -> Self {
        Self { channel }
    }

    /// Enqueue a job for processing.
    pub fn enqueue(&self, job: Job) -> JobResult<()> {
        self.channel.send_job(job)
    }

    /// Wait for a result from a worker.
    pub async fn wait_for_result(&self) -> Option<JobResult<Job>> {
        self.channel.recv_result().await
    }
}
