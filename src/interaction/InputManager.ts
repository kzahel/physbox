import Matter from 'matter-js';
import { Camera } from '../engine/Camera';
import { Game } from '../engine/Game';
import type { EntityTemplate } from '../entities/Templates';

export type InteractionMode = 'play' | 'edit';

export class InputManager {
    private element: HTMLElement;
    private camera: Camera;
    private game: Game;
    private isDragging: boolean = false;
    private lastMousePos: { x: number, y: number } = { x: 0, y: 0 };

    private mode: InteractionMode = 'play';
    private currentTemplate: EntityTemplate | null = null;

    private previewBody: Matter.Body | Matter.Composite | null = null;
    private isDrawing: boolean = false;
    private drawStartPos: { x: number, y: number } | null = null;
    private drawPreviewBody: Matter.Body | null = null;

    private rapidFireInterval: number | null = null;
    private currentMousePos: { x: number, y: number } = { x: 0, y: 0 };

    private touchStartPos: { x: number, y: number } | null = null;
    private lastTouchPos: { x: number, y: number } | null = null;
    private initialPinchDistance: number | null = null;
    private isPinching: boolean = false;
    private touchStartTime: number = 0;
    private keysPressed: { [key: string]: boolean } = {};

    constructor(element: HTMLElement, camera: Camera, game: Game) {
        this.element = element;
        this.camera = camera;
        this.game = game;
        this.setupEvents();
        this.setupKeyboardEvents();
        this.startGameLoop(); // For keyboard panning
    }

    private startGameLoop() {
        const update = () => {
            this.handleKeyboardPan();
            requestAnimationFrame(update);
        };
        requestAnimationFrame(update);
    }

    private handleKeyboardPan() {
        const panSpeed = 10; // Pixels per frame
        let dx = 0;
        let dy = 0;

        if (this.keysPressed['ArrowLeft']) dx -= panSpeed;
        if (this.keysPressed['ArrowRight']) dx += panSpeed;
        if (this.keysPressed['ArrowUp']) dy -= panSpeed;
        if (this.keysPressed['ArrowDown']) dy += panSpeed;

        if (dx !== 0 || dy !== 0) {
            this.camera.pan(dx, dy);
        }
    }

    public setMode(mode: InteractionMode) {
        this.mode = mode;
        // Update mouse constraint visibility based on mode?
        // Actually, in edit mode we might want to disable mouse constraint so we don't accidentally drag things while trying to place?
        // Or just keep it.
        const mouseConstraint = this.game.getEngine().world.constraints.find(c => c.label === 'Mouse Constraint');
        if (mouseConstraint) {
            // We can't easily disable it without removing it, but we can change collision filter or something.
            // For now, let's just keep it.
        }
        this.clearPreview();
        this.isDrawing = false;
        if (this.drawPreviewBody) {
            Matter.Composite.remove(this.game.getEngine().world, this.drawPreviewBody);
            this.drawPreviewBody = null;
        }
        this.stopRapidFire();
    }

    public setTemplate(template: EntityTemplate) {
        this.currentTemplate = template;
        this.clearPreview();
        this.stopRapidFire();
    }

    private clearPreview() {
        if (this.previewBody) {
            Matter.Composite.remove(this.game.getEngine().world, this.previewBody);
            this.previewBody = null;
        }
    }

    private stopRapidFire() {
        if (this.rapidFireInterval !== null) {
            window.clearInterval(this.rapidFireInterval);
            this.rapidFireInterval = null;
        }
    }

    private placeCurrentTemplate(x: number, y: number) {
        if (!this.currentTemplate || this.currentTemplate.isTool) return;

        const worldPos = this.camera.screenToWorld(x, y);
        const body = this.currentTemplate.create(worldPos.x, worldPos.y);
        Matter.Composite.add(this.game.getEngine().world, body);
    }

    private setupKeyboardEvents() {
        window.addEventListener('keydown', (e) => {
            this.keysPressed[e.key] = true;
        });

        window.addEventListener('keyup', (e) => {
            this.keysPressed[e.key] = false;
        });
    }

    private setupEvents() {
        this.element.addEventListener('wheel', (e) => {
            e.preventDefault();

            const mouseConstraint = this.game.getMouseConstraint();
            if (mouseConstraint.body) {
                const scaleFactor = e.deltaY > 0 ? 0.9 : 1.1;
                Matter.Body.scale(mouseConstraint.body, scaleFactor, scaleFactor);
            } else {
                const zoomFactor = e.deltaY > 0 ? 1.1 : 0.9;
                this.camera.zoom(zoomFactor, e.clientX, e.clientY);
            }
        });

        this.element.addEventListener('mousedown', (e) => {
            if (e.button === 1 || (e.button === 2 && this.mode === 'play')) { // Middle or Right click (in play mode)
                this.isDragging = true;
                this.lastMousePos = { x: e.clientX, y: e.clientY };
                e.preventDefault();
            } else if (e.button === 0) { // Left click
                if (this.mode === 'edit' && this.currentTemplate) {
                    if (this.currentTemplate.isTool && this.currentTemplate.name === 'Draw Wall') {
                        const worldPos = this.camera.screenToWorld(e.clientX, e.clientY);
                        this.isDrawing = true;
                        this.drawStartPos = worldPos;
                    } else if (this.currentTemplate.isTool && this.currentTemplate.name === 'Eraser') {
                        this.isDragging = true; // Use dragging flag for continuous erasing
                        this.eraseAt(e.clientX, e.clientY);
                    } else if (!this.currentTemplate.isTool) {
                        if (e.shiftKey) {
                            this.placeCurrentTemplate(e.clientX, e.clientY);
                            this.rapidFireInterval = window.setInterval(() => {
                                this.placeCurrentTemplate(this.currentMousePos.x, this.currentMousePos.y);
                            }, 100);
                        } else {
                            this.placeCurrentTemplate(e.clientX, e.clientY);
                        }
                    }
                }
            } else if (e.button === 2 && this.mode === 'edit') { // Right click in edit mode
                // Rotation toggle logic handled in contextmenu or here?
                // contextmenu event fires after mousedown, usually.
                // Let's handle it here to be responsive.
                const worldPos = this.camera.screenToWorld(e.clientX, e.clientY);
                const bodies = Matter.Composite.allBodies(this.game.getEngine().world);
                const clickedBodies = Matter.Query.point(bodies, worldPos);

                clickedBodies.forEach(body => {
                    if (body.label === 'CarWheel' || body.label === 'Monster') {
                        body.plugin.direction = (body.plugin.direction || 1) * -1;
                    }
                });
            }
        });

        window.addEventListener('mousemove', (e) => {
            this.currentMousePos = { x: e.clientX, y: e.clientY };
            const worldPos = this.camera.screenToWorld(e.clientX, e.clientY);

            if (this.isDragging) {
                if (this.mode === 'edit' && this.currentTemplate?.name === 'Eraser') {
                    this.eraseAt(e.clientX, e.clientY);
                } else {
                    const dx = e.clientX - this.lastMousePos.x;
                    const dy = e.clientY - this.lastMousePos.y;
                    this.camera.pan(dx, dy);
                    this.lastMousePos = { x: e.clientX, y: e.clientY };
                }
            }

            // Preview logic
            if (this.mode === 'edit' && this.currentTemplate && !this.isDrawing) {
                if (this.currentTemplate.name === 'Eraser') {
                    if (!this.previewBody) {
                        this.previewBody = Matter.Bodies.rectangle(worldPos.x, worldPos.y, 50, 50, {
                            isStatic: true,
                            isSensor: true,
                            render: { fillStyle: '#FF0000', opacity: 0.3 },
                            collisionFilter: { group: -1, category: 0, mask: 0 }
                        });
                        Matter.Composite.add(this.game.getEngine().world, this.previewBody);
                    } else {
                        Matter.Body.setPosition(this.previewBody as Matter.Body, worldPos);
                    }
                } else if (!this.currentTemplate.isTool) {
                    if (!this.previewBody) {
                        this.previewBody = this.currentTemplate.create(worldPos.x, worldPos.y);
                        this.makeBodyPreview(this.previewBody);
                        Matter.Composite.add(this.game.getEngine().world, this.previewBody);
                    } else {
                        // Update position
                        // For composite, we need to translate. For body, setPosition.
                        // Simplest is to remove and recreate, but that's heavy.
                        // Let's try to move it.
                        if (this.previewBody.type === 'body') {
                            Matter.Body.setPosition(this.previewBody as Matter.Body, worldPos);
                        } else {
                            // Composite: calculate delta
                            // This is hard because we don't track center of composite easily.
                            // Recreating is safer for now to ensure correct relative positioning.
                            Matter.Composite.remove(this.game.getEngine().world, this.previewBody);
                            this.previewBody = this.currentTemplate.create(worldPos.x, worldPos.y);
                            this.makeBodyPreview(this.previewBody);
                            Matter.Composite.add(this.game.getEngine().world, this.previewBody);
                        }
                    }
                } else {
                    this.clearPreview();
                }
            } else {
                this.clearPreview();
            }

            // Drawing logic
            if (this.isDrawing && this.drawStartPos) {
                if (this.drawPreviewBody) {
                    Matter.Composite.remove(this.game.getEngine().world, this.drawPreviewBody);
                }

                const dx = worldPos.x - this.drawStartPos.x;
                const dy = worldPos.y - this.drawStartPos.y;
                const length = Math.sqrt(dx * dx + dy * dy);
                const angle = Math.atan2(dy, dx);
                const x = (worldPos.x + this.drawStartPos.x) / 2;
                const y = (worldPos.y + this.drawStartPos.y) / 2;

                // Ensure min size
                const w = Math.max(length, 10);
                const h = 20; // Fixed thickness for wall

                this.drawPreviewBody = Matter.Bodies.rectangle(x, y, w, h, {
                    isStatic: true,
                    angle: angle,
                    render: { fillStyle: '#888', opacity: 0.5 },
                    collisionFilter: { group: -1, category: 0, mask: 0 }
                });
                Matter.Composite.add(this.game.getEngine().world, this.drawPreviewBody);
            }
        });

        window.addEventListener('mouseup', (e) => {
            this.isDragging = false;
            this.stopRapidFire();

            if (this.isDrawing && this.drawStartPos) {
                const worldPos = this.camera.screenToWorld(e.clientX, e.clientY);

                // Create final wall
                const dx = worldPos.x - this.drawStartPos.x;
                const dy = worldPos.y - this.drawStartPos.y;
                const length = Math.sqrt(dx * dx + dy * dy);
                const angle = Math.atan2(dy, dx);
                const x = (worldPos.x + this.drawStartPos.x) / 2;
                const y = (worldPos.y + this.drawStartPos.y) / 2;

                if (length > 10) {
                    const wall = Matter.Bodies.rectangle(x, y, length, 20, {
                        isStatic: true,
                        angle: angle,
                        render: { fillStyle: '#888' },
                        label: 'Ground'
                    });
                    // Apply current friction
                    wall.friction = this.game.groundFriction;

                    Matter.Composite.add(this.game.getEngine().world, wall);
                }

                // Cleanup
                if (this.drawPreviewBody) {
                    Matter.Composite.remove(this.game.getEngine().world, this.drawPreviewBody);
                    this.drawPreviewBody = null;
                }
                this.isDrawing = false;
                this.drawStartPos = null;
            }
        });

        this.element.addEventListener('contextmenu', (e) => e.preventDefault());

        // Touch Events
        this.element.addEventListener('touchstart', (e) => {
            e.preventDefault(); // Prevent default touch actions like scrolling

            if (e.touches.length === 1) {
                const touch = e.touches[0];
                this.touchStartPos = { x: touch.clientX, y: touch.clientY };
                this.lastTouchPos = { x: touch.clientX, y: touch.clientY };
                this.touchStartTime = Date.now();
                this.isPinching = false;

                // Check if touching a body (for potential interaction, though Matter.Mouse usually handles this if we pass events correctly,
                // but Matter.Mouse doesn't support touch natively well without some help or using the MouseConstraint with touch events mapped).
                // Actually Matter.Mouse DOES support touch if configured, but we want custom background pan.

                // For now, let's assume if we are NOT touching a dynamic body, we pan.
                // But wait, Matter.MouseConstraint handles dragging bodies.
                // If we want to pan on background, we need to know if we hit something.

                // We can query the world at the touch point.
                const worldPos = this.camera.screenToWorld(touch.clientX, touch.clientY);
                const bodies = Matter.Composite.allBodies(this.game.getEngine().world);
                const clickedBodies = Matter.Query.point(bodies, worldPos);

                // Filter out static bodies (like walls) if we want to allow panning when touching walls? 
                // Usually we only drag dynamic bodies.
                const dynamicBodies = clickedBodies.filter(b => !b.isStatic);

                if (dynamicBodies.length === 0) {
                    this.isDragging = true; // Reuse isDragging for camera pan
                } else {
                    // Let Matter.MouseConstraint handle it?
                    // We might need to manually feed it or just let it be if it's hooked up to the element.
                    // Matter.Mouse attaches to element, so it receives events?
                    // Matter.Mouse listens to mouse events. For touch, we might need to simulate or it might handle it.
                    // Matter.js Mouse supports touch if the element is correct.
                    this.isDragging = false;
                }

            } else if (e.touches.length === 2) {
                this.isPinching = true;
                this.isDragging = false;
                const dx = e.touches[0].clientX - e.touches[1].clientX;
                const dy = e.touches[0].clientY - e.touches[1].clientY;
                this.initialPinchDistance = Math.sqrt(dx * dx + dy * dy);
            }
        }, { passive: false });

        this.element.addEventListener('touchmove', (e) => {
            e.preventDefault();

            if (this.isPinching && e.touches.length === 2) {
                const dx = e.touches[0].clientX - e.touches[1].clientX;
                const dy = e.touches[0].clientY - e.touches[1].clientY;
                const currentDistance = Math.sqrt(dx * dx + dy * dy);

                if (this.initialPinchDistance) {
                    // const scale = currentDistance / this.initialPinchDistance;
                    // Zoom based on scale change
                    // We need to apply relative zoom.
                    // If scale > 1, we are zooming in (magnifying).
                    // Camera.zoom(factor) multiplies scale.
                    // If we want 1:1 mapping, it's tricky because zoom is cumulative.
                    // Let's just apply a factor based on the ratio of change since last move?
                    // Or just reset initial distance every move?

                    // Resetting initial distance every move is smoother for continuous updates
                    // const zoomFactor = scale > 1 ? 1.02 : 0.98; // Small steps
                    // Or better: use the actual ratio
                    const ratio = currentDistance / this.initialPinchDistance;
                    // Limit ratio to avoid jumps
                    const safeRatio = Math.max(0.9, Math.min(ratio, 1.1));

                    // Invert for camera zoom? 
                    // Camera.zoom(amount): scale *= amount.
                    // If we pinch out (distance increases), we want to zoom in? Or out?
                    // Usually pinch out = zoom in (enlarge).
                    // If distance increases, ratio > 1.
                    // Camera.zoom(1.1) -> scale increases -> view gets smaller? 
                    // Let's check Camera.ts:
                    // zoom(amount) { this.scale *= amount; }
                    // updateBounds() { width = w * scale; }
                    // If scale increases, width increases -> we see MORE of the world -> Zoom OUT.
                    // So if we pinch out (ratio > 1), we want to Zoom IN -> scale should DECREASE.

                    const invertedRatio = 1 / safeRatio;

                    const centerX = (e.touches[0].clientX + e.touches[1].clientX) / 2;
                    const centerY = (e.touches[0].clientY + e.touches[1].clientY) / 2;

                    this.camera.zoom(invertedRatio, centerX, centerY);

                    this.initialPinchDistance = currentDistance;
                }
            } else if (this.isDragging && e.touches.length === 1) {
                const touch = e.touches[0];
                if (this.mode === 'edit' && this.currentTemplate?.name === 'Eraser') {
                    this.eraseAt(touch.clientX, touch.clientY);
                } else if (this.lastTouchPos) {
                    const dx = touch.clientX - this.lastTouchPos.x;
                    const dy = touch.clientY - this.lastTouchPos.y;
                    this.camera.pan(dx, dy);
                    this.lastTouchPos = { x: touch.clientX, y: touch.clientY };
                }
            }
        }, { passive: false });

        this.element.addEventListener('touchend', (e) => {
            // Check for tap
            if (!this.isPinching && this.touchStartPos && e.changedTouches.length > 0) {
                const touch = e.changedTouches[0];
                const dx = touch.clientX - this.touchStartPos.x;
                const dy = touch.clientY - this.touchStartPos.y;
                const dist = Math.sqrt(dx * dx + dy * dy);
                const timeDiff = Date.now() - this.touchStartTime;

                if (dist < 10 && timeDiff < 300) {
                    // It's a tap!
                    // Handle placement
                    if (this.mode === 'edit' && this.currentTemplate) {
                        if (this.currentTemplate.name === 'Eraser') {
                            this.eraseAt(touch.clientX, touch.clientY);
                        } else if (!this.currentTemplate.isTool) {
                            this.placeCurrentTemplate(touch.clientX, touch.clientY);
                        }
                    }
                }
            }

            if (e.touches.length === 0) {
                this.isDragging = false;
                this.isPinching = false;
                this.touchStartPos = null;
                this.lastTouchPos = null;
                this.initialPinchDistance = null;
            } else if (e.touches.length === 1) {
                // Switch back to panning if one finger remains?
                this.isPinching = false;
                this.isDragging = true; // Maybe? Or just stop.
                this.lastTouchPos = { x: e.touches[0].clientX, y: e.touches[0].clientY };
            }
        });
    }

    private makeBodyPreview(body: Matter.Body | Matter.Composite) {
        const setPreview = (b: Matter.Body) => {
            b.isSensor = true;
            b.isStatic = true; // Ensure it doesn't move
            b.render.opacity = 0.5;
            b.collisionFilter = { group: -1, category: 0, mask: 0 };
        };

        if (body.type === 'body') {
            setPreview(body as Matter.Body);
        } else {
            Matter.Composite.allBodies(body as Matter.Composite).forEach(setPreview);
        }
    }

    private eraseAt(x: number, y: number) {
        const worldPos = this.camera.screenToWorld(x, y);
        // Define eraser box size (same as Box template: 50x50)
        const eraserSize = 50;
        const bounds = {
            min: { x: worldPos.x - eraserSize / 2, y: worldPos.y - eraserSize / 2 },
            max: { x: worldPos.x + eraserSize / 2, y: worldPos.y + eraserSize / 2 }
        };

        const bodies = Matter.Composite.allBodies(this.game.getEngine().world);
        const bodiesToRemove = Matter.Query.region(bodies, bounds);

        bodiesToRemove.forEach(body => {
            // Don't erase the preview body itself!
            if (body === this.previewBody || body === this.drawPreviewBody) return;
            // Don't erase walls/ground? User said "remove any items".
            // Maybe keep ground?
            if (body.label === 'Ground') return;

            // If it's part of a composite (like a car), remove the whole composite?
            // Matter.js bodies don't strictly know their parent composite.
            // But we can try to remove the body.
            Matter.Composite.remove(this.game.getEngine().world, body);
        });
    }
}
