# Release Channels

Source: reference/release-channels.md
URL: /reference/release-channels

# Release Channels

Nabu publishes Stable and Alpha release metadata to GitHub Pages.

## Stable

Stable follows manually promoted releases. This is the right channel for normal use.

The stable updater metadata lives at:

```txt
/stable/latest.json
```

The public download page points at the latest stable release.

## Alpha

Alpha follows pushes to `main`. It receives fixes and features earlier, but it can be rougher than Stable.

The alpha updater metadata lives at:

```txt
/alpha/latest.json
```

Compatibility endpoints also point to the alpha metadata:

```txt
/latest.json
/latest-canary.json
```

## Before Switching

Commit or push important vault changes before changing release channel or installing an update. Your notes are local files, but a clean Git state makes recovery simpler.