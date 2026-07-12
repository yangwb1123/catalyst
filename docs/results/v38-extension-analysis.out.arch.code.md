I understand the Code Implementer role and am ready to produce production-quality code. However, I need you to specify which design document and implementation plan to implement.

From today's batch, I see several complete `.arch.md` + `.impl-plan.md` pairs ready for implementation:

| # | Design | Tasks | Priority |
|---|--------|-------|----------|
| 1 | **Execution Semantics Gap Analysis** (方向一~五) | Phase副作用合约, 统一错误类型, Agent输出弹性解析, 格式版本化, Trace因果关系 | P0-P3 |
| 2 | **Five Unseen Systemic Operational Gaps** (方向一~五) | dry-run改进, 预算降级螺旋, 自DoS防护, 环境安全, 跨存储一致性 | P0-P2 |
| 3 | **Five Uncovered Structural Extensions** (方向一~五) | 声明-实现漂移检测, 语义日志, 意图一致性, 产物Schema强制, Core内部遥测 | P1-P2 |
| 4 | **Five Hidden Product Quality Gaps** | (design doc exists) | — |
| 5 | **High-Value Extension Directions** | (design doc exists) | — |

**Please tell me which design/implementation plan to implement**, or if you'd like me to implement a specific direction (e.g., "方向四: 环境安全" from document #2) across all of them.

Once you specify, I'll:
1. Read the relevant `.arch.md` and `.impl-plan.md` in full
2. Study the existing codebase structure
3. Implement all tasks with proper error handling, tests, and gate verification
4. Run `forge accept` to verify compliance
