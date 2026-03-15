#pragma once

#include "../../include/nu_ffi.h"

#include <cstdint>
#include <stdexcept>

namespace nu::gl {

using GLenum = std::uint32_t;
using GLuint = std::uint32_t;
using GLint = std::int32_t;
using GLsizei = std::uint32_t;
using GLsizeiptr = std::uint64_t;
using GLintptr = std::uint64_t;
using GLfloat = float;

inline constexpr GLenum GL_COLOR_BUFFER_BIT = NU_FFI_CLEAR_COLOR_BIT;
inline constexpr GLenum GL_DEPTH_BUFFER_BIT = NU_FFI_CLEAR_DEPTH_BIT;
inline constexpr GLenum GL_STENCIL_BUFFER_BIT = NU_FFI_CLEAR_STENCIL_BIT;

inline constexpr GLenum GL_DEPTH_TEST = NU_FFI_RENDER_STATE_DEPTH_TEST;
inline constexpr GLenum GL_BLEND = NU_FFI_RENDER_STATE_BLEND;
inline constexpr GLenum GL_CULL_FACE = NU_FFI_RENDER_STATE_CULL_FACE;

inline constexpr GLenum GL_ARRAY_BUFFER = NU_FFI_BUFFER_TARGET_ARRAY;
inline constexpr GLenum GL_ELEMENT_ARRAY_BUFFER = NU_FFI_BUFFER_TARGET_ELEMENT_ARRAY;
inline constexpr GLenum GL_UNIFORM_BUFFER = NU_FFI_BUFFER_TARGET_UNIFORM;

inline constexpr GLenum GL_STATIC_DRAW = NU_FFI_BUFFER_USAGE_STATIC_DRAW;
inline constexpr GLenum GL_DYNAMIC_DRAW = NU_FFI_BUFFER_USAGE_DYNAMIC_DRAW;
inline constexpr GLenum GL_STREAM_DRAW = NU_FFI_BUFFER_USAGE_STREAM_DRAW;

inline constexpr GLenum GL_FLOAT = NU_FFI_VERTEX_ATTRIB_FLOAT32;
inline constexpr GLenum GL_UNSIGNED_BYTE = 0x1401;
inline constexpr GLenum GL_UNSIGNED_SHORT = NU_FFI_INDEX_TYPE_U16;
inline constexpr GLenum GL_UNSIGNED_INT = NU_FFI_INDEX_TYPE_U32;

inline constexpr GLenum GL_TRIANGLES = NU_FFI_TOPOLOGY_TRIANGLES;
inline constexpr GLenum GL_LINES = NU_FFI_TOPOLOGY_LINES;
inline constexpr GLenum GL_POINTS = NU_FFI_TOPOLOGY_POINTS;

inline constexpr GLenum GL_TEXTURE_2D = 0x0DE1;
inline constexpr GLenum GL_TEXTURE0 = 0;
inline constexpr GLenum GL_FRAMEBUFFER = 0x8D40;
inline constexpr GLenum GL_RENDERBUFFER = 0x8D41;
inline constexpr GLenum GL_COLOR_ATTACHMENT0 = NU_FFI_ATTACHMENT_COLOR0;
inline constexpr GLenum GL_DEPTH_ATTACHMENT = NU_FFI_ATTACHMENT_DEPTH;
inline constexpr GLenum GL_DEPTH_STENCIL_ATTACHMENT = NU_FFI_ATTACHMENT_DEPTH_STENCIL;
inline constexpr GLenum GL_RGBA = NU_FFI_TEXTURE_FORMAT_RGBA8;
inline constexpr bool GL_FALSE = false;
inline constexpr bool GL_TRUE = true;

class ScratchContext {
public:
    ScratchContext() : handle_(nu_ffi_gl_context_create()) {
        if (!handle_) {
            throw std::runtime_error("nu_ffi_gl_context_create failed");
        }
    }

    ~ScratchContext() {
        if (handle_) {
            nu_ffi_gl_context_destroy(handle_);
        }
    }

    ScratchContext(const ScratchContext&) = delete;
    ScratchContext& operator=(const ScratchContext&) = delete;

    ScratchContext(ScratchContext&& other) noexcept : handle_(other.handle_) {
        other.handle_ = nullptr;
    }

    ScratchContext& operator=(ScratchContext&& other) noexcept {
        if (this != &other) {
            if (handle_) {
                nu_ffi_gl_context_destroy(handle_);
            }
            handle_ = other.handle_;
            other.handle_ = nullptr;
        }
        return *this;
    }

    void Reset() { nu_ffi_gl_context_reset(handle_); }
    std::uint64_t CommandCount() const { return nu_ffi_gl_command_count(handle_); }
    bool PreviewWindow(const char* title, std::uint32_t width, std::uint32_t height) const {
        return nu_ffi_gl_preview_window(handle_, title, width, height);
    }
    NuGlScratchContext* Handle() const { return handle_; }

private:
    NuGlScratchContext* handle_;
};

inline thread_local ScratchContext* g_current = nullptr;
inline thread_local GLuint g_bound_texture_2d = 0;

inline void MakeCurrent(ScratchContext& ctx) {
    g_current = &ctx;
}

inline NuGlScratchContext* Current() {
    if (!g_current) {
        throw std::runtime_error("no current nu::gl::ScratchContext");
    }
    return g_current->Handle();
}

inline bool RunPreviewWindow(const char* title, std::uint32_t width, std::uint32_t height) {
    if (!g_current) {
        throw std::runtime_error("no current nu::gl::ScratchContext");
    }
    return g_current->PreviewWindow(title, width, height);
}

inline void glClearColor(GLfloat r, GLfloat g, GLfloat b, GLfloat a) {
    nu_ffi_gl_clear_color(Current(), r, g, b, a);
}

inline void glClear(GLenum flags) {
    nu_ffi_gl_clear(Current(), flags);
}

inline void glViewport(GLint x, GLint y, GLsizei width, GLsizei height) {
    nu_ffi_gl_viewport(Current(), x, y, width, height);
}

inline void glEnable(GLenum state) {
    if (!nu_ffi_gl_enable(Current(), state)) {
        throw std::runtime_error("nu_ffi_gl_enable rejected state");
    }
}

inline void glDisable(GLenum state) {
    if (!nu_ffi_gl_disable(Current(), state)) {
        throw std::runtime_error("nu_ffi_gl_disable rejected state");
    }
}

inline void glUseProgram(GLuint shader) {
    nu_ffi_gl_use_program(Current(), shader);
}

inline void glBindVertexArray(GLuint mesh) {
    nu_ffi_gl_bind_vertex_array(Current(), mesh);
}

inline void glBindFramebuffer(GLenum, GLuint framebuffer) {
    nu_ffi_gl_bind_framebuffer(Current(), framebuffer);
}

inline void glBindBuffer(GLenum target, GLuint buffer) {
    if (!nu_ffi_gl_bind_buffer(Current(), target, buffer)) {
        throw std::runtime_error("nu_ffi_gl_bind_buffer rejected target");
    }
}

inline void glBindBufferBase(GLenum target, GLuint index, GLuint buffer) {
    if (!nu_ffi_gl_bind_buffer_base(Current(), target, index, buffer)) {
        throw std::runtime_error("nu_ffi_gl_bind_buffer_base rejected target");
    }
}

inline void glActiveTexture(GLenum slot) {
    nu_ffi_gl_active_texture(Current(), slot);
}

inline void glBindTexture(GLenum target, GLuint texture) {
    if (target != GL_TEXTURE_2D) {
        throw std::runtime_error("nu scratch only supports GL_TEXTURE_2D binding");
    }
    g_bound_texture_2d = texture;
    nu_ffi_gl_bind_texture_2d(Current(), texture);
}

template <typename T>
inline void glBufferData(GLenum target, std::size_t size_bytes, const T* data, GLenum usage) {
    if (!nu_ffi_gl_buffer_data(
            Current(),
            target,
            static_cast<std::uint64_t>(size_bytes),
            reinterpret_cast<const std::uint8_t*>(data),
            usage)) {
        throw std::runtime_error("nu_ffi_gl_buffer_data rejected target/usage");
    }
}

template <typename T>
inline void glBufferSubData(GLenum target, GLintptr offset_bytes, std::size_t size_bytes, const T* data) {
    if (!nu_ffi_gl_buffer_sub_data(
            Current(),
            target,
            static_cast<std::uint64_t>(offset_bytes),
            static_cast<std::uint64_t>(size_bytes),
            reinterpret_cast<const std::uint8_t*>(data))) {
        throw std::runtime_error("nu_ffi_gl_buffer_sub_data rejected target");
    }
}

inline void glTexImage2D(
    GLenum target,
    GLint level,
    GLint internal_format,
    GLsizei width,
    GLsizei height,
    GLint,
    GLenum format,
    GLenum type,
    const void* pixels
) {
    if (target != GL_TEXTURE_2D || level != 0 || internal_format != static_cast<GLint>(GL_RGBA) ||
        format != GL_RGBA || type != GL_UNSIGNED_BYTE || g_bound_texture_2d == 0) {
        throw std::runtime_error("nu scratch preview only supports bound GL_TEXTURE_2D RGBA8 uploads");
    }
    if (!nu_ffi_gl_tex_image_2d_rgba8(
            Current(),
            g_bound_texture_2d,
            width,
            height,
            reinterpret_cast<const std::uint8_t*>(pixels))) {
        throw std::runtime_error("nu_ffi_gl_tex_image_2d_rgba8 failed");
    }
}

inline void glVertexAttribPointer(
    GLuint index,
    GLint size,
    GLenum attrib_type,
    bool normalized,
    GLsizei stride,
    std::uint64_t offset_bytes
) {
    if (!nu_ffi_gl_vertex_attrib_pointer(
            Current(),
            index,
            size,
            attrib_type,
            normalized,
            stride,
            offset_bytes)) {
        throw std::runtime_error("nu_ffi_gl_vertex_attrib_pointer rejected attrib type");
    }
}

inline void glEnableVertexAttribArray(GLuint index) {
    nu_ffi_gl_enable_vertex_attrib_array(Current(), index);
}

inline void glDisableVertexAttribArray(GLuint index) {
    nu_ffi_gl_disable_vertex_attrib_array(Current(), index);
}

inline void glVertexAttribDivisor(GLuint index, GLuint divisor) {
    nu_ffi_gl_vertex_attrib_divisor(Current(), index, divisor);
}

inline void glFramebufferTexture2D(GLenum, GLenum attachment, GLenum, GLuint texture, GLint level) {
    if (!nu_ffi_gl_framebuffer_texture_2d(Current(), attachment, texture, level)) {
        throw std::runtime_error("nu_ffi_gl_framebuffer_texture_2d rejected attachment");
    }
}

inline void glFramebufferRenderbuffer(GLenum, GLenum attachment, GLenum, GLuint renderbuffer) {
    if (!nu_ffi_gl_framebuffer_renderbuffer(Current(), attachment, renderbuffer)) {
        throw std::runtime_error("nu_ffi_gl_framebuffer_renderbuffer rejected attachment");
    }
}

inline void glDrawArrays(GLenum topology, GLuint first, GLuint count) {
    if (!nu_ffi_gl_draw_arrays(Current(), topology, first, count)) {
        throw std::runtime_error("nu_ffi_gl_draw_arrays rejected topology");
    }
}

inline void glDrawElements(GLenum topology, GLuint count, GLenum index_type, std::uint64_t offset_bytes) {
    if (!nu_ffi_gl_draw_elements(Current(), topology, count, index_type, offset_bytes)) {
        throw std::runtime_error("nu_ffi_gl_draw_elements rejected topology/index type");
    }
}

inline void glUniformMatrix4fv(const char* name, const float* values) {
    if (!nu_ffi_gl_uniform_mat4(Current(), name, values)) {
        throw std::runtime_error("nu_ffi_gl_uniform_mat4 failed");
    }
}

inline void glUniform3f(const char* name, GLfloat x, GLfloat y, GLfloat z) {
    if (!nu_ffi_gl_uniform_vec3(Current(), name, x, y, z)) {
        throw std::runtime_error("nu_ffi_gl_uniform_vec3 failed");
    }
}

inline void glGenBuffers(GLsizei count, GLuint* ids) {
    nu_ffi_gl_gen_buffers(Current(), count, ids);
}

inline void glGenTextures(GLsizei count, GLuint* ids) {
    nu_ffi_gl_gen_textures(Current(), count, ids);
}

inline void glGenVertexArrays(GLsizei count, GLuint* ids) {
    nu_ffi_gl_gen_vertex_arrays(Current(), count, ids);
}

inline void glGenFramebuffers(GLsizei count, GLuint* ids) {
    nu_ffi_gl_gen_framebuffers(Current(), count, ids);
}

inline void glGenRenderbuffers(GLsizei count, GLuint* ids) {
    nu_ffi_gl_gen_renderbuffers(Current(), count, ids);
}

inline void glDeleteBuffers(GLsizei count, const GLuint* ids) {
    nu_ffi_gl_delete_buffers(Current(), count, ids);
}

inline void glDeleteTextures(GLsizei count, const GLuint* ids) {
    nu_ffi_gl_delete_textures(Current(), count, ids);
}

inline void glDeleteVertexArrays(GLsizei count, const GLuint* ids) {
    nu_ffi_gl_delete_vertex_arrays(Current(), count, ids);
}

inline void glDeleteFramebuffers(GLsizei count, const GLuint* ids) {
    nu_ffi_gl_delete_framebuffers(Current(), count, ids);
}

inline void glDeleteRenderbuffers(GLsizei count, const GLuint* ids) {
    nu_ffi_gl_delete_renderbuffers(Current(), count, ids);
}

}  // namespace nu::gl
