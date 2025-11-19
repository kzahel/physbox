import { Game } from '../engine/Game';

export class SettingsPane {
    private container: HTMLElement;
    private content: HTMLElement;
    private game: Game;
    private isCollapsed: boolean = true;

    constructor(game: Game) {
        this.game = game;

        // Create main container
        this.container = document.createElement('div');
        this.container.style.position = 'absolute';
        this.container.style.top = '10px';
        this.container.style.right = '10px';
        this.container.style.width = '250px';
        this.container.style.backgroundColor = 'rgba(30, 30, 30, 0.9)';
        this.container.style.color = '#fff';
        this.container.style.padding = '10px';
        this.container.style.borderRadius = '8px';
        this.container.style.fontFamily = 'Arial, sans-serif';
        this.container.style.zIndex = '1000';
        this.container.style.maxHeight = '90vh';
        this.container.style.overflowY = 'auto';

        // Header with toggle
        const header = document.createElement('div');
        header.style.display = 'flex';
        header.style.justifyContent = 'space-between';
        header.style.alignItems = 'center';
        header.style.marginBottom = '10px';
        header.style.cursor = 'pointer';

        const title = document.createElement('h3');
        title.textContent = 'Global Settings';
        title.style.margin = '0';
        title.style.fontSize = '16px';

        const toggleBtn = document.createElement('button');
        toggleBtn.textContent = '▲';
        toggleBtn.style.background = 'none';
        toggleBtn.style.border = 'none';
        toggleBtn.style.color = '#fff';
        toggleBtn.style.cursor = 'pointer';

        header.appendChild(title);
        header.appendChild(toggleBtn);
        header.onclick = () => this.toggle();
        this.container.appendChild(header);

        // Content container
        this.content = document.createElement('div');
        this.content.style.display = 'none';
        this.content.style.flexDirection = 'column';
        this.content.style.gap = '10px';
        this.container.appendChild(this.content);

        document.body.appendChild(this.container);

        this.initControls();
    }

    private toggle() {
        this.isCollapsed = !this.isCollapsed;
        this.content.style.display = this.isCollapsed ? 'none' : 'flex';
        (this.container.querySelector('button') as HTMLElement).textContent = this.isCollapsed ? '▲' : '▼';
    }

    private addSlider(label: string, min: number, max: number, step: number, initial: number, onChange: (val: number) => void) {
        const wrapper = document.createElement('div');
        wrapper.style.display = 'flex';
        wrapper.style.flexDirection = 'column';

        const labelEl = document.createElement('label');
        labelEl.textContent = `${label}: ${initial}`;
        labelEl.style.fontSize = '12px';
        labelEl.style.marginBottom = '2px';

        const slider = document.createElement('input');
        slider.type = 'range';
        slider.min = min.toString();
        slider.max = max.toString();
        slider.step = step.toString();
        slider.value = initial.toString();
        slider.style.width = '100%';

        slider.oninput = (e) => {
            const val = parseFloat((e.target as HTMLInputElement).value);
            labelEl.textContent = `${label}: ${val}`;
            onChange(val);
        };

        wrapper.appendChild(labelEl);
        wrapper.appendChild(slider);
        this.content.appendChild(wrapper);
    }

    private addSection(title: string) {
        const el = document.createElement('div');
        el.textContent = title;
        el.style.fontSize = '14px';
        el.style.fontWeight = 'bold';
        el.style.marginTop = '5px';
        el.style.borderBottom = '1px solid #555';
        el.style.paddingBottom = '2px';
        this.content.appendChild(el);
    }

    private initControls() {
        // Controls
        this.addSection('Controls');

        const resetBtn = document.createElement('button');
        resetBtn.textContent = 'Reset World';
        resetBtn.style.width = '100%';
        resetBtn.style.padding = '5px';
        resetBtn.style.marginBottom = '5px';
        resetBtn.style.cursor = 'pointer';
        resetBtn.style.backgroundColor = '#d32f2f'; // Red color for destructive action
        resetBtn.style.color = 'white';
        resetBtn.style.border = 'none';
        resetBtn.style.borderRadius = '4px';
        resetBtn.onclick = () => {
            if (confirm('Are you sure you want to reset the world?')) {
                this.game.reset();
            }
        };
        this.content.appendChild(resetBtn);

        const pauseBtn = document.createElement('button');
        pauseBtn.textContent = 'Pause';
        pauseBtn.style.width = '100%';
        pauseBtn.style.padding = '5px';
        pauseBtn.style.marginBottom = '5px';
        pauseBtn.style.cursor = 'pointer';
        pauseBtn.onclick = () => {
            this.game.togglePause();
            pauseBtn.textContent = pauseBtn.textContent === 'Pause' ? 'Play' : 'Pause';
        };
        this.content.appendChild(pauseBtn);

        // Simulation Settings
        this.addSection('Simulation');
        this.addSlider('Sim Speed', 0.1, 3, 0.1, this.game.getSimulationSpeed(), (v) => this.game.setSimulationSpeed(v));
        this.addSlider('Gravity Y', -2, 2, 0.1, this.game.getGravity(), (v) => this.game.setGravity(v));
        this.addSlider('Air Friction', 0, 0.5, 0.01, this.game.globalAirFriction, (v) => this.game.setGlobalAirFriction(v));
        this.addSlider('Ground Friction', 0, 1, 0.1, this.game.groundFriction, (v) => this.game.setGroundFriction(v));

        // Ball Settings
        this.addSection('New Balls');
        this.addSlider('Size', 5, 100, 5, this.game.ballSize, (v) => this.game.ballSize = v);
        this.addSlider('Elasticity', 0, 1.5, 0.1, this.game.ballElasticity, (v) => this.game.ballElasticity = v);

        // Car Settings
        this.addSection('Cars');
        this.addSlider('Torque', -1, 1, 0.05, this.game.wheelTorque, (v) => this.game.wheelTorque = v);
        this.addSlider('Max Speed', 0, 5, 0.1, this.game.maxCarSpeed, (v) => this.game.maxCarSpeed = v);

        // Framerate Settings
        this.addSection('Performance');
        this.initFramerateControl();

        // Stats
        this.addSection('Stats');
        this.initStatsDisplay();
    }

    private async initFramerateControl() {
        const wrapper = document.createElement('div');
        wrapper.style.display = 'flex';
        wrapper.style.justifyContent = 'space-between';
        wrapper.style.alignItems = 'center';
        wrapper.style.marginBottom = '5px';

        const label = document.createElement('label');
        label.textContent = 'Max FPS:';
        label.style.fontSize = '12px';

        const select = document.createElement('select');
        select.style.background = '#333';
        select.style.color = '#fff';
        select.style.border = '1px solid #555';
        select.style.borderRadius = '4px';
        select.style.padding = '2px';

        wrapper.appendChild(label);
        wrapper.appendChild(select);
        this.content.appendChild(wrapper);

        // Detect refresh rate
        const refreshRate = await this.game.detectRefreshRate();
        // Round to nearest 10 to handle slight variances (e.g. 59.9 -> 60)
        const roundedRate = Math.round(refreshRate / 10) * 10;

        const options = [30, 60, 120, 240];
        const validOptions = options.filter(opt => opt <= roundedRate);
        // Ensure at least 60 is there if screen is 60, or just use what we found
        if (validOptions.length === 0) validOptions.push(60);

        validOptions.forEach(opt => {
            const option = document.createElement('option');
            option.value = opt.toString();
            option.textContent = opt.toString();
            select.appendChild(option);
        });

        // Set default to 60 or max available if less
        const defaultFPS = 60;
        if (validOptions.includes(defaultFPS)) {
            select.value = defaultFPS.toString();
            this.game.setTargetFPS(defaultFPS);
        } else {
            select.value = validOptions[validOptions.length - 1].toString();
            this.game.setTargetFPS(validOptions[validOptions.length - 1]);
        }

        select.onchange = () => {
            this.game.setTargetFPS(parseInt(select.value));
        };
    }

    private initStatsDisplay() {
        const fpsEl = document.createElement('div');
        fpsEl.style.fontSize = '12px';
        fpsEl.style.color = '#aaa';
        this.content.appendChild(fpsEl);

        const objEl = document.createElement('div');
        objEl.style.fontSize = '12px';
        objEl.style.color = '#aaa';
        this.content.appendChild(objEl);

        setInterval(() => {
            if (!this.isCollapsed) {
                fpsEl.textContent = `FPS: ${this.game.getCurrentFPS()}`;
                objEl.textContent = `Dynamic Objects: ${this.game.getObjectCount()}`;
            }
        }, 500);
    }
}
