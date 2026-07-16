# ForgeOS — 五个未被已有 84+ 分析覆盖的高价值扩展方向

> **角色**: 资深架构师 / 产品经理  
> **方法**: 全局深扫 forge-core（19 Go 包 / ~35k LOC 生产代码 / 纯 stdlib 零依赖）、
>   harness（39+ 模块 / ~10.5k LOC 执法层）、`.agent/`（12 agent 卡 / 9 skill 卡 / 5 workflow / 
>   policies / ADR 全部）、examples/、CI、Sprint 1–31 完整演进记录。
> **纪律**: 逐方向交叉验证 **84+ 份已有分析文档**（`docs/requirements/` 44 篇 + `docs/analysis/` 40 篇 +
>   `FUNCTIONAL_REQUIREMENTS_AUDIT.md`）确认核心论点未被覆盖。
> **不写代码。仅基于代码现状分析。**
> **日期**: 2026-07-10

---

## 全景定位：已有 84+ 分析的密集覆盖域

为避免重复，以下主题已被充分覆盖，本文不再展开：

| 主题 | 代表文档 |
|------|----------|
| 跨项目联邦 / 多仓库治理 | `architectural-extensions-v38.md` 方向一，`expansion-five-product-blindspots.md` 方向四 |
| YAML 双解析器差分 | `expansion-production-readiness.md` 方向四，`forge-core-five-unseen-structural-gaps.md` 方向二 |
| Agent 输出契约 / schema 验证 | `execution-semantic-gaps.md` 方向三，`architectural-expansion-perspectives.md` 方向二 |
| Memory 知识生命周期 / 跨会话 | `architect-product-perspective-five-directions.md` 方向一，`novel-five-perspectives-2026-07-10-deep.md` 方向四 |
| Binary 分发 / 版本治理 | `genuine-uncovered-five-binary-state-output-session-datalifecycle.md` 方向一，`five-product-operational-gaps.md` 方向一 |
| On-disk 格式版本化 / 迁移 | `five-verifiable-code-level-gaps.md` 方向三，`execution-semantic-gaps.md` 方向四 |
| 运行时健康 / 自诊断 | `four-truly-unexplored-architectural-gaps.md` 方向三，`five-verifiable-code-level-gaps.md` 方向二 |
| Eval 闭环 → 路由回灌 | `expansion-five-systemic-learning-loop-gaps.md` 方向一，`architect-product-perspective-five-directions.md` 方向一 |
| 并发安全 / 并行竞态 | `five-gaps-from-global-scan-2026-07-10.md` 方向五，`expansion-five-systemic-architectural-gaps.md` 方向五 |
| 自适应 mode 调优 | `novel-five-highvalue-extensions.md` 方向五 |
| 测试基础设施 / Fuzz | `expansion-production-readiness.md` 方向三/四（脚注提及 fuzz） |
| 发布工程 / 供应链 | `five-high-value-extensions-v44.md` 方向一 |
| 外部 hook / 插件系统 | `novel-architectural-extensions-v40.md` 方向五 |
| 自治运行退化检测 | `next-five-frontiers.md` 方向三 |
| 资源 accounting / 磁盘监控 | `production-operational-gaps.md` 方向一/二 |

---

## 本文五个方向

### 方向一 · `internal/adr` — 从「人类文档校验器」到「可执行治理资产」

**当前状态**：`internal/adr` 包（`forge-core/internal/adr/`）是一个纯文档合法性校验器：检查 ADR 文件是否包含 Required 段、引用是否匹配、格式是否正确。它在 `forge validate` 路径中被调用，输出 PASS/FAIL。**ADRs 存在于工作树中的 `docs/adr/*.md`，但 forge-core 的编排引擎、路由引擎、gate 引擎——没有一个消费 ADR 的结构化内容。** ADR-0004 声明了「可执行 ADR 治理闭环」的设计意图，但实现层是零。

**代码级证据**：

```go
// forge-core/internal/adr/adr.go — 全部导出符号:
//   Validate(path) []Result  ← 只做格式校验
//   ValidateAll(dir) []Result ← 批量校验
// 零函数签名包含 Workflow / Engine / Policy 输入。
// 零函数从 ADR 提取结构化决策（"decision: use Go stdlib"）供路由或 gate 消费。
```

ADR-0001~0004 在 `forge validate` 中被检查是否格式正确、引用未断。但 `internal/routing`（模型路由）、`internal/orchestrator`（编排决策）、`internal/mode`（中枢旋钮）**完全无视 ADR 的内容**。ADR 的「批准」「否决」「重设计」等五择一裁决（Sprint 28 在 CTO 卡中定义的机读契约）只在 review 相位输出，从未被持久化到可回溯的结构化记录中，从未被 gate 条件引用。

**为什么需要**：

ForgeOS 的核心论点是「架构决策 > 代码」。但当前架构决策（ADR）的产出仅停留在人类可读的 Markdown 中。当 build.yml 决定走什么技术栈、当 router 判断什么模型适合此任务、当 gate 判断是否需加强检查——这些运行时决策完全依赖于 forge-core 自己的硬编码逻辑，**不参考项目自身已作出的架构决策**。结果是：ADR 作为治理载体有了「规定」的职能，但没有「驱动」的职能。

**这是一个产品层面的缺口**：ForgeOS 项目花大量精力在 ADR 纪律上（Sprint 30 审计时 ADR 是 4 篇关键权威源之一），但这些纪律产生的结构化决策从未被系统消费。用户问「为什么 forge 选了这个模型」——答案从 ADR 来，而不是从 routing.go 的散装 logic 来。

**具体方向**：

- 为 ADR 定义一个 YAML front-matter schema（`decision:` / `status:` / `applies_to:` / `constraints:`），与现有散文 MD body 共存
- 扩展 `ADR 0004` 的机读裁决契约：review 相位输出的 `VERDICT: APPROVE_WITH_SIMPLIFICATION` 等裁决可自动写入 ADR 元数据
- 建立 ADR-derived constraint 查询层：供 `internal/routing.TierFor`、`internal/mode.Policy` 读取当前生效的架构约束
- 接入 `forge validate --adr-constraints` 验证运行时决策与 ADR 声明的一致性

**诚实边界**：不建议立即让 ADR 完全驱动路由（那是范式转换）。建议先让 ADR 的决策状态成为可查询的一等公民——`forge adr list --status approved` 能跑、`forge run build --why` 能列出"因为 ADR-0003 决定 submodule 所以允许跨仓引用"。

**预估规模**: ~600 行（ADR 元数据 schema + front-matter 解析 + 查询接口 + 3 处消费接线）+ 跨 sprint 渐进。

---

### 方向二 · `.forge/` 目录缺少元数据护照（运行身份 + 格式兼容 + 来源追溯）

**当前状态**：`.forge/` 是 forge-core 的运行时状态目录（`internal/doctor/doctor.go:36-38`），包含 `checkpoint.json`、`trace.jsonl`、`memory.jsonl`、`scorecards.json`。每个文件有各自的格式标识——Trace 有 `Format: "forgeos.trace.v1"`（`trace.go:40-42`），Checkpoint 有 `FormatVersion: 1`（`checkpoint.go`），Memory 纯粹是 JSONL 无显式版本。**但没有任何一个文件记录「哪个 forge 版本创建了此文件」「属于哪个 run ID」「创建时间」「checksum」。** 更关键的是：**没有单个 `.forge/metadata.json` 文件聚合描述整个状态目录的身份。**

```go
// forge-core/internal/persist/checkpoint.go — Checkpoint 结构:
type Checkpoint struct {
    FormatVersion int    `json:"format_version"`    // 当前恒为 1
    PhaseIndex    int    `json:"phase_index"`
    Stage         string `json:"stage"`
    // 缺少: ForgeVersion string
    // 缺少: RunID       string
    // 缺少: CreatedAt   time.Time
    // 缺少: Checksum    string
}
```

```go
// forge-core/internal/trace/trace.go — Event 结构:
type Event struct {
    Format string `json:"_format"`    // "forgeos.trace.v1"
    Seq    int    `json:"seq"`
    // 缺少 Trace 自身的 run_id 字段（只有 Event 级别的 seq）
    // 缺少整个 trace 文件的创建者/来源
}
```

**为什么需要**：

这是一个**渐增的但无可避免的问题**：每次 forge-core 版本升级，.forge/ 格式可能微变。当前 forge-core 是单版本运行（自己 git clone 自己编译），但一旦 forge-core 作为二进制分发、一旦 CI 中的 forge 版本与本地不同、一旦 A/B 测试新旧版本——**老的 .forge/ 目录可能静默地被新版本误读**。

Sprint 30 的 FUNCTIONAL_REQUIREMENTS_AUDIT 列出了 "persistent storage version marker: written but never checked" 为已知缺口。但更根本的问题是：**没有任何机制让你分辨一个 .forge/ 目录是谁、何时、用什么版本、在哪个 run 中创建的。** 当你发现 `.forge/trace.jsonl` 里有可疑数据时，无法回答最基本的取证问题：「这个文件是哪个 forge 版本写的？」

Run ID（`five-uncovered-architectural-frontiers.md` 方向一提到）是一个未实现的概念。状态目录元数据护照是 Run ID 的前提条件——没有目录级护照，Run ID 无处可写。

**具体方向**：

- 新文件 `.forge/meta.json`：`ForgeVersion` + `RunID`（ULID）+ `CreatedAt` + `FormatVersions{trace, checkpoint, memory, scorecard}` + `Checksum`
- 每次 `forge run` / `forge evolve` 起跑时创建/更新 `meta.json`
- `forge status` 读取并显示 meta.json 内容
- `forge doctor` 增加 `CheckMetadata` 检查：meta.json 存在 + 格式版本与当前 forge 兼容 + 可选 checksum 验证
- `forge run --resume` 增加元数据兼容性检查：如果 checkpoint 的格式版本高于当前 forge 支持的范围，拒绝 resume 并给出明确提示

**诚实边界**：meta.json 本身引入了新的维护面。建议版本号用单个 int（递增），checksum 用 SHA-256 但容忍缺失（向后兼容老 .forge/ 目录即无 meta.json 的目录仍可运行，只是不享受兼容性保护）。RunID 不做跨进程协调（不防并发冲突，那是方向四/五的问题域）。

**预估规模**: ~400 行（meta 结构 + 读/写/升级 + `forge status` 显示 + `forge doctor` 检查 + `forge run --resume` 兼容守卫 + 测试）。

---

### 方向三 · 运行后「执行报告」与跨运行差异摘要

**当前状态**：`forge run` 和 `forge evolve` 运行完毕后，输出收敛到终端日志和 `.forge/` 数据文件中。但**没有一个结构化的、人类可读的「本次运行改变了什么」的摘要**。`forge status` 显示当前目录的健康状态。Trace 文件是机器可读的事件流。Scorecard 包含聚合指标。但如果你问——「上次 build workflow 运行后，哪些文件变了？gate 状态从红变绿了几项？成本比预算高还是低？这次迭代和上次比有改善吗？」——你需要手动 grep trace + diff + 读 scorecard。

```go
// forge-core/cmd/forge/main.go — cmdRun 的结尾:
// line 372-385: 打印 convergence 结果 + 每个 phase 一行状态
// 没有结构化报告输出 (除 --json 提供 basic JSON 外)
// 没有跨 diff 比较
// 没有文件变更总结
```

```go
// forge-core/internal/converge/converge.go — Signals 包含:
//   RoadmapCompletion, GatesGreen, GateProof, FileDelta, ...
// 但 reportConvergence 只打印 "convergence: MET (stop)"
// 不打印 "文件变更率: 34% (高于 30% 阈值, 诚实性检查满意)"
// 不打印 "相比上次: gates 从 3/6 绿提升到 6/6 绿"
```

Sprint 29 添加了 `FileDelta` 信号但只用于诚实性交叉验证（`roadmap>50% 且 FileDelta<30%` → 警告）。它的值从未以正面的、结构化的方式呈现给用户。

**为什么需要**：

**产品视角**：用户（尤其是非 ForgeOS 开发者、只是使用它的团队）需要一个「下班前跑完 forge evolve，第二天早上看发生了什么」的入口。当前 exit code + terminal scrollback 不是产品级的交互模式。没有摘要意味着：

- 你无法判断一次 evolve 迭代是「做了很多工作」还是「兜圈子」
- 你无法快速验证「我改了 workflow，运行结果是否符合预期」
- 你无法向团队证明「这次改进使 gate 通过率从 60% 升到 80%」
- 你无法在 CI/CD 中消费 forge 运行结果来做自动化决策（当前只有 exit code）

**产品定位**：这不是一个「把 trace 画成图」的可视化需求——那是 Web UI（v3/north-star）。这是一个「把运行的关键指标汇总成一页纸的报告」的结构化数据需求。类似 `go test -v` 到 `go test -json` 的进化。

**具体方向**：

- `forge report` 子命令：从 `.forge/` 数据（trace + converge + checkpoint + scorecard）合成执行报告
  - 最新一次运行的概要：phase 列表 + 各 gate 结果 + 成本 + 持续时间 + 收敛状态
  - 文件变更摘要：新增/修改/删除文件数，按类型分组
  - 关键信号值：`RoadmapCompletion`, `GatesGreen`, `FileDelta`, `ReviewStatus`
- `forge report --diff <run-id-a> <run-id-b>`：比较两次运行的指标差异
  - gate 套数变化、成本变化、收敛速度变化
  - 这需要 RunID（方向二）作为前提
- `forge run --report` / `forge evolve --report`：运行完毕后自动输出报告（人类可读 + `--json`）
- 报告是**纯计算**、无副作用的：从已有的 `.forge/` 数据合成，不产生新的 trace/memory

**诚实边界**：方向三不要求实时进度（那是 v3 WebSocket/SSE 的范围）。也不要求持续监控（那是方向五的范围）。这是一个命令，你主动调用它，它读盘、计算、输出。复杂度可控，但依赖方向二的 RunID 作为跨运行关联键。可以在无 RunID 时退化为「最近一次运行」（按 checkpoint 的时间戳）。

**预估规模**: ~800 行（`cmd/forge/report.go` + 聚合逻辑 + 跨 diff 引擎 + `--json` 输出 + 测试）。

---

### 方向四 · 文件系统级相位隔离与原子性（Inter-Phase File System Isolation）

**当前状态**：所有相位（planner、implementer、reviewer、qa）共享同一个工作树。当一个 phase 编辑文件（使用 `acceptEdits`），它直接修改项目源代码。**forge-core 不提供任何机制来隔离、回滚或检测 phase 之间的文件系统冲突。**

```go
// forge-core/internal/orchestrator/command_executor.go — CommandExecutor.Dir
// 设置为 o.root（项目根目录），所有 phase 共享
// 无 per-phase 临时目录
// 无 pre-phase 快照
// 无 post-phase 变更清单（除了已实现的 phaseOutputLedger 做简单的情报传递）
```

```go
// forge-core/cmd/forge/main.go — defaultAgentAllowedTools:
// "Bash(node --test*) Bash(node harness/gate.mjs*)"
// 这些都是 READ-ONLY 验证器
// 但 phase 的写操作（edit/write）直接落到共享工作树
// 没有 per-phase branch/stash/snapshot
```

**当前防护**：
- `readonly` 声明在 Sprint 31 中通过 `--disallowedTools` 实现了写权限控制
- 四维安全护栏（recursion / budget / timeout / output-cap）防止资源失控
- `on_fail` loop-back 允许修复
- `phaseOutputLedger` 做前馈情报传递

但这些都是**流程/权限**层面的隔离，不是**文件系统**层面的隔离。

**为什么需要**：

这是一个**粒度问题**。当前防护可以防止「agent 永远不能写」或「agent 每个 phase 都能写全部」。但它无法防止：

1. **Phase A 和 Phase B 同时编辑同一个文件**（在 `RunParallel` 模式下）：两个 agent 的编辑静默覆盖彼此。当前 `RunParallel` 只保护了 in-memory 状态（agentCalls counter），不保护文件系统。
2. **Phase 意外修改了不应改的领域**：implementer 本应只改 `src/domain/`，但不小心改了 `harness/policies.yml`——当前只有 `readonly` 的 `--disallowedTools` 防护，但 app-level 写权限是粗粒度的。
3. **Phase 修改了工作树但之后被 REJECTED**：不成功的 phase 留下的修改仍然存在于工作树，没有自动回滚机制。用户需要自己 `git checkout`。
4. **Loop-back 修复积累了中间产物**：phase 失败、loop-back 重跑、再失败、再重跑——每轮都产生文件修改，但只保留最新一次的结果，中间状态的产物（如日志、备份）残留在工作树中。

**具体方向**（分层建议，从低到高）：

**Level 1（P1，~400 行）**：**Post-phase diff capture**。每个 agent phase 结束后，自动运行 `git diff --stat`（或 forge-core 内置的 stat-only diff，不依赖 git）生成该 phase 的变更清单，记录到 trace 事件中。现有 `phaseOutputLedger` 框架可复用。这不是隔离，是**可见性**——让你知道每个 phase 改了哪些文件。

**Level 2（P2，~600 行）**：**Per-phase Git 暂存/自动 stash**。在 agent phase 执行前自动 `git stash push --include-untracked --message "forge pre-phase <name>"`，执行后生成变更摘要，如果 phase FAIL 则自动 `git stash pop --index`（回到前一个 phase 的状态）。这是 Git 原生的原子性机制，forge-core 零额外状态，纯 CLI 指令。**诚实话**：这假设工作树是 git 仓库，且用户接受 automagic git operations。建议只在新 workflow phase 类型（`isolation: git`）中启用，不作为默认行为。

**Level 3（v3，north-star）**：**Firecracker microVM per sandbox phase**（已在 north-star 中规划，本文不展开）。

**诚实边界**：Level 2 的 git 操作必须可禁用（`--isolation none`），且必须保护 stash 堆栈不被耗尽（max stashes guard）。Level 1 的 diff capture 不需要 git——forge-core 可以在 run 前后统计工作树的文件元数据（大小、mtime、路径集合）并做集合 diff，纯 Go stdlib 可实现。

**预估规模**: L1 ~400 行，L2 ~600 行（主要在于 git CLI 交互 + 错误处理 + 测试 + 各种边界情况）。

---

### 方向五 · 自治运行的监督者模式（Supervisor / Watchdog 元进程）

**当前状态**：当 `forge evolve` 以 `--executor=command --agent-cmd=claude` 运行时，它成为一个长时间运行的自治进程。当前防护包括：

- 四维资源护栏（recursion / budget / timeout / output-cap）
- 信号感知 context（SIGINT/SIGTERM → cancel）
- Checkpoint/resume（断电恢复）
- `max-iter` + `no-progress tripwire` 防无限循环

但**没有进程级监督者**。如果 `forge` 进程自身 crash（OOM、panic、SIGKILL），没有 watchdog 重启它。如果运行中的工作流「挂起」（agent 进程存活但无输出 10 分钟），没有心跳超时。如果磁盘空间在运行期间耗尽，没有主动检测。

```go
// forge-core/internal/orchestrator/loop.go — LoopEngine.Run 的签名:
//   func (l LoopEngine) Run(wf asset.Workflow, mode string) (RunStats, error)
// 这是一个同步阻塞调用。调用者负责：
// - 进程生命周期（main goroutine）
// - 信号处理（signal.NotifyContext）
// - 资源回收（defer）
// 没有内置的 health check 端点
// 没有内置的 progress 轮询接口
// 没有内置的 watchdog 信号
```

**与已有分析的关系**：`strategic-production-gaps.md` 方向一讨论了「生产事故响应工作流」。`five-verifiable-code-level-gaps.md` 方向二讨论「运行时健康子系统」。`novel-five-perspectives-2026-07-10.md` 方向四涉及「无人值守运行时健康遥测」。这些分析的共同点是**从外面观察 forge**——给运维者提供健康指标。本方向关注的不是如何暴露健康状态，而是如何让 forge-core 在无人值守运行时**内部维持一个监督循环**——一个没有外部依赖的、进程内 watchdog。

**为什么需要**：

阅读 Sprint 24–26 的「真点火」记录（Sprint 25：「真 claude multi-agent 跑到 converge MET」；Sprint 26 发现并修了 8 个真实 gap），一个清晰的模式浮现：**每次真点火都暴露了 forge 在长时间自治运行下的脆弱性**——task 忘记注入、写权限缺失、模型路由未生效、trace latency 恒 0、reviewer 烧穿 budget。这些 gap 被发现了是因为有人类观察者在监控。一旦 forge 进入 24h 无人值守运行，「谁观察观察者」就成了元问题。

当前 forge-core 缺乏一个**自我维持的监督循环**：一个独立的 goroutine（或独立子进程），它的唯一职责是确保主循环的健康。

**具体方向**：

- **进程内监督者 goroutine**：`LoopEngine.Run` 启动时 spawn 一个监督 goroutine，在独立 ticker 上运行（默认 30 秒间隔）：
  - **进度心跳**：主循环每完成一个 phase `OnPhase` 回调心跳。监督者检查心跳间隔是否超过阈值（默认 5 分钟无新 phase → 判定 hung）。当判定 hung 时，监督者可选：① log warning ② 强制取消 context ③ 触发 checkpoint 后 exit(1)
  - **资源水位**：检查 `.forge/` 目录的磁盘使用量（`os.Stat` 周期扫描），超过阈值（如 1GB）发出警告。内置而不是依赖外部 disk monitor，因为无人值守场景不一定有 external monitoring
  - **子进程孤儿检测**：如果监督者发现主 context 已取消但 agent 子进程仍存活（`os.FindProcess` 探测），发送 SIGTERM 并记录（`command_executor_unix.go` 已有进程组管理，这是补全孤儿清理路径）

- **监督者的监督者（可选）**：如果 forge-core 作为 long-running daemon（非当前 CLI 模型），进程序列应为 `forge supervise -- forge run build --executor=command`。`forge supervise` 是极薄的 wrapper：它 spawn `forge run` 作为子进程，检测其退出代码——OOM 退出（exit code 137）→ 发出可区分的告警，不简单地「重启了事」。**诚实话**：daemon 模式是 v3 north-star，当前 CLI 模型下进程内监督者已足够。

- **`.forge/supervisor.log`**：监督者的专用日志文件，与 trace.jsonl 分离。Trace 是「系统发生什么」的结构化事件。Supervisor log 是「监督者看到什么」的运营日志——不会被 agent phase 的输出污染，即使在 trace 写入因磁盘满失败时，supervisor log 仍可通过 `sync` 写入单独的 fd。

**诚实边界**：

- 方向五不尝试「自我修复」：监督者不重启 phase、不重调度 agent、不修 .forge/ 文件。它的职责是**检测、记录、告警、可选终止**。修复是 operator/用户的责任。
- 方向五不解决「监督者自己挂了怎么办」——那是 v3 多进程/watchdog 的范畴（systemd/kubernetes liveness probe）。进程内 goroutine 的存活与主 goroutine 共存亡，其价值在于比主循环更早检测到异常（hung/livelock）、在 panic 时记录最后一口气。
- 监督者不读取 trace/memory/checkpoint 文件（避免与主循环的文件锁冲突）。它只使用 OS 级的信号（`os.ProcessState`、磁盘 stat、进程列表）和 forge-core 已有的回调（`OnPhase` 心跳）。

**预估规模**: ~500 行（监督者 goroutine + 3 类检查 + supervisor.log writer + `forge status` 整合 + 测试）。

---

## 优先级与收敛建议

| 方向 | 优先级 | 类别 | 前置依赖 | 预估规模 | 最高杠杆理由 |
|------|--------|------|----------|----------|-------------|
| **① ADR 可执行化** | P1 | 架构完整性 | 无（独立增量） | ~600 行 | 闭合「架构决策 > 代码」的核心论点——ADR 从文档变成驱动 |
| **② .forge/ 元数据护照** | P0 | 数据安全/运维 | 无（独立增量） | ~400 行 | 防止版本不兼容的数据损坏——所有后续方向的基础设施 |
| **③ 运行报告 + 跨 diff** | P1 | 产品/可观测性 | 依赖方向②(RunID) | ~800 行 | 让非 ForgeOS 开发者能用工具——产品化必须的「结果可见性」 |
| **④ 文件系统相位隔离** | P1 | 可靠性 | 无（L1 独立） | ~400~1200 行 | 阻止并行/失败 phase 无声地破坏工作树——自治运行的信任地基 |
| **⑤ 监督者模式** | P2 | 运维韧性 | 建议在方向③后 | ~500 行 | 24h 无人值守的保险——但非 8h 交互式使用的紧急需求 |

**核心建议**：

- **立即做方向②**（~400 行，P0）。它是数据基础：没有 .forge/ 的元数据护照，任何跨运行操作（diff、恢复、取证）都是徒劳的。这也是 Sprint 30 FUNCTIONAL_REQUIREMENTS_AUDIT 中已标注但未收口的「persistent storage format version」缺口的正交扩展——不修 version 字段本身，而是建立一个目录级的元数据层来兜底。

- **方向① 和 方向③ 可并行推进**（P1，互不依赖除方向②外的共同基础设施）。方向① 增强架构治理纵深，方向③ 增强产品体验。两者都不需要方向② 就可启动——方向① 是解析 `.agent/` 已有的 ADR 文件（无运行时状态依赖）；方向③ 可以从「仅最后一次运行」开始（无 RunID 退化模式）。

- **方向④ 的 L1（diff capture）可与方向③ 共享 diff 引擎**：方向③ 的 `forge report` 需要汇总文件变更；方向④ 的 post-phase diff capture 产出个体变更记录。两者应该共享同一个 diff/stat 工具函数包。建议方向③ 的 diff 引擎作为前提先做，方向④ 在其上构建 phase 级捕获。

- **方向⑤（监督者）建议放在最后**（P2）。它解决的是「已跑过多人次真点火、已基本稳定、准备进入 24h 无人值守」这个阶段的问题。当前 ForgeOS 的采纳状态是「已验证可行，但每次点火需 operator 授权」——监督者的价值在无人值守前夜才最大化。过早实现可能为还不存在的 hung 问题设计方案。

---

## 与已有 84+ 分析的差异化总结

本文件 5 个方向的共同特征是：**它们不增加 forge-core 的新能力边界，而是为已有的能力建立信任和可管理的基础设施。**

- **方向①** 让 ADR 从文档变成可执行的治理驱动——这是架构决策严谨性的垂直深化，不是新功能。
- **方向②** 给 `.forge/` 一个身份证——不做新事情，让已有事情可追溯。
- **方向③** 给已有运行结果一个消费界面——不改变 forge run 的行为，让产出可理解。
- **方向④** 给已有相位编排加上文件系统的安全带——不变编排语义，防止无声的破坏。
- **方向⑤** 给已有自治循环加一个健康守护——不改变循环逻辑，让监督有来源。

*本文基于 2026-07-10 工作树的完整代码扫描。所有代码引用均来自当前工作树的实际文件。与 84+ 份已有分析文档逐方向交叉验证，确认核心论点未被覆盖。*
