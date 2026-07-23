#include "oracle_shim.h"

#include "particle_system.h"

#include <box2d/box2d.h>
#include <box2cpp/box2cpp.h>

#include <cmath>
#include <cstdint>
#include <new>
#include <vector>

using physbox3::particle_sidecar::CreateParticleSystem;
using physbox3::particle_sidecar::DestroyParticleSystem;
using physbox3::particle_sidecar::ParticleSystemDef;
using physbox3::particle_sidecar::ParticleSystemSidecar;

namespace {

struct OracleWorld
{
    b2::World world;
    ParticleSystemSidecar* particles = nullptr;
    std::vector<b2BodyId> bodies;
    float particleRadius = 0.1f;

    OracleWorld(
        float gravityX,
        float gravityY,
        float particleRadiusValue,
        float particleDensity,
        float particleGravityScale,
        std::uint32_t particleCapacity)
        : world(WorldParameters(gravityX, gravityY)), particleRadius(particleRadiusValue)
    {
        ParticleSystemDef definition;
        definition.radius = particleRadiusValue;
        definition.density = particleDensity;
        definition.gravityScale = particleGravityScale;
        definition.initialCapacity = static_cast<int>(particleCapacity);
        definition.maxParticles = static_cast<int>(particleCapacity);
        particles = CreateParticleSystem(world, definition);
        bodies.reserve(64);
    }

    ~OracleWorld()
    {
        DestroyParticleSystem(particles);
    }

    static b2::World::Params WorldParameters(float gravityX, float gravityY)
    {
        b2::World::Params parameters;
        parameters.gravity = {gravityX, gravityY};
        parameters.enableSleep = true;
        return parameters;
    }
};

int32_t AddBody(
    OracleWorld* oracle,
    bool dynamicBody,
    float positionX,
    float positionY,
    float angle,
    float linearVelocityX,
    float linearVelocityY,
    float angularVelocity,
    float inverseMass,
    float inverseInertia,
    float friction,
    float restitution,
    bool sleeping,
    bool circle,
    float extentX,
    float extentY)
{
    if (oracle == nullptr)
    {
        return -1;
    }

    b2BodyDef bodyDefinition = b2DefaultBodyDef();
    bodyDefinition.type = dynamicBody ? b2_dynamicBody : b2_staticBody;
    bodyDefinition.position = {positionX, positionY};
    bodyDefinition.rotation = b2MakeRot(angle);
    bodyDefinition.linearVelocity = {linearVelocityX, linearVelocityY};
    bodyDefinition.angularVelocity = angularVelocity;
    bodyDefinition.enableSleep = true;
    bodyDefinition.isAwake = !sleeping;
    const b2BodyId body = b2CreateBody(oracle->world.Handle(), &bodyDefinition);
    if (!B2_IS_NON_NULL(body))
    {
        return -1;
    }

    b2ShapeDef shapeDefinition = b2DefaultShapeDef();
    shapeDefinition.density = dynamicBody && inverseMass > 0.0f ? 1.0f : 0.0f;
    shapeDefinition.material.friction = friction;
    shapeDefinition.material.restitution = restitution;
    if (circle)
    {
        const b2Circle circleShape = {
            .center = {0.0f, 0.0f},
            .radius = extentX,
        };
        b2CreateCircleShape(body, &shapeDefinition, &circleShape);
    }
    else
    {
        const b2Polygon rectangle = b2MakeBox(extentX, extentY);
        b2CreatePolygonShape(body, &shapeDefinition, &rectangle);
    }

    if (dynamicBody && inverseMass > 0.0f && inverseInertia > 0.0f)
    {
        const b2MassData massData = {
            .mass = 1.0f / inverseMass,
            .center = {0.0f, 0.0f},
            .rotationalInertia = 1.0f / inverseInertia,
        };
        b2Body_SetMassData(body, massData);
    }
    if (sleeping)
    {
        b2Body_SetAwake(body, false);
    }
    oracle->bodies.push_back(body);
    return static_cast<int32_t>(oracle->bodies.size() - 1);
}

} // namespace

extern "C" void* pb3_oracle_create(
    float gravityX,
    float gravityY,
    float particleRadius,
    float particleDensity,
    float particleGravityScale,
    std::uint32_t particleCapacity)
{
    if (!(particleRadius > 0.0f) || !(particleDensity > 0.0f) || particleCapacity == 0)
    {
        return nullptr;
    }
    return new (std::nothrow) OracleWorld(
        gravityX,
        gravityY,
        particleRadius,
        particleDensity,
        particleGravityScale,
        particleCapacity);
}

extern "C" void pb3_oracle_destroy(void* handle)
{
    delete static_cast<OracleWorld*>(handle);
}

extern "C" int32_t pb3_oracle_add_circle(
    void* handle,
    std::uint32_t dynamicBody,
    float positionX,
    float positionY,
    float angle,
    float linearVelocityX,
    float linearVelocityY,
    float angularVelocity,
    float radius,
    float inverseMass,
    float inverseInertia,
    float friction,
    float restitution,
    std::uint32_t sleeping)
{
    return AddBody(
        static_cast<OracleWorld*>(handle),
        dynamicBody != 0,
        positionX,
        positionY,
        angle,
        linearVelocityX,
        linearVelocityY,
        angularVelocity,
        inverseMass,
        inverseInertia,
        friction,
        restitution,
        sleeping != 0,
        true,
        radius,
        radius);
}

extern "C" int32_t pb3_oracle_add_rectangle(
    void* handle,
    std::uint32_t dynamicBody,
    float positionX,
    float positionY,
    float angle,
    float linearVelocityX,
    float linearVelocityY,
    float angularVelocity,
    float halfExtentX,
    float halfExtentY,
    float inverseMass,
    float inverseInertia,
    float friction,
    float restitution,
    std::uint32_t sleeping)
{
    return AddBody(
        static_cast<OracleWorld*>(handle),
        dynamicBody != 0,
        positionX,
        positionY,
        angle,
        linearVelocityX,
        linearVelocityY,
        angularVelocity,
        inverseMass,
        inverseInertia,
        friction,
        restitution,
        sleeping != 0,
        false,
        halfExtentX,
        halfExtentY);
}

extern "C" std::uint32_t pb3_oracle_spawn_rectangle(
    void* handle,
    float lowerX,
    float lowerY,
    float upperX,
    float upperY,
    float particleRadius,
    float spacing,
    float velocityX,
    float velocityY)
{
    OracleWorld* oracle = static_cast<OracleWorld*>(handle);
    if (
        oracle == nullptr ||
        oracle->particles == nullptr ||
        upperX <= lowerX ||
        upperY <= lowerY ||
        particleRadius <= 0.0f ||
        spacing <= 0.0f)
    {
        return 0;
    }

    // The pinned first-party sidecar intentionally exposes only circle
    // spawning. A radius r with stride 1.5r emits exactly one lattice point at
    // center + (0.5r, 0.5r). Use bounded one-point calls to project the neutral
    // authored rectangle exactly without patching or reaching into the clean
    // sidecar implementation.
    const float microRadius = std::min(particleRadius, spacing) * 0.001f;
    const float microStride = microRadius * 1.5f;
    const b2Vec2 velocity = {velocityX, velocityY};
    std::uint32_t created = 0;
    for (float y = lowerY + particleRadius; y <= upperY - particleRadius + 1.0e-6f; y += spacing)
    {
        for (float x = lowerX + particleRadius; x <= upperX - particleRadius + 1.0e-6f; x += spacing)
        {
            const int pointCount = oracle->particles->SpawnParticlesInCircle(
                {x - 0.5f * microRadius, y - 0.5f * microRadius},
                microRadius,
                microStride,
                velocity);
            if (pointCount != 1)
            {
                return 0;
            }
            ++created;
        }
    }
    return created;
}

extern "C" void pb3_oracle_set_gravity(void* handle, float gravityX, float gravityY)
{
    OracleWorld* oracle = static_cast<OracleWorld*>(handle);
    if (oracle != nullptr)
    {
        b2World_SetGravity(oracle->world.Handle(), {gravityX, gravityY});
    }
}

extern "C" std::uint32_t pb3_oracle_apply_impulse(
    void* handle,
    std::uint32_t bodyIndex,
    float impulseX,
    float impulseY,
    float pointX,
    float pointY)
{
    OracleWorld* oracle = static_cast<OracleWorld*>(handle);
    if (oracle == nullptr || bodyIndex >= oracle->bodies.size())
    {
        return 0;
    }
    b2Body_ApplyLinearImpulse(
        oracle->bodies[bodyIndex],
        {impulseX, impulseY},
        {pointX, pointY},
        true);
    return 1;
}

extern "C" std::uint32_t pb3_oracle_step(void* handle, float timeStep, std::uint32_t substeps)
{
    OracleWorld* oracle = static_cast<OracleWorld*>(handle);
    if (oracle == nullptr || oracle->particles == nullptr || !(timeStep > 0.0f))
    {
        return 0;
    }
    oracle->particles->StepWithWorld(timeStep, static_cast<int>(substeps > 0 ? substeps : 1));
    return 1;
}

extern "C" std::uint32_t pb3_oracle_particle_count(const void* handle)
{
    const OracleWorld* oracle = static_cast<const OracleWorld*>(handle);
    return oracle == nullptr || oracle->particles == nullptr
        ? 0
        : static_cast<std::uint32_t>(oracle->particles->GetParticleCount());
}

extern "C" const float* pb3_oracle_particle_positions(const void* handle)
{
    const OracleWorld* oracle = static_cast<const OracleWorld*>(handle);
    if (oracle == nullptr || oracle->particles == nullptr || oracle->particles->GetParticleCount() == 0)
    {
        return nullptr;
    }
    return reinterpret_cast<const float*>(oracle->particles->GetPositionData());
}

extern "C" std::uint32_t pb3_oracle_body_count(const void* handle)
{
    const OracleWorld* oracle = static_cast<const OracleWorld*>(handle);
    return oracle == nullptr ? 0 : static_cast<std::uint32_t>(oracle->bodies.size());
}

extern "C" std::uint32_t pb3_oracle_body_state(
    const void* handle,
    std::uint32_t index,
    Pb3BodyState* state)
{
    const OracleWorld* oracle = static_cast<const OracleWorld*>(handle);
    if (oracle == nullptr || state == nullptr || index >= oracle->bodies.size())
    {
        return 0;
    }
    const b2BodyId body = oracle->bodies[index];
    const b2Vec2 position = b2Body_GetPosition(body);
    const b2Rot rotation = b2Body_GetRotation(body);
    const b2Vec2 linearVelocity = b2Body_GetLinearVelocity(body);
    state->position_x = position.x;
    state->position_y = position.y;
    state->angle = std::atan2(rotation.s, rotation.c);
    state->linear_velocity_x = linearVelocity.x;
    state->linear_velocity_y = linearVelocity.y;
    state->angular_velocity = b2Body_GetAngularVelocity(body);
    state->awake = b2Body_IsAwake(body) ? 1u : 0u;
    return 1;
}
