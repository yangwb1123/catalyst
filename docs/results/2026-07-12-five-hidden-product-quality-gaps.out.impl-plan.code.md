I've received the **Code Implementer role prompt**. I understand my responsibilities: transform technical designs into production-quality code, following ForgeOS conventions (Go zero-deps for forge-core, Node/Python zero external deps for harness).

However, I still need the **specific inputs** to begin implementation:

1. 📐 **Architecture Design Document** (`.arch.md`) — describing the system/feature design
2. 📋 **Implementation Plan** (`.impl-plan.md`) — step-by-step tasks
3. 📝 **Original Requirements** — the feature/change description

From scanning the workspace, I see there are many `docs/results/*.arch.md` and `docs/results/*.impl-plan.md` files from previous runs. Which one should I implement? Please specify:

- **Which design set** to implement (e.g., a specific `.arch.md` + `.impl-plan.md` pair)
- **What to build** — specific module, feature, or fix
- **Any additional constraints** — performance targets, compatibility requirements, etc.

Once you provide these, I'll produce the code with:
- Full error handling and edge case coverage
- Tests as appropriate
- Gate verification (`forge accept` / `go build` / `go vet`)
- A structured implementation report
