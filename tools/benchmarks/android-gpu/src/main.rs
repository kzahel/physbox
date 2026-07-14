use physbox_contracts::CapacityRecordV1;
use physbox_engine::{
    AdapterDiagnostics, GpuPrimitiveKind, GpuSolverCandidate, authored_chain_scenario,
    benchmark_gpu_primitive, benchmark_gpu_soft_step_case,
};
use serde::Serialize;

const SUCCESS_SENTINEL: &str = "PHYSBOX5_ANDROID_GPU_BENCHMARK_SUCCESS";

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Report {
    schema_version: u32,
    run_id: String,
    source_revision: String,
    source_dirty: bool,
    host: &'static str,
    adapter: AdapterDiagnostics,
    solver_contacts: u32,
    primitive_elements: u32,
    warmup_steps: u32,
    samples: u32,
    solvers: Vec<Lane>,
    primitives: Vec<Lane>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Lane {
    name: String,
    work_elements: u32,
    dispatches: u32,
    gpu_step_seconds: Option<Vec<f64>>,
    host_completion_seconds: Vec<f64>,
    cpu_encode_submit_seconds: Vec<f64>,
    timestamp_period_nanoseconds: Option<f32>,
    capacities: Vec<CapacityRecordV1>,
}

fn main() {
    if let Err(error) = run() {
        eprintln!("physbox-android-gpu-benchmark error: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let arguments = Arguments::parse()?;
    let scenario = authored_chain_scenario(arguments.solver_contacts + 1, 0x5eed)
        .map_err(|error| error.to_string())?;
    let mut solvers = Vec::new();
    let mut adapter = None;
    for candidate in [
        GpuSolverCandidate::JacobiGather,
        GpuSolverCandidate::ColoredSoftTgs,
    ] {
        let result = pollster::block_on(benchmark_gpu_soft_step_case(
            &scenario,
            candidate,
            arguments.warmup,
            arguments.samples,
        ))?;
        adapter.get_or_insert_with(|| result.adapter.clone());
        solvers.push(Lane {
            name: candidate.name().to_owned(),
            work_elements: arguments.solver_contacts,
            dispatches: result.dispatches,
            gpu_step_seconds: result.gpu_step_seconds,
            host_completion_seconds: result.host_completion_seconds,
            cpu_encode_submit_seconds: result.cpu_encode_submit_seconds,
            timestamp_period_nanoseconds: result.timestamp_period_nanoseconds,
            capacities: result.capacities,
        });
    }
    let mut primitives = Vec::new();
    for primitive in [
        GpuPrimitiveKind::PrefixScan,
        GpuPrimitiveKind::RadixSort,
        GpuPrimitiveKind::Unique,
        GpuPrimitiveKind::Compaction,
    ] {
        let result = pollster::block_on(benchmark_gpu_primitive(
            primitive,
            arguments.primitive_elements,
            arguments.warmup,
            arguments.samples,
        ))?;
        adapter.get_or_insert_with(|| result.adapter.clone());
        primitives.push(Lane {
            name: primitive.name().to_owned(),
            work_elements: arguments.primitive_elements,
            dispatches: result.dispatches,
            gpu_step_seconds: result.gpu_step_seconds,
            host_completion_seconds: result.host_completion_seconds,
            cpu_encode_submit_seconds: result.cpu_encode_submit_seconds,
            timestamp_period_nanoseconds: result.timestamp_period_nanoseconds,
            capacities: result.capacities,
        });
    }
    let report = Report {
        schema_version: physbox_contracts::SCHEMA_VERSION,
        run_id: arguments.run_id,
        source_revision: arguments.source_revision,
        source_dirty: arguments.source_dirty,
        host: "android-native-vulkan",
        adapter: adapter.ok_or_else(|| "Android benchmark produced no adapter".to_owned())?,
        solver_contacts: arguments.solver_contacts,
        primitive_elements: arguments.primitive_elements,
        warmup_steps: arguments.warmup,
        samples: arguments.samples,
        solvers,
        primitives,
    };
    println!(
        "PHYSBOX5_ANDROID_BENCHMARK_RESULT {}",
        serde_json::to_string(&report).map_err(|error| error.to_string())?
    );
    println!("{SUCCESS_SENTINEL}");
    Ok(())
}

struct Arguments {
    run_id: String,
    source_revision: String,
    source_dirty: bool,
    solver_contacts: u32,
    primitive_elements: u32,
    warmup: u32,
    samples: u32,
}

impl Arguments {
    fn parse() -> Result<Self, String> {
        let mut run_id = None;
        let mut source_revision = None;
        let mut source_dirty = None;
        let mut solver_contacts = None;
        let mut primitive_elements = None;
        let mut warmup = None;
        let mut samples = None;
        let mut arguments = std::env::args().skip(1);
        while let Some(argument) = arguments.next() {
            let value = arguments
                .next()
                .ok_or_else(|| format!("missing value for {argument}"))?;
            match argument.as_str() {
                "--run-id" => run_id = Some(value),
                "--source-revision" => source_revision = Some(value),
                "--source-dirty" => source_dirty = Some(parse_bool(&value)?),
                "--solver-contacts" => solver_contacts = Some(parse(&value, "solver contacts")?),
                "--primitive-elements" => {
                    primitive_elements = Some(parse(&value, "primitive elements")?)
                }
                "--warmup" => warmup = Some(parse(&value, "warmup")?),
                "--samples" => samples = Some(parse(&value, "samples")?),
                _ => return Err(format!("unknown argument {argument}")),
            }
        }
        let output = Self {
            run_id: run_id.ok_or_else(|| "missing --run-id".to_owned())?,
            source_revision: source_revision
                .ok_or_else(|| "missing --source-revision".to_owned())?,
            source_dirty: source_dirty.ok_or_else(|| "missing --source-dirty".to_owned())?,
            solver_contacts: solver_contacts
                .ok_or_else(|| "missing --solver-contacts".to_owned())?,
            primitive_elements: primitive_elements
                .ok_or_else(|| "missing --primitive-elements".to_owned())?,
            warmup: warmup.ok_or_else(|| "missing --warmup".to_owned())?,
            samples: samples.ok_or_else(|| "missing --samples".to_owned())?,
        };
        if output.solver_contacts == 0
            || output.primitive_elements == 0
            || output.warmup == 0
            || output.samples == 0
        {
            return Err("Android GPU benchmark counts must be non-zero".to_owned());
        }
        Ok(output)
    }
}

fn parse<T: std::str::FromStr>(value: &str, name: &str) -> Result<T, String> {
    value
        .parse()
        .map_err(|_| format!("invalid {name}: {value}"))
}

fn parse_bool(value: &str) -> Result<bool, String> {
    match value {
        "true" => Ok(true),
        "false" => Ok(false),
        _ => Err(format!("invalid boolean: {value}")),
    }
}
