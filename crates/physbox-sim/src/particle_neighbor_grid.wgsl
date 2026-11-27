fn cell_for(position: vec2f) -> vec2i {
    return vec2i(floor((position - params.grid.xy) / params.grid.z));
}

fn cell_is_valid(cell: vec2i) -> bool {
    return all(cell >= vec2i(0)) &&
        cell.x < i32(params.counts.z) &&
        cell.y < i32(params.counts.w);
}

fn cell_index(cell: vec2i) -> u32 {
    return u32(cell.y) * params.counts.z + u32(cell.x);
}

// Scatter leaves the start of each packed range in cell_offsets. The last
// range ends at the total number of live records, even when it is empty.
fn grid_record_count() -> u32 {
    let last = params.counts.z * params.counts.w - 1u;
    return cell_offsets[last] + atomicLoad(&cell_counts[last]);
}
