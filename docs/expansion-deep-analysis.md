# ForgeOS 高价值扩展方向 — 全局深度扫描

> 基于代码库全局扫描 (`forge-core` 13 包 + `harness` 全套 + `.agent` 完整骨架 + `examples`),
> 以资深架构师/产品经理视角分析 **当前实现边界以外** 的核心扩展方向、Edge cases 与性能优化点。
>
> 前置条件:已完整阅读 BOOTSTRAP、PROJECT、ARCHITECTURE、ROADMAP、CURRENT_SPRINT、AGENTS 及全部关键代码。
> **引用依据为实际代码而非文档声明。**

---

## 目录

1. [方向一:Agent-Runtime 执行层 — 当前最大「空转」缺口](#1-agent-runtime-执行层)
2. [方向二:多厂商模型池 + 自适应路由 — 承诺但未兑现的供应商中立](#2-多厂商模型池--自适应路由)
3. [方向三:载重墙/沙箱执行 — 自治 agent 的唯一安全地基](#3-载重墙沙箱执行)
4. [方向四:工作流动态派生与自适应编排 — 从静态 YAML 到活的工作流](#4-工作流动态派生与自适应编排)
5. [方向五:知识引擎 + 语义检索 — 从 TF-IDF 标题匹配到真正的 RAG](#5-知识引擎--语义检索)
6. [六个重要 Edge Case / 性能优化](#6-edge-cases--性能优化)
7. [总结优先级矩阵](#7-优先级矩阵)

---

## 1. Agent-Runtime 执行层

### 现状分析

当前 `forge-core` 的 agent 执行层是 `orchestrator.AgentExecutor` 接口,只提供两个实现:

| 实现 | 行为 |
|---|---|
| **DryRunExecutor** | (`executor.go:48-59`) 只打印路由 tier,不调 LLM。 |
| **CommandExecutor** | (`command_executor.go`) shell 出 `claude -p <prompt>` 并捕获 stdout。 |

`CommandExecutor` 的实现本质是一个**薄 shell 包装器**:它把 prompt 字符串传给 `claude` CLI,
读取其 JSON 输出,解析 `total_cost_usd`,然后返回。**没有中间步骤的可见性或控制力。**

### 根本缺口

真实 agent 执行不是一个单次 API 调用——它是一个**多步工具使用循环**:

```
	 prompt → [LLM] → 思考/工具调用 → [结果] → [LLM] → 工具调用 ...
```

当前的 `claude -p` 模式(print-mode)是**一步完成**:

- agent 在单次 LLM 调用内输出所有思考和代码,然后退出。
- 如果 agent 需要运行 `node --test` 来验证,它必须通过 `--allowedTools` 获得 Bash 权限。
- **但如果测试失败,agent 已经没有第二轮调用来修复了**——print-mode 是一次性的。

架构文档声明的引擎列表 (`ARCHITECTURE.md`) 包括:

> Agent-Runtime · Knowledge-Engine · Sandbox(载重墙) · Web-UI

Agent-Runtime **是五个声明但未实现的引擎之一**。`forge-core` 目前只实现了
Orchestrator · Model-Router(部分) · Context-Engine(部分) · Memory-Engine · Evaluation-Engine。

### 高价值原因

1. **工具使用生命周期管理**:允许 agent 声明式地请求工具调用,由运行时管控、执行、返回结果,
   而非依赖 `--allowedTools` 的白名单授权。这是 AGENTS.md 工程红线中「绝不写上帝文件」的执行保障。

2. **自我纠错循环**:测试失败后自动触发修复-重试循环,无需依赖 `orchestrator.backoff.go`
   的纯延迟重试。当前 loop-back 是 workflow 级别的(跳回 implementer phase),粒度太粗。

3. **上下文窗口管理**:多轮工具调用的累积 token 需要策略性缩减/摘要。当前无任何窗口管理,
   典型 `claude -p` 单轮输出即结束。

4. **成本可观测性**:每个工具调用 step 而非每个 phase 归因成本。目前 `cost.go` 按 phase 归因,
   但一个 phase 内可能发生多次 LLM 调用(迭代式修复),当前全部合并为一行 `total_cost_usd`。

5. **安全边界**:工具调用级别控制——允许 agent 读文件但禁止写,允许多个 `node --test` 但禁止网络。
   当前 shell 出 `claude -p` 后在 agent 侧配置 `--allowedTools`,运行时无中间 enforcement。

### 实现线索

代码中已有一些先兆:
- `orchestrator/command_executor.go` 的 `ClassifyOverload` / `RenderLog` 回调机制(`command_executor.go:121-148`),
  正是为未来 agent-runtime 预留的注入点。
- `prompt_context.go` 的 `observeFor` 函数已经是一个「中间结果处理管道」的雏形。
- Harness 层的 `gate.mjs`、`secret-scan.mjs` 等已经是独立可执行工具——agent-runtime
  只需要把它们注册为可用工具,而非靠 agent 自己记住要跑哪些命令。

### Edge Cases 警示

- **⚡ 分叉风险**:Agent-Runtime 的实现极易膨胀为「微型 Kubernetes operator」——必须紧守
  forge-core 零依赖纪律,保持纯标准库。
- **⚡ prompt 注入升级**:工具返回结果若包含恶意内容,agent-runtime 既要注入给 LLM 又要
  防止其污染后续调用。当前 `sanitizeAgentOutput` (`prompt_context.go:265-278`) 仅做基本
  Unicode 清理,不防语义注入。
- **⚡ 死循环防护**:工具调用循环可能无限自我修复。当前 `MaxRetries` 是粗粒度计数器;
  未来 agent-runtime 需要按「修复步数×修复模式」独立计数。

---

## 2. 多厂商模型池 + 自适应路由

### 现状分析

`forge-core/internal/routing/` 包有清晰的架构分层:

| 组件 | 文件 | 状态 |
|---|---|---|
| Tier 分配 | `routing.go:29-49` | ✅ 工作(Haiku/Sonnet/Opus) |
| 多维度评分 | `routing.go:86-210` | ✅ 工作(Score + TierForScore) |
| 预算感知降级 | `routing.go:220-283` | ✅ 工作(BudgetAdjustTier) |
| 学习循环回灌 | `routing.go:285-378` | ✅ 工作(HistoryTiebreak) |
| **跨厂商模型映射** | `routing.go:383-410` | ❌ 骨架(仅 Anthropic) |
| **厂商自动故障转移** | (不存在) | ❌ 未实现 |
| **成本优化路由** | (不存在) | ❌ 未实现 |

`ModelMap` (`routing.go:389-394`) 的核心:

```go
var ModelMap = map[string]map[string]string{
	"anthropic": {
		Haiku:  "claude-sonnet-4-haiku",
		Sonnet: "claude-sonnet-4",
		Opus:   "claude-opus-4",
	},
}
```

**一个厂商,三个模型,硬编码。**

`ResolveModel` (`routing.go:398-409`) 和 `cmd/forge/route.go` 的 `--scorecard` 路径
都假设「厂商 = anthropic」是唯一选项。`CommandExecutor` 的 `Build` 函数硬编码 `claude` 命令。

### 根本缺口

1. **无厂商抽象层**:`engine_build.go:89-106` (`agentExecutor`) 的 `isClaude := strings.Contains(o.agentCmd, "claude")`
   直接将 agent-cmd 包含 "claude" 作为判断标准——这是 vendor 判断的字符串匹配 hack。

2. **无提示词格式化路由**:不同模型需要不同的提示词格式(OpenAI 的 chat template、Gemini 的
   system instruction、Claude 的 XML tags)。当前只有 `prompt.Build` 一种格式。

3. **无自动故障转移**:若 Claude API 返回 529 overloaded,当前 `backoff.go` 做指数退避重试
   同厂商。更优方案是自动 fallback 到备选厂商(Gemini / OpenAI)。

4. **无成本感知路由选择**:`routing.BudgetAdjustTier` 只做 tier 间降级(Sonnet→Haiku),
   但从不在厂商间选择(如 Sonnet 在 Anthropic 比 Gemini Pro 贵但质量相近时自动选便宜的)。

5. **无区域/延迟路由**:若用户在欧洲,自动路由到欧洲可用的模型端点。

### Edge Cases 警示

- **⚡ 提示词泄漏**:向非 Anthropic 厂商发送带 `[context:gate-results]` 的 prompt 可能
  泄漏 ForgeOS 内部标记。需要 prompt 剥离/重写层。
- **⚡ 评分不可比**:`HistoryTiebreak` 基于 scorecard 的历史 quality_score,但不同厂商的
  quality_score 不跨厂商可比——Sonnet 的 0.9 和 GPT-4o 的 0.9 不是同一标尺。
- **⚡ 厂商退出/API 变更**:硬编码的 `total_cost_usd` JSON 路径(`cmd/forge/cost.go`)
  依赖 claude `--output-format json` 的具体 schema,切换厂商需解析完全不同格式。

---

## 3. 载重墙/沙箱执行

### 现状分析

当前 `forge accept`和 `forge gate` 的所有检查、`forge run --executor=command` 的 agent
均在**宿主机的同一用户空间**运行。具体风险:

| 风险 | 代码路径 | 影响 |
|---|---|---|
| Agent 写任意文件 | `command_executor.go:93` → `claude --permission-mode acceptEdits` | agent 可覆盖 `.agent/AGENTS.md` 等治理文件 |
| 读取宿主机 secret | `claude --allowedTools` 若含 Bash | agent 可 `cat ~/.ssh/id_rsa` |
| fork-bomb 攻击 | `command_executor.go` 的 `FORGE_AGENT_DEPTH` guard | 仅计数,不隔离,恶意 agent 可 fork 其他进程绕过 CC 计数 |
| 持久化后门 | 无沙箱 | agent 可写 crontab / systemd service |

架构文档明确说:

> **真相之源 = 带外执法层**(Sandbox / CI runner 跑 harness 闸门),host-independent。
> …
> Gateway · **Agent-Runtime** · Knowledge-Engine · **Sandbox(载重墙)** · Web-UI

**Sandbox(载重墙)是七个引擎之一,目前为零实现。**

### 为什么现在需要

1. **真点火已坐实**:Sprint 24-26真实 `claude --executor=command` 已端到端跑通——现在是
   真 LLM 在 autonomously 写代码、跑测试、接受 gate 裁决。**无沙箱意味着每一次 `forge evolve` 都给予 agent 等同开发者的代码库写权限。**

2. **防「恶意外包」攻击**:OWASP Agentic Top-10 2025-12 的第一风险路径是
   "恶意 prompt → agent 执行 → 持久化后门"。ForgeOS 作为编排层,有责任提供执行隔离。

3. **防实验污染**:`forge route --from-git` (`route.go:244-249`) 自动读取 git diff 并
   推导风险特征。若 agent 的代码改动未被隔离,同一 git 工作区会被多次`forge evolve`轮次污染。

### 架构线索

`harness/` 目录已经是「带外执法」的正确物理位置——所有工具(`gate.mjs`·`check.py`·`secret-scan.mjs`·`arch-check.mjs`)
都是独立可执行文件,不依赖 agent-runtime 进程。沙箱应嵌套在 `CommandExecutor` 之下:

```
CommandExecutor
  → [沙箱垫片] 启动 Firecracker / gVisor / landlock 隔离
    → 在隔离环境内执行 agent-claude / gate.mjs / go test
    → 捕获 stdout/stderr + exit code + 文件系统 diff
  → 返回结果
```

当前 `CommandExecutor` 有一个关键 hook 未使用:

```go
// ClassifyOverload is an OPTIONAL recognizer for a transient overload
// response from the agent command.
ClassifyOverload func(out []byte) bool
```

和 `Observe` sink (`command_executor.go:128`)——这些正是为沙箱集成预留的 seam。

### Edge Cases 警示

- **⚡ 性能损耗**:Firecracker microVM 启动 ~125ms,每次 agent phase 都起一个 VM 会导致
  数百 ms 开销。对于 `forge run` 的 gate phases(仅 shell 检查),这是不可接受的。
  解决方案:gate phase 走轻量 `landlock`/`seccomp`(无 VM 开销),agent phase 走 Firecracker。

- **⚡ 宿主机资源耗尽**:多个 `forge evolve --parallel` 可能启动多个 microVM 耗尽内存。
  需对标 max-agent-calls 做 VM-level 资源预算。

- **⚡ 文件系统同步**:沙箱内的代码改动需要同步回宿主机才能被 gate 检测。
  当前 `command_executor.go:Dir = o.root` 直接设工作目录,沙箱模式下需 mount 或 rsync。

---

## 4. 工作流动态派生与自适应编排

### 现状分析

当前工作流是完全静态的 YAML 定义(`.agent/workflows/{build,design,discover,evolve}.yml`):

```yaml
# build.yml — 固定 6 个 phase,固定顺序,固定 gate 集合
phases:
  - name: planner
    agent: planner
    feeds_forward: true
  - name: implementer
    agent: implementer
    required_gates: [complexity, check]
  - name: harness-gates
    agent: implementer
    required_gates: [architecture, security, test, lint, coverage]
    on_fail: {action: loop_back, target_phase: implementer}
  - name: reviewer
    agent: reviewer
    required_when: ...reviewer
  - name: qa
    agent: qa
    required_gates: [test, app_test, architecture]
    on_fail: {action: loop_back, target_phase: implementer}
```

**一切都在运行前决定。** 运行时不做:

- 风险评估 → 增删安全检查
- 文件分析 → 只测受影响模块而非全量
- 进度判断 → 在 `roadmap_completion=0%` 时跳过 review/qa
- 历史学习 → 若某 phase 在最近 10 次 iteration 从未触发 loop-back,考虑合并或跳过

### 根本缺口

1. **`mode × lifecycle` 是静态选择器**:`mode.Effective` 从 `modes.yml` 加载预设阈值,
   是「哪个阶段门开几扇」而非「工作流本身形态可变」。
   如 explorer 只是跳过部分 phase + 降低 gate 严格度,但无法动态插入「简单变更→跳过架构审核」。

2. **`risk.FromChangedPaths` 的结果未被用于改变工作流**:Sprint 9 实现了自动风险检测
   (`risk_diff.go`),`resolveAutoRisk` (`engine_build.go:230-246`) 会打印风险等级,
   但当前只用于 `riskAdjustedTier` 提升模型 tier。**从未用于动态调整 workflow 结构本身。**

3. **无自适应 gate 选择**:`gatesFor` (`mode_gating.go:29-39`) 只做静态交集(required_gates ∩ policy.gates)。
   不会根据「本次改动仅涉及 test 文件 → 跳过 architecture 和 security gate」做动态缩减。

### 高价值场景

- **支付模块改动 → 自动注入 security-review phase**。当前需用户手动切到 `engineering` mode,
  未来应自动派生:在 `discover` phase 检测到 `payment.go` 被改动后,evolve loop 自动在 workflow
  中插入 security-review 和 payment-expert 两个 phase。

- **90% completion + 连续 3 次 gate 全绿 → 自动缩减 review scope**。
  系统学习的 skip 模式,减少不必要的 token 消耗。

- **典型「拼写错误/README 更新」→ 只跑语法门,跳过 architecture/security gate**。
  从 `git diff --stat` 判断,若改动 ≤ 5 行且无代码文件,自动走浅工作流。

### 实现线索

- `asset.Phase.OptionalFor` 字段(`asset.go:98`)已预留:「discover.yml 的 market-research phase
  标记了 `optional_for: [balanced]`」——这是一个声明式「可选跳过」机制。
- `asset.StopCondition.OnUnmet` (`asset.go:158-163`) 已经是条件性跳转声明。
- `internal/routing/routing.go` 的 `CandidatesForTier` 加上 `HistoryTiebreak` 构成了
  **基于历史证据做决策的完整模式**——同样的决策框架可复用于「基于历史证据选择 phase 集合」。

### Edge Cases 警示

- **⚡ 自适应不稳定**:若每次 iteration 都小幅增减 phase,agent 的行为会持续不收敛。
  需要 inertia 机制:一旦选定 work 流形态,锁定至少 N 个 iteration 不变。
- **⚡ 可重现性丢失**:同一输入得到不同工作流 → 调试困难。需 checkpoint 记录「本次运行的
  派生工作流完整形态」,而非仅记录 checkpoint.Workflow 名字。
- **⚡ 声明式 → 命令式边界模糊**:当前「工作流声明在 YAML」是核心设计原则。自适应编排
  不应要求 agent 写 YAML;而是由运行时从预设 phase 池中装配。池 + 装配规则仍需声明在 YAML。

---

## 5. 知识引擎 + 语义检索

### 现状分析

当前知识管理分三块:

| 组件 | 实现 | 实际能力 |
|---|---|---|
| **ADR 检索** | `prompt/retrieve.go: TF-IDF 标题匹配` | 只匹配 ADR 标题(< 20 word),不匹配正文。topK=6(=全部,当前只有 4 个 ADR) |
| **跨 session 记忆** | `memory/memory.go: JSONL 追加日志` | 存储 gap/decision/lesson 三类 text,查询精确 topic 匹配 |
| **工程约束注入** | `prompt/Gather → AGENTS.md 前 6 bullet` | 固定 6 条 bullet,无检索 |

`retrieve.go` 的核心算法:

```go
// score rates one doc against the query terms (TF · IDF-lite, length-normalized).
func score(qTerms, docToks []string, df map[string]int, totalDocs int) float64 { ... }
```

这是**纯关键词匹配**:不分词干、无同义词、无语义。当项目增长到 50+ ADR 时:
- 标题关键词不匹配时,相关 ADR 不会被检索到
- 相同学术词(如 "orchestrator")出现在 30 个 ADR 中,idfWeight ~= 0.4,无法区分哪个最相关
- memory 有 5000+ entries 时,`Query(es, KindGap, "")` 返回所有 gap,无排序

### 缺口的具体后果

1. **ADR 利用率随时间下降**:项目早期 4 个 ADR,全部注入没问题。100 个 ADR 时,标题关键词
   匹配的召回率极低,agent 可能忽略与当前任务高度相关的历史决策——导致架构退化。

2. **Memory 信息淹没**:`recordMemory` 在每次 iteration 写入一个 trajectory + 可能多个
   gap/decision/lesson。100 次 iteration × 平均 2 entry = 200 entries。当前对所有 entry
   一视同仁注入,200 行文本占上下文窗口大量空间(约 1500 token),却很少提供决策相关信息。

3. **无代码级知识**:agent 不知道项目已有的代码结构、设计模式、API 约定——这些靠模型内化知识,
   而非项目自身语料。若项目使用特定模式(如 repository pattern、CQRS),agent 可能因模型训练
   数据中的流行模式而偏离项目约定。

### 架构建议

```
┌─────────────────────────────────────────────────────────────────────┐
│                    Knowledge-Engine (v3)                             │
├─────────────────────────────────────────────────────────────────────┤
│  Retrieval API  →  多源 Query  +  Reranker  +  Context Builder     │
│                                                                     │
│  源 1. ADR 正文全文索引 (BM25 + 可选 embedding)                      │
│  源 2. Memory 语义检索 (非 topic 精确匹配)                           │
│  源 3. 代码结构索引 (package/type/function → 摘要 + 位置)            │
│  源 4. AGENTS.md 规则权重 (硬约束 → 高优先级注入)                    │
│  源 5. 最近 iteration 输出摘要 (assistant 输出向量化)                │
└─────────────────────────────────────────────────────────────────────┘
```

### Edge Cases 警示

- **⚡ 冷启动**:新项目无历史知识,检索返回空,fallback 到"注入最近 ADR 标题"。
- **⚡ 知识膨胀**:embedding 索引 > 100MB,超出 forge-core 零依赖哲学。解决方案:
  索引生成在外部(如 `forge index` 子命令),运行时只读嵌入。
- **⚡ 知识不一致**:ADR A 说「用 PostgreSQL」,ADR B 说「用 SQLite」——检索到两条冲突知识,
  agent 可能选择错误的。需要冲突检测 + 时间戳优先。
- **⚡ memory 毒化**:受损/误导性 memory entry 可能无限期影响后续决策。`memory.filterSuperseded`
  已有「覆盖」机制,但需要可信来源标记(如「仅 agent 决策可覆盖,gate 失败不可」)。

---

## 6. Edge Cases & 性能优化

### 6.1 并发 checkpoint 竞争

**问题**:`forge evolve --parallel` 并行模式下,多个 agent phase 可能同时尝试写 checkpoint。
当前实现(`evolve.go:155-182`):

```go
func phaseCheckpointHook(...) func(iter, phaseIdx int) {
    return func(iter, phaseIdx int) {
        cp := persist.Checkpoint{...}
        if err := persist.Save(checkpointPath(o.root), cp, 5); err != nil {
            // WARN, continue
        }
    }
}
```

`persist.Save` 使用 temp + rename 原子写入。但在 parallel 模式下两个 phase 同时完成:
- 两个 goroutine 都执行 `persist.Checkpoint{PhaseIndex: X}` → 后写的会覆盖先写的
- 先写的 phase 进度丢失,resume 时从后写的 PhaseIndex 开始 → 重新执行已完成的 phase

**影响**:并行度越高,丢失 checkpoint 的概率越大。在多次 `--resume` 场景下累积开销显著。

**建议**:
- 非阻塞:并行模式下使用 `sync.Map` 或 channel 聚合 phase 索引,只在 iteration 边界写出。
- `orchestrator/parallel.go:13-22` 的锁序注释已意识到此问题("no per-phase checkpoint in parallel mode")。

### 6.2 YAML 转码依赖 bash + python shim

**问题**:`main.go:242-254`:

```go
out, err := exec.Command("python3", shim, ymlPath).Output()
```

**每个** `forge run` 和 `forge evolve` 都 shell 出一个 `python3` 进程转码 YAML→JSON。
这不是一次性的:每次 `loadWorkflow` 调用都发起新的子进程。

- 对于 `forge evolve` 每个 iteration 都会重新 `loadWorkflow`(在 `execLoop` 之前加载一次,
  但 loop-evolve 循环中每次 Run 前不重新加载——检查确认:WF 加载仅一次)。
- 更隐蔽的:没有 python3 环境时,用户收到错误而非降级路径。

**影响**:低——每个转码 ~10ms。但跨平台可移植性受限(Windows 无 `python3`)。

**建议**:Go 1.24+ 标准库有 `maps` 和 `slices`,但无 YAML 解析器。长远看加一个 Go YAML 库
(如 `gopkg.in/yaml.v3`)是最干净的,但这打破了「纯净 Go 标准库零依赖」的硬红线。
可选中间方案:运行时缓存转码结果 + 文件 mtime 检测,避免重复 shell。

### 6.3 Acceptance probe 全量运行 → 仅部分 gate 关联

**问题**:`gate.ProbeAll` (`gate.go:101-138`) 运行整个 `acceptance.mjs --json`,它触发
**所有** probe:
- `probeTests` (全部 harness 测试)
- `probeAppTests` (全部 example 应用测试)
- `probeLint` (全部语言 lint)
- `probeCoverage` (全部语言覆盖率)
- `probeSCA` (全部 manifest SCA)
- `probeSecurity` (secret scan)
- `probeComplexity` (structural gate)
- `probeArchitecture` (arch-check 8 检查)

每次 `forge run` 花 ~2-5 秒跑所有 gate,即使其中很多与本次改动无关。
例如只改了一个 markdown 文件:仍然全量跑 architecture / security / lint / coverage。

**影响**:每次 `forge run` 的 gate 延迟主要由无畏的全量 probe 驱动。
在 `forge evolve` 中每个 iteration 都重新 probe one(通过 `loopProbe.refresh()`)——N iteration × 5s = 很多浪费的时间。

**建议**:
- 增加 `git diff` 驱动的 gate 选择:若只有 `.md` 变化,跳过后 6 个 gate。
- 或让 phase 的 `required_gates` 支持条件绑定:`required_gates: [complexity, check, {name: arch, if: changed("*.go")}]`。

### 6.4 Memory 存储无界增长的静默风险

**问题**:`memory.go` 的 `Append` 是纯追加写。100 iteration 后,若每个 iteration
写入 5 个 entry + 见 `recordMemory` 的 escalation entries(iter 2+ 额外写 Decision),
memory.jsonl 可能膨胀到 500+ entries。`DefaultCompactThreshold = 500`——也就是说
在触发 compact 之前,已经有 500 条 entry 可能全部注入到每个 phase 的 prompt 中。

`buildPrompt` (`prompt_context.go:385-410`) 的 `memoryContext` 当前:
```go
func memoryContext(repoRoot, query string) []string {
    entries, err := memory.Load(memoryPath(repoRoot))
    ...
    return []string{..."Cross-session memory:"+strings.Join(lines, "\n")}
}
```

**加载所有 entry,注入所有 entry。** 无排序、无选择。

**影响**:500 entry × ~80 words/entry = 40,000 words ≈ 10,000 token 的 prompt 上下文
被记忆吞噬——对于边界模型(Haiku, 8K context)是灾难性的。

**建议**:
- `memory.Query` 已支持 kind+topic 过滤,但 `memoryContext` 不传 query。
- v1 应立即绑定:Query(entries, kind="decision" + topic=wf.Stage) 仅取决策类记忆。
- 或者按 `confidence` + recency 排序并 topK 截断——这正是 retrieval 引擎应干的活儿。

### 6.5 AGENTS.md 约束的单向性(可违反性无检测)

**问题**:AGENTS.md 是工程红线文档,但它的约束是**单向文本注入**:
- `prompt.Gather` 读取前 6 bullet 注入到 prompt
- agent 被告知「必须遵守」
- **但没有任何机制检测 agent 是否实际遵守**

具体看 `constraints` (`prompt.go:128-138`):

```go
func constraints(repoRoot string) string {
    b, err := os.ReadFile(filepath.Join(repoRoot, ".agent", "AGENTS.md"))
    ...
    return leadingBullets(string(b), 6)
}
```

「函数 ≤ 50 行」「循环依赖=0」等硬约束注入给 agent,但:
- agent 可能写出 70 行函数 → 只有 `arch-check.mjs` 在 gate 阶段才会捕获
- agent 可能在单次 LLM 响应内不可撤销地违反约束(gate 在之后运行,但已产生违规代码)
- agent 可能构造一个「用模板绕过函数长度限制」的间接违反

**影响**:治理的「预防」vs 「检测」缺口。当前是检测偏置——gate 抓违规,但不阻止违规产生。
对于自治运行(无人类在循环中),这意味每次 iteration 都可能产生被 gate 否决的代码,浪费 token。

**建议**:
- v1 立即:在 agent prompt 中添加「先规划,再实施」指令,并将 gate 规则嵌入为结构化约束
  (非自然语言 bullet)。
- v2:agent-runtime 层可以在每次工具调用后增量检查(如:写文件后立即检查该文件行数)。
- 架构上:这是一个「运行时强制执行」vs 「prompt 劝导」的权衡,与方向 3(沙箱)相关。

### 6.6 Signal-context 传播与 `forge route` 的风险推理孤岛

**问题**:`forge route` (`route.go`) 有一个完整的风险推理管道:路径启发式 → `risk.Classify` →
`routing.TierForScore`。但 `resolveAutoRisk` (`engine_build.go:230-246`) 是**另一个**独立的
相似管道,用于 `forge run` 和 `forge evolve`:

```go
func resolveAutoRisk(root string) (level string, reasons []string) {
    paths := gitChangedPaths(root)
    sig, reasons := risk.FromChangedPaths(paths)
    level, _ = risk.Classify(sig)
    return level, reasons
}
```

对比 `route.go` 的:
```go
func applyDiffSignals(o *routeOpts) {
    ...
    auto, reasons := risk.FromChangedPaths(paths)
    o.sig = mergeAutoSignals(o.sig, auto)
    ...
}
```

**两套相同的逻辑,不同入口,不同 merge 策略。**

- `route.go` 的 `mergeAutoSignals` 取 OR/max/AND 组合明确 feature 与自动 feature。
- `engine_build.go` 的 `resolveAutoRisk` 只输出 level,不暴露出哪些 feature 触发的。
- `logAutoRisk` 只打印「auto-detected risk=high(payment surface)」不打印完整的信号细节。

**影响**:不同子命令对相同 diff 可能得出不同的风险信号组合,
最终影响模型 tier 分配不一致。

**建议**:
- 统一 `resolveAutoRisk` 的产出物:返回 `risk.Signals` 而非仅 level string。
- 在 `phaseTierResolver` (`engine_build.go:158-185`) 的 risk 升档步骤中改走 `risk.Classify` 的完整路径。
- 最终让 `forge run` 的 tier 与 `forge route` 的 tier 完全一致——同输入,同输出。

---

## 7. 优先级矩阵

| # | 方向 | 复杂度 | 影响范围 | 紧急度 | 前置依赖 | 推荐顺序 |
|---|---|---|---|---|---|---|
| 1 | **Agent-Runtime 执行层** | 高 | 安全·成本·能力 | 高(S24-25 已证明真 agent 执行) | 无 | ★★★ 优先 |
| 2 | **多厂商模型池 + 自适应路由** | 中 | 供应商锁定·成本 | 中(当前 Claude-only 可用) | Agent-Runtime 可选 | ★★ 第二 |
| 3 | **载重墙/沙箱** | 高 | 安全·信任边界 | 高(真 agent = 真风险) | Agent-Runtime 先期 | ★★★ 并行于 1 |
| 4 | **工作流动态派生** | 中 | 效率·智能 | 低(当前静态够用) | 风险检测已就绪 | ★ 第四 |
| 5 | **知识引擎 + 语义检索** | 中-高 | Prompt 质量·可扩展 | 中(ADR 4 个→100 个时失效) | 无 | ★★ 第三 |
| 6a | Edge: 并行 checkpoint 竞争 | 低 | 可靠性 | 中(parallel 已落地) | 无 | ★★ 立即修 |
| 6b | Edge: 无 python 降级 | 低 | 可移植性 | 低 | 无 | ★ 待触发 |
| 6c | Edge: 无畏 gate 全量运行 | 中 | 延迟 | 中(每次 evolve 多花 ~5s) | 无 | ★ 可优化 |
| 6d | Edge: memory 无界增长 | 低 | Prompt 中毒 | 中(500 entry 后) | 无 | ★ 立即修 |
| 6e | Edge: 约束单向性 | 低 | 治理完整 | 中 | 无 | ★ 增强 |
| 6f | Edge: 风险推理孤岛 | 低 | 一致性 | 中 | 无 | ★ 立即对齐 |

### 行动建议

1. **今周(红色)**:修 6a(并行 checkpoint)、6d(memory topK 截断)、6f(风险推理统一)。

2. **本月(橙色)**:开始方向 1(Agent-Runtime 雏形)和方向 3(沙箱垫片)。
   当前 `command_executor.go` 的 `Observe` + `ClassifyOverload` seam 就是为 Agent-Runtime
   预留的钩子,应首先利用它们实现「工具调用拦截层」:在 agent 的 stdout 中解析工具调用
   (如 `Claude Code` 的 Structured Output),注册可执行工具集,并在工具执行前后加钩子。

3. **本季(黄色)**:方向 2(多厂商)+ 方向 5(语义检索)。
   多厂商依赖 prompt 格式化抽象——当前 `prompt.Build` 的输出格式是 Claude 优化的 XML tags,
   转为通用 template engine 后即解锁。

4. **半年(蓝色)**:方向 4(自适应编排)。
   需要前三个方向的 infrastructure 积累。

---

*文档生成基于 commit HEAD 日期 2026-07-01。*
*代码引用基于 forge-core 13 包 + harness 全套工具的静态分析,不依赖运行时。*
