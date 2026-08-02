use crate::jobs::cancellation::CancellationToken;
use crate::jobs::priority::Priority;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

/// The canonical Job model for all asynchronous work in Nabu.
/// Every background task is represented as a Job.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Job {
    /// Unique job identifier
    pub id: Uuid,

    /// Discriminator for the type of work to perform
    pub job_type: JobType,

    /// Opaque payload — job-specific data
    pub payload: serde_json::Value,

    /// Execution priority
    pub priority: Priority,

    /// Current status
    pub status: JobStatus,

    /// When the job was created
    pub created_at: DateTime<Utc>,

    /// When the job is scheduled to execute (None = immediate)
    pub scheduled_at: Option<DateTime<Utc>>,

    /// When execution started
    pub started_at: Option<DateTime<Utc>>,

    /// When execution completed
    pub finished_at: Option<DateTime<Utc>>,

    /// Number of times this job has been retried
    pub retry_count: u32,

    /// Maximum number of retries before permanent failure
    pub maximum_retries: u32,

    /// Progress indicator (0.0–1.0)
    pub progress: f64,

    /// Human-readable progress message
    pub progress_message: Option<String>,

    /// Tags for filtering / grouping
    pub tags: Vec<String>,

    /// Custom metadata
    pub metadata: HashMap<String, String>,

    /// Last error message (if failed)
    pub last_error: Option<String>,

    /// Which processor this job targets
    pub processor_name: String,

    /// The KnowledgeObject ID this job is processing
    pub object_id: Option<Uuid>,
}

impl Job {
    pub fn new(
        job_type: JobType,
        payload: serde_json::Value,
        processor_name: impl Into<String>,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            job_type,
            payload,
            priority: Priority::Normal,
            status: JobStatus::Queued,
            created_at: Utc::now(),
            scheduled_at: None,
            started_at: None,
            finished_at: None,
            retry_count: 0,
            maximum_retries: 3,
            progress: 0.0,
            progress_message: None,
            tags: Vec::new(),
            metadata: HashMap::new(),
            last_error: None,
            processor_name: processor_name.into(),
            object_id: None,
        }
    }

    pub fn with_priority(mut self, priority: Priority) -> Self {
        self.priority = priority;
        self
    }

    pub fn with_schedule(mut self, scheduled_at: DateTime<Utc>) -> Self {
        self.scheduled_at = Some(scheduled_at);
        self
    }

    pub fn with_max_retries(mut self, max: u32) -> Self {
        self.maximum_retries = max;
        self
    }

    pub fn with_object_id(mut self, object_id: Uuid) -> Self {
        self.object_id = Some(object_id);
        self
    }

    pub fn with_tag(mut self, tag: impl Into<String>) -> Self {
        self.tags.push(tag.into());
        self
    }

    pub fn with_metadata(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.metadata.insert(key.into(), value.into());
        self
    }

    /// Whether this job is ready to execute (scheduled time has passed).
    pub fn is_ready(&self) -> bool {
        match self.scheduled_at {
            Some(scheduled) => Utc::now() >= scheduled,
            None => true,
        }
    }

    /// Whether this job should be retried.
    ///
    /// `maximum_retries` is the number of retry *attempts* allowed, so a job
    /// with `max_retries = 1` may fail once and be retried once (retry_count
    /// becomes 1), then fail permanently on the next failure (retry_count 2).
    pub fn should_retry(&self) -> bool {
        self.retry_count <= self.maximum_retries
    }

    /// Create a cancellation token for this job (for cooperative cancellation).
    pub fn cancellation_token(&self) -> CancellationToken {
        // In a real implementation, this would look up a stored token.
        // For now, each call creates a fresh token.
        CancellationToken::new()
    }
}

/// Types of jobs that can be enqueued.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum JobType {
    /// OCR processing
    Ocr,
    /// Whisper transcription
    Whisper,
    /// PDF text extraction
    PdfTextExtraction,
    /// PDF metadata extraction
    PdfMetadataExtraction,
    /// PDF annotation processing
    PdfAnnotationProcessing,
    /// Metadata extraction from content
    MetadataExtraction,
    /// Metadata enrichment
    MetadataEnrichment,
    /// Content classification
    ContentClassification,
    /// Duplicate detection
    DuplicateDetection,
    /// Timeline extraction
    TimelineExtraction,
    /// Auto-filing / routing
    AutoFiling,
    /// Embedding generation
    EmbeddingGeneration,
    /// Semantic enrichment
    SemanticEnrichment,
    /// AI summarisation
    AiSummarisation,
    /// Search indexing
    SearchIndexing,
    /// Graph update
    GraphUpdate,
    /// Storage persistence
    StoragePersistence,
    /// Export
    Export,
    /// Plugin execution
    Plugin,
    /// Custom job type
    Custom(String),
}

impl JobType {
    pub fn name(&self) -> &'static str {
        match self {
            JobType::Ocr => "ocr",
            JobType::Whisper => "whisper",
            JobType::PdfTextExtraction => "pdf_text_extraction",
            JobType::PdfMetadataExtraction => "pdf_metadata_extraction",
            JobType::PdfAnnotationProcessing => "pdf_annotation_processing",
            JobType::MetadataExtraction => "metadata_extraction",
            JobType::MetadataEnrichment => "metadata_enrichment",
            JobType::ContentClassification => "content_classification",
            JobType::DuplicateDetection => "duplicate_detection",
            JobType::TimelineExtraction => "timeline_extraction",
            JobType::AutoFiling => "auto_filing",
            JobType::EmbeddingGeneration => "embedding_generation",
            JobType::SemanticEnrichment => "semantic_enrichment",
            JobType::AiSummarisation => "ai_summarisation",
            JobType::SearchIndexing => "search_indexing",
            JobType::GraphUpdate => "graph_update",
            JobType::StoragePersistence => "storage_persistence",
            JobType::Export => "export",
            JobType::Plugin => "plugin",
            JobType::Custom(_) => "custom",
        }
    }

    pub fn default_priority(&self) -> Priority {
        match self {
            JobType::StoragePersistence | JobType::GraphUpdate => Priority::Critical,
            JobType::Ocr | JobType::MetadataExtraction => Priority::High,
            JobType::PdfTextExtraction
            | JobType::PdfMetadataExtraction
            | JobType::PdfAnnotationProcessing
            | JobType::MetadataEnrichment
            | JobType::ContentClassification
            | JobType::DuplicateDetection
            | JobType::TimelineExtraction
            | JobType::AutoFiling
            | JobType::SearchIndexing => Priority::Normal,
            JobType::Whisper
            | JobType::EmbeddingGeneration
            | JobType::SemanticEnrichment
            | JobType::AiSummarisation
            | JobType::Export => Priority::Low,
            JobType::Plugin | JobType::Custom(_) => Priority::Background,
        }
    }
}

/// Job status lifecycle.
/// Queued → Running → Completed
///         → Running → Failed → Queued (retry)
///         → Running → Failed → Failed (permanent)
///         → Running → Cancelled
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum JobStatus {
    /// Job is waiting to be executed
    Queued,
    /// Job is currently executing
    Running,
    /// Job completed successfully
    Completed,
    /// Job failed and may be retried
    Failed,
    /// Job was cancelled
    Cancelled,
    /// Job is scheduled for future execution
    Scheduled,
}

impl JobStatus {
    pub fn label(&self) -> &'static str {
        match self {
            JobStatus::Queued => "queued",
            JobStatus::Running => "running",
            JobStatus::Completed => "completed",
            JobStatus::Failed => "failed",
            JobStatus::Cancelled => "cancelled",
            JobStatus::Scheduled => "scheduled",
        }
    }

    pub fn is_terminal(&self) -> bool {
        matches!(
            self,
            JobStatus::Completed | JobStatus::Failed | JobStatus::Cancelled
        )
    }

    pub fn is_active(&self) -> bool {
        matches!(
            self,
            JobStatus::Queued | JobStatus::Running | JobStatus::Scheduled
        )
    }
}
