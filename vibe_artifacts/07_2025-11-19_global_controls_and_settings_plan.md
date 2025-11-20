# Global Settings Pane & New Controls

## Goal Description
Reorganize the UI by moving global controls to a collapsible side pane and adding new simulation parameters.

## User Review Required
> [!NOTE]
> - **UI Change**: A new collapsible "Settings" pane will appear on the right (or left) side.
> - **New Controls**:
>     - **Ball Size**: Controls the radius of newly created balls.
>     - **Ball Elasticity**: Controls the bounciness of newly created balls.
>     - **Simulation Speed**: Controls the time scale of the physics engine.
>     - **Gravity**: Controls the global gravity Y-component.
>     - **Air Friction**: Controls the air resistance for all bodies (updates existing and new).

## Proposed Changes

### UI

#### [NEW] [src/ui/SettingsPane.ts](file:///home/kgraehl/code/physbox/src/ui/SettingsPane.ts)
- Create a class `SettingsPane` that manages a collapsible HTML container.
- Implement methods to add sliders/controls.
- Move logic for "Car Torque", "Car Max Speed", "Friction" here.
- Add new sliders for Ball Size, Elasticity, Sim Speed, Gravity, Air Friction.

#### [MODIFY] [src/ui/Toolbar.ts](file:///home/kgraehl/code/physbox/src/ui/Toolbar.ts)
- Remove existing settings sliders.
- Instantiate `SettingsPane`.

### Engine & Entities

#### [MODIFY] [src/engine/Game.ts](file:///home/kgraehl/code/physbox/src/engine/Game.ts)
- Add properties:
    - `ballSize` (default 25)
    - `ballElasticity` (default 0.9)
    - `globalAirFriction` (default 0.01)
- Update `setGroundFriction` to maybe be more generic or just keep as is.
- Add methods/setters for Sim Speed (`engine.timing.timeScale`) and Gravity (`engine.gravity.y`).
- Add method `setGlobalAirFriction(val)`: updates `this.globalAirFriction` and iterates all bodies to update `frictionAir`.

#### [MODIFY] [src/entities/Templates.ts](file:///home/kgraehl/code/physbox/src/entities/Templates.ts)
- Update `create` signature: `(x: number, y: number, game: Game) => ...`
- Update 'Ball' template to use `game.ballSize` and `game.ballElasticity`.
- Update 'Car' and others to use `game.globalAirFriction` if needed (or just rely on default).

#### [MODIFY] [src/interaction/InputManager.ts](file:///home/kgraehl/code/physbox/src/interaction/InputManager.ts)
- Update `placeCurrentTemplate` to pass `this.game` to `create`.

## Verification Plan

### Manual Verification
- **UI**: Verify pane collapses/expands.
- **Ball Settings**: Change size/elasticity, spawn ball. Verify changes.
- **Physics Settings**:
    - Change Gravity: Objects should fall faster/slower or float up.
    - Change Sim Speed: Everything moves faster/slower.
    - Change Air Friction: Objects should slow down more/less in air.
