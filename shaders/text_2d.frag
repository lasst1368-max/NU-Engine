#version 450

layout(set = 0, binding = 0) uniform sampler2D font_atlas;

layout(location = 0) in vec2 out_uv;
layout(location = 1) in vec4 out_color;

layout(location = 0) out vec4 frag_color;

void main() {
    float alpha = texture(font_atlas, out_uv).a;
    frag_color = vec4(out_color.rgb, out_color.a * alpha);
    if (frag_color.a <= 0.001) {
        discard;
    }
}
