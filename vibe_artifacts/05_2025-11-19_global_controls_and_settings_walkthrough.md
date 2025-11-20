# Walkthrough - Global Settings Pane

I have implemented a new collapsible **Settings Pane** and enhanced the global controls for the PhysBox simulation.

## Changes

### UI Enhancements
- **Settings Pane**: A new collapsible panel in the top-right corner houses all simulation controls, reducing clutter in the main toolbar.
- **Toolbar Refactor**: The main toolbar now focuses on Mode selection (View, Create, Erase) and Object Templates.

### New Global Controls
The Settings Pane includes the following controls:
- **Simulation**:
    - **Sim Speed**: Adjust the time scale of the physics engine (0.1x to 3x).
    - **Gravity Y**: Control the vertical gravity force (-2 to 2).
    - **Air Friction**: Set global air resistance (0 to 0.5).
    - **Ground Friction**: Set friction for newly created walls.
- **New Balls**:
    - **Size**: Adjust the radius of newly created balls (5 to 100).
    - **Elasticity**: Adjust the bounciness of newly created balls (0 to 1.5).
- **Cars**:
    - **Torque**: Control car wheel power (including reverse).
    - **Max Speed**: Limit the maximum rotation speed of car wheels.

### Code Improvements
- **InputManager Refactor**: Refactored interaction modes to `view`, `create`, and `erase` for clearer logic and better extensibility.
- **Lint Fixes**: Resolved duplicate property declarations and import issues.

## Verification Results

### Automated Build
- `npm run build` completed successfully.

### Manual Verification Steps
1.  **Open the App**: Verify the new "Global Settings" pane appears in the top-right.
2.  **Toggle Settings**: Click the arrow button to collapse and expand the settings pane.
3.  **Test Simulation Controls**:
    - Change **Sim Speed** and observe objects moving faster/slower.
    - Change **Gravity Y** to negative and watch objects float up.
    - Increase **Air Friction** and observe objects slowing down in mid-air.
4.  **Test Ball Settings**:
    - Increase **Ball Size** and create a new ball. Verify it is larger.
    - Increase **Elasticity** and create a new ball. Drop it and verify it bounces higher.
5.  **Test Car Controls**:
    - Create a Car.
    - Adjust **Torque** and verify the car accelerates/reverses.
    - Adjust **Max Speed** and verify the car's top speed changes.
6.  **Test Eraser**:
    - Select "Eraser" from the mode dropdown.
    - Drag to erase objects.
    - Tap to erase a single object.

## Screenshots
*(Placeholder for screenshots if available)*
