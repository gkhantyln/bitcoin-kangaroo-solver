// Minimal: just the main function with a while loop, no mod_mul etc.
struct Params {
    target_x: array<u32, 8>,
    target_y: array<u32, 8>,
    dist_bits: u32,
    table_size: u32,
    dp_capacity: u32,
    range_bits: u32,
}
struct KangarooInit { dist_x: array<u32, 8>, dist_y: array<u32, 8>, distance: array<u32, 8>, is_tame: u32, }
struct JumpTablePoint { dx: array<u32, 8>, dy: array<u32, 8>, }
struct JumpTableDist { ddist: array<u32, 8>, }
struct DistPointOutput { x: array<u32, 8>, distance: array<u32, 8>, dist_type: u32, }
struct FoundKeyOutput { key: array<u32, 8>, }
struct OutputState { dist_x: array<u32, 8>, dist_y: array<u32, 8>, distance: array<u32, 8>, is_tame: u32, }

@group(0) @binding(0) var<storage, read> params: Params;
@group(0) @binding(1) var<storage, read_write> kangaroos: array<KangarooInit>;
@group(0) @binding(2) var<storage, read> jump_points: array<JumpTablePoint>;
@group(0) @binding(3) var<storage, read> jump_dists: array<JumpTableDist>;
@group(0) @binding(4) var<storage, read_write> dist_points: array<DistPointOutput>;
@group(0) @binding(5) var<storage, read_write> dist_count: atomic<u32>;
@group(0) @binding(6) var<storage, read_write> found_key: array<FoundKeyOutput>;
@group(0) @binding(7) var<storage, read_write> output_state: array<OutputState>;

var<private> P: array<u32, 8> = array<u32, 8>(
    0xFFFFFC2Fu, 0xFFFFFFFFu, 0xFFFFFFFFu, 0xFFFFFFFFu,
    0xFFFFFFFFu, 0xFFFFFFFFu, 0xFFFFFFFFu, 0xFFFFFFFFu,
);

var<workgroup> wg_zvals: array<array<u32, 8>, 64>;
var<workgroup> wg_invz: array<array<u32, 8>, 64>;
var<workgroup> wg_xaff: array<array<u32, 8>, 64>;

@compute @workgroup_size(64)
fn main(@builtin(global_invocation_id) gid: vec3<u32>, @builtin(local_invocation_index) local_id: u32) {
    let idx = gid.x;
    var pt_x: array<u32, 8>;
    var pt_y: array<u32, 8>;
    var pt_z: array<u32, 8>;
    for (var i = 0u; i < 8u; i++) { pt_x[i] = kangaroos[idx].dist_x[i]; }
    for (var i = 0u; i < 8u; i++) { pt_y[i] = kangaroos[idx].dist_y[i]; }
    pt_z[0] = 1u; for (var i = 1u; i < 8u; i++) { pt_z[i] = 0u; }

    var dist: array<u32, 8>;
    for (var i = 0u; i < 8u; i++) { dist[i] = kangaroos[idx].distance[i]; }
    let is_tame = kangaroos[idx].is_tame != 0u;

    let MAX_STEPS = 1000u;
    let DP_REPORT_INTERVAL = 2048u;

    var step = 0u;

    // Minimal while loop
    while (step < MAX_STEPS) {
        for (var i = 0u; i < 8u; i++) { wg_zvals[local_id][i] = pt_z[i]; }
        workgroupBarrier();

        if (local_id == 0u) {
            for (var j = 0u; j < 64u; j++) {
                for (var i = 0u; i < 8u; i++) { wg_invz[j][i] = wg_zvals[j][i]; }
            }
        }
        workgroupBarrier();

        for (var i = 0u; i < 8u; i++) { wg_xaff[local_id][i] = pt_x[i]; }

        var h: u32 = 0u;
        var ji: u32 = h % 64u;

        var jp_x: array<u32, 8>;
        var jp_y: array<u32, 8>;
        for (var i = 0u; i < 8u; i++) { jp_x[i] = jump_points[ji].dx[i]; }
        for (var i = 0u; i < 8u; i++) { jp_y[i] = jump_points[ji].dy[i]; }

        var jd_ddist: array<u32, 8>;
        for (var i = 0u; i < 8u; i++) { jd_ddist[i] = jump_dists[ji].ddist[i]; }

        step++;

        if (step % DP_REPORT_INTERVAL == 0u) {
            let dp_idx = atomicAdd(&dist_count, 1u);
            if (dp_idx < params.dp_capacity) {
                for (var i = 0u; i < 8u; i++) { dist_points[dp_idx].x[i] = pt_x[i]; }
            }
        }
    }

    for (var i = 0u; i < 8u; i++) { output_state[idx].dist_x[i] = pt_x[i]; }
    for (var i = 0u; i < 8u; i++) { output_state[idx].distance[i] = dist[i]; }
    output_state[idx].is_tame = select(0u, 1u, is_tame);
}
