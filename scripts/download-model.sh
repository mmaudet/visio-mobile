#!/bin/bash
set -euo pipefail

MODEL_DIR="models"
MODEL_FILE="$MODEL_DIR/selfie_segmentation.onnx"

if [ -f "$MODEL_FILE" ]; then
    echo "Model already exists at $MODEL_FILE"
    exit 0
fi

mkdir -p "$MODEL_DIR"

# MediaPipe selfie segmentation model converted to ONNX
# Source: https://storage.googleapis.com/mediapipe-models/image_segmenter/selfie_segmentation/float16/latest/selfie_segmentation.tflite
# We use an ONNX-converted version
echo "Downloading selfie segmentation ONNX model..."
# TODO: Host the ONNX model and update this URL
# For now, download the TFLite model from MediaPipe and convert manually
# curl -L -o "$MODEL_FILE" "URL_TBD"
echo "WARNING: Model URL not yet configured. Please download the MediaPipe selfie segmentation model"
echo "and convert it to ONNX format, then place it at: $MODEL_FILE"
echo ""
echo "Steps:"
echo "1. Download from: https://storage.googleapis.com/mediapipe-models/image_segmenter/selfie_segmentation/float16/latest/selfie_segmentation.tflite"
echo "2. Convert to ONNX using tf2onnx or mediapipe-to-onnx tools"
echo "3. Place at: $MODEL_FILE"
