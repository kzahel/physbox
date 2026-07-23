#pragma once

#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

typedef struct LiquidFunBodyState
{
    float position_x;
    float position_y;
    float angle;
    float linear_velocity_x;
    float linear_velocity_y;
    float angular_velocity;
    uint32_t awake;
} LiquidFunBodyState;

void* liquidfun_oracle_create(
    float gravity_x,
    float gravity_y,
    float particle_radius,
    float particle_density,
    float particle_gravity_scale,
    uint32_t particle_capacity);

void liquidfun_oracle_destroy(void* handle);

int32_t liquidfun_oracle_add_circle(
    void* handle,
    uint32_t dynamic_body,
    float position_x,
    float position_y,
    float angle,
    float linear_velocity_x,
    float linear_velocity_y,
    float angular_velocity,
    float radius,
    float inverse_mass,
    float inverse_inertia,
    float friction,
    float restitution,
    uint32_t sleeping);

int32_t liquidfun_oracle_add_rectangle(
    void* handle,
    uint32_t dynamic_body,
    float position_x,
    float position_y,
    float angle,
    float linear_velocity_x,
    float linear_velocity_y,
    float angular_velocity,
    float half_extent_x,
    float half_extent_y,
    float inverse_mass,
    float inverse_inertia,
    float friction,
    float restitution,
    uint32_t sleeping);

uint32_t liquidfun_oracle_spawn_rectangle(
    void* handle,
    float lower_x,
    float lower_y,
    float upper_x,
    float upper_y,
    float spacing,
    float velocity_x,
    float velocity_y);

void liquidfun_oracle_set_gravity(void* handle, float gravity_x, float gravity_y);

uint32_t liquidfun_oracle_apply_impulse(
    void* handle,
    uint32_t body_index,
    float impulse_x,
    float impulse_y,
    float point_x,
    float point_y);

uint32_t liquidfun_oracle_step(void* handle, float time_step, uint32_t substeps);
uint32_t liquidfun_oracle_particle_count(const void* handle);
const float* liquidfun_oracle_particle_positions(const void* handle);
uint32_t liquidfun_oracle_body_contact_count(const void* handle);
uint32_t liquidfun_oracle_body_state(
    const void* handle,
    uint32_t index,
    LiquidFunBodyState* state);

#ifdef __cplusplus
}
#endif
