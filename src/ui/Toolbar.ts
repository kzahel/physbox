import { Game } from '../engine/Game';
import { Templates } from '../entities/Templates';
import { SettingsPane } from './SettingsPane';

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

        // Mode Selector
        const modeSelect = document.createElement('select');
        modeSelect.style.padding = '5px';
        modeSelect.style.borderRadius = '4px';

        const modes = [
            { value: 'edit', label: 'Edit' },
            { value: 'view', label: 'View' }
        ];

        modes.forEach(m => {
            const option = document.createElement('option');
            option.value = m.value;
            option.textContent = m.label;
            modeSelect.appendChild(option);
        });

        controlsGroup.appendChild(modeSelect);

        // Object Selectors
        const objectGroup = document.createElement('div');
        objectGroup.className = 'toolbar-group templates';
        objectGroup.style.display = 'flex'; // Visible by default (Edit mode)
        objectGroup.style.gap = '5px';
        objectGroup.style.flexWrap = 'wrap';

        modeSelect.onchange = (e) => {
            const mode = (e.target as HTMLSelectElement).value;
            if (mode === 'view') {
                this.game.getInputManager().setMode('view');
                objectGroup.style.display = 'none';
            } else {
                // Edit mode: show tools and set to create mode (or last used)
                // Default to create mode with currently selected template if any
                this.game.getInputManager().setMode('create');
                objectGroup.style.display = 'flex';
            }
        };

        Templates.forEach(template => {
            // Removed check for isTool to allow all templates including tools (like Wall)

            const btn = document.createElement('button');
            btn.textContent = template.name;
            btn.style.padding = '5px 10px';
            btn.style.cursor = 'pointer';
            btn.onclick = () => {
                if (template.name === 'Eraser') {
                    this.game.getInputManager().setMode('erase');
                } else {
                    this.game.getInputManager().setMode('create');
                    this.game.getInputManager().setTemplate(template);
                }
                // Highlight selected
                Array.from(objectGroup.children).forEach(c => c.classList.remove('active'));
                btn.classList.add('active');
            };
            objectGroup.appendChild(btn);
        });

        // Select first template by default
        if (Templates.length > 0) {
            // Default to first template
            const firstTemplate = Templates[0];
            this.game.getInputManager().setMode('create');
            this.game.getInputManager().setTemplate(firstTemplate);
            if (objectGroup.firstElementChild) {
                objectGroup.firstElementChild.classList.add('active');
            }
        }

        toolbar.appendChild(controlsGroup);
        toolbar.appendChild(objectGroup);

        // Initialize Settings Pane
        new SettingsPane(this.game);

        this.container.appendChild(toolbar);
    }
}
