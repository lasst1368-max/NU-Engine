#include "nu_gl_scratch.hpp"

#include <array>
#include <cstddef>
#include <cstdint>
#include <iostream>
#include <vector>

using namespace nu::gl;

struct BlockVertex {
    float position[3];
    float normal[3];
    float uv[2];
};

static std::array<float, 16> identity4x4() {
    return {
        1.0f, 0.0f, 0.0f, 0.0f,
        0.0f, 1.0f, 0.0f, 0.0f,
        0.0f, 0.0f, 1.0f, 0.0f,
        0.0f, 0.0f, 0.0f, 1.0f,
    };
}

static std::vector<BlockVertex> build_minecraft_block_vertices() {
    const float u0 = 0.0f;
    const float u1 = 1.0f / 3.0f;
    const float u2 = 2.0f / 3.0f;
    const float u3 = 1.0f;
    const float v0 = 0.0f;
    const float v1 = 1.0f;

    const std::array<std::array<float, 3>, 8> corners = {{
        {{-0.5f, -0.5f, -0.5f}},
        {{ 0.5f, -0.5f, -0.5f}},
        {{ 0.5f,  0.5f, -0.5f}},
        {{-0.5f,  0.5f, -0.5f}},
        {{-0.5f, -0.5f,  0.5f}},
        {{ 0.5f, -0.5f,  0.5f}},
        {{ 0.5f,  0.5f,  0.5f}},
        {{-0.5f,  0.5f,  0.5f}},
    }};

    struct Face {
        std::array<int, 4> indices;
        std::array<float, 3> normal;
        std::array<float, 4> uv_rect;
    };

    const std::array<Face, 6> faces = {{
        {{{4, 5, 6, 7}}, {{ 0.0f,  0.0f,  1.0f}}, {{u1, v0, u2, v1}}}, // grass side
        {{{1, 0, 3, 2}}, {{ 0.0f,  0.0f, -1.0f}}, {{u1, v0, u2, v1}}},
        {{{0, 4, 7, 3}}, {{-1.0f,  0.0f,  0.0f}}, {{u1, v0, u2, v1}}},
        {{{5, 1, 2, 6}}, {{ 1.0f,  0.0f,  0.0f}}, {{u1, v0, u2, v1}}},
        {{{3, 7, 6, 2}}, {{ 0.0f,  1.0f,  0.0f}}, {{u0, v0, u1, v1}}}, // grass top
        {{{0, 1, 5, 4}}, {{ 0.0f, -1.0f,  0.0f}}, {{u2, v0, u3, v1}}}, // dirt bottom
    }};

    std::vector<BlockVertex> vertices;
    vertices.reserve(24);

    for (const Face& face : faces) {
        const float face_u0 = face.uv_rect[0];
        const float face_v0 = face.uv_rect[1];
        const float face_u1 = face.uv_rect[2];
        const float face_v1 = face.uv_rect[3];
        const std::array<std::array<float, 2>, 4> uvs = {{
            {{face_u0, face_v0}},
            {{face_u1, face_v0}},
            {{face_u1, face_v1}},
            {{face_u0, face_v1}},
        }};

        for (int i = 0; i < 4; ++i) {
            const auto& corner = corners[face.indices[i]];
            vertices.push_back({
                {corner[0], corner[1], corner[2]},
                {face.normal[0], face.normal[1], face.normal[2]},
                {uvs[i][0], uvs[i][1]},
            });
        }
    }

    return vertices;
}

static std::vector<std::uint32_t> build_minecraft_block_indices() {
    std::vector<std::uint32_t> indices;
    indices.reserve(36);
    for (std::uint32_t face = 0; face < 6; ++face) {
        const std::uint32_t base = face * 4;
        indices.push_back(base + 0);
        indices.push_back(base + 1);
        indices.push_back(base + 2);
        indices.push_back(base + 0);
        indices.push_back(base + 2);
        indices.push_back(base + 3);
    }
    return indices;
}

static std::vector<std::uint8_t> build_minecraft_block_atlas_rgba8(std::uint32_t width, std::uint32_t height) {
    std::vector<std::uint8_t> pixels(width * height * 4, 255);
    const std::uint32_t tile_width = width / 3;

    auto fill_tile = [&](std::uint32_t tile_x, std::uint8_t r, std::uint8_t g, std::uint8_t b) {
        const std::uint32_t start_x = tile_x * tile_width;
        const std::uint32_t end_x = start_x + tile_width;
        for (std::uint32_t y = 0; y < height; ++y) {
            for (std::uint32_t x = start_x; x < end_x; ++x) {
                const std::size_t index = (static_cast<std::size_t>(y) * width + x) * 4;
                pixels[index + 0] = r;
                pixels[index + 1] = g;
                pixels[index + 2] = b;
                pixels[index + 3] = 255;
            }
        }
    };

    fill_tile(0, 99, 170, 58);   // grass top
    fill_tile(1, 113, 85, 47);   // grass side / dirt
    fill_tile(2, 90, 58, 32);    // dirt bottom

    return pixels;
}

int main() {
    ScratchContext ctx;
    MakeCurrent(ctx);

    GLuint vao = 0;
    GLuint vbo = 0;
    GLuint ebo = 0;
    GLuint atlas = 0;
    glGenVertexArrays(1, &vao);
    glGenBuffers(1, &vbo);
    glGenBuffers(1, &ebo);
    glGenTextures(1, &atlas);

    const std::vector<BlockVertex> vertices = build_minecraft_block_vertices();
    const std::vector<std::uint32_t> indices = build_minecraft_block_indices();
    const std::vector<std::uint8_t> atlas_pixels = build_minecraft_block_atlas_rgba8(96, 32);
    const std::array<float, 16> model = identity4x4();
    const std::array<float, 16> view = identity4x4();
    const std::array<float, 16> projection = identity4x4();

    glViewport(0, 0, 1280, 720);
    glClearColor(0.53f, 0.81f, 0.92f, 1.0f);
    glClear(GL_COLOR_BUFFER_BIT | GL_DEPTH_BUFFER_BIT);
    glEnable(GL_DEPTH_TEST);
    glEnable(GL_CULL_FACE);

    glBindVertexArray(vao);

    glBindBuffer(GL_ARRAY_BUFFER, vbo);
    glBufferData(GL_ARRAY_BUFFER, vertices.size() * sizeof(BlockVertex), vertices.data(), GL_STATIC_DRAW);

    glBindBuffer(GL_ELEMENT_ARRAY_BUFFER, ebo);
    glBufferData(GL_ELEMENT_ARRAY_BUFFER, indices.size() * sizeof(std::uint32_t), indices.data(), GL_STATIC_DRAW);

    glVertexAttribPointer(0, 3, GL_FLOAT, GL_FALSE, sizeof(BlockVertex), 0);
    glEnableVertexAttribArray(0);
    glVertexAttribPointer(1, 3, GL_FLOAT, GL_FALSE, sizeof(BlockVertex), offsetof(BlockVertex, normal));
    glEnableVertexAttribArray(1);
    glVertexAttribPointer(2, 2, GL_FLOAT, GL_FALSE, sizeof(BlockVertex), offsetof(BlockVertex, uv));
    glEnableVertexAttribArray(2);

    glActiveTexture(GL_TEXTURE0);
    glBindTexture(GL_TEXTURE_2D, atlas);
    glTexImage2D(GL_TEXTURE_2D, 0, GL_RGBA, 96, 32, 0, GL_RGBA, GL_UNSIGNED_BYTE, atlas_pixels.data());

    glUseProgram(101);
    glUniformMatrix4fv("u_model", model.data());
    glUniformMatrix4fv("u_view", view.data());
    glUniformMatrix4fv("u_projection", projection.data());
    glUniform3f("u_sunDirection", -0.45f, 0.82f, -0.35f);

    glDrawElements(GL_TRIANGLES, static_cast<GLuint>(indices.size()), GL_UNSIGNED_INT, 0);

    std::cout
        << "nu C++ OpenGL-syntax scratch sample\n"
        << "recorded commands: " << ctx.CommandCount() << "\n"
        << "minecraft-style block: grass top, dirt bottom, grass side atlas layout\n"
        << "walk: WASD  look: arrows  place: E  remove: Q\n"
        << "vao=" << vao << " vbo=" << vbo << " ebo=" << ebo << " atlas=" << atlas << "\n"
        << std::flush;

    if (!RunPreviewWindow("nu C++ Scratch Preview", 1280, 720)) {
        std::cerr << "failed to open nu scratch preview window\n";
        return 1;
    }

    return 0;
}
