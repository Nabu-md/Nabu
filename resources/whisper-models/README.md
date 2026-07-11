# Whisper Models

This directory holds whisper.cpp model files for audio dictation.

## Models

- `ggml-base.en.bin` — Base English model (~140 MB). Fast, good for short dictations.
- `ggml-large-v3-turbo-q5_0.bin` — Large-V3 Turbo Q5 model (~550 MB). Higher accuracy, more RAM.

## Download

In development, run:

```bash
bash scripts/download-whisper-model.sh base
bash scripts/download-whisper-model.sh large-v3-turbo-q5
```

In production builds, the Base model is bundled here automatically by the build script.
The Large model is downloaded on demand from Settings → Audio Dictation.

## Notes

- Models are never committed to git (see `.gitignore`).
- The whisper binary itself lives at `process.resourcesPath/whisper` in production.
