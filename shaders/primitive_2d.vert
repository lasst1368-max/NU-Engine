#version 450

layout(location = 0) in vec4 in_color;
layout(location = 1) in vec4 in_data0;
layout(location = 2) in vec4 in_data1;
layout(location = 3) in vec4 in_data2;

layout(location = 0) out vec2 local_uv;
layout(location = 1) out vec4 draw_color;
layout(location = 2) flat out float primitive_kind;
layout(location = 3) out vec2 draw_size;
layout(location = 4) flat out float stroke_width;

vec2 positions[6] = vec2[](
    vec2(-0.5, -0.5),
    vec2(0.5, -0.5),
    vec2(0.5, 0.5),
    vec2(-0.5, -0.5),
    vec2(0.5, 0.5),
    vec2(-0.5, 0.5)
);

void main() {
    primitive_kind = in_data2.x;
    draw_color = in_color;

    if (int(primitive_kind + 0.5) == 5) {
        vec2 p0 = in_data0.xy;
        vec2 p1 = in_data0.zw;
        vec2 p2 = in_data1.xy;
        vec2 p3 = in_data1.zw;
        vec2 quad_vertices[6] = vec2[](p0, p1, p2, p0, p2, p3);

        local_uv = vec2(0.0, 0.0);
        draw_size = vec2(1.0, 1.0);
        stroke_width = 0.0;
        gl_Position = vec4(quad_vertices[gl_VertexIndex], 0.0, 1.0);
        return;
    }

    vec2 local = positions[gl_VertexIndex];
    vec2 scaled = local * in_data0.zw;
    vec2 rotated = vec2(
        scaled.x * in_data1.x - scaled.y * in_data1.y,
        scaled.x * in_data1.y + scaled.y * in_data1.x
    );

    local_uv = local * 2.0;
    draw_size = in_data0.zw;
    stroke_width = in_data2.y;
    gl_Position = vec4(rotated + in_data0.xy, 0.0, 1.0);
}
