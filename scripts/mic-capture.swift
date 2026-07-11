/**
 * mic-capture.swift
 *
 * macOS microphone capture helper for streaming audio to whisper.cpp.
 * Spawned as a child process from Electron main process.
 *
 * Usage: swift mic-capture.swift
 *
 * Output: Raw 16-bit PCM audio at 16kHz mono to stdout.
 *
 * Exit codes:
 * - 0: Success (normal exit)
 * - 1: Permission denied (microphone access denied)
 * - 2: Audio capture error
 *
 * Requirements: 41.3, 42.1
 */

import Foundation
import AVFoundation

// Audio format constants
let SAMPLE_RATE: Double = 16000.0
let CHANNELS: AVAudioChannelCount = 1
let FORMAT: AVAudioCommonFormat = .pcmFormatInt16

// Exit codes
enum ExitCode: Int {
    case success = 0
    case permissionDenied = 1
    case audioError = 2
}

// Global flag for stopping capture
var shouldStop = false

// Signal handler for SIGTERM
signal(SIGTERM) { _ in
    shouldStop = true
}

// Check if this is macOS
#if os(macOS)
let isMacOS = true
#else
let isMacOS = false
#endif

// Non-macOS platforms should skip gracefully
if !isMacOS {
    // Output empty JSON to indicate skip
    print("{\"error\": \"Microphone capture only available on macOS\", \"skip\": true}")
    exit(ExitCode.success.rawValue)
}

// Request microphone permission
let session = AVAudioSession.sharedInstance()
do {
    try session.setCategory(.record, mode: .voiceChat, options: [])
    try session.setActive(true)
    
    // Request permission
    let permissionStatus = session.recordPermission
    if permissionStatus == .denied {
        print("{\"error\": \"Microphone access denied\", \"permission\": false}")
        exit(ExitCode.permissionDenied.rawValue)
    }
    
    if permissionStatus == .undetermined {
        // Request permission - this will show the system dialog
        try session.requestRecordPermission { granted in
            // Permission request is async, but we need to continue
        }
    }
} catch {
    print("{\"error\": \"Failed to configure audio session: \(error.localizedDescription)\"}")
    exit(ExitCode.audioError.rawValue)
}

// Set up audio engine
let audioEngine = AVAudioEngine()
let inputNode = audioEngine.inputNode

// Get the desired format
let inputFormat = AVAudioFormat(commonFormat: FORMAT, sampleRate: SAMPLE_RATE, channels: CHANNELS, interleaved: true)!

// Install tap on input node
inputNode.installTap(onBus: 0, bufferSize: 1024, format: inputFormat) { buffer, time in
    // Check if we should stop
    if shouldStop {
        return
    }
    
    // Convert to 16-bit PCM and write to stdout
    guard let channelData = buffer.floatChannelData else { return }
    
    let frameLength = Int(buffer.frameLength)
    let bytes = frameLength * MemoryLayout<Int16>.size
    
    // Allocate buffer for Int16 samples
    var pcmBuffer = [Int16](repeating: 0, count: frameLength)
    
    // Convert float samples to Int16
    for i in 0..<frameLength {
        let floatSample = channelData[0][i]
        // Clamp to [-1, 1] and convert to Int16 range
        let clamped = max(-1.0, min(1.0, Double(floatSample)))
        pcmBuffer[i] = Int16(clamped * Double(Int16.max))
    }
    
    // Write raw PCM to stdout
    let data = Data(bytes: pcmBuffer, count: bytes)
    FileHandle.standardOutput.write(data)
}

// Prepare and start audio engine
audioEngine.prepare()
do {
    try audioEngine.start()
} catch {
    print("{\"error\": \"Failed to start audio engine: \(error.localizedDescription)\"}")
    exit(ExitCode.audioError.rawValue)
}

// Keep the process running until stopped
// The main process will send SIGTERM to stop
while !shouldStop {
    Thread.sleep(forTimeInterval: 0.1)
}

// Clean up
inputNode.removeTap(onBus: 0)
audioEngine.stop()
try? session.setActive(false)

// Exit successfully
exit(ExitCode.success.rawValue)