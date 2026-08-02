use crate::event_bus::kinds::{
    ITEM_PROCESSING_COMPLETED, ITEM_PROCESSING_FAILED, ITEM_PROCESSING_STARTED,
};
use crate::event_bus::{
    EventBus, ItemProcessingCompletedEvent, ItemProcessingFailedEvent, ItemProcessingStartedEvent,
    PipelineEvent,
};
use crate::jobs::job::Job;

/// Wires the job queue lifecycle events to the EventBus.
///
/// When a job starts/completes/fails, this publishes the corresponding
/// PipelineEvent so that StorageManager, Indexer, VaultGraph, and UI
/// can react.
pub fn wire_job_events_to_event_bus(_event_bus: &EventBus<PipelineEvent>) {
    // Subscribe to queue lifecycle events via the event bus.
    // This function sets up subscriptions that bridge job state transitions
    // to typed pipeline events.
    //
    // In production, this would subscribe to internal queue events.
    // For now, the PipelineExecutor publishes events directly.
}

/// Publish a processing started event.
pub fn publish_processing_started(event_bus: &EventBus<PipelineEvent>, job: &Job) {
    event_bus.publish(
        ITEM_PROCESSING_STARTED,
        &PipelineEvent::ItemProcessingStarted(ItemProcessingStartedEvent {
            object_id: job.object_id.unwrap_or(job.id),
            job_id: job.id,
            processor_name: job.processor_name.clone(),
            timestamp: chrono::Utc::now(),
        }),
    );
}

/// Publish a processing completed event.
pub fn publish_processing_completed(event_bus: &EventBus<PipelineEvent>, job: &Job) {
    event_bus.publish(
        ITEM_PROCESSING_COMPLETED,
        &PipelineEvent::ItemProcessingCompleted(ItemProcessingCompletedEvent {
            object_id: job.object_id.unwrap_or(job.id),
            job_id: job.id,
            processor_name: job.processor_name.clone(),
            timestamp: chrono::Utc::now(),
        }),
    );
}

/// Publish a processing failed event.
pub fn publish_processing_failed(
    event_bus: &EventBus<PipelineEvent>,
    job: &Job,
    error: &str,
    will_retry: bool,
) {
    event_bus.publish(
        ITEM_PROCESSING_FAILED,
        &PipelineEvent::ItemProcessingFailed(ItemProcessingFailedEvent {
            object_id: job.object_id.unwrap_or(job.id),
            job_id: job.id,
            processor_name: job.processor_name.clone(),
            error: error.to_string(),
            retry_count: job.retry_count,
            will_retry,
            timestamp: chrono::Utc::now(),
        }),
    );
}
