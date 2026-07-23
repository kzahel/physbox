#include "oracle_shim.h"

#include <Box2D/Box2D.h>

#include <cstdint>
#include <new>
#include <vector>

namespace {

struct OracleWorld
{
    b2World world;
    b2ParticleSystem* particles;
    std::vector<b2Body*> bodies;

    OracleWorld(
        float gravityX,
        float gravityY,
        float particleRadius,
        float particleDensity,
        float particleGravityScale,
        std::uint32_t particleCapacity)
        : world(b2Vec2(gravityX, gravityY)), particles(NULL)
    {
        b2ParticleSystemDef definition;
        definition.radius = particleRadius;
        definition.density = particleDensity;
        definition.gravityScale = particleGravityScale;
        definition.maxCount = static_cast<int32>(particleCapacity);
        definition.destroyByAge = false;
        particles = world.CreateParticleSystem(&definition);
        bodies.reserve(64);
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
    if (oracle == NULL)
    {
        return -1;
    }

    b2BodyDef bodyDefinition;
    bodyDefinition.type = dynamicBody ? b2_dynamicBody : b2_staticBody;
    bodyDefinition.position.Set(positionX, positionY);
    bodyDefinition.angle = angle;
    bodyDefinition.linearVelocity.Set(linearVelocityX, linearVelocityY);
    bodyDefinition.angularVelocity = angularVelocity;
    bodyDefinition.allowSleep = true;
    bodyDefinition.awake = !sleeping;
    b2Body* body = oracle->world.CreateBody(&bodyDefinition);
    if (body == NULL)
    {
        return -1;
    }

    b2FixtureDef fixtureDefinition;
    fixtureDefinition.density = dynamicBody && inverseMass > 0.0f ? 1.0f : 0.0f;
    fixtureDefinition.friction = friction;
    fixtureDefinition.restitution = restitution;
    b2CircleShape circleShape;
    b2PolygonShape rectangleShape;
    if (circle)
    {
        circleShape.m_radius = extentX;
        fixtureDefinition.shape = &circleShape;
    }
    else
    {
        rectangleShape.SetAsBox(extentX, extentY);
        fixtureDefinition.shape = &rectangleShape;
    }
    body->CreateFixture(&fixtureDefinition);

    if (dynamicBody && inverseMass > 0.0f && inverseInertia > 0.0f)
    {
        b2MassData massData;
        massData.mass = 1.0f / inverseMass;
        massData.center.SetZero();
        massData.I = 1.0f / inverseInertia;
        body->SetMassData(&massData);
    }
    if (sleeping)
    {
        body->SetAwake(false);
    }
    oracle->bodies.push_back(body);
    return static_cast<int32_t>(oracle->bodies.size() - 1);
}

} // namespace

extern "C" void* liquidfun_oracle_create(
    float gravityX,
    float gravityY,
    float particleRadius,
    float particleDensity,
    float particleGravityScale,
    std::uint32_t particleCapacity)
{
    if (!(particleRadius > 0.0f) || !(particleDensity > 0.0f) || particleCapacity == 0)
    {
        return NULL;
    }
    OracleWorld* oracle = new (std::nothrow) OracleWorld(
        gravityX,
        gravityY,
        particleRadius,
        particleDensity,
        particleGravityScale,
        particleCapacity);
    if (oracle != NULL && oracle->particles == NULL)
    {
        delete oracle;
        return NULL;
    }
    return oracle;
}

extern "C" void liquidfun_oracle_destroy(void* handle)
{
    delete static_cast<OracleWorld*>(handle);
}

extern "C" int32_t liquidfun_oracle_add_circle(
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

extern "C" int32_t liquidfun_oracle_add_rectangle(
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

extern "C" std::uint32_t liquidfun_oracle_spawn_rectangle(
    void* handle,
    float lowerX,
    float lowerY,
    float upperX,
    float upperY,
    float spacing,
    float velocityX,
    float velocityY)
{
    OracleWorld* oracle = static_cast<OracleWorld*>(handle);
    if (oracle == NULL || oracle->particles == NULL)
    {
        return 0;
    }
    const int32 before = oracle->particles->GetParticleCount();
    b2PolygonShape shape;
    const float halfWidth = 0.5f * (upperX - lowerX);
    const float halfHeight = 0.5f * (upperY - lowerY);
    shape.SetAsBox(halfWidth, halfHeight);
    b2ParticleGroupDef definition;
    definition.flags = b2_waterParticle;
    definition.position.Set(lowerX + halfWidth, lowerY + halfHeight);
    definition.linearVelocity.Set(velocityX, velocityY);
    definition.shape = &shape;
    definition.stride = spacing;
    oracle->particles->CreateParticleGroup(definition);
    const int32 created = oracle->particles->GetParticleCount() - before;
    return created > 0 ? static_cast<std::uint32_t>(created) : 0;
}

extern "C" void liquidfun_oracle_set_gravity(void* handle, float gravityX, float gravityY)
{
    OracleWorld* oracle = static_cast<OracleWorld*>(handle);
    if (oracle != NULL)
    {
        oracle->world.SetGravity(b2Vec2(gravityX, gravityY));
    }
}

extern "C" std::uint32_t liquidfun_oracle_apply_impulse(
    void* handle,
    std::uint32_t bodyIndex,
    float impulseX,
    float impulseY,
    float pointX,
    float pointY)
{
    OracleWorld* oracle = static_cast<OracleWorld*>(handle);
    if (oracle == NULL || bodyIndex >= oracle->bodies.size())
    {
        return 0;
    }
    oracle->bodies[bodyIndex]->ApplyLinearImpulse(
        b2Vec2(impulseX, impulseY),
        b2Vec2(pointX, pointY),
        true);
    return 1;
}

extern "C" std::uint32_t liquidfun_oracle_step(
    void* handle,
    float timeStep,
    std::uint32_t substeps)
{
    OracleWorld* oracle = static_cast<OracleWorld*>(handle);
    if (oracle == NULL || oracle->particles == NULL || !(timeStep > 0.0f))
    {
        return 0;
    }
    const std::uint32_t count = substeps > 0 ? substeps : 1;
    const float substep = timeStep / static_cast<float>(count);
    for (std::uint32_t index = 0; index < count; ++index)
    {
        oracle->world.Step(substep, 8, 3, 1);
    }
    return 1;
}

extern "C" std::uint32_t liquidfun_oracle_particle_count(const void* handle)
{
    const OracleWorld* oracle = static_cast<const OracleWorld*>(handle);
    return oracle == NULL || oracle->particles == NULL
        ? 0
        : static_cast<std::uint32_t>(oracle->particles->GetParticleCount());
}

extern "C" const float* liquidfun_oracle_particle_positions(const void* handle)
{
    const OracleWorld* oracle = static_cast<const OracleWorld*>(handle);
    if (oracle == NULL || oracle->particles == NULL || oracle->particles->GetParticleCount() == 0)
    {
        return NULL;
    }
    return reinterpret_cast<const float*>(oracle->particles->GetPositionBuffer());
}

extern "C" std::uint32_t liquidfun_oracle_body_contact_count(const void* handle)
{
    const OracleWorld* oracle = static_cast<const OracleWorld*>(handle);
    return oracle == NULL || oracle->particles == NULL
        ? 0
        : static_cast<std::uint32_t>(oracle->particles->GetBodyContactCount());
}

extern "C" std::uint32_t liquidfun_oracle_body_state(
    const void* handle,
    std::uint32_t index,
    LiquidFunBodyState* state)
{
    const OracleWorld* oracle = static_cast<const OracleWorld*>(handle);
    if (oracle == NULL || state == NULL || index >= oracle->bodies.size())
    {
        return 0;
    }
    const b2Body* body = oracle->bodies[index];
    const b2Vec2 position = body->GetPosition();
    const b2Vec2 linearVelocity = body->GetLinearVelocity();
    state->position_x = position.x;
    state->position_y = position.y;
    state->angle = body->GetAngle();
    state->linear_velocity_x = linearVelocity.x;
    state->linear_velocity_y = linearVelocity.y;
    state->angular_velocity = body->GetAngularVelocity();
    state->awake = body->IsAwake() ? 1u : 0u;
    return 1;
}
