use physbox_contracts::{CapacityRecordV1, ContractError, ProvenanceV1, SCHEMA_VERSION, Validate};
use serde::{Deserialize, Serialize};

use crate::{
    LIQUID_SCALE_BENCHMARK_VERSION, LIQUID_SCALE_FIXED_RESERVE_CAPACITY,
    LIQUID_SCALE_FIXED_RESERVE_LIVE_COUNTS, LIQUID_SCALE_PARTICLE_COUNTS, LiquidScaleSolidTier,
};

pub const LIQUID_SCALE_ARTIFACT_VERSION: u32 = 1;
pub const LIQUID_SCALE_PERCENTILE_METHOD: &str = "nearest-rank-ceil-v1";
pub const LIQUID_SCALE_FIXED_DT_SECONDS: f64 = 1.0 / 60.0;
pub const LIQUID_SCALE_RIGID_SUBSTEPS: u32 = 4;
pub const LIQUID_SCALE_RIGID_ITERATIONS: u32 = 1;
pub const LIQUID_SCALE_PARTICLE_SUBSTEPS: u32 = 2;
pub const LIQUID_SCALE_PRESSURE_ITERATIONS: u32 = 2;
pub const LIQUID_SCALE_COUPLING_ITERATIONS: u32 = 1;
pub const LIQUID_SCALE_SUCCESS_SENTINEL: &str = "PHYSBOX5_LIQUID_SCALE_SUCCESS";

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum LiquidScaleTimingModeV1 {
    PhysicsOnlyExplicitSteps,
    FullFrameInteractive,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum LiquidScaleHostKindV1 {
    NativeDesktop,
    BrowserWebGpu,
    AndroidNative,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum LiquidScaleCouplingQualityV1 {
    Standard,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LiquidScaleBenchmarkConfigV1 {
    pub version: u32,
    pub timing_mode: LiquidScaleTimingModeV1,
    pub solid_tier: LiquidScaleSolidTier,
    pub particle_count: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub particle_capacity: Option<u32>,
    pub warmup_steps: u32,
    pub retained_samples: u32,
    pub outer_steps_per_sample: u32,
    pub outer_dt_seconds: f64,
    pub rigid_substeps: u32,
    pub rigid_iterations: u32,
    pub particle_substeps: u32,
    pub pressure_iterations: u32,
    pub coupling_iterations: u32,
    pub coupling_quality: LiquidScaleCouplingQualityV1,
    pub render_extent: Option<[u32; 2]>,
    pub render_scale: Option<f64>,
    pub interactive_duration_seconds: Option<f64>,
}

impl LiquidScaleBenchmarkConfigV1 {
    pub fn physics_only(
        solid_tier: LiquidScaleSolidTier,
        particle_count: u32,
        warmup_steps: u32,
        retained_samples: u32,
    ) -> Self {
        Self {
            version: LIQUID_SCALE_BENCHMARK_VERSION,
            timing_mode: LiquidScaleTimingModeV1::PhysicsOnlyExplicitSteps,
            solid_tier,
            particle_count,
            particle_capacity: None,
            warmup_steps,
            retained_samples,
            outer_steps_per_sample: 1,
            outer_dt_seconds: LIQUID_SCALE_FIXED_DT_SECONDS,
            rigid_substeps: LIQUID_SCALE_RIGID_SUBSTEPS,
            rigid_iterations: LIQUID_SCALE_RIGID_ITERATIONS,
            particle_substeps: LIQUID_SCALE_PARTICLE_SUBSTEPS,
            pressure_iterations: LIQUID_SCALE_PRESSURE_ITERATIONS,
            coupling_iterations: LIQUID_SCALE_COUPLING_ITERATIONS,
            coupling_quality: LiquidScaleCouplingQualityV1::Standard,
            render_extent: None,
            render_scale: None,
            interactive_duration_seconds: None,
        }
    }

    pub fn with_particle_capacity(mut self, particle_capacity: u32) -> Self {
        self.particle_capacity =
            (particle_capacity != self.particle_count).then_some(particle_capacity);
        self
    }

    pub fn resolved_particle_capacity(&self) -> u32 {
        self.particle_capacity.unwrap_or(self.particle_count)
    }
}

impl Validate for LiquidScaleBenchmarkConfigV1 {
    fn validate(&self) -> Result<(), ContractError> {
        let particle_capacity = self.resolved_particle_capacity();
        let standard_population = self.particle_capacity.is_none()
            && LIQUID_SCALE_PARTICLE_COUNTS.contains(&self.particle_count);
        let fixed_reserve_population = particle_capacity == LIQUID_SCALE_FIXED_RESERVE_CAPACITY
            && LIQUID_SCALE_FIXED_RESERVE_LIVE_COUNTS.contains(&self.particle_count);
        if self.version != LIQUID_SCALE_BENCHMARK_VERSION
            || (!standard_population && !fixed_reserve_population)
            || self.warmup_steps == 0
            || self.retained_samples == 0
            || self.outer_steps_per_sample != 1
            || !self.outer_dt_seconds.is_finite()
            || self.outer_dt_seconds.to_bits() != LIQUID_SCALE_FIXED_DT_SECONDS.to_bits()
            || self.rigid_substeps != LIQUID_SCALE_RIGID_SUBSTEPS
            || self.rigid_iterations != LIQUID_SCALE_RIGID_ITERATIONS
            || self.particle_substeps != LIQUID_SCALE_PARTICLE_SUBSTEPS
            || self.pressure_iterations != LIQUID_SCALE_PRESSURE_ITERATIONS
            || self.coupling_iterations != LIQUID_SCALE_COUPLING_ITERATIONS
            || self.coupling_quality != LiquidScaleCouplingQualityV1::Standard
        {
            return Err(ContractError(
                "liquid scale configuration changes the versioned count or quality contract"
                    .to_owned(),
            ));
        }
        match self.timing_mode {
            LiquidScaleTimingModeV1::PhysicsOnlyExplicitSteps => {
                if self.render_extent.is_some()
                    || self.render_scale.is_some()
                    || self.interactive_duration_seconds.is_some()
                {
                    return Err(ContractError(
                        "physics-only liquid timing cannot carry presentation settings".to_owned(),
                    ));
                }
            }
            LiquidScaleTimingModeV1::FullFrameInteractive => {
                let Some(extent) = self.render_extent else {
                    return Err(ContractError(
                        "full-frame liquid timing requires a render extent".to_owned(),
                    ));
                };
                let Some(scale) = self.render_scale else {
                    return Err(ContractError(
                        "full-frame liquid timing requires a render scale".to_owned(),
                    ));
                };
                let Some(duration) = self.interactive_duration_seconds else {
                    return Err(ContractError(
                        "full-frame liquid timing requires a measured duration".to_owned(),
                    ));
                };
                if extent.into_iter().any(|value| value == 0)
                    || !scale.is_finite()
                    || !(0.0..=1.0).contains(&scale)
                    || scale == 0.0
                    || !duration.is_finite()
                    || duration <= 0.0
                {
                    return Err(ContractError(
                        "full-frame liquid presentation settings must be finite and positive"
                            .to_owned(),
                    ));
                }
            }
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LiquidScaleTimingSeriesV1 {
    pub sample_count: u32,
    pub zero_sample_count: u32,
    pub p50_seconds: f64,
    pub p95_seconds: f64,
    pub p99_seconds: f64,
    pub raw_seconds: Vec<f64>,
}

impl LiquidScaleTimingSeriesV1 {
    pub fn host(raw_seconds: Vec<f64>) -> Result<Self, ContractError> {
        Self::from_raw(raw_seconds, false)
    }

    pub fn gpu(raw_seconds: Vec<f64>) -> Result<Self, ContractError> {
        Self::from_raw(raw_seconds, true)
    }

    fn from_raw(raw_seconds: Vec<f64>, allow_zero: bool) -> Result<Self, ContractError> {
        validate_raw_timing(&raw_seconds, allow_zero)?;
        let mut sorted = raw_seconds.clone();
        sorted.sort_by(f64::total_cmp);
        Ok(Self {
            sample_count: raw_seconds.len() as u32,
            zero_sample_count: raw_seconds.iter().filter(|sample| **sample == 0.0).count() as u32,
            p50_seconds: percentile(&sorted, 50),
            p95_seconds: percentile(&sorted, 95),
            p99_seconds: percentile(&sorted, 99),
            raw_seconds,
        })
    }

    fn validate_for_clock(&self, allow_zero: bool) -> Result<(), ContractError> {
        validate_raw_timing(&self.raw_seconds, allow_zero)?;
        if self.sample_count != self.raw_seconds.len() as u32
            || self.zero_sample_count
                != self
                    .raw_seconds
                    .iter()
                    .filter(|sample| **sample == 0.0)
                    .count() as u32
        {
            return Err(ContractError(
                "liquid timing sample counters do not match the raw series".to_owned(),
            ));
        }
        let mut sorted = self.raw_seconds.clone();
        sorted.sort_by(f64::total_cmp);
        for (actual, expected) in [
            (self.p50_seconds, percentile(&sorted, 50)),
            (self.p95_seconds, percentile(&sorted, 95)),
            (self.p99_seconds, percentile(&sorted, 99)),
        ] {
            if !actual.is_finite() || actual.to_bits() != expected.to_bits() {
                return Err(ContractError(
                    "liquid timing percentiles do not match nearest-rank raw samples".to_owned(),
                ));
            }
        }
        Ok(())
    }
}

fn validate_raw_timing(raw_seconds: &[f64], allow_zero: bool) -> Result<(), ContractError> {
    if raw_seconds.is_empty()
        || raw_seconds
            .iter()
            .any(|sample| !sample.is_finite() || *sample < 0.0 || (!allow_zero && *sample == 0.0))
    {
        return Err(ContractError(
            "liquid timing samples must be finite and positive except quantized GPU zeros"
                .to_owned(),
        ));
    }
    Ok(())
}

fn percentile(sorted: &[f64], percentile: usize) -> f64 {
    let index = sorted
        .len()
        .saturating_mul(percentile)
        .div_ceil(100)
        .saturating_sub(1);
    sorted[index]
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum LiquidScaleGpuStageV1 {
    Rigid,
    ParticleIntegrationAndBoundary,
    ParticleGrid,
    ParticleDensityAndCorrection,
    ParticleBodyReaction,
    ParticleVelocity,
    ParticleCompaction,
    LiquidTotal,
    PhysicsTotal,
    Render,
    FullFrame,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LiquidScaleGpuStageTimingV1 {
    pub stage: LiquidScaleGpuStageV1,
    pub series: LiquidScaleTimingSeriesV1,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum LiquidScaleGpuTimestampStatusV1 {
    Unsupported,
    Valid,
    QuantizedZeros,
    AdvertisedUnusableZero,
    AdvertisedUnusableOrder,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LiquidScaleTimingsV1 {
    pub percentile_method: String,
    pub gpu_timestamp_status: LiquidScaleGpuTimestampStatusV1,
    pub timestamp_quantization_seconds: Option<f64>,
    pub gpu_stages: Vec<LiquidScaleGpuStageTimingV1>,
    pub rejected_gpu_timestamp_raw_seconds: Vec<f64>,
    pub cpu_encode_submit: LiquidScaleTimingSeriesV1,
    pub host_completion: LiquidScaleTimingSeriesV1,
}

impl LiquidScaleTimingsV1 {
    fn validate(&self, expected_samples: u32) -> Result<(), ContractError> {
        if self.percentile_method != LIQUID_SCALE_PERCENTILE_METHOD {
            return Err(ContractError(
                "liquid timing percentile method is unsupported".to_owned(),
            ));
        }
        self.cpu_encode_submit.validate_for_clock(false)?;
        self.host_completion.validate_for_clock(false)?;
        if self.cpu_encode_submit.sample_count != expected_samples
            || self.host_completion.sample_count != expected_samples
        {
            return Err(ContractError(
                "liquid host timing series does not match the retained sample count".to_owned(),
            ));
        }
        if let Some(quantization) = self.timestamp_quantization_seconds
            && (!quantization.is_finite() || quantization <= 0.0)
        {
            return Err(ContractError(
                "liquid GPU timestamp quantization must be finite and positive".to_owned(),
            ));
        }
        if self
            .gpu_stages
            .windows(2)
            .any(|window| window[0].stage >= window[1].stage)
        {
            return Err(ContractError(
                "liquid GPU timing stages must be sorted and unique".to_owned(),
            ));
        }
        let allow_zero =
            self.gpu_timestamp_status == LiquidScaleGpuTimestampStatusV1::QuantizedZeros;
        for stage in &self.gpu_stages {
            stage.series.validate_for_clock(allow_zero)?;
            if stage.series.sample_count != expected_samples {
                return Err(ContractError(
                    "liquid GPU timing series does not match the retained sample count".to_owned(),
                ));
            }
        }
        match self.gpu_timestamp_status {
            LiquidScaleGpuTimestampStatusV1::Unsupported => {
                if !self.gpu_stages.is_empty()
                    || !self.rejected_gpu_timestamp_raw_seconds.is_empty()
                    || self.timestamp_quantization_seconds.is_some()
                {
                    return Err(ContractError(
                        "unsupported GPU timestamps cannot publish GPU samples".to_owned(),
                    ));
                }
            }
            LiquidScaleGpuTimestampStatusV1::Valid => {
                if self.gpu_stages.is_empty()
                    || !self.rejected_gpu_timestamp_raw_seconds.is_empty()
                    || self
                        .gpu_stages
                        .iter()
                        .any(|stage| stage.series.zero_sample_count != 0)
                {
                    return Err(ContractError(
                        "valid GPU timestamps require retained nonzero stage samples".to_owned(),
                    ));
                }
            }
            LiquidScaleGpuTimestampStatusV1::QuantizedZeros => {
                if self.gpu_stages.is_empty()
                    || self.timestamp_quantization_seconds.is_none()
                    || !self.rejected_gpu_timestamp_raw_seconds.is_empty()
                    || !self
                        .gpu_stages
                        .iter()
                        .any(|stage| stage.series.zero_sample_count != 0)
                {
                    return Err(ContractError(
                        "quantized GPU timestamps require retained stages with zero samples"
                            .to_owned(),
                    ));
                }
            }
            LiquidScaleGpuTimestampStatusV1::AdvertisedUnusableZero => {
                if !self.gpu_stages.is_empty()
                    || self.rejected_gpu_timestamp_raw_seconds.is_empty()
                    || self
                        .rejected_gpu_timestamp_raw_seconds
                        .iter()
                        .any(|sample| *sample != 0.0)
                {
                    return Err(ContractError(
                        "unusable GPU timestamps must preserve only rejected zero samples"
                            .to_owned(),
                    ));
                }
            }
            LiquidScaleGpuTimestampStatusV1::AdvertisedUnusableOrder => {
                if !self.gpu_stages.is_empty()
                    || self.rejected_gpu_timestamp_raw_seconds.is_empty()
                    || self.timestamp_quantization_seconds.is_none()
                    || self
                        .rejected_gpu_timestamp_raw_seconds
                        .iter()
                        .any(|sample| !sample.is_finite())
                    || !self
                        .rejected_gpu_timestamp_raw_seconds
                        .iter()
                        .any(|sample| *sample < 0.0)
                {
                    return Err(ContractError(
                        "unordered GPU timestamps must preserve a rejected negative delta"
                            .to_owned(),
                    ));
                }
            }
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LiquidScaleWorkVectorV1 {
    pub version: u32,
    pub dynamic_bodies: u32,
    pub fixed_bodies: u32,
    pub live_bodies: u32,
    pub awake_bodies: u32,
    pub body_capacity: u32,
    pub candidate_pairs: u64,
    pub manifolds: u32,
    pub contact_points: u32,
    pub particle_capacity: u32,
    pub live_particles: u32,
    pub grid_insertions: u64,
    pub occupied_cells: u32,
    pub neighbor_checks: u64,
    pub accepted_neighbors: u64,
    pub particle_body_contacts: u64,
    pub body_contributions: u64,
    pub fluid_wake_roots: u32,
    pub dispatches_per_outer_step: u32,
    pub workgroups_per_outer_step: Option<u64>,
}

impl Validate for LiquidScaleWorkVectorV1 {
    fn validate(&self) -> Result<(), ContractError> {
        if self.version != LIQUID_SCALE_ARTIFACT_VERSION
            || self.live_bodies != self.dynamic_bodies.saturating_add(self.fixed_bodies)
            || self.live_bodies > self.body_capacity
            || self.awake_bodies > self.live_bodies
            || self.live_particles > self.particle_capacity
            || self.grid_insertions < u64::from(self.live_particles)
            || self.accepted_neighbors > self.neighbor_checks
            || self.dispatches_per_outer_step == 0
            || self.workgroups_per_outer_step == Some(0)
        {
            return Err(ContractError("invalid liquid scale work vector".to_owned()));
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LiquidScaleCorrectnessV1 {
    pub completed_outer_steps: u32,
    pub expected_final_active_particles: u32,
    pub final_active_particles: u32,
    pub final_state_finite: bool,
    pub generations_and_live_flags_valid: bool,
    pub capacity_clean: bool,
    pub state_hash: String,
    pub effective_simulation_scale: Option<f64>,
    pub slipped_wall_seconds: Option<f64>,
    pub sustained_60_hz: bool,
}

impl LiquidScaleCorrectnessV1 {
    fn validate(&self, config: &LiquidScaleBenchmarkConfigV1) -> Result<(), ContractError> {
        let expected_steps = config
            .retained_samples
            .checked_mul(config.outer_steps_per_sample)
            .ok_or_else(|| ContractError("liquid completed-step count overflowed".to_owned()))?;
        if self.completed_outer_steps != expected_steps
            || self.expected_final_active_particles != config.particle_count
            || self.final_active_particles != config.particle_count
            || !self.final_state_finite
            || !self.generations_and_live_flags_valid
            || !self.capacity_clean
            || !is_sha256(&self.state_hash)
        {
            return Err(ContractError(
                "liquid final correctness or completed-step validation failed".to_owned(),
            ));
        }
        match config.timing_mode {
            LiquidScaleTimingModeV1::PhysicsOnlyExplicitSteps => {
                if self.effective_simulation_scale.is_some()
                    || self.slipped_wall_seconds.is_some()
                    || self.sustained_60_hz
                {
                    return Err(ContractError(
                        "explicit-step throughput cannot claim interactive pacing".to_owned(),
                    ));
                }
            }
            LiquidScaleTimingModeV1::FullFrameInteractive => {
                let Some(scale) = self.effective_simulation_scale else {
                    return Err(ContractError(
                        "interactive liquid evidence requires effective simulation scale"
                            .to_owned(),
                    ));
                };
                let Some(slipped) = self.slipped_wall_seconds else {
                    return Err(ContractError(
                        "interactive liquid evidence requires slipped wall time".to_owned(),
                    ));
                };
                if !scale.is_finite()
                    || !(0.0..=1.0).contains(&scale)
                    || !slipped.is_finite()
                    || slipped < 0.0
                {
                    return Err(ContractError(
                        "interactive pacing evidence must be finite and bounded".to_owned(),
                    ));
                }
            }
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LiquidScaleBenchmarkArtifactV1 {
    pub schema_version: u32,
    pub artifact_version: u32,
    pub scenario_id: String,
    pub source_definition_hash: String,
    pub scenario_hash: String,
    pub host_kind: LiquidScaleHostKindV1,
    pub browser_version: Option<String>,
    pub build_profile: String,
    pub comparator_enabled: bool,
    pub provenance: ProvenanceV1,
    pub config: LiquidScaleBenchmarkConfigV1,
    pub work: LiquidScaleWorkVectorV1,
    pub capacities: Vec<CapacityRecordV1>,
    pub timings: LiquidScaleTimingsV1,
    pub correctness: LiquidScaleCorrectnessV1,
}

impl Validate for LiquidScaleBenchmarkArtifactV1 {
    fn validate(&self) -> Result<(), ContractError> {
        if self.schema_version != SCHEMA_VERSION
            || self.artifact_version != LIQUID_SCALE_ARTIFACT_VERSION
            || self.scenario_id.is_empty()
            || !is_sha256(&self.source_definition_hash)
            || !is_sha256(&self.scenario_hash)
            || self.build_profile != "release"
            || self.comparator_enabled
        {
            return Err(ContractError(
                "liquid artifact identity requires the current schema, release, and comparator-off build"
                    .to_owned(),
            ));
        }
        match self.host_kind {
            LiquidScaleHostKindV1::BrowserWebGpu => {
                if self.browser_version.as_deref().is_none_or(str::is_empty) {
                    return Err(ContractError(
                        "Browser WebGPU liquid evidence requires a browser version".to_owned(),
                    ));
                }
            }
            LiquidScaleHostKindV1::NativeDesktop | LiquidScaleHostKindV1::AndroidNative => {
                if self.browser_version.is_some() {
                    return Err(ContractError(
                        "native liquid evidence cannot carry a browser version".to_owned(),
                    ));
                }
            }
        }
        self.provenance.validate()?;
        if self.provenance.source_dirty
            && self
                .provenance
                .source_patch_digest
                .as_deref()
                .is_none_or(|digest| !is_sha256(digest))
        {
            return Err(ContractError(
                "dirty liquid evidence requires a SHA-256 patch digest".to_owned(),
            ));
        }
        self.config.validate()?;
        self.work.validate()?;
        if self.work.dynamic_bodies != self.config.solid_tier.dynamic_solid_count()
            || self.work.fixed_bodies != 3
            || self.work.body_capacity != self.config.solid_tier.expected_body_capacity()
            || self.work.particle_capacity != self.config.resolved_particle_capacity()
            || self.work.live_particles != self.config.particle_count
        {
            return Err(ContractError(
                "liquid work vector does not match the shared scenario definition".to_owned(),
            ));
        }
        if self.capacities.is_empty()
            || self
                .capacities
                .windows(2)
                .any(|window| window[0].class >= window[1].class)
        {
            return Err(ContractError(
                "liquid capacities must be non-empty, sorted, and unique".to_owned(),
            ));
        }
        for capacity in &self.capacities {
            capacity.validate()?;
            if capacity.overflowed {
                return Err(ContractError(
                    "accepted liquid evidence cannot contain capacity overflow".to_owned(),
                ));
            }
        }
        validate_required_capacity(
            &self.capacities,
            "bodies",
            u64::from(self.work.body_capacity),
            u64::from(self.work.live_bodies),
        )?;
        validate_required_capacity(
            &self.capacities,
            "particle-slots",
            u64::from(self.work.particle_capacity),
            u64::from(self.work.live_particles),
        )?;
        self.timings.validate(self.config.retained_samples)?;
        if self.config.timing_mode == LiquidScaleTimingModeV1::PhysicsOnlyExplicitSteps
            && self.timings.gpu_stages.iter().any(|timing| {
                matches!(
                    timing.stage,
                    LiquidScaleGpuStageV1::Render | LiquidScaleGpuStageV1::FullFrame
                )
            })
        {
            return Err(ContractError(
                "physics-only liquid evidence cannot contain render timing".to_owned(),
            ));
        }
        self.correctness.validate(&self.config)?;
        let sustained = self.config.timing_mode == LiquidScaleTimingModeV1::FullFrameInteractive
            && self.config.interactive_duration_seconds.unwrap_or(0.0) >= 30.0
            && self.correctness.effective_simulation_scale.unwrap_or(0.0) >= 0.95
            && self.timings.host_completion.p95_seconds <= LIQUID_SCALE_FIXED_DT_SECONDS;
        if self.correctness.sustained_60_hz != sustained {
            return Err(ContractError(
                "liquid sustained-60-Hz disposition does not match the measured contract"
                    .to_owned(),
            ));
        }
        Ok(())
    }
}

fn validate_required_capacity(
    capacities: &[CapacityRecordV1],
    class: &str,
    expected_capacity: u64,
    expected_used: u64,
) -> Result<(), ContractError> {
    let Some(record) = capacities.iter().find(|record| record.class == class) else {
        return Err(ContractError(format!(
            "liquid evidence is missing required {class} capacity"
        )));
    };
    if record.capacity_elements != expected_capacity || record.used_elements != expected_used {
        return Err(ContractError(format!(
            "liquid {class} capacity does not match the work vector"
        )));
    }
    Ok(())
}

fn is_sha256(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

#[cfg(test)]
mod tests {
    use physbox_contracts::NamedValueV1;

    use super::*;

    fn timing(values: Vec<f64>) -> LiquidScaleTimingSeriesV1 {
        LiquidScaleTimingSeriesV1::host(values).unwrap()
    }

    fn artifact() -> LiquidScaleBenchmarkArtifactV1 {
        let config = LiquidScaleBenchmarkConfigV1::physics_only(
            LiquidScaleSolidTier::Solids20,
            8_192,
            16,
            3,
        );
        LiquidScaleBenchmarkArtifactV1 {
            schema_version: SCHEMA_VERSION,
            artifact_version: LIQUID_SCALE_ARTIFACT_VERSION,
            scenario_id: "liquid-scale-solids-20-8192-v1".to_owned(),
            source_definition_hash: "0".repeat(64),
            scenario_hash: "1".repeat(64),
            host_kind: LiquidScaleHostKindV1::NativeDesktop,
            browser_version: None,
            build_profile: "release".to_owned(),
            comparator_enabled: false,
            provenance: ProvenanceV1 {
                run_id: "run".to_owned(),
                source_revision: "revision".to_owned(),
                source_dirty: false,
                source_patch_digest: None,
                host: "host".to_owned(),
                adapter: "adapter".to_owned(),
                backend: "Vulkan".to_owned(),
                driver: "driver".to_owned(),
                toolchain: "rustc".to_owned(),
                build_options: vec![NamedValueV1 {
                    name: "profile".to_owned(),
                    value: "release".to_owned(),
                }],
                runtime_settings: vec![NamedValueV1 {
                    name: "timing-mode".to_owned(),
                    value: "physics-only-explicit-steps".to_owned(),
                }],
            },
            config,
            work: LiquidScaleWorkVectorV1 {
                version: LIQUID_SCALE_ARTIFACT_VERSION,
                dynamic_bodies: 20,
                fixed_bodies: 3,
                live_bodies: 23,
                awake_bodies: 20,
                body_capacity: 64,
                candidate_pairs: 0,
                manifolds: 0,
                contact_points: 0,
                particle_capacity: 8_192,
                live_particles: 8_192,
                grid_insertions: 8_192,
                occupied_cells: 1,
                neighbor_checks: 1,
                accepted_neighbors: 1,
                particle_body_contacts: 1,
                body_contributions: 1,
                fluid_wake_roots: 1,
                dispatches_per_outer_step: 92,
                workgroups_per_outer_step: Some(1),
            },
            capacities: vec![
                CapacityRecordV1 {
                    class: "bodies".to_owned(),
                    capacity_elements: 64,
                    used_elements: 23,
                    allocated_bytes: 1_024,
                    high_water_bytes: 1_024,
                    overflowed: false,
                },
                CapacityRecordV1 {
                    class: "particle-slots".to_owned(),
                    capacity_elements: 8_192,
                    used_elements: 8_192,
                    allocated_bytes: 65_536,
                    high_water_bytes: 65_536,
                    overflowed: false,
                },
            ],
            timings: LiquidScaleTimingsV1 {
                percentile_method: LIQUID_SCALE_PERCENTILE_METHOD.to_owned(),
                gpu_timestamp_status: LiquidScaleGpuTimestampStatusV1::Unsupported,
                timestamp_quantization_seconds: None,
                gpu_stages: Vec::new(),
                rejected_gpu_timestamp_raw_seconds: Vec::new(),
                cpu_encode_submit: timing(vec![0.001, 0.002, 0.003]),
                host_completion: timing(vec![0.010, 0.011, 0.012]),
            },
            correctness: LiquidScaleCorrectnessV1 {
                completed_outer_steps: 3,
                expected_final_active_particles: 8_192,
                final_active_particles: 8_192,
                final_state_finite: true,
                generations_and_live_flags_valid: true,
                capacity_clean: true,
                state_hash: "2".repeat(64),
                effective_simulation_scale: None,
                slipped_wall_seconds: None,
                sustained_60_hz: false,
            },
        }
    }

    #[test]
    fn liquid_scale_artifact_accepts_a_complete_physics_only_row() {
        let artifact = artifact();
        artifact.validate().unwrap();
        let bytes = serde_json::to_vec(&artifact).unwrap();
        let decoded: LiquidScaleBenchmarkArtifactV1 = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(decoded, artifact);
        decoded.validate().unwrap();
    }

    #[test]
    fn liquid_scale_artifact_rejects_contract_drift_and_incomplete_evidence() {
        let mut changed_quality = artifact();
        changed_quality.config.pressure_iterations += 1;
        assert!(changed_quality.validate().is_err());

        let mut comparator = artifact();
        comparator.comparator_enabled = true;
        assert!(comparator.validate().is_err());

        let mut overflow = artifact();
        overflow.capacities[1].overflowed = true;
        assert!(overflow.validate().is_err());

        let mut wrong_population = artifact();
        wrong_population.correctness.final_active_particles -= 1;
        assert!(wrong_population.validate().is_err());

        let mut no_samples = artifact();
        no_samples.timings.host_completion.raw_seconds.clear();
        assert!(no_samples.validate().is_err());

        let mut bad_percentile = artifact();
        bad_percentile.timings.host_completion.p95_seconds = 0.010;
        assert!(bad_percentile.validate().is_err());

        let mut dirty_without_digest = artifact();
        dirty_without_digest.provenance.source_dirty = true;
        assert!(dirty_without_digest.validate().is_err());
    }

    #[test]
    fn liquid_scale_timing_preserves_raw_samples_and_nearest_rank_percentiles() {
        let series = LiquidScaleTimingSeriesV1::host(vec![4.0, 1.0, 3.0, 2.0]).unwrap();
        assert_eq!(series.raw_seconds, vec![4.0, 1.0, 3.0, 2.0]);
        assert_eq!(series.p50_seconds, 2.0);
        assert_eq!(series.p95_seconds, 4.0);
        assert_eq!(series.p99_seconds, 4.0);
        series.validate_for_clock(false).unwrap();
        assert!(LiquidScaleTimingSeriesV1::host(vec![0.0]).is_err());
        assert!(LiquidScaleTimingSeriesV1::gpu(vec![0.0]).is_ok());
    }
}
