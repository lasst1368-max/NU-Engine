#version 450

layout(location = 0) in vec2 local_uv;
layout(location = 1) in vec4 draw_color;
layout(location = 2) flat in float primitive_kind;
layout(location = 3) in vec2 draw_size;
layout(location = 4) flat in float stroke_width;

layout(location = 0) out vec4 out_color;

float sdBox(vec2 point, vec2 half_extent) {
    vec2 distance = abs(point) - half_extent;
    return length(max(distance, vec2(0.0))) + min(max(distance.x, distance.y), 0.0);
}

float fillRectDistance(vec2 point) {
    return sdBox(point, vec2(1.0));
}

float fillCircleDistance(vec2 point) {
    return length(point) - 1.0;
}

float strokeRectDistance(vec2 point, float stroke_width) {
    vec2 stroke_uv = vec2(
        (2.0 * stroke_width) / max(draw_size.x, 0.0001),
        (2.0 * stroke_width) / max(draw_size.y, 0.0001)
    );
    vec2 inner_half_extent = max(vec2(0.0), vec2(1.0) - stroke_uv);
    float outer = sdBox(point, vec2(1.0));
    float inner = sdBox(point, inner_half_extent);
    return max(outer, -inner);
}

float strokeCircleDistance(vec2 point, float stroke_width) {
    float outer = fillCircleDistance(point);
    float inner_radius =
        max(0.0, 1.0 - ((2.0 * stroke_width) / max(min(draw_size.x, draw_size.y), 0.0001)));
    return max(outer, inner_radius - length(point));
}

void main() {
    int kind = int(primitive_kind + 0.5);

    if (kind == 5) {
        out_color = draw_color;
        return;
    }

    float width = max(stroke_width, 0.0);
    float signed_distance = fillRectDistance(local_uv);

    if (kind == 1) {
        signed_distance = fillCircleDistance(local_uv);
    } else if (kind == 3) {
        signed_distance = strokeRectDistance(local_uv, width);
    } else if (kind == 4) {
        signed_distance = strokeCircleDistance(local_uv, width);
    }

    float aa_width = max(fwidth(signed_distance), 0.001);
    float coverage = 1.0 - smoothstep(-aa_width, aa_width, signed_distance);
    vec4 color = vec4(draw_color.rgb, draw_color.a * coverage);

    if (color.a <= 0.001) {
        discard;
    }

    out_color = color;
}
