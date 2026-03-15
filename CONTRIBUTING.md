# Contributing to NU Engine

NU is still pre-`1.0.0`. That means the engine is intentionally moving fast, APIs are unstable,
and contribution quality matters more than contribution volume.

## Before You Start

- read [LICENSE](LICENSE)
- read [README.md](README.md)
- check the current roadmap in [ROADMAP.md](ROADMAP.md)

## Current Contribution Rules

During the pre-release stage:

- source is visible for learning and contribution
- shipping Products with NU is not licensed yet
- contributions should be submitted as public pull requests
- core changes should not be kept as a private fork if they are intended for real use beyond
  evaluation or learning

## What We Want

- renderer/runtime correctness fixes
- editor usability improvements
- physics and asset-pipeline work
- clear tests for engine-facing behavior
- focused documentation updates

## What Makes a Good PR

- one coherent change
- compiles cleanly
- includes tests where practical
- does not mix unrelated refactors
- explains the reason for the change, not just the change itself

## Style

- keep code direct and explicit
- prefer simple data flow over clever abstractions
- preserve the existing engine direction unless you are intentionally proposing a change in
  architecture

## Large Changes

For large changes, open an issue or draft PR first if the direction is not already obvious from the
roadmap.

Examples:

- new asset pipeline formats
- renderer architecture changes
- licensing/process changes
- editor interaction model changes

## Testing

Use the VS 2022 toolchain environment on Windows if your default MSVC setup is broken.

Typical validation path:

```powershell
cmd /c 'call "C:\Program Files (x86)\Microsoft Visual Studio\2022\BuildTools\VC\Auxiliary\Build\vcvars64.bat" && cd /d D:\3D\API && cargo fmt --all && cargo test --lib'
```

## Review and Merge

- contributor keeps credit
- maintainers keep final merge rights
- accepted code lands under the same license as the rest of the project

For now, the primary maintainer is:

- Mannu
