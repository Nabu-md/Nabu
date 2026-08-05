#!/usr/bin/env python3
"""Fix shortcut keys that got corrupted by shell encoding."""

import os

path = os.path.join("crates", "nabu-ui", "src", "components", "navigation", "shortcuts.rs")
with open(path, "r", encoding="utf-8") as f:
    content = f.read()

# The shortcuts were corrupted by shell encoding:
#   "⌘⇧C" became "C"
#   "⌘⇧R" became "R" (then changed to "⌘⇧1" which became "1")
#   "⌘⇧M" became "M"
#   "⌘⇧S" became "S"
# And the ⌘1 → ⌘3 shortcuts may have lost the ⌘ prefix too.

replacements = [
    # Fix shortcut reference entries
    ('Shortcut { category: "Navigation", keys: "1", description: "Open Reader Mode" },',
     'Shortcut { category: "Navigation", keys: "\u2318\u21e71", description: "Open Reader Mode" },'),
    ('Shortcut { category: "Navigation", keys: "C", description: "Open Canvas" },',
     'Shortcut { category: "Navigation", keys: "\u2318\u21e7C", description: "Open Canvas" },'),
    ('Shortcut { category: "Navigation", keys: "M", description: "Open Comparison View" },',
     'Shortcut { category: "Navigation", keys: "\u2318\u21e7M", description: "Open Comparison View" },'),
    ('Shortcut { category: "Navigation", keys: "S", description: "Open Statistics" },',
     'Shortcut { category: "Navigation", keys: "\u2318\u21e7S", description: "Open Statistics" },'),
]

for old, new in replacements:
    if old in content:
        content = content.replace(old, new, 1)
        print(f"Fixed: {old.strip()}")
    else:
        print(f"Not found: {old.strip()}")

# Also fix the NavBar shortcut reference entries if they were in a similar table
# (these entries may also be corrupted since they use the same ⌘ symbols)
# Fix any bare "1" in shortcut keys that should be ⌘1..⌘9
# Don't touch unrelated content - only shortcut key strings
import re

with open(path, "w", encoding="utf-8") as f:
    f.write(content)

# Verify the final state
with open(path, "r", encoding="utf-8") as f:
    lines = f.readlines()
for i, line in enumerate(lines):
    if "Canvas" in line or "Reader Mode" in line or "Comparison View" in line or "Statistics" in line:
