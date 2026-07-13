# Scope: Milestone M5_facet (Facet Audit)

## Architecture
- `facet` is a windowing, compositor, rasterization, and UI framework written in Salt.
- Modules:
  - `window`: Window creation, OS event handling, and input mapping.
  - `gpu`: GPU abstraction, pipeline setup, shader management, buffer allocation.
  - `raster`: Vector and raster drawing routines, path rasterization, text rendering.
  - `compositor`: Multi-window management, layering, texture composition.
  - `ui`: UI widget trees, layout engines, event propagation.

## Milestones
| # | Name | Scope | Dependencies | Status |
|---|------|-------|-------------|--------|
| 1 | M5_facet | Audit all proprietary code/docs in /Users/kevin/projects/facet to remove AI slop, hyperbole, and legacy artifacts | none | IN_PROGRESS |

## Interface Contracts
- Standard Salt conventions and repository boundaries apply.
- Cleanups must not break public APIs or module interfaces defined within the codebase.
