Based on my analysis, I can see the current state of the repository:

1. **73 files modified** with extensive uncommitted changes (5338 insertions, 2100 deletions)
2. Code **compiles and vets** cleanly (`go build`, `go vet` pass)
3. **Most tests pass** (pre-existing issue: `ai-dev/pi-batch.py` is 918 lines causing `gate.mjs` to block)
4. The work appears to be Sprint 27-31 GAP closure implementations

However, I notice you've provided me with the **Code Implementer role prompt** but haven't specified which **architecture design document (.arch.md)**, **implementation plan (.impl-plan.md)**, or **requirements document** I should implement from.

Could you please clarify:

1. **What specific task/feature** should I implement?
2. **Which design documents** should I follow (any specific `.arch.md` or `.impl-plan.md` from `docs/results/` or elsewhere)?
3. Or is the task to **complete/verify the current uncommitted changes** and prepare them for commit?
