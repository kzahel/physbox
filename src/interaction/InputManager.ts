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

    constructor(element: HTMLElement, camera: Camera, game: Game) {
        this.element = element;
        this.camera = camera;
        this.game = game;
        this.setupEvents();
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
                const dx = e.clientX - this.lastMousePos.x;
                const dy = e.clientY - this.lastMousePos.y;
                this.camera.pan(dx, dy);
                this.lastMousePos = { x: e.clientX, y: e.clientY };
            }

            // Preview logic
            if (this.mode === 'edit' && this.currentTemplate && !this.isDrawing && !this.currentTemplate.isTool) {
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
}
