import './style.css';
import { Game } from './engine/Game';
import { Toolbar } from './ui/Toolbar';
import Matter from 'matter-js';

// Initialize game
const game = new Game('canvas-container');
new Toolbar('ui-layer', game);
game.start();

// Add some test objects
const engine = game.getEngine();
const render = game.getRender();

const ground = Matter.Bodies.rectangle(render.options.width! / 2, render.options.height! - 20, render.options.width!, 40, { isStatic: true, render: { fillStyle: '#333' } });
const box = Matter.Bodies.rectangle(render.options.width! / 2, 200, 80, 80, { render: { fillStyle: '#F35' } });

Matter.Composite.add(engine.world, [ground, box]);


