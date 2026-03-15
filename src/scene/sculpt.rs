//! CPU-side sculpting system for deforming mesh geometry interactively.
//!
//! # Workflow
//! 1. Build a [`SculptMesh`] from any `MeshAsset3D` (or a primitive generator).
//! 2. On each input event, call [`SculptMesh::apply_brush`] with a world-space
//!    hit position and your chosen [`SculptBrush`] parameters.
//! 3. Call [`SculptMesh::to_asset`] each frame to get an updated `Arc<MeshAsset3D>`
//!    that can be handed to `MeshDraw3D` for rendering.
//!
//! Normals are recomputed automatically after every brush stroke.

use std::sync::Arc;

use super::{MeshAsset3D, MeshVertex3D};

// ── Brush configuration ─────────────────────────────────────────────────────

/// How the brush modifies the surface.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BrushMode {
    /// Displace vertices along their vertex normal (pulls outward).
    Inflate,
    /// Displace vertices inward along their vertex normal.
    Deflate,
    /// Move vertices toward the brush hit point projected along the surface normal
    /// (equivalent to "Grab" with inward limit).
    Push,
    /// Move vertices away from the hit point.
    Pull,
    /// Average each vertex position with its neighbors (weighted by influence).
    Smooth,
    /// Flatten vertices toward the average plane of the affected region.
    Flatten,
    /// Move vertices toward the brush center (pinch/crease).
    Pinch,
}

/// Falloff curve that controls how brush influence fades toward the outer edge.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BrushFalloff {
    /// Linear fade from 1 at centre to 0 at radius.
    Linear,
    /// Smooth S-curve (smoothstep).
    Smooth,
    /// Constant — full strength everywhere inside the radius.
    Constant,
    /// Spherical falloff: sqrt(1 - (d/r)²).
    Sphere,
}

impl BrushFalloff {
    /// Compute an influence weight in [0, 1] from a normalised distance `t ∈ [0, 1]`.
    pub fn weight(self, t: f32) -> f32 {
        let t = t.clamp(0.0, 1.0);
        match self {
            Self::Linear => 1.0 - t,
            Self::Smooth => {
                let s = 1.0 - t;
                s * s * (3.0 - 2.0 * s)
            }
            Self::Constant => 1.0,
            Self::Sphere => (1.0 - t * t).max(0.0).sqrt(),
        }
    }
}

/// Parameters for a sculpting brush stroke.
#[derive(Debug, Clone, Copy)]
pub struct SculptBrush {
    pub mode: BrushMode,
    /// World-space radius of influence (in mesh-local normalised space, typically 0–2).
    pub radius: f32,
    /// Maximum displacement per second at full influence.
    pub strength: f32,
    pub falloff: BrushFalloff,
    /// If `true`, symmetry is applied across the X axis (mirror sculpting).
    pub symmetric_x: bool,
}

impl Default for SculptBrush {
    fn default() -> Self {
        Self {
            mode: BrushMode::Inflate,
            radius: 0.4,
            strength: 0.8,
            falloff: BrushFalloff::Smooth,
            symmetric_x: false,
        }
    }
}

// ── SculptMesh ──────────────────────────────────────────────────────────────

/// An editable mesh that supports brush-based sculpting.
///
/// Positions and normals are stored separately from UVs and tangents so that
/// deformation only touches the parts of the vertex that actually change.
pub struct SculptMesh {
    name: String,
    positions: Vec<[f32; 3]>,
    normals: Vec<[f32; 3]>,
    uvs: Vec<[f32; 2]>,
    tangents: Vec<[f32; 4]>,
    base_size: [f32; 3],
    /// Cached vertex count — normals are face-averaged so we store per-vertex normals.
    dirty: bool,
}

impl SculptMesh {
    /// Create a `SculptMesh` from an existing `MeshAsset3D`.
    pub fn from_asset(asset: &MeshAsset3D) -> Self {
        let n = asset.vertices.len();
        let mut positions = Vec::with_capacity(n);
        let mut normals = Vec::with_capacity(n);
        let mut uvs = Vec::with_capacity(n);
        let mut tangents = Vec::with_capacity(n);
        for v in asset.vertices.iter() {
            positions.push(v.position);
            normals.push(v.normal);
            uvs.push(v.uv);
            tangents.push(v.tangent);
        }
        Self {
            name: asset.name.clone(),
            positions,
            normals,
            uvs,
            tangents,
            base_size: asset.base_size,
            dirty: false,
        }
    }

    /// Number of vertices.
    pub fn vertex_count(&self) -> usize {
        self.positions.len()
    }

    /// Apply a brush stroke at `hit_position` (mesh-local space) for `delta_time` seconds.
    ///
    /// The brush reads from the current normals to determine displacement direction, then
    /// recomputes all normals after the stroke.
    pub fn apply_brush(&mut self, brush: &SculptBrush, hit_position: [f32; 3], delta_time: f32) {
        let scale = brush.strength * delta_time;
        let r2 = brush.radius * brush.radius;

        // --- First pass: collect the affected region's average normal / centroid ---
        let mut avg_normal = [0.0_f32; 3];
        let mut avg_position = [0.0_f32; 3];
        let mut total_weight = 0.0_f32;

        for i in 0..self.positions.len() {
            let d2 = dist2(self.positions[i], hit_position);
            if d2 >= r2 {
                continue;
            }
            let t = (d2 / r2).sqrt();
            let w = brush.falloff.weight(t);
            avg_normal = add3(avg_normal, scale3(self.normals[i], w));
            avg_position = add3(avg_position, scale3(self.positions[i], w));
            total_weight += w;
        }

        if total_weight < 0.0001 {
            return;
        }

        let avg_normal = normalize3(avg_normal);
        let avg_position = scale3(avg_position, 1.0 / total_weight);

        // --- Second pass: apply displacement ---
        let n = self.positions.len();

        // For Smooth: gather neighbour averages (triangle-sharing)
        // We approximate by averaging positions of all vertices within radius.
        let smooth_targets: Vec<[f32; 3]> = if brush.mode == BrushMode::Smooth {
            (0..n)
                .map(|i| {
                    let d2 = dist2(self.positions[i], hit_position);
                    if d2 >= r2 {
                        return self.positions[i];
                    }
                    // Average with all triangle neighbours (same triangle-list index groups of 3)
                    let tri_idx = i / 3;
                    let base = tri_idx * 3;
                    let a = self.positions[base];
                    let b = self.positions[(base + 1).min(n - 1)];
                    let c = self.positions[(base + 2).min(n - 1)];
                    [
                        (a[0] + b[0] + c[0]) / 3.0,
                        (a[1] + b[1] + c[1]) / 3.0,
                        (a[2] + b[2] + c[2]) / 3.0,
                    ]
                })
                .collect()
        } else {
            Vec::new()
        };

        for i in 0..n {
            let d2 = dist2(self.positions[i], hit_position);
            if d2 >= r2 {
                continue;
            }
            let t = (d2 / r2).sqrt();
            let w = brush.falloff.weight(t) * scale;

            let delta = match brush.mode {
                BrushMode::Inflate => scale3(self.normals[i], w),
                BrushMode::Deflate => scale3(self.normals[i], -w),
                BrushMode::Push => scale3(avg_normal, -w),
                BrushMode::Pull => scale3(avg_normal, w),
                BrushMode::Smooth => {
                    let target = smooth_targets[i];
                    scale3(sub3(target, self.positions[i]), w * 2.0)
                }
                BrushMode::Flatten => {
                    // Project toward the average plane
                    let to_plane = dot3(sub3(avg_position, self.positions[i]), avg_normal);
                    scale3(avg_normal, to_plane * w)
                }
                BrushMode::Pinch => {
                    let to_center = sub3(hit_position, self.positions[i]);
                    scale3(to_center, w)
                }
            };

            self.positions[i] = add3(self.positions[i], delta);

            // Mirror stroke over X axis if requested
            if brush.symmetric_x {
                let mirror_hit = [-hit_position[0], hit_position[1], hit_position[2]];
                let mirror_pos = [-self.positions[i][0], self.positions[i][1], self.positions[i][2]];
                let _ = (mirror_hit, mirror_pos); // handled in a second pass below
            }
        }

        // Mirror pass (separate to avoid double-applying)
        if brush.symmetric_x {
            let mirror_hit = [-hit_position[0], hit_position[1], hit_position[2]];
            for i in 0..n {
                let d2 = dist2(
                    [-self.positions[i][0], self.positions[i][1], self.positions[i][2]],
                    mirror_hit,
                );
                if d2 >= r2 {
                    continue;
                }
                let t = (d2 / r2).sqrt();
                let w = brush.falloff.weight(t) * scale;
                let delta = match brush.mode {
                    BrushMode::Inflate => scale3(self.normals[i], w),
                    BrushMode::Deflate => scale3(self.normals[i], -w),
                    BrushMode::Push => scale3([-avg_normal[0], avg_normal[1], avg_normal[2]], -w),
                    BrushMode::Pull => scale3([-avg_normal[0], avg_normal[1], avg_normal[2]], w),
                    BrushMode::Smooth => scale3(sub3(avg_position, self.positions[i]), w * 2.0),
                    BrushMode::Flatten => {
                        let to_plane = dot3(sub3(avg_position, self.positions[i]), avg_normal);
                        scale3(avg_normal, to_plane * w)
                    }
                    BrushMode::Pinch => {
                        let to_center = sub3(mirror_hit, [-self.positions[i][0], self.positions[i][1], self.positions[i][2]]);
                        scale3(to_center, w)
                    }
                };
                self.positions[i] = add3(self.positions[i], delta);
            }
        }

        self.dirty = true;
        self.recalculate_normals();
    }

    /// Recompute per-vertex normals by averaging the face normals of every triangle
    /// each vertex belongs to.
    pub fn recalculate_normals(&mut self) {
        let n = self.positions.len();
        let mut normals = vec![[0.0_f32; 3]; n];

        // Iterate over triangles (3 vertices each)
        for tri in 0..n / 3 {
            let base = tri * 3;
            let a = self.positions[base];
            let b = self.positions[base + 1];
            let c = self.positions[base + 2];
            let ab = sub3(b, a);
            let ac = sub3(c, a);
            let face_normal = cross3(ab, ac); // area-weighted
            for k in 0..3 {
                normals[base + k] = add3(normals[base + k], face_normal);
            }
        }

        // Normalize and write back
        for i in 0..n {
            self.normals[i] = normalize3(normals[i]);
        }
        self.dirty = false;
    }

    /// Smooths the entire mesh by one Laplacian pass over all triangles.
    /// Useful for cleaning up sculpting artifacts.
    pub fn smooth_all(&mut self, strength: f32) {
        let n = self.positions.len();
        let mut smoothed = self.positions.clone();
        for tri in 0..n / 3 {
            let base = tri * 3;
            let avg = [
                (self.positions[base][0] + self.positions[base+1][0] + self.positions[base+2][0]) / 3.0,
                (self.positions[base][1] + self.positions[base+1][1] + self.positions[base+2][1]) / 3.0,
                (self.positions[base][2] + self.positions[base+1][2] + self.positions[base+2][2]) / 3.0,
            ];
            for k in 0..3 {
                smoothed[base + k] = lerp3(self.positions[base + k], avg, strength.clamp(0.0, 1.0));
            }
        }
        self.positions = smoothed;
        self.recalculate_normals();
    }

    /// Flatten all vertices toward the mesh centroid's Y=0 plane by `strength` ∈ [0, 1].
    pub fn flatten_base(&mut self, strength: f32) {
        let s = strength.clamp(0.0, 1.0);
        for p in self.positions.iter_mut() {
            p[1] *= 1.0 - s;
        }
        self.recalculate_normals();
    }

    /// Export the current state as a renderable `MeshAsset3D`.
    ///
    /// This allocates a new vertex slice. For best performance call this once per
    /// frame rather than multiple times.
    pub fn to_asset(&self, name: &str) -> Arc<MeshAsset3D> {
        let n = self.positions.len();
        let mut vertices = Vec::with_capacity(n);
        for i in 0..n {
            vertices.push(MeshVertex3D {
                position: self.positions[i],
                normal: self.normals[i],
                uv: self.uvs[i],
                tangent: self.tangents[i],
            });
        }
        Arc::new(MeshAsset3D {
            name: name.to_string(),
            vertices: Arc::<[MeshVertex3D]>::from(vertices),
            base_size: self.base_size,
        })
    }

    /// Returns a reference to the raw positions (mesh-local space, in base_size units).
    pub fn positions(&self) -> &[[f32; 3]] {
        &self.positions
    }

    /// Returns a reference to the computed normals.
    pub fn normals(&self) -> &[[f32; 3]] {
        &self.normals
    }

    /// `true` if the mesh has been modified since the last `recalculate_normals`.
    pub fn is_dirty(&self) -> bool {
        self.dirty
    }

    /// Scale all vertex positions uniformly (useful for changing resolution mid-sculpt).
    pub fn scale(&mut self, factor: f32) {
        for p in self.positions.iter_mut() {
            p[0] *= factor;
            p[1] *= factor;
            p[2] *= factor;
        }
        self.recalculate_normals();
    }

    /// Translate all vertices by a world-space delta.
    pub fn translate(&mut self, delta: [f32; 3]) {
        for p in self.positions.iter_mut() {
            p[0] += delta[0];
            p[1] += delta[1];
            p[2] += delta[2];
        }
    }

    /// Reset all positions back to those in `original`.
    pub fn reset_to(&mut self, original: &MeshAsset3D) {
        for (i, v) in original.vertices.iter().enumerate() {
            if i < self.positions.len() {
                self.positions[i] = v.position;
                self.normals[i] = v.normal;
            }
        }
        self.dirty = false;
    }
}

// ── Math helpers (private) ──────────────────────────────────────────────────

fn add3(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

fn sub3(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn scale3(v: [f32; 3], s: f32) -> [f32; 3] {
    [v[0] * s, v[1] * s, v[2] * s]
}

fn dot3(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn cross3(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

fn normalize3(v: [f32; 3]) -> [f32; 3] {
    let len = dot3(v, v).sqrt().max(0.0001);
    [v[0] / len, v[1] / len, v[2] / len]
}

fn dist2(a: [f32; 3], b: [f32; 3]) -> f32 {
    let d = sub3(a, b);
    dot3(d, d)
}

fn lerp3(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
    [
        a[0] + (b[0] - a[0]) * t,
        a[1] + (b[1] - a[1]) * t,
        a[2] + (b[2] - a[2]) * t,
    ]
}

// ── Unit tests ──────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scene::primitives::generate_icosphere;

    fn make_sculpt_mesh() -> SculptMesh {
        let asset = generate_icosphere("test", 2);
        SculptMesh::from_asset(&asset)
    }

    #[test]
    fn inflate_moves_vertices_outward() {
        let mut mesh = make_sculpt_mesh();
        let hit = [0.0, 1.0, 0.0]; // top of sphere
        let original_positions: Vec<_> = mesh.positions().to_vec();
        let brush = SculptBrush {
            mode: BrushMode::Inflate,
            radius: 0.6,
            strength: 2.0,
            falloff: BrushFalloff::Smooth,
            symmetric_x: false,
        };
        mesh.apply_brush(&brush, hit, 0.1);
        let moved = mesh
            .positions()
            .iter()
            .zip(original_positions.iter())
            .any(|(a, b)| {
                let d = [a[0] - b[0], a[1] - b[1], a[2] - b[2]];
                (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]) > 0.0001
            });
        assert!(moved, "inflate brush should move at least one vertex");
    }

    #[test]
    fn smooth_reduces_local_variance() {
        let mut mesh = make_sculpt_mesh();
        // Inflate first to create a lump
        let brush_inflate = SculptBrush { mode: BrushMode::Inflate, radius: 0.5, strength: 3.0, falloff: BrushFalloff::Constant, symmetric_x: false };
        mesh.apply_brush(&brush_inflate, [0.0, 1.0, 0.0], 0.2);
        let var_before: f32 = mesh.positions().windows(2).map(|w| dist2(w[0], w[1])).sum();

        let brush_smooth = SculptBrush { mode: BrushMode::Smooth, radius: 0.8, strength: 1.0, falloff: BrushFalloff::Smooth, symmetric_x: false };
        mesh.apply_brush(&brush_smooth, [0.0, 1.0, 0.0], 0.1);
        let var_after: f32 = mesh.positions().windows(2).map(|w| dist2(w[0], w[1])).sum();
        // Smoothing should reduce positional variance
        assert!(var_after <= var_before * 1.05, "smooth should not increase variance significantly");
    }

    #[test]
    fn to_asset_round_trips_vertex_count() {
        let mesh = make_sculpt_mesh();
        let vc = mesh.vertex_count();
        let asset = mesh.to_asset("round_trip");
        assert_eq!(asset.vertices.len(), vc);
    }

    #[test]
    fn normals_are_unit_length_after_deformation() {
        let mut mesh = make_sculpt_mesh();
        let brush = SculptBrush { mode: BrushMode::Inflate, radius: 0.5, strength: 1.0, falloff: BrushFalloff::Smooth, symmetric_x: false };
        mesh.apply_brush(&brush, [0.0, 1.0, 0.0], 0.1);
        for n in mesh.normals() {
            let len = (n[0]*n[0] + n[1]*n[1] + n[2]*n[2]).sqrt();
            assert!((len - 1.0).abs() < 0.01, "normal not unit after sculpt: len={len}");
        }
    }

    #[test]
    fn falloff_weights_are_in_range() {
        for mode in [BrushFalloff::Linear, BrushFalloff::Smooth, BrushFalloff::Constant, BrushFalloff::Sphere] {
            assert!((mode.weight(0.0) - 1.0).abs() < 0.001, "{mode:?} weight at 0 should be ~1");
            assert!(mode.weight(1.0) >= 0.0 && mode.weight(1.0) <= 1.0, "{mode:?} weight at 1 out of range");
        }
    }
}
