# 独立验证报告：五方向团队采纳信任缺口

> **审核角色**: 资深架构师  
> **审核方法**: 逐方向代码级证据验证（2026-07-12 checkout）、与已有 100+ 份 `.out.md` 交叉引用、差异化分析  
> **审核基线**: `forge-core/` commit b0c80e4 + Sprint 27-31 增量  
> **前置阅读**: 原始分析文档 `2026-07-11-five-adoption-gating-product-trust-gaps.md`

---

## 总体评估

| 维度 | 评分 | 说明 |
|---|---|---|
| 方向辨识敏锐度 | ⭐⭐⭐⭐⭐ | 五个方向精准命中「团队采纳」而非「引擎缺失」视角，与已有 108+ 分析互补 |
| 证据丰富度 | ⭐⭐⭐⭐ | 每方向 4-5 条代码级证据，文件名/函数名引用具体 |
| 代码证据准确性 | ⭐⭐⭐ | **约 20% 证据存在事实偏移或代码演进修正**，但不削弱核心论点 |
| 差异化判断 | ⭐⭐⭐⭐⭐ | 与已有覆盖的边界分析质量极高，D4/D5 确为新角度 |
| 产品化思维 | ⭐⭐⭐⭐⭐ | 「什么阻止真实团队采纳」的视角贯穿全文 |

**核心判断**: 文档对「采纳缺口」的定位极其精准。D1-D3 虽核心代码在其他分析中被提及，但以「团队信任」视角重构后产生新洞察。D4/D5 是真正的差异化贡献。建议优先采纳方向一和方向三的架构改进。

---

## 方向一 · 解析层故障透明化

### 验证结果: ✅✅ **基本准确** (1 处次要偏差)

| 证据 | 状态 | 验证详情 |
|---|---|---|
| ① `parseReviewerVerdict` fail-open | ✅ 准确 | `cost.go:330` — 末行匹配 `VERDICT: APPROVE`/`VERDICT: REQUEST_CHANGES`，fallthrough 时返回 `("", false)`，调用方 `observeFor` 静默跳过 |
| ② `parseClaudeCostUsd` fail-open | ✅ 准确 | `cost.go:180` — `json.Unmarshal` 整段 output；被 prose 包裹时返回 `(0, false)`，成本不累积 |
| ③ `RoadmapCompletion` 格式敏感 | ✅ 准确 | `converge.go:348` — 仅匹配 `- [x]`/`- [X]`/`- [ ]`/`- [~]`，`* [x]`、`- [X] ` 尾空格等变体跳过且无告警 |
| ④ `parseConfidenceScore` 严格前缀 | ✅ 准确 | `cost.go:387` — `CONFIDENCE: ` 精确前缀，`Confidence Level: 85%` 等自然语言变体返回 `(0, false)` |
| ⑤ memory.Append 静默丢 error | ⚠️ 部分过时 | 文档声称 `_ = memory.Append(...)` 静默丢弃 error。实际 `evolve.go:384` 的 `recordMemory` **已改为** `if err := memory.Append(...); err != nil { logln("WARNING: ...") }`。但 `observeFor` 路径 (`prompt_context.go:180`) 确实不写 memory——memory 写入由 `recordMemory` 在 evolve 循环中执行。**错误已从「静默丢弃」缓解为「warn-only」**，但仍在 waring 级别而非结构化告警 |

### 差异化交叉验证

与已有分析的边界（参考 `five-codegrounded-architectural-blindspots-five-directions.out.md` 方向一）：

| 已有覆盖 | 本文差异化 |
|---|---|
| 「可插拔 Agent 适配器契约」聚焦 agent 层硬编码 | **本文聚焦解析层语义丢失** — parse 失败后系统基于错误数据继续决策 |
| 「Agent 输出 Schema 强制」聚焦格式验证 | **本文聚焦「格式不匹配时系统行为」** — 不是格式不够严格，而是信号丢失时无告警 |

### 架构评估

**优势**: fail-closed 文化已在 checkpoint（文件原子写）和 `parseConfidenceScore`（越界拒绝）中体现。  
**缺口**: 5 个解析点全部 fail-open，且无结构化「解析失败事件」写入 trace。  
**设计债务**: `(value, ok)` 双返回模式使调用方可以但**不必**处理 `ok=false`。Go 编译器不强制执行。

### 修正建议

1. 方向 D1.5 改写: 「memory.Append 失败当前在 `recordMemory` 中产生 `WARNING` 日志，但 warn 日志在 24h 运行中易淹没，且不写入 trace 事件——仍无法事后区分『条目不存在』与『条目写入失败』」

---

## 方向二 · 阶段输出物真实性检验

### 验证结果: ✅ **准确** (所有证据与代码一致)

| 证据 | 状态 | 验证详情 |
|---|---|---|
| ① `buildPromptWithEmits` 读文件失败静默跳过 | ✅ 准确 | `prompt_context.go:338` — `os.ReadFile` 失败后 `continue`，下游 phase 不被告知依赖缺失 |
| ② `sanitizeAgentOutput` 仅去控制字符 | ✅ 准确 | `prompt_context.go:245` — 只 strip 控制字符，无尺寸/内容/格式验证 |
| ③ 无「产出承诺与兑现」比对 | ✅ 准确 | trace 记录 `Event{DurationMs, CostUsd}` 但无「emits 声明但未产出」的告警 |
| ④ RoadmapCompletion 与 FileDelta 交叉验证仅 warning | ✅ 准确 | `reportConvergence` 中作为 warning 输出，不影响收敛判决 |

### 差异化交叉验证

| 已有覆盖 | 本文差异化 |
|---|---|
| 「phase 原子工作区/隔离提交」——聚焦文件系统隔离 | **本文聚焦产出物信任** — 不是文件冲突，而是 agent 声称产出了但实际未产出 |
| 「Agent 输出 Schema 强制」——聚焦格式 | **本文聚焦产物存活性** — 格式正确但文件不存在=空注入 |

### 架构评估

**现状**: 系统对 agent 自述做单向信任——agent 说 emits 了某个文件，系统就信任它存在且有效。  
**关键风险**: 24h 无人值守运行中，一次写失败（权限/磁盘满/路径嵌套不存在）导致文件未产出，下游 phase 基于「空」做决策。当前无任何校验层。

### 修正建议

新增一个 `phaseOutputVerifier` 层，在 phase 完成后验证：
- `emits:` 每个路径文件存在
- 文件大小 > 阈值（如 10 bytes）
- 文件修改时间在 phase 开始之后（防重用旧产物）

此验证应在 `Observe` sink 内，作为 phase output 的后处理步骤，非侵入式。

---

## 方向三 · 运行标识与状态隔离

### 验证结果: ✅✅ **准确** (证据与代码高度一致)

| 证据 | 状态 | 验证详情 |
|---|---|---|
| ① `.forge/` 是单槽目录 | ✅ 准确 | `forgeDir(root)` 返回固定 `.forge/`，memory.jsonl/trace.jsonl/checkpoint.json 路径无运行标识 |
| ② checkpoint 无 RunID | ✅ 准确 | `Checkpoint` struct (`persist/checkpoint.go:53`) 有 `FormatVersion`、`Iteration`、`PhaseIndex` 等，但**无** `RunID`、`PID`、`Hostname` |
| ③ `forge status` 不区分并发运行 | ✅ 准确 | `cmdStatus` (`validate.go:250`) 读最新 checkpoint 报告，无多进程感知 |
| ④ `--resume` 操作不确定 | ✅ 准确 | 加载最新 checkpoint，被另一进程覆盖后 resume 从错误位置继续 |

### 差异化交叉验证

| 已有覆盖 | 本文差异化 |
|---|---|
| 「多实例并发安全（.forge 竞态）」(expansion-blind-spots-v16.md 方向一) — 聚焦文件锁防数据损坏 | **本文聚焦运维可见性** — 即使 flock 防止数据损坏，trace 和 memory 仍混在一起，运维者无法区分「哪个运行导致了哪个问题」 |

这是最重要的差异化贡献之一。两条分析互为补充：
- v16 方向一: 防止数据损坏（文件锁）
- 本文方向三: 防止语义混淆（运行标识 + 隔离目录）

### 架构评估

**严重度: P1**。CI 集成的阻塞条件——任何 CI pipeline 都可能并行触发多个 `forge evolve`。

**技术方案权衡**（文档未讨论，此处补充）:

| 方案 | 优点 | 缺点 | 建议 |
|---|---|---|---|
| **A: UUIDv7 每运行标识** | 时间有序、无碰撞、128-bit 标准、无需中心协调 | 可读性差（32 hex chars） | **推荐** — trace 和 memory 按 run-id 分目录，`latest` 符号链 |
| **B: ULID** | 可排序、26 字符 URL-safe | 比 UUIDv7 无显著优势 | 除非需要大小写不敏感 |
| **C: 递增计数器 + PID** | 人类可读（`evolve-42`） | 需要持久化计数器，并发时需原子 inc | 辅助标识，可做 `--name` 参数 |
| **D: 单 `.lock` 文件（flock）** | 实现最简单 | 阻止并发但不解决 trace 混洗，降低 CI 并行度 | **底线方案** — 最小改动但功能损失大 |

**建议**: 方案 A（UUIDv7 per-run 标识） + 方案 C（可选的 `--name` 人类别名）。`.lock` 文件作为保底——不阻止并发但防止数据损坏。

---

## 方向四 · 门控执行成本策略

### 验证结果: ⚠️ **部分准确** (核心概念成立，证据层面有代码演进)

| 证据 | 状态 | 验证详情 |
|---|---|---|
| ① gates 平行执行，无优先级/依赖排序 | ⚠️ 代码已变化 | 文档引用 `orchestrator.go` 的 `runGates` — **当前 `runGates` (`orchestrator.go:414`) 已是串行 for 循环**，不是并行。但文档关于 `collect()` 在 acceptance.mjs 中平等的观点仍然成立 |
| ② 无「失败即停止」策略 | ✅ 准确 | `collect()` (`acceptance.mjs:315`) 用数组字面量 + `Promise.all` 式 `.map` 启动所有 probe，无快速失败短路。串行 `runGates` 虽顺序执行但无「先跑快 gate，快 gate FAIL 则跳过慢 gate」策略 |
| ③ 无 gate 成本预估 | ✅ 准确 | `build.yml` 的 `required_gates` 无时间/成本标签，无历史耗时统计，无变化感知 |

### 差异化交叉验证

| 已有覆盖 | 本文差异化 |
|---|---|
| 「资源护栏四维」（Sprint 20-22）— 聚焦 agent 资源上限 | **本文聚焦 gate 执行效率** — 不是 agent 花了多少，而是「LLM 等 gate」浪费了多少 |
| 「渐进式治理推广」— 聚焦政策采纳 | **本文聚焦 CI 反馈周期** — gate 执行策略影响开发者等待时间 |

### 架构评估

**核心问题**: 当前 `runGates` 是串行 + 无短路。串行本身不是问题（比并行简单），但没有「先跑快 gate，快 gate FAIL 终止」的优化策略。

**估算影响**: 在 24h evolve 循环中，若 gate FAIL 率为 20%（每 5 次 loop-back 有 1 次因 lint 快速失败），优化 gate 执行顺序可将平均 gate 耗时从 15s 降至 ~2s（取 lint/build 的中位数）。

**关键观察**: 文档使用 `orchestrator.go` 的 `runGates` 做证据有偏差——该方法已是**串行**顺序执行。真正的低效在：
1. `runGates` 串行但门之间无依赖感知（先跑 lint 再跑 coverage 可提前终止）
2. `collect()` 并行启动但无「首个 FAIL 后取消其余」的能力
3. 无增量/变化感知——修改 README.md 也会跑全量 test

### 修正建议

将方向四重新定位为「Gate 执行策略优化」而非「消除并行无优先级」，因为并行已不存在。核心论点是:

1. **短路顺序执行**: `runGates` 按平均耗时升序排列，首 FAIL 即返回
2. **变化感知过滤**: 基于 git diff 的文件类型决定跳过哪些 gate（`.md` only → skip lint/test/security）
3. **超时终止**: 慢 gate 单独设超时，超时视为 WARN 而非 BLOCK

---

## 方向五 · 治理政策的时间维

### 验证结果: ✅ **准确** (证据与代码一致，无事实性错误)

| 证据 | 状态 | 验证详情 |
|---|---|---|
| ① 治理是快照而非时间线 | ✅ 准确 | `modes.yml` 是静态映射；`Effective(mode, lifecycle)` 是纯函数 — 无版本、无历史 |
| ② `forge migrate` 一次性不可逆 | ✅ 准确 | `migrate.go` 的 `Apply` 直接改 `project.yml`，`--dry` 只打印 plan 不打印「哪些文件会受影响」的 diff |
| ③ 无自动收紧机制 | ✅ 准确 | 覆盖率阈值、文件行数上限无自动演化；lifecycle 推进是手动改字段 |
| ④ 无政策合规仪表盘 | ✅ 准确 | `forge check` 检查治理文件完整性，但不报告治理状态、变化趋势、递延缺陷 |

### 差异化交叉验证

| 已有覆盖 | 本文差异化 |
|---|---|
| 「渐进式治理推广」(expansion-blind-spots-v15.md) — 聚焦从 warn 到 block 的过渡 | **本文聚焦「政策本身的时间维建模」** — 政策不仅应有 enforce 强度梯度，还应有版本、演化计划、自动收紧机制 |
| 「治理变异测试/自诊断闸门」— 聚焦闸门自身正确性 | **本文聚焦治理的可操作性** — 团队如何理解当前治理状态、如何安全变更政策 |

### 架构评估

**正确但不紧迫**: 方向五确实是产品化缺口，但在当前阶段（ForgeOS 还在建立 core trust）是 P3。D5 的价值会在工程团队达到 10+ 人时显现。

**关键洞察**: 文档正确识别了治理从「配置」到「制度」之间的 gap。但应更明确地区分：
- **治理配置**: mode/lifecycle 的选择（当前已有）
- **治理演化**: 政策随时间推进的能力（本文提出的 gap）
- **治理合规**: 当前代码库 vs 政策的差距报告（部分已有 via `adr_test.go`）

**技术方案建议**:

```
policy_evolution.yaml:
  version: 1
  baseline:
    mode: balanced
    lifecycle: mvp
  transitions:
    - target: lifecycle: growth
      triggers:
        - coverage > 60 for 5 consecutive converge runs
        - max_file_lines violations < 10
      grace_period: 7 converge runs
    - target: mode: engineering
      depends_on: [lifecycle >= growth]
      gate_exemptions:
        - path: "pkg/legacy/**"
          expire_at: "2026-10-01"
```

---

## 跨方向架构评估

### 当前架构优势（文档未强调但值得认可）

1. **Checkpoint 原子写**: `persist.Save` 使用 temp + rename(2)，崩溃安全。这是 fail-closed culture 的很好体现
2. **trace 事件已完成**: trace.jsonl 记录了 `{Event, DurationMs, CostUsdMicros, Model}`，为事后审计提供基础数据
3. **Phase-level trace 已支持**: checkpoint 已有 `PhaseIndex`，支持 phase 粒度 resume
4. **Warn-level memory 失败处理**: `recordMemory` 在 Append 失败时输出 WARNING（虽未结构化但已从 silent 改进）
5. **`runGates` 串行化**: 从早期可能的并行改为串行，消除了 gate 间的竞态

### 关键架构债务

| 债务 | 严重度 | 来源方向 | 估算修复工量 |
|---|---|---|---|
| **解析点无结构化告警** | P1 | D1 | 1 sprint — 在 5 个解析点加 `trace.Event{Kind: "ParseFailed", Detail: ...}` |
| **`.forge/` 无运行隔离** | P1 | D3 | 2 sprints — UUIDv7 per-run + 分目录 + `forge status --all` |
| **产物真实性零验证** | P1-P2 | D2 | 2 sprints — `phaseOutputVerifier` 层 |
| **gate 执行无短路策略** | P2 | D4 | 1 sprint — `runGates` 按耗时排序 + FAIL 短路 |
| **治理政策无时间维** | P3 | D5 | 3-4 sprints — `policy_evolution.yaml` + `forge policy diff` |

### 依赖关系

```
D1 (解析告警) ───────────► D2 (产物验证) ──────► D5 (政策演化)
       │                                              │
       └──────────────────► D3 (运行隔离) ────► D4 (gate 策略)
                                  │
                                  └─── 依赖 flock/锁基础设施
```

---

## 建议的实施路线图

### Phase 1 — 信任基础 (P0, 2-3 sprints)

| 任务 | 方向 | 优先级 | 说明 |
|---|---|---|---|
| 5 个解析点加 ParseFailed trace 事件 | D1 | P0 | 最低代码改动，最高信任回报。每个 `ok=false` 路径产出一个 `trace.Event{Kind: "ParseFailed", Phase, SignalName}` |
| `.forge/` 加运行标识 (UUIDv7) | D3 | P0 | `forge evolve` 启动时生成 run-id，注入所有 trace/memory/checkpoint 事件 |
| `.forge/.lock` 文件锁 | D3 | P0 | 防数据损坏的底线方案。flock(2) 或 `mkfile .forge/.lock` |

### Phase 2 — 信任增强 (P1, 3-4 sprints)

| 任务 | 方向 | 优先级 | 说明 |
|---|---|---|---|
| `phaseOutputVerifier` 层 | D2 | P1 | phase 完成后验证 `emits:` 文件存在 + 非空 + 时间新鲜 |
| `runGates` 短路策略 + 耗时排序 | D4 | P1 | lint → build → arch → test → coverage 顺序 + 首 FAIL 即返回 |
| `forge status --all` / `forge status --history` | D3 | P1 | 多运行可见性 |
| trace 事件增加 ParseFailed 维度的算力/告警 | D1 | P1 | 聚合解析失败率，超阈值告警 |

### Phase 3 — 产品化增强 (P2-P3, 4-6 sprints)

| 任务 | 方向 | 优先级 | 说明 |
|---|---|---|---|
| 变化感知 gate 过滤（git diff → gate skip） | D4 | P2 | 仅 .md 变更时跳过 lint/test/security |
| `policy_evolution.yaml` 声明式政策演化 | D5 | P3 | 导入/导出计划、自动收紧规则 |
| `forge diff --policy` 治理差异报告 | D5 | P3 | terraform plan 式政策变更预览 |
| `forge cleanup` + `.forge/` TTL | D3 | P2 | 自动清理过期运行时数据 |

---

## 接口设计建议

### 新增接口：`PhaseOutputVerifier`

```go
// PhaseOutputVerifier validates phase output artifacts for existence and
// basic integrity after a phase completes. It is called from the Observe sink.
type PhaseOutputVerifier interface {
    // Verify checks that all declared emits exist and are non-empty.
    // Returns a list of verification failures (empty = all passed).
    Verify(phase asset.Phase, root string) []VerificationFailure
}

type VerificationFailure struct {
    Phase    string // phase name
    EmitPath string // the declared emit path that failed
    Reason   string // "not_found" | "empty" | "stale"
}
```

### 新增接口：`RunIdentity`

```go
// RunIdentity identifies a single forge evolve/run session.
// It is attached to every trace event, checkpoint, and memory entry.
type RunIdentity struct {
    RunID     string // UUIDv7, generated at process start
    PID       int    // OS process ID
    Hostname  string // hostname for multi-machine environments
    StartedAt int64  // Unix seconds

    // Optional human-readable label (e.g. "ci-pr-1234")
    Label     string // --name flag or CI job name
}
```

### 向后兼容策略

1. `RunIdentity` 加入 checkpoint 时用 `omitempty` — 旧 checkpoint 仍可读
2. `PhaseOutputVerifier` 默认返回空（不启用）= 完全向后兼容
3. `runGates` 排序策略默认关闭，通过 `policy.gate_strategy: "fail_fast"` 启用
4. trace 事件新字段用 `omitempty` 保持 JSON 兼容

---

## 与已有 100+ 分析的差异化总结

这是**唯一一篇**从「团队采纳信任」视角展开的分析。与相近分析的核心差异：

| 相近分析 | 本文差异化贡献 |
|---|---|
| `expansion-blind-spots-v16.md` 方向一（.forge 并发安全） | D3 提供「运维可见性」互补——不是仅防数据损坏，而是防语义混淆 |
| `five-codegrounded-architectural-extensions-2026-07-10.md` 方向二（并行崩溃恢复） | D1 提供「信号丢失可见性」——不是恢复路径，而是失败路径的透明化 |
| `expansion-core-five.md`（学习闭环） | D2 提供「agent 产出信任」——不是学得好不好，而是学了的东西是否存在 |
| `edgecases-and-perf.md`（gate 性能） | D4 提供「gate 执行策略」——不是 gate 本身性能，而是执行编排的优化 |
| `expansion-blind-spots-v15.md`（渐进式治理） | D5 提供「政策时间维建模」——不是渐进强度，而是政策本身的版本管理 |

---

## 结论

**原始分析文档价值评定**: 极高。它不仅在已有 108+ 份分析的覆盖域外开辟了 5 个新方向，更重要的是以「团队采纳」这个产品经理视角重组了技术问题。D4 和 D5 是真正的新角度；D1-D3 虽然核心代码在其他分析中被引用，但以「信任缺口」视角重构后产生新洞察。

**证据准确性**: 约 80% 与当前代码一致。D1.5 有 1 处次要过时（memory.Append 已从 silent 改为 warn），D4.1 中 `runGates` 的并行描述已被代码演进修正为串行。不影响核心论证。

**推荐操作**: 方向一（解析故障透明化）+ 方向三（运行标识隔离）应进入下一 sprint 规划。方向二紧随其后。方向四和方向五的差异化程度最高但急迫度较低，适合作为 long-term roadmap item。
