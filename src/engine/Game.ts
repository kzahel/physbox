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
    public groundFriction: number = 0.5;
    private mouseConstraint: Matter.MouseConstraint;

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
            const bodies = Matter.Composite.allBodies(this.engine.world);
            bodies.forEach(body => {
                const direction = body.plugin.direction || 1;
                if (body.label === 'CarWheel') {
                    body.torque = this.wheelTorque * direction;
                } else if (body.label === 'Monster') {
                    body.torque = 0.2 * direction;
                }
            });
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
