#include "comparator_shim.h"

#include <box2d/box2d.h>

#include <math.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

typedef struct pb5_box2d_slot {
    b2BodyId body;
    uint32_t generation;
    uint32_t shape_kind;
    uint32_t flags;
    float half_extent_x;
    float half_extent_y;
} pb5_box2d_slot;

struct pb5_box2d_world {
    b2WorldId world;
    pb5_box2d_slot* slots;
    uint32_t capacity;
    b2BodyId grab_target;
    b2JointId grab_joint;
    float grab_x;
    float grab_y;
    bool grab_active;
    char error[160];
};

static void pb5_set_error(pb5_box2d_world* world, const char* message) {
    if (world == NULL) {
        return;
    }
    snprintf(world->error, sizeof(world->error), "%s", message);
}

static bool pb5_slot_matches(
    const pb5_box2d_world* world,
    uint32_t slot,
    uint32_t generation
) {
    return world != NULL && slot < world->capacity &&
        (world->slots[slot].flags & PB5_BOX2D_BODY_LIVE) != 0 &&
        world->slots[slot].generation == generation;
}

pb5_box2d_world* pb5_box2d_world_create(const pb5_box2d_world_def* definition) {
    if (definition == NULL || definition->capacity == 0) {
        return NULL;
    }
    pb5_box2d_world* world = calloc(1, sizeof(pb5_box2d_world));
    if (world == NULL) {
        return NULL;
    }
    world->slots = calloc(definition->capacity, sizeof(pb5_box2d_slot));
    if (world->slots == NULL) {
        free(world);
        return NULL;
    }
    world->capacity = definition->capacity;

    b2WorldDef world_def = b2DefaultWorldDef();
    world_def.gravity = (b2Vec2){definition->gravity_x, definition->gravity_y};
    world_def.restitutionThreshold = definition->restitution_threshold;
    world_def.workerCount = 1;
    world_def.enqueueTask = NULL;
    world_def.finishTask = NULL;
    world_def.enableSleep = definition->sleeping_enabled != 0;
    world_def.enableContinuous = true;
    world_def.enableContactSoftening = false;
    world->world = b2CreateWorld(&world_def);
    b2World_EnableWarmStarting(world->world, true);
    return world;
}

void pb5_box2d_world_end_grab(pb5_box2d_world* world) {
    if (world == NULL || !world->grab_active) {
        return;
    }
    b2DestroyJoint(world->grab_joint, true);
    b2DestroyBody(world->grab_target);
    world->grab_active = false;
}

void pb5_box2d_world_destroy(pb5_box2d_world* world) {
    if (world == NULL) {
        return;
    }
    pb5_box2d_world_end_grab(world);
    b2DestroyWorld(world->world);
    free(world->slots);
    free(world);
}

bool pb5_box2d_world_add_body(pb5_box2d_world* world, const pb5_box2d_body* body) {
    if (world == NULL || body == NULL || body->slot >= world->capacity) {
        pb5_set_error(world, "body slot exceeds comparator capacity");
        return false;
    }
    pb5_box2d_slot* slot = world->slots + body->slot;
    if ((slot->flags & PB5_BOX2D_BODY_LIVE) != 0 || body->generation == 0) {
        pb5_set_error(world, "body identity is already live or has zero generation");
        return false;
    }

    b2BodyDef body_def = b2DefaultBodyDef();
    body_def.type = (body->flags & PB5_BOX2D_BODY_FIXED) != 0
        ? b2_staticBody
        : b2_dynamicBody;
    body_def.position = (b2Vec2){body->position_x, body->position_y};
    body_def.rotation = b2MakeRot(body->angle);
    body_def.linearVelocity = (b2Vec2){body->velocity_x, body->velocity_y};
    body_def.angularVelocity = body->angular_velocity;
    body_def.isAwake = (body->flags & PB5_BOX2D_BODY_AWAKE) != 0;
    body_def.enableSleep = true;
    b2BodyId raw_body = b2CreateBody(world->world, &body_def);

    b2ShapeDef shape_def = b2DefaultShapeDef();
    shape_def.material.friction = body->friction;
    shape_def.material.restitution = body->restitution;
    shape_def.material.rollingResistance = 0.0f;
    shape_def.material.tangentSpeed = 0.0f;
    shape_def.density = 1.0f;
    if (body->shape_kind == PB5_BOX2D_SHAPE_CIRCLE) {
        b2Circle circle = {{0.0f, 0.0f}, body->half_extent_x};
        b2CreateCircleShape(raw_body, &shape_def, &circle);
    } else if (body->shape_kind == PB5_BOX2D_SHAPE_RECTANGLE) {
        b2Polygon polygon = b2MakeBox(body->half_extent_x, body->half_extent_y);
        b2CreatePolygonShape(raw_body, &shape_def, &polygon);
    } else {
        b2DestroyBody(raw_body);
        pb5_set_error(world, "unsupported comparator shape");
        return false;
    }
    if ((body->flags & PB5_BOX2D_BODY_FIXED) == 0) {
        b2MassData mass_data = {
            1.0f / body->inverse_mass,
            {0.0f, 0.0f},
            1.0f / body->inverse_inertia,
        };
        b2Body_SetMassData(raw_body, mass_data);
    }

    slot->body = raw_body;
    slot->generation = body->generation;
    slot->shape_kind = body->shape_kind;
    slot->flags = PB5_BOX2D_BODY_LIVE |
        (body->flags & PB5_BOX2D_BODY_FIXED);
    slot->half_extent_x = body->half_extent_x;
    slot->half_extent_y = body->half_extent_y;
    world->error[0] = '\0';
    return true;
}

bool pb5_box2d_world_remove_body(
    pb5_box2d_world* world,
    uint32_t slot_index,
    uint32_t generation
) {
    if (!pb5_slot_matches(world, slot_index, generation)) {
        pb5_set_error(world, "remove targeted a stale comparator identity");
        return false;
    }
    if (world->grab_active) {
        pb5_box2d_world_end_grab(world);
    }
    b2DestroyBody(world->slots[slot_index].body);
    memset(world->slots + slot_index, 0, sizeof(pb5_box2d_slot));
    world->error[0] = '\0';
    return true;
}

bool pb5_box2d_world_apply_impulse(
    pb5_box2d_world* world,
    uint32_t slot,
    uint32_t generation,
    float impulse_x,
    float impulse_y,
    float point_x,
    float point_y
) {
    if (!pb5_slot_matches(world, slot, generation)) {
        pb5_set_error(world, "impulse targeted a stale comparator identity");
        return false;
    }
    b2Body_ApplyLinearImpulse(
        world->slots[slot].body,
        (b2Vec2){impulse_x, impulse_y},
        (b2Vec2){point_x, point_y},
        true
    );
    world->error[0] = '\0';
    return true;
}

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
) {
    if (!pb5_slot_matches(world, slot, generation) ||
        (world->slots[slot].flags & PB5_BOX2D_BODY_FIXED) != 0) {
        pb5_set_error(world, "grab targeted a stale or fixed comparator identity");
        return false;
    }
    pb5_box2d_world_end_grab(world);
    b2BodyId body = world->slots[slot].body;
    b2BodyDef target_def = b2DefaultBodyDef();
    target_def.type = b2_kinematicBody;
    target_def.position = (b2Vec2){target_x, target_y};
    target_def.enableSleep = false;
    world->grab_target = b2CreateBody(world->world, &target_def);

    b2MotorJointDef joint_def = b2DefaultMotorJointDef();
    joint_def.base.bodyIdA = world->grab_target;
    joint_def.base.bodyIdB = body;
    joint_def.base.localFrameB.p =
        b2Body_GetLocalPoint(body, (b2Vec2){anchor_x, anchor_y});
    const float angular_frequency = sqrtf(spring_acceleration);
    joint_def.linearHertz = angular_frequency / (2.0f * B2_PI);
    joint_def.linearDampingRatio =
        damping_rate / (2.0f * angular_frequency);
    b2MassData mass_data = b2Body_GetMassData(body);
    joint_def.maxSpringForce = mass_data.mass * max_acceleration;
    world->grab_joint = b2CreateMotorJoint(world->world, &joint_def);
    world->grab_x = target_x;
    world->grab_y = target_y;
    world->grab_active = true;
    world->error[0] = '\0';
    return true;
}

bool pb5_box2d_world_update_grab(pb5_box2d_world* world, float target_x, float target_y) {
    if (world == NULL || !world->grab_active) {
        pb5_set_error(world, "grab target requires an active comparator grab");
        return false;
    }
    world->grab_x = target_x;
    world->grab_y = target_y;
    world->error[0] = '\0';
    return true;
}

void pb5_box2d_world_set_gravity(pb5_box2d_world* world, float x, float y) {
    if (world != NULL) {
        b2World_SetGravity(world->world, (b2Vec2){x, y});
    }
}

void pb5_box2d_world_step(pb5_box2d_world* world, float dt, int32_t substeps) {
    if (world == NULL) {
        return;
    }
    if (world->grab_active) {
        b2Transform target = {
            {world->grab_x, world->grab_y},
            {1.0f, 0.0f},
        };
        b2Body_SetTargetTransform(world->grab_target, target, dt);
    }
    b2World_Step(world->world, dt, substeps);
}

uint32_t pb5_box2d_world_snapshot(
    const pb5_box2d_world* world,
    pb5_box2d_snapshot* output,
    uint32_t capacity
) {
    if (world == NULL || output == NULL || capacity < world->capacity) {
        return 0;
    }
    uint32_t count = 0;
    for (uint32_t index = 0; index < world->capacity; ++index) {
        const pb5_box2d_slot* slot = world->slots + index;
        if ((slot->flags & PB5_BOX2D_BODY_LIVE) == 0) {
            continue;
        }
        const b2Vec2 position = b2Body_GetPosition(slot->body);
        const b2Rot rotation = b2Body_GetRotation(slot->body);
        const b2Vec2 velocity = b2Body_GetLinearVelocity(slot->body);
        output[count++] = (pb5_box2d_snapshot){
            index,
            slot->generation,
            slot->shape_kind,
            slot->flags | (b2Body_IsAwake(slot->body) ? PB5_BOX2D_BODY_AWAKE : 0),
            position.x,
            position.y,
            b2Rot_GetAngle(rotation),
            b2Body_GetAngularVelocity(slot->body),
            velocity.x,
            velocity.y,
            slot->half_extent_x,
            slot->half_extent_y,
        };
    }
    return count;
}

const char* pb5_box2d_world_last_error(const pb5_box2d_world* world) {
    return world == NULL ? "comparator world is unavailable" : world->error;
}
