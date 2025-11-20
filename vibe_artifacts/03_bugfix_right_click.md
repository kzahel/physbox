# Phase 3: Bugfix - Right-Click Placement
**Created**: 2025-11-20T07:34:45Z

**Objective**: Fix an issue where right-clicking was deleting blocks instead of performing the intended action (or doing nothing).

## Issues Addressed
- **Bug**: Right-click events were triggering the eraser or default context menu behaviors incorrectly in certain modes.

## Fixes
- Adjusted `InputManager` event listeners to properly handle `contextmenu` events.
- Ensured right-click is reserved for specific interactions (like rotating direction) or ignored in placement modes to prevent accidental deletions.
