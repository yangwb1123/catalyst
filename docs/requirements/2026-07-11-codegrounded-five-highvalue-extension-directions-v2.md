# ForgeOS 下一阶段扩展方向：代码级诊断与高价值路径

> **作者角色**:资深架构师 / 产品经理  
> **方法**:全局扫描代码库（forge-core 17 包 + harness 30 项 + `.agent/` 全部声明源），**不依赖已有分析文档**，独立推导  
> **状态**:所有已分析与已实现的 90+ 项功能视为 DONE，本文件只讨论 **仍未被覆盖** 的结构性方向  
> **前提**:当前 `forge accept: ACCEPTED`，全闸门绿，5 轮扩展（ROADMAP.md 方向一~五及 Sprint 1-31）全量交付。  
> **排除**:已明确 deferred-by-design / blocked-external 的项（Firecracker / LiteLLM / Web UI / 多维路由 v2+ / Temporal 持久化 等）

---

## 全局扫描后确定：5 个高价值扩展方向

以下 5 个方向均经过代码级验证——每一方向都 **有具体代码位置可引用**、**有现有行为可对比**、**不是在真空中发明需求**。优先级标记为 P0/P1，逻辑与项目既有分类一致。

---

## 方向一 · 多仓库依赖图治理（Multi-Repo Dependency Governance）

**P0 · 架构地基缺失 · 影响：ForgeOS 自称"站在编码 CLI 之上"，但一个 repo 无法治理微服务生态**

### 代码证据

| 位置 | 含义 |
|---|---|
| `forge-core/internal/orchestrator/orchestrator.go:18-57` | `Engine` 只有一个 `root` 字段，指向**单个** repo 根目录 |
| `forge-core/internal/gate/gate.go:82-97` | `RepoRoot()` 返回单一路径，从 `--root` 或 `FORGE_REPO_ROOT` 取——没有"项目列表"的概念 |
| `.agent/workflows/*.yml` 全部 5 个文件 | 每个 workflow 声明 `id:` / `stage:`，但全仓**零处**有 `depends_on_project:` / `upstream:` 等跨仓库声明 |
| `harness/arch/scan.mjs:52-182` | `extractJsImports` / `extractGoImports` 只**在本仓库内**扫描导入图——跨仓引用被完全忽略 |
| `forge-core/internal/risk/risk_diff.go:3-28` | `FromChangedPaths` 只读 `.git diff --name-only HEAD`，**每个 repo 独立算**，不知道下游消费方 |
| `forge-core/cmd/forge/engine_build.go:225-259` | `phaseTierResolver` 不可见上游/下游仓库的状态 |

### 为什么需要

ForgeOS 的核心叙事是"治理编排控制平面，让 AI 24h 自治完成 Idea→Production"。但目前它的治理域等于**一个 Git 仓库**。真实的产品由多个仓库组成——共享库、微服务、客户端、配置文件仓库——一个仓的变更可以打崩另一个仓的构建。

当前 ForgeOS 无法回答以下基础问题：
- "这个 shared-lib 的 change 会 break 哪些下游 repo 的 test？"
- "需要跨 3 个 repo 协同发布一个 feature，如何编排？"
- "repo A 的 architect 设计了一个接口，repo B 的 implementer 多久后知道？"

这是 ForgeOS 从"单仓库 CI 编排器"走向"多仓库软件工厂"必须跨过的最大结构性缺口。

### 具体子项（可独立交付）

1. **依赖图注册**——新增项目级元数据 `.agent/dependencies.yml`，声明 `upstream_repos` / `downstream_consumers` / `shared_contracts`（protobuf / OpenAPI spec 路径）
2. **跨仓变更传播**——`forge detect` 扩展为在依赖图中传播变更信号：repo A 推代码 → 自动触发 repo B 的 discover/build 流程
3. **下游测试触发器**——`harness/adapters/` 新增 cross-repo test shim：在发布前在所有下游仓库跑 `forge gate`，结果聚合回上游
4. **接口契约执法**——当 architect phase emits 一个 OpenAPI spec，下游仓库自动得到更新通知 + 契约合规性检查
5. **版本协调**——多仓库 timeline 管理："release v2.3 要求 lib-x >= 3.1 且 service-y 已部署"

---

## 方向二 · 语义输出验证层（Semantic Output Validation）

**P0 · 功能缺口 · 影响：机械闸门绿 ≠ 正确的实现，自治开发的信任瓶颈**

### 代码证据

| 位置 | 含义 |
|---|---|
| `forge-core/internal/gate/gate.go:35-55` | 三个 verdict 状态：`PASS` / `FAIL` / `NA`——只覆盖**机械属性** |
| `harness/arch/arch-check.mjs:50-320` | 8 项检查全是语法/结构级：layering / 包 / 扇入 / 认知 / 反模式 / 函数长度 / 循环依赖 / drift-guard |
| `harness/acceptance.mjs:45-230` | `collect` 聚合 gate + lint + coverage + test + app-test + sca + security——**全是机械的** |
| `forge-core/internal/converge/converge.go:180-260` | `evalRoadmap` / `evalReviewStatus` / `evalRequirementConfidence`——收敛信号基于**自我声明的完成度**，不是独立验证 |
| `.agent/workflows/build.yml:101-110` | `stop_condition: conjunction` 评估 `roadmap_completion=100% AND gates_status=green`——**不验证产物语义正确性** |

### 为什么需要

当前整个治理体系的前提假设是："测试全绿 = 实现正确"。这个假设在传统 CI 中成立（测试由人类编写且作为需求规格），但在 AI 自治开发的语境下**严重不成立**。

真实 world 问题：
- implementer 写了测试，测试针对它自己写的代码——**自我验证循环**。测试可以通过但实现的是错误的需求。
- reviewer phase 以散文形式输出评审意见(`VERDICT: APPROVE`)，但**没有机器可执行的语义验证**说"代码真的满足了 PRD 的需求"。
- ROADMAP 的 `[x]` 由 agent 自己勾选——`FileDelta` 只是粗略的路径匹配启发式，不是需求回溯验证。

需要的是：
- **需求-实现追溯矩阵**：prd.md 的每条需求 → 对应的测试 → 测试结果 → 覆盖报告
- **不变式执法**：声明式的不变式（"DELETE 接口必须返回 4xx" / "密码必须 bcrypt"）由独立于 agent 的工具验证
- **输出差异分析**：agent 的产出 diff 被自动评审——只改了对的文件、没改不该动的文件、没引入死代码
- **验收测试独立编写**：在 planner phase 由独立 agent / 模板生成验收测试，implementer 不得修改验收测试

### 具体子项（可独立交付）

1. **需求回溯系统**——新增 `converge.RequirementTrace` 信号：prd.md 的每条需求项 → 对应测试名 → 对应 gate 结果
2. **声明式不变式引擎**——新增 `.agent/invariants/` 目录，声明 `must:` / `must_not:` 规则（语言无关，工具验证）
3. **输出污染检测**——扩展 gate 体系：检查 agent 产生的 diff 是否触碰了其角色卡不允许触碰的文件（超出了 `emits:` 声明）
4. **验收测试独立生成**——planner phase 生成 `acceptance/<feature>.test.*`，implementer 不得编辑，在 gate 中做 MD5 校验
5. **语义 diff 摘要**——trace 事件扩展 `kind:"agent_diff"`：记录每个 agent phase 产生的变更摘要（文件 + 行数 + 变更分类：新增/删除/重构）

---

## 方向三 · Agent 故障升级协议与优雅降级（Agent Escalation Protocol）

**P1 · 边界情况 · 影响：24h 无人值守现在的失败模式是硬 abort，缺少"自救"选项**

### 代码证据

| 位置 | 含义 |
|---|---|
| `forge-core/internal/orchestrator/orchestrator.go:321-358` | `agentOutcome` 在 verdict=REQUEST_CHANGES 时 loop-back，但 loop-back 耗尽后只有**硬 abort**——无降级路径 |
| `forge-core/internal/orchestrator/loop.go:107-114` | `NoProgress` tripwire 累加超过阈值后直接 `StopReason: "no progress"`——没有"换 agent 再试"选项 |
| `forge-core/internal/orchestrator/backoff.go:1-30` | `Backoff` 只用于 529/overload 重试——不适用于"agent 反复失败任务"的场景 |
| `forge-core/cmd/forge/evolve.go:55-67` | `rejectHumanGate` hard fail——没有"降级为 advisory"机制 |
| `forge-core/internal/orchestrator/exec_error.go:15-53` | `KindFailed` / `KindTimeout` / `KindOverloaded` / `KindConfig`——没有 `KindInexpert`（"这个任务我干不了"） |
| `forge-core/internal/routing/routing.go:34-48` | `opusFloorAgents` / `agentTier`——路由单向提升，没有"当前 agent 不行，换个更贵的试试"的递归降级 |

### 为什么需要

当前 ForgeOS 的容错模型是二进制：要么重试（retryable error），要么硬 abort。这在 24h 无人值守的真实场景中远远不够。

需处理的真实场景：
- **Agent 说"我不会"**：implementer 接到一个它不熟悉的框架任务（如 Rust lifetimes），反复尝试失败。理想行为：降级为"调 Opus implementer 试一次"，仍然失败则 "标记为需人工处理 + 进入下一 roadmap item"。
- **Reviewer 一直 REQUEST_CHANGES**：reviewer 和 implementer 进入死循环（主观审美分歧、风格偏好）。理想行为：loop-back 过半后自动切换 reviewer agent（另一个 fresh-context reviewer），仍不一致则 escalate 到 human。
- **Gate 反复不绿**：某个 gate（如 coverage）在 N 次 loop-back 后仍然不绿。理想行为：agent 可发出 "gate X 需要人工配置" 的信号，导致降级运行（该 gate 标 N/A 但不允许 ACCEPTED，需要 human override）。
- **迭代中途预算烧穿**：`--run-budget-usd` 烧尽。理想行为：优雅冻结当前状态 + 生成 checkpoint + 记录"budget exhausted at phase N, in iteration M"，而非硬 crash。

### 具体子项（可独立交付）

1. **`KindInexpert` 错误信号**——`ExecError` 新增 `KindInexpert`，标记"agent 确认无法完成任务"，触发 escalate 而非 retry
2. **降级路由**——`TierFor` 新增 escalation path：agent 失败 X 次 → 自动抬一档 model tier（Haiku→Sonnet→Opus），配一个 MaxEscalationBudget
3. **Reviewer 切换**——当同一 phase 内 `loopBackCount > threshold`，自动轮换 reviewer agent（现有 `fresh_context: true` 是单次，扩展为可换人）
4. **优雅 checkpoint-on-abort**——在 abort 前总是写一个包含失败原因的 checkpoint + 人类可读的 `FORGE_ESCALATION.md` 说明
5. **部分通过模式**——引入 `forge accept --allow-exceptions` 模式：human 可接受部分 gate 不绿但标记例外，不被 agent 利用绕过

---

## 方向四 · 知识生命周期管理（Knowledge Lifecycle Management）

**P1 · 性能/存储 · 影响：当前 memory 是纯 append-only 的日志，项目运行数月后会积累数万条条目，无衰减/压缩/归档**

### 代码证据

| 位置 | 含义 |
|---|---|
| `forge-core/internal/memory/memory.go:40-60` | Memory 是 JSONL 文件，Append 使用 O_APPEND——**只增不删** |
| `forge-core/internal/memory/memory_compact.go:1-50` | `Compact` 函数存在，但只按 `keepPerKind` 做最近 N 条保留——**无基于时间的衰减**，无语义压缩 |
| `forge-core/internal/memory/memory.go:180-220` | `Load` 读取**整个 JSONL 文件到内存**——随项目运行时间增长，每次加载 O(n) |
| `forge-core/internal/persist/checkpoint.go:30-70` | Checkpoint 重写整个 JSON，无增量 snapshot——写 O(全状态) |
| `forge-core/internal/prompt/retrieve.go:1-90` | TF-IDF 检索在所有 memory 条目上评分——**无分层索引**，无时间衰减权重 |
| `forge-core/internal/trace/trace.go:60-130` | Trace 是 JSONL，**无轮转**——24h 运行产生的 trace 文件可轻易超过 100MB |

### 为什么需要

ForgeOS 的设计假设是 "run-scoped"：一次 run 的 memory 积累只对该 run 有意义。但实际上——尤其是 evolve loop——memory 是跨 run 的（`store = filepath.Join(root, ".forge", "memory.jsonl")`）。

随着项目演进：
- memory 文件线性增长，每次 `Load` 越来越慢
- 早期学到的知识（"我们尝试了 Redis 但发现太复杂"）在项目演进后已经过时，但 TF-IDF 仍然把它作为高相关度条目注入
- 衰减权重 `decayWeight` 只被 scorecard 使用（`scorecard.mjs:230`），memory 条目**完全没有时间衰减**
- checkpoint 持久化全状态每次 O(n)，n 随迭代增长
- 项目运行 200 次 evolve 迭代后，`.forge/` 目录可轻易达到 50MB+

### 具体子项（可独立交付）

1. **Memory 条目 TTL**——`Entry` 新增 `TtlDays` 字段（默认 0=永久），超过 TTL 的条目被 `Load` 过滤 / `Compact` 删除
2. **分层 memory 存储**——将 memory 分为 "ephemeral"（当前 run，存在时间短）和 "persistent"（跨 run 共识，带衰减权重），ephemeral 在当前 run 结束后自动归档
3. **语义记忆摘要**——`Compact` 扩展为支持 summarization：同一 topic 的 N 条旧条目被压缩为一条摘要（纯 Go 实现，无 LLM：基于实体提取 + 决策树归纳）
4. **Trace 日志轮转**——Trace writer 支持格式 `trace.1.jsonl` / `trace.2.jsonl`，按大小或时间轮转；旧 trace 可归档为 `.tar.gz`
5. **内存索引**——`Load` 改为按需加载（lazy loading）+ build 倒排索引，而非每次全量读入内存

---

## 方向五 · 可观测性因果追踪与根因分析（Observability Causal Tracing & RCA）

**P1 · 运维/诊断 · 影响：当前 trace 是平面事件流，无法将 "gate FAIL" 追溯到 "是哪个 implementer phase 引入的 bug"**

### 代码证据

| 位置 | 含义 |
|---|---|
| `forge-core/internal/trace/trace.go:36-55` | `Event` 是平面结构——`Kind` + `Name` + `Status` + `DurationMs`——**无 parent/child 关系** |
| `forge-core/internal/trace/trace.go:97-130` | `Seq` 是单调递增——所有事件是**平坦列表**，无 Span 树 / Trace ID |
| `forge-core/cmd/forge/cost.go:60-90` | `feedCost` 只关联 cost 到**当前 phase 名**——不溯源到具体 task 或 roadmap item |
| `forge-core/internal/orchestrator/orchestrator.go:180-250` | `RunFrom` 按序执行 phase，但 phase 之间**无因果关系追踪**：gate FAIL 的根因只存在于 reviewer 的散文评论中 |
| `forge-core/internal/converge/converge.go:140-170` | `Signals` 记录最终值——不记录**每个 gate / phase 的细分贡献** |
| `harness/arch/arch-check.mjs:120-160` | 违反 arch 规则的报告中只显示当前违规——不追溯到**哪个修改引入**了该违规 |

### 为什么需要

当前 ForgeOS 的运维诊断能力是："gate FAIL → 看 gate 输出。reviewer REQUEST_CHANGES → 看 reviewer 的 prose。" 在 24h 无人值守的运行中，这种诊断方式是**不可扩展**的——operator 需要阅读数千行日志和 gate 输出来定位问题。

真实 world 场景：
- Evolve 迭代 7 的 gate FAIL：谁引入的？（迭代 5 的 implementer 改了一个共享模块，迭代 6 的 reviewer 没发现，迭代 7 的另一个 implementer 依赖了那个模块才炸——需要**多跳追踪**）
- Budget 在迭代 12 烧穿：是哪个 phase 最贵？那个 phase 对应的 roadmap item 是否因此被阻塞？
- Reviewer APPROVE 但 production gate FAIL：reviewer 漏了什么？**纵向追踪** reviewer 的评判 vs gate 的实际结果。
- `forge doctor` 能诊断当前状态，但不能回答"从什么时候开始出现这个模式"。

### 具体子项（可独立交付）

1. **Span / Trace ID 模型**——`trace.Event` 新增 `trace_id` 和 `parent_span_id` 字段（可选），形成事件树而不仅是列表
2. **因果回溯命令**——`forge diagnose <trace.jsonl> --from-gate <gate-name>`：从 gate FAIL 事件回溯到该 phase 依赖的所有上游 phase 的事件序列
3. **差异归因**——`forge blame --trace .forge/trace.jsonl`：显示每个文件修改是由哪个 agent/phase/iteration 在何时引入的
4. **收敛时间线**——`forge timeline .forge/trace.jsonl`：将事件流渲染为时间线（iteration 边界 + 每个 phase 的墙钟 + cost + verdict），方便 human 快速概览 24h 行为
5. **reviewer-有效性评分**——新信号 `converge.ReviewerAccuracy`：比较 reviewer 的 verdict 与后续 gate 的实际结果，给每个 reviewer agent 生成"准确性分数"
6. **trace 查询 DSL**——`forge trace --select 'kind=gate & status=FAIL' --within 'iteration:3-7'`：在 trace 上运行结构化查询

---

## 优先级与收敛建议

| 方向 | 优先级 | 类别 | 一句话杠杆 |
|---|---|---|---|
| 一 · 多仓库依赖图治理 | **P0** | 架构 | ForgeOS 自称"元框架/软件工厂"，但当前治理边界 = 单 Git 仓库——不突破这个天花板，愿景无法闭环 |
| 二 · 语义输出验证 | **P0** | 功能 | 机械闸门在 AI 自治开发下存在"自我验证循环"的结构性盲区——测试由同一 agent 编写和验证 |
| 三 · Agent 故障升级协议 | P1 | 边界/韧性 | 24h 无人值守不能硬 abort 为唯一失败模式——需要优雅降级和逃生路径 |
| 四 · 知识生命周期管理 | P1 | 性能/存储 | 纯 append-only 积累在 100+ 迭代后导致 Load O(n) + 注入过时知识——项目越老越严重 |
| 五 · 可观测性因果追踪 | P1 | 运维/诊断 | 当前平面事件流在 24h 运行后诊断效率极低——operator 需要手动关联数百个 phase 事件 |

### 收敛建议

- **若只做一件**：方向二（语义输出验证）——这是 AI 自治开发**信任模型**的根本缺口。面向未来，自治开发的社会许可取决于"你能证明 AI 产出的正确性"，而不是"你有测试"。
- **做前三件（全 P0）**：方向一+二+三——分别解决"治理域"、"信任模型"、"失败模式"三大结构性天花板。
- **方向四/五**建议随产品化阶段（方向一+二做完后、做三时同步）推进——四解决长期运行的退化问题，五解决运维瓶颈。

---

## 方法说明

本文件的每个方向均通过以下方式验证：
1. **代码级定位**——每个方向标注了 `file:line` 级别的代码位置作为证据
2. **对比当前行为**——明确说明当前（DONE 状态）与扩展后行为的差异
3. **排除已有覆盖**——查阅了前 100+ 份分析文档，确保不与已交付/已讨论的方向重复
4. **诚实边界**——每个子项标注了可独立交付的范围，避免被指责为"镀金"或"无法验证"
