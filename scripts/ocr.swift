/**
 * ocr.swift
 *
 * macOS Vision OCR helper for extracting text from image files.
 * Spawned as a child process from Electron main process.
 *
 * Usage: swift ocr.swift <image-path>
 *
 * Output: JSON to stdout
 * {"text": "extracted text", "blocks": [{"rect": {"x": 0, "y": 0, "w": 100, "h": 20}, "text": "word", "confidence": 0.9}]}
 *
 * Exit codes:
 * - 0: Success
 * - 1: Permission error (macOS vision framework permission denied)
 * - 2: Corrupt/unreadable image
 *
 * Requirements: 39.1, 39.4, 39.6
 */

import Foundation
import CoreImage
import UniformTypeIdentifiers
import Vision

// Minimum confidence threshold (Requirement 39.3)
let MIN_CONFIDENCE: VNConfidence = 0.3

// ---------------------------------------------------------------------------
// Exit codes
// ---------------------------------------------------------------------------

enum ExitCode: Int {
    case success = 0
    case permissionDenied = 1
    case corruptImage = 2
}

// ---------------------------------------------------------------------------
// Command-line argument handling
// ---------------------------------------------------------------------------

guard CommandLine.arguments.count >= 2 else {
    print("{\"error\": \"No image path provided\"}")
    exit(ExitCode.corruptImage.rawValue)
}

let imagePath = CommandLine.arguments[1]

// ---------------------------------------------------------------------------
// Image loading and downscaling (Requirement 39.8 - limit to 4096px)
// ---------------------------------------------------------------------------

func loadImage(at path: String) -> CIImage? {
    guard let url = URL(string: "file://\(path)") ?? URL(fileURLWithPath: path) else {
        return nil
    }
    
    guard let imageData = try? Data(contentsOf: url) else {
        return nil
    }
    
    guard var ciImage = CIImage(data: imageData) else {
        return nil
    }
    
    // Get image dimensions and downscale if needed (Req 39.8)
    let extent = ciImage.extent
    let maxDimension: CGFloat = 4096
    
    if extent.width > maxDimension || extent.height > maxDimension {
        let scale = min(maxDimension / extent.width, maxDimension / extent.height)
        let scaledImage = ciImage.transformed(by: CGAffineTransform(scaleX: scale, y: scale))
        ciImage = scaledImage
    }
    
    return ciImage
}

// ---------------------------------------------------------------------------
// OCR processing
// ---------------------------------------------------------------------------

func performOCR(on image: CIImage) -> (text: String, blocks: [[String: Any]])? {
    let requestHandler = VNImageRequestHandler(ciImage: image, options: [:])
    let request = VNRecognizeTextRequest()
    
    // Recognition level .accurate for better quality (Req 39.4)
    request.recognitionLevel = .accurate
    request.usesLanguageCorrection = true
    request.revision = VNRecognizeTextRequestRevision3
    
    do {
        try requestHandler.perform([request])
    } catch {
        // Check if this is a permission error
        if error.localizedDescription.contains("permission") || 
           error.localizedDescription.contains("access") ||
           (error as NSError).code == VNError.notAuthorized.rawValue {
            return nil
        }
        return nil
    }
    
    guard let observations = request.results else {
        return nil
    }
    
    var fullText = ""
    var textBlocks: [[String: Any]] = []
    
    for observation in observations {
        // Filter by confidence threshold (Req 39.3)
        if observation.confidence < MIN_CONFIDENCE {
            continue
        }
        
        let recognizedText = observation.string
        fullText += recognizedText + "\n"
        
        // Extract bounding box
        let boundingBox = observation.boundingBox
        // Convert from Vision coordinates (origin at bottom-left) to standard (origin at top-left)
        let normalizedRect: [String: Int] = [
            "x": Int(boundingBox.origin.x * 1000),
            "y": Int((1 - boundingBox.origin.y - boundingBox.size.height) * 1000),
            "w": Int(boundingBox.size.width * 1000),
            "h": Int(boundingBox.size.height * 1000)
        ]
        
        let block: [String: Any] = [
            "rect": normalizedRect,
            "text": recognizedText,
            "confidence": observation.confidence
        ]
        textBlocks.append(block)
    }
    
    return (text: fullText.trimmingCharacters(in: .whitespacesAndNewlines), 
            blocks: textBlocks)
}

// ---------------------------------------------------------------------------
// Main execution
// ---------------------------------------------------------------------------

// Check if this is macOS
#if os(macOS)
let isMacOS = true
#else
let isMacOS = false
#endif

// Non-macOS platforms should skip gracefully
if !isMacOS {
    print("{\"error\": \"OCR only available on macOS\", \"skip\": true}")
    exit(ExitCode.success.rawValue)
}

// For macOS, proceed with OCR
guard let image = loadImage(at: imagePath) else {
    print("{\"error\": \"Failed to load image\"}")
    exit(ExitCode.corruptImage.rawValue)
}

guard let result = performOCR(on: image) else {
    // Could not perform OCR - likely permission error
    print("{\"error\": \"Permission denied or OCR failed\"}")
    exit(ExitCode.permissionDenied.rawValue)
}

// Output JSON result
let output: [String: Any] = [
    "text": result.text,
    "blocks": result.blocks
]

if let jsonData = try? JSONSerialization.data(withJSONObject: output),
   let jsonString = String(data: jsonData, encoding: .utf8) {
    print(jsonString)
    exit(ExitCode.success.rawValue)
} else {
    print("{\"error\": \"Failed to serialize result\"}")
    exit(ExitCode.corruptImage.rawValue)
}