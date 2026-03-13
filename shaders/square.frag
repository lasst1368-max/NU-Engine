#version 450

layout(push_constant) uniform PrimitivePushConstants {
    vec4 color;
    vec2 center;
    vec2 size;
} pc;

layout(location = 0) out vec4 out_color;

void main() {
    out_color = pc.color;
}
