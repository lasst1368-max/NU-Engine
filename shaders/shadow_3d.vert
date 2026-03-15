#version 450

layout(location = 0) in vec3 in_position;
layout(location = 4) in uint in_object_index;

layout(push_constant) uniform ShadowCamera {
    vec4 rows[4];
} pc;

struct SceneCube {
    vec4 center;
    vec4 half_extents;
    vec4 axis_x;
    vec4 axis_y;
    vec4 axis_z;
};

layout(set = 0, binding = 1) readonly buffer CubeObjects {
    SceneCube cubes[];
} cube_objects;

void main() {
    SceneCube cube = cube_objects.cubes[int(in_object_index)];
    vec3 local_position = in_position * cube.half_extents.xyz;
    vec3 world_position =
        cube.center.xyz +
        (cube.axis_x.xyz * local_position.x) +
        (cube.axis_y.xyz * local_position.y) +
        (cube.axis_z.xyz * local_position.z);
    vec4 world = vec4(world_position, 1.0);
    gl_Position = vec4(
        dot(pc.rows[0], world),
        dot(pc.rows[1], world),
        dot(pc.rows[2], world),
        dot(pc.rows[3], world)
    );
}
