#ifndef PHYSBOX5_BOX2D_COMPARATOR_SHIM_H
#define PHYSBOX5_BOX2D_COMPARATOR_SHIM_H

#include <stdbool.h>
#include <stdint.h>

typedef struct pb5_box2d_world pb5_box2d_world;

typedef struct pb5_box2d_world_def {
    float gravity_x;
    float gravity_y;
    float restitution_threshold;
    uint32_t capacity;
    uint32_t sleeping_enabled;
} pb5_box2d_world_def;

typedef struct pb5_box2d_body {
    uint32_t slot;
    uint32_t generation;
    uint32_t shape_kind;
    uint32_t flags;
    float position_x;
    float position_y;
    float angle;
    float angular_velocity;
    float velocity_x;
    float velocity_y;
    float half_extent_x;
    float half_extent_y;
    float inverse_mass;
    float inverse_inertia;
    float friction;
    float restitution;
} pb5_box2d_body;

typedef struct pb5_box2d_snapshot {
    uint32_t slot;
    uint32_t generation;
    uint32_t shape_kind;
    uint32_t flags;
    float position_x;
    float position_y;
    float angle;
    float angular_velocity;
    float velocity_x;
    float velocity_y;
    float half_extent_x;
    float half_extent_y;
} pb5_box2d_snapshot;

enum {
    PB5_BOX2D_SHAPE_CIRCLE = 1,
    PB5_BOX2D_SHAPE_RECTANGLE = 2,
    PB5_BOX2D_BODY_LIVE = 1 << 0,
    PB5_BOX2D_BODY_FIXED = 1 << 1,
    PB5_BOX2D_BODY_AWAKE = 1 << 2,
};

pb5_box2d_world* pb5_box2d_world_create(const pb5_box2d_world_def* definition);
void pb5_box2d_world_destroy(pb5_box2d_world* world);
bool pb5_box2d_world_add_body(pb5_box2d_world* world, const pb5_box2d_body* body);
bool pb5_box2d_world_remove_body(
    pb5_box2d_world* world,
    uint32_t slot,
    uint32_t generation
);
bool pb5_box2d_world_apply_impulse(
    pb5_box2d_world* world,
    uint32_t slot,
    uint32_t generation,
    float impulse_x,
    float impulse_y,
    float point_x,
    float point_y
);
bool pb5_box2d_world_reset_body(
    pb5_box2d_world* world,
    uint32_t slot,
    uint32_t generation,
    float position_x,
    float position_y,
    float angle,
    float velocity_x,
    float velocity_y,
    float angular_velocity
);
bool pb5_box2d_world_begin_grab(
    pb5_box2d_world* world,
    uint32_t slot,
    uint32_t generation,
    float anchor_x,
    float anchor_y,
    float target_x,
    float target_y,
    float spring_acceleration,
    float damping_rate,
    float max_acceleration
);
bool pb5_box2d_world_update_grab(pb5_box2d_world* world, float target_x, float target_y);
void pb5_box2d_world_end_grab(pb5_box2d_world* world);
void pb5_box2d_world_set_gravity(pb5_box2d_world* world, float x, float y);
void pb5_box2d_world_step(pb5_box2d_world* world, float dt, int32_t substeps);
uint32_t pb5_box2d_world_snapshot(
    const pb5_box2d_world* world,
    pb5_box2d_snapshot* output,
    uint32_t capacity
);
const char* pb5_box2d_world_last_error(const pb5_box2d_world* world);

#endif
