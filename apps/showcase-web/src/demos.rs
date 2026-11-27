use physbox_sim::{BodyDef, ParticleRegionDef, ParticleSystemDef, ShapeDef, World};

use crate::{ShapeVisual, add_body, palette};

pub(super) fn add_domino_run(world: &mut World, visuals: &mut [ShapeVisual]) -> Result<(), String> {
    let half_height = 0.85;
    let half_width = 0.10;
    for tier in 0..4 {
        let left = -9.2 + tier as f32 * 4.6;
        let top = 0.65 - tier as f32 * 0.50;
        add_body(
            world,
            visuals,
            BodyDef::fixed().with_position([left + 2.3, top - 0.11]),
            ShapeDef::rectangle([2.3, 0.11]).with_friction(0.72),
            [0.22, 0.31, 0.42, 1.0],
        )?;
        for index in 0..10 {
            let x = left + 0.23 + index as f32 * 0.46;
            let body = if tier == 0 && index == 0 {
                let angle = -0.32_f32;
                let y = top + half_height * angle.cos() - half_width * angle.sin();
                BodyDef::dynamic()
                    .with_position([x, y])
                    .with_angle(angle)
                    .with_angular_velocity(-2.5)
            } else {
                BodyDef::dynamic().with_position([x, top + half_height])
            };
            add_body(
                world,
                visuals,
                body,
                ShapeDef::rectangle([half_width, half_height])
                    .with_friction(0.62)
                    .with_restitution(0.02),
                palette(tier),
            )?;
        }
    }
    Ok(())
}

pub(super) fn add_marble_pegboard(
    world: &mut World,
    visuals: &mut [ShapeVisual],
) -> Result<(), String> {
    for row in 0..7 {
        for column in -5..=5 {
            let x = column as f32 * 1.24 + (row % 2) as f32 * 0.62;
            let y = 1.65 - row as f32 * 0.57;
            add_body(
                world,
                visuals,
                BodyDef::fixed().with_position([x, y]),
                ShapeDef::circle(0.15)
                    .with_friction(0.12)
                    .with_restitution(0.30),
                [0.31, 0.44, 0.55, 1.0],
            )?;
        }
    }
    for column in -4..=4 {
        add_body(
            world,
            visuals,
            BodyDef::fixed().with_position([column as f32 * 1.24, -3.47]),
            ShapeDef::rectangle([0.065, 0.63]).with_friction(0.35),
            [0.23, 0.35, 0.46, 1.0],
        )?;
    }
    for row in 0..8 {
        for column in 0..8 {
            let x = (column as f32 - 3.5) * 0.52 + (row % 2) as f32 * 0.22;
            let y = 2.15 + row as f32 * 0.34;
            let radius = 0.13 + ((row + column) % 3) as f32 * 0.018;
            add_body(
                world,
                visuals,
                BodyDef::dynamic()
                    .with_position([x, y])
                    .with_linear_velocity([(column as f32 - 3.5) * 0.16, -0.8]),
                ShapeDef::circle(radius)
                    .with_friction(0.20)
                    .with_restitution(0.45),
                palette((row / 2 + column / 2) as usize),
            )?;
        }
    }
    Ok(())
}

pub(super) fn add_water_surge(
    world: &mut World,
    visuals: &mut [ShapeVisual],
    particles: &mut Option<ParticleSystemDef>,
) -> Result<u32, String> {
    for (x, density, color) in [
        (-1.05, 0.30, palette(1)),
        (1.40, 0.30, palette(1)),
        (4.60, 2.5, palette(2)),
    ] {
        add_body(
            world,
            visuals,
            BodyDef::dynamic().with_position([x, -3.02]),
            ShapeDef::rectangle([0.63, 0.25])
                .with_density(density)
                .with_friction(0.45)
                .with_restitution(0.04),
            color,
        )?;
    }
    let Some(definition) = particles.take() else {
        return Ok(0);
    };
    let definition = definition
        .with_region(ParticleRegionDef::rectangle(
            [-6.7, -3.94],
            [-2.2, 2.51],
            0.1,
        ))
        .with_region(ParticleRegionDef::rectangle(
            [-2.14, -3.94],
            [6.8, -3.32],
            0.1,
        ));
    let count = definition
        .initial_particle_count()
        .map_err(|error| error.to_string())?;
    *particles = Some(definition);
    Ok(count)
}
