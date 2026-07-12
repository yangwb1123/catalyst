这份分析文档非常扎实。我仔细阅读了全部五个方向，以下是我的反馈：

---

## 总体评价

「从『自己能跑』到『让别人能跑、敢跑、规模化跑』」——这个矛盾判断精准。Sprint 1–31 的演化轨迹是 **infrastructure proving**，下阶段的本质是 **platform packaging**。五个方向的选择均来自代码库已铺路但未激活的管道，不是空中楼阁。

---

## 对每个方向的深入回应

### 方向一：Event Gateway

**亮点判断**——`external` stop 类型和 `triggers` 字段已声明但零消费，这个事实我交叉验证过（`evolve.yml` schema 中 `stop_condition.type` 的枚举值包含 `external`，但 `internal/engine/loop.go` 的 `shouldStop` 从未对 `external` 做 switch-case）。这是「已布线但没通电」的典型架构债务。

**一个需要澄清的技术细节**：`forge run --daemon` 的守护进程模式和 `gateway` 的事件接收是两个正交维度。守护进程意味着 `forge-core` 需要处理 `SIGHUP`/`SIGTERM` → 优雅关闭，当前 `orchestrator.go` 的 `Ctx` 是 `context.Background()`，没有 signal handler。建议在文档中明确标识这两个维度的依赖关系——先做 webhook endpoint（被动接收），再做 daemon（主动常驻），后者依赖前者的路由。

**想补充的一个 edge case**：webhook 的去重。GitHub webhook 在失败时有 retry 语义（最多 2 次），同一个 `push` event 可能到达多次。如果 `gateway` 不做 dedup，一次 PR commit 可能触发两次 `forge run`。这在当前单例模式下会导致方向无关第 1 点（多版本并行冲突）。**建议将 dedup 列为 gateway 的内建契约**（通过 event ID + 最近处理窗口的 LRU set）。

### 方向二：知识引擎

**最强洞察**——「`Append` 调用者在代码中极少」。我翻阅了 `memory` 包的全部 callers：
- `evolve.go:Append` — 1 处
- `store.jsonl` — 只有 `WriteKnowledge` 操作
- `Query` — 只在 `retrieve.go` 中被调用（prompt assembly 时）

这意味着 memory 当前的附加值远低于其基础设施的成熟度。Learning loop 产生了 `trace.jsonl`（含 `scorecard`）和 `memory_compact.go` 的 `Prune` 逻辑，但中间的提炼管道是空的。

**一个可能的采纳路径**（比全文案更轻量）：先做 **1 个 `harvest.go` 的最小可行**——把 `trace[iteration-1].scorecard.quality_gate` 如果为 FAIL，自动 Append 一条 `kind=lesson` 的知识："[iteration N-1] 方案因 {scorecard.reason} 被 gate 拦截"。只需 20 行代码，就能让下一轮迭代看到上一轮失败的 `memory` 检索结果。这个信号增益极高，实现成本几乎为零。

**同意 embedding deferred**。`internal/prompt/retrieve.go` 的 TF-IDF 对于当前知识规模（<1000 条目）完全够用。当条目量级达到 10^4 时再考虑向量化不迟。

### 方向三：多仓治理

**优先级判断认同**——P0 是正确的。ForgeOS 如果只治理 `forge-core` 自己，本质上不是「OS」而是「单项目脚手架」。ADR 0003 的 `extends` 字段空置一天，架构债务就累积一天。

但我想对 **Phase A 的实现风险** 做一个提醒：

`project.yml extends: [agent-os]` 的解析器改造涉及 `harness/scaffold/forge-init.mjs` 和 `forge-upgrade.mjs` 两条路径。当前 `forge-init` 的架构是：读 `project.yml` → 按白名单复制。如果引入 `extends`，则变为：读 `project.yml` → 解析 `extends` → 从 `.forgeos/` submodule 读取共享资产 → 按 `白名单 + 本地覆盖层` 合并 → 落地。

**中间状态（Phase A 未完成时）最危险**——`extends` 字段存在但不被解析，是静默破坏。`agent.reject` 中 `acceptance.mjs` 的 `manifest-integrity` 检查需要同步扩展，否则 `forge accept` 会通过一个不完整的治理布局。**建议 Phase A 的 first commit 就更新 `manifest-integrity` 检查**，使其在 `extends` 非空但 `.forgeos/` 不存在时直接 REJECTED。

### 方向四：预算治理引擎

**最重要的贡献**——把预算从「CLI flag（开发时设置）」提升为「policy 文件（签入到 `.agent/` 的治理资产）」。这是组织级信任所必需的。

我对 YAML schema 设计有一个**具体建议**：

```
protect:
  - when: task_type == "security"
    then: force_tier: opus
    cost: no_downgrade
```

这里 `when` 的条件表达式引擎是 new code。当前 `mode.Policy` 的 `gates` 和 `enforce` 用的是 Go 结构体字段匹配（`map[string]any`），没有表达式 DSL。如果要保持「不引入新语言」的承诺，建议**复用 `task_type` 的现有分类**（`modes.yml` 中 `Role` 的 `task_type: [discover, design, review, implement]` 四枚举），使 `when` 的条件限制为 `task_type in [set]` 匹配，不需要图灵完备表达式。

即：

```yaml
protect:
  - task_types: [security, payment]
    force_tier: opus
    no_downgrade: true
```

这与 `mode.Policy` 的已有模式一致（enum 匹配，不是任意表达式）。

### 方向五：可观测性

**P2 优先级判断同意**——没有这个依旧能跑。但我想补充一个数据点：

当前 `trace.jsonl` 缺少 span 父子关系，导致一个场景无法回答：*「evolve 跑了 8 小时，撞 budget cap 了，中间哪个 gate FAIL 引起了 loop-back，导致那轮 iteration 跑了 3 次而不是 1 次？」* 这是 debug 事故时的真实问题，不是镀金。`forge stats` CLI 子命令如果能输出一个折叠的 iteration 摘要树（类似 `git log --oneline --graph`），对 operator 的日常监控增益巨大。

**同意不做实时仪表盘**。CLI + `/metrics` 端点 + JSONL trace 三件套是当前阶段的最佳交付物。Prometheus 生态的 `textfile collector` 可以让 operator 用已有工具链消费 metrics。

---

## 跨领域 Edge Cases 的补充

### Edge Case 1：多版本并行运行冲突

文件锁的引入比看起来更微妙。`internal/persist` 当前写入三种文件：
- `trace.jsonl`（append-only，并发安全天然 ok）
- `memory.jsonl`（append-only，同上）
- `checkpoint.gob`（overwrite，**不安全**）

如果两轮 evolve 同时 `saveCheckpoint`，后写入的会覆盖前一个，导致不一致状态。**建议先只锁 checkpoint 文件**，trace/memory 的 append 操作通过 OS 的 `O_APPEND` 原子性已有保障。最小改动：引入 `internal/persist/flock.go`，在 `saveCheckpoint` 前后用 `flock` advisory lock 包裹。

### Edge Case 3：gate 网络分区

这个我特别想强调，因为触及了 `check.py` 和 `sca.mjs` 的外部依赖契约。当前 `gate.mjs` 和 `arch-check.mjs` 是纯本地执行，但 `check.py`（治理完整性）会 pull OSV DB。如果 OSV 不可达，`sca.mjs` 不会抛异常，而是返回空库结果（零漏洞），**静默跳过安全扫描**。这是一个安全风险。

**建议的 degrade 契约**：所有 gate 的 `run()` 函数返回一个 `{status: 'pass'|'fail'|'error', degraded?: boolean, reason?: string}`。如果依赖不可达，gate 应返回 `{status: 'error', degraded: true, reason: 'OSV DB unreachable'}`，而不是 `pass`。`acceptance.mjs` 在聚合时对 `degraded: true` 的 gate 发出 warning 但不阻断流程（与当前 N/A 策略一致，但信息更丰富）。

### Edge Case 5：gob 兼容性

`encoding/gob` 的兼容性保证在 Go 1.x 范围内通常是稳定的，但有已知陷阱：注册顺序变化、未导出字段的零值变化。`persist.Checkpoint` 结构体中包含 `map[string]*PhaseResult`，如果后续 sprint 给 `PhaseResult` 加了新字段，旧 gob 反序列化会忽略新字段（兼容）但新序列化后旧二进制可能读不到。

**务实建议**：不需要立即重构为 JSON（JSON 有精度问题：`cost_usd_micros int64` 在 JSON 中是 number，可能丢精度）。在 `Checkpoint` 结构中增加一个 `Version int` 字段，`save` 时写入当前版本号，`load` 时做版本兼容性检查。这是最小代价的 forward-compat 守卫。

---

## 总结

| 我的评估 | 方向 |
|---|---|
| ✅ 最强分析亮点 | `external` 零消费、`Append` 仅 1 处调用、`extends` 字段空置 |
| ✅ 优先级排序 | P0→P5 的 ordering 合理 |
| ⚠️ 需要补充 | 方向一的 dedup edge case、方向四的条件表达式简化方案、Edge Case 3 的 degrade 契约设计 |
| ⚠️ 实施风险提示 | 方向三 Phase A 的 manifest-integrity 检查需要先于 `extends` 解析器改造 |

好文档——每一条建议都从代码快照出发，引用具体文件/函数/行，不泛泛而谈。值得作为下一阶段 roadmap 讨论的 baseline。
