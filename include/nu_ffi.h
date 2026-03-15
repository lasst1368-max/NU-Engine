#ifndef NU_FFI_H
#define NU_FFI_H

#include <stdbool.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

#define NU_FFI_CLEAR_COLOR_BIT 1u
#define NU_FFI_CLEAR_DEPTH_BIT 2u
#define NU_FFI_CLEAR_STENCIL_BIT 4u

#define NU_FFI_RENDER_STATE_DEPTH_TEST 1u
#define NU_FFI_RENDER_STATE_BLEND 2u
#define NU_FFI_RENDER_STATE_CULL_FACE 4u

#define NU_FFI_BUFFER_TARGET_ARRAY 1u
#define NU_FFI_BUFFER_TARGET_ELEMENT_ARRAY 2u
#define NU_FFI_BUFFER_TARGET_UNIFORM 3u

#define NU_FFI_BUFFER_USAGE_STATIC_DRAW 1u
#define NU_FFI_BUFFER_USAGE_DYNAMIC_DRAW 2u
#define NU_FFI_BUFFER_USAGE_STREAM_DRAW 3u

#define NU_FFI_VERTEX_ATTRIB_FLOAT32 1u
#define NU_FFI_VERTEX_ATTRIB_UNSIGNED_SHORT 2u
#define NU_FFI_VERTEX_ATTRIB_UNSIGNED_INT 3u

#define NU_FFI_INDEX_TYPE_U16 1u
#define NU_FFI_INDEX_TYPE_U32 2u

#define NU_FFI_TOPOLOGY_TRIANGLES 1u
#define NU_FFI_TOPOLOGY_LINES 2u
#define NU_FFI_TOPOLOGY_POINTS 3u

#define NU_FFI_ATTACHMENT_COLOR0 1u
#define NU_FFI_ATTACHMENT_DEPTH 2u
#define NU_FFI_ATTACHMENT_DEPTH_STENCIL 3u
#define NU_FFI_TEXTURE_FORMAT_RGBA8 1u
#define NU_FFI_BACKEND_KIND_VULKAN 1u
#define NU_FFI_BACKEND_KIND_DX12 2u
#define NU_FFI_BACKEND_KIND_METAL 3u

typedef struct NuGlScratchContext NuGlScratchContext;

uint32_t nu_ffi_backend_kind(void);
const char *nu_ffi_backend_name(void);
const char *nu_ffi_backend_dll_name(void);
const char *nu_ffi_backend_display_name(void);

NuGlScratchContext *nu_ffi_gl_context_create(void);
void nu_ffi_gl_context_destroy(NuGlScratchContext *ctx);
void nu_ffi_gl_context_reset(NuGlScratchContext *ctx);
uint64_t nu_ffi_gl_command_count(NuGlScratchContext *ctx);
bool nu_ffi_gl_preview_window(
    NuGlScratchContext *ctx,
    const char *title,
    uint32_t width,
    uint32_t height
);

void nu_ffi_gl_clear_color(NuGlScratchContext *ctx, float r, float g, float b, float a);
void nu_ffi_gl_clear(NuGlScratchContext *ctx, uint32_t flags);
void nu_ffi_gl_viewport(NuGlScratchContext *ctx, int32_t x, int32_t y, uint32_t width, uint32_t height);
bool nu_ffi_gl_enable(NuGlScratchContext *ctx, uint32_t state);
bool nu_ffi_gl_disable(NuGlScratchContext *ctx, uint32_t state);

void nu_ffi_gl_use_program(NuGlScratchContext *ctx, uint32_t shader);
void nu_ffi_gl_bind_vertex_array(NuGlScratchContext *ctx, uint32_t mesh);
void nu_ffi_gl_bind_framebuffer(NuGlScratchContext *ctx, uint32_t framebuffer);
bool nu_ffi_gl_bind_buffer(NuGlScratchContext *ctx, uint32_t target, uint32_t buffer);
bool nu_ffi_gl_bind_buffer_base(NuGlScratchContext *ctx, uint32_t target, uint32_t index, uint32_t buffer);
void nu_ffi_gl_active_texture(NuGlScratchContext *ctx, uint32_t slot);
void nu_ffi_gl_bind_texture_2d(NuGlScratchContext *ctx, uint32_t texture);

bool nu_ffi_gl_buffer_data(
    NuGlScratchContext *ctx,
    uint32_t target,
    uint64_t size_bytes,
    const uint8_t *data,
    uint32_t usage
);
bool nu_ffi_gl_buffer_sub_data(
    NuGlScratchContext *ctx,
    uint32_t target,
    uint64_t offset_bytes,
    uint64_t size_bytes,
    const uint8_t *data
);
bool nu_ffi_gl_vertex_attrib_pointer(
    NuGlScratchContext *ctx,
    uint32_t index,
    int32_t size,
    uint32_t attrib_type,
    bool normalized,
    int32_t stride,
    uint64_t offset_bytes
);
void nu_ffi_gl_enable_vertex_attrib_array(NuGlScratchContext *ctx, uint32_t index);
void nu_ffi_gl_disable_vertex_attrib_array(NuGlScratchContext *ctx, uint32_t index);
void nu_ffi_gl_vertex_attrib_divisor(NuGlScratchContext *ctx, uint32_t index, uint32_t divisor);

bool nu_ffi_gl_framebuffer_texture_2d(
    NuGlScratchContext *ctx,
    uint32_t attachment,
    uint32_t texture,
    int32_t level
);
bool nu_ffi_gl_tex_image_2d_rgba8(
    NuGlScratchContext *ctx,
    uint32_t texture,
    uint32_t width,
    uint32_t height,
    const uint8_t *pixels
);
bool nu_ffi_gl_framebuffer_renderbuffer(
    NuGlScratchContext *ctx,
    uint32_t attachment,
    uint32_t renderbuffer
);

bool nu_ffi_gl_draw_arrays(NuGlScratchContext *ctx, uint32_t topology, uint32_t first, uint32_t count);
bool nu_ffi_gl_draw_elements(
    NuGlScratchContext *ctx,
    uint32_t topology,
    uint32_t count,
    uint32_t index_type,
    uint64_t offset_bytes
);

bool nu_ffi_gl_uniform_mat4(NuGlScratchContext *ctx, const char *name, const float *values);
bool nu_ffi_gl_uniform_vec3(NuGlScratchContext *ctx, const char *name, float x, float y, float z);

uint32_t nu_ffi_gl_gen_buffers(NuGlScratchContext *ctx, uint32_t count, uint32_t *ids);
uint32_t nu_ffi_gl_gen_textures(NuGlScratchContext *ctx, uint32_t count, uint32_t *ids);
uint32_t nu_ffi_gl_gen_vertex_arrays(NuGlScratchContext *ctx, uint32_t count, uint32_t *ids);
uint32_t nu_ffi_gl_gen_framebuffers(NuGlScratchContext *ctx, uint32_t count, uint32_t *ids);
uint32_t nu_ffi_gl_gen_renderbuffers(NuGlScratchContext *ctx, uint32_t count, uint32_t *ids);

void nu_ffi_gl_delete_buffers(NuGlScratchContext *ctx, uint32_t count, const uint32_t *ids);
void nu_ffi_gl_delete_textures(NuGlScratchContext *ctx, uint32_t count, const uint32_t *ids);
void nu_ffi_gl_delete_vertex_arrays(NuGlScratchContext *ctx, uint32_t count, const uint32_t *ids);
void nu_ffi_gl_delete_framebuffers(NuGlScratchContext *ctx, uint32_t count, const uint32_t *ids);
void nu_ffi_gl_delete_renderbuffers(NuGlScratchContext *ctx, uint32_t count, const uint32_t *ids);

#ifdef __cplusplus
}
#endif

#endif
