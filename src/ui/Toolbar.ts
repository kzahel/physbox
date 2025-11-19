import { Game } from '../engine/Game';
import { Templates } from '../entities/Templates';

export class Toolbar {
    private container: HTMLElement;
    private game: Game;

    constructor(containerId: string, game: Game) {
        this.container = document.getElementById(containerId)!;
        this.game = game;
        this.render();
    }

    private render() {
        const toolbar = document.createElement('div');
        toolbar.className = 'toolbar ui-element';

        // Controls Group
        const controlsGroup = document.createElement('div');
        controlsGroup.className = 'toolbar-group';

        const playPauseBtn = document.createElement('button');
        playPauseBtn.textContent = 'Pause';
        playPauseBtn.onclick = () => {
            this.game.togglePause();
            playPauseBtn.textContent = playPauseBtn.textContent === 'Pause' ? 'Play' : 'Pause';
        };
        controlsGroup.appendChild(playPauseBtn);

        const modeSelect = document.createElement('select');
        modeSelect.innerHTML = `
      <option value="play">Play Mode</option>
      <option value="edit">Edit Mode</option>
    `;
        modeSelect.onchange = (e) => {
            const mode = (e.target as HTMLSelectElement).value as 'play' | 'edit';
            this.game.getInputManager().setMode(mode);
            templateGroup.style.display = mode === 'edit' ? 'flex' : 'none';
        };
        controlsGroup.appendChild(modeSelect);

        toolbar.appendChild(controlsGroup);

        // Templates Group
        const templateGroup = document.createElement('div');
        templateGroup.className = 'toolbar-group templates';
        templateGroup.style.display = 'none'; // Hidden by default (Play mode)

        Templates.forEach(template => {
            const btn = document.createElement('button');
            btn.textContent = template.name;
            btn.onclick = () => {
                this.game.getInputManager().setTemplate(template);
                // Highlight selected
                Array.from(templateGroup.children).forEach(c => c.classList.remove('active'));
                btn.classList.add('active');
            };
            templateGroup.appendChild(btn);
        });

        // Select first template by default
        if (Templates.length > 0) {
            this.game.getInputManager().setTemplate(Templates[0]);
            if (templateGroup.firstElementChild) {
                templateGroup.firstElementChild.classList.add('active');
            }
        }

        // Settings Group
        const settingsGroup = document.createElement('div');
        settingsGroup.className = 'toolbar-group settings';
        settingsGroup.style.display = 'flex';
        settingsGroup.style.flexDirection = 'column';
        settingsGroup.style.gap = '5px';

        // Wheel Speed Slider
        const wheelSpeedContainer = document.createElement('div');
        const wheelSpeedLabel = document.createElement('label');
        wheelSpeedLabel.textContent = 'Wheel Speed: ' + this.game.wheelTorque;
        const wheelSpeedSlider = document.createElement('input');
        wheelSpeedSlider.type = 'range';
        wheelSpeedSlider.min = '0';
        wheelSpeedSlider.max = '1';
        wheelSpeedSlider.step = '0.05';
        wheelSpeedSlider.value = this.game.wheelTorque.toString();
        wheelSpeedSlider.oninput = (e) => {
            const val = parseFloat((e.target as HTMLInputElement).value);
            this.game.wheelTorque = val;
            wheelSpeedLabel.textContent = 'Wheel Speed: ' + val;
        };
        wheelSpeedContainer.appendChild(wheelSpeedLabel);
        wheelSpeedContainer.appendChild(wheelSpeedSlider);
        settingsGroup.appendChild(wheelSpeedContainer);

        // Ground Friction Slider
        const frictionContainer = document.createElement('div');
        const frictionLabel = document.createElement('label');
        frictionLabel.textContent = 'Friction: ' + this.game.groundFriction;
        const frictionSlider = document.createElement('input');
        frictionSlider.type = 'range';
        frictionSlider.min = '0';
        frictionSlider.max = '1';
        frictionSlider.step = '0.1';
        frictionSlider.value = this.game.groundFriction.toString();
        frictionSlider.oninput = (e) => {
            const val = parseFloat((e.target as HTMLInputElement).value);
            this.game.setGroundFriction(val);
            frictionLabel.textContent = 'Friction: ' + val;
        };
        frictionContainer.appendChild(frictionLabel);
        frictionContainer.appendChild(frictionSlider);
        settingsGroup.appendChild(frictionContainer);

        toolbar.appendChild(settingsGroup);

        toolbar.appendChild(templateGroup);
        this.container.appendChild(toolbar);
    }
}
