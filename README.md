# PhysBox

A physics sandbox built with Matter.js.

## Controls

### Mouse
- **Left Click**: Place item (in Edit Mode) or interact with objects (in Play Mode).
- **Right Click**:
    - **Play Mode**: Drag to pan camera.
    - **Edit Mode**: Rotate special objects (Car Wheel, Monster).
- **Middle Click**: Pan camera.
- **Scroll Wheel**: Zoom in/out (Play Mode) or scale object (if dragging).

### Touch
- **One Finger Drag (Background)**: Pan camera.
- **Two Finger Pinch**: Zoom in/out.
- **Tap**: Place item (in Edit Mode).
- **Drag (Eraser)**: Erase items continuously.

### Keyboard
- **Arrow Keys**: Pan camera.

## Tools
- **View / Drag**: Pan the camera (drag background) or interact with objects (drag objects).
- **Create**: Select an object template to place it.
    - **Box**: A simple dynamic box.
    - **Ball**: A dynamic circle. Size and elasticity can be configured in Settings.
    - **Car**: A composite object with a chassis and two wheels.
    - **Monster**: A complex composite object.
    - **Draw Wall**: Click and drag to create a static wall.
- **Eraser**: Drag to erase objects in a 50x50 area. Tap to erase a single spot.

## Global Settings
A collapsible settings pane in the top-right corner allows you to control:
- **Simulation**:
    - **Sim Speed**: Time scale of the physics engine.
    - **Gravity Y**: Global gravity force.
    - **Air Friction**: Air resistance for all bodies.
    - **Ground Friction**: Friction for newly created walls.
- **New Balls**:
    - **Size**: Radius of new balls.
    - **Elasticity**: Bounciness (restitution) of new balls.
- **Cars**:
    - **Torque**: Wheel torque (power). Negative values for reverse.
    - **Max Speed**: Maximum rotation speed of wheels.
