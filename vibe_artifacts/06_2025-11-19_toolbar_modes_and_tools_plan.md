# Toolbar and Settings Reorganization

## Goal Description
Refactor the toolbar to support only "View" and "Edit" modes. In "View" mode, creation tools should be hidden. Move the pause button to the settings pane (side with sliders). Restore the missing "Wall" tool to the toolbar.

## Proposed Changes

### UI
#### [MODIFY] [Toolbar.ts](file:///home/kgraehl/code/physbox/src/ui/Toolbar.ts)
- Update `modeSelect` to have only 'View' and 'Edit' options.
- On mode change:
    - If 'View': set InputManager mode to 'view', hide `objectGroup`.
    - If 'Edit': set InputManager mode to 'create' (or restore previous), show `objectGroup`.
- Remove `playPauseBtn` from `Toolbar`.
- in `objectGroup` loop: Remove `if (template.isTool) return;` to allow Wall tool to be rendered.
- Ensure `objectGroup` visibility is toggled based on mode.

#### [MODIFY] [SettingsPane.ts](file:///home/kgraehl/code/physbox/src/ui/SettingsPane.ts)
- Add a Pause/Play button at the top of the settings pane (or in a "Controls" section).
- Implement the pause toggle logic (using `this.game.togglePause()`).

### Interaction
#### [MODIFY] [InputManager.ts](file:///home/kgraehl/code/physbox/src/interaction/InputManager.ts)
- Ensure the Wall tool is correctly registered and accessible.

## Verification Plan
### Manual Verification
- Open the app.
- Check for "View" and "Edit" mode toggle.
- Switch to "View" mode: verify creation tools are hidden.
- Switch to "Edit" mode: verify creation tools are visible.
- Verify "Wall" tool is present in "Edit" mode and works.
- Verify "Pause" button is in the Settings pane and works.
