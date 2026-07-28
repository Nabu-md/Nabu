# Nabu Safari Extension

Safari Web Extension for capturing web content directly into Nabu.

## Architecture

```
Safari Extension
    ↓ (native messaging)
Native Messaging Host
    ↓ (Unix socket)
Tauri App (CaptureEngine)
    ↓
ProcessingPipeline → StorageManager
```

## Components

### Safari Extension

The browser extension provides three capture modes:

- **Capture Page**: Captures the current page URL, title, and favicon as a Bookmark
- **Capture Selection**: Captures selected text with source URL and title as a Note
- **Capture Full Page**: Captures the complete HTML content as a Document

### Native Messaging Host

A separate binary (`native-messaging-host`) that:

1. Reads length-prefixed JSON messages from Safari via stdin
2. Validates messages (command, payload size, capture type)
3. Forwards validated messages to the Tauri app via Unix socket
4. Reads responses from the Tauri app
5. Writes length-prefixed JSON responses to Safari via stdout

### Tauri App Integration

The Tauri app:

1. Registers `BrowserCaptureHandler` with the `CaptureEngine`
2. Starts a Unix socket server at `/tmp/nabu-native-messaging.sock`
3. Receives capture requests from the native messaging host
4. Dispatches requests to the `CaptureEngine`
5. Returns `CaptureResult` responses

## Installation

### 1. Build the Native Messaging Host

```bash
cargo build --bin native-messaging-host --release
```

### 2. Install the Native Messaging Host

Copy the binary to a permanent location:

```bash
cp target/release/native-messaging-host /usr/local/bin/nabu-native-messaging-host
chmod +x /usr/local/bin/nabu-native-messaging-host
```

### 3. Register the Native Messaging Host

Create the Safari native messaging hosts directory if it doesn't exist:

```bash
mkdir -p ~/Library/Application\ Support/com.apple.Safari/NativeMessagingHosts
```

Copy the plist file:

```bash
cp extensions/safari/native-messaging/com.nabu.capture.host.plist \
   ~/Library/Application\ Support/com.apple.Safari/NativeMessagingHosts/
```

Update the plist to point to the correct binary path if needed.

### 4. Install the Safari Extension

1. Open Safari
2. Go to Safari → Settings → Advanced
3. Enable "Show Develop menu in menu bar"
4. Go to Develop → Show Web Extension Builder
5. Click "Add Extension" and select the `extensions/safari` folder
6. Enable the extension in Safari Settings → Extensions

## Security

The native messaging host implements several security measures:

- **Command allow-listing**: Only `capture` command is accepted
- **Payload size limit**: Maximum 1MB payload size
- **Capture type validation**: Only `bookmark`, `note`, and `document` are accepted
- **Unix socket permissions**: Socket is created with appropriate permissions

## Message Protocol

### Request Format

```json
{
  "requestId": 1,
  "command": "capture",
  "captureType": "bookmark",
  "payload": {
    "url": "https://example.com",
    "title": "Example Domain",
    "favicon": "https://example.com/favicon.ico"
  }
}
```

### Response Format

```json
{
  "requestId": 1,
  "success": true,
  "result": {
    "success": true,
    "knowledge_object_id": "uuid",
    "message": "Browser bookmark captured successfully"
  }
}
```

## Capture Types

### Bookmark

Captures:
- URL
- Page title
- Favicon (when available)

Creates a `KnowledgeObject` of type `Bookmark`.

### Note

Captures:
- Selected text
- Source URL
- Page title

Creates a `KnowledgeObject` of type `Note`.

### Document

Captures:
- Complete HTML
- Page URL
- Title

Creates a `KnowledgeObject` of type `Document`.

## Development

### Running Tests

```bash
# Run nabu-core tests
cargo test -p nabu-core

# Run native messaging host tests
cargo test --bin native-messaging-host
```

### Debugging

Enable debug logging in the Tauri app to see capture requests and responses.

## Troubleshooting

### Native messaging host not found

Ensure the plist file is correctly installed in:
```
~/Library/Application Support/com.apple.Safari/NativeMessagingHosts/com.nabu.capture.host.plist
```

### Socket connection refused

Ensure the Tauri app is running before attempting to capture. The socket server starts when the app launches.

### Permission denied

Ensure the native messaging host binary is executable:
```bash
chmod +x /usr/local/bin/nabu-native-messaging-host
```
