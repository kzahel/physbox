# Implement Player-Controllable Robot
**Created**: 2025-11-20T15:46:22+01:00

## Goal Description
Add a new "player" item to the toolbar that places a robot character. The robot should be controllable via arrow keys (movement and jetpack), stay upright, and not rotate. Camera panning should be removed from arrow keys.

## User Review Required
- Confirming the visual design of the robot (taller than wide, two wheels).
- Confirming the control scheme (Arrow keys for robot, no camera panning on arrows).

## Proposed Changes

### Entities
#### [MODIFY] [Templates.ts](file:///home/kgraehl/code/physbox/src/entities/Templates.ts)
- Add a new template `Player`.
- Create a Composite containing:
    - Main Body: Rectangle (e.g., 40x80), `inertia: Infinity` (fixed rotation), label 'PlayerBody'.
    - Wheels: Two circles at the bottom, label 'PlayerWheel'.
    - Constraints: Axles connecting wheels to body.
    - **Visuals**: Add eyes and mouth to the main body render (using sub-parts or custom render logic if possible, or just simple shapes added to the composite).
- Ensure the robot is upright and taller than wide.

### UI
#### [MODIFY] [Toolbar.ts](file:///home/kgraehl/code/physbox/src/ui/Toolbar.ts)
- No changes needed as it iterates over `Templates`.

### Interaction
#### [MODIFY] [InputManager.ts](file:///home/kgraehl/code/physbox/src/interaction/InputManager.ts)
- Remove arrow key checks from `handleKeyboardPan`.
- Add `public isKeyDown(key: string): boolean` method to expose key state.

### Engine
#### [MODIFY] [Game.ts](file:///home/kgraehl/code/physbox/src/engine/Game.ts)
- In `setupEvents` -> `beforeUpdate`:
    - Iterate over `this.engine.world.composites`.
    - Identify Player composites (check if they contain a body with label 'PlayerBody').
    - Apply controls:
        - **Left/Right**: Directly set `angularVelocity` on 'PlayerWheel' bodies (constant speed).
        - **Up**: Apply upward force to 'PlayerBody'.
        - **Down**: Apply downward force to 'PlayerBody'.
- In `drawCustomOverlay` (or `afterRender`):
    - If "Up" key is pressed and player exists, draw a red square/spark below the robot to simulate jetpack.


## Verification Plan
### Manual Verification
- Launch the game.
- Select "Player" from toolbar.
- Place a robot.
- Use Arrow keys:
    - Left/Right should move the robot.
    - Up should fly.
    - Down should push down.
    - Camera should NOT pan.
- Verify robot stays upright.
