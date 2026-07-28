# Rich Browser Capture Handlers

This document describes the specialised browser capture handlers that transform common web content into structured KnowledgeObjects.

## Handlers

### ArticleCaptureHandler

Extracts article content from web pages using a Readability-inspired algorithm.

**Source type:** `article`

**Object type:** `Document`

**Payload format:**
```json
{
  "html": "<html>...</html>",
  "url": "https://example.com/article"
}
```

**Extracted metadata:**
- `title` — Article title from `<h1>` or `<title>`
- `author` — From meta tags or structured data
- `published_date` — From meta tags
- `canonical_url` — From `<link rel="canonical">` or `og:url`
- `reading_time_minutes` — Estimated based on word count (200 WPM)

**Content extraction:**
1. Tries common article selectors (`article`, `.post-content`, `.entry-content`, etc.)
2. Falls back to paragraph extraction
3. Last resort: all page text

**Fallback:** If extraction fails, the handler returns an error. The calling code should fall back to standard page capture.

### YouTubeCaptureHandler

Extracts YouTube video metadata without downloading the video or subtitles.

**Source type:** `youtube`

**Object type:** `AudioRecording`

**Payload format:**
```json
{
  "html": "<html>...</html>",
  "url": "https://www.youtube.com/watch?v=VIDEO_ID"
}
```

**Extracted metadata:**
- `title` — Video title
- `channel` — Channel name
- `publish_date` — From `itemprop="datePublished"`
- `duration` — ISO 8601 duration from `itemprop="duration"`
- `thumbnail_url` — From `og:image` or constructed from video ID
- `description` — From `og:description`

**Video ID extraction:**
Supports multiple YouTube URL formats:
- `youtube.com/watch?v=VIDEO_ID`
- `youtu.be/VIDEO_ID`
- `youtube.com/embed/VIDEO_ID`

### GitHubRepositoryHandler

Extracts GitHub repository metadata without cloning the repository.

**Source type:** `github`

**Object type:** `Repository`

**Payload format:**
```json
{
  "html": "<html>...</html>",
  "url": "https://github.com/owner/repo"
}
```

**Extracted metadata:**
- `owner` — Repository owner from URL
- `repo_name` — Repository name from URL
- `description` — From page content or `og:description`
- `star_count` — From star counter element
- `primary_language` — From language indicator
- `license` — From license link
- `topics` — List of topic tags
- `readme_preview` — First 500 characters of README

## Registration

All handlers are registered with the `CaptureEngine` in the Tauri app setup:

```rust
engine.register(Arc::new(ArticleCaptureHandler::new()));
engine.register(Arc::new(YouTubeCaptureHandler::new()));
engine.register(Arc::new(GitHubRepositoryHandler::new()));
```

## Error Handling

If specialised extraction fails, the handler returns a `CaptureResult` with `success: false`. The calling code should fall back to standard page capture (`BrowserCaptureHandler`).

## Future Compatibility

New capture types can be added by:
1. Implementing `CaptureHandler` for a new handler struct
2. Registering it with the `CaptureEngine`

No modifications to `CaptureEngine` or other handlers are required.

## Dependencies

- `scraper` — HTML parsing and CSS selector queries
- `regex` — URL pattern matching
