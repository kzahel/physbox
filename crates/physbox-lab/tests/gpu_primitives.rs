use physbox_engine::{GpuPrimitiveKind, benchmark_gpu_primitive};
use physbox_lab::capacity_is_valid;

#[test]
fn portable_primitives_validate_after_delayed_measurement() {
    for primitive in [
        GpuPrimitiveKind::PrefixScan,
        GpuPrimitiveKind::RadixSort,
        GpuPrimitiveKind::Unique,
        GpuPrimitiveKind::Compaction,
    ] {
        let result = pollster::block_on(benchmark_gpu_primitive(primitive, 513, 1, 2)).unwrap();
        assert_eq!(result.element_count, 513);
        assert_eq!(result.host_completion_seconds.len(), 2);
        assert_eq!(result.cpu_encode_submit_seconds.len(), 2);
        assert!(result.dispatches >= 3);
        assert!(result.output_elements > 0);
        capacity_is_valid(&result.capacities).unwrap();
        match (
            &result.gpu_step_seconds,
            result.timestamp_period_nanoseconds,
        ) {
            (Some(samples), Some(period)) => {
                assert_eq!(samples.len(), 2);
                assert!(samples.iter().all(|sample| *sample > 0.0));
                assert!(period > 0.0);
            }
            (None, None) => {}
            mismatch => panic!("inconsistent timestamp evidence: {mismatch:?}"),
        }
    }
}

#[test]
fn portable_primitives_validate_the_required_thirty_thousand_scale() {
    for primitive in [
        GpuPrimitiveKind::PrefixScan,
        GpuPrimitiveKind::RadixSort,
        GpuPrimitiveKind::Unique,
        GpuPrimitiveKind::Compaction,
    ] {
        let result = pollster::block_on(benchmark_gpu_primitive(primitive, 30_000, 1, 1)).unwrap();
        assert_eq!(result.element_count, 30_000);
        assert!(result.output_elements > 0);
        capacity_is_valid(&result.capacities).unwrap();
    }
}
