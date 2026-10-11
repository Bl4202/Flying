## What and why

## Checklist

- [ ] `pnpm lint` and `pnpm test` pass locally
- [ ] Sim-core or protocol change: `pnpm determinism` passes (golden re-blessed only with a reason below)
- [ ] No `sin/cos/exp/pow` etc. outside `sim-core::dmath`; no `HashMap` iteration in sim crates
- [ ] New dependency: licence is on the allow-list, justified here
- [ ] Architectural decision: ADR added or updated in `docs/adr/`

## Golden re-bless reason (if any)
