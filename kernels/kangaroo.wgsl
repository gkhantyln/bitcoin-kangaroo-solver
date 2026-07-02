// SPDX-License-Identifier: MIT
//
// WGSL compute shader — Pollard's Kangaroo for secp256k1
// ======================================================
//
// Design:
//   workgroup_size(64): 64 kangaroos run cooperatively per workgroup.
//   Batch Montgomery inversion: All 64 kangaroos share their Z values,
//   thread 0 computes all 64 inverses at once (1 Fermat inverse + 2n mults),
//   giving ~40x speedup vs individual inverses.
//   Shared memory: jump table, Z values, inverted Z values.
//
// Each workgroup = 64 kangaroos. Dispatch = total_kangaroos / 64 workgroups.
//
// Bindings:
//   0: uniform  Params          — target point, dist bits, table size
//   1: storage  KangarooState[] — per-kangaroo init state + distance
//   2: storage  JumpPoints[]    — jump table points (affine)
//   3: storage  JumpDists[]     — jump table distance deltas (scalar)
//   4: storage  DistPoints[]    — output DP buffer
//   5: storage  DistCount       — atomic counter
//   6: storage  FoundKey        — private key output (0 = not found)
//   7: storage  OutputState[]   — final kangaroo state (affine)

// ── secp256k1 field prime P ──
var<private> P: array<u32, 8> = array<u32, 8>(
    0xFFFFFC2Fu, 0xFFFFFFFFu, 0xFFFFFFFFu, 0xFFFFFFFFu,
    0xFFFFFFFFu, 0xFFFFFFFFu, 0xFFFFFFFFu, 0xFFFFFFFFu,
);

// ─── Shared memory (unused in production — kept for test compatibility) ──
var<workgroup> wg_zvals:  array<array<u32, 8>, 64>;
var<workgroup> wg_invz:   array<array<u32, 8>, 64>;
var<workgroup> wg_xaff:   array<array<u32, 8>, 64>;

// ── helpers ──
fn zero_256(out: ptr<function, array<u32, 8>>) {
    for (var i = 0u; i < 8u; i++) { (*out)[i] = 0u; }
}

fn one_256(out: ptr<function, array<u32, 8>>) {
    zero_256(out); (*out)[0] = 1u;
}

fn copy_256(src: ptr<function, array<u32, 8>>, dst: ptr<function, array<u32, 8>>) {
    for (var i = 0u; i < 8u; i++) { (*dst)[i] = (*src)[i]; }
}

// ── u32 × u32 → vec2<u32>(lo, hi) ──
fn mul_32x32(a: u32, b: u32) -> vec2<u32> {
    let aL = a & 0xFFFFu;  let aH = a >> 16u;
    let bL = b & 0xFFFFu;  let bH = b >> 16u;
    let lo  = aL * bL;
    let m1  = aL * bH;
    let m2  = aH * bL;
    let hi  = aH * bH;
    let sum = (lo >> 16u) + m1;
    let r0  = (lo & 0xFFFFu) | ((sum & 0xFFFFu) << 16u);
    let r1  = hi + (sum >> 16u) + m2;
    return vec2<u32>(r0, r1);
}

// ── 256-bit add with carry ──
fn add_256(a: ptr<function, array<u32, 8>>, b: ptr<function, array<u32, 8>>, out: ptr<function, array<u32, 8>>) {
    var c: u32 = 0u;
    for (var i = 0u; i < 8u; i++) {
        let s1 = (*a)[i] + (*b)[i];
        let c1 = u32(s1 < (*a)[i]);
        let s2 = s1 + c;
        let c2 = u32(s2 < s1);
        (*out)[i] = s2;
        c = c1 + c2;
    }
}

// ── 256-bit sub with borrow ──
fn sub_256(a: ptr<function, array<u32, 8>>, b: ptr<function, array<u32, 8>>, out: ptr<function, array<u32, 8>>) {
    var borrow: u32 = 0u;
    for (var i = 0u; i < 8u; i++) {
        let d = (*a)[i] - (*b)[i] - borrow;
        borrow = u32(d > (*a)[i]);
        (*out)[i] = d;
    }
}

fn lt_256(a: ptr<function, array<u32, 8>>, b: ptr<function, array<u32, 8>>) -> bool {
    for (var i = 7u; i < 8u; i--) {
        if ((*a)[i] < (*b)[i]) { return true; }
        if ((*a)[i] > (*b)[i]) { return false; }
    }
    return false;
}

fn gte_256(a: ptr<function, array<u32, 8>>, b: ptr<function, array<u32, 8>>) -> bool {
    return !lt_256(a, b);
}

fn is_zero_256(a: ptr<function, array<u32, 8>>) -> bool {
    for (var i = 0u; i < 8u; i++) { if ((*a)[i] != 0u) { return false; } }
    return true;
}

fn mod_reduce(a: ptr<function, array<u32, 8>>, out: ptr<function, array<u32, 8>>) {
    var t: array<u32, 8>;
    copy_256(a, &t);
    var p = P;
    while (gte_256(&t, &p)) {
        sub_256(&t, &p, &t);
    }
    copy_256(&t, out);
}

fn mod_add(a: ptr<function, array<u32, 8>>, b: ptr<function, array<u32, 8>>, out: ptr<function, array<u32, 8>>) {
    var tmp: array<u32, 8>;
    add_256(a, b, &tmp);
    mod_reduce(&tmp, out);
}

fn mod_sub(a: ptr<function, array<u32, 8>>, b: ptr<function, array<u32, 8>>, out: ptr<function, array<u32, 8>>) {
    var tmp: array<u32, 8>;
    sub_256(a, b, &tmp);
    var p = P;
    if (gte_256(&tmp, &p)) {
        sub_256(&tmp, &p, out);
    } else {
        copy_256(&tmp, out);
    }
}

// ── Modular multiplication: (a * b) mod P ──
fn mod_mul(a: ptr<function, array<u32, 8>>, b: ptr<function, array<u32, 8>>, out: ptr<function, array<u32, 8>>) {
    var lo: array<u32, 8>;
    var hi: array<u32, 8>;
    for (var i = 0u; i < 8u; i++) { lo[i] = 0u; hi[i] = 0u; }

    for (var i = 0u; i < 8u; i++) {
        var carry: u32 = 0u;
        for (var j = 0u; j < 8u; j++) {
            let prod = mul_32x32((*a)[i], (*b)[j]);
            let idx = i + j;

            if (idx < 8u) {
                let s1 = lo[idx] + prod.x;
                let c1 = u32(s1 < lo[idx]);
                let s2 = s1 + carry;
                let c2 = u32(s2 < s1);
                lo[idx] = s2;

                let t1 = prod.y + c1;
                let tc1 = u32(t1 < prod.y);
                let t2 = t1 + c2;
                let tc2 = u32(t2 < t1);
                carry = t2;

                if (tc1 + tc2 > 0u) {
                    if (idx + 1u < 8u) {
                        lo[idx + 1u] = lo[idx + 1u] + tc1 + tc2;
                    } else {
                        hi[0u] = hi[0u] + tc1 + tc2;
                    }
                }
            } else {
                let hidx = idx - 8u;
                let s1 = hi[hidx] + prod.x;
                let c1 = u32(s1 < hi[hidx]);
                let s2 = s1 + carry;
                let c2 = u32(s2 < s1);
                hi[hidx] = s2;

                let t1 = prod.y + c1;
                let tc1 = u32(t1 < prod.y);
                let t2 = t1 + c2;
                let tc2 = u32(t2 < t1);
                carry = t2;

                if (tc1 + tc2 > 0u && hidx + 1u < 8u) {
                    hi[hidx + 1u] = hi[hidx + 1u] + tc1 + tc2;
                }
            }
        }
        if (carry > 0u) {
            hi[i] = hi[i] + carry;
        }
    }

    // Reduction using P = 2^256 - 2^32 - 977
    // 2^256 ≡ 2^32 + 977 (mod P) = 0x1000003D1
    if (is_zero_256(&hi)) { mod_reduce(&lo, out); return; }

    // Round 1: result = lo + hi * (2^32 + 977)
    var hi977: array<u32, 9>;
    var c: u32 = 0u;
    for (var i = 0u; i < 8u; i++) {
        let prod = mul_32x32(hi[i], 977u);
        let s    = prod.x + c;
        let c1   = u32(s < c);
        hi977[i] = s;
        c        = prod.y + c1;
    }
    hi977[8u] = c;

    var r1: array<u32, 9>;
    for (var i = 0u; i < 8u; i++) { r1[i] = lo[i]; }
    r1[8u] = 0u;

    // r1 += hi << 32
    c = 0u;
    for (var i = 0u; i < 8u; i++) {
        let s1 = r1[i + 1u] + hi[i];
        let c1 = u32(s1 < r1[i + 1u]);
        let s2 = s1 + c;
        let c2 = u32(s2 < s1);
        r1[i + 1u] = s2;
        c = c1 + c2;
    }
    r1[8u] = r1[8u] + c;

    // r1 += hi977
    c = 0u;
    for (var i = 0u; i < 9u; i++) {
        let s1 = r1[i] + hi977[i];
        let c1 = u32(s1 < r1[i]);
        let s2 = s1 + c;
        let c2 = u32(s2 < s1);
        r1[i]  = s2;
        c = c1 + c2;
    }

    // 9th limb overflow → reduce again
    if (c > 0u || r1[8u] > 0u) {
        var extra = r1[8u];
        for (var i = 0u; i < 8u; i++) { r1[i] = r1[i]; }
        r1[8u] = 0u;

        let prod = mul_32x32(extra, 977u);
        var carry2: u32 = 0u;
        let s = r1[0u] + prod.x;
        carry2 = u32(s < r1[0u]);
        r1[0u] = s;

        for (var i = 1u; i < 8u; i++) {
            let s2 = r1[i] + carry2;
            carry2 = u32(s2 < r1[i]);
            r1[i] = s2;
        }

        carry2 = 0u;
        let s2 = r1[1u] + extra;
        var c2 = u32(s2 < r1[1u]);
        r1[1u] = s2;
        for (var i = 2u; i < 8u; i++) {
            let s3 = r1[i] + c2;
            c2 = u32(s3 < r1[i]);
            r1[i] = s3;
        }

        carry2 = 0u;
        let s_end = r1[7u] + prod.y;
        carry2 = u32(s_end < r1[7u]);
        r1[7u] = s_end;
    }

    var result: array<u32, 8>;
    for (var i = 0u; i < 8u; i++) { result[i] = r1[i]; }
    mod_reduce(&result, out);
}

fn mod_sq(a: ptr<function, array<u32, 8>>, out: ptr<function, array<u32, 8>>) {
    mod_mul(a, a, out);
}

// ── Modular inverse via Fermat: a⁻¹ ≡ a^(P-2) mod P ──
fn mod_inv(a: ptr<function, array<u32, 8>>, out: ptr<function, array<u32, 8>>) {
    one_256(out);
    var exp = P;
    exp[0] = P[0] - 2u;
    var base: array<u32, 8>;
    copy_256(a, &base);

    for (var i = 7u; i < 8u; i--) {
        var bit: u32 = 0x80000000u;
        while (bit != 0u) {
            mod_sq(out, out);
            if ((exp[i] & bit) != 0u) {
                mod_mul(out, &base, out);
            }
            bit = bit >> 1u;
        }
    }
}

// ── Jacobian point on secp256k1 ──
struct Jacobian {
    x: array<u32, 8>,
    y: array<u32, 8>,
    z: array<u32, 8>,
}

fn point_copy(dst: ptr<function, Jacobian>, src: ptr<function, Jacobian>) {
    for (var i = 0u; i < 8u; i++) {
        (*dst).x[i] = (*src).x[i];
        (*dst).y[i] = (*src).y[i];
        (*dst).z[i] = (*src).z[i];
    }
}

fn const_3(out: ptr<function, array<u32, 8>>) { zero_256(out); (*out)[0] = 3u; }
fn const_2(out: ptr<function, array<u32, 8>>) { zero_256(out); (*out)[0] = 2u; }
fn const_4(out: ptr<function, array<u32, 8>>) { zero_256(out); (*out)[0] = 4u; }
fn const_8(out: ptr<function, array<u32, 8>>) { zero_256(out); (*out)[0] = 8u; }

// ── Jacobian double: r = 2*p ──
fn point_double(p: ptr<function, Jacobian>) -> Jacobian {
    var px = (*p).x;
    var py = (*p).y;
    var pz = (*p).z;

    var three: array<u32, 8>; const_3(&three);
    var two: array<u32, 8>;   const_2(&two);
    var four: array<u32, 8>;  const_4(&four);
    var eight: array<u32, 8>; const_8(&eight);

    var x2: array<u32, 8>;    mod_sq(&px, &x2);
    var t: array<u32, 8>;     mod_mul(&x2, &three, &t);
    var y2: array<u32, 8>;    mod_sq(&py, &y2);
    var y4: array<u32, 8>;    mod_sq(&y2, &y4);
    var xy2: array<u32, 8>;   mod_mul(&px, &y2, &xy2);
    var xy2_8: array<u32, 8>; mod_mul(&xy2, &eight, &xy2_8);
    var t2: array<u32, 8>;    mod_sq(&t, &t2);
    var x3: array<u32, 8>;    mod_sub(&t2, &xy2_8, &x3);
    var xy2_4: array<u32, 8>; mod_mul(&xy2, &four, &xy2_4);
    var tmp: array<u32, 8>;   mod_sub(&xy2_4, &x3, &tmp);
    var tmp2: array<u32, 8>;  mod_mul(&t, &tmp, &tmp2);
    var y4_8: array<u32, 8>;  mod_mul(&y4, &eight, &y4_8);
    var y3: array<u32, 8>;    mod_sub(&tmp2, &y4_8, &y3);
    var yz: array<u32, 8>;    mod_mul(&py, &pz, &yz);
    var z3: array<u32, 8>;    mod_mul(&yz, &two, &z3);

    return Jacobian(x3, y3, z3);
}

// ── Jacobian full add: r = a + b ──
fn point_add(a: ptr<function, Jacobian>, b: ptr<function, Jacobian>) -> Jacobian {
    var ax = (*a).x; var ay = (*a).y; var az = (*a).z;
    var bx = (*b).x; var by = (*b).y; var bz = (*b).z;

    var z1_2: array<u32, 8>;  mod_sq(&az, &z1_2);
    var z2_2: array<u32, 8>;  mod_sq(&bz, &z2_2);
    var u1: array<u32, 8>;    mod_mul(&ax, &z2_2, &u1);
    var u2: array<u32, 8>;    mod_mul(&bx, &z1_2, &u2);
    var z1_3: array<u32, 8>;  mod_mul(&z1_2, &az, &z1_3);
    var z2_3: array<u32, 8>;  mod_mul(&z2_2, &bz, &z2_3);
    var s1: array<u32, 8>;    mod_mul(&ay, &z2_3, &s1);
    var s2: array<u32, 8>;    mod_mul(&by, &z1_3, &s2);
    var h: array<u32, 8>;     mod_sub(&u2, &u1, &h);
    var r: array<u32, 8>;     mod_sub(&s2, &s1, &r);

    var zero: array<u32, 8>; zero_256(&zero);
    if (is_zero_256(&h)) {
        if (is_zero_256(&r)) { return point_double(a); }
        return Jacobian(zero, zero, zero);
    }

    var h2: array<u32, 8>;    mod_sq(&h, &h2);
    var h3: array<u32, 8>;    mod_mul(&h2, &h, &h3);
    var u1h2: array<u32, 8>;  mod_mul(&u1, &h2, &u1h2);
    var r2: array<u32, 8>;    mod_sq(&r, &r2);
    var u1h2_2: array<u32, 8>; mod_add(&u1h2, &u1h2, &u1h2_2);

    var x3: array<u32, 8>;    mod_sub(&r2, &h3, &x3);
    var x3_2: array<u32, 8>;  mod_sub(&x3, &u1h2_2, &x3_2);
    var tmp: array<u32, 8>;   mod_sub(&u1h2, &x3_2, &tmp);
    var r_tmp: array<u32, 8>; mod_mul(&r, &tmp, &r_tmp);
    var s1h3: array<u32, 8>;  mod_mul(&s1, &h3, &s1h3);
    var y3: array<u32, 8>;    mod_sub(&r_tmp, &s1h3, &y3);
    var hz1: array<u32, 8>;   mod_mul(&h, &az, &hz1);
    var z3: array<u32, 8>;    mod_mul(&hz1, &bz, &z3);

    return Jacobian(x3_2, y3, z3);
}

// ── Jacobian mixed add: r = a + b (b is affine, z_b = 1) ──
fn point_add_mixed(a: ptr<function, Jacobian>, bx: ptr<function, array<u32, 8>>,
                   by: ptr<function, array<u32, 8>>) -> Jacobian {
    var az = (*a).z;

    var z1_2: array<u32, 8>;  mod_sq(&az, &z1_2);
    var u1    = (*a).x;
    var u2: array<u32, 8>;    mod_mul(bx, &z1_2, &u2);
    var z1_3: array<u32, 8>;  mod_mul(&z1_2, &az, &z1_3);
    var s1    = (*a).y;
    var s2: array<u32, 8>;    mod_mul(by, &z1_3, &s2);
    var h: array<u32, 8>;     mod_sub(&u2, &u1, &h);
    var r: array<u32, 8>;     mod_sub(&s2, &s1, &r);

    var zero: array<u32, 8>; zero_256(&zero);
    if (is_zero_256(&h)) {
        if (is_zero_256(&r)) { return point_double(a); }
        return Jacobian(zero, zero, zero);
    }

    var h2: array<u32, 8>;    mod_sq(&h, &h2);
    var h3: array<u32, 8>;    mod_mul(&h2, &h, &h3);
    var u1h2: array<u32, 8>;  mod_mul(&u1, &h2, &u1h2);
    var r2: array<u32, 8>;    mod_sq(&r, &r2);
    var u1h2_2: array<u32, 8>; mod_add(&u1h2, &u1h2, &u1h2_2);

    var x3: array<u32, 8>;    mod_sub(&r2, &h3, &x3);
    var x3_2: array<u32, 8>;  mod_sub(&x3, &u1h2_2, &x3_2);
    var tmp: array<u32, 8>;   mod_sub(&u1h2, &x3_2, &tmp);
    var r_tmp: array<u32, 8>; mod_mul(&r, &tmp, &r_tmp);
    var s1h3: array<u32, 8>;  mod_mul(&s1, &h3, &s1h3);
    var y3: array<u32, 8>;    mod_sub(&r_tmp, &s1h3, &y3);
    var z3: array<u32, 8>;    mod_mul(&h, &az, &z3);

    return Jacobian(x3_2, y3, z3);
}

// ── XOR-fold hash of X (affine) for jump index ──
fn hash_x(x: ptr<function, array<u32, 8>>) -> u32 {
    var h: u32 = 0u;
    for (var i = 0u; i < 8u; i++) { h = h ^ (*x)[i]; }
    return h;
}

// ── Distinguished point test ──
fn is_distinguished(x: ptr<function, array<u32, 8>>, bits: u32) -> bool {
    if (bits == 0u) { return true; }
    let h = hash_x(x);
    let mask = (1u << bits) - 1u;
    return (h & mask) == 0u;
}

// ═══════════════════════════════════════════════════════════════════
// Buffer structures (must match Rust host)
// ═══════════════════════════════════════════════════════════════════

struct Params {
    target_x      : array<u32, 8>,
    target_y      : array<u32, 8>,
    dist_bits     : u32,
    table_size    : u32,
    _pad          : u32,
    negate_y      : u32,
    dp_capacity   : u32,
}

struct KangarooInit {
    dist_x        : array<u32, 8>,
    dist_y        : array<u32, 8>,
    distance      : array<u32, 8>,
    is_tame       : u32,
}

struct JumpTablePoint {
    dx            : array<u32, 8>,
    dy            : array<u32, 8>,
}

struct JumpTableDist {
    ddist         : array<u32, 8>,
}

struct DistPointOutput {
    x             : array<u32, 8>,
    distance      : array<u32, 8>,
    dist_type     : u32,
}

struct FoundKeyOutput {
    key           : array<u32, 8>,
}

// ═══════════════════════════════════════════════════════════════════
// Compute shader entry point
// ═══════════════════════════════════════════════════════════════════

@group(0) @binding(0) var<storage, read> params: Params;
@group(0) @binding(1) var<storage, read_write> kangaroos: array<KangarooInit>;
@group(0) @binding(2) var<storage, read> jump_points: array<JumpTablePoint>;
@group(0) @binding(3) var<storage, read> jump_dists: array<JumpTableDist>;
@group(0) @binding(4) var<storage, read_write> dist_points: array<DistPointOutput>;
@group(0) @binding(5) var<storage, read_write> dist_count: atomic<u32>;
@group(0) @binding(6) var<storage, read_write> found_key: array<FoundKeyOutput>;
@group(0) @binding(7) var<storage, read_write> output_state: array<KangarooInit>;

const WG_SIZE: u32 = 64u;
const MAX_STEPS: u32 = 65535u;
const DP_REPORT_INTERVAL: u32 = 256u;

@compute @workgroup_size(64)
fn main(@builtin(local_invocation_id) lid: vec3<u32>,
        @builtin(global_invocation_id) gid: vec3<u32>) {
    let idx = gid.x;
    let local_id = lid.x;

    // ── Load initial state from storage ──
    var pt: Jacobian;
    for (var i = 0u; i < 8u; i++) { pt.x[i] = kangaroos[idx].dist_x[i]; }
    for (var i = 0u; i < 8u; i++) { pt.y[i] = kangaroos[idx].dist_y[i]; }
    for (var i = 0u; i < 8u; i++) { pt.z[i] = 0u; }
    pt.z[0] = 1u;

    var dist: array<u32, 8>;
    for (var i = 0u; i < 8u; i++) { dist[i] = kangaroos[idx].distance[i]; }

    let is_tame = kangaroos[idx].is_tame != 0u;
    let negate  = params.negate_y != 0u;

    var step: u32 = 0u;

    var found: array<u32, 8>;
    for (var i = 0u; i < 8u; i++) { found[i] = found_key[0u].key[i]; }
    var key_is_zero = is_zero_256(&found);

    while (step < MAX_STEPS && key_is_zero) {
        // ── Step 1: Per-thread Z inverse (avoids AMD driver crash with barriers + point_add_mixed) ──
        var inv_z: array<u32, 8>;
        mod_inv(&pt.z, &inv_z);

        // ── Step 2: Compute affine X ──
        var inv_z_sq: array<u32, 8>;
        mod_sq(&inv_z, &inv_z_sq);

        var x_aff: array<u32, 8>;
        mod_mul(&pt.x, &inv_z_sq, &x_aff);

        // ── Step 3: Y_aff for negation map ──
        if (negate) {
            var y_aff: array<u32, 8>;
            var inv_z_cb: array<u32, 8>;
            mod_mul(&inv_z_sq, &inv_z, &inv_z_cb);
            mod_mul(&pt.y, &inv_z_cb, &y_aff);

            if ((y_aff[7u] & 0x80u) != 0u) {
                for (var i = 0u; i < 8u; i++) { pt.y[i] = P[i] - pt.y[i]; }
            }
        }

        // ── Step 4: Jump index and apply jump ──
        let h = hash_x(&x_aff);
        let ji = h % params.table_size;

        var jp_x: array<u32, 8>;
        var jp_y: array<u32, 8>;
        for (var i = 0u; i < 8u; i++) { jp_x[i] = jump_points[ji].dx[i]; }
        for (var i = 0u; i < 8u; i++) { jp_y[i] = jump_points[ji].dy[i]; }

        var new_pt = point_add_mixed(&pt, &jp_x, &jp_y);
        point_copy(&pt, &new_pt);

        // ── Step 5: Update distance ──
        var jd_ddist: array<u32, 8>;
        for (var i = 0u; i < 8u; i++) { jd_ddist[i] = jump_dists[ji].ddist[i]; }
        mod_add(&dist, &jd_ddist, &dist);

        step++;

        // ── Step 6: Distinguished point check ──
        if (step % DP_REPORT_INTERVAL == 0u) {
            if (is_distinguished(&x_aff, params.dist_bits)) {
                let dp_idx = atomicAdd(&dist_count, 1u);
                if (dp_idx < params.dp_capacity) {
                    for (var i = 0u; i < 8u; i++) { dist_points[dp_idx].x[i] = x_aff[i]; }
                    for (var i = 0u; i < 8u; i++) { dist_points[dp_idx].distance[i] = dist[i]; }
                    dist_points[dp_idx].dist_type = select(1u, 0u, is_tame);
                }
            }

            for (var i = 0u; i < 8u; i++) { found[i] = found_key[0u].key[i]; }
            key_is_zero = is_zero_256(&found);
        }
    }

    // ── Write final state back to host ──
    var inv_z_final: array<u32, 8>;
    mod_inv(&pt.z, &inv_z_final);

    var inv_z_sq_final: array<u32, 8>;
    mod_sq(&inv_z_final, &inv_z_sq_final);

    var x_aff_final: array<u32, 8>;
    mod_mul(&pt.x, &inv_z_sq_final, &x_aff_final);

    var inv_z_cb_final: array<u32, 8>;
    mod_mul(&inv_z_sq_final, &inv_z_final, &inv_z_cb_final);

    var y_aff_final: array<u32, 8>;
    mod_mul(&pt.y, &inv_z_cb_final, &y_aff_final);

    for (var i = 0u; i < 8u; i++) { output_state[idx].dist_x[i] = x_aff_final[i]; }
    for (var i = 0u; i < 8u; i++) { output_state[idx].dist_y[i] = y_aff_final[i]; }
    for (var i = 0u; i < 8u; i++) { output_state[idx].distance[i] = dist[i]; }
    output_state[idx].is_tame = select(0u, 1u, is_tame);
}
