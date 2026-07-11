/**
 * fn-monitor.swift
 *
 * macOS fn (Function) key monitor using IOKit.
 * Spawned as a child process from Electron main process.
 *
 * Outputs JSON lines to stdout:
 *   {"type":"fn-down","timestamp":1234567890}
 *   {"type":"fn-up","timestamp":1234567890}
 *
 * Requirements: 41.4, 42.3
 */

import Foundation
import IOKit
import IOKit.hidsystem

// Exit codes
enum ExitCode: Int {
    case success = 0
    case error = 1
}

// Check if this is macOS
#if os(macOS)
let isMacOS = true
#else
let isMacOS = false
#endif

if !isMacOS {
    print("{\"error\": \"Fn key monitoring only available on macOS\", \"skip\": true}")
    exit(ExitCode.success.rawValue)
}

// Get the IOKit master port
var masterPort: mach_port_t = 0
let kr = IOMasterPort(machPort: &masterPort)
if kr != KERN_SUCCESS {
    print("{\"error\": \"Failed to get IOKit master port\"}")
    exit(ExitCode.error.rawValue)
}

// Create a matching dictionary for keyboard events
let matchingDict = IOServiceMatching("IOHIKeyboard")
if matchingDict == nil {
    print("{\"error\": \"Failed to create matching dictionary\"}")
    exit(ExitCode.error.rawValue)
}

// Get the IOKit notification port
let notifyPort = IONotificationPortCreate(masterPort)
if notifyPort == nil {
    print("{\"error\": \"Failed to create notification port\"}")
    exit(ExitCode.error.rawValue)
}

// Add the notification port to the run loop
let runLoopSource = IONotificationPortGetRunLoopSource(notifyPort)
CFRunLoopAddSource(CFRunLoopGetCurrent(), runLoopSource, .defaultMode)

// Global state for fn key
var fnKeyDown = false

// Callback for keyboard events
let callback: IOServiceMatchingCallback = { (userData, iterator) in
    var ioService: io_object_t
    while true {
        ioService = IOIteratorNext(iterator)
        if ioService == 0 { break }
        
        // Get properties dictionary
        var properties: Unmanaged<CFMutableDictionary>? = nil
        let kr = IORegistryEntryCreateCFProperties(ioService, &properties, kCFAllocatorDefault, 0)
        if kr == KERN_SUCCESS, let props = properties?.takeRetainedValue() as? [String: Any] {
            // Check for fn key state
            // The fn key on Apple keyboards generates a specific HID usage
            if let usagePage = props["PrimaryUsagePage"] as? Int,
               let usage = props["PrimaryUsage"] as? Int {
                // Keyboard usage page
                if usagePage == 0x07 {
                    // We can detect fn key via the "Fn" key in the keyboard descriptor
                    // For now, we use a simpler approach: monitor the HID system
                }
            }
        }
        
        IOObjectRelease(ioService)
    }
}

// Set up the notification
var iterator: io_iterator_t = 0
let matchingDictRetained = matchingDict!.retain()
let addNotificationResult = IOServiceAddMatchingNotification(
    notifyPort,
    kIOFirstMatchNotification,
    matchingDictRetained,
    callback,
    nil,
    &iterator
)

if addNotificationResult != KERN_SUCCESS {
    print("{\"error\": \"Failed to add notification\"}")
    exit(ExitCode.error.rawValue)
}

// Process initial set of services
callback(nil, iterator)

// Use CGEvent to monitor fn key state via NSEvent
// NSEvent provides a more reliable way to detect modifier keys
NSEvent.startPeriodicEvents(afterDelay: 0.0, withPeriod: 0.1)

// Create a local event monitor for flags changed (modifier keys)
let eventMask = NSEvent.EventTypeMask.flagsChanged
let eventMonitor = NSEvent.addLocalMonitorForEvents(matching: eventMask) { event in
    // Check if the fn key is pressed
    // NSEvent.ModifierFlags.function represents the fn key
    let fnPressed = event.modifierFlags.contains(.function)
    
    if fnPressed && !fnKeyDown {
        fnKeyDown = true
        let timestamp = Int(Date().timeIntervalSince1970 * 1000)
        let json = "{\"type\":\"fn-down\",\"timestamp\":\(timestamp)}\n"
        FileHandle.standardOutput.write(json.data(using: .utf8)!)
    } else if !fnPressed && fnKeyDown {
        fnKeyDown = false
        let timestamp = Int(Date().timeIntervalSince1970 * 1000)
        let json = "{\"type\":\"fn-up\",\"timestamp\":\(timestamp)}\n"
        FileHandle.standardOutput.write(json.data(using: .utf8)!)
    }
    
    return event
}

// Keep the process running
RunLoop.current.run()

// Clean up (unreachable in normal operation)
if let monitor = eventMonitor {
    NSEvent.removeMonitor(monitor)
}
IOObjectRelease(iterator)
IONotificationPortDestroy(notifyPort)