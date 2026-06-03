# Model Resources

This directory will contain the BGE-micro ONNX model files for embedding generation.

The model files will be downloaded from `Xenova/bge-micro-v2` and bundled with the application for offline operation.

Required files:
- `bge-micro-v2/onnx/model_quantized.onnx`
- `bge-micro-v2/tokenizer.json`
- `bge-micro-v2/tokenizer_config.json`
- Other model configuration files

These files will be bundled via electron-builder's `extraResources` configuration.
