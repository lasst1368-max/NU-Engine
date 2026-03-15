# Security Policy

## Supported Status

NU Engine is pre-`1.0.0` and still under active development.

Security fixes are handled on a best-effort basis during this stage. There is no formal SLA yet.

## Reporting a Vulnerability

Do not open a public issue for an unpatched security vulnerability.

For now, report security issues privately to:

- `lasst1368@gmail.com`

Include:

- affected commit or branch
- reproduction steps
- impact summary
- any proof-of-concept if safe to share

## Scope

Security reports are most useful for issues involving:

- asset loading
- file parsing
- editor/runtime filesystem access
- shader compilation and hot reload
- unsafe Vulkan or native-memory handling

## What to Expect

- acknowledgement when the report is received
- a best-effort triage
- a fix or mitigation if the issue is confirmed and practical to address in the current stage

Pre-`1.0.0`, fixes may land directly on `main` without a separate security release process.
