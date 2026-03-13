#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BodyHandle(pub usize);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BodyType {
    Static,
    Dynamic,
    Kinematic,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ColliderShape {
    Cuboid { half_extents: [f32; 3] },
    Sphere { radius: f32 },
    Plane { normal: [f32; 3], offset: f32 },
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PhysicsMaterial {
    pub restitution: f32,
    pub friction: f32,
}

impl Default for PhysicsMaterial {
    fn default() -> Self {
        Self {
            restitution: 0.05,
            friction: 0.65,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RigidBody {
    pub body_type: BodyType,
    pub position: [f32; 3],
    pub rotation_radians: [f32; 3],
    pub linear_velocity: [f32; 3],
    pub angular_velocity: [f32; 3],
    pub mass: f32,
    pub collider: ColliderShape,
    pub material: PhysicsMaterial,
}

impl RigidBody {
    pub fn static_body(position: [f32; 3], collider: ColliderShape) -> Self {
        Self {
            body_type: BodyType::Static,
            position,
            rotation_radians: [0.0, 0.0, 0.0],
            linear_velocity: [0.0, 0.0, 0.0],
            angular_velocity: [0.0, 0.0, 0.0],
            mass: 0.0,
            collider,
            material: PhysicsMaterial::default(),
        }
    }

    pub fn dynamic_body(position: [f32; 3], mass: f32, collider: ColliderShape) -> Self {
        Self {
            body_type: BodyType::Dynamic,
            position,
            rotation_radians: [0.0, 0.0, 0.0],
            linear_velocity: [0.0, 0.0, 0.0],
            angular_velocity: [0.0, 0.0, 0.0],
            mass: mass.max(0.0001),
            collider,
            material: PhysicsMaterial::default(),
        }
    }

    pub fn kinematic_body(position: [f32; 3], collider: ColliderShape) -> Self {
        Self {
            body_type: BodyType::Kinematic,
            position,
            rotation_radians: [0.0, 0.0, 0.0],
            linear_velocity: [0.0, 0.0, 0.0],
            angular_velocity: [0.0, 0.0, 0.0],
            mass: 0.0,
            collider,
            material: PhysicsMaterial::default(),
        }
    }

    pub fn inverse_mass(self) -> f32 {
        match self.body_type {
            BodyType::Dynamic => 1.0 / self.mass.max(0.0001),
            BodyType::Static | BodyType::Kinematic => 0.0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PhysicsConfig {
    pub gravity: [f32; 3],
    pub solver_iterations: u32,
    pub position_slop: f32,
    pub position_correction_factor: f32,
}

impl Default for PhysicsConfig {
    fn default() -> Self {
        Self {
            gravity: [0.0, -9.81, 0.0],
            solver_iterations: 6,
            position_slop: 0.001,
            position_correction_factor: 0.8,
        }
    }
}

#[derive(Debug, Default)]
pub struct PhysicsWorld {
    config: PhysicsConfig,
    bodies: Vec<Option<RigidBody>>,
}

impl PhysicsWorld {
    pub fn new(config: PhysicsConfig) -> Self {
        Self {
            config,
            bodies: Vec::new(),
        }
    }

    pub fn config(&self) -> PhysicsConfig {
        self.config
    }

    pub fn set_config(&mut self, config: PhysicsConfig) {
        self.config = config;
    }

    pub fn insert_body(&mut self, body: RigidBody) -> BodyHandle {
        let handle = BodyHandle(self.bodies.len());
        self.bodies.push(Some(body));
        handle
    }

    pub fn remove_body(&mut self, handle: BodyHandle) -> Option<RigidBody> {
        self.bodies.get_mut(handle.0)?.take()
    }

    pub fn body(&self, handle: BodyHandle) -> Option<&RigidBody> {
        self.bodies.get(handle.0)?.as_ref()
    }

    pub fn body_mut(&mut self, handle: BodyHandle) -> Option<&mut RigidBody> {
        self.bodies.get_mut(handle.0)?.as_mut()
    }

    pub fn bodies(&self) -> impl Iterator<Item = (BodyHandle, &RigidBody)> {
        self.bodies
            .iter()
            .enumerate()
            .filter_map(|(index, body)| body.as_ref().map(|body| (BodyHandle(index), body)))
    }

    pub fn step(&mut self, delta_time_seconds: f32) {
        if delta_time_seconds <= 0.0 {
            return;
        }

        for body in self.bodies.iter_mut().filter_map(Option::as_mut) {
            if body.body_type != BodyType::Dynamic {
                continue;
            }
            body.linear_velocity = add3(
                body.linear_velocity,
                scale3(self.config.gravity, delta_time_seconds),
            );
            body.position = add3(
                body.position,
                scale3(body.linear_velocity, delta_time_seconds),
            );
            body.rotation_radians = add3(
                body.rotation_radians,
                scale3(body.angular_velocity, delta_time_seconds),
            );
        }

        for _ in 0..self.config.solver_iterations {
            let len = self.bodies.len();
            for a_index in 0..len {
                for b_index in (a_index + 1)..len {
                    self.solve_pair(BodyHandle(a_index), BodyHandle(b_index));
                }
            }
        }
    }

    fn solve_pair(&mut self, a_handle: BodyHandle, b_handle: BodyHandle) {
        let (Some(a), Some(b)) = (
            self.bodies.get(a_handle.0).and_then(|body| *body),
            self.bodies.get(b_handle.0).and_then(|body| *body),
        ) else {
            return;
        };
        let Some(contact) = detect_collision(a, b) else {
            return;
        };
        if a.inverse_mass() + b.inverse_mass() <= 0.0 {
            return;
        }

        let mut a_next = a;
        let mut b_next = b;
        resolve_contact(&self.config, &mut a_next, &mut b_next, contact);
        if let Some(slot) = self.bodies.get_mut(a_handle.0) {
            *slot = Some(a_next);
        }
        if let Some(slot) = self.bodies.get_mut(b_handle.0) {
            *slot = Some(b_next);
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CollisionContact {
    pub normal: [f32; 3],
    pub penetration: f32,
}

pub fn detect_collision(a: RigidBody, b: RigidBody) -> Option<CollisionContact> {
    match (a.collider, b.collider) {
        (
            ColliderShape::Sphere { radius: a_radius },
            ColliderShape::Sphere { radius: b_radius },
        ) => collide_sphere_sphere(a.position, a_radius, b.position, b_radius),
        (
            ColliderShape::Cuboid {
                half_extents: a_half,
            },
            ColliderShape::Cuboid {
                half_extents: b_half,
            },
        ) => collide_cuboid_cuboid(a.position, a_half, b.position, b_half),
        (ColliderShape::Sphere { radius }, ColliderShape::Cuboid { half_extents }) => {
            collide_sphere_cuboid(a.position, radius, b.position, half_extents).map(|contact| {
                CollisionContact {
                    normal: scale3(contact.normal, -1.0),
                    penetration: contact.penetration,
                }
            })
        }
        (ColliderShape::Cuboid { half_extents }, ColliderShape::Sphere { radius }) => {
            collide_sphere_cuboid(b.position, radius, a.position, half_extents)
        }
        (ColliderShape::Plane { normal, offset }, ColliderShape::Sphere { radius }) => {
            collide_plane_sphere(normalize3(normal), offset, b.position, radius)
        }
        (ColliderShape::Sphere { radius }, ColliderShape::Plane { normal, offset }) => {
            collide_plane_sphere(normalize3(normal), offset, a.position, radius).map(|contact| {
                CollisionContact {
                    normal: scale3(contact.normal, -1.0),
                    penetration: contact.penetration,
                }
            })
        }
        (ColliderShape::Plane { normal, offset }, ColliderShape::Cuboid { half_extents }) => {
            collide_plane_cuboid(normalize3(normal), offset, b.position, half_extents)
        }
        (ColliderShape::Cuboid { half_extents }, ColliderShape::Plane { normal, offset }) => {
            collide_plane_cuboid(normalize3(normal), offset, a.position, half_extents).map(
                |contact| CollisionContact {
                    normal: scale3(contact.normal, -1.0),
                    penetration: contact.penetration,
                },
            )
        }
        (ColliderShape::Plane { .. }, ColliderShape::Plane { .. }) => None,
    }
}

fn collide_sphere_sphere(
    a_center: [f32; 3],
    a_radius: f32,
    b_center: [f32; 3],
    b_radius: f32,
) -> Option<CollisionContact> {
    let delta = sub3(b_center, a_center);
    let distance = length3(delta);
    let radius_sum = a_radius + b_radius;
    if distance >= radius_sum {
        return None;
    }
    let normal = if distance <= 0.0001 {
        [0.0, 1.0, 0.0]
    } else {
        scale3(delta, 1.0 / distance)
    };
    Some(CollisionContact {
        normal,
        penetration: radius_sum - distance,
    })
}

fn collide_cuboid_cuboid(
    a_center: [f32; 3],
    a_half: [f32; 3],
    b_center: [f32; 3],
    b_half: [f32; 3],
) -> Option<CollisionContact> {
    let delta = sub3(b_center, a_center);
    let overlap = [
        a_half[0] + b_half[0] - delta[0].abs(),
        a_half[1] + b_half[1] - delta[1].abs(),
        a_half[2] + b_half[2] - delta[2].abs(),
    ];
    if overlap[0] <= 0.0 || overlap[1] <= 0.0 || overlap[2] <= 0.0 {
        return None;
    }
    let mut axis = 0usize;
    if overlap[1] < overlap[axis] {
        axis = 1;
    }
    if overlap[2] < overlap[axis] {
        axis = 2;
    }
    let mut normal = [0.0, 0.0, 0.0];
    normal[axis] = if delta[axis] >= 0.0 { 1.0 } else { -1.0 };
    Some(CollisionContact {
        normal,
        penetration: overlap[axis],
    })
}

fn collide_sphere_cuboid(
    sphere_center: [f32; 3],
    radius: f32,
    box_center: [f32; 3],
    half_extents: [f32; 3],
) -> Option<CollisionContact> {
    let min = sub3(box_center, half_extents);
    let max = add3(box_center, half_extents);
    let closest = [
        sphere_center[0].clamp(min[0], max[0]),
        sphere_center[1].clamp(min[1], max[1]),
        sphere_center[2].clamp(min[2], max[2]),
    ];
    let delta = sub3(sphere_center, closest);
    let distance = length3(delta);
    if distance >= radius {
        return None;
    }
    let normal = if distance <= 0.0001 {
        [0.0, 1.0, 0.0]
    } else {
        scale3(delta, 1.0 / distance)
    };
    Some(CollisionContact {
        normal,
        penetration: radius - distance,
    })
}

fn collide_plane_sphere(
    plane_normal: [f32; 3],
    plane_offset: f32,
    sphere_center: [f32; 3],
    radius: f32,
) -> Option<CollisionContact> {
    let distance = dot3(plane_normal, sphere_center) - plane_offset;
    if distance >= radius {
        return None;
    }
    Some(CollisionContact {
        normal: plane_normal,
        penetration: radius - distance,
    })
}

fn collide_plane_cuboid(
    plane_normal: [f32; 3],
    plane_offset: f32,
    cuboid_center: [f32; 3],
    half_extents: [f32; 3],
) -> Option<CollisionContact> {
    let projected_radius = half_extents[0] * plane_normal[0].abs()
        + half_extents[1] * plane_normal[1].abs()
        + half_extents[2] * plane_normal[2].abs();
    let distance = dot3(plane_normal, cuboid_center) - plane_offset;
    if distance >= projected_radius {
        return None;
    }
    Some(CollisionContact {
        normal: plane_normal,
        penetration: projected_radius - distance,
    })
}

fn resolve_contact(
    config: &PhysicsConfig,
    a: &mut RigidBody,
    b: &mut RigidBody,
    contact: CollisionContact,
) {
    let inv_mass_a = a.inverse_mass();
    let inv_mass_b = b.inverse_mass();
    let inv_mass_sum = inv_mass_a + inv_mass_b;
    if inv_mass_sum <= 0.0 {
        return;
    }

    let relative_velocity = sub3(b.linear_velocity, a.linear_velocity);
    let separating_velocity = dot3(relative_velocity, contact.normal);
    if separating_velocity < 0.0 {
        let restitution = a.material.restitution.min(b.material.restitution);
        let impulse_scalar = -(1.0 + restitution) * separating_velocity / inv_mass_sum;
        let impulse = scale3(contact.normal, impulse_scalar);
        if a.body_type == BodyType::Dynamic {
            a.linear_velocity = sub3(a.linear_velocity, scale3(impulse, inv_mass_a));
        }
        if b.body_type == BodyType::Dynamic {
            b.linear_velocity = add3(b.linear_velocity, scale3(impulse, inv_mass_b));
        }
    }

    let tangent_velocity = sub3(
        relative_velocity,
        scale3(contact.normal, dot3(relative_velocity, contact.normal)),
    );
    let tangent_speed = length3(tangent_velocity);
    if tangent_speed > 0.0001 {
        let tangent = scale3(tangent_velocity, 1.0 / tangent_speed);
        let friction = (a.material.friction * b.material.friction).sqrt();
        let tangent_impulse_scalar = -(dot3(relative_velocity, tangent)) / inv_mass_sum;
        let tangent_impulse = scale3(tangent, tangent_impulse_scalar.clamp(-friction, friction));
        if a.body_type == BodyType::Dynamic {
            a.linear_velocity = sub3(a.linear_velocity, scale3(tangent_impulse, inv_mass_a));
        }
        if b.body_type == BodyType::Dynamic {
            b.linear_velocity = add3(b.linear_velocity, scale3(tangent_impulse, inv_mass_b));
        }
    }

    let correction_magnitude = ((contact.penetration - config.position_slop).max(0.0)
        * config.position_correction_factor)
        / inv_mass_sum;
    let correction = scale3(contact.normal, correction_magnitude);
    if a.body_type == BodyType::Dynamic {
        a.position = sub3(a.position, scale3(correction, inv_mass_a));
    }
    if b.body_type == BodyType::Dynamic {
        b.position = add3(b.position, scale3(correction, inv_mass_b));
    }
}

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

fn length3(v: [f32; 3]) -> f32 {
    dot3(v, v).sqrt()
}

fn normalize3(v: [f32; 3]) -> [f32; 3] {
    let len = length3(v);
    if len <= 0.0001 {
        [0.0, 1.0, 0.0]
    } else {
        scale3(v, 1.0 / len)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dynamic_cuboid_falls_onto_plane() {
        let mut world = PhysicsWorld::new(PhysicsConfig::default());
        let floor = world.insert_body(RigidBody::static_body(
            [0.0, 0.0, 0.0],
            ColliderShape::Plane {
                normal: [0.0, 1.0, 0.0],
                offset: 0.0,
            },
        ));
        let cube = world.insert_body(RigidBody::dynamic_body(
            [0.0, 4.0, 0.0],
            1.0,
            ColliderShape::Cuboid {
                half_extents: [0.5, 0.5, 0.5],
            },
        ));
        assert!(world.body(floor).is_some());
        for _ in 0..240 {
            world.step(1.0 / 60.0);
        }
        let cube = world.body(cube).expect("cube should exist");
        assert!(cube.position[1] >= 0.49);
        assert!(cube.position[1] <= 0.7);
        assert!(cube.linear_velocity[1].abs() < 0.25);
    }

    #[test]
    fn spheres_separate_after_overlap() {
        let mut world = PhysicsWorld::new(PhysicsConfig::default());
        let a = world.insert_body(RigidBody::dynamic_body(
            [0.0, 0.0, 0.0],
            1.0,
            ColliderShape::Sphere { radius: 0.5 },
        ));
        let b = world.insert_body(RigidBody::dynamic_body(
            [0.75, 0.0, 0.0],
            1.0,
            ColliderShape::Sphere { radius: 0.5 },
        ));
        world.step(1.0 / 60.0);
        let a_pos = world.body(a).expect("body a").position;
        let b_pos = world.body(b).expect("body b").position;
        assert!(b_pos[0] - a_pos[0] >= 0.99);
    }

    #[test]
    fn kinematic_bodies_are_not_gravity_integrated() {
        let mut world = PhysicsWorld::new(PhysicsConfig::default());
        let body = world.insert_body(RigidBody::kinematic_body(
            [0.0, 2.0, 0.0],
            ColliderShape::Sphere { radius: 0.5 },
        ));
        world.step(1.0);
        assert_eq!(
            world.body(body).expect("body should exist").position,
            [0.0, 2.0, 0.0]
        );
    }

    #[test]
    fn detection_reports_cuboid_overlap() {
        let a = RigidBody::static_body(
            [0.0, 0.0, 0.0],
            ColliderShape::Cuboid {
                half_extents: [1.0, 1.0, 1.0],
            },
        );
        let b = RigidBody::static_body(
            [1.5, 0.0, 0.0],
            ColliderShape::Cuboid {
                half_extents: [1.0, 1.0, 1.0],
            },
        );
        let contact = detect_collision(a, b).expect("bodies should overlap");
        assert_eq!(contact.normal, [1.0, 0.0, 0.0]);
        assert!((contact.penetration - 0.5).abs() < 0.0001);
    }

    #[test]
    fn detection_reports_no_contact_for_separated_spheres() {
        let a = RigidBody::static_body([0.0, 0.0, 0.0], ColliderShape::Sphere { radius: 0.5 });
        let b = RigidBody::static_body([2.0, 0.0, 0.0], ColliderShape::Sphere { radius: 0.5 });
        assert!(detect_collision(a, b).is_none());
    }
}
