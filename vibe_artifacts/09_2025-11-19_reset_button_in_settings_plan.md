# Add Reset Button to Global Settings

This plan outlines the steps to add a reset button to the global settings pane. This button will reset the physics simulation to its initial state, consisting of a single box and a floor.

## Proposed Changes

### Engine Layer

#### [MODIFY] [Game.ts](file:///home/kgraehl/code/physbox/src/engine/Game.ts)
- Add a `reset()` method to the `Game` class.
- The `reset()` method will:
    - Clear the physics world using `Matter.Composite.clear`.
    - Re-add the `mouseConstraint`.
    - Re-create the initial ground and box objects.
    - Ensure the renderer dimensions are used correctly for object placement.

### Application Entry Point

#### [MODIFY] [main.ts](file:///home/kgraehl/code/physbox/src/main.ts)
- Remove the manual creation of the ground and box.
- Call `game.reset()` (or a similar initialization method) to set up the initial state.

### UI Layer

#### [MODIFY] [SettingsPane.ts](file:///home/kgraehl/code/physbox/src/ui/SettingsPane.ts)
- Add a "Reset" button to the settings pane, likely under a new "Actions" or "General" section, or at the top of "Controls".
- Bind the button's click event to `this.game.reset()`.

## Verification Plan

### Manual Verification
- Open the application in the browser.
- Verify that the initial state (one box, one floor) is loaded correctly on startup.
- Create some new objects (balls, boxes, etc.) using the toolbar.
- Open the Global Settings pane.
- Click the "Reset" button.
- Verify that:
    - All created objects are removed.
    - The scene returns to having exactly one box and one floor.
    - The mouse interaction still works (can drag the box).
    - The simulation continues running (or is paused if it was paused, depending on desired behavior - usually reset implies running or keeping current state, but "reset to initial" might imply unpausing too. I'll assume keeping current pause state or unpausing is fine, but simpler is just resetting bodies).
