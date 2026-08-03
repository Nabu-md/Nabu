# Frequently Asked Questions & Troubleshooting

Common questions and solutions for Nabu.

## Installation

### Q: macOS says "Nabu can't be opened because it's from an unidentified developer"

**A**: This is normal for apps not in the App Store. Right-click the app and select "Open", then click "Open" again in the dialog.

### Q: Windows Defender is blocking the installer

**A**: Click "More info" → "Run anyway". We're working on code signing for the official release.

### Q: Linux AppImage won't run

**A**: Make sure it's executable:
```bash
chmod +x Nabu-0.1.0.AppImage
```

If you get a library error, you may need to install FUSE:
```bash
# Ubuntu/Debian
sudo apt install libfuse2

# Fedora
sudo dnf install fuse
```

---

## Vault Management

### Q: Where is my vault stored?

**A**: Your vault is wherever you chose to create it. It's a regular folder. Click the vault name in the toolbar to see the path.

### Q: Can I move my vault after creating it?

**A**: Yes! Close Nabu, move the folder, then reopen Nabu and select the new location. Your `.nabu` metadata folder moves with it.

### Q: What's the `.nabu` folder?

**A**: It stores metadata like search indexes, graph data, and settings. Your notes are safe without it. You can exclude it from sync services.

### Q: Can I use a Dropbox/iCloud/Google Drive folder as my vault?

**A**: Yes, but we recommend:
- Use the folder outside the sync service's internal folder
- Exclude `.nabu` from syncing to avoid conflicts
- Close Nabu before syncing large changes

---

## Notes & Editing

### Q: My note won't save

**A**: Check:
1. Do you have write permission for the vault folder?
2. Is the file open in another app?
3. Is your disk full?

### Q: Can I use Nabu with other text editors?

**A**: Yes! Your notes are plain Markdown files. You can edit them with any text editor. Refresh Nabu to see changes.

### Q: Does Nabu support real-time collaboration?

**A**: Not yet. For now, avoid editing the same note in multiple apps simultaneously.

### Q: How do I format bold/italic/links?

**A**: Use Markdown syntax:
- **Bold**: `**text**`
- *Italic*: `*text*`
- [Link](url): `[text](url)`
- [[Link]]: `[[Note Title]]`

See [User Guide → Note Editing](USER_GUIDE.md#note-editing) for more.

---

## Search

### Q: Search isn't finding my note

**A**:
1. Make sure indexing is enabled (Settings → Search → Index on Startup)
2. Try a manual re-index: close and reopen the vault
3. Check file permissions

### Q: Can I search within a specific folder?

**A**: Use the folder filter in the search panel, or use Smart Folders.

### Q: Does search include archived notes?

**A**: Yes, all `.md` files are indexed including those in `archive/`.

---

## Graph & Links

### Q: Why are my wikilinks showing as "broken"?

**A**: Check:
1. Note title matches exactly (case-sensitive)
2. Note exists in your vault
3. File extension is `.md`

### Q: Can I link to a specific section in a note?

**A**: Yes: `[[Note Title#Section Heading]]`

### Q: The graph is too cluttered

**A**: Try:
- Enable folder nodes to group related notes
- Adjust gravity and spacing in Settings → Graph
- Use filters to show only relevant notes

---

## Performance

### Q: Nabu is slow with large vaults

**A**:
1. Increase worker pool size in Settings → Performance
2. Disable graph auto-refresh
3. Exclude large binary files from vault
4. Consider splitting vault into projects

### Q: High memory usage

**A**:
1. Close unused notes
2. Restart Nabu periodically for long sessions
3. Check for large PDFs with OCR enabled

### Q: Search is slow

**A**:
1. Enable "Index on Startup" (Settings → Search)
2. Reduce max results if you have thousands of notes
3. Disable fuzzy matching for faster results

---

## Import/Export

### Q: Can I import from [app]?

**A**: See [Import Guide](IMPORT_GUIDE.md) for supported sources.

### Q: Import failed with an error

**A**:
1. Check file permissions
2. Ensure files are UTF-8 encoded
3. Try importing a smaller batch
4. Check `.nabu/logs/` for details

### Q: Where are exported files saved?

**A**: You choose the destination folder when exporting. By default, it's your Downloads folder or the folder containing the original note.

---

## Voice Dictation

### Q: Dictation isn't working

**A**:
1. Grant microphone permission when prompted
2. Check system audio settings
3. Try a different Whisper model in Settings → Experimental
4. Ensure you have an internet connection (for larger models)

### Q: Dictation accuracy is poor

**A**:
1. Use a better microphone
2. Try the `ggml-small` model (Settings → Experimental)
3. Speak clearly and at a moderate pace
4. Enable "Auto-format filler words"

---

## Platform-Specific

### macOS

**Q: Nabu isn't in my menu bar**

**A**: Check System Preferences → Control Center → Menu Bar. Nabu should appear when running.

**Q: Can't drag files into Nabu**

**A**: Grant Nabu accessibility permissions in System Preferences → Security & Privacy → Privacy → Accessibility.

### Windows

**Q: File associations not working**

**A**: Reinstall Nabu and check "Associate .md files" during installation.

**Q: Windows Defender SmartScreen is blocking**

**A**: Click "More info" → "Run anyway". We're working on code signing.

### Linux

**Q: Nabu won't start**

**A**: Check you have required dependencies:
```bash
# Ubuntu/Debian
sudo apt install libwebkit2gtk-4.1-0 libgtk-3-0
```

**Q: Desktop entry missing**

**A**: Run:
```bash
nabu --install-desktop-entry
```

---

## Settings

### Q: Can I backup my settings?

**A**: Yes! Go to Settings → Import & Export → Export Settings. Save the JSON file.

### Q: How do I restore settings?

**A**: Settings → Import & Export → Import Settings. Select your backup JSON.

### Q: Settings got corrupted**

**A**: Delete `.nabu/settings.json` and restart Nabu. Settings will reset to defaults.

---

## Privacy & Security

### Q: Does Nabu send my data anywhere?

**A**: No, not by default. Nabu is local-first. Optional features:
- **Analytics**: Disabled by default. You can enable in Settings → Privacy.
- **Crash reporting**: Disabled by default. You can enable in Settings → Privacy.
- **Auto-updates**: Check for updates but don't download without permission.

See [Privacy Policy](PRIVACY.md) for details.

### Q: Is my data encrypted?

**A**: Notes are stored as plain files. For encryption:
- Use an encrypted vault folder (e.g., VeraCrypt)
- Use file system encryption (FileVault, BitLocker)

### Q: Can Nabu access my cloud storage?

**A**: Nabu doesn't directly access cloud storage. Use your sync service's client (Dropbox, iCloud, etc.) to sync the vault folder.

---

## Troubleshooting

### Nabu Won't Start

1. **Check logs**: Look in `.nabu/logs/` for error messages
2. **Delete settings**: Remove `.nabu/settings.json`
3. **Reinstall**: Download fresh installer
4. **Check permissions**: Ensure vault folder is writable

### Performance Issues

1. **Restart Nabu**: Clears memory
2. **Reduce vault size**: Archive old notes
3. **Disable features**: Turn off graph, OCR if not needed
4. **Check disk space**: Ensure adequate free space

### Display Issues

1. **Update graphics drivers**
2. **Try different theme**: Settings → Appearance → Theme
3. **Disable reduced motion**: Settings → Accessibility

### Sync Conflicts

1. **Close Nabu** before syncing
2. **Use version control** (Git) for conflict resolution
3. **Enable conflict detection** in sync service
4. **Manually merge** if needed

---

## Getting Help

### Resources

- **Documentation**: [docs.nabu.md](https://docs.nabu.md)
- **GitHub Issues**: [Report bugs](https://github.com/farolabs/nabu/issues)
- **Community**: [Discord](https://discord.gg/farolabs)
- **Email**: support@farolabs.com

### Reporting Bugs

When reporting issues, include:
1. **Nabu version**: Check Settings → About
2. **Operating system**: macOS/Windows/Linux + version
3. **Steps to reproduce**: Detailed steps
4. **Expected vs actual**: What should happen vs what happened
5. **Logs**: Attach `.nabu/logs/nabu.log`
6. **Screenshots**: If applicable

### Feature Requests

We welcome feature requests! Open a GitHub issue with:
- Clear description of the feature
- Use case (why you want it)
- How you'd use it

---

## Known Issues

### Current Limitations

1. **Real-time collaboration**: Not supported yet
2. **Mobile apps**: No iOS/Android apps yet
3. **Large PDFs**: Very large PDFs (>100MB) may be slow
4. **Evernote**: Complex tables may not convert perfectly
5. **Notion**: Databases require manual restructuring

### Workarounds

See specific FAQ items above for workarounds to common issues.

---

## Tips & Tricks

### Productivity

- **Use templates** for recurring note types
- **Set up smart folders** for automatic organization
- **Use the reading queue** to track what you're reading
- **Quick capture** for fast note-taking

### Organization

- **Tag consistently**: Use a tag taxonomy
- **Folder hierarchy**: Keep it flat (2-3 levels deep)
- **Archive liberally**: Hide notes you don't need
- **Use frontmatter**: Add metadata to notes

### Performance

- **Keep vault size manageable**: <10,000 notes for best performance
- **Exclude binaries**: Don't store videos, large datasets in vault
- **Regular maintenance**: Archive old notes periodically
- **Index on startup**: Enable for faster search

---

## Update History

- **v0.1.0** (2025-08-03): Initial FAQ for Phase 15.2

---

**Still have questions?** Reach out at support@farolabs.com or open a GitHub issue.