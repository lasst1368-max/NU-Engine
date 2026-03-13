#version 450

struct SceneCube {
    vec4 center;
    vec4 half_extents;
    vec4 axis_x;
    vec4 axis_y;
    vec4 axis_z;
};

layout(set = 0, binding = 0) uniform CubeScene {
    vec4 camera_position;
    vec4 light_position_range;
    vec4 light_color_intensity;
    vec4 ambient_color_intensity;
    vec4 fill_direction_intensity;
    vec4 fill_color;
    vec4 material;
    vec4 shadow_params;
} scene;

layout(set = 0, binding = 1) readonly buffer CubeObjects {
    SceneCube cubes[];
} cube_objects;

layout(set = 1, binding = 0) uniform sampler2D albedo_texture;

layout(location = 0) in vec3 world_position;
layout(location = 1) in vec3 world_normal;
layout(location = 2) in vec2 draw_uv;
layout(location = 3) in vec4 draw_albedo;
layout(location = 4) flat in uint draw_object_index;
layout(location = 0) out vec4 out_color;

const vec2 POINT_SAMPLE_OFFSETS[5] = vec2[](
    vec2(0.0, 0.0),
    vec2(0.8, 0.0),
    vec2(-0.8, 0.0),
    vec2(0.0, 0.8),
    vec2(0.0, -0.8)
);

const vec2 DIRECTIONAL_SAMPLE_OFFSETS[3] = vec2[](
    vec2(0.0, 0.0),
    vec2(0.75, 0.0),
    vec2(-0.75, 0.0)
);

bool ray_intersects_cube(
    vec3 ray_origin,
    vec3 ray_direction,
    float max_distance,
    SceneCube cube
) {
    vec3 origin_delta = ray_origin - cube.center.xyz;
    vec3 local_origin = vec3(
        dot(origin_delta, cube.axis_x.xyz),
        dot(origin_delta, cube.axis_y.xyz),
        dot(origin_delta, cube.axis_z.xyz)
    );
    vec3 local_direction = vec3(
        dot(ray_direction, cube.axis_x.xyz),
        dot(ray_direction, cube.axis_y.xyz),
        dot(ray_direction, cube.axis_z.xyz)
    );

    float t_min = 0.0;
    float t_max = max_distance;

    for (int axis = 0; axis < 3; axis++) {
        float origin = local_origin[axis];
        float direction = local_direction[axis];
        float min_bound = -cube.half_extents[axis];
        float max_bound = cube.half_extents[axis];

        if (abs(direction) < 0.0001) {
            if (origin < min_bound || origin > max_bound) {
                return false;
            }
            continue;
        }

        float inv_direction = 1.0 / direction;
        float near_hit = (min_bound - origin) * inv_direction;
        float far_hit = (max_bound - origin) * inv_direction;
        if (near_hit > far_hit) {
            float swap_value = near_hit;
            near_hit = far_hit;
            far_hit = swap_value;
        }

        t_min = max(t_min, near_hit);
        t_max = min(t_max, far_hit);
        if (t_min > t_max) {
            return false;
        }
    }

    return t_max > 0.0 && t_min < max_distance;
}

vec3 shadow_basis_tangent(vec3 direction) {
    vec3 up = abs(direction.y) < 0.95
        ? vec3(0.0, 1.0, 0.0)
        : vec3(1.0, 0.0, 0.0);
    return normalize(cross(up, direction));
}

float mix_shadow_visibility(float visibility) {
    float min_visibility = scene.shadow_params.x;
    return min_visibility + ((1.0 - min_visibility) * clamp(visibility, 0.0, 1.0));
}

float point_shadow_visibility(vec3 normal) {
    vec3 ray_origin = world_position + normal * scene.shadow_params.y;
    vec3 light_vector = scene.light_position_range.xyz - world_position;
    float light_distance = max(length(light_vector), 0.0001);
    vec3 light_direction = light_vector / light_distance;
    vec3 tangent = shadow_basis_tangent(light_direction);
    vec3 bitangent = normalize(cross(light_direction, tangent));
    float visible_samples = 0.0;
    int cube_count = int(scene.material.z + 0.5);

    for (int sample_index = 0; sample_index < 5; sample_index++) {
        vec2 offset = POINT_SAMPLE_OFFSETS[sample_index] * scene.shadow_params.z;
        vec3 light_sample =
            scene.light_position_range.xyz +
            tangent * offset.x +
            bitangent * offset.y;
        vec3 sample_vector = light_sample - world_position;
        float sample_distance = max(length(sample_vector), 0.0001);
        vec3 sample_direction = sample_vector / sample_distance;
        bool occluded = false;

        for (int cube_index = 0; cube_index < cube_count; cube_index++) {
            if (cube_index == int(draw_object_index)) {
                continue;
            }
            if (ray_intersects_cube(
                ray_origin,
                sample_direction,
                max(sample_distance - scene.shadow_params.y, 0.0),
                cube_objects.cubes[cube_index]
            )) {
                occluded = true;
                break;
            }
        }

        if (!occluded) {
            visible_samples += 1.0;
        }
    }

    return mix_shadow_visibility(visible_samples / 5.0);
}

float directional_shadow_visibility(vec3 normal, vec3 base_direction) {
    vec3 ray_origin = world_position + normal * scene.shadow_params.y;
    vec3 tangent = shadow_basis_tangent(base_direction);
    vec3 bitangent = normalize(cross(base_direction, tangent));
    float visible_samples = 0.0;
    int cube_count = int(scene.material.z + 0.5);

    for (int sample_index = 0; sample_index < 3; sample_index++) {
        vec2 offset = DIRECTIONAL_SAMPLE_OFFSETS[sample_index] * scene.shadow_params.w;
        vec3 sample_direction = normalize(
            base_direction +
            tangent * offset.x +
            bitangent * offset.y
        );
        bool occluded = false;

        for (int cube_index = 0; cube_index < cube_count; cube_index++) {
            if (cube_index == int(draw_object_index)) {
                continue;
            }
            if (ray_intersects_cube(
                ray_origin,
                sample_direction,
                100.0,
                cube_objects.cubes[cube_index]
            )) {
                occluded = true;
                break;
            }
        }

        if (!occluded) {
            visible_samples += 1.0;
        }
    }

    return mix_shadow_visibility(visible_samples / 3.0);
}

void main() {
    vec4 albedo_sample = texture(albedo_texture, draw_uv);
    vec4 surface_albedo = vec4(draw_albedo.rgb * albedo_sample.rgb, draw_albedo.a * albedo_sample.a);
    vec3 normal = normalize(world_normal);
    vec3 light_vector = scene.light_position_range.xyz - world_position;
    float distance_to_light = max(length(light_vector), 0.0001);
    vec3 light_direction = light_vector / distance_to_light;
    vec3 view_direction =
        normalize(scene.camera_position.xyz - world_position);
    vec3 half_vector = normalize(light_direction + view_direction);
    vec3 fill_direction = normalize(scene.fill_direction_intensity.xyz);
    float point_shadow = point_shadow_visibility(normal);
    float fill_shadow = 1.0;

    float diffuse = max(dot(normal, light_direction), 0.0);
    float fill_diffuse = max(dot(normal, fill_direction), 0.0);
    float hemisphere = normal.y * 0.5 + 0.5;
    float specular = 0.0;
    if (diffuse > 0.0) {
        specular = pow(max(dot(normal, half_vector), 0.0), scene.material.y)
            * scene.material.x;
    }

    float range = max(scene.light_position_range.w, 0.0001);
    float attenuation =
        1.0 / (1.0 + (distance_to_light * distance_to_light) / (range * range));
    vec3 light_radiance = scene.light_color_intensity.rgb
        * (scene.light_color_intensity.w * attenuation);
    vec3 fill_radiance = scene.fill_color.rgb
        * scene.fill_direction_intensity.w;
    vec3 sky_ambient = scene.ambient_color_intensity.rgb;
    vec3 ground_ambient = scene.ambient_color_intensity.rgb * vec3(0.28, 0.24, 0.22);
    vec3 ambient_color = mix(ground_ambient, sky_ambient, hemisphere);
    vec3 ambient = surface_albedo.rgb
        * ambient_color
        * (scene.ambient_color_intensity.w * 0.95);
    vec3 shadow_lift = surface_albedo.rgb * 0.035;
    vec3 diffuse_term = surface_albedo.rgb * light_radiance * diffuse * point_shadow;
    vec3 fill_term = surface_albedo.rgb * fill_radiance * fill_diffuse * fill_shadow * 0.55;
    vec3 specular_term = light_radiance * specular * point_shadow;
    vec3 shaded = ambient + shadow_lift + fill_term + diffuse_term + specular_term;

    out_color = vec4(clamp(shaded, vec3(0.0), vec3(1.0)), surface_albedo.a);
}
