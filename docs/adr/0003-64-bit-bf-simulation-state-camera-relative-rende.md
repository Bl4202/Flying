# ADR-03: 64-bit BF simulation state, camera-relative rendering

- **Status:** Accepted
- **Owner:** Bl4202
- **Source:** GDD §0.4 (imported verbatim at v0.0a)
- **Affects:** §0.3, §44, §33

## Decision

Sim state is stored in **64-bit float BF**; rendering is camera-relative 32-bit float.

## Alternatives considered

Floating-origin world shifting

## Rationale

Shifting breaks networking, replays, and long-range sensor maths.
