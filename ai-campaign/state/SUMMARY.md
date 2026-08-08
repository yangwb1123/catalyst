# Campaign summary: repository-improvements

Generated: 2026-08-08T10:18:01+00:00

| Module | Direction | Status | Reason | Evidence |
|---|---|---|---|---|
| __tool__ |  | TOOL_VERSION | tool code digest (pbatch + pi-batch.py + pi-batch.yaml); invalidates analysis/direction reuse when the tool changes | c727e2ae4c32f553190ae001635112f81cd14233 |
| forge-core | Bound the gate/harness bridge subprocesses with context deadline, timeout, and output cap (reuse the orchestrator's bounded-run pattern) | PIPELINE_FAILED | implement | /home/u1/catalyst/ai-campaign/state/runs/bound-the-gate-harness-bridge-subprocesses-with--798f6166/pipeline.yaml, /home/u1/catalyst/ai-campaign/state/runs/bound-the-gate-harness-bridge-subprocesses-with--798f6166/artifacts/requirements-10762e10/requirements.md, /home/u1/catalyst/ai-campaign/state/runs/bound-the-gate-harness-bridge-subprocesses-with--798f6166/artifacts/design-a77de8a6/task-1-design.md, /home/u1/catalyst/ai-campaign/state/runs/bound-the-gate-harness-bridge-subprocesses-with--798f6166/artifacts/adversarial_review-9c87f3a7/meta/architecture_reviewer.md, /home/u1/catalyst/ai-campaign/state/runs/bound-the-gate-harness-bridge-subprocesses-with--798f6166/artifacts/adversarial_review-9c87f3a7/meta/testing_reviewer.md, /home/u1/catalyst/ai-campaign/state/runs/bound-the-gate-harness-bridge-subprocesses-with--798f6166/artifacts/adversarial_review-9c87f3a7/meta/concurrency_reviewer.md, /home/u1/catalyst/ai-campaign/state/runs/bound-the-gate-harness-bridge-subprocesses-with--798f6166/artifacts/adversarial_review-9c87f3a7/meta/security_engineer.md, /home/u1/catalyst/ai-campaign/state/runs/bound-the-gate-harness-bridge-subprocesses-with--798f6166/artifacts/adversarial_review-9c87f3a7/meta/ci_integration_reviewer.md, /home/u1/catalyst/ai-campaign/state/runs/bound-the-gate-harness-bridge-subprocesses-with--798f6166/artifacts/adversarial_review-9c87f3a7/meta/api_contract_reviewer.md, /home/u1/catalyst/ai-campaign/state/runs/bound-the-gate-harness-bridge-subprocesses-with--798f6166/artifacts/design_gate-6a76b0dd/task-1-design-gate.md |
| forge-core | Close the durability gap in statefs.AtomicWrite: fsync the parent directory after rename, as commitTrackedTemp already does | DIRECTION_REJECTED | rejected because the selection quota was already filled | /home/u1/catalyst/ai-campaign/state/analyses/forge-core-6b2cbb7d.json |
| forge-core | Make firecracker sandbox boot helper binaries context- and timeout-bound (mke2fs, debugfs, firecracker launch) | DIRECTION_REJECTED | rejected because the selection quota was already filled | /home/u1/catalyst/ai-campaign/state/analyses/forge-core-6b2cbb7d.json |

## Counts

- DIRECTION_REJECTED: 2
- PIPELINE_FAILED: 1
- TOOL_VERSION: 1
