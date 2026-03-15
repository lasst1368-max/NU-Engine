#version 450

layout(set = 0, binding = 0) uniform CubeScene {
    vec4 camera_position;
    vec4 point_light_positions[4];    // xyz=pos, w=range
    vec4 point_light_colors[4];       // xyz=color, w=intensity
    vec4 point_light_shadow_flags;    // per-light shadow enable (0 or 1)
    vec4 ambient_color_intensity;     // xyz=color, w=intensity
    vec4 fill_direction_intensity;    // xyz=direction, w=intensity
    vec4 fill_color;                  // xyz=color, w=point_light_count
    vec4 material;                    // z=object_count, w=ao_strength
    vec4 shadow_params;
    vec4 shadow_rows[4];
    // Spotlights — packed as 3 vec4s per light:
    //   [0] xyz=position,  w=range
    //   [1] xyz=direction, w=intensity
    //   [2] xyz=color,     w=spot_light_count (only used in lights[0].w)
    //   [3] x=cos_inner,   y=cos_outer  (cone cutoffs)
    vec4 spot_lights[4 * 4];
} scene;

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

layout(set = 1, binding = 0) uniform sampler2D albedo_texture;
layout(set = 2, binding = 0) uniform sampler2D shadow_map;
// Normal map (tangent-space). Bind a 1×1 flat-blue texture (0.5,0.5,1.0) when unused.
// draw_material.w >= 0.5 enables the normal map sample; < 0.5 skips it.
layout(set = 3, binding = 0) uniform sampler2D normal_map;

layout(location = 0) in vec3 world_position;
layout(location = 1) in vec3 world_normal;
layout(location = 2) in vec2 draw_uv;
layout(location = 3) in vec4 draw_albedo;
layout(location = 4) flat in uint draw_object_index;
layout(location = 5) in vec4 draw_material;
layout(location = 7) in vec3 draw_tangent;
layout(location = 8) in vec3 draw_bitangent;
layout(location = 0) out vec4 out_color;

vec3 aces_film(vec3 color) {
    const float a = 2.51;
    const float b = 0.03;
    const float c = 2.43;
    const float d = 0.59;
    const float e = 0.14;
    return clamp((color * (a * color + b)) / (color * (c * color + d) + e), 0.0, 1.0);
}

vec3 linear_to_srgb(vec3 color) {
    return pow(clamp(color, vec3(0.0), vec3(1.0)), vec3(1.0 / 2.2));
}

vec4 transform_shadow(vec4 world) {
    return vec4(
        dot(scene.shadow_rows[0], world),
        dot(scene.shadow_rows[1], world),
        dot(scene.shadow_rows[2], world),
        dot(scene.shadow_rows[3], world)
    );
}

float sample_shadow_depth(vec2 uv) {
    return texture(shadow_map, uv).r;
}

float hash11(float value) {
    return fract(sin(value * 91.3458) * 47453.5453);
}

mat2 rotation2(float angle) {
    float s = sin(angle);
    float c = cos(angle);
    return mat2(c, -s, s, c);
}

vec2 vogel_disk_sample(int index, int sample_count) {
    const float golden_angle = 2.39996323;
    float radius = sqrt((float(index) + 0.5) / max(float(sample_count), 1.0));
    float theta = float(index) * golden_angle;
    return vec2(cos(theta), sin(theta)) * radius;
}

float shadow_visibility(vec3 normal) {
    vec3 fill_direction = normalize(scene.fill_direction_intensity.xyz);
    float bias = max(
        scene.shadow_params.y * (1.0 - max(dot(normal, fill_direction), 0.0)),
        0.00035
    );
    vec3 shadow_origin = world_position + normal * (bias * 3.0);
    vec4 shadow_clip = transform_shadow(vec4(shadow_origin, 1.0));
    if (shadow_clip.w <= 0.0001) {
        return 1.0;
    }

    vec3 shadow_ndc = shadow_clip.xyz / shadow_clip.w;
    vec2 uv = shadow_ndc.xy * 0.5 + 0.5;
    if (uv.x <= 0.0 || uv.x >= 1.0 || uv.y <= 0.0 || uv.y >= 1.0) {
        return 1.0;
    }
    if (shadow_ndc.z <= 0.0 || shadow_ndc.z >= 1.0) {
        return 1.0;
    }

    float current_depth = shadow_ndc.z - max(bias * 0.18, 0.00005);
    float texel_size = max(scene.shadow_params.z, 0.00001);
    float filter_radius = max(scene.shadow_params.w, 1.0);
    float random_angle = hash11(float(draw_object_index) * 0.618) * 6.2831853;
    mat2 basis = rotation2(random_angle);
    int sample_count = filter_radius >= 2.4 ? 64 : 12;
    float visible = 0.0;
    float total = float(sample_count);

    for (int i = 0; i < 64; i++) {
        if (i >= sample_count) {
            break;
        }
        vec2 offset = basis * vogel_disk_sample(i, sample_count) * texel_size * filter_radius * 1.8;
        float closest_depth = sample_shadow_depth(uv + offset);
        visible += current_depth <= closest_depth ? 1.0 : 0.0;
    }

    float visibility = visible / max(total, 1.0);
    float min_visibility = scene.shadow_params.x;
    return min_visibility + ((1.0 - min_visibility) * clamp(visibility, 0.0, 1.0));
}

vec3 closest_point_on_cube(SceneCube cube, vec3 point) {
    vec3 relative = point - cube.center.xyz;
    vec3 local = vec3(
        dot(relative, cube.axis_x.xyz),
        dot(relative, cube.axis_y.xyz),
        dot(relative, cube.axis_z.xyz)
    );
    vec3 clamped_local = clamp(local, -cube.half_extents.xyz, cube.half_extents.xyz);
    return cube.center.xyz
        + cube.axis_x.xyz * clamped_local.x
        + cube.axis_y.xyz * clamped_local.y
        + cube.axis_z.xyz * clamped_local.z;
}

bool ray_segment_intersects_cube(
    SceneCube cube,
    vec3 origin,
    vec3 direction,
    float max_distance
) {
    vec3 relative = origin - cube.center.xyz;
    vec3 local_origin = vec3(
        dot(relative, cube.axis_x.xyz),
        dot(relative, cube.axis_y.xyz),
        dot(relative, cube.axis_z.xyz)
    );
    vec3 local_direction = vec3(
        dot(direction, cube.axis_x.xyz),
        dot(direction, cube.axis_y.xyz),
        dot(direction, cube.axis_z.xyz)
    );

    float t_min = 0.0;
    float t_max = max_distance;
    for (int axis = 0; axis < 3; axis++) {
        float origin_axis = local_origin[axis];
        float direction_axis = local_direction[axis];
        float extent_axis = cube.half_extents[axis];
        if (abs(direction_axis) < 0.0001) {
            if (origin_axis < -extent_axis || origin_axis > extent_axis) {
                return false;
            }
            continue;
        }
        float inv_direction = 1.0 / direction_axis;
        float t0 = (-extent_axis - origin_axis) * inv_direction;
        float t1 = (extent_axis - origin_axis) * inv_direction;
        if (t0 > t1) {
            float tmp = t0;
            t0 = t1;
            t1 = tmp;
        }
        t_min = max(t_min, t0);
        t_max = min(t_max, t1);
        if (t_max < t_min) {
            return false;
        }
    }

    return t_max > 0.0 && t_min < max_distance;
}

float ambient_occlusion(vec3 normal) {
    int object_count = clamp(int(scene.material.z), 0, 64);
    float ao_strength = clamp(scene.material.w, 0.0, 1.0);
    if (object_count <= 1 || ao_strength <= 0.001) {
        return 1.0;
    }

    vec3 sample_origin = world_position + normal * 0.04;
    float occlusion = 0.0;
    for (int i = 0; i < object_count; i++) {
        if (i == int(draw_object_index)) {
            continue;
        }
        SceneCube cube = cube_objects.cubes[i];
        if (cube.center.w <= 0.001) {
            continue;
        }
        vec3 closest = closest_point_on_cube(cube, sample_origin);
        vec3 to_occluder = closest - sample_origin;
        float distance_to_occluder = length(to_occluder);
        if (distance_to_occluder <= 0.001 || distance_to_occluder > 3.5) {
            continue;
        }
        vec3 occluder_direction = to_occluder / distance_to_occluder;
        float facing = max(dot(normal, occluder_direction), 0.0);
        if (facing <= 0.0001) {
            continue;
        }
        float extent = max(cube.half_extents.x, max(cube.half_extents.y, cube.half_extents.z));
        float reach = max(0.45, extent * 1.55);
        float falloff = 1.0 - smoothstep(0.05, reach, distance_to_occluder);
        occlusion += facing * falloff * cube.center.w;
    }

    float ao = 1.0 - clamp(occlusion * 0.18 * ao_strength, 0.0, 0.38);
    return clamp(ao, 0.62, 1.0);
}

float point_light_visibility(vec3 normal, vec3 light_position, float shadow_enabled) {
    if (shadow_enabled < 0.5) {
        return 1.0;
    }
    int object_count = clamp(int(scene.material.z), 0, 128);
    if (object_count <= 1) {
        return 1.0;
    }

    vec3 ray = world_position - light_position;
    float distance_to_surface = length(ray);
    if (distance_to_surface <= 0.001) {
        return 1.0;
    }
    vec3 direction = ray / distance_to_surface;
    vec3 origin = light_position + direction * 0.03;
    vec3 receiver = world_position - normal * 0.025;
    float max_distance = max(length(receiver - origin), 0.0);

    for (int i = 0; i < object_count; i++) {
        if (i == int(draw_object_index)) {
            continue;
        }
        SceneCube cube = cube_objects.cubes[i];
        if (cube.axis_x.w <= 0.001) {
            continue;
        }
        if (ray_segment_intersects_cube(cube, origin, direction, max_distance)) {
            return 0.0;
        }
    }
    return 1.0;
}

float distribution_ggx(vec3 N, vec3 H, float roughness) {
    float a = roughness * roughness;
    float a2 = a * a;
    float NdotH = max(dot(N, H), 0.0);
    float NdotH2 = NdotH * NdotH;
    float denom = NdotH2 * (a2 - 1.0) + 1.0;
    return a2 / max(3.14159265 * denom * denom, 0.0001);
}

float geometry_schlick_ggx(float NdotV, float roughness) {
    float r = roughness + 1.0;
    float k = (r * r) / 8.0;
    return NdotV / max(NdotV * (1.0 - k) + k, 0.0001);
}

float geometry_smith(vec3 N, vec3 V, vec3 L, float roughness) {
    return geometry_schlick_ggx(max(dot(N, V), 0.0), roughness)
        * geometry_schlick_ggx(max(dot(N, L), 0.0), roughness);
}

vec3 fresnel_schlick(float cosTheta, vec3 F0) {
    return F0 + (1.0 - F0) * pow(clamp(1.0 - cosTheta, 0.0, 1.0), 5.0);
}

// ── Spotlight evaluation ─────────────────────────────────────────────────────
// Each spotlight is stored as 4 consecutive vec4s in scene.spot_lights[]:
//   base+0: xyz=position,  w=range
//   base+1: xyz=direction, w=intensity
//   base+2: xyz=color,     w=<unused per-light>
//   base+3: x=cos_inner,   y=cos_outer
//
// Cone attenuation: smoothstep from cos_inner to cos_outer, multiplied by
// inverse-square distance attenuation.  Full PBR specular + diffuse are computed
// identically to the point-light loop.
vec3 evaluate_spot_light(
    int base,
    vec3 N, vec3 V, vec3 albedo,
    float roughness, float metallic, vec3 F0
) {
    vec3 lpos       = scene.spot_lights[base + 0].xyz;
    float range     = max(scene.spot_lights[base + 0].w, 0.0001);
    vec3 ldir       = normalize(scene.spot_lights[base + 1].xyz);
    float intensity = scene.spot_lights[base + 1].w;
    vec3 lcolor     = scene.spot_lights[base + 2].xyz;
    float cos_inner = scene.spot_lights[base + 3].x;
    float cos_outer = scene.spot_lights[base + 3].y;

    vec3 to_light     = lpos - world_position;
    float dist        = max(length(to_light), 0.0001);
    vec3 L            = to_light / dist;
    float NdotL       = max(dot(N, L), 0.0);
    if (NdotL <= 0.0) { return vec3(0.0); }

    // Distance attenuation (same formula as point lights).
    float attenuation = 1.0 / (1.0 + (dist * dist) / (range * range));

    // Cone attenuation: 1 inside inner, smooth falloff to 0 at outer.
    float cos_angle     = dot(-L, ldir);
    float penumbra      = max(cos_inner - cos_outer, 0.0001);
    float cone_factor   = clamp((cos_angle - cos_outer) / penumbra, 0.0, 1.0);
    cone_factor         = cone_factor * cone_factor; // squared for smoother falloff

    if (cone_factor <= 0.0) { return vec3(0.0); }

    vec3 radiance = lcolor * (intensity * attenuation * cone_factor);

    vec3 H    = normalize(L + V);
    vec3 F    = fresnel_schlick(max(dot(H, V), 0.0), F0);
    float NDF = distribution_ggx(N, H, roughness);
    float G   = geometry_smith(N, V, L, roughness);
    vec3 specular = (NDF * G * F) / max(4.0 * max(dot(N, V), 0.0) * NdotL, 0.0001);
    vec3 kS   = F;
    vec3 kD   = (vec3(1.0) - kS) * (1.0 - metallic);

    return ((kD * albedo / 3.14159265) + specular) * radiance * NdotL;
}

void main() {
    vec4 albedo_sample = texture(albedo_texture, draw_uv);
    vec4 surface_albedo = vec4(
        draw_albedo.rgb * albedo_sample.rgb,
        draw_albedo.a * albedo_sample.a
    );

    vec3 albedo = pow(max(surface_albedo.rgb, vec3(0.0)), vec3(2.2));

    // Normal mapping: draw_material.w >= 0.5 means a normal map is bound at set=3.
    // The TBN matrix transforms tangent-space normals into world space.
    vec3 normal;
    if (draw_material.w >= 0.5) {
        vec3 nm_sample = texture(normal_map, draw_uv).rgb;
        vec3 tangent_normal = nm_sample * 2.0 - 1.0; // unpack [0,1] -> [-1,1]
        // Build TBN: columns are world-space T, B, N.
        mat3 TBN = mat3(
            normalize(draw_tangent),
            normalize(draw_bitangent),
            normalize(world_normal)
        );
        normal = normalize(TBN * tangent_normal);
    } else {
        normal = normalize(world_normal);
    }

    vec3 view_direction = normalize(scene.camera_position.xyz - world_position);
    vec3 fill_direction = normalize(scene.fill_direction_intensity.xyz);
    float roughness = clamp(draw_material.x, 0.045, 1.0);
    float metallic = clamp(draw_material.y, 0.0, 1.0);
    float emissive_intensity = max(draw_material.z, 0.0);
    float hemisphere = normal.y * 0.5 + 0.5;
    vec3 fill_radiance = scene.fill_color.rgb * scene.fill_direction_intensity.w;
    vec3 sky_ambient = scene.ambient_color_intensity.rgb;
    vec3 ground_ambient = scene.ambient_color_intensity.rgb * vec3(0.28, 0.24, 0.22);
    vec3 ambient_color = mix(ground_ambient, sky_ambient, hemisphere);
    vec3 F0 = mix(vec3(0.04), albedo, metallic);
    vec3 shadow_lift = albedo * 0.035;
    float scene_shadow = shadow_visibility(normal);
    float ao = ambient_occlusion(normal);
    vec3 direct_lighting = vec3(0.0);

    float fill_NdotL = max(dot(normal, fill_direction), 0.0);
    if (fill_NdotL > 0.0) {
        vec3 fill_half = normalize(fill_direction + view_direction);
        vec3 F = fresnel_schlick(max(dot(fill_half, view_direction), 0.0), F0);
        float NDF = distribution_ggx(normal, fill_half, roughness);
        float G = geometry_smith(normal, view_direction, fill_direction, roughness);
        vec3 specular = (NDF * G * F) / max(4.0 * max(dot(normal, view_direction), 0.0) * fill_NdotL, 0.0001);
        vec3 kS = F;
        vec3 kD = (vec3(1.0) - kS) * (1.0 - metallic);
        direct_lighting += ((kD * albedo / 3.14159265) + specular)
            * fill_radiance
            * fill_NdotL
            * scene_shadow;
    }

    int point_light_count = clamp(int(scene.fill_color.w), 0, 4);
    for (int i = 0; i < point_light_count; i++) {
        vec3 light_vector = scene.point_light_positions[i].xyz - world_position;
        float distance_to_light = max(length(light_vector), 0.0001);
        vec3 light_direction = light_vector / distance_to_light;
        float NdotL = max(dot(normal, light_direction), 0.0);
        if (NdotL <= 0.0) {
            continue;
        }
        vec3 half_vector = normalize(light_direction + view_direction);
        float range = max(scene.point_light_positions[i].w, 0.0001);
        float attenuation =
            1.0 / (1.0 + (distance_to_light * distance_to_light) / (range * range));
        vec3 light_radiance = scene.point_light_colors[i].rgb
            * (scene.point_light_colors[i].w * attenuation);
        float point_shadow = point_light_visibility(
            normal,
            scene.point_light_positions[i].xyz,
            scene.point_light_shadow_flags[i]
        );
        vec3 F = fresnel_schlick(max(dot(half_vector, view_direction), 0.0), F0);
        float NDF = distribution_ggx(normal, half_vector, roughness);
        float G = geometry_smith(normal, view_direction, light_direction, roughness);
        vec3 specular = (NDF * G * F) / max(4.0 * max(dot(normal, view_direction), 0.0) * NdotL, 0.0001);
        vec3 kS = F;
        vec3 kD = (vec3(1.0) - kS) * (1.0 - metallic);
        direct_lighting += ((kD * albedo / 3.14159265) + specular)
            * light_radiance
            * NdotL
            * point_shadow;
    }

    // Spotlight loop — spot_lights[0].z holds the count in the .w channel of lights[0].
    int spot_count = clamp(int(scene.spot_lights[2].w), 0, 4);
    for (int i = 0; i < spot_count; i++) {
        direct_lighting += evaluate_spot_light(
            i * 4,
            normal, view_direction, albedo, roughness, metallic, F0
        );
    }

    vec3 ambient_diffuse = (1.0 - metallic) * albedo * ambient_color * scene.ambient_color_intensity.w;
    vec3 ambient_specular = mix(vec3(0.02), ambient_color, 0.35) * F0 * mix(0.35, 1.0, 1.0 - roughness);
    float fresnel = pow(1.0 - max(dot(normal, view_direction), 0.0), 5.0);
    vec3 rim_term = mix(vec3(0.0), sky_ambient * 0.08, fresnel) * (1.0 - roughness);
    vec3 shaded = ((ambient_diffuse + ambient_specular + shadow_lift) * ao)
        + direct_lighting
        + rim_term;

    float view_distance = length(scene.camera_position.xyz - world_position);
    float fog = 1.0 - exp(-view_distance * 0.028);
    fog *= 0.42 + hemisphere * 0.58;
    vec3 fog_color = mix(
        ground_ambient * 0.8,
        sky_ambient * 1.35 + fill_radiance * 0.25,
        hemisphere
    );
    // Emissive: bypasses lighting and fog so it always glows at full intensity.
    // draw_material.z is the emissive multiplier (0 = off, >0 = HDR glow).
    vec3 emissive = albedo * emissive_intensity;

    vec3 graded = mix(shaded, fog_color, clamp(fog, 0.0, 0.38));
    graded += emissive;
    vec3 mapped = aces_film(max(graded, vec3(0.0)));

    out_color = vec4(linear_to_srgb(mapped), surface_albedo.a);
}
