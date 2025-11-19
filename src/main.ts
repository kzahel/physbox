import './style.css';
import { Game } from './engine/Game';
import { Toolbar } from './ui/Toolbar';

// Initialize game
const game = new Game('canvas-container');
new Toolbar('ui-layer', game);
game.start();

// Initial state
game.reset();


