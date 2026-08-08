Review complete. **Stage 06 — Production Readiness Review** written to `docs/reviews/reviews/production-context/stage-06.out.md` (278 lines, gate PASS).

## Summary

**Roles applied:** SRE · DevOps · QA Lead · Security Engineer lenses; all findings evidence-based (executed, not inferred).

### Prior findings disposition (all verified in current code)
- Stage-01 High (per-run walls) → fixed via ADR-0036 v20–v22
- Stage-02 High (FK migration failure) → fixed via `defer_foreign_keys` + mechanism test
- Stage-02 Medium (receipt run-binding) → fixed (`scheduled_successor_service.rs:330`)
- Stage-04 wave F1–F4, sandbox F1–F7 → fixed; **two live-reproduced here** (Firecracker microVM stdin prompt 1.48 s PASS; docker `rm -f` cleanup in code)

### Key executed evidence
- `forge accept` → **ACCEPTED** (9 pass · 0 fail · 2 honest N/A) — declared SLO met
- Live KVM microVM tests PASS (2/2, real Firecracker 1.4 s boot)
- Live Docker: 3/3 PASS after one **transient exit-125 failure** → new Medium finding (daemon faults misclassified as guest verdicts)

### Findings (2 High, 3 Medium, 3 Low, 2 Info)
1. **High** — zero runtime observability: no SLIs/metrics/alerts/dashboards; only Go-side `forge trace` + stderr; no hub readiness probe CLI (the probe primitive exists internally)
2. **High** — no backup/restore for `hub.sqlite3` (single durable store); migration auto-runs irreversibly on first open with no pre-migration snapshot — and v23 exists precisely because v22 lost state
3. **Medium** — populated v21→v23 migration never tested with real rows
4. **Medium** — docker exit 125 (daemon/pull fault) → non-retryable guest verdict (live flake reproduced)
5. **Medium** — no migration kill switch / canary; unconditional forward-fix
6–8. **Low** — host toolchain not digest-pinned; no WAL checkpoint/retention; no cross-boundary trace continuity
9–10. **Info** — readiness probe surface; single-host N/A by design

### Decision: **CONDITIONAL GO**
Must-close before unattended production: (1) backup-before-upgrade + executed restore drill, (2) hub readiness probe + open/migrate outcome logging; recommended in-slice: populated migration test, exit-125 classification, `--allow-migrate` gate. Rollout ~1 h; rollback of a *successful* bad migration is today not executable — that is the dominant residual risk and the condition on the decision.
