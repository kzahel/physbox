import Matter from 'matter-js';
import { Camera } from './Camera';
import { InputManager } from '../interaction/InputManager';

export class Game {
    private engine: Matter.Engine;
    private render: Matter.Render;
    private canvasContainer: HTMLElement;
    private camera: Camera;
    private inputManager: InputManager;
    public wheelTorque: number = 0.1;
    public maxCarSpeed: number = 0.5;
    public groundFriction: number = 0.5;

    // New Global Settings
    public ballSize: number = 25;
    public ballElasticity: number = 0.9;
    public globalAirFriction: number = 0.01;
    private mouseConstraint: Matter.MouseConstraint;
    private readonly DEAD_ZONE_Y = 2000;

    // Custom Loop & Framerate
    private animationFrameId: number | null = null;
    private lastTime: number = 0;
    private targetFPS: number = 60;
    private frameInterval: number = 1000 / 60;
    private currentFPS: number = 0;
    private frameCount: number = 0;
    private lastFpsTime: number = 0;
    private isPaused: boolean = false;

    constructor(containerId: string) {
        this.canvasContainer = document.getElementById(containerId)!;

        // Create engine
        this.engine = Matter.Engine.create();

        // Create renderer
        // We don't use Matter.Render.run, so we just create it to hold config and context
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

        this.setupEvents();
    }

    private setupEvents() {
        Matter.Events.on(this.engine, 'beforeUpdate', () => {
            const world = this.engine.world;

            // 1. Check Composites (e.g. Cars)
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
            this.drawCustomOverlay();
        });
    }

    private drawCustomOverlay() {
        const context = this.render.context;
        const bounds = this.render.bounds;
        const scaleY = this.render.canvas.height / (bounds.max.y - bounds.min.y);
        const scaleX = this.render.canvas.width / (bounds.max.x - bounds.min.x);

        // Draw Ball Spokes
        const bodies = Matter.Composite.allBodies(this.engine.world);
        context.beginPath();
        bodies.forEach(body => {
            if (body.label === 'Ball') {
                const radius = 25;
                const endX = body.position.x + Math.cos(body.angle) * radius;
                const endY = body.position.y + Math.sin(body.angle) * radius;

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

        // Draw Dead Zone
        const screenY = (this.DEAD_ZONE_Y - bounds.min.y) * scaleY;
        if (screenY < this.render.canvas.height) {
            context.fillStyle = 'rgba(50, 50, 50, 0.8)';
            context.fillRect(0, screenY, this.render.canvas.width, this.render.canvas.height - screenY);

            context.beginPath();
            context.moveTo(0, screenY);
            context.lineTo(this.render.canvas.width, screenY);
            context.strokeStyle = '#FF0000';
            context.lineWidth = 2;
            context.stroke();
        }
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

    public setGlobalAirFriction(friction: number) {
        this.globalAirFriction = friction;
        const bodies = Matter.Composite.allBodies(this.engine.world);
        bodies.forEach(body => {
            body.frictionAir = friction;
        });
    }

    public setSimulationSpeed(speed: number) {
        this.engine.timing.timeScale = speed;
    }

    public setGravity(y: number) {
        this.engine.gravity.y = y;
    }

    public getSimulationSpeed(): number {
        return this.engine.timing.timeScale;
    }

    public getGravity(): number {
        return this.engine.gravity.y;
    }

    public start() {
        if (!this.animationFrameId) {
            this.lastTime = performance.now();
            this.lastFpsTime = this.lastTime;
            this.gameLoop(this.lastTime);
        }
    }

    public stop() {
        if (this.animationFrameId) {
            cancelAnimationFrame(this.animationFrameId);
            this.animationFrameId = null;
        }
    }

    public togglePause() {
        this.isPaused = !this.isPaused;
    }

    private gameLoop(time: number) {
        this.animationFrameId = requestAnimationFrame((t) => this.gameLoop(t));

        const deltaTime = time - this.lastTime;

        if (deltaTime >= this.frameInterval) {
            // Calculate FPS
            this.frameCount++;
            if (time - this.lastFpsTime >= 1000) {
                this.currentFPS = this.frameCount;
                this.frameCount = 0;
                this.lastFpsTime = time;
            }

            // Adjust lastTime to target framerate, but don't spiral if we lag
            this.lastTime = time - (deltaTime % this.frameInterval);

            if (!this.isPaused) {
                // Update physics
                // We use a fixed delta based on target framerate to ensure consistent simulation speed
                // If we lag, we still step by the same amount, effectively slowing down time
                Matter.Engine.update(this.engine, this.frameInterval);
            }

            // Render
            Matter.Render.world(this.render);
        }
    }

    public setTargetFPS(fps: number) {
        this.targetFPS = fps;
        this.frameInterval = 1000 / fps;
    }

    public getTargetFPS(): number {
        return this.targetFPS;
    }

    public getCurrentFPS(): number {
        return this.currentFPS;
    }

    public getObjectCount(): number {
        // Count dynamic bodies (not static)
        return this.engine.world.bodies.filter(b => !b.isStatic).length +
            this.engine.world.composites.reduce((acc, comp) => acc + Matter.Composite.allBodies(comp).length, 0);
    }

    public async detectRefreshRate(): Promise<number> {
        return new Promise((resolve) => {
            let lastTime = performance.now();
            let frames = 0;
            const deltas: number[] = [];

            const loop = (time: number) => {
                const delta = time - lastTime;
                lastTime = time;

                // Skip the first couple of frames as they might be unstable
                if (frames > 2) {
                    deltas.push(delta);
                }
                frames++;

                // Collect enough frames to find a stable minimum delta
                // 30 frames is usually enough to find the vsync interval
                if (frames < 30) {
                    requestAnimationFrame(loop);
                } else {
                    // Analyze deltas to find the refresh rate
                    // We look at the fastest frames because lag spikes (long frames) 
                    // shouldn't lower the detected refresh rate capability.
                    deltas.sort((a, b) => a - b);

                    // Take the median of the fastest 5 frames to avoid outliers
                    const fastDeltas = deltas.slice(0, 5);
                    const avgFastDelta = fastDeltas.reduce((a, b) => a + b, 0) / fastDeltas.length;

                    // Convert to FPS
                    const fps = 1000 / avgFastDelta;
                    resolve(fps);
                }
            };
            requestAnimationFrame(loop);
        });
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

    public reset() {
        Matter.Composite.clear(this.engine.world, false);
        Matter.Engine.clear(this.engine);

        // Re-add mouse constraint
        Matter.Composite.add(this.engine.world, this.mouseConstraint);

        // Re-create initial objects
        const width = this.render.options.width!;
        const height = this.render.options.height!;

        const ground = Matter.Bodies.rectangle(width / 2, height - 20, width, 40, {
            isStatic: true,
            render: { fillStyle: '#333' },
            label: 'Ground',
            friction: this.groundFriction
        });

        const box = Matter.Bodies.rectangle(width / 2, 200, 80, 80, {
            render: { fillStyle: '#F35' },
            label: 'Box'
        });

        Matter.Composite.add(this.engine.world, [ground, box]);
    }
}
