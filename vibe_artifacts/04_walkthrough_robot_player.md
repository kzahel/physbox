# Verification: Robot Player

## Changes Made
- Added `Player` template to `Templates.ts` with a robot body, wheels, and cute face.
- Modified `InputManager.ts` to remove camera panning on Arrow keys and expose key states.
- Modified `Game.ts` to apply forces/velocities to the robot based on Arrow key input and render a jetpack visual.
- **Fixes**:
    - Added spokes to wheels for visual rotation.
    - Fixed constraint offsets to prevent wheels from snapping to the center.
    - Tuned movement speed.

## Verification Steps

### 1. Robot Placement
- [ ] Select "Player" from the toolbar.
- [ ] Click in the world to place a robot.
- [ ] **Verify**: The robot appears with a body, head, eyes, mouth, and wheels.
- [ ] **Verify**: Wheels are at the bottom corners, NOT snapped to the center.
- [ ] **Verify**: Wheels have a light-colored spoke (line) across them.

### 2. Movement Control
- [ ] Press **Left Arrow**: Robot should move left.
- [ ] **Verify**: Wheel spokes rotate counter-clockwise.
- [ ] Press **Right Arrow**: Robot should move right.
- [ ] **Verify**: Wheel spokes rotate clockwise.
- [ ] Release Left/Right: Robot wheels should stop spinning.

### 3. Jetpack & Gravity
- [ ] Press **Up Arrow**: Robot should fly upwards.
- [ ] **Visual Check**: A red square/spark should appear below the robot while Up is pressed.
- [ ] Press **Down Arrow**: Robot should be pushed downwards faster than gravity.

### 4. Stability & Camera
- [ ] Move the robot around. Verify it stays upright and does not tip over.
- [ ] Press Arrow keys. Verify the camera **does not** pan.
