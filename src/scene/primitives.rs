//! Procedural 3D primitive mesh generators.
//!
//! All generators return an `Arc<MeshAsset3D>` ready for use as `Mesh3D::Custom(_)`.
//! Vertices are in the range [-1, 1] on each axis with `base_size` encoding the diameter,
//! so the caller can scale via `MeshDraw3D::size` like any other mesh.

use std::f32::consts::{PI, TAU};
use std::sync::Arc;

use super::{MeshAsset3D, MeshVertex3D};

// ── Public convenience entry points ────────────────────────────────────────

/// Tessellated cylinder, aligned on the Y-axis. Caps are included.
///
/// * `radial_segments` — number of slices around the circumference (≥ 3)
/// * `height_segments` — subdivisions along the height axis (≥ 1)
pub fn generate_cylinder(
    name: impl Into<String>,
    radial_segments: u32,
    height_segments: u32,
) -> Arc<MeshAsset3D> {
    let radial = radial_segments.max(3) as usize;
    let height = height_segments.max(1) as usize;
    let mut vertices = Vec::new();

    // ── Side faces ─────────────────────────────────────────────────────────
    for h in 0..height {
        let y0 = (h as f32 / height as f32) * 2.0 - 1.0;
        let y1 = ((h + 1) as f32 / height as f32) * 2.0 - 1.0;
        let v0 = h as f32 / height as f32;
        let v1 = (h + 1) as f32 / height as f32;

        for r in 0..radial {
            let angle0 = r as f32 / radial as f32 * TAU;
            let angle1 = (r + 1) as f32 / radial as f32 * TAU;
            let u0 = r as f32 / radial as f32;
            let u1 = (r + 1) as f32 / radial as f32;

            let (s0, c0) = angle0.sin_cos();
            let (s1, c1) = angle1.sin_cos();

            let p00 = [c0, y0, s0];
            let p10 = [c1, y0, s1];
            let p11 = [c1, y1, s1];
            let p01 = [c0, y1, s0];
            let n0 = [c0, 0.0, s0];
            let n1 = [c1, 0.0, s1];
            let t0 = [-s0, 0.0, c0, 1.0_f32]; // tangent along ring direction
            let t1 = [-s1, 0.0, c1, 1.0_f32];

            // Two triangles per quad
            push_tri(
                &mut vertices,
                (p00, n0, [u0, v0], t0),
                (p10, n1, [u1, v0], t1),
                (p11, n1, [u1, v1], t1),
            );
            push_tri(
                &mut vertices,
                (p00, n0, [u0, v0], t0),
                (p11, n1, [u1, v1], t1),
                (p01, n0, [u0, v1], t0),
            );
        }
    }

    // ── Top cap (y = +1) ──────────────────────────────────────────────────
    append_disk_cap(&mut vertices, 1.0, [0.0, 1.0, 0.0], true, radial);

    // ── Bottom cap (y = -1) ───────────────────────────────────────────────
    append_disk_cap(&mut vertices, -1.0, [0.0, -1.0, 0.0], false, radial);

    Arc::new(MeshAsset3D {
        name: name.into(),
        vertices: Arc::<[MeshVertex3D]>::from(vertices),
        base_size: [2.0, 2.0, 2.0],
    })
}

/// Torus (donut) shape.
///
/// * `major_segments` — slices around the main ring (≥ 3)
/// * `minor_segments` — slices around the tube cross-section (≥ 3)
///
/// The torus lies in the XZ-plane. Major radius = 1.0 (sets `base_size`);
/// minor radius is 0.35 of that and can be scaled with `MeshDraw3D::size`.
pub fn generate_torus(
    name: impl Into<String>,
    major_segments: u32,
    minor_segments: u32,
) -> Arc<MeshAsset3D> {
    let maj = major_segments.max(3) as usize;
    let min = minor_segments.max(3) as usize;
    let major_r = 0.65_f32; // in [-1,1] space
    let minor_r = 0.35_f32;
    let mut vertices = Vec::new();

    for i in 0..maj {
        let theta0 = i as f32 / maj as f32 * TAU;
        let theta1 = (i + 1) as f32 / maj as f32 * TAU;
        let u0 = i as f32 / maj as f32;
        let u1 = (i + 1) as f32 / maj as f32;

        for j in 0..min {
            let phi0 = j as f32 / min as f32 * TAU;
            let phi1 = (j + 1) as f32 / min as f32 * TAU;
            let v0 = j as f32 / min as f32;
            let v1 = (j + 1) as f32 / min as f32;

            let p00 = torus_point(theta0, phi0, major_r, minor_r);
            let p10 = torus_point(theta1, phi0, major_r, minor_r);
            let p11 = torus_point(theta1, phi1, major_r, minor_r);
            let p01 = torus_point(theta0, phi1, major_r, minor_r);

            let n00 = torus_normal(theta0, phi0);
            let n10 = torus_normal(theta1, phi0);
            let n11 = torus_normal(theta1, phi1);
            let n01 = torus_normal(theta0, phi1);

            // Tangent along major ring direction
            let t0 = [-theta0.sin(), 0.0, theta0.cos(), 1.0_f32];
            let t1 = [-theta1.sin(), 0.0, theta1.cos(), 1.0_f32];

            push_tri(
                &mut vertices,
                (p00, n00, [u0, v0], t0),
                (p10, n10, [u1, v0], t1),
                (p11, n11, [u1, v1], t1),
            );
            push_tri(
                &mut vertices,
                (p00, n00, [u0, v0], t0),
                (p11, n11, [u1, v1], t1),
                (p01, n01, [u0, v1], t0),
            );
        }
    }

    Arc::new(MeshAsset3D {
        name: name.into(),
        vertices: Arc::<[MeshVertex3D]>::from(vertices),
        base_size: [2.0, 2.0 * minor_r, 2.0],
    })
}

/// Cone, apex at y = +1, base circle at y = -1.
///
/// * `radial_segments` — slices around the base (≥ 3)
/// * `height_segments` — subdivisions along the height axis (≥ 1)
pub fn generate_cone(
    name: impl Into<String>,
    radial_segments: u32,
    height_segments: u32,
) -> Arc<MeshAsset3D> {
    let radial = radial_segments.max(3) as usize;
    let height = height_segments.max(1) as usize;
    let mut vertices = Vec::new();

    // ── Side faces ─────────────────────────────────────────────────────────
    for h in 0..height {
        let y0 = (h as f32 / height as f32) * 2.0 - 1.0;
        let y1 = ((h + 1) as f32 / height as f32) * 2.0 - 1.0;
        // Radius linearly decreases from 1 at base to 0 at apex
        let r0 = 1.0 - (h as f32 / height as f32);
        let r1 = 1.0 - ((h + 1) as f32 / height as f32);
        let v0 = h as f32 / height as f32;
        let v1 = (h + 1) as f32 / height as f32;

        for r in 0..radial {
            let angle0 = r as f32 / radial as f32 * TAU;
            let angle1 = (r + 1) as f32 / radial as f32 * TAU;
            let u0 = r as f32 / radial as f32;
            let u1 = (r + 1) as f32 / radial as f32;

            let (s0, c0) = angle0.sin_cos();
            let (s1, c1) = angle1.sin_cos();

            let p00 = [c0 * r0, y0, s0 * r0];
            let p10 = [c1 * r0, y0, s1 * r0];
            let p11 = [c1 * r1, y1, s1 * r1];
            let p01 = [c0 * r1, y1, s0 * r1];

            // Cone normal: outward + upward tilt by slope angle
            let slope = 1.0_f32 / 2.0_f32.sqrt(); // 45-degree slope for unit cone
            let n0 = prim_normalize([c0 * slope, slope, s0 * slope]);
            let n1 = prim_normalize([c1 * slope, slope, s1 * slope]);
            let t0 = [-s0, 0.0, c0, 1.0_f32];
            let t1 = [-s1, 0.0, c1, 1.0_f32];

            push_tri(
                &mut vertices,
                (p00, n0, [u0, v0], t0),
                (p10, n1, [u1, v0], t1),
                (p11, n1, [u1, v1], t1),
            );
            if r0 > 0.001 {
                push_tri(
                    &mut vertices,
                    (p00, n0, [u0, v0], t0),
                    (p11, n1, [u1, v1], t1),
                    (p01, n0, [u0, v1], t0),
                );
            }
        }
    }

    // ── Base cap (y = -1) ─────────────────────────────────────────────────
    append_disk_cap(&mut vertices, -1.0, [0.0, -1.0, 0.0], false, radial);

    Arc::new(MeshAsset3D {
        name: name.into(),
        vertices: Arc::<[MeshVertex3D]>::from(vertices),
        base_size: [2.0, 2.0, 2.0],
    })
}

/// Capsule — cylinder with hemispherical end-caps, aligned on the Y-axis.
///
/// * `radial_segments` — slices around the circumference (≥ 4)
/// * `cap_segments` — latitude subdivisions per hemisphere (≥ 2)
pub fn generate_capsule(
    name: impl Into<String>,
    radial_segments: u32,
    cap_segments: u32,
) -> Arc<MeshAsset3D> {
    let radial = radial_segments.max(4) as usize;
    let caps = cap_segments.max(2) as usize;
    let mut vertices = Vec::new();
    // In [-1, 1] space: cylinder body from y=-0.5 to y=0.5, caps extend to ±1.
    let body_half = 0.5_f32;
    let cap_r = 0.5_f32; // hemisphere radius in local space = 0.5

    // ── Top hemisphere ─────────────────────────────────────────────────────
    for lat in 0..caps {
        let phi0 = lat as f32 / caps as f32 * (PI * 0.5);
        let phi1 = (lat + 1) as f32 / caps as f32 * (PI * 0.5);
        let v0 = lat as f32 / caps as f32 * 0.25;
        let v1 = (lat + 1) as f32 / caps as f32 * 0.25;

        for r in 0..radial {
            let angle0 = r as f32 / radial as f32 * TAU;
            let angle1 = (r + 1) as f32 / radial as f32 * TAU;
            let u0 = r as f32 / radial as f32;
            let u1 = (r + 1) as f32 / radial as f32;

            let p00 = hemi_point(angle0, phi0, cap_r, body_half, true);
            let p10 = hemi_point(angle1, phi0, cap_r, body_half, true);
            let p11 = hemi_point(angle1, phi1, cap_r, body_half, true);
            let p01 = hemi_point(angle0, phi1, cap_r, body_half, true);
            let n00 = sphere_normal(angle0, phi0, true);
            let n10 = sphere_normal(angle1, phi0, true);
            let n11 = sphere_normal(angle1, phi1, true);
            let n01 = sphere_normal(angle0, phi1, true);
            let t0 = [-angle0.sin(), 0.0, angle0.cos(), 1.0_f32];
            let t1 = [-angle1.sin(), 0.0, angle1.cos(), 1.0_f32];

            push_tri(
                &mut vertices,
                (p00, n00, [u0, v0], t0),
                (p10, n10, [u1, v0], t1),
                (p11, n11, [u1, v1], t1),
            );
            push_tri(
                &mut vertices,
                (p00, n00, [u0, v0], t0),
                (p11, n11, [u1, v1], t1),
                (p01, n01, [u0, v1], t0),
            );
        }
    }

    // ── Cylinder body ──────────────────────────────────────────────────────
    for r in 0..radial {
        let angle0 = r as f32 / radial as f32 * TAU;
        let angle1 = (r + 1) as f32 / radial as f32 * TAU;
        let u0 = r as f32 / radial as f32;
        let u1 = (r + 1) as f32 / radial as f32;
        let (s0, c0) = angle0.sin_cos();
        let (s1, c1) = angle1.sin_cos();

        let p00 = [c0 * cap_r, -body_half, s0 * cap_r];
        let p10 = [c1 * cap_r, -body_half, s1 * cap_r];
        let p11 = [c1 * cap_r, body_half, s1 * cap_r];
        let p01 = [c0 * cap_r, body_half, s0 * cap_r];
        let n0 = [c0, 0.0, s0];
        let n1 = [c1, 0.0, s1];
        let t0 = [-s0, 0.0, c0, 1.0_f32];
        let t1 = [-s1, 0.0, c1, 1.0_f32];

        push_tri(
            &mut vertices,
            (p00, n0, [u0, 0.25], t0),
            (p10, n1, [u1, 0.25], t1),
            (p11, n1, [u1, 0.75], t1),
        );
        push_tri(
            &mut vertices,
            (p00, n0, [u0, 0.25], t0),
            (p11, n1, [u1, 0.75], t1),
            (p01, n0, [u0, 0.75], t0),
        );
    }

    // ── Bottom hemisphere ──────────────────────────────────────────────────
    for lat in 0..caps {
        let phi0 = lat as f32 / caps as f32 * (PI * 0.5);
        let phi1 = (lat + 1) as f32 / caps as f32 * (PI * 0.5);
        let v0 = 0.75 + lat as f32 / caps as f32 * 0.25;
        let v1 = 0.75 + (lat + 1) as f32 / caps as f32 * 0.25;

        for r in 0..radial {
            let angle0 = r as f32 / radial as f32 * TAU;
            let angle1 = (r + 1) as f32 / radial as f32 * TAU;
            let u0 = r as f32 / radial as f32;
            let u1 = (r + 1) as f32 / radial as f32;

            let p00 = hemi_point(angle0, phi0, cap_r, body_half, false);
            let p10 = hemi_point(angle1, phi0, cap_r, body_half, false);
            let p11 = hemi_point(angle1, phi1, cap_r, body_half, false);
            let p01 = hemi_point(angle0, phi1, cap_r, body_half, false);
            let n00 = sphere_normal(angle0, phi0, false);
            let n10 = sphere_normal(angle1, phi0, false);
            let n11 = sphere_normal(angle1, phi1, false);
            let n01 = sphere_normal(angle0, phi1, false);
            let t0 = [-angle0.sin(), 0.0, angle0.cos(), 1.0_f32];
            let t1 = [-angle1.sin(), 0.0, angle1.cos(), 1.0_f32];

            push_tri(
                &mut vertices,
                (p00, n00, [u0, v0], t0),
                (p11, n11, [u1, v1], t1),
                (p10, n10, [u1, v0], t1),
            );
            push_tri(
                &mut vertices,
                (p00, n00, [u0, v0], t0),
                (p01, n01, [u0, v1], t0),
                (p11, n11, [u1, v1], t1),
            );
        }
    }

    Arc::new(MeshAsset3D {
        name: name.into(),
        vertices: Arc::<[MeshVertex3D]>::from(vertices),
        base_size: [2.0 * cap_r, body_half * 2.0 + cap_r * 2.0, 2.0 * cap_r],
    })
}

/// Icosphere — UV-sphere approximation via icosahedron subdivision.
///
/// * `subdivisions` — number of midpoint-split passes (0 = raw icosahedron, 3 is smooth)
pub fn generate_icosphere(name: impl Into<String>, subdivisions: u32) -> Arc<MeshAsset3D> {
    // Start from an icosahedron
    let phi = (1.0 + 5.0_f32.sqrt()) * 0.5;
    let s = 1.0 / (1.0 + phi * phi).sqrt();
    let t = phi * s;

    #[rustfmt::skip]
    let raw: [[f32; 3]; 12] = [
        [-s,  t,  0.0], [ s,  t,  0.0], [-s, -t,  0.0], [ s, -t,  0.0],
        [ 0.0, -s,  t], [ 0.0,  s,  t], [ 0.0, -s, -t], [ 0.0,  s, -t],
        [ t,  0.0, -s], [ t,  0.0,  s], [-t,  0.0, -s], [-t,  0.0,  s],
    ];

    #[rustfmt::skip]
    let indices: Vec<[usize; 3]> = vec![
        [0,11,5],[0,5,1],[0,1,7],[0,7,10],[0,10,11],
        [1,5,9],[5,11,4],[11,10,2],[10,7,6],[7,1,8],
        [3,9,4],[3,4,2],[3,2,6],[3,6,8],[3,8,9],
        [4,9,5],[2,4,11],[6,2,10],[8,6,7],[9,8,1],
    ];

    let mut points: Vec<[f32; 3]> = raw.to_vec();
    let mut tris = indices;

    for _ in 0..subdivisions {
        let mut next_tris = Vec::with_capacity(tris.len() * 4);
        for [a, b, c] in &tris {
            let ab = midpoint_on_sphere(&points, *a, *b, &mut points);
            let bc = midpoint_on_sphere(&points, *b, *c, &mut points);
            let ca = midpoint_on_sphere(&points, *c, *a, &mut points);
            next_tris.push([*a, ab, ca]);
            next_tris.push([*b, bc, ab]);
            next_tris.push([*c, ca, bc]);
            next_tris.push([ab, bc, ca]);
        }
        tris = next_tris;
    }

    let mut vertices = Vec::with_capacity(tris.len() * 3);
    for [a, b, c] in &tris {
        for &idx in &[*a, *b, *c] {
            let p = points[idx];
            let n = prim_normalize(p); // sphere: normal == position
            // UV from spherical coordinates
            let u = p[2].atan2(p[0]) / TAU + 0.5;
            let v = p[1].asin() / PI + 0.5;
            // Tangent perpendicular to N in XZ plane
            let tx = -p[2].atan2(p[0]).sin();
            let tz = p[2].atan2(p[0]).cos();
            let tangent = [tx, 0.0, tz, 1.0];
            vertices.push(MeshVertex3D {
                position: p,
                normal: n,
                uv: [u, v],
                tangent,
            });
        }
    }

    Arc::new(MeshAsset3D {
        name: name.into(),
        vertices: Arc::<[MeshVertex3D]>::from(vertices),
        base_size: [2.0, 2.0, 2.0],
    })
}

// ── Internal helpers ────────────────────────────────────────────────────────

fn push_tri(
    out: &mut Vec<MeshVertex3D>,
    a: ([f32; 3], [f32; 3], [f32; 2], [f32; 4]),
    b: ([f32; 3], [f32; 3], [f32; 2], [f32; 4]),
    c: ([f32; 3], [f32; 3], [f32; 2], [f32; 4]),
) {
    out.push(MeshVertex3D { position: a.0, normal: a.1, uv: a.2, tangent: a.3 });
    out.push(MeshVertex3D { position: b.0, normal: b.1, uv: b.2, tangent: b.3 });
    out.push(MeshVertex3D { position: c.0, normal: c.1, uv: c.2, tangent: c.3 });
}

fn append_disk_cap(
    out: &mut Vec<MeshVertex3D>,
    y: f32,
    normal: [f32; 3],
    top: bool,
    segments: usize,
) {
    let center = [0.0_f32, y, 0.0];
    for r in 0..segments {
        let angle0 = r as f32 / segments as f32 * TAU;
        let angle1 = (r + 1) as f32 / segments as f32 * TAU;
        let (s0, c0) = angle0.sin_cos();
        let (s1, c1) = angle1.sin_cos();
        let p0 = [c0, y, s0];
        let p1 = [c1, y, s1];
        let uv_c = [0.5, 0.5];
        let uv0 = [0.5 + c0 * 0.5, 0.5 + s0 * 0.5];
        let uv1 = [0.5 + c1 * 0.5, 0.5 + s1 * 0.5];
        let t = [1.0, 0.0, 0.0, 1.0_f32];

        if top {
            push_tri(out, (center, normal, uv_c, t), (p0, normal, uv0, t), (p1, normal, uv1, t));
        } else {
            push_tri(out, (center, normal, uv_c, t), (p1, normal, uv1, t), (p0, normal, uv0, t));
        }
    }
}

fn torus_point(theta: f32, phi: f32, major_r: f32, minor_r: f32) -> [f32; 3] {
    let (st, ct) = theta.sin_cos();
    let (sp, cp) = phi.sin_cos();
    [
        (major_r + minor_r * cp) * ct,
        minor_r * sp,
        (major_r + minor_r * cp) * st,
    ]
}

fn torus_normal(theta: f32, phi: f32) -> [f32; 3] {
    let (st, ct) = theta.sin_cos();
    let (sp, cp) = phi.sin_cos();
    prim_normalize([cp * ct, sp, cp * st])
}

fn hemi_point(angle: f32, phi: f32, cap_r: f32, body_half: f32, top: bool) -> [f32; 3] {
    let (sa, ca) = angle.sin_cos();
    let (sp, cp) = phi.sin_cos();
    let y_sign = if top { 1.0 } else { -1.0 };
    [ca * cp * cap_r, y_sign * (sp * cap_r + body_half), sa * cp * cap_r]
}

fn sphere_normal(angle: f32, phi: f32, top: bool) -> [f32; 3] {
    let (sa, ca) = angle.sin_cos();
    let (sp, cp) = phi.sin_cos();
    let y_sign = if top { 1.0 } else { -1.0 };
    prim_normalize([ca * cp, y_sign * sp, sa * cp])
}

fn midpoint_on_sphere(
    points: &[[f32; 3]],
    a: usize,
    b: usize,
    out: &mut Vec<[f32; 3]>,
) -> usize {
    let mid = [
        (points[a][0] + points[b][0]) * 0.5,
        (points[a][1] + points[b][1]) * 0.5,
        (points[a][2] + points[b][2]) * 0.5,
    ];
    let normalized = prim_normalize(mid);
    let idx = out.len();
    out.push(normalized);
    idx
}

fn prim_normalize(v: [f32; 3]) -> [f32; 3] {
    let len = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt().max(0.0001);
    [v[0] / len, v[1] / len, v[2] / len]
}

// ── Unit tests ──────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cylinder_has_geometry() {
        let m = generate_cylinder("test_cyl", 8, 2);
        assert!(!m.vertices.is_empty());
        // All vertex counts must be multiples of 3 (triangle list)
        assert_eq!(m.vertices.len() % 3, 0);
    }

    #[test]
    fn torus_has_geometry() {
        let m = generate_torus("test_torus", 12, 8);
        assert!(!m.vertices.is_empty());
        assert_eq!(m.vertices.len() % 3, 0);
    }

    #[test]
    fn cone_has_geometry() {
        let m = generate_cone("test_cone", 8, 2);
        assert!(!m.vertices.is_empty());
        assert_eq!(m.vertices.len() % 3, 0);
    }

    #[test]
    fn capsule_has_geometry() {
        let m = generate_capsule("test_capsule", 8, 4);
        assert!(!m.vertices.is_empty());
        assert_eq!(m.vertices.len() % 3, 0);
    }

    #[test]
    fn icosphere_raw_has_20_triangles() {
        let m = generate_icosphere("test_ico", 0);
        assert_eq!(m.vertices.len(), 20 * 3);
    }

    #[test]
    fn icosphere_subdivisions_multiply_triangles() {
        let m0 = generate_icosphere("ico0", 0);
        let m1 = generate_icosphere("ico1", 1);
        let m2 = generate_icosphere("ico2", 2);
        assert_eq!(m1.vertices.len(), m0.vertices.len() * 4);
        assert_eq!(m2.vertices.len(), m1.vertices.len() * 4);
    }

    #[test]
    fn icosphere_normals_are_unit_length() {
        let m = generate_icosphere("ico_n", 2);
        for v in m.vertices.iter() {
            let n = v.normal;
            let len = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
            assert!((len - 1.0).abs() < 0.0001, "normal not unit: len={len}");
        }
    }
}
