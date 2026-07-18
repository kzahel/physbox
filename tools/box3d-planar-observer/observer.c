// SPDX-License-Identifier: MIT
// Independent public-API observer for the pinned MIT-licensed Box3D reference.

#include <box3d/box3d.h>

#include <errno.h>
#include <math.h>
#include <stdbool.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#define BOX3D_PIN "8441b4a06d6d09dcfb0b0f704df4d847d1437b92"
#define FIXED_DT ( 1.0f / 60.0f )
#define SUBSTEP_COUNT 4
#define SETTLED_START_STEP 900
#define SLEEP_SPEED_THRESHOLD 0.05
#define TOPPLE_ANGLE_RADIANS 0.7853981633974483
#define PI_RADIANS 3.14159265358979323846
#define MINIMUM_HEIGHT_RETENTION 0.7
#define MAXIMUM_FOOTPRINT_EXPANSION 1.5

typedef enum ScenarioKind
{
	PYRAMID_2D_12,
	LARGE_PYRAMID_90,
} ScenarioKind;

typedef struct BodyRecord
{
	b3BodyId id;
	b3Pos initialPosition;
	float mass;
	float inertiaZ;
} BodyRecord;

typedef struct Scenario
{
	ScenarioKind kind;
	const char* name;
	int bodyCount;
	float halfExtent;
	float groundExtent;
	float initialWidth;
	float initialHeight;
	bool planarLocked;
	BodyRecord* bodies;
} Scenario;

typedef struct ContactStats
{
	int contacts;
	int manifolds;
	int points;
	int persistedPoints;
	int freshPoints;
	double minimumSeparation;
	double maximumSeparation;
	double normalImpulse;
	double totalNormalImpulse;
	double centralFrictionImpulse;
	double twistImpulse;
	double rollingImpulse;
} ContactStats;

typedef struct BodyStats
{
	double kineticEnergy;
	double translationalEnergy;
	double rotationalEnergy;
	double maximumLinearSpeed;
	double maximumAngularSpeed;
	double maximumAngularEdgeSpeed;
	double maximumPlanarDisplacement;
	double maximumAbsoluteZ;
	double heightRetention;
	double footprintExpansion;
	double minimumX;
	double maximumX;
	double minimumY;
	double maximumY;
	double minimumZ;
	double maximumZ;
	int toppledCount;
	int displacedCount;
	int lostCount;
} BodyStats;

typedef struct SettledSummary
{
	int observationCount;
	int nearRestCount;
	int uprightCount;
	int consecutiveEnergyIncreases;
	int maximumConsecutiveEnergyIncreases;
	double firstEnergy;
	double finalEnergy;
	double minimumEnergy;
	double maximumEnergy;
	double previousEnergy;
	bool initialized;
} SettledSummary;

static void Fail( const char* message )
{
	fprintf( stderr, "box3d_planar_observer: %s\n", message );
	exit( EXIT_FAILURE );
}

static void FailErrno( const char* operation )
{
	fprintf( stderr, "box3d_planar_observer: %s: %s\n", operation, strerror( errno ) );
	exit( EXIT_FAILURE );
}

static double Length3( b3Vec3 value )
{
	return sqrt( (double)value.x * value.x + (double)value.y * value.y + (double)value.z * value.z );
}

static double PlanarAngle( b3Quat rotation )
{
	double angle = 2.0 * atan2( rotation.v.z, rotation.s );
	while ( angle > PI_RADIANS )
	{
		angle -= 2.0 * PI_RADIANS;
	}
	while ( angle < -PI_RADIANS )
	{
		angle += 2.0 * PI_RADIANS;
	}
	return angle;
}

static double OrientationDeparture( b3Quat rotation, bool planarLocked )
{
	if ( planarLocked )
	{
		return fabs( PlanarAngle( rotation ) );
	}
	double cosineHalfAngle = fmin( 1.0, fabs( rotation.s ) );
	return 2.0 * acos( cosineHalfAngle );
}

static void CreateGround( b3WorldId worldId, float extent, bool contactRecycling )
{
	b3BodyDef bodyDef = b3DefaultBodyDef();
	bodyDef.position = (b3Pos){ 0.0f, -1.0f, 0.0f };
	bodyDef.enableContactRecycling = contactRecycling;
	b3BodyId groundId = b3CreateBody( worldId, &bodyDef );
	b3ShapeDef shapeDef = b3DefaultShapeDef();
	b3BoxHull ground = b3MakeBoxHull( extent, 1.0f, extent );
	b3CreateHullShape( groundId, &shapeDef, &ground.base );
}

static void StoreBody( Scenario* scenario, int index, b3BodyId bodyId, b3Pos initialPosition )
{
	if ( index < 0 || index >= scenario->bodyCount )
	{
		Fail( "body index exceeds frozen source arithmetic" );
	}
	b3Matrix3 inertia = b3Body_GetLocalRotationalInertia( bodyId );
	scenario->bodies[index] = (BodyRecord){
		.id = bodyId,
		.initialPosition = initialPosition,
		.mass = b3Body_GetMass( bodyId ),
		.inertiaZ = inertia.cz.z,
	};
}

static Scenario CreatePyramid2D12( b3WorldId worldId, bool contactRecycling )
{
	Scenario scenario = {
		.kind = PYRAMID_2D_12,
		.name = "pyramid2d-12",
		.bodyCount = 78,
		.halfExtent = 1.0f,
		.groundExtent = 40.0f,
		.initialWidth = 22.0f,
		.initialHeight = 30.0f,
		.planarLocked = true,
	};
	scenario.bodies = calloc( (size_t)scenario.bodyCount, sizeof( BodyRecord ) );
	if ( scenario.bodies == NULL )
	{
		FailErrno( "allocating Pyramid2D body records" );
	}

	CreateGround( worldId, scenario.groundExtent, contactRecycling );
	b3BoxHull box = b3MakeBoxHull( 1.0f, 1.0f, 1.0f );
	b3BodyDef bodyDef = b3DefaultBodyDef();
	bodyDef.type = b3_dynamicBody;
	bodyDef.enableSleep = false;
	bodyDef.enableContactRecycling = contactRecycling;
	bodyDef.motionLocks.linearZ = true;
	bodyDef.motionLocks.angularX = true;
	bodyDef.motionLocks.angularY = true;
	b3ShapeDef shapeDef = b3DefaultShapeDef();

	int index = 0;
	for ( int row = 0; row < 12; ++row )
	{
		for ( int column = 0; column < 12 - row; ++column )
		{
			b3Pos position = { -10.0f + 2.0f * column + row, 1.5f + 2.5f * row, 0.0f };
			bodyDef.position = position;
			b3BodyId bodyId = b3CreateBody( worldId, &bodyDef );
			b3CreateHullShape( bodyId, &shapeDef, &box.base );
			StoreBody( &scenario, index, bodyId, position );
			index += 1;
		}
	}
	if ( index != scenario.bodyCount )
	{
		Fail( "Pyramid2D body arithmetic changed" );
	}
	return scenario;
}

static Scenario CreateLargePyramid90( b3WorldId worldId, bool contactRecycling )
{
	Scenario scenario = {
		.kind = LARGE_PYRAMID_90,
		.name = "large-pyramid-90",
		.bodyCount = 4095,
		.halfExtent = 0.5f,
		.groundExtent = 400.0f,
		.initialWidth = 90.0f,
		.initialHeight = 90.0f,
		.planarLocked = false,
	};
	scenario.bodies = calloc( (size_t)scenario.bodyCount, sizeof( BodyRecord ) );
	if ( scenario.bodies == NULL )
	{
		FailErrno( "allocating large-pyramid body records" );
	}

	CreateGround( worldId, scenario.groundExtent, contactRecycling );
	b3BodyDef bodyDef = b3DefaultBodyDef();
	bodyDef.type = b3_dynamicBody;
	bodyDef.enableSleep = false;
	bodyDef.enableContactRecycling = contactRecycling;
	b3ShapeDef shapeDef = b3DefaultShapeDef();
	shapeDef.density = 100.0f;
	b3BoxHull box = b3MakeBoxHull( 0.5f, 0.5f, 0.5f );

	int index = 0;
	for ( int row = 0; row < 90; ++row )
	{
		float y = ( 2.0f * row + 1.0f ) * 0.5f;
		for ( int column = row; column < 90; ++column )
		{
			float x = ( row + 1.0f ) * 0.5f + 2.0f * ( column - row ) * 0.5f - 45.0f;
			b3Pos position = { x, y, 0.0f };
			bodyDef.position = position;
			b3BodyId bodyId = b3CreateBody( worldId, &bodyDef );
			b3CreateHullShape( bodyId, &shapeDef, &box.base );
			StoreBody( &scenario, index, bodyId, position );
			index += 1;
		}
	}
	if ( index != scenario.bodyCount )
	{
		Fail( "large-pyramid body arithmetic changed" );
	}
	return scenario;
}

static bool OwnsContactRecord( b3BodyId currentBody, const b3ContactData* contact )
{
	b3BodyId bodyA = b3Shape_GetBody( contact->shapeIdA );
	if ( B3_ID_EQUALS( currentBody, bodyA ) )
	{
		return true;
	}
	return b3Body_GetType( bodyA ) == b3_staticBody;
}

static ContactStats MeasureContacts( const Scenario* scenario )
{
	ContactStats stats = {
		.minimumSeparation = INFINITY,
		.maximumSeparation = -INFINITY,
	};
	b3ContactData* data = NULL;
	int allocation = 0;

	for ( int bodyIndex = 0; bodyIndex < scenario->bodyCount; ++bodyIndex )
	{
		b3BodyId bodyId = scenario->bodies[bodyIndex].id;
		int capacity = b3Body_GetContactCapacity( bodyId );
		if ( capacity > allocation )
		{
			b3ContactData* replacement = realloc( data, (size_t)capacity * sizeof( b3ContactData ) );
			if ( replacement == NULL )
			{
				free( data );
				FailErrno( "allocating contact observation buffer" );
			}
			data = replacement;
			allocation = capacity;
		}
		int count = capacity > 0 ? b3Body_GetContactData( bodyId, data, capacity ) : 0;
		for ( int contactIndex = 0; contactIndex < count; ++contactIndex )
		{
			const b3ContactData* contact = data + contactIndex;
			if ( OwnsContactRecord( bodyId, contact ) == false )
			{
				continue;
			}
			stats.contacts += 1;
			stats.manifolds += contact->manifoldCount;
			for ( int manifoldIndex = 0; manifoldIndex < contact->manifoldCount; ++manifoldIndex )
			{
				const b3Manifold* manifold = contact->manifolds + manifoldIndex;
				stats.centralFrictionImpulse += Length3( manifold->frictionImpulse );
				stats.twistImpulse += fabs( manifold->twistImpulse );
				stats.rollingImpulse += Length3( manifold->rollingImpulse );
				stats.points += manifold->pointCount;
				for ( int pointIndex = 0; pointIndex < manifold->pointCount; ++pointIndex )
				{
					const b3ManifoldPoint* point = manifold->points + pointIndex;
					stats.persistedPoints += point->persisted ? 1 : 0;
					stats.freshPoints += point->persisted ? 0 : 1;
					stats.minimumSeparation = fmin( stats.minimumSeparation, point->separation );
					stats.maximumSeparation = fmax( stats.maximumSeparation, point->separation );
					stats.normalImpulse += fabs( point->normalImpulse );
					stats.totalNormalImpulse += fabs( point->totalNormalImpulse );
				}
			}
		}
	}
	free( data );
	if ( stats.points == 0 )
	{
		stats.minimumSeparation = 0.0;
		stats.maximumSeparation = 0.0;
	}
	return stats;
}

static BodyStats MeasureBodies( const Scenario* scenario )
{
	BodyStats stats = {
		.minimumX = INFINITY,
		.maximumX = -INFINITY,
		.minimumY = INFINITY,
		.maximumY = -INFINITY,
		.minimumZ = INFINITY,
		.maximumZ = -INFINITY,
	};
	double fullWidth = 2.0 * scenario->halfExtent;
	double edgeRadius = sqrt( 2.0 ) * scenario->halfExtent;

	for ( int index = 0; index < scenario->bodyCount; ++index )
	{
		const BodyRecord* body = scenario->bodies + index;
		b3Pos position = b3Body_GetPosition( body->id );
		b3Quat rotation = b3Body_GetRotation( body->id );
		b3Vec3 linearVelocity = b3Body_GetLinearVelocity( body->id );
		b3Vec3 angularVelocity = b3Body_GetAngularVelocity( body->id );
		double linearSpeed = Length3( linearVelocity );
		double angularSpeed = Length3( angularVelocity );
		double translationalEnergy = 0.5 * body->mass * linearSpeed * linearSpeed;
		double rotationalEnergy = 0.5 * body->inertiaZ * angularVelocity.z * angularVelocity.z;
		double dx = position.x - body->initialPosition.x;
		double dy = position.y - body->initialPosition.y;
		double dz = position.z - body->initialPosition.z;
		double displacement = scenario->planarLocked ? sqrt( dx * dx + dy * dy ) : sqrt( dx * dx + dy * dy + dz * dz );
		double orientationDeparture = OrientationDeparture( rotation, scenario->planarLocked );

		stats.translationalEnergy += translationalEnergy;
		stats.rotationalEnergy += rotationalEnergy;
		stats.maximumLinearSpeed = fmax( stats.maximumLinearSpeed, linearSpeed );
		stats.maximumAngularSpeed = fmax( stats.maximumAngularSpeed, angularSpeed );
		stats.maximumAngularEdgeSpeed = fmax( stats.maximumAngularEdgeSpeed, fabs( angularVelocity.z ) * edgeRadius );
		stats.maximumPlanarDisplacement = fmax( stats.maximumPlanarDisplacement, displacement );
		stats.maximumAbsoluteZ = fmax( stats.maximumAbsoluteZ, fabs( position.z ) );
		stats.minimumX = fmin( stats.minimumX, position.x );
		stats.maximumX = fmax( stats.maximumX, position.x );
		stats.minimumY = fmin( stats.minimumY, position.y );
		stats.maximumY = fmax( stats.maximumY, position.y );
		stats.minimumZ = fmin( stats.minimumZ, position.z );
		stats.maximumZ = fmax( stats.maximumZ, position.z );
		stats.toppledCount += orientationDeparture > TOPPLE_ANGLE_RADIANS ? 1 : 0;
		stats.displacedCount += displacement > fullWidth ? 1 : 0;
		stats.lostCount += position.y < -1.0 || fabs( position.x ) > scenario->groundExtent ||
			fabs( position.z ) > scenario->groundExtent ? 1 : 0;
	}
	stats.kineticEnergy = stats.translationalEnergy + stats.rotationalEnergy;
	stats.heightRetention = ( stats.maximumY + scenario->halfExtent ) / scenario->initialHeight;
	stats.footprintExpansion = ( stats.maximumX - stats.minimumX + 2.0 * scenario->halfExtent ) / scenario->initialWidth;
	return stats;
}

static bool IsCheckpoint( int step, int finalStep )
{
	return step == 0 || step == 200 || step == 1200 || step == 5000 || step == finalStep;
}

static bool ShouldObserve( const Scenario* scenario, int step, int finalStep )
{
	if ( IsCheckpoint( step, finalStep ) )
	{
		return true;
	}
	return scenario->kind == PYRAMID_2D_12 && step >= SETTLED_START_STEP;
}

static void WriteCounters( FILE* output, b3Counters counters )
{
	fprintf( output,
		"\"counters\":{\"bodies\":%d,\"shapes\":%d,\"contacts\":%d,\"awakeContacts\":%d,"
		"\"islands\":%d,\"satCalls\":%d,\"satCacheHits\":%d,\"recycledContacts\":%d,\"colors\":[",
		counters.bodyCount, counters.shapeCount, counters.contactCount, counters.awakeContactCount, counters.islandCount,
		counters.satCallCount, counters.satCacheHitCount, counters.recycledContactCount );
	for ( int index = 0; index < 24; ++index )
	{
		fprintf( output, "%s%d", index == 0 ? "" : ",", counters.colorCounts[index] );
	}
	fputs( "]}", output );
}

static void WriteBodyStats( FILE* output, const Scenario* scenario, BodyStats stats )
{
	bool nearRest = stats.maximumLinearSpeed <= SLEEP_SPEED_THRESHOLD && stats.maximumAngularEdgeSpeed <= SLEEP_SPEED_THRESHOLD;
	bool outOfPlaneRetained = scenario->planarLocked || stats.maximumAbsoluteZ <= 2.0 * scenario->halfExtent;
	bool upright = stats.lostCount == 0 && stats.heightRetention >= MINIMUM_HEIGHT_RETENTION &&
		stats.footprintExpansion <= MAXIMUM_FOOTPRINT_EXPANSION && outOfPlaneRetained;
	fprintf( output,
		"\"body\":{\"kineticEnergy\":%.17g,\"translationalEnergy\":%.17g,\"rotationalEnergy\":%.17g,"
		"\"maxLinearSpeed\":%.17g,\"maxAngularSpeed\":%.17g,\"maxAngularEdgeSpeed\":%.17g,"
		"\"maxCenterDisplacement\":%.17g,\"maxAbsZ\":%.17g,\"heightRetention\":%.17g,"
		"\"footprintExpansion\":%.17g,\"bounds\":[%.17g,%.17g,%.17g,%.17g,%.17g,%.17g],"
		"\"toppled\":%d,\"displaced\":%d,\"lost\":%d,\"nearRest\":%s,\"uprightStructure\":%s}",
		stats.kineticEnergy, stats.translationalEnergy, stats.rotationalEnergy, stats.maximumLinearSpeed,
		stats.maximumAngularSpeed, stats.maximumAngularEdgeSpeed, stats.maximumPlanarDisplacement, stats.maximumAbsoluteZ,
		stats.heightRetention, stats.footprintExpansion, stats.minimumX, stats.maximumX, stats.minimumY, stats.maximumY,
		stats.minimumZ, stats.maximumZ, stats.toppledCount, stats.displacedCount, stats.lostCount,
		nearRest ? "true" : "false", upright ? "true" : "false" );
}

static void WriteContactStats( FILE* output, ContactStats stats )
{
	fprintf( output,
		"\"contact\":{\"contacts\":%d,\"manifolds\":%d,\"points\":%d,\"persistedPoints\":%d,"
		"\"freshPoints\":%d,\"minSeparation\":%.17g,\"maxSeparation\":%.17g,\"normalImpulse\":%.17g,"
		"\"totalNormalImpulse\":%.17g,\"centralFrictionImpulse\":%.17g,\"twistImpulse\":%.17g,"
		"\"rollingImpulse\":%.17g}",
		stats.contacts, stats.manifolds, stats.points, stats.persistedPoints, stats.freshPoints, stats.minimumSeparation,
		stats.maximumSeparation, stats.normalImpulse, stats.totalNormalImpulse, stats.centralFrictionImpulse,
		stats.twistImpulse, stats.rollingImpulse );
}

static void WriteBodyStates( FILE* output, const Scenario* scenario )
{
	fputs( "\"bodies\":[", output );
	for ( int index = 0; index < scenario->bodyCount; ++index )
	{
		const BodyRecord* body = scenario->bodies + index;
		b3Pos position = b3Body_GetPosition( body->id );
		b3Quat rotation = b3Body_GetRotation( body->id );
		b3Vec3 linearVelocity = b3Body_GetLinearVelocity( body->id );
		b3Vec3 angularVelocity = b3Body_GetAngularVelocity( body->id );
		fprintf( output,
			"%s{\"index\":%d,\"position\":[%.17g,%.17g,%.17g],\"angle\":%.17g,"
			"\"linearVelocity\":[%.17g,%.17g,%.17g],\"angularVelocity\":[%.17g,%.17g,%.17g]}",
			index == 0 ? "" : ",", index, (double)position.x, (double)position.y, (double)position.z,
			PlanarAngle( rotation ), (double)linearVelocity.x, (double)linearVelocity.y, (double)linearVelocity.z,
			(double)angularVelocity.x, (double)angularVelocity.y, (double)angularVelocity.z );
	}
	fputc( ']', output );
}

static void UpdateSettledSummary( SettledSummary* summary, const Scenario* scenario, int step, BodyStats stats )
{
	if ( step < SETTLED_START_STEP )
	{
		return;
	}
	bool nearRest = stats.maximumLinearSpeed <= SLEEP_SPEED_THRESHOLD && stats.maximumAngularEdgeSpeed <= SLEEP_SPEED_THRESHOLD;
	bool outOfPlaneRetained = scenario->planarLocked || stats.maximumAbsoluteZ <= 2.0 * scenario->halfExtent;
	bool upright = stats.lostCount == 0 && stats.heightRetention >= MINIMUM_HEIGHT_RETENTION &&
		stats.footprintExpansion <= MAXIMUM_FOOTPRINT_EXPANSION && outOfPlaneRetained;
	if ( summary->initialized == false )
	{
		summary->initialized = true;
		summary->firstEnergy = stats.kineticEnergy;
		summary->minimumEnergy = stats.kineticEnergy;
		summary->maximumEnergy = stats.kineticEnergy;
	}
	else if ( stats.kineticEnergy > summary->previousEnergy )
	{
		summary->consecutiveEnergyIncreases += 1;
		if ( summary->consecutiveEnergyIncreases > summary->maximumConsecutiveEnergyIncreases )
		{
			summary->maximumConsecutiveEnergyIncreases = summary->consecutiveEnergyIncreases;
		}
	}
	else
	{
		summary->consecutiveEnergyIncreases = 0;
	}
	summary->observationCount += 1;
	summary->nearRestCount += nearRest ? 1 : 0;
	summary->uprightCount += upright ? 1 : 0;
	summary->finalEnergy = stats.kineticEnergy;
	summary->minimumEnergy = fmin( summary->minimumEnergy, stats.kineticEnergy );
	summary->maximumEnergy = fmax( summary->maximumEnergy, stats.kineticEnergy );
	summary->previousEnergy = stats.kineticEnergy;
}

static void WriteObservation( FILE* output, b3WorldId worldId, const Scenario* scenario, int step, int finalStep,
	bool first, SettledSummary* summary )
{
	BodyStats bodyStats = MeasureBodies( scenario );
	ContactStats contactStats = MeasureContacts( scenario );
	UpdateSettledSummary( summary, scenario, step, bodyStats );
	fprintf( output, "%s{\"step\":%d,", first ? "" : ",", step );
	WriteBodyStats( output, scenario, bodyStats );
	fputc( ',', output );
	WriteContactStats( output, contactStats );
	fputc( ',', output );
	WriteCounters( output, b3World_GetCounters( worldId ) );
	if ( IsCheckpoint( step, finalStep ) )
	{
		fputc( ',', output );
		WriteBodyStates( output, scenario );
	}
	fputc( '}', output );
}

static void WriteSummary( FILE* output, SettledSummary summary )
{
	fprintf( output,
		"\"settledWindow\":{\"startStep\":%d,\"observations\":%d,\"nearRestObservations\":%d,"
		"\"uprightObservations\":%d,\"firstEnergy\":%.17g,\"finalEnergy\":%.17g,\"minEnergy\":%.17g,"
		"\"maxEnergy\":%.17g,\"maxConsecutiveEnergyIncreases\":%d}",
		SETTLED_START_STEP, summary.observationCount, summary.nearRestCount, summary.uprightCount, summary.firstEnergy,
		summary.finalEnergy, summary.minimumEnergy, summary.maximumEnergy, summary.maximumConsecutiveEnergyIncreases );
}

static ScenarioKind ParseScenario( const char* value )
{
	if ( strcmp( value, "pyramid2d-12" ) == 0 )
	{
		return PYRAMID_2D_12;
	}
	if ( strcmp( value, "large-pyramid-90" ) == 0 )
	{
		return LARGE_PYRAMID_90;
	}
	Fail( "--scenario must be pyramid2d-12 or large-pyramid-90" );
	return PYRAMID_2D_12;
}

static int ParseSteps( const char* value )
{
	char* end = NULL;
	errno = 0;
	long parsed = strtol( value, &end, 10 );
	if ( errno != 0 || end == value || *end != '\0' || parsed < 0 || parsed > 1000000 )
	{
		Fail( "--steps must be an integer from 0 through 1000000" );
	}
	return (int)parsed;
}

static bool ParseContactRecycling( const char* value )
{
	if ( strcmp( value, "on" ) == 0 )
	{
		return true;
	}
	if ( strcmp( value, "off" ) == 0 )
	{
		return false;
	}
	Fail( "--contact-recycling must be on or off" );
	return true;
}

int main( int argc, char** argv )
{
	const char* scenarioArgument = NULL;
	const char* outputPath = NULL;
	int steps = 5000;
	bool contactRecycling = true;
	for ( int index = 1; index < argc; ++index )
	{
		if ( strcmp( argv[index], "--scenario" ) == 0 && index + 1 < argc )
		{
			scenarioArgument = argv[++index];
		}
		else if ( strcmp( argv[index], "--steps" ) == 0 && index + 1 < argc )
		{
			steps = ParseSteps( argv[++index] );
		}
		else if ( strcmp( argv[index], "--output" ) == 0 && index + 1 < argc )
		{
			outputPath = argv[++index];
		}
		else if ( strcmp( argv[index], "--contact-recycling" ) == 0 && index + 1 < argc )
		{
			contactRecycling = ParseContactRecycling( argv[++index] );
		}
		else
		{
			Fail( "usage: --scenario NAME [--steps COUNT] [--contact-recycling on|off] --output PATH" );
		}
	}
	if ( scenarioArgument == NULL || outputPath == NULL )
	{
		Fail( "usage: --scenario NAME [--steps COUNT] [--contact-recycling on|off] --output PATH" );
	}

	ScenarioKind kind = ParseScenario( scenarioArgument );
	b3WorldDef worldDef = b3DefaultWorldDef();
	worldDef.workerCount = 1;
	worldDef.enableSleep = false;
	b3WorldId worldId = b3CreateWorld( &worldDef );
	b3World_EnableSleeping( worldId, false );
	b3World_EnableWarmStarting( worldId, true );
	Scenario scenario = kind == PYRAMID_2D_12 ? CreatePyramid2D12( worldId, contactRecycling ) :
											 CreateLargePyramid90( worldId, contactRecycling );

	size_t temporaryLength = strlen( outputPath ) + 5;
	char* temporaryPath = malloc( temporaryLength );
	if ( temporaryPath == NULL )
	{
		FailErrno( "allocating temporary output path" );
	}
	snprintf( temporaryPath, temporaryLength, "%s.tmp", outputPath );
	FILE* output = fopen( temporaryPath, "wb" );
	if ( output == NULL )
	{
		FailErrno( "opening temporary output" );
	}

	fprintf( output,
		"{\"schema\":\"physbox5.box3d-planar-observer/v2\",\"source\":{\"engine\":\"Box3D\","
		"\"revision\":\"%s\",\"role\":\"source-reference\"},\"contract\":{\"scenario\":\"%s\","
		"\"steps\":%d,\"dt\":%.17g,\"substeps\":%d,\"workers\":1,\"sleeping\":false,"
		"\"warmStarting\":true,\"contactRecycling\":%s,\"simd\":true,\"validation\":true,\"planarLocked\":%s,"
		"\"dynamicBodies\":%d,\"settledStartStep\":%d,\"checkpoints\":[0,200,1200,5000],"
		"\"nearRestSpeed\":%.17g,\"toppleAngleRadians\":%.17g,\"displacementWidth\":%.17g,"
		"\"minimumHeightRetention\":%.17g,\"maximumFootprintExpansion\":%.17g,\"maximumOutOfPlaneCenters\":%.17g},"
		"\"observations\":[",
		BOX3D_PIN, scenario.name, steps, (double)FIXED_DT, SUBSTEP_COUNT, contactRecycling ? "true" : "false",
		scenario.planarLocked ? "true" : "false", scenario.bodyCount, SETTLED_START_STEP, SLEEP_SPEED_THRESHOLD,
		TOPPLE_ANGLE_RADIANS, 2.0 * scenario.halfExtent,
		MINIMUM_HEIGHT_RETENTION, MAXIMUM_FOOTPRINT_EXPANSION, 2.0 * scenario.halfExtent );

	SettledSummary summary = { 0 };
	bool firstObservation = true;
	for ( int step = 0; step <= steps; ++step )
	{
		if ( ShouldObserve( &scenario, step, steps ) )
		{
			WriteObservation( output, worldId, &scenario, step, steps, firstObservation, &summary );
			firstObservation = false;
		}
		if ( step < steps )
		{
			b3World_Step( worldId, FIXED_DT, SUBSTEP_COUNT );
		}
	}
	fputs( "],", output );
	WriteSummary( output, summary );
	fputs( "}\n", output );

	if ( fflush( output ) != 0 || fclose( output ) != 0 )
	{
		FailErrno( "flushing observer output" );
	}
	if ( rename( temporaryPath, outputPath ) != 0 )
	{
		FailErrno( "publishing observer output" );
	}

	free( temporaryPath );
	free( scenario.bodies );
	b3DestroyWorld( worldId );
	fprintf( stderr,
		"PHYSBOX5_VALIDATION_SUCCESS source=box3d scenario=%s steps=%d contact_recycling=%s output=%s\n",
		scenario.name, steps, contactRecycling ? "on" : "off", outputPath );
	return EXIT_SUCCESS;
}
