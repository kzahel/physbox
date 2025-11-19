import Matter from 'matter-js';
import { Game } from '../engine/Game';

export interface EntityTemplate {
    name: string;
    create: (x: number, y: number, game: Game) => Matter.Body | Matter.Composite;
    isTool?: boolean;
}

export const Templates: EntityTemplate[] = [
    {
        name: 'Box',
        create: (x, y) => Matter.Bodies.rectangle(x, y, 50, 50, { render: { fillStyle: '#F35' } })
    },
    {
        name: 'Ball',
        create: (x, y, game) => Matter.Bodies.circle(x, y, game.ballSize, {
            label: 'Ball',
            render: {
                fillStyle: '#35F',
                strokeStyle: 'black',
                lineWidth: 2
            },
            restitution: game.ballElasticity,
            frictionAir: game.globalAirFriction
        })
    },
    {
        name: 'Floor',
        create: (x, y) => Matter.Bodies.rectangle(x, y, 200, 20, { isStatic: true, render: { fillStyle: '#888' }, label: 'Ground' })
    },
    {
        name: 'Wall',
        create: (x, y) => Matter.Bodies.rectangle(x, y, 20, 200, { isStatic: true, render: { fillStyle: '#888' }, label: 'Ground' })
    },
    {
        name: 'Draw Wall',
        isTool: true,
        create: (_x, _y) => { throw new Error('Tool template cannot be created directly'); }
    },
    {
        name: 'Eraser',
        isTool: true,
        create: (_x, _y) => { throw new Error('Tool template cannot be created directly'); }
    },
    {
        name: 'Car',
        create: (x, y) => {
            const group = Matter.Body.nextGroup(true);
            const wheelAOffset = -50;
            const wheelBOffset = 50;
            const wheelYOffset = 20;

            const chassis = Matter.Bodies.rectangle(x, y, 150, 30, {
                collisionFilter: { group: group },
                render: { fillStyle: '#C84' }
            });

            const createWheel = (wx: number, wy: number) => {
                const circle = Matter.Bodies.circle(wx, wy, 20, {
                    render: { fillStyle: '#444' }
                });
                const spoke = Matter.Bodies.rectangle(wx, wy, 40, 4, {
                    render: { fillStyle: '#EEE' }
                });

                const wheel = Matter.Body.create({
                    parts: [circle, spoke],
                    collisionFilter: { group: group },
                    friction: 0.8,
                    label: 'CarWheel'
                });
                return wheel;
            };

            const wheelA = createWheel(x + wheelAOffset, y + wheelYOffset);
            const wheelB = createWheel(x + wheelBOffset, y + wheelYOffset);

            const axelA = Matter.Constraint.create({
                bodyA: chassis,
                bodyB: wheelA,
                pointA: { x: wheelAOffset, y: wheelYOffset },
                stiffness: 1,
                length: 0
            });

            const axelB = Matter.Constraint.create({
                bodyA: chassis,
                bodyB: wheelB,
                pointA: { x: wheelBOffset, y: wheelYOffset },
                stiffness: 1,
                length: 0
            });

            return Matter.Composite.create({
                bodies: [chassis, wheelA, wheelB],
                constraints: [axelA, axelB]
            });
        }
    },
    {
        name: 'Monster',
        create: (x, y) => {
            const head = Matter.Bodies.polygon(x, y, 8, 40, {
                render: { fillStyle: '#8A2BE2' }
            });

            const eyeLeft = Matter.Bodies.circle(x - 15, y - 10, 5, {
                render: { fillStyle: '#FFF' }
            });

            const eyeRight = Matter.Bodies.circle(x + 15, y - 10, 5, {
                render: { fillStyle: '#FFF' }
            });

            const tentacleA = Matter.Bodies.rectangle(x - 30, y + 30, 10, 40, {
                render: { fillStyle: '#8A2BE2' }
            });

            const tentacleB = Matter.Bodies.rectangle(x + 30, y + 30, 10, 40, {
                render: { fillStyle: '#8A2BE2' }
            });

            const body = Matter.Body.create({
                parts: [head, eyeLeft, eyeRight, tentacleA, tentacleB],
                label: 'Monster',
                frictionAir: 0.05 // Add some air friction to make movement more blob-like
            });

            return body;
        }
    }
];
