#version 450

layout(push_constant) uniform PrimitivePushConstants {
    vec4 color;
    vec2 center;
    vec2 size;
} pc;

vec2 positions[6] = vec2[](
    vec2(-0.5, -0.5),
    vec2(0.5, -0.5),
    vec2(0.5, 0.5),
    vec2(-0.5, -0.5),
    vec2(0.5, 0.5),
    vec2(-0.5, 0.5)
);

void main() {
    vec2 pos = positions[gl_VertexIndex] * pc.size + pc.center;
    gl_Position = vec4(pos, 0.0, 1.0);
}
