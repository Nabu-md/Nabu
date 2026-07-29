use std::sync::Arc;
use tokio::sync::mpsc::{self, Receiver, Sender};
use tokio::sync::RwLock;

use super::cancellation::CancellationToken;
use super::errors::{JobError, JobResult};
use super::job::{Job, JobId};

/// Messages sent from the queue to workers.
#[derive(Debug, Clone)]
pub enum WorkerMessage {
    /// A new job is available for processing.
    NewJob(Job),

    /// A previously running job has been cancelled.
    /// Workers should check their cancellation token, but this provides
    /// an explicit notification for prompt cancellation.
    JobCancelled(JobId),

    /// The system is shutting down. Workers should finish their current
    /// job and stop accepting new work.
    Shutdown,
}

/// Messages sent from workers back to the queue.
#[derive(Debug, Clone)]
pub enum QueueMessage {
    /// The worker has completed a job successfully.
    JobCompleted(JobId),

    /// The worker has failed a job with an error.
    JobFailed(JobId, String),

    /// The worker has acknowledged cancellation and stopped processing a job.
    JobCancelled(JobId),

    /// The worker has started processing a job.
    JobStarted(JobId),

    /// A progress update from the worker.
    JobProgress(JobId, f64, String),
}

/// The communication channel between the job queue and a pool of workers.
///
/// This provides a typed, bounded channel for:
/// - Dispatching jobs to workers
/// - Notifying workers of cancellation
/// - Receiving status updates from workers
/// - Broadcasting shutdown signals
///
/// The channel is designed to be shared between the queue and all workers
/// via `Arc<WorkerChannel>`.
#[derive(Debug, Clone)]
pub struct WorkerChannel {
    /// Sender for dispatching jobs and commands to workers.
    to_worker: Sender<WorkerMessage>,

    /// Receiver for receiving status updates from workers.
    from_worker: Arc<RwLock<Option<Receiver<QueueMessage>>>>,

    /// Capacity of the channel.
    capacity: usize,

    /// Whether the channel has been shut down.
    shutdown: CancellationToken,
}

impl WorkerChannel {
    /// Creates a new worker communication channel with the given capacity.
    ///
    /// The capacity controls how many pending jobs can be queued for workers.
    /// If workers are busy, additional jobs will remain in the main queue.
    pub fn new(capacity: usize) -> (Self, WorkerHandle) {
        let (tx_worker, rx_worker) = mpsc::channel::<WorkerMessage>(capacity);
        let (tx_queue, rx_queue) = mpsc::channel::<QueueMessage>(capacity);

        let channel = WorkerChannel {
            to_worker: tx_worker,
            from_worker: Arc::new(RwLock::new(Some(rx_queue))),
            capacity,
            shutdown: CancellationToken::new(),
        };

        let handle = WorkerHandle {
            from_queue: rx_worker,
            to_queue: tx_queue,
        };

        (channel, handle)
    }

    /// Dispatches a job to a worker. Returns an error if all workers are busy.
    pub async fn dispatch(&self, job: Job) -> Result<(), JobError> {
        if self.shutdown.is_cancelled() {
            return Err(JobError::QueueShuttingDown);
        }
        self.to_worker
            .send(WorkerMessage::NewJob(job))
            .await
            .map_err(|_| JobError::Internal("worker channel closed".into()))
    }

    /// Notifies workers that a job has been cancelled.
    pub async fn notify_cancelled(&self, job_id: JobId) -> Result<(), JobError> {
        self.to_worker
            .send(WorkerMessage::JobCancelled(job_id))
            .await
            .map_err(|_| JobError::Internal("worker channel closed".into()))
    }

    /// Broadcasts a shutdown signal to all workers.
    pub async fn shutdown(&self) {
        self.shutdown.cancel();
        // Try to send shutdown; ignore if channel is full (workers will drain)
        let _ = self.to_worker.send(WorkerMessage::Shutdown).await;
    }

    /// Tries to receive a message from a worker without blocking.
    pub async fn try_recv(&self) -> Option<QueueMessage> {
        let mut rx = self.from_worker.write().await;
        if let Some(ref mut rx) = *rx {
            rx.try_recv().ok()
        } else {
            None
        }
    }

    /// Receives a message from a worker, blocking until one is available.
    pub async fn recv(&self) -> Option<QueueMessage> {
        let mut rx = self.from_worker.write().await;
        if let Some(ref mut rx) = *rx {
            rx.recv().await
        } else {
            None
        }
    }

    /// Returns the channel capacity.
    pub fn capacity(&self) -> usize {
        self.capacity
    }

    /// Returns `true` if the channel has been shut down.
    pub fn is_shutdown(&self) -> bool {
        self.shutdown.is_cancelled()
    }
}

/// The worker-side handle for communicating with the queue.
///
/// Each worker receives one of these to receive jobs and send status updates.
#[derive(Debug)]
pub struct WorkerHandle {
    /// Receiver for incoming jobs and commands from the queue.
    from_queue: Receiver<WorkerMessage>,

    /// Sender for sending status updates back to the queue.
    to_queue: Sender<QueueMessage>,
}

impl WorkerHandle {
    /// Tries to receive the next message from the queue without blocking.
    pub async fn try_recv(&mut self) -> Option<WorkerMessage> {
        self.from_queue.try_recv().ok()
    }

    /// Receives the next message from the queue, blocking until one is available.
    pub async fn recv(&mut self) -> Option<WorkerMessage> {
        self.from_queue.recv().await
    }

    /// Sends a status update back to the queue.
    pub async fn send(&self, msg: QueueMessage) -> Result<(), JobError> {
        self.to_queue
            .send(msg)
            .await
            .map_err(|_| JobError::Internal("queue channel closed".into()))
    }

    /// Reports that a job has been started.
    pub async fn report_started(&self, job_id: JobId) -> Result<(), JobError> {
        self.send(QueueMessage::JobStarted(job_id)).await
    }

    /// Reports that a job has completed successfully.
    pub async fn report_completed(&self, job_id: JobId) -> Result<(), JobError> {
        self.send(QueueMessage::JobCompleted(job_id)).await
    }

    /// Reports that a job has failed.
    pub async fn report_failed(&self, job_id: JobId, error: String) -> Result<(), JobError> {
        self.send(QueueMessage::JobFailed(job_id, error)).await
    }

    /// Reports progress on a running job (0.0 — 1.0).
    pub async fn report_progress(&self, job_id: JobId, progress: f64, message: String) -> Result<(), JobError> {
        self.send(QueueMessage::JobProgress(job_id, progress, message))
            .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::jobs::JobPayload;

    #[tokio::test]
    async fn test_dispatch_and_receive() {
        let (channel, mut handle) = WorkerChannel::new(16);
        let job = Job::new("test", JobPayload::new());

        channel.dispatch(job.clone()).await.unwrap();
        let msg = handle.recv().await.unwrap();

        match msg {
            WorkerMessage::NewJob(received) => {
                assert_eq!(received.id, job.id);
                assert_eq!(received.job_type, job.job_type);
            }
            _ => panic!("expected NewJob message"),
        }
    }

    #[tokio::test]
    async fn test_worker_sends_status() {
        let (channel, mut handle) = WorkerChannel::new(16);
        let job = Job::new("test", JobPayload::new());

        channel.dispatch(job.clone()).await.unwrap();

        // Worker receives the job and sends a status
        let msg = handle.recv().await.unwrap();
        match msg {
            WorkerMessage::NewJob(j) => {
                handle.report_started(j.id).await.unwrap();
            }
            _ => panic!("expected NewJob"),
        }

        // Queue receives the status
        let status = channel.recv().await.unwrap();
        match status {
            QueueMessage::JobStarted(id) => {
                assert_eq!(id, job.id);
            }
            _ => panic!("expected JobStarted"),
        }
    }

    #[tokio::test]
    async fn test_cancellation_notification() {
        let (channel, mut handle) = WorkerChannel::new(16);
        let job = Job::new("test", JobPayload::new());
        channel.dispatch(job.clone()).await.unwrap();

        // Receive the job in the worker
        let _msg = handle.recv().await.unwrap();

        // Notify cancellation
        channel.notify_cancelled(job.id).await.unwrap();

        // Worker receives the cancellation
        let msg = handle.recv().await.unwrap();
        match msg {
            WorkerMessage::JobCancelled(id) => {
                assert_eq!(id, job.id);
            }
            _ => panic!("expected JobCancelled"),
        }
    }

    #[tokio::test]
    async fn test_shutdown_signal() {
        let (channel, mut handle) = WorkerChannel::new(16);

        channel.shutdown().await;

        let msg = handle.recv().await.unwrap();
        match msg {
            WorkerMessage::Shutdown => {} // expected
            _ => panic!("expected Shutdown"),
        }
    }

    #[tokio::test]
    async fn test_dispatch_after_shutdown_fails() {
        let (channel, _handle) = WorkerChannel::new(16);
        channel.shutdown().await;

        let result = channel.dispatch(Job::new("test", JobPayload::new())).await;
        assert!(result.is_err());
        match result {
            Err(JobError::QueueShuttingDown) => {}
            _ => panic!("expected QueueShuttingDown"),
        }
    }

    #[tokio::test]
    async fn test_worker_progress_report() {
        let (channel, mut handle) = WorkerChannel::new(16);
        let job = Job::new("test", JobPayload::new());

        channel.dispatch(job.clone()).await.unwrap();
        let msg = handle.recv().await.unwrap();

        match msg {
            WorkerMessage::NewJob(j) => {
                handle.report_progress(j.id, 0.5, "halfway there".into()).await.unwrap();
            }
            _ => panic!("expected NewJob"),
        }

        let status = channel.recv().await.unwrap();
        match status {
            QueueMessage::JobProgress(id, progress, msg) => {
                assert_eq!(id, job.id);
                assert!((progress - 0.5).abs() < f64::EPSILON);
                assert_eq!(msg, "halfway there");
            }
            _ => panic!("expected JobProgress"),
        }
    }
}
