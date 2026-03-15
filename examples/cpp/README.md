# C++ OpenGL-Syntax Scratch Example

This folder contains a small C++ sample that links against `nu.dll` through the
`extern "C"` FFI boundary and uses OpenGL-style syntax on the C++ side.

Files:

- `nu_gl_scratch.hpp`
  - thin C++ wrapper over `include/nu_ffi.h`
  - exposes calls like `glClearColor`, `glBindBuffer`, `glVertexAttribPointer`,
    `glUseProgram`, `glDrawElements`
- `minecraft_block.cpp`
  - records a small Minecraft-like block draw:
    - grass top
    - dirt bottom
    - grass-side atlas strip
- `build_minecraft_block.ps1`
  - rebuilds `nu.dll` with Visual Studio 2022 MSVC
  - builds the sample with `C:\msys64\ucrt64\bin\g++.exe`
  - embeds the `nu` logo as the Windows executable icon

Build:

```powershell
cd D:\3D\API
powershell -ExecutionPolicy Bypass -File .\examples\cpp\build_minecraft_block.ps1
```

Toolchains used:

- Rust / `nu.dll`: Visual Studio 2022 MSVC via `vcvars64.bat`
- C++ / `minecraft_block.exe`: `C:\msys64\ucrt64\bin\g++.exe`

Run:

```powershell
cd D:\3D\API\examples\cpp
.\minecraft_block.exe
```

Expected output:

```txt
nu C++ OpenGL-syntax scratch sample
recorded commands: 28
minecraft-style block: grass top, dirt bottom, grass side atlas layout
vao=1 vbo=2 ebo=3 atlas=4
```

Important scope:

- This is a scratch rendering command surface, not the full engine host API yet.
- The C++ side is intentionally OpenGL-like.
- The Rust side still records the work into `nu`'s compatibility command model.
