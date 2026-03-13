#version 450

layout(location = 0) in vec4 in_color;
layout(location = 1) in vec4 in_rect;
layout(location = 2) in vec4 in_uv_rect;

layout(location = 0) out vec2 out_uv;
layout(location = 1) out vec4 out_color;

vec2 positions[6] = vec2[](
    vec2(-0.5, -0.5),
    vec2(0.5, -0.5),
    vec2(0.5, 0.5),
    vec2(-0.5, -0.5),
    vec2(0.5, 0.5),
    vec2(-0.5, 0.5)
);

vec2 uv_positions[6] = vec2[](
    vec2(0.0, 1.0),
    vec2(1.0, 1.0),
    vec2(1.0, 0.0),
    vec2(0.0, 1.0),
    vec2(1.0, 0.0),
    vec2(0.0, 0.0)
);

void main() {
    vec2 local = positions[gl_VertexIndex];
    vec2 scaled = local * in_rect.zw;
    gl_Position = vec4(in_rect.xy + scaled, 0.0, 1.0);

    vec2 uv = uv_positions[gl_VertexIndex];
    out_uv = mix(in_uv_rect.xy, in_uv_rect.zw, uv);
    out_color = in_color;
}
