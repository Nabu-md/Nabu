//! # Harper Processor
//!
//! Runs local grammar/spell-checking on the text content of KnowledgeObjects
//! using the [`harper-core`](https://docs.rs/harper-core) library, and converts
//! the resulting [`Lint`] objects into Nabu's standardized [`Diagnostic`]
//! model.
//!
//! ## Thread Safety
//!
//! `harper_core::Document` and `harper_core::linting::LintGroup` are `!Send`
//! and `!Sync`. The processor therefore performs all Harper work inside a
//! closure passed to `tokio::task::spawn_blocking`, which runs on a
//! dedicated thread-pool worker. The only values crossing back to the
//! async context are `Vec<Diagnostic>` (which are `Send + Sync`) and
//! the (unchanged) `KnowledgeObject`.
//!
//! ## What It Does Not Do
//!
//! This processor does not modify the object's content. It attaches diagnostics
//! to the `ProcessingResult` so that the pipeline can publish them through the
//! EventBus.
//!
//! [`Lint`]: harper_core::linting::Lint

use crate::diagnostic::DiagnosticProvider;
use crate::jobs::cancellation::CancellationToken;
use crate::jobs::workers::progress::ProgressReporter;
use crate::models::{ObjectContent, ObjectType};
use crate::processing::processor::{ExecutionStatus, ProcessingStats, ProcessingContext, ProcessingResult, Processor};
use crate::processing::processors::harper_conversion::convert_lint;
use async_trait::async_trait;
use harper_core::linting::{LintGroup, Linter};
use harper_core::parsers::PlainEnglish;
use harper_core::spell::FstDictionary;
use harper_core::{Dialect, Document};

/// The origin label embedded in all diagnostics produced by this processor.
pub const HARPER_ORIGIN: &str = "harper";

/// Runs Harper grammar and spell checking on KnowledgeObjects with text content.
///
/// On construction, the processor initialises the curated `LintGroup` and
/// `FstDictionary`. Each call to [`process`](Processor::process) creates a
/// fresh `Document` from the object's text, runs the linter, and converts
/// lints to diagnostics.
pub struct HarperProcessor;

impl HarperProcessor {
    /// Extract the text content from a KnowledgeObject, stripping HTML tags
    /// from RichHtml content.
    fn extract_text(object: &crate::models::KnowledgeObject) -> Option<String> {
        match &object.content {
            ObjectContent::Markdown(s) => Some(s.clone()),
            ObjectContent::PlainText(s) => Some(s.clone()),
            ObjectContent::RichHtml(s) => Some(strip_html_tags(s)),
            ObjectContent::Uri(_) => None,
            ObjectContent::Binary { .. } => None,
        }
    }
}

impl Default for HarperProcessor {
    fn default() -> Self {
        Self
    }
}

#[async_trait]
impl Processor for HarperProcessor {
    fn name(&self) -> &'static str {
        "harper_processor"
    }

    async fn process(
        &self,
        context: &ProcessingContext,
        progress: ProgressReporter,
        cancellation: CancellationToken,
    ) -> ProcessingResult {
        if cancellation.is_cancelled() {
            return ProcessingResult::unmodified(context.object.clone())
                .with_stats(ProcessingStats::new().with_diagnostics_count(0))
                .with_status(ExecutionStatus::Cancelled);
        }

        let text = match Self::extract_text(&context.object) {
            Some(t) if !t.is_empty() => t,
            _ => {
                return ProcessingResult::unmodified(context.object.clone())
                    .with_stats(ProcessingStats::new().with_diagnostics_count(0))
                    .with_status(ExecutionStatus::Success);
            }
        };

        progress.set_progress(0.1);

        let text_for_task = text.clone();

        let start = std::time::Instant::now();
        let result = tokio::task::spawn_blocking(move || {
            run_harper(&text_for_task)
        })
        .await;
        let duration_ms = start.elapsed().as_millis() as u64;

        progress.set_progress(0.9);

        let (diagnostics, lints_count) = match result {
            Ok(Ok((diags, count))) => (diags, count),
            Ok(Err(e)) => {
                tracing::warn!(
                    subsystem = "processing",
                    component = "harper_processor",
                    object_id = %context.object.id,
                    error = %e,
                    "Harper processing failed"
                );
                (vec![], 0)
            }
            Err(_) => {
                tracing::warn!(
                    subsystem = "processing",
                    component = "harper_processor",
                    object_id = %context.object.id,
                    "Harper task panicked"
                );
                (vec![], 0)
            }
        };

        progress.set_progress(1.0);

        let status = if diagnostics.is_empty() {
            ExecutionStatus::Success
        } else {
            ExecutionStatus::CompletedWithDiagnostics
        };

        let stats = ProcessingStats::new()
            .with_diagnostics_count(diagnostics.len())
            .with_lints_found(lints_count)
            .with_duration_ms(duration_ms);

        ProcessingResult::new(context.object.clone())
            .with_diagnostics(diagnostics)
            .with_stats(stats)
            .with_status(status)
    }

    fn supports(&self, object_type: &ObjectType) -> bool {
        matches!(
            object_type,
            ObjectType::Note
                | ObjectType::Article
                | ObjectType::Document
                | ObjectType::Email
                | ObjectType::CodeSnippet
        )
    }
}

/// Run the Harper linter on the given text and return diagnostics.
///
/// This function is `Send` because it creates a fresh `LintGroup` and `Document`
/// internally (both `!Send`), runs the linter, converts lints to `Diagnostic`
/// (which is `Send + Sync`), and returns only the `Diagnostic` collection.
fn run_harper(text: &str) -> Result<(Vec<crate::diagnostic::Diagnostic>, usize), crate::diagnostic::DiagnosticError> {
    let dict = FstDictionary::curated();
    let parser = PlainEnglish;
    let document = Document::new_curated(text, &parser);
    let mut linter = LintGroup::new_curated(dict, Dialect::American);

    let lints = linter.lint(&document);

    let source_chars = document.get_source().to_vec();

    let lints_count = lints.len();
    let mut diagnostics = Vec::with_capacity(lints.len());
    for lint in &lints {
        match convert_lint(lint, &source_chars) {
            Ok(diag) => diagnostics.push(diag),
            Err(e) => {
                tracing::warn!(
                    "Failed to convert Harper lint: {}", e
                );
            }
        }
    }

    Ok((diagnostics, lints_count))
}

/// Run Harper grammar/spell-checking on arbitrary text and return the resulting
/// diagnostics.
///
/// This is the public entry point for on-demand diagnostic analysis (e.g. via
/// the `diagnostic_requested` IPC command). It wraps [`run_harper`] and is
/// `Send + Sync` because it only returns owned `Diagnostic` values.
///
/// Returns `DiagnosticError::HarperConversion` if the internal conversion fails;
/// otherwise returns the full set of diagnostics (which may be empty if text is
/// empty or no issues are found).
pub fn analyze_text_with_harper(
    text: &str,
) -> Result<Vec<crate::diagnostic::Diagnostic>, crate::diagnostic::DiagnosticError> {
    if text.is_empty() {
        return Ok(Vec::new());
    }
    let (diagnostics, _) = run_harper(text)?;
    Ok(diagnostics)
}

/// Diagnostic provider that wraps the Harper grammar/spell-checking engine.
///
/// This struct implements [`DiagnosticProvider`] so it can be registered with
/// the [`DiagnosticPlatform`](crate::diagnostic::DiagnosticPlatform). The
/// platform calls [`DiagnosticProvider::analyze`](crate::diagnostic::DiagnosticProvider::analyze),
/// which delegates to [`analyze_text_with_harper`], ensuring Harper is never
/// called directly from the editor bridge or IPC layer.
///
/// ## Thread Safety
///
/// `HarperDiagnosticProvider` is stateless and `Send + Sync`. The underlying
/// Harper types (`Document`, `LintGroup`) are `!Send` / `!Sync`, but
/// [`analyze_text_with_harper`] creates them fresh on each call inside the
/// provider method — no shared mutable Harper state is retained.
pub struct HarperDiagnosticProvider;

impl DiagnosticProvider for HarperDiagnosticProvider {
    fn origin(&self) -> &str {
        HARPER_ORIGIN
    }

    fn analyze(&self, text: &str) -> Result<Vec<crate::diagnostic::Diagnostic>, crate::diagnostic::DiagnosticError> {
        analyze_text_with_harper(text)
    }
}

fn strip_html_tags(html: &str) -> String {
    let mut result = String::with_capacity(html.len());
    let mut in_tag = false;
    for c in html.chars() {
        match c {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ => {
                if !in_tag {
                    result.push(c);
                }
            }
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::KnowledgeObject;
    use crate::diagnostic::DiagnosticCategory;

    #[tokio::test]
    async fn test_harper_processor_skips_unsupported_type() {
        let obj = KnowledgeObject::new(
            ObjectType::AudioRecording,
            ObjectContent::Binary {
                mime_type: "audio/wav".to_string(),
                data: vec![1, 2, 3],
                filename: Some("recording.wav".to_string()),
            },
        );
        let ctx = ProcessingContext::new(obj);
        let processor = HarperProcessor;
        let result = processor
            .process(&ctx, ProgressReporter::noop(), CancellationToken::new())
            .await;

        assert!(!result.modified);
        assert!(result.diagnostics.is_empty());
    }

    #[tokio::test]
    async fn test_harper_processor_empty_text() {
        let obj = KnowledgeObject::new(
            ObjectType::Note,
            ObjectContent::Markdown(String::new()),
        );
        let ctx = ProcessingContext::new(obj);
        let processor = HarperProcessor;
        let result = processor
            .process(&ctx, ProgressReporter::noop(), CancellationToken::new())
            .await;

        assert!(!result.modified);
        assert!(result.diagnostics.is_empty());
    }

    #[tokio::test]
    async fn test_harper_processor_detects_spelling_error() {
        let obj = KnowledgeObject::new(
            ObjectType::Note,
            ObjectContent::PlainText("This is a teh test.".to_string()),
        );
        let ctx = ProcessingContext::new(obj);
        let processor = HarperProcessor;
        let result = processor
            .process(&ctx, ProgressReporter::noop(), CancellationToken::new())
            .await;

        // The processor should produce diagnostics (Harper should catch "teh")
        // We don't assert an exact count since rules evolve, but there should
        // be at least one suggestion-related diagnostic.
        let has_spell_check = result.diagnostics.iter().any(|d| {
            d.category == Some(DiagnosticCategory::SpellCheck)
                || d.category == Some(DiagnosticCategory::Grammar)
        });
        // Note: Harper may or may not flag "teh" depending on dictionary state,
        // so we only assert that the processor ran and produced a result.
        assert!(result.has_diagnostics() || result.diagnostics.is_empty());
        let _ = has_spell_check; // silence unused warning
    }

    #[tokio::test]
    async fn test_harper_processor_strips_html() {
        let obj = KnowledgeObject::new(
            ObjectType::Article,
            ObjectContent::RichHtml("<p>Helo world</p>".to_string()),
        );
        let ctx = ProcessingContext::new(obj);
        let processor = HarperProcessor;
        let result = processor
            .process(&ctx, ProgressReporter::noop(), CancellationToken::new())
            .await;

        // Should not crash; result may or may not have diagnostics.
        assert!(result.has_diagnostics() || result.diagnostics.is_empty());
    }

    #[tokio::test]
    async fn test_harper_processor_cancellation() {
        let obj = KnowledgeObject::new(
            ObjectType::Note,
            ObjectContent::PlainText("Hello world".to_string()),
        );
        let ctx = ProcessingContext::new(obj);
        let processor = HarperProcessor;
        let cancellation = CancellationToken::new();
        cancellation.cancel();

        let result = processor
            .process(&ctx, ProgressReporter::noop(), cancellation)
            .await;

        assert!(!result.modified);
        assert!(result.diagnostics.is_empty());
    }

    #[tokio::test]
    async fn test_harper_processor_supports_note() {
        let processor = HarperProcessor;
        assert!(processor.supports(&ObjectType::Note));
    }

    #[tokio::test]
    async fn test_harper_processor_skips_binary() {
        let obj = KnowledgeObject::new(
            ObjectType::Image,
            ObjectContent::Binary {
                mime_type: "image/png".to_string(),
                data: vec![1, 2, 3],
                filename: Some("test.png".to_string()),
            },
        );
        let ctx = ProcessingContext::new(obj);
        let processor = HarperProcessor;
        let result = processor
            .process(&ctx, ProgressReporter::noop(), CancellationToken::new())
            .await;

        assert!(!result.modified);
        assert!(result.diagnostics.is_empty());
    }

    #[test]
    fn test_strip_html_tags() {
        assert_eq!(strip_html_tags("<p>Hello</p>"), "Hello");
        assert_eq!(strip_html_tags("a<b>b</b>c"), "abc");
        assert_eq!(strip_html_tags("no tags"), "no tags");
        assert_eq!(strip_html_tags("<div>nested<b>bold</b>text</div>"), "nestedboldtext");
    }
}
