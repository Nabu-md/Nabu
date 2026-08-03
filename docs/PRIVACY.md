# Privacy & Security

How Nabu handles your data, privacy controls, and security practices.

## Table of Contents

1. [Privacy Philosophy](#privacy-philosophy)
2. [Data Storage](#data-storage)
3. [Telemetry](#telemetry)
4. [Crash Reporting](#crash-reporting)
5. [Privacy Controls](#privacy-controls)
6. [Data Export & Deletion](#data-export--deletion)
7. [Security](#security)
8. [Third-Party Services](#third-party-services)
9. [Compliance](#compliance)

---

## Privacy Philosophy

**Nabu is local-first.** Your data stays on your device by default.

### Core Principles

1. **Local-first**: All notes, vaults, and metadata are stored locally on your device
2. **No cloud sync**: Nabu does not sync your data to any cloud service
3. **User control**: You decide what data is collected (if anything)
4. **Transparency**: Clear documentation of all data handling
5. **Minimal collection**: Only essential data is ever collected (and only with permission)

---

## Data Storage

### What Nabu Stores Locally

All data is stored on your device in the following locations:

#### Vault Folder
- **Location**: Wherever you chose to create it
- **Contents**: Your notes (Markdown files), attachments, images, PDFs
- **Format**: Plain text Markdown files
- **Access**: You can open/edit with any text editor

#### .nabu Metadata Folder
- **Location**: Inside your vault folder
- **Contents**:
  - `settings.json` - App settings
  - `queue/` - Background processing queue
  - `logs/` - Application logs
  - Search indexes
  - Graph data
- **Access**: Managed by Nabu, but files are plain JSON/text

#### macOS
```
~/Library/Application Support/Nabu/
├── settings.json
└── logs/
```

#### Windows
```
%APPDATA%\Nabu\
├── settings.json
└── logs\
```

#### Linux
```
~/.config/nabu/
├── settings.json
└── logs/
```

### What Nabu Does NOT Store

- ❌ Your notes in any cloud service
- ❌ Personal information (unless you enter it in notes)
- ❌ Browsing history
- ❌ Usage analytics (unless you enable it)
- ❌ Crash reports (unless you enable them)

---

## Telemetry

### Default Settings

**Telemetry is DISABLED by default.**

### What Telemetry Collects (if enabled)

When you enable analytics, Nabu collects:

#### Anonymous Usage Data
- Feature usage (which settings you use)
- Performance metrics (startup time, memory usage)
- Error rates (crashes, failed operations)
- Platform information (OS, version)

#### What Telemetry Does NOT Collect
- ❌ Your note content
- ❌ Vault file names or paths
- ❌ Personal information
- ❌ Keystrokes or text input
- ❌ File contents

### Why We Collect Telemetry

Telemetry helps us:
- Identify bugs and crashes
- Prioritize features based on usage
- Improve performance
- Understand platform-specific issues

### How to Control Telemetry

1. Go to **Settings** → **Privacy**
2. Toggle **"Analytics Enabled"**
3. Changes take effect immediately

### Telemetry Data Retention

- Data is retained for 90 days
- Data is aggregated and anonymized
- You can request deletion at any time

---

## Crash Reporting

### Default Settings

**Crash reporting is DISABLED by default.**

### What Crash Reports Contain (if enabled)

When you enable crash reporting, Nabu sends:

#### Diagnostic Information
- Crash stack trace
- Operating system and version
- Nabu version
- Memory state at crash
- List of loaded plugins

#### What Crash Reports Do NOT Contain
- ❌ Your note content
- ❌ Vault paths (only generic paths like "vault_root")
- ❌ Personal information
- ❌ Full memory dumps

### Why We Collect Crash Reports

Crash reports help us:
- Fix bugs quickly
- Prioritize stability improvements
- Identify platform-specific issues

### How to Control Crash Reporting

1. Go to **Settings** → **Privacy**
2. Toggle **"Crash Reporting"**
3. Changes take effect immediately

### Local Crash Logs

Even with crash reporting disabled, Nabu keeps local logs:

**Location**: `.nabu/logs/crash.log`

**Contents**:
- Crash timestamps
- Stack traces
- Error messages
- System information

**You can**:
- View logs anytime
- Export logs for bug reports
- Delete logs manually

---

## Privacy Controls

### Settings → Privacy

All privacy controls are in one place:

#### Launch at Startup
- **Default**: OFF
- **What it does**: Starts Nabu when you log in
- **Privacy impact**: None

#### Analytics Enabled
- **Default**: OFF
- **What it does**: Sends anonymous usage data
- **Privacy impact**: Minimal (see Telemetry section)

#### Crash Reporting
- **Default**: OFF
- **What it does**: Sends crash reports when enabled
- **Privacy impact**: Low (see Crash Reporting section)

#### Auto-lock on Idle
- **Default**: OFF
- **What it does**: Locks Nabu after inactivity
- **Privacy impact**: Positive (protects your notes)

#### Auto-lock Timeout
- **Default**: 15 minutes
- **What it does**: Time before auto-lock activates

### Granular Controls

You can control data collection at multiple levels:

1. **Global**: Disable all telemetry and crash reporting
2. **Per-feature**: Enable/disable specific features
3. **Per-session**: Toggle during use (where applicable)

---

## Data Export & Deletion

### Export Your Data

Nabu makes it easy to export all your data:

#### Export Settings
1. Go to **Settings** → **Import & Export**
2. Click **"Export Settings"**
3. Save the JSON file

**Settings export includes**:
- All preferences
- Keyboard shortcuts
- Templates
- Smart folders
- Canvases

#### Export Notes
1. Right-click a note → **Export**
2. Choose format (Markdown, HTML, PDF, etc.)
3. Select destination

#### Export All Notes
1. Use your file manager to copy the vault folder
2. All notes are plain Markdown files

### Delete Your Data

#### Delete a Single Note
1. Right-click note → **Delete**
2. Note moves to Trash
3. Empty Trash to permanently delete

#### Delete All Notes
1. Close Nabu
2. Delete the vault folder
3. Delete the `.nabu` metadata folder

#### Delete Settings
1. Close Nabu
2. Delete `.nabu/settings.json` (or `settings.json` in platform-specific location)
3. Restart Nabu

#### Delete Logs
1. Navigate to `.nabu/logs/`
2. Delete log files manually

### Request Data Deletion

If you've enabled telemetry or crash reporting and want your data deleted:

1. Email privacy@farolabs.com
2. Include your Nabu version and approximate dates
3. We'll delete your data within 30 days

---

## Security

### Data Protection

#### Local Storage
- Notes stored as plain files (you control access)
- Settings stored as JSON (in user-writable location)
- No encryption by default (use system encryption)

#### File Permissions
- Nabu only accesses your vault folder
- Nabu does not run with elevated privileges
- You control folder permissions

### Network Security

#### Network Access
- Nabu does NOT connect to the internet by default
- Optional features that connect:
  - Auto-update checker (checks for updates)
  - Telemetry (if enabled)
  - Crash reporting (if enabled)

#### Firewall
- All network connections are opt-in
- You can block Nabu in your firewall for complete isolation

### Code Security

#### Dependencies
- Regular security audits
- Minimal external dependencies
- No telemetry SDKs

#### Updates
- Regular security patches
- Automatic update notifications (optional)
- Signed releases (coming in v1.0)

---

## Third-Party Services

### Optional Integrations

Nabu may integrate with third-party services only if you choose to:

#### AI Services (Experimental)
- **Whisper**: Voice dictation (downloads models locally)
- **AI Summarization**: May call external API (future)
- **Semantic Search**: May call external API (future)

**Privacy**: AI features are opt-in. Check settings for details.

### No Mandatory Third-Party Services

Nabu does not require:
- ❌ Account creation
- ❌ Cloud services
- ❌ External APIs
- ❌ License validation servers

---

## Compliance

### GDPR (EU Users)

Your rights under GDPR:

- **Right to access**: Export all your data
- **Right to deletion**: Delete all your data
- **Right to portability**: Export in standard formats
- **Right to opt-out**: Disable all telemetry

### CCPA (California Users)

Your rights under CCPA:

- **Right to know**: What data is collected
- **Right to delete**: Delete your data
- **Right to opt-out**: Opt-out of data sales (we don't sell data)
- **Right to non-discrimination**: No penalty for exercising rights

### COPPA (Children)

Nabu is not intended for users under 13. We do not knowingly collect data from children.

---

## Audit Log

Nabu maintains a local audit log of:

### Settings Changes
- Timestamp
- Setting changed
- Old value
- New value

### Vault Operations
- Vault opened
- Vault closed
- Vault switched

### Data Access
- Files created/modified/deleted
- Imports performed
- Exports performed

**Location**: `.nabu/logs/audit.log`

**Retention**: 30 days (configurable)

---

## Privacy Checklist

Use this checklist to ensure your privacy:

- [ ] Telemetry is disabled (Settings → Privacy)
- [ ] Crash reporting is disabled (unless you want to help)
- [ ] Vault is stored locally (not on a shared drive)
- [ ] `.nabu` folder is excluded from cloud sync
- [ ] Auto-lock is enabled (if on shared computer)
- [ ] Strong system password/FaceID/TouchID enabled
- [ ] File system encryption enabled (FileVault/BitLocker/LUKS)

---

## Contact

For privacy questions or concerns:

- **Email**: privacy@farolabs.com
- **GitHub**: [github.com/farolabs/nabu](https://github.com/farolabs/nabu)
- **Mailing address**: Faro Labs, [Address]

---

## Updates

- **v0.1.0** (2025-08-03): Initial privacy documentation for Phase 15.2

---

**Your privacy matters.** Nabu is designed to respect it.