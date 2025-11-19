import Matter from 'matter-js';

export class Camera {
    private render: Matter.Render;
    private scale: number = 1;
    private center: Matter.Vector;

    constructor(render: Matter.Render) {
        this.render = render;
        this.center = { x: render.options.width! / 2, y: render.options.height! / 2 };

        // Initial bounds setup
        this.updateBounds();
    }

    public zoom(amount: number, _x: number, _y: number) {
        this.scale *= amount;
        // Clamp scale
        this.scale = Math.max(0.1, Math.min(this.scale, 20));

        // TODO: Zoom towards mouse position (x, y)
        // For now just zoom center
        this.updateBounds();
    }

    public pan(dx: number, dy: number) {
        this.center.x -= dx * this.scale;
        this.center.y -= dy * this.scale;
        this.updateBounds();
    }

    private updateBounds() {
        const width = this.render.options.width! * this.scale;
        const height = this.render.options.height! * this.scale;

        this.render.bounds.min.x = this.center.x - width / 2;
        this.render.bounds.max.x = this.center.x + width / 2;
        this.render.bounds.min.y = this.center.y - height / 2;
        this.render.bounds.max.y = this.center.y + height / 2;

        // Update mouse offset for correct interaction
        if (this.render.mouse) {
            Matter.Mouse.setScale(this.render.mouse, { x: this.scale, y: this.scale });
            Matter.Mouse.setOffset(this.render.mouse, this.render.bounds.min);
        }
    }

    public screenToWorld(x: number, y: number): { x: number, y: number } {
        const bounds = this.render.bounds;


        // Calculate relative position in the canvas (0 to 1)
        // Note: render.bounds already accounts for the view (min/max)
        // But we need to map screen pixel (x,y) to world coordinate.

        // Simple mapping:
        // worldX = bounds.min.x + (x / width) * (bounds.max.x - bounds.min.x)

        // We need to use the actual canvas size, not the options width/height which might be initial size
        const canvasWidth = this.render.canvas.width;
        const canvasHeight = this.render.canvas.height;

        return {
            x: bounds.min.x + (x / canvasWidth) * (bounds.max.x - bounds.min.x),
            y: bounds.min.y + (y / canvasHeight) * (bounds.max.y - bounds.min.y)
        };
    }
}
