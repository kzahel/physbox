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
    },
    {
        name: 'Player',
        create: (x, y) => {
            const group = Matter.Body.nextGroup(true);
            const bodyWidth = 40;
            const bodyHeight = 80;
            const wheelSize = 15;

            // Main Body (Robot Torso)
            const torso = Matter.Bodies.rectangle(x, y - 20, bodyWidth, bodyHeight, {
                collisionFilter: { group: group },
                render: { fillStyle: '#555' },
                label: 'PlayerBodyPart'
            });

            // Head (Cute Face)
            const headSize = 30;
            const head = Matter.Bodies.rectangle(x, y - 70, headSize, headSize, {
                collisionFilter: { group: group },
                render: { fillStyle: '#DDD' },
                label: 'PlayerHead'
            });

            // Eyes
            const eyeLeft = Matter.Bodies.circle(x - 8, y - 75, 3, {
                collisionFilter: { group: group },
                render: { fillStyle: '#000' }
            });
            const eyeRight = Matter.Bodies.circle(x + 8, y - 75, 3, {
                collisionFilter: { group: group },
                render: { fillStyle: '#000' }
            });

            // Mouth (Simple rectangle for now)
            const mouth = Matter.Bodies.rectangle(x, y - 65, 10, 3, {
                collisionFilter: { group: group },
                render: { fillStyle: '#000' }
            });

            // Combine torso and head parts into one rigid body
            const mainBody = Matter.Body.create({
                parts: [torso, head, eyeLeft, eyeRight, mouth],
                collisionFilter: { group: group },
                inertia: Infinity, // Prevent rotation
                frictionAir: 0.05,
                label: 'PlayerBody'
            });

            // Helper to create a wheel with a spoke
            const createWheel = (wx: number, wy: number) => {
                const circle = Matter.Bodies.circle(wx, wy, wheelSize, {
                    render: { fillStyle: '#333' }
                });
                const spoke = Matter.Bodies.rectangle(wx, wy, wheelSize * 1.8, 4, {
                    render: { fillStyle: '#EEE' }
                });

                return Matter.Body.create({
                    parts: [circle, spoke],
                    collisionFilter: { group: group },
                    friction: 0.8,
                    label: 'PlayerWheel'
                });
            };

            const wheelA = createWheel(x - bodyWidth / 2, y + bodyHeight / 2 - 20);
            const wheelB = createWheel(x + bodyWidth / 2, y + bodyHeight / 2 - 20);

            // Axles - Use dynamic offsets to prevent snapping
            const axelA = Matter.Constraint.create({
                bodyB: mainBody,
                bodyA: wheelA,
                pointB: { x: wheelA.position.x - mainBody.position.x, y: wheelA.position.y - mainBody.position.y },
                stiffness: 1,
                length: 0
            });

            const axelB = Matter.Constraint.create({
                bodyB: mainBody,
                bodyA: wheelB,
                pointB: { x: wheelB.position.x - mainBody.position.x, y: wheelB.position.y - mainBody.position.y },
                stiffness: 1,
                length: 0
            });

            return Matter.Composite.create({
                bodies: [mainBody, wheelA, wheelB],
                constraints: [axelA, axelB],
                label: 'Player'
            });
        }
    }
];
