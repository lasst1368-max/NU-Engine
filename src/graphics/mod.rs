use crate::scene::{QuadDraw, SceneFrame};

#[derive(Debug, Clone, Copy)]
pub struct Camera3D {
    pub position: [f32; 3],
    pub focal_length: f32,
    pub near_clip: f32,
}

impl Default for Camera3D {
    fn default() -> Self {
        Self {
            position: [0.0, 0.0, -3.5],
            focal_length: 1.4,
            near_clip: 0.1,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Block3D {
    pub center: [f32; 3],
    pub size: [f32; 3],
    pub rotation_radians: [f32; 3],
    pub color: [f32; 4],
    pub layer: i32,
    pub slices: u32,
    pub connector_thickness: f32,
}

impl Default for Block3D {
    fn default() -> Self {
        Self {
            center: [0.0, 0.0, 0.0],
            size: [0.9, 0.9, 0.9],
            rotation_radians: [0.0, 0.0, 0.0],
            color: [0.95, 0.05, 0.05, 1.0],
            layer: 10,
            slices: 0,
            connector_thickness: 0.0,
        }
    }
}

#[derive(Clone, Copy)]
struct Face2D {
    points: [[f32; 2]; 4],
    depth: f32,
    brightness: f32,
}

#[derive(Clone, Copy)]
struct Face3D {
    center: [f32; 3],
    right: [f32; 3],
    up: [f32; 3],
    normal: [f32; 3],
}

pub struct Canvas3D<'a> {
    frame: &'a mut SceneFrame,
    camera: Camera3D,
}

impl<'a> Canvas3D<'a> {
    pub fn new(frame: &'a mut SceneFrame, camera: Camera3D) -> Self {
        Self { frame, camera }
    }

    pub fn draw_block(&mut self, block: Block3D) {
        let half = [
            block.size[0] * 0.5,
            block.size[1] * 0.5,
            block.size[2] * 0.5,
        ];
        let faces = [
            face(
                block.center,
                rotate_vector([half[0], 0.0, 0.0], block.rotation_radians),
                rotate_vector([0.0, half[1], 0.0], block.rotation_radians),
                rotate_vector([0.0, 0.0, -half[2]], block.rotation_radians),
            ),
            face(
                block.center,
                rotate_vector([-half[0], 0.0, 0.0], block.rotation_radians),
                rotate_vector([0.0, half[1], 0.0], block.rotation_radians),
                rotate_vector([0.0, 0.0, half[2]], block.rotation_radians),
            ),
            face(
                block.center,
                rotate_vector([0.0, 0.0, half[2]], block.rotation_radians),
                rotate_vector([0.0, half[1], 0.0], block.rotation_radians),
                rotate_vector([half[0], 0.0, 0.0], block.rotation_radians),
            ),
            face(
                block.center,
                rotate_vector([0.0, 0.0, -half[2]], block.rotation_radians),
                rotate_vector([0.0, half[1], 0.0], block.rotation_radians),
                rotate_vector([-half[0], 0.0, 0.0], block.rotation_radians),
            ),
            face(
                block.center,
                rotate_vector([half[0], 0.0, 0.0], block.rotation_radians),
                rotate_vector([0.0, 0.0, -half[2]], block.rotation_radians),
                rotate_vector([0.0, half[1], 0.0], block.rotation_radians),
            ),
            face(
                block.center,
                rotate_vector([half[0], 0.0, 0.0], block.rotation_radians),
                rotate_vector([0.0, 0.0, half[2]], block.rotation_radians),
                rotate_vector([0.0, -half[1], 0.0], block.rotation_radians),
            ),
        ];

        let mut visible_faces = Vec::new();
        for face in faces {
            if let Some(projected) = project_face(self.camera, face) {
                visible_faces.push(projected);
            }
        }

        visible_faces.sort_by(|left, right| {
            left.depth
                .partial_cmp(&right.depth)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        for (index, projected) in visible_faces.into_iter().enumerate() {
            let brightness = projected.brightness.clamp(0.35, 1.0);
            self.frame.draw_quad(QuadDraw {
                points: projected.points,
                color: [
                    block.color[0] * brightness,
                    block.color[1] * brightness,
                    block.color[2] * brightness,
                    block.color[3],
                ],
                layer: block.layer + index as i32,
                ..QuadDraw::default()
            });
        }
    }
}

impl Camera3D {
    pub fn project(self, point: [f32; 3]) -> Option<[f32; 2]> {
        let camera_space = sub3(point, self.position);
        if camera_space[2] <= self.near_clip {
            return None;
        }

        let perspective = self.focal_length / camera_space[2];
        Some([camera_space[0] * perspective, camera_space[1] * perspective])
    }
}

fn face(center: [f32; 3], right: [f32; 3], up: [f32; 3], normal: [f32; 3]) -> Face3D {
    Face3D {
        center: add3(center, normal),
        right,
        up,
        normal: normalize3(normal),
    }
}

fn project_face(camera: Camera3D, face: Face3D) -> Option<Face2D> {
    let to_camera = sub3(camera.position, face.center);
    if dot3(face.normal, to_camera) <= 0.0 {
        return None;
    }

    let brightness = 0.35 + dot3(face.normal, normalize3(scale3(to_camera, -1.0))).abs() * 0.65;
    let corners = [
        sub3(face.center, add3(face.right, face.up)),
        add3(sub3(face.center, face.up), face.right),
        add3(face.center, add3(face.right, face.up)),
        add3(sub3(face.center, face.right), face.up),
    ];

    Some(Face2D {
        points: [
            camera.project(corners[0])?,
            camera.project(corners[1])?,
            camera.project(corners[2])?,
            camera.project(corners[3])?,
        ],
        depth: face.center[2],
        brightness,
    })
}

fn rotate_vector(vector: [f32; 3], rotation_radians: [f32; 3]) -> [f32; 3] {
    let [mut x, mut y, mut z] = vector;

    let (sin_x, cos_x) = rotation_radians[0].sin_cos();
    let rotated_y = (y * cos_x) - (z * sin_x);
    let rotated_z = (y * sin_x) + (z * cos_x);
    y = rotated_y;
    z = rotated_z;

    let (sin_y, cos_y) = rotation_radians[1].sin_cos();
    let rotated_x = (x * cos_y) + (z * sin_y);
    let rotated_z = (-x * sin_y) + (z * cos_y);
    x = rotated_x;
    z = rotated_z;

    let (sin_z, cos_z) = rotation_radians[2].sin_cos();
    let rotated_x = (x * cos_z) - (y * sin_z);
    let rotated_y = (x * sin_z) + (y * cos_z);

    [rotated_x, rotated_y, z]
}

fn add3(left: [f32; 3], right: [f32; 3]) -> [f32; 3] {
    [left[0] + right[0], left[1] + right[1], left[2] + right[2]]
}

fn sub3(left: [f32; 3], right: [f32; 3]) -> [f32; 3] {
    [left[0] - right[0], left[1] - right[1], left[2] - right[2]]
}

fn scale3(vector: [f32; 3], scale: f32) -> [f32; 3] {
    [vector[0] * scale, vector[1] * scale, vector[2] * scale]
}

fn dot3(left: [f32; 3], right: [f32; 3]) -> f32 {
    (left[0] * right[0]) + (left[1] * right[1]) + (left[2] * right[2])
}

fn length3(vector: [f32; 3]) -> f32 {
    dot3(vector, vector).sqrt()
}

fn normalize3(vector: [f32; 3]) -> [f32; 3] {
    let length = length3(vector).max(0.0001);
    [vector[0] / length, vector[1] / length, vector[2] / length]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scene::{PrimitiveDraw, SceneFrame};

    #[test]
    fn block_canvas_emits_face_rects() {
        let mut frame = SceneFrame::default();
        let mut canvas = Canvas3D::new(&mut frame, Camera3D::default());
        canvas.draw_block(Block3D {
            rotation_radians: [0.4, -0.5, 0.0],
            ..Block3D::default()
        });

        let quads = frame
            .draws()
            .iter()
            .filter(|draw| matches!(draw, PrimitiveDraw::Quad(_)))
            .count();

        assert!(quads >= 2);
        assert!(quads <= 3);
    }

    #[test]
    fn camera_projects_points_in_front_of_it() {
        let camera = Camera3D::default();
        let projected = camera.project([0.0, 0.0, 0.0]);

        assert_eq!(projected, Some([0.0, 0.0]));
    }
}
