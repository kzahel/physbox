# Phase 2: Drawing, Preview, and Advanced Interaction
**Created**: 2025-11-19T15:43:21Z

**Objective**: Enhance the sandbox with creative tools and more complex entities.

## Features Implemented
- **Freehand Drawing**: Added "Draw Wall" tool to create static walls by dragging.
- **Object Preview**: Added semi-transparent ghosts of objects before placement.
- **Advanced Interaction**:
    - **Scaling**: Mouse wheel while dragging scales objects.
    - **Rotation**: Right-click on specific objects (Cars, Monsters) to toggle rotation/direction.
- **New Templates**:
    - `Monster`: A blob-like soft body entity with tentacles.
    - `Car`: A composite entity with a chassis and wheels.

## Technical Details
- Enhanced `InputManager` to handle different modes (`create`, `view`, `erase`).
- implemented `Templates` interface for extensible object creation.
