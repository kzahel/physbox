import Matter from 'matter-js';
import { Camera } from './Camera';
import { InputManager } from '../interaction/InputManager';

export class Game {
    private engine: Matter.Engine;
    private render: Matter.Render;
    private runner: Matter.Runner;
    private canvasContainer: HTMLElement;
    private camera: Camera;
    private inputManager: InputManager;
    public wheelTorque: number = 0.1;
    public maxCarSpeed: number = 0.5;
    public groundFriction: number = 0.5;
    private mouseConstraint: Matter.MouseConstraint;
    private readonly DEAD_ZONE_Y = 2000;

    constructor(containerId: string) {
        this.canvasContainer = document.getElementById(containerId)!;

        // Create engine
        this.engine = Matter.Engine.create();

        // Create renderer
        this.render = Matter.Render.create({
            element: this.canvasContainer,
            engine: this.engine,
            options: {
                width: this.canvasContainer.clientWidth,
                height: this.canvasContainer.clientHeight,
                background: '#1a1a1a',
                wireframes: false,
                showAngleIndicator: false,
                hasBounds: true
            }
        });

        // Create runner
        this.runner = Matter.Runner.create();

        // Handle window resize
        window.addEventListener('resize', () => this.handleResize());

        // Initial resize to ensure correct size
        this.handleResize();

        // Add mouse control
        const mouse = Matter.Mouse.create(this.render.canvas);
        this.mouseConstraint = Matter.MouseConstraint.create(this.engine, {
            mouse: mouse,
            constraint: {
                stiffness: 0.2,
                render: {
                    visible: false
                }
            }
        });
        Matter.Composite.add(this.engine.world, this.mouseConstraint);
        this.render.mouse = mouse;

        this.camera = new Camera(this.render);
        this.inputManager = new InputManager(this.canvasContainer, this.camera, this);

        Matter.Events.on(this.engine, 'beforeUpdate', () => {
            const world = this.engine.world;

            // 1. Check Composites (e.g. Cars)
            // We iterate backwards to safely remove
            for (let i = world.composites.length - 1; i >= 0; i--) {
                const composite = world.composites[i];
                const bodies = Matter.Composite.allBodies(composite);
                let isDead = false;
                for (const body of bodies) {
                    if (body.position.y > this.DEAD_ZONE_Y) {
                        isDead = true;
                        break;
                    }
                }
                if (isDead) {
                    Matter.Composite.remove(world, composite);
                } else {
                    // Update torque for cars if alive
                    bodies.forEach(body => {
                        const direction = body.plugin.direction || 1;
                        if (body.label === 'CarWheel') {
                            // Only apply torque if below max speed
                            if (Math.abs(body.angularVelocity) < this.maxCarSpeed) {
                                body.torque = this.wheelTorque * direction;
                            }
                        } else if (body.label === 'Monster') {
                            body.torque = 0.2 * direction;
                        }
                    });
                }
            }

            // 2. Check Direct Bodies (e.g. Boxes, Balls)
            for (let i = world.bodies.length - 1; i >= 0; i--) {
                const body = world.bodies[i];
                if (body.position.y > this.DEAD_ZONE_Y) {
                    Matter.Composite.remove(world, body);
                }
            }
        });

        Matter.Events.on(this.render, 'afterRender', () => {
            const context = this.render.context;
            const bounds = this.render.bounds;

            // Calculate screen position of the dead zone line
            const scaleY = this.render.canvas.height / (bounds.max.y - bounds.min.y);

            // Draw Ball Spokes
            const bodies = Matter.Composite.allBodies(this.engine.world);
            context.beginPath();
            bodies.forEach(body => {
                if (body.label === 'Ball') {
                    const radius = 25; // We know the radius from Templates
                    // Calculate end point of spoke
                    const endX = body.position.x + Math.cos(body.angle) * radius;
                    const endY = body.position.y + Math.sin(body.angle) * radius;

                    // Map to screen coordinates
                    // We need a reliable WorldToScreen function.
                    // Re-deriving scaleX/scaleY from bounds is safest.
                    const scaleX = this.render.canvas.width / (bounds.max.x - bounds.min.x);
                    // scaleY is already calculated above

                    const screenX = (body.position.x - bounds.min.x) * scaleX;
                    const screenY = (body.position.y - bounds.min.y) * scaleY;

                    const screenEndX = (endX - bounds.min.x) * scaleX;
                    const screenEndY = (endY - bounds.min.y) * scaleY;

                    context.moveTo(screenX, screenY);
                    context.lineTo(screenEndX, screenEndY);
                }
            });
            context.strokeStyle = 'black';
            context.lineWidth = 2;
            context.stroke();

            const screenY = (this.DEAD_ZONE_Y - bounds.min.y) * scaleY;

            if (screenY < this.render.canvas.height) {
                context.fillStyle = 'rgba(50, 50, 50, 0.8)'; // Dark grey
                context.fillRect(0, screenY, this.render.canvas.width, this.render.canvas.height - screenY);

                // Draw line
                context.beginPath();
                context.moveTo(0, screenY);
                context.lineTo(this.render.canvas.width, screenY);
                context.strokeStyle = '#FF0000';
                context.lineWidth = 2;
                context.stroke();
            }
        });
    }

    public setGroundFriction(friction: number) {
        this.groundFriction = friction;
        const bodies = Matter.Composite.allBodies(this.engine.world);
        bodies.forEach(body => {
            if (body.label === 'Ground') {
                body.friction = friction;
            }
        });
    }

    public start() {
        Matter.Render.run(this.render);
        Matter.Runner.run(this.runner, this.engine);
    }

    public stop() {
        Matter.Render.stop(this.render);
        Matter.Runner.stop(this.runner);
    }

    public togglePause() {
        this.runner.enabled = !this.runner.enabled;
    }

    private handleResize() {
        this.render.canvas.width = this.canvasContainer.clientWidth;
        this.render.canvas.height = this.canvasContainer.clientHeight;
    }

    public getEngine() {
        return this.engine;
    }

    public getRender() {
        return this.render;
    }

    public getCamera() {
        return this.camera;
    }

    public getInputManager() {
        return this.inputManager;
    }

    public getMouseConstraint() {
        return this.mouseConstraint;
    }
}
