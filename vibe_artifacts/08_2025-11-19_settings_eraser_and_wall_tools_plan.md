# Settings, Eraser, and Wall Tool Updates

## Goal Description
Address three specific user requests:
1.  **Global Settings**: Should be collapsed by default.
2.  **Eraser**: Should be able to erase static objects (walls, ground).
3.  **Wall Tool**: Implement a touch-friendly interaction (double-tap to start, tap to end) for drawing walls on mobile.

## User Review Required
- **Wall Tool Interaction**: The double-tap to start and tap to end is a specific interaction model requested by the user. I should confirm if this conflicts with existing double-tap gestures (like zoom, though usually that's double-tap-drag or pinch).

## Proposed Changes

## Proposed Changes

### Global Settings
#### [MODIFY] [SettingsPane.ts](file:///home/kgraehl/code/physbox/src/ui/SettingsPane.ts)
- Change `private isCollapsed: boolean = false;` to `true`.
- In `constructor`, initialize `this.content.style.display` to `'none'` if collapsed.
- Set the toggle button text to '▲' (or correct arrow) initially if collapsed.

### Eraser Tool
#### [MODIFY] [InputManager.ts](file:///home/kgraehl/code/physbox/src/interaction/InputManager.ts)
- In `eraseAt` method, remove or modify the check `if (body.label === 'Ground') return;`.
- Ensure it still ignores the preview body.

### Touch Wall Drawing
#### [MODIFY] [InputManager.ts](file:///home/kgraehl/code/physbox/src/interaction/InputManager.ts)
- Add `private lastTapTime: number = 0;` to track double taps.
- Add `private wallStartPoint: { x: number, y: number } | null = null;` to track the start of the wall segment.
- In `touchend`:
    - Detect double tap: `if (timeDiff < 300 && timeSinceLastTap < 300)` (simplified logic).
    - If double tap AND mode is 'create' AND template is 'Draw Wall':
        - Set `wallStartPoint` to current touch position (converted to world).
        - Set `isDrawing = true`.
        - Set `drawStartPos` to `wallStartPoint`.
        - Create `drawPreviewBody`.
    - If single tap AND `wallStartPoint` is set:
        - This is the end point.
        - Execute the wall creation logic (similar to `mouseup`).
        - Reset `wallStartPoint`, `isDrawing`, `drawStartPos`.
- In `touchmove`:
    - If `wallStartPoint` is set, update the preview body (similar to `mousemove` drawing logic).


## Verification Plan
### Manual Verification
- **Settings**: Reload app, verify pane is collapsed.
- **Eraser**: Create a wall, try to erase it. Try to erase the ground.
- **Wall Tool**:
    - On mobile (simulated): Double tap to start wall. Pan around. Tap elsewhere to finish wall.
    - Verify mouse interaction still works as before (drag to draw).
