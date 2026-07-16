# 代码验证审阅报告

> **审阅对象**: `2026-07-12-five-genuine-architectural-frontiers-senior-architect-pm.md`
> **方法**:逐方向代码验证(forge-core 18 包 + harness + `cmd/forge`)+ 与已有 100+ `.out.md` 覆盖交叉验证
> **结论**:2 个方向有显著事实错误,3 个方向代码基础准确但差异化夸大。最大价值在方向二(跨厂商切换)和方向三(HITL)的直觉,方向五的代码描述需要大幅修正。

---

## 逐方向验证结果

### ✅❌ 方向一 · 多项目工作区编排

| 代码断言 | 验证 | 证据 |
|---------|:----:|------|
| 无 `forge workspace` 子命令或 `Workspace` struct | ✅ | `grep -rn "workspace\|Workspace" forge-core/cmd/forge/` 仅命中 Rust `[workspace]` 测试数据 |
| `asset.Workflow.DependsOn` 只解 phase 拓扑,不解跨 repo | ✅ | `asset.go:153` `DependsOn []string` 确认为 phase name 列表 |
| `ModelMap` 全局,无 per-project 路由覆盖 | ✅ | `routing.go:315` 文件级 var |
| `internal/budget` per-process,无共享池 | ✅ | `budget.go` 中 `agentCalls` 和 `runBudget` 均为 Engine 字段 |
| `childEnv` 只继承宿主 env | ✅ | `command_executor.go:293` childEnv 只过滤 `FORGE_AGENT_DEPTH`,其余全部 `os.Environ()` |

**累计**:5/5 ✅ — 完全确认。

**补充发现**: `command_executor.go:293-301` 的 `childEnv` 被命名为 "environment variable guard" 但只过滤了一个键——这其实是方向二(env 泄漏)的共性问题,方向二文档未提。`forge-core` 没有任何 per-project 凭证隔离。

**差异化真实性**:方向一在已有分析中**高频提及但从未系统展开**——此前 14 个 `.out.md` 提及 `RunID`/session 隔离但无 workspace 方案。此文档是首次给出完整工作区架构设计。👍

---

### ❌⚠️ 方向二 · 跨厂商模型池与实时故障切换

| 代码断言 | 验证 | 证据 |
|---------|:----:|------|
| `ModelMap` 硬编码 Anthropic + 三 tier | ✅ | `routing.go:315-321` 确为 `{"anthropic": {haiku, sonnet, opus}}` |
| 无 `openai/gpt-4o` / `gemini` | ✅ | 同上,仅一个 provider |
| `classifyRunErr` 把 `KindOverloaded` 标记为 retryable | ✅ | `exec_error.go:97` `Retryable()` 对 KindOverloaded 返回 true |
| 重试是等待后重试同一厂商,非切换 | ✅ | `backoff.go:54-55` retry 逻辑: `overloadBackoff(attempt)` + `continue` 重试同一 executor |
| 退避无抖动(no jitter) | ✅ | `backoff.go:65-66` 注释自证: "v1 single-run: NO JITTER" |
| `BudgetAdjustTier` 只能降档,不能换厂商 | ✅ | `routing.go:297-307` 只调用 `DowngradeOne`,不跨 provider |

**累计**:5/5 ✅ — 完全确认。

**差异化真实性**:此方向的**核心直觉完全正确且差异化最大**——此前分析多次提到退避/降档,但**从未将「退避 vs 切换」作为对比框架提出**。唯一在代码上无法验证的是单厂商 99.5% 可用性声明(那是 Anthropic 的外部运营指标)。

**附加建议**:方向二与方向一的凭证隔离有重叠——跨厂商模型切换必然需要 per-provider API key 管理,而这与 workspace 的 per-project 凭证映射是同一接口的不同维度。建议两方向联合设计 `ProviderCredential` 抽象。

---

### ✅⚠️ 方向三 · 人机交互协议(Pause/Resume/Inspect)

| 代码断言 | 验证 | 证据 |
|---------|:----:|------|
| 交互仅两种:binary approval + CTRL+C | ✅ | `converge.go:47-55` HumanApproved 布尔字段; `evolve.go` replay-on-`--resume` |
| `--approved` flag + `.forge/<stage>.approved` 文件 | ✅ | `main.go:419-424` `--approved` 解析; `gates.go:169-176` on-disk marker |
| `converge.humanGate` 收到 true→MET | ✅ | `converge.go:172-175` `if sig.HumanApproved` |
| 无 `forge tui` / `forge resume` / `forge abort` | ✅ | `grep -rn "tui\|resume\|abort" forge-core/cmd/forge/` 无命中 |
| `NoProgress` tripwire 是 hard stop | ✅ | `loop.go:201` `l.tripped(*stale)` 返回 `staleOutcome`,无 pause 路径 |

**累计**:5/5 ✅ — 完全确认。

**关键补充 — evolve 拒绝 human_gate**:
> `evolve.go` 中 `rejectHumanGate` 在 LoopEngine 构造前直接拒绝包含 human_gate stop 的 workflow。设计:human_gate 是单次审批闸门,不是自治循环目标。这意味着**方向三的「Design→Build 之间 human_approval gate」在 evolve 路径中完全不存在**——它只存在于单次的 `forge run design`。这是文档未指出的架构约束。

**环境变量泄漏风险确认**:
> `command_executor.go:293` childEnv 只过滤 `FORGE_AGENT_DEPTH`,将 `os.Environ()` 中所有 env(含 `GITHUB_TOKEN`、`AWS_CREDS`)无条件传递给子进程。这是安全问题,方向三优先级当因此上调。

**差异化真实性**:方向三的直觉(operator visibility = 信任的前提)是**正确的且此前分析覆盖较浅**。但工作量估计(800+400+200=1400 行)偏低——TUI 集成涉及 `runAgentPhase` 的回调架构变更和 `LoopEngine` 的 pause 协议(当前 `runIteration` 是一次性调用,无状态挂起点),真实工作量估计 ~2500 行。

---

### ✅✅ 方向四 · 语义级 Agent 产出验证

| 代码断言 | 验证 | 证据 |
|---------|:----:|------|
| Gate 类型:lint/test/build/complexity/arch/security | ✅ | `gate/resolve.go:79-92` ResolveGate switch 确认六种 |
| 全是机械门(格式/编译/测试/依赖/SAST) | ✅ | 逐 gate 验证:lint(风格),test(测试通过),build(编译),complexity(圈复杂度),arch(依赖方向),security(secret 扫描) |
| 无 `contract`/`property`/`mutation` gate | ✅ | `resolve.go` switch 中无这些 case |
| `internal/risk.FromChangedPaths` 存在(从 diff 提取信号) | ✅ | `risk/risk_diff.go:64` `func FromChangedPaths` |
| `harness/acceptance-quality.mjs` 有 `probeLint`/`probeCoverage` 模式 | ✅ | 文件中存在这些函数 |

**累计**:5/5 ✅ — 完全确认。

**额外发现**:`harness/policies.yml` 的实际内容与文档描述不完全一致。文档列出一组 YAML gate 定义,但 `policies.yml` 实际是:
```yaml
max_file_lines: 500
max_function_lines: 50
max_root_files: 15
circular_dependency_count: 0
coverage_min: 80
enforce: block
```
Gate 类型定义在 Go 代码(`gate/resolve.go`)而非 policies.yml 中。这是一个描述偏差但不影响论证正确性。

**差异化真实性**:此方向是**真实缺口但差异化较低**——"机械门不够"的论点在之前分析中已多次出现。文档的真正贡献是将它连接到 `internal/risk`(已有)和语义门架构,提供了具体的实施路径(contract → property → mutation 优先级)。这是工程化落地价值而非发现价值。

---

### ❌❌ 方向五 · 跨 Session 知识生命周期管理

**⚠️ 此方向有三个关键事实错误,需要大幅修正。**

| 代码断言 | 验证 | 证据 | 纠正 |
|---------|:----:|------|------|
| "`memory.Load` 在 forge evolve 中零处被调用" | ❌ **错误** | `prompt_memory.go:165` `memoryContext` 调用 `memory.Load`; `prompt_context.go:368` 从 `buildPromptWithEmits` 调用 `memoryContext`; `engine_build.go:53` 在 evolve 路径中构造 agent argv 时调用 `buildPrompt` | **memory 在每次 agent phase 执行时被读取并注入 prompt** |
| "`memory.Append` 没有任何 workflow phase 真正调用" | ❌ **错误** | `evolve.go:384` `recordMemory` 每迭代调用 `memory.Append` | **memory 在每次 evolve 迭代后被写入**(轨迹 + reviewer findings + gate 失败) |
| "无自动过期策略,Prune/Compact 是手动调用" | ❌ **错误** | `evolve.go:434` `compactMemoryIfDue` 每 10 迭代自动触发; `memory_compact.go:54` `DefaultCompactThreshold`; `memory_compact.go:63` `CompactAgeSeconds=86400` | **存在自动 compaction:每 10 迭代 + age-based(24h)+ threshold-based(500 entries)** |
| "memoryContext lane 从未被装配到任何 workflow 中" | ❌ **错误** | `prompt_context.go:368` 在 `buildPromptWithEmits` 中调用 `memoryContext`,作为第三路 context lane | **memory lane 已装配到所有 agent prompt 中**(包括 evolve 路径) |
| "无 TTL 过期,memory.jsonl 无限增长" | ⚠️ **部分不准确** | `CompactAgeSeconds=86400`(24h)是 age-based 保留; 但 `Compact` 只压缩旧条目而非删除 | **有 age 基元,但确实无显式 TTL(保留旧摘要而非丢弃)** |
| "无优先级保留,所有条目一视同仁" | ✅ **准确** | `Compact` 的 `keepPerKind` 按 kind 分组保留,但无 confidence-based 差异化保留 | **确认** |
| "无跨 session 去重" | ✅ **准确** | `memory.go` 中 `Supersedes` 依赖显式指定,无自动语义去重 | **确认** |

**差异化真实性**:方向五的问题域(知识生命周期管理)是真实缺口,但**文档对现有实现状态的描述严重不准确**。Memory 的**基本读写路径已完整装配到 evolve loop 中**——每次 agent 调用都会读取 memory 作为 prompt context,每次迭代结束都会追加 memory entry。文档描述的 "存储基础设施已就绪但消费者为零" 不符合代码现状。

**真实缺口(文档未正确指出的)**:
1. **无 confidence-based 差异化保留** — ✅ 确认,所有条目一视同仁
2. **无自动语义去重** — ✅ 确认,Supersedes 是手动
3. **摘要的消费端不存在** — ✅ 确认,`Compact` 的 summarizeBlock 产生的摘要从未被注入 prompt(memoryContext 读原始条目,非摘要)
4. **Memory 写入内容结构化不足** — 当前写入的是自由文本 `Detail` 字段,无结构化知识(如决策表、gap 分析框架),限制了消费端的使用价值

---

## 跨方向事实错误汇总

| # | 方向 | 错误声明 | 代码事实 | 严重度 |
|---|------|---------|---------|:------:|
| 1 | 五 | "memory.Load 在 evolve 中零处调用" | `memoryContext → buildPrompt → evolve 路径`,每个 agent phase 都读 memory | **高** |
| 2 | 五 | "memory.Append 无 workflow phase 调用" | `recordMemory` 每迭代写 memory(trajectory + findings + gate) | **高** |
| 3 | 五 | "Compact/Prune 是手动调用" | `compactMemoryIfDue` 每 10 迭代自动触发 | **高** |
| 4 | 五 | "memory lane 从未装配" | `prompt_context.go:368` 已装配为第三路 context lane | **高** |
| 5 | 五 | "无自动过期" | `CompactAgeSeconds=86400`(24h) + threshold=500 | **中** |
| 6 | 四 | "policies.yml 定义了 lint/test/build/complexity等 gate" | gate 类型定义在 `gate/resolve.go`而不是 policies.yml;policies.yml 只有尺寸/复杂度参数 | **低** |
| 7 | 三 | 未提及 `rejectHumanGate` 约束 | evolve 路径禁止 human_gate,方向三的 approval gate 场景在 evolve 中不成立 | **中** |

---

## 差异化定位 vs 已有分析覆盖

grep 已有 100+ `.out.md` 检查每个方向的核心术语:

| 方向 | 核心术语 | 命中数 | 覆盖状态 |
|------|---------|:------:|---------|
| D1 Workspace | `workspace` / 跨项目 | 0 / 2 | ✅ **新贡献** — 此前分析仅提及 RunID/session 隔离,无完整工作区方案 |
| D2 跨厂商池 | `failover` / multi-provider | 0 / 0 | ✅ **新贡献** — 首次以「单厂商 99.5%→切换是数学必要条件」框架提出 |
| D3 HITL | `pause.*resume` / `tui` | 1 / 0 | ⚠️ 部分覆盖 — `forge approve --with-conditions` 在方向 4 中提及,但 TUI 和 pause/resume 协议是新角度 |
| D4 语义门 | `semantic.*gate` / `contract.*test` | 0 / 3 | ⚠️ 新角度 — 此前分析多次提及「机械门不够」但无人给出从 `FromChangedPaths` 到语义门的工程化路径 |
| D5 Memory | `memory.*consumer` / `memory.*TTL` | 2 / 5 | ❌ **差异化因事实错误被削弱** — 问题域正确但现有实现描述有误,需要重写后再评估增量价值 |

---

## 优先级与工作量重新评估

| 方向 | 文档优先级 | 调整后 | 工作量(文档) | 工作量(调整) | 理由 |
|------|:---------:|:------:|:-----------:|:-----------:|------|
| ① 工作区 | P1 | **P1** | 1500 行 | 1500 行 | 唯一新贡献方向,无事实错误 |
| ② 跨厂商池 | P1 | **P1** | 1200 行 | 1200 行 | 差异化最大,代码基础确认 |
| ③ HITL | P0 | **P0** | 1400 行 | ~2500 行 | 工作量低估(LoopEngine pause 协议需架构变更+TUI 集成);evolve 与 human_gate 的不兼容需要额外设计 |
| ④ 语义门 | P2 | **P2** | 600+ | 600+ | 正确但非新发现;依赖工具生态 |
| ⑤ Memory 生命周期 | P2 | **P2** | 1200 行 | ~500 行 | 因事实错误高估;真实缺口仅为去重+confidence保留+摘要消费,不含装配工作 |

---

## 结论

**最好的部分**:方向一(工作区)和方向二(跨厂商模型池)是**真正新颖、代码验证通过、差异化最大**的方向。方向三(HITL)的直觉正确,但低估了工程量并忽略了 `rejectHumanGate` 架构约束。

**需要修正的部分**:方向五(内存生命周期)的三个核心事实错误严重削弱了文档的可信度。memory 的基本读写路径已经装配到 evolve loop 中——描述应改为「memory 已装配读写路径但缺乏智能生命周期管理」而非「消费端为零」。

**整体价值**:作为一个资深架构师/产品经理视角,这篇文档的**战略直觉是强的**——工作区解锁组织采用、跨厂商池解锁 24h 可靠、HITL 解锁用户信任,这三个叙事互锁且市场价值清晰。但事实准确性的缺口(尤其是方向五)需要在引用前修复,否则会误导后续 Sprint 规划。
