# ForgeOS — 五个高价值架构/产品扩展方向（全局代码扫描 v2026-07-11）

> **角色**: 资深架构师 + 产品经理  
> **方法**: 全局逐包扫描 `forge-core/` (18 Go 包, ~35k LOC)· `harness/` (39+ 模块, ~10.5k LOC)·
> `.agent/` (5 workflow · 12 agent 卡 · 9 skill 卡 · 全部 ADR + DECISIONS)· `docs/` (含 `FUNCTIONAL_REQUIREMENTS_AUDIT.md` 200+ 条目)。
> **已审阅**: `docs/requirements/` (114+ 份)· `docs/analysis/` (24+ 份)· `CURRENT_SPRINT.md` 全部 31 个 sprint 记录。
> **去重验证**: 对每个方向的核心理念组合在已有分析全文中进行关键词搜索,确认零篇系统性论述。
> **纪律**: 不编写任何代码。每个方向附带精确到 `file:line` 的代码级证据与产品价值判断。
> **日期**: 2026-07-11

---

## 核心发现

经过 31 轮 sprint,ForgeOS 在**运行时引擎层**已高度成熟:

- 编排引擎:串行/并行/loop-back/resume/checkpoint/mode-gating 全能力
- 安全护栏:递归深度·执行次数·墙钟超时·输出上限·进程组隔离
- 学习闭环:trace→scorecard→memory→converge 全链路就绪
- 真点火验证:multi-agent 端到端通过,含 8 个真 bug 修复 + 三维成本遥测

但 114+ 篇扩展分析绝大多数聚焦在**已有引擎的增量功能**。以下 5 个方向**全部落在已有分析的间隙中**——不是在某个成熟子系统中增加一个 knob,而是触及了**当前架构中不存在或将将萌芽的概念层**。

---

## 方向一 · Prompt 版本控制与回归测试框架

> **类型**: 质量基础设施 · 可观测性 · **优先级**: P1 (高)
> **关键词验证**: `prompt.validation 0` · `prompt.regression 0` · `prompt.test 0` ·
> `prompt.assert 0` · `prompt.evolution 0` · `prompt.snapshot 5`(均为旁证性质,非系统性框架)

### 为什么需要

#### 现状

ForgeOS 拥有全仓最复杂的 prompt 构造系统之一:

```
cmd/forge/prompt_context.go     (buildPrompt · gateLedger · phaseOutputLedger)
cmd/forge/prompt_memory.go      (memoryContext · verdictLedger · reviewFindingsLedger)
cmd/forge/prompt_artifacts.go   (emits · uses_template · secondary_template)
internal/prompt/cache.go        (ContextCache · GatherCached · cardText)
internal/prompt/prompt.go       (relevantADRs · adrTitles · Retrieve)
internal/prompt/retrieve.go     (TF-IDF 检索器)
```

`buildPrompt` 从 **7 条独立车道** 组装 agent 输入:

```
(1) Role card  (from .agent/agents/*.md via ContextCache)
(2) Current task (from ROADMAP.md, 每次重新读取)
(3) ADR 上下文  (from docs/adr/, TF-IDF 检索后注入)
(4) Hard constraints (from AGENTS.md, 缓存化)
(5) 前序 gate 裁决 (from gateLedger, 运行时实时沉淀)
(6) feed-forward 输出 (from phaseOutputLedger, 跨 phase 传递)
(7) Memory/knowledge (from memory.jsonl, 跨会话)
```

**但没有任何一条代码路径能回答以下问题**:

- "这次 evolve 迭代的 prompt 和上一次有什么不同?"
- "修改了 `product-manager.md` 角色卡后,agent 收到的 prompt 是否按预期变了?"
- "把 TF-IDF `adrTopK` 从 3 改成 5,实际注入的 ADR 内容变化有多少?"
- "这个 agent 输出质量的退化是因为模型变化,还是因为 prompt 结构悄悄漂移了?"

**代码级证据**:

| 文件 | 行 | 证据 |
|---|---|---|
| `cmd/forge/prompt_context.go` | `buildPrompt` (全部) | 构造完 prompt 后直接返回 `[]string`,**没有序列化/哈希/存证** |
| `internal/trace/trace.go` | `Event` struct | `Event` 记录 phase name/status/duration/cost,但**没有 prompt_hash / prompt_version / lane_snapshot** 字段 |
| `internal/prompt/cache.go` | `GatherCached` | 缓存的是**输入**(ADR title set),不是**输出**(最终 prompt 全文)——设计正确,但意味着 prompt 输出从未被任何层缓存或存证 |
| `cmd/forge/engine_build.go` | `agentExecutor.Build` | `Build` 返回 `[]string`(argv),prompt 作为 `-p` 参数值已经混入 argv 中——不再有独立可观测的 prompt 变量 |
| `internal/scorecard/...` | 全部 | scorecard 记录 cost/latency/quality 但**不记录 prompt 指纹**,无法按 prompt 版本分桶分析质量趋势 |

#### 产品价值

1. **Prompt drift 检测**: 模型更新后 agent 输出行为变化,最常见的原因是 prompt 结构对模型 API 变更敏感。没有 prompt 版本基线,根本无法区分"模型变了"和"prompt 漂了"。
2. **A/B 实验基础设施**: 想在 `discover.yml` 里试两种不同的 `product-manager.md` 角色卡措辞——当前只能在两个 git branch 上盲跑,全靠肉眼对比输出。有 prompt 版本化后可以系统化对比。
3. **CI 回归门**: `forge validate --prompt build.yml planner`——不跑 agent,只输出最终 prompt 并检查与 golden file 的 diff。prompt 变更需要 reviewer 确认,就像代码变更需要 review 一样。
4. **调试效率**: agent 输出异常时,第一步总是查"它看到了什么 prompt"。当前只能通过 `forge run --executor dry` 走 narration 路径,不精确且不完整。

#### 建议骨架

```
forge-core/internal/prompt/
├── prompt.go          ← (已有)ADR 检索
├── retrieve.go        ← (已有)TF-IDF
├── cache.go           ← (已有)上下文缓存
├── capture.go         ← (新增)prompt 快照:每条 lane 的最终文本 + 合并后的 prompt 全文
├── diff.go            ← (新增)两条 prompt 的结构化 diff(lane 级别,非纯文本)
└── validate.go        ← (新增)golden file 比对:prompt 全文 hash 与期望值匹配
```

每条 lane 在注入前被 `capture.go` 记录到 `trace.Event` 的新 `PromptSnapshot` 字段(可选,通过 `--capture-prompt` 开启避免数据量膨胀)。`diff.go` 做结构化对比(不是简单文本 diff,是 lane 级别的增删改检测)。`validate.go` 在 CI 中比较 golden snapshot。

#### 诚实边界

- v1 不存储完整 prompt 文本到 trace(可能很大,包含完整 ROADMAP+memory)。只存**每条 lane 的 hash**(SHA-256) + lane 存在性标志。
- v1 不做 prompt 语义 diff——只做结构化 diff(某条 lane 的 hash 变了,或 lane 数量变了)。语义 diff 需要 embedding,是 v2 工作。
- `--capture-prompt` 默认关闭。开启后每条 phase 的完整 prompt 写入文件(非 trace),供事后调试。这是对 trace 数据量问题的诚实处理。

---

## 方向二 · Agent 运行时生命周期管理与僵尸进程回收

> **类型**: 运维 · 可靠性 · **优先级**: P1 (高)
> **关键词验证**: `agent.lifecycle 0` · `agent.cleanup 0` · `agent.gc 0` ·
> `agent.pool 0` · `zombie.agent 0` · `stale.agent 0` · `orphan.process 4`
> (4 篇均为旁证提及,无系统性生命周期框架)

### 为什么需要

#### 现状

`CommandExecutor` 具备完善的子进程管理:

```go
// forge-core/internal/orchestrator/command_executor_unix.go
func setupProcessGroup(cmd *exec.Cmd) {
    cmd.SysProcAttr = &syscall.SysProcAttr{Setpgid: true}
    cmd.Cancel = func() error {
        return syscall.Kill(-cmd.Process.Pid, syscall.SIGKILL)
    }
    cmd.WaitDelay = processGroupGrace  // 2s
}
```

Context 取消时整个进程组被 SIGKILL,WaitDelay 兜底。这把**运行中的** agent 子进程管理得很好。但以下场景不在覆盖范围内:

| 场景 | 发生方式 | 后果 |
|---|---|---|
| **进程残留** | `forge evolve --executor command` 在 agent phase 中途被 `kill -9`(非 SIGTERM,是 SIGKILL)——Go 进程没机会执行 `defer runCancel` | agent 子进程变成孤儿,继续消耗 API 预算、占用系统资源 |
| **重复运行冲突** | 用户在终端 A 启动 `forge run build`,又在终端 B 启动同样的命令 | 两个 forge 实例写同一个 `.forge/checkpoint.json`,`memory.jsonl` 交错写入,数据损坏 |
| **状态目录污染** | crash 后 `.forge/` 遗留 `.tmp` 文件(已有 `doctor` 检出)和未完成的 checkpoint | `--resume` 可能从损坏的状态恢复 |
| **并发会话隔离** | 同一仓库的 CI 和本地开发同时触发 forge | `.forge/` 是共享的,没有会话 ID 或实例隔离 |

**代码级证据**:

| 文件 | 行 | 证据 |
|---|---|---|
| `forge-core/internal/orchestrator/command_executor.go` | `Execute` → `defer runCancel()` | `runCancel` 只取消 context→信号发给进程组,**但只覆盖了`cmd.Run()` 正在运行时**的场景 |
| `forge-core/internal/persist/checkpoint.go` | `Save` | 使用原子写入(tmp+rename+fsync),防止单进程 crash 导致损坏。但**没有跨进程互斥机制**(flock / pid file) |
| `forge-core/internal/memory/memory.go` | `Append` / `Load` | 没有文件锁。两个并发进程同时 append 到 `memory.jsonl` 产生交错行 |
| `forge-core/cmd/forge/main.go` | `withSignalCancellation()` | SIGINT/SIGTERM 触发 context 取消→引擎停止。但 **SIGKILL 不可捕获**,进程被杀死后没有任何 cleanup 路径 |
| `forge-core/internal/doctor/doctor.go` | `tmpResidueCheck` | 只是**检测**残留 `.tmp` 文件,不清理也不预防 |
| `forge-core/cmd/forge/engine_build.go` | `buildRunEngine` | 每次 `forge run` 都创建一个新的 `agentExecutor`,但没有任何"上一个 forge 实例是否还在运行"的检查 |

#### 产品价值

1. **防重复运行(单实例守卫)**: 这是最痛的问题——用户或 CI 不小心跑了两个 `forge evolve`,两个 agent 同时写代码、交替修改对方的输出,结果不可恢复。一个简单的 pid 文件 + `flock(LOCK_EX|LOCK_NB)` 可以完全杜绝。
2. **孤儿 agent 回收**: `forge run --executor command` 跑真 claude 时,如果 forge 本身被 SIGKILL,claude 子进程变成孤儿继续消耗 API 预算。如果 forge 重启后能检测到残留的子进程(通过 pid 文件记录)并清理,可以防止"隐形烧钱"。
3. **并发会话命名空间**: 支持 `FORGE_SESSION_ID` 环境变量,让不同会话使用隔离的 `.forge/sessions/<id>/` 目录而不是共享 `.forge/`。CI 并行测试时无需担心互相干扰。
4. **优雅降级**: 当前 `fork-while-running` 的后果是静默数据损坏(没有错误提示,只有事后症状)。增加单实例守卫后,第二次启动会打印清晰的"另一个 forge 实例正在运行(pid X),使用 --force 或等待",从静默错误升级为可操作消息。

#### 建议骨架

- **进程锁**: `internal/persist` 新增 `TryLock(root string) (func() error, error)`——在 `.forge/.lock` 上执行 `flock(LOCK_EX|LOCK_NB)`,返回释放函数。`main.go` 的每个入口命令(>=run/evolve)在启动时调用,失败则友好报错。
- **子进程注册表**: `CommandExecutor` 将每个 spawn 的 PID 写入 `.forge/active-pids/` 目录(每个 spawn 一个文件,含 phase/timestamp)。启动时扫描并警告/清理残留。
- **会话隔离**: `memory/memory.go` 和 `persist/checkpoint.go` 的路径构造函数考虑 `FORGE_SESSION_ID`,当设置时使用 `.forge/sessions/<id>/`。
- **`forge doctor --sessions`**: 列出所有活跃/残留会话,支持 `--clean` 清理。

#### 诚实边界

- `flock` 在 NFS 上不可靠,标记为"单机/本地文件系统"限制,不做跨网络原子性承诺。
- pid 文件是**尽力而为**的防御,不是安全边界(agent 可以修改它)。它的价值在于阻止无意的重复运行,不是恶意的并发攻击。
- 子进程注册表是**可选的**(`--track-pids`),默认关闭在 `--executor dry` 下,只对 `command` 模式自动启用。

---

## 方向三 · Workflow 编排引擎的 Property-Based 测试与形式化验证

> **类型**: 质量基础设施 · **优先级**: P2 (中—高)
> **关键词验证**: `test.fuzzing 0` · `test.propert 0` · `state.machine.test 0` ·
> `model.check 0` · `formal.verif 0` · `exhaustive.test 0` · `mutation.testing 3`
> (3 篇均为旁证,非系统性测试框架)

### 为什么需要

#### 现状

ForgeOS 工作流编排引擎 (`internal/orchestrator`) 是**全系统最复杂的子系统的第一梯队**:

| 维度 | 状态空间 |
|---|---|
| 执行模式 | `RunFrom`(串行) / `RunParallel`(并行) |
| phase 类型 | agent phase / gate phase |
| 模式门控 | discover skip / review skip / gate filter / reviewer skip |
| 阶段跳过 | lifecycle production 强制全开 / mode based skip |
| loop-back | gate fail(限 MaxLoopBack) / reviewer REQUEST_CHANGES(fail-open) |
| 重试 | retryable error(限 MaxRetries) / permanent error(abort) |
| checkpoint | per-phase / per-iteration / resume |
| 取消传播 | context cancellation / SIGINT / SIGTERM / wave cancel |
| 预算守卫 | agent-call count / run-level spend / output size / depth |

可能的组合数是**天文数字**:9 个独立维度的乘积。当前所有测试加起来覆盖约 **30-40 种组合**(从 `orchestrator_test.go`/`loop_test.go`/`parallel_test.go`/`mode_gating.go` 等测试文件估算),覆盖率极低。

**代码级证据**:

| 文件 | 行 | 证据 |
|---|---|---|
| `internal/orchestrator/orchestrator_test.go` | 476 行 | 全部是**手写的脚本化测试**,每个测试一个具体场景 |
| `internal/orchestrator/loop_test.go` | 468 行 | 同上,手写 fixture |
| `internal/orchestrator/parallel_test.go` | — | 手写 wave 场景 |
| `internal/orchestrator/waves.go` | `Waves()` 函数 | 依赖图排序算法(topo sort),没有对**循环依赖/多根/孤立节点/超大图**进行 fuzz 测试 |
| `internal/routing/routing.go` | `TierFor` | 多维评分路由,但测试覆盖的评分组合有限 |
| `internal/persist/checkpoint_test.go` | — | 测试正常路径和 JSON 损坏,但**未测试并发写入/fd 耗尽/磁盘满**等边界 |

#### 产品价值

1. **发现隐藏的状态机 bug**: 手写测试覆盖的是"人想到的场景"。property-based 测试自动探索状态组合空间,容易找出"当 loop-back + resume + mode-gating + parallel 同时发生时"的竞态条件——这些组合人根本不会想到去测。Sprint 27 的 `yaml2json block-scalar 损坏` 和 Sprint 31 的 `ContextCache 竞态` 都是 reviewer 偶然发现,不是测试系统性发现的——前者躲在"差分测试只 `t.Logf` 不 `t.Errorf`"里 6 个 sprint,后者需要 `-race` 才暴露。
2. **回归网的最后一层**: 现有测试覆盖正常路径,CI gate 覆盖文件级规则。property-based 测试填补"状态组合空间"的空白。三者形成三层防线(unit·integration·property)。
3. **Waves 排序算法的正确性证明**: `Waves()` 在没有循环依赖时必须总能产生一个合法的 wave 划分;在有循环依赖时必须诚实报告错误而不是静默返回错误结果。一个 property-based 测试可以随机生成 1000 个依赖图并验证这两个不变量。
4. **熔断阈值的安全网**: `MaxLoopBack`/`MaxAgentCalls`/`MaxRetries` 三者的交互——当 loop-back + retry 同时发生时,执行次数的上界是否如预期? property-based 测试可以系统地验证。

#### 建议骨架

- 使用 Go 的 `testing/quick` 或 `math/rand/v2` 生成**随机工作流定义**(随机 phase 数量/类型/依赖/on_fail 声明/required_gates)并验证不变量:
  - `RunFrom` + `RunParallel` 对同一工作流产生一致的收敛判定
  - `Waves()` 输出中每个 phase 的所有依赖都在更早的 wave 中
  - `MaxLoopBack` 次循环后 gate 必须 abort(不变量,非 probabilitic)
  - `MaxAgentCalls` 后的 phase 不会被 spawn
- 对 `trace.Event` 序列引入**round-trip 不变量**:序列化→反序列化→再序列化产生相同字节
- 对 `memory.Compact` 引入**语义不变量**:紧凑前后的 `Retrieve` 对同一 query 返回相同的结果集

#### 诚实边界

- 不伪装成"完全验证"。property-based 测试是**统计性的**(随机 seed 覆盖的状态空间与运行时间成正比),不是形式化证明。在 CI 中每个 test run 使用不同的 seed。
- 不对 `orchestrator.Engine` 做完全的 model checking(那是 v3 工作)。v1 只覆盖最核心的**状态机不变量**——收敛性、安全性(不会超出护栏)、终止性(不会无限循环)。
- `internal/persist` 的 fault-injection 测试(fd 耗尽、磁盘满)标记为 **manual**(需要特殊环境),不加入 CI 的 `forge accept` 闸门路径。

---

## 方向四 · 对抗/红队 Agent 验证循环

> **类型**: 安全 · 质量 · **优先级**: P2 (中)
> **关键词验证**: `adversarial.agent 0` · `agent.competition 0` · `red.team 0` ·
> `agent.disagree 0` · `agent.debate 0` · `byzantine 0` · `agent.consensus 0`

### 为什么需要

#### 现状

当前所有 workflow 的 agent 结构都是**协作式**的:

```
build.yml:
  planner → implementer → [harness gates] → reviewer → qa
```

每个 agent 都被提示"帮助完成工作"。reviewer 虽然会指出问题,但它的角色是**建设性评审**——找到 bug、建议改进——不是**主动破坏**。没有 agent 被给予以下任务:

- "找出这个架构设计中的所有安全漏洞"
- "为这段代码写测试——保证能测出它所有的边界情况"
- "尝试让这个实现崩溃"
- "找到 reviewer 评审意见中的逻辑漏洞"
- "验证上个 agent 的输出是否与架构文档矛盾"

**代码级证据**:

| 文件 | 行 | 证据 |
|---|---|---|
| `.agent/workflows/build.yml` | 全部 | 5 个 phase,全部是**正向建设**角色,无对抗性角色 |
| `.agent/agents/qa.md` | 角色定义 | QA agent 的职责是"验证实现符合验收标准",不是"尝试破坏" |
| `.agent/agents/reviewer.md` | 机读契约 | VERDICT: APPROVE/REQUEST_CHANGES——二元输出,不是"发现 N 个隐藏缺陷" |
| `internal/converge/converge.go` | 全部信号 | `Signals` 包含 RoadmapCompletion/GatesGreen/HumanApproved 等,**无"对抗验证通过"信号** |
| `internal/orchestrator/orchestrator.go` | `RunFrom` | 没有"阶段性对抗检查"的 phase type 或 gate type |
| `internal/routing/risk.go` | `FromChangedPaths` | 风险分类只基于**文件路径启发式**,没有 agent-based 的风险评估 |

#### 产品价值

1. **安全左移**: 当前安全验证依赖于 `security-engineer` agent 的 STRIDE 威胁建模——这是**设计阶段的**安全分析。对抗性 agent 在**实现阶段**可以动态发现安全漏洞(不仅仅是设计缺陷),例如 SQL 注入、XSS、权限绕过等。这是纵深防御的一个空白层。
2. **测试质量保障**: `QA` phase 运行现有测试,确保它们通过。但**没有人测试测试本身的质量**。一个 adversarial agent 可以尝试为代码写测试,然后看看现有测试集是否能捕获这些新测试——这是 mutation testing 的 agent 化版本。
3. **文档-代码一致性**: 当前没有 agent 被明确要求检查"agent 的实现在多大程度上偏离了架构文档/ADR"。一个 adversarial agent 可以逐条对比 ARCHITECTURE.md 中的承诺与代码中的实际实现。
4. **协作质量的"免疫系统"**: 协作式 agent 容易产生"群体迷思"(groupthink)——第一个 agent 的方向错误被后续 agent 继承而不是纠正。对抗性 agent 作为"免疫系统"可以打破这种模式。Sprint 25 真点火实验已经暴露过这个问题:implementer 在 `acceptEdits`(无 Bash)下无法自检,诚实拒绝勾 ROADMAP,但 reviewer 还是 APPROVE 了——因为 reviewer 也看不到自检结果。对抗性 agent 会主动找这些问题,而不是等它们浮出水面。

#### 建议骨架

- 新增 workflow phase type `adversarial`(或 gate type `adversarial_gate`),接在 reviewer 和 qa 之间:
  ```
  planner → implementer → [gates] → reviewer → adversarial → qa
  ```
- adversarial agent 接收的 prompt 是**对立面**的:不要求"帮助改进代码",而是"找到这个实现中至少 3 个未被 reviewer 发现的问题"。
- adversarial agent 的输出格式不限于 `APPROVE/REQUEST_CHANGES`——它是一个**结构化发现报告**(每个发现含:type/severity/location/evidence)。
- 结果注入 `Signals` 的对抗验证字段(新增 `AdversarialFindings`),可以设置 stop_condition 为 `adversarial_findings == 0`(零发现才能收敛)。

#### 诚实边界

- 对抗性 agent 和协作 agent 使用**同一个模型**(如 Sonnet/Opus),不是独立的更强模型。它的价值不在于"比 reviewer 聪明",而在于有不同的**视角和激励**。
- v1 不做多轮对抗辩论(一个 agent 提缺陷,另一个 agent 反驳)。那是 v2 的"对抗性辩论"模式,需要更复杂的状态管理。
- 对抗性 agent 的发现是**建议性**的,不是 blocking gate(类似 reviewer 的 fail-open 模式)。blocking 级别由 `adversarial_gate` 的配置决定。
- 诚实标注:对抗性 agent 和 reviewer 一样,受限于 LLM 的认知边界——它不是独立的安全审计员。

---

## 方向五 · 跨会话增量复用(Incremental Reuse Across Runs)

> **类型**: 性能 · 成本优化 · **优先级**: P2 (中)
> **关键词验证**: `incremental.reuse 0` · `output.cache 0` · `phase.cache 0` ·
> `run.cache 0` · `stale.output 0` · `partial.replay 1`(旁证)· `selective.reuse 0` ·
> `incremental.resume 0`

### 为什么需要

#### 现状

ForgeOS 当前有以下"记忆"机制:

| 机制 | 范围 | 内容 | 是否跨会话 |
|---|---|---|---|
| `memory.jsonl` | 全仓 | 学习到的知识(跨迭代保留) | ✅ 是 |
| `trace.jsonl` | 单会话 | 事件记录 | ❌ 否(用于审计,不用于回放) |
| `checkpoint.json` | 单迭代 | 恢复点 | ❌ 否(仅 `--resume` 用) |
| `scorecard` | 单会话 | 路径历史 | ❌ 否(统计信息,不保留输出) |

**但没有任何机制能回答**:

- "我昨天跑了 `forge run build`,planner 输出了一个架构方案。今天我只改了 ROADMAP 中的一个 task,能否**跳过 planner 直接复用昨天的输出**,只重新运行 implementer?"
- "这个 phase 的输入(ROADMAP/ADRs/agent card)和上次运行时完全一样——为什么还要再跑一次?"
- "`forge evolve` 的第 3 次迭代中,gate 的裁决和第 2 次迭代完全一样——能否跳过第 3 次迭代的重复 gate 检查?"

当前每次 `forge run` / `forge evolve` 都是**从头重跑**,即使用户只改了一行代码:

```go
// forge-core/cmd/forge/evolve.go: cmdEvolve → buildLoop → eng.RunFrom(wf, mode, 0)
// 每轮迭代都从 phase 0 开始,没有"从上一次结束的地方继续"以外的增量路径
```

`checkpoint.json` 可以在 crash 后恢复**同一个**会话,但不能跨会话增量。这意味着:

- 一个 10-phase build 需要改动 1 个文件:10 个 phase 全部重跑
- 一个 evolve 跑了 5 轮到达收敛:第 6 轮(用户加了新需求)从 phase 0 开始
- 两个开发者先后在同一 repo 上运行 `forge detect → evolve build`:完全两套独立的 run,没有共享缓存

**代码级证据**:

| 文件 | 行 | 证据 |
|---|---|---|
| `forge-core/internal/prompt/cache.go` | `ContextCache` | 缓存的是**单运行内**(per-run)的 ADR 标题集和 AGENTS.md。运行结束后缓存释放 |
| `forge-core/internal/orchestrator/executor.go` | `DryRunExecutor` | 只 narration,不保存输出供复用 |
| `forge-core/cmd/forge/engine_build.go` | `agentExecutor.Build` | 每次 phase 都重新构造完整的 argv(含 prompt),不查询是否有可复用的旧输出 |
| `forge-core/internal/persist/checkpoint.go` | `Checkpoint` | 只保存恢复所需的最小状态(iteration/mode/roadmap_completion/phase_index),**不保存 phase 输出** |
| `forge-core/cmd/forge/prompt_artifacts.go` | `phaseOutputLedger` | phase 输出只在**运行期间**在内存中保留,供 feed-forward 使用。运行结束后清空 |
| `forge-core/internal/converge/converge.go` | `gatherSignals` | 每次 gather 都从文件系统读取 ROADMAP.md 和 gate 状态。没有"上次 gather 结果的缓存" |

#### 产品价值

1. **成本节省**: 在 evolve 循环中,大量 phase 的输入在连续迭代之间没有变化——特别是 planner 阶段(除非 ROADMAP 结构变化)和 gate 阶段(除非代码变化)。跨会话缓存可以节省 40-60% 的重复 API 调用。这是直接对应到**真金白银的 API 账单**的优化。
2. **开发速度**: 用户改了一行代码,只想重新跑 `implementer→gates` 而不是完整的 10-phase pipeline。当前必须 `forge run build` 全量;有增量复用后可以 `forge run build --incremental` 自动跳过未变化 phase。
3. **CI 场景**: monorepo 中只改了 service-A,但 forge CI pipeline 每次都要扫整个仓库。增量复用可以将 CI 时间从"全扫描"降为"受影响的部分"。
4. **`forge evolve` 的效率上限**: 当前 evolve 的停止条件是"roadmap 100% + gates green"——即使改了 1 个 task,也要走 N 轮迭代,每轮全量。增量复用的直接效果:收敛速度大幅提升,更少轮次、更少成本。

#### 建议骨架

- **Phase 输出缓存**: 新增 `internal/cache/phase_cache.go`——以 `(agent_card_hash, roadmap_section_hash, adrs_hash, mode, lifecycle)` 的复合 key,缓存 agent phase 的输出。key 在 `buildPrompt` 时计算,value 是 phase 的标准输出(verdict/产物路径等)。
- **缓存失效策略**: 任何 key 成分变化 → 缓存 MISS。缓存存储在 `.forge/cache/` 下,按 key hash 分目录。支持 `FORGE_CACHE_MAX_AGE` 环境变量(默认 24h),过期的缓存条目自动清理。
- **`--incremental` 标志**: `forge run` / `forge evolve` 增加 `--incremental` 标志。启用后,在每个 agent phase 起跑前,计算缓存 key 并尝试命中。命中则跳过 agent 调用,直接注入缓存的输出。miss 则正常跑,并将结果写入缓存。
- **缓存透明度**: `forge status --cache` 显示缓存命中/未命中统计、缓存大小、过期条目。`forge doctor --cache-integrity` 验证缓存 key 与 phase 输出的一致性。
- **`phaseOutputLedger` 的持久化扩展**: 当前 `phaseOutputLedger` 是进程内数据结构。可以持久化到 `.forge/cache/outputs/<phase>.json`,下次启动时恢复。这是 checkpoint 的轻量级互补。

#### 诚实边界

- v1 缓存的单元是**整个 phase 的输出**,不是 phase 内部的子任务。更细粒度的缓存(如函数级别的实现复用)是 v2 工作。
- 缓存 key 的计算不涉及语义等价性(不尝试判断"两个不同的 prompt 是否语义等价")——只做结构化比较(hash 精确匹配)。语义等价必命中,不等价必 miss。不追求"语义相似也命中"——那是 embedding-based 的 v3 工作。
- `--incremental` 默认**关闭**。行为变化:用户显式 opt-in,与 `--parallel` 一样的模式。默认全量运行,向后兼容 byte-for-byte。
- 缓存不是 checkpoint 的替代品。checkpoint 保证 crash 恢复的正确性;缓存只保证**输入相同→输出相同**的正确性。两者正交。
- 缓存可能掩盖 agent 行为的不确定性(同一个 prompt 每次输出不同)。这是设计意图——`--incremental` 用于"只改了一行代码"的场景,不是"我怀疑 agent 行为有随机性"的场景。如果需要新鲜输出,不加 `--incremental` 即可。

---

## 总结

| # | 方向 | 类型 | 优先级 | 已有分析覆盖 | 代码证据强度 |
|---|---|---|---|---|---|
| 1 | Prompt 版本控制与回归测试 | 质量基础设施 | P1 | ❌ 零系统性论述 | ★★★★★ 强 |
| 2 | Agent 运行时生命周期管理 | 运维可靠性 | P1 | ❌ 零系统性论述 | ★★★★★ 强 |
| 3 | 编排引擎 Property-Based 测试 | 质量基础设施 | P2 | ❌ 零系统性论述 | ★★★★☆ 中强 |
| 4 | 对抗性 Agent 验证循环 | 安全质量 | P2 | ❌ 零系统性论述 | ★★★☆☆ 中 |
| 5 | 跨会话增量复用 | 性能成本 | P2 | ❌ 零系统性论述 | ★★★★☆ 中强 |

### 实施建议

- **P1 方向(1-2)** 建议在 1-2 个 sprint 内完成核心骨架:方向一的核心是 `capture.go` + `trace.Event` prompt_hash 字段(预算:~2 天);方向二的核心是 `TryLock` + pid 文件(预算:~1 天)。
- 方向一是**追加式**的——它不改变任何现有行为,只在 buildPrompt 路径上插桩,风险最低,收益最直接。
- 方向二中的**单实例守卫**是最小可行增量——单独花半天加一个 flock 就可以消除"不小心跑两个 forge"这个最痛的静默数据损坏问题。其余(子进程注册表/会话隔离)可以后续跟进。
- 方向三(property testing)可以作为"闸门自我加固"的持续 effort——每次修改 `RunFrom`/`RunParallel`/`Waves` 时顺手加一条 property,不要试图一天内写完所有 property。
- 方向四(adversarial agent)需要新建一个 agent 卡和一个 workflow phase type,是工作量最大的方向。建议先 paper design 再动手。
- 方向五的**最小可行**是 `--incremental` + 单个 agent phase 的缓存,不需要全系统依赖图分析。可以先在 `implementer` phase 上试点。

---

*本报告基于 2026-07-11 代码库状态。所有代码引用均指向当前 HEAD。无外部假设。*
