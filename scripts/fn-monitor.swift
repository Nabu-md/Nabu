#!/usr/bin/env swift

/**
 * fn-monitor.swift
 *
 * macOS CGEventTap helper that detects fn key down/up events and arrow
 * navigation while fn is held. Requires Accessibility permissions.
 *
 * Outputs JSON lines to stdout:
 *   {"event":"fn-down"}         — fn key pressed
 *   {"event":"fn-up"}           — fn key released
 *   {"event":"nav-down"}        — fn+↓ (suppressed from system)
 *   {"event":"nav-up"}          — fn+↑
 *   {"event":"error","message":…}  — startup failure
 *
 * Build for production:
 *   swiftc -O -o fn-monitor fn-monitor.swift
 */

import Foundation
import CoreGraphics

// MARK: - Constants

let FN_KEY: CGKeyCode = 0x3F

// Arrow key codes
let KEY_UP: CGKeyCode    = 0x7E
let KEY_DOWN: CGKeyCode  = 0x7D
let KEY_LEFT: CGKeyCode  = 0x7B
let KEY_RIGHT: CGKeyCode = 0x7C

// MARK: - State

var fnPressed = false

// MARK: - Event Callback

func eventCallback(
  proxy: CGEventTapProxy,
  type: CGEventType,
  event: CGEvent,
  refcon: UnsafeMutableRawPointer?
) -> Unmanaged<CGEvent>? {
  let keyCode = event.getIntegerValueField(.keyboardEventKeycode)

  switch type {
  case .keyDown:
    if keyCode == FN_KEY {
      fnPressed = true
      print(#"{"event":"fn-down"}"#)
      fflush(stdout)
      return nil // swallow fn key
    }
    if fnPressed {
      switch keyCode {
      case KEY_UP:
        print(#"{"event":"nav-up"}"#)
        fflush(stdout)
        return nil
      case KEY_DOWN:
        print(#"{"event":"nav-down"}"#)
        fflush(stdout)
        return nil
      case KEY_LEFT:
        print(#"{"event":"nav-left"}"#)
        fflush(stdout)
        return nil
      case KEY_RIGHT:
        print(#"{"event":"nav-right"}"#)
        fflush(stdout)
        return nil
      default:
        break
      }
    }

  case .keyUp:
    if keyCode == FN_KEY {
      fnPressed = false
      print(#"{"event":"fn-up"}"#)
      fflush(stdout)
      return nil // swallow fn key
    }

  case .flagsChanged:
    // Key-up mask for fn key
    if !fnPressed && keyCode == FN_KEY {
      // Sometimes fn-up comes through flagsChanged
      print(#"{"event":"fn-up"}"#)
      fflush(stdout)
      return nil
    }

  default:
    break
  }

  return Unmanaged.passUnretained(event)
}

// MARK: - Setup Event Tap

let eventMask =
  (1 << CGEventType.keyDown.rawValue) |
  (1 << CGEventType.keyUp.rawValue) |
  (1 << CGEventType.flagsChanged.rawValue)

guard let tap = CGEvent.tapCreate(
  tap: .cgSessionEventTap,
  place: .headInsertEventTap,
  options: .defaultTap,
  eventsOfInterest: CGEventMask(eventMask),
  callback: eventCallback,
  userInfo: nil
) else {
  let msg = "Failed to create event tap. Grant Accessibility permissions in System Settings → Privacy & Security → Accessibility."
  print(#"{"event":"error","message":"\#(msg)"}"#)
  fflush(stdout)
  exit(1)
}

let runLoopSource = CFMachPortCreateRunLoopSource(kCFAllocatorDefault, tap, 0)
CFRunLoopAddSource(CFRunLoopGetCurrent(), runLoopSource, .commonModes)
CGEvent.tapEnable(tap: tap, enable: true)
CFRunLoopRun()
