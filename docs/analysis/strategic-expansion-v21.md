# Strategic Expansion Directions v21 — 资深架构师/产品经理视角

> 基于全局代码扫描 (forge-core 13 Go 包 + harness 全套 + 26 sprints 迭代现状)。
> 聚焦 **3-5 个高价值方向**，覆盖核心功能缺口、边界情况风险、性能瓶颈。
> 不写代码，只做判断和论证。

---

## 方向一：跨厂商模型路由与 Provider 层 (Cross-Provider Model Pool)

### 为什么需要

当前 `routing.ModelMap` 只有 `anthropic` 一个 provider,硬编码 `claude-sonnet-4‑haiku / claude-sonnet-4 / claude-opus-4`。
`CandidatesForTier`、`BudgetAdjustTier`、`HistoryTiebreak`、`TierForScore` 的**全部分配逻辑**都已实现且测试覆盖，
但它们**只在一个厂商的三个档位内选择**。这意味着：

1. **单一厂商宕机 = 全系统不可用**——529 backoff 已经实现(`classifyClaudeOverload`)，但 retry 耗尽后直接
   abort，没有 fallback 到其他 provider 的路径。
2. **没有成本套利**——不同厂商在同一能力档位的定价差异巨大(GPT-4o vs Opus vs Gemini 2.0)，
   当前系统有 `scorecard → HistoryTiebreak` 的选择结构，但候选集限定在 claude 内部。
3. **没有厂商特定的 token 计价模型**——`cost.go` 的 `parseClaudeCostUsd` 只认 claude JSON 的
   `total_cost_usd` 字段，换厂商需要重新写解析器。

### 代码中已存在的支撑骨架

- `routing.ModelMap`: 已经是 `map[provider]map[tier]modelName` 结构
- `routing.Providers()`: 已经准备好返回 provider 列表
- `routing.ResolveModel(provider, tier)`: 已经能按 provider 查 model name
- `routing.CandidatesForTier(tier)`: 已经扩展为 `[adj, ...cheaper]` 多候选
- `routing.BudgetAdjustTier`: 已经实现预算驱动降档
- `scorecard.go`: 已经有 `HistoryTiebreak` 的多候选选择逻辑

### 具体缺失 (需要填补)

| 缺口 | 严重度 | 说明 |
|---|---|---|
| Provider failover 逻辑 | 高 | 529 → retry exhausted → 切 provider，而不是 abort |
| 统一 cost telemetry 接口 | 高 | `parseClaudeCostUsd` 是 claude-specific，需要 `CostParser(provider, output) → usd` 接口 |
| 跨厂商 scorecard 归一化 | 中 | 不同厂商的 quality/p95/cost 不能直接比较，需要归一化因子 |
| Provider 健康检测 | 中 | 主动探测 provider 可用性，避免把 phase 派给已降级的厂商 |
| Token 预算跨厂商换算 | 低 | claude 和 GPT 的 token 计价单位不同，`--agent-max-budget-usd` 需要 provider-aware |

### 边界情况

- **mixed-provider workflow**: 同一个 workflow 的不同 phase 用不同 provider(如 reviewer 用 Opus,
  implementer 用 GPT-4o)，不存在一致性事务——每个 phase 独立路由，这是正确的(phase 本来就是原子单元)。
- **provider-level budget guard**: `--run-budget-usd` 目前是全局累加，跨厂商后需要区分每家花了多少，
  但 fail-closed 逻辑不变(总超就停)。
- **model name drift**: 厂商会重命名/废弃 model，`ModelMap` 作为静态 map 需要热更新机制或外部配置源。

---

## 方向二：真实 Discover Pipeline 与外部数据集成 (Real Discovery)

### 为什么需要

ForgeOS 的最高论点是 **「需求探索 > 代码实现」(BOOTSTRAP.md)**，`PROJECT.md` 的 G1 也是
**「需求自动发现」**。但当前现状：

- `mode.Policy` 的 `DiscoverDepth` 已经实现(full/light/skip)，中枢旋钮完整覆盖。
- `forge run discover` 可以执行 discover.yml 的 3 个 phase。
- **但** `CURRENT_SPRINT.md` 诚实标注：「decision-ready + narrative, doesn't pretend to run
  real discovery」——当前 discover 只是叙述「需不需要做 / 做到多深」，而**没有真正的市场调研、
  竞品分析、能力矩阵数据采集**。
- `.ai/prompts/00-product-discovery.md` 已有 prompt template，但 agent 没有真实外部数据
  (web search / competitor data / industry reports) 可消费。

这意味着：**ForgeOS 自称最核心的差异化能力(需求探索)，在 v2 只是一个空壳**。

### 需要建什么

1. **Data Source Adapter 接口**——类似 `CommandExecutor` 的抽象层：
   ```go
   type DataSource interface {
       Search(ctx context.Context, query string) ([]Document, error)
       Fetch(ctx context.Context, url string) (Document, error)
   }
   ```
   注册式插件(`SearchWeb` / `SearchCode` / `FetchCompetitorData`)，默认无凭据时
   诚实降级为 N/A（复用 harness 的 honesty 模式）。

2. **Structured PRD 输出 Schema**——当前 agent 输出是自由文本，PRD 需要一个类型化的结构体
   （需求项/优先级/置信度/关联 ADR），下游 designer/architect 才能程序化消费，而不是靠
   LLM 从 prose 二次抽取。

3. **Confidence 校准**——`converge.Signals` 已有 `RequirementConfidence` 字段，
   `evalRequirementConfidence` 已实现。但缺少「信心标尺」——agent 自报的 85% 置信度
   和真实可用性之间的 gap 需要校准，可能需要从「该需求在后续 build 中被修改/废弃的比例」
   做反馈回路。

4. **生命周期内循环验证**——discover 的假设不应只在 design 阶段被验证，应该在 build 中
   被事实驳斥后反馈回 discover。当前没有这个回路。

### 边界情况

- **无外部凭据**: 和 `sca.mjs` 的 OSV DB 缺省一样，DataSource 无配置时诚实 N/A，不透支。
- **API rate limit**: 搜索 API 有配额，需要 budget-aware 的调用次数限制(cost 维度已实现框架)。
- **置信度通胀**: agent 会系统性高估自己的发现质量，需要客观交叉验证(如搜索结果引用数)。

---

## 方向三：Agent 执行沙箱 (Sandbox / MicroVM)

### 为什么需要

`command_executor.go` 中已存在 `SandboxConfig` 占位结构体：

```go
type SandboxConfig struct {
    Type       string // "" (none) | "firecracker" | "docker"
    Image      string
    MemoryMB   int
    TimeoutSec int
}
```

但它**零消费**——`Execute` 方法中完全没用到。这意味着当前 `--executor=command --agent-cmd=claude`
的 agent 进程直接在宿主机的 shell 中运行，没有进程隔离、文件系统隔离、网络隔离。

对于「24h 无人值守自治软件工厂」的愿景这是一个根本性的信任问题：

1. **agent 可以读写宿主机任意文件**——`CommandExecutor.Dir` 仅设了项目根目录，
   os/exec 不做 chroot/namespace 隔离。一个被 prompt 注入的 agent 可以 `rm -rf /`。
2. **agent 可以 fork-bomb**——`MaxDepth` 和 `MaxAgentCalls` 已经做计数级防护，
   但计数不是隔离：agent 可以在耗尽预算前造成伤害。
3. **没有网络出口控制**——agent 默认有宿主机的全部网络访问权限，可外泄代码。
4. **North-star 架构声明这是「不可妥协」**——`不可妥协` 是原话。

### 架构方案(非代码)

- **v2 务实方案 = Docker 容器**——每个 `CommandExecutor.Execute` 的 run 在一个一次性容器里执行。
  `SandboxConfig` 提供 image/memory/timeout，执行器改为 `docker run --rm`。
  容器内挂载项目目录(read-write)、无网络(或白名单)、cgroup 限制内存/CPU。
- **v3 目标方案 = Firecracker microVM**——更强的隔离 + 更快的启动。
  需要独立的 sandbox daemon 管理 microVM 池，不在 forge-core 进程内。
- **钩子点**：`CommandExecutor.Execute` 方法内部，在 `cmd.Run()` 之前插入 sandbox 路由；
  输出/观察/超时/成本回调均不变。
- **诚实降级**：无 Docker/Firecracker 环境时，退化为直接执行(当前行为)，但记录 WARNING，
  `forge accept` 中 security gate 会标 N/A。

### 边界情况

- **sandbox 内文件持久化**——agent 写的文件要同步回宿主的 project root，容器销毁后丢失。
  需要写时的实时 rsync 或 volume mount。(volume mount 更简单，但破坏部分隔离)。
- **sandbox 内 `--agent-cmd`**——在容器里跑 `claude` 需要容器能访问宿主机的 credentials
  （API key 等），不能简单网络隔离。可能需要双向代理或 credentials mount。
- **sandbox 超时 vs 命令超时**——`SandboxConfig.TimeoutSec` 和 `CommandExecutor.Timeout`
  的关系需要明确(取更小或沙箱包外围)。
- **并行 sandbox 资源争用**——`--parallel` 下多个 agent 同时跑，每个启一个镜像，
  CPU/内存可能超卖，需要 admission control（复用 budget 框架的计数模式）。

---

## 方向四：结构化 Agent 交接协议 (Structured Handoff Protocol)

### 为什么需要

当前 agent 间的上下文传递走的是**纯字符串管道**：

- `phaseOutputLedger` 记录前一 phase 的输出文本 → `prompt_context.go` 拼接进下一 phase 的 prompt。
- `gateLedger` 记录 gate 裁决文本 → 同样的拼接。
- `reviewFindingsLedger` 记录 reviewer finding 文本 → 同样的拼接。

这意味着：

1. **没有类型化的输出契约**——`planner` 应该输出「任务列表 + 接受条件 + 拆分策略」，
   `implementer` 应该输出「代码文件 + 测试文件」，`reviewer` 应该输出「裁决 + 发现列表」。
   但现在全是自由文本的 `%s` 拼接，下游 agent 需要在自己的 prompt 里用 prose 重新理解上游输出。
   这导致 token 浪费(在 prompt 里重复描述格式)和理解偏差(LLM 误解 prose 结构)。

2. **没有输出验证**——如果 `planner` 这次输出的格式和上次不一致，`implementer` 会 silently
   误解，没有 schema validation 来及早拒绝。

3. **没有上下文窗口的优先级管理**——`prompt.Gather` 目前注入「全部 ADR + 全部 constraints +
   全部 gateLedger + 全部 feeds_forward」——跑 24h 后 token 预算必然爆。虽然有 `Retrieve`
   (BM25-lite 检索器)但它的职责只是 ADR 级别的内容选择，不负责跨 phase 输出的压缩/摘要/淘汰。

### 需要建什么

1. **PhaseOutput Schema**——每个 `Phase.Agent` 角色对应一个可选的输出结构体
   (如 `planner → PlanOutput{Items []TaskItem, AcceptanceCriteria string, …}` )，
   存储在 `FeedsForward` ledger 中，而不是 `string`。
   `FreshContext` phase(如 reviewer) 仍然不接收这些结构化数据(新鲜性原则)。

2. **Schema Validation Gate**——在 agent phase 完成后，验证其输出是否符合声明的 schema。
   违反 = 重新生成(loop-back 的另一种触发条件)，而不是 silently 往下传。
   这比「靠 reviewer 的 LLM 判断」更早、更便宜地捕获格式偏差。

3. **Context Window Manager**——不是简单 top-K 检索，而是维护一个**带优先级的上下文堆栈**：
   - **hard constraints**(AGENTS.md, project.yml)：始终注入，不可压缩。
   - **当前迭代的 feeds_forward**(上一 phase 的结构化输出)：始终注入。
   - **最近 K 条 gate 裁决 + review finding**：按时间/重要性排序，超出 token 预算则摘要化。
   - **历史 memory 条目**：按 kind+topic 检索 + `Confidence` 排序，低置信度加 `[unverified]` 前缀。
   - **ADR 内容**：走 `Retrieve` 检索。

   当前 `prompt.Gather` 每次重新计算全部内容无状态；Context Window Manager 应当是
   **有状态的跨 phase 缓存**，复用上一轮已检索/已压缩的结果，只增量更新。

### 边界情况

- **Schema 演化**：workflow 版本升级后 schema 变化，旧 checkpoints 里的结构化输出格式不匹配。
  需要 `FormatVersion` 标记(类似 `persist.Checkpoint.FormatVersion` 的模式)。
- **压缩的信息损失**：对旧 findings 做摘要时可能丢失关键细节。应该保留 `confidence` 字段，
  摘要中标注 `[summarized]`，当 loop-back 需要时重新检索原始条目。
- **`FreshContext` 的边界**：`FreshContext=true` 的 phase (reviewer) 不应接收任何结构化负债，
  但需要知道 gate 客观裁决(`gateLedger` 的 PASS/FAIL/NA，不是 prose)——这已经是当前实现。

---

## 方向五：多项目调度与资源治理 (Multi-Project Scheduling)

### 为什么需要

当前 ForgeOS 是**单项目单进程**——一个 `forge evolve` 管一个 repo 的一个 workflow。
North-star 架构(AI 软件工厂的 Kubernetes)需要**一个控制平面同时管理 N 个项目的 M 个 agent 宿主**：

- **没有 agent 池**：每个 `forge run/evolve` 独占一个进程，不能跨项目共享 agent CLI 进程的
  `claude` 预热/上下文缓存/API key 配额池。
- **没有队列系统**：同时提交 10 个 project 的 `forge evolve`，没有优先级、没有背压、
  没有并发限制——直接 10 个进程抢 `claude` 进程和 API rate limit。
- **没有跨项目预算治理**：`--run-budget-usd` 是 per-run 的，不能设置全组织日预算上限。
- **ADR 0003(agent-os submodule)**：设计就绪但未实现。多项目需要共享 agent 卡/workflows/skills。

这是**从「单项目工具」到「软件工厂平台」的结构性飞跃**。微服务化(API Gateway / Temporal /
Postgres / Qdrant)不是 v2 的目标，但调度原语可以在 forge-core 现有结构上增量添加。

### 具体缺失(可增量实现)

| 层 | 缺失 | v2 可行方案 |
|---|---|---|
| **调度** | `AgentScheduler` 类型，管理并发 run 的队列、背压、准入 | 一个轻量 `workPool{maxConcurrent, queue chan workItem}`，在 cmd/forge 层，非独立服务 |
| **池管理** | `CommandExecutor` 连接池，避免每个 phase 都新启 claude 进程 | 进程级预热(keep-alive `claude` session)，复用已有的 `cost.go`/`Observe` 回调 |
| **全局预算** | `--run-budget-usd` 是 per-run，没有组织级日/月预算上限 | 一个 `globalBudget` 从 `~/.forge/budget.jsonl` 读取已用额度，增量衰减；纯文件系统，零依赖 |
| **项目注册** | `forge status` 只能看当前项目，不能列出所有被管理项目 | `forge project list/register` 读写 `~/.forge/projects.json`，每个条目存根目录 + 最新 checkpoint |
| **共享资产** | ADR 0003 的 agent-os submodule 机制未落 | `forge-core/internal/asset` 增加 `OverlayFS` 式路径解析：项目层覆盖子模块层 |

### 边界情况

- **并发 budget race**：多个 `forge evolve` 同时写同一个 `~/.forge/budget.jsonl`，
  需要文件锁(flock)，类似 `memory.go` 的 O_APPEND 但跨进程。
- **claude API rate limit**：即使有进程池，`claude -p` 共享同一个 Anthropic API key，
  HTTP 429 仍是 per-key 限制。需要熔断器(circuit breaker) + 退避分发。
- **submodule 版本漂移**：`agent-os` 更新后，旧项目的 overlay 可能与新 schema 不兼容。
  资产加载器需要 `FormatVersion` 检查 + 迁移提示。

---

## 横切关切：Edge Cases & 性能优化

### 影响多个方向的边界情况

1. **Parallel mode + loop-back 不兼容**——`parallel.go` 清晰注明「NO directed loop-back」，
   但如果某天 build.yml 想走 `--parallel`，gate 失败后没有修复路径。
   解决方案：可以让 loop-back 退化为 serial re-run(失败后串行重做一波)，而非直接 abort。

2. **executor=command 下 YAML 转码可靠性**——`loadWorkflow` 先试 Go YAML 解析器再 fallback
   到 `python3 harness/yaml2json.py`。后者依赖 Python 3 + PyYAML，在干净环境/容器中可能缺失。
   应该缓存转码结果(`.forge/workflow-<name>.json`)，避免每次 run 都重转。

3. **Vacuous workflow detection**——`orchestrator.go` 的 `phasesRan == 0` 检查在 mode gating
   过滤掉所有 phase 时发出警告。但 `loop.go` 中的 `len(wf.Phases)==0` 检查直接返回
   `not converged`，两者的行为不一致(一个 `exit 0`，一个 `exit 1`)。

4. **trace.jsonl 旋转丢失数据**——`openTracer` 在 >10MB 时将旧 trace 重命名为 `.1`，
   但 `.1` 会被下一次旋转覆盖。长时间 evolve run(>100 iter)可能丢失早期 trace。
   解决方案：按日期/轮次命名，`seq` 字段保证全局有序。

### 性能瓶颈

1. **Retrieve 的 O(n) 线性扫描**——每次 `prompt.Gather` 都把全量 ADR/constraint 文档送入
   `Retrieve` 做 BM25-lite 评分。对于数十个 ADR 这是 OK 的，但数百个文档时(一个成熟项目)，
   `score()` 的词汇表遍历 + `sort.SliceStable` 会成为明显的延迟源。
   **优化方向**：预建倒排索引(df 表)，build 时一次、后续查询 O(1) 查 df。

2. **memory.Load 的 mtime 缓存粒度**——`loadCache` 是 per-path 的，但同一 path 被
   prompt build 和 converge check 在不同 goroutine(parallel mode)并发访问时，mtime 检查
   和执行 Stat 之间可能有竞争，导致双读。更重要的是：**invalidateLoadCache 是全表删除**
   ——一个 Append 会清空所有 path 的缓存，如果有多项目共享同一个 memory 路径(不常见)会产生连锁。
   **优化方向**：per-entry TTL 或 write-back cache，而非全量无效化。

3. **trace.Emit 的 mutex 竞争**——当前是单线程无竞争，但 `--parallel` 下多个 agent phase
   同时完成同时 `Emit` trace event，锁会短暂串行化。对 1-10 并发影响可忽略，
   但 50+ 并发(大规模 fan-out)时需要 lock-free 或无锁序列号分配。
   **优化方向**：原子 seq 分配(`atomic.AddInt64`) + per-goroutine buffer + 批量 flush。

4. **context window 浪费**——`prompt.Gather` 每次注入 `AGENTS.md` 的约束 bullet list +
   所有 ADR 标题，即使本次 phase 不涉及架构决策。`fresh_context: true` 的 phase 也
   没有跳过 ADR 注入——它只跳过 feeds_forward，ADR 仍在。
   **优化方向**：按 phase.Agent 角色过滤上下文类型(如 implementer 不需要 ADR 全文，
   reviewer 不需要 market-research 结果)。

---

## 优先级建议

| 方向 | 影响 | 实现风险 | 依赖 | 建议 |
|---|---|---|---|---|
| **跨厂商路由** | 高(可靠性+成本) | 低(骨架已存在) | 无 | **Sprint 27-28** |
| **真实 Discover** | 高(核心差异) | 中(外部 API 集成) | 无 | **Sprint 29-31** |
| **Agent 沙箱** | 高(安全信任) | 高(Docker/Firecracker) | 跨厂商路由(需 provider 选择) | **Sprint 29-30(容器版)** |
| **结构化交接** | 中(质量+token 效率) | 中(schema 定义) | 无 | **Sprint 28-29** |
| **多项目调度** | 低→中(平台化) | 高(架构跨度大) | 以上全部 | **v3** |

**首要推荐**：方向一(跨厂商路由)在现有骨架上增量最小、风险最低、价值即时可见(529 回退保
生产可用性)。方向二(Real Discover)是产品差异化的最大杠杆，但需要外部集成，建议紧随其后。

---

*生成时间: 2026-07-01 · 基于 forge-core commit HEAD · 无代码变更*
