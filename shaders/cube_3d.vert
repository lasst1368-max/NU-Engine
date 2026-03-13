#version 450

layout(location = 0) in vec3 in_position;
layout(location = 1) in vec3 in_normal;
layout(location = 2) in vec2 in_uv;
layout(location = 3) in vec4 in_albedo;
layout(location = 4) in uint in_object_index;

layout(push_constant) uniform CubeCamera {
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

layout(location = 0) out vec3 world_position;
layout(location = 1) out vec3 world_normal;
layout(location = 2) out vec2 draw_uv;
layout(location = 3) out vec4 draw_albedo;
layout(location = 4) flat out uint draw_object_index;

void main() {
    SceneCube cube = cube_objects.cubes[int(in_object_index)];
    vec3 local_position = in_position * cube.half_extents.xyz;
    vec3 world_position_value =
        cube.center.xyz +
        (cube.axis_x.xyz * local_position.x) +
        (cube.axis_y.xyz * local_position.y) +
        (cube.axis_z.xyz * local_position.z);
    vec3 world_normal_value = normalize(
        (cube.axis_x.xyz * in_normal.x) +
        (cube.axis_y.xyz * in_normal.y) +
        (cube.axis_z.xyz * in_normal.z)
    );
    vec4 world = vec4(world_position_value, 1.0);
    gl_Position = vec4(
        dot(pc.rows[0], world),
        dot(pc.rows[1], world),
        dot(pc.rows[2], world),
        dot(pc.rows[3], world)
    );
    world_position = world_position_value;
    world_normal = world_normal_value;
    draw_uv = in_uv;
    draw_albedo = in_albedo;
    draw_object_index = in_object_index;
}
