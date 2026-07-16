# ForgeOS — 五个真正未被覆盖的架构/产品扩展方向

> **角色**: 资深架构师 + 产品经理  
> **方法**:  
> 1. 全局逐文件扫描 forge-core（19 Go 包 / ~35k LOC 运行时 + CLI 层）、harness（39+ 模块 / ~10.5k LOC 执法层）、`.agent/`（12 agent 卡 / 9 skill 卡 / 5 工作流 / 全部 policies+ADR+DECISIONS）、examples/、`.github/workflows/`、`.forge/` 运行时产物  
> 2. 完整阅读 31 轮 Sprint 演进记录（CURRENT_SPRINT.md）、FUNCTIONAL_REQUIREMENTS_AUDIT.md（200+ 条目，0 GAP）、ADR-0001~0004、所有核心架构文档  
> 3. 逐方向交叉验证：对 **85+ 份已有分析文档**（`docs/requirements/` ~45 篇 + `docs/analysis/` ~40 篇 + 核心策略文档）进行全文关键词检索 + 语义比对，确认每个方向的核心论点**从未被已有分析作为独立系统性方向展开**。如有子概念曾在某篇文档的侧栏被提及，本文明确标注该已有出处并说明差异点。  
> 4. **纪律**: 不编写任何代码。每个方向附精确到 `file:line` 的代码级证据、实际影响、边界场景。  
> **日期**: 2026-07-10

---

## 已有覆盖 vs. 本文方向

| 已有高密度覆盖域 | 代表篇数 | 本文不重复 |
|---|---|---|
| 引擎补齐（编排/路由/记忆/收敛/信号/并行/回灌） | ~20 | ✅ |
| 生产可靠性（超时/重试/护栏/进程组/自愈/退避） | ~15 | ✅ |
| 执行语义形式化（原子性/幂等/因果一致性/回滚/版本） | ~12 | ✅ |
| 安全纵深（secret-scan/递归/预算/SCA/prompt injection 防御） | ~12 | ✅ [1] |
| 学习闭环（trace/telemetry/scorecard/自适应收敛） | ~10 | ✅ |
| 治理/执法（arch-check 8 检查/check.py 10 检查/漂移守卫） | ~10 | ✅ |
| 运营可信度（Run Identity/状态隔离/审计/健康检查） | ~8 | ✅ |
| 二阶伴生（配置爆炸/知识衰减/TOCTOU/无声数据丢失） | ~8 | ✅ |
| CLI DX / 配置管理 / tutorial / shell 集成 | ~6 | ✅ |
| 第三地平线（多仓库联邦/事件驱动/Web UI/管道组合） | ~10 | ✅ |

**本文的 5 个方向落在上述所有覆盖域的间隙中**。每个方向回答一个问题：**「当系统核心能力完备后，什么二阶问题会浮现？」** 不是「加什么新功能」，而是「已经有的东西，在什么看不见的维度上仍有结构性缺口」。

> [1] 已有 `five-novel-architectural-frontiers-2026-07-10.md` 方向二深入分析了 **prompt 装配管线内部的信任域模型**（结构边界、逐 lane token 预算、输出验证、ROADMAP 读取消毒）。这是 prompt-level 安全。本文方向一分析的是 **OS-level 安全**（进程列表可见性、环境变量传播、临时文件残留、跨进程隔离）——两个完全不同的攻击面和安全边界。

---

## 方向一 · OS 级安全边界：Prompt 与凭证的进程层隔离

> **优先级**: 🔴 **P0** | **类别**: 安全 · 信任边界 | **预估**: ~2 sprints  
> **差异化证明**: 关键词 `ps aux`、`cmdline`、`argv leak`、`process list visibility`、`prompt leak` 在全部 85+ 篇已有分析文档中 **零命中**。已有 credential 分析文档（`genuinely-novel-expansion-directions.md`）讨论的是 **agent-level 凭证注入**（如何向 agent 安全传递 secret），本文讨论的是 **OS-level 凭证保护**（同机器其他进程能否读到所有 prompt 和 API key）。

### 问题描述

ForgeOS 将完整的系统 prompt（含 ADR、ROADMAP、memory、AGENTS.md 硬约束、feed-forward 输出）作为 CLI 参数传递给 agent 命令：

```go
// forge-core/cmd/forge/engine_build.go:52-53
Build: func(p asset.Phase, mode string) []string {
    argv := claudeArgv(o, isClaude, tierOf(p), p)
    return append(argv, "-p", requiresToolsGuard(...,
        buildPrompt(o.root, p, mode, tierOf, ctxCache, gates, phaseOut, findings)))
```

`buildPrompt` 产出的是一个可能数万 token 的字符串，包含了项目的完整上下文、架构决策、安全约束、商业逻辑。这个字符串被直接嵌入 `argv` 作为 `claude -p <...>` 的参数。**在同一主机上的任何其他用户（或进程）只需要执行 `ps aux`，就能读取当前运行的 ForgeOS agent 的完整 prompt——包括 API key（如果 prompt 中包含了）、商业机密、未公开的架构决策。**

与此同时，子进程的环境变量直接从父进程继承，不做任何过滤：

```go
// forge-core/internal/orchestrator/command_executor.go:287-294
func childEnv(depth int) []string {
    prefix := agentDepthEnv + "="
    base := os.Environ()
    out := make([]string, 0, len(base)+1)
    for _, kv := range base {
        if !strings.HasPrefix(kv, prefix) {
            out = append(out, kv)
        }
    }
    return append(out, fmt.Sprintf("%s=%d", agentDepthEnv, depth+1))
}
```

这段代码唯一过滤的 key 是 `FORGE_AGENT_DEPTH`。`ANTHROPIC_API_KEY`、`OPENAI_API_KEY`、云服务凭证——**全部静默透传给子进程**。agent 子进程（以及它通过 Bash spawn 的任何孙子进程）都能读到这些凭据。

第三，forge-core 运行时会产生临时残留文件。`internal/doctor/doctor.go:121` 已经有 `tmpResidueCheck` 来检测 `.forge/*.tmp` 残留，但这是一个**在 `forge doctor` 时才运行的被动检测**，并非主动防护。此外，`claude -p` 的返回值（可能包含敏感信息）保留在 `cappedBuffer` 中，写入 trace 文件后被持久化——没有机制在 trace 轮转或清除时安全擦除。

### 边界场景

| 场景 | 风险严重度 | 当前处理 |
|------|-----------|----------|
| 共享开发机：用户 A 跑 `forge evolve`，用户 B 执行 `ps aux \| grep claude` | B 读到 A 的完整 prompt（ADR/ROADMAP/memory） | **无防护** |
| CI 环境：多个 job 并行在同一 VM 上运行 | job A 的 ForgeOS 进程被 job B 读到 | **无防护** |
| Agent 子进程被入侵或产生 core dump | core dump 包含 env vars（含 API key） | **无防护** |
| agent 产出的临时文件在 phase 完成后未清理 | 敏感对话历史遗留在磁盘上 | `tmpResidueCheck` 被动检测，无自动清理 |
| `childEnv` 透传的 `ANTHROPIC_API_KEY` 被孙子进程泄漏 | 凭据经由非预期的子进程泄露 | **无过滤** |
| `claude -p` 的 prompt 中包含用户代码库中硬编码的 secret（即使被 secret-scan 漏过） | secret 在进程列表中以明文可见 | **无防护** |

### 建议方向

1. **Prompt 传输通道隔离**: 改用临时文件（`os.MkdirTemp` + 0600 权限）或标准输入管道传递 prompt，替代 CLI 参数。使 `ps aux` 无法读取 prompt 内容。
2. **环境变量最小化**: 在 `childEnv` 中实现 allowlist 机制，只透传 `FORGE_AGENT_DEPTH` 和 agent 工作需要的最小环境变量集（如 `PATH`、`HOME`）。API 凭据通过专用的凭证注入通道（如临时只读文件 descriptor）传递。
3. **临时文件安全生命周期**: 对 agent phase 产生的所有临时文件注册 `defer` 清理（包括 crash 路径的清理担保）。对包含敏感数据的文件在 `Remove` 前进行 `fallocate(FALLOC_FL_PUNCH_HOLE)` 或覆盖写入。
4. **SandboxConfig 落地**: 现有的 `SandboxConfig` struct（`command_executor.go:111-113`）目前是 v1 placeholder skeleton。将进程隔离、环境变量过滤、文件系统隔离作为 `SandboxConfig` 的真实实现，而非等到 v3 Firecracker。
5. **Trace 数据脱敏**: 在 `trace.Emit` 路径增加可选的敏感字段过滤器（正则模式匹配 secret-like 字符串 → `[REDACTED]`），防止 API key 等凭据意外写入持久化的 trace 文件。

### 代码入口点

| 文件 | 行号 | 对应的改动点 |
|------|------|-------------|
| `cmd/forge/engine_build.go` | 52-53 | prompt 从 argv 移到 stdin 或 temp file |
| `internal/orchestrator/command_executor.go` | 287-294 | `childEnv` 增加 env allowlist |
| `internal/orchestrator/command_executor.go` | 111-113 | `SandboxConfig` 从 skeleton 到真实隔离 |
| `internal/trace/trace.go` | 108-117 | `Emit` 增加敏感数据脱敏钩子 |
| `internal/doctor/doctor.go` | 119-129 | `tmpResidueCheck` 升级为主动清理 |

---

## 方向二 · Agent 输出确定性契约：从概率生成器到可验证执行者

> **优先级**: 🟠 **P1** | **类别**: 架构 · 信任模型 | **预估**: ~3 sprints  
> **差异化证明**: 已有文档中「determinism」仅出现在两处——(a) `routing.go` 的 `TierFor` 是确定性函数（与 LLM 无关）; (b) 测试用固定种子保证 CI 稳定（`five-uncovered-horizontal-frontiers.md`、`structural-gaps-v41-genuinely-unexplored.md`）。**没有文档分析**：编排引擎的验证管线（gate 重跑、文件 diff、converge 重新评估）在本质上假设 agent 的**输出是可复现的**，但 LLM agent 天生非确定——这个假设裂缝是整个系统信任模型的结构性缺口。

### 问题描述

ForgeOS 构建了一个精密的验证管线：

```
planner → implementer → [harness gates] → reviewer → qa → converge evaluation
```

这条管线使用了多种比较/验证机制：
- `computeCodeTestRatio` 比较代码行数和测试行数
- `FileDelta` 比较 ROADMAP 声明和 git 实际改动
- `harness gates` 在每次迭代中重新评估代码质量
- `converge` 检查 `gates_status == green` 和 `roadmap_completion >= 100%`

所有这些验证都基于一个隐式假设：**agent 的输出是可复现、可比较、可验证的**。但 LLM agent 的每一次调用都可能产生不同的结果——不同措辞、不同代码风格、不同实现路径、甚至不同的架构选择。

这个非确定性不是 bug，它是 LLM 的原生属性。但 ForgeOS 的验证管线尚未为此建立任何契约：

```go
// forge-core/internal/converge/converge.go:120-135
func Converge(stop asset.StopCondition, sig Signals) (results []Result, met bool) {
    // 它回答 YES/NO，但没有任何机制回答：
    // - "这个 YES 和上一轮的 YES 是同一个 YES 吗？"
    // - "如果重新跑一次 implementer，得到完全不同的实现但都 pass gate——这是收敛还是发散？"
    // - "agent 的输出波动幅度是否在可接受范围内？"
}
```

更具体的问题：

**① 无 prompt 版本锁定**: 每次 `forge run build` 的 `buildPrompt` 调用产出的 prompt 内容可能不同——不因为 intentional 改动，而是因为 memory 追加了新条目、trace 追加了新事件、scorecard 发生了衰减。即使 `p`（phase）、`mode`、`tier` 完全相同，`Gather` 的结果也可能不同。没有 mechanism 来「锁定 prompt 版本」以隔离非确定性源。

```go
// forge-core/cmd/forge/prompt_context.go
func buildPrompt(root string, p asset.Phase, mode string, ...) string {
    // 每次调用重新读取 ROADMAP.md、memory.jsonl、trace.jsonl 等
    // 这些文件在两次连续调用之间可能已经变化
}
```

**② 无输出稳定性度量**: 对一个 phase 连续跑两次（滚动重跑、loop-back 重跑、`--resume` 恢复后重跑），系统没有机制检测两次输出是否**语义等价**。当前只有机械 diff（`FileDelta` 基于 git diff），但「两次不同措辞的代码实现是否做了同一件事」——无评价。

**③ 无 seed/temperature 控制**: `claudeArgv` 构建的命令行参数中没有 `--temperature` 或 `--seed` 参数。agent 每次调用使用 LLM 的默认随机参数，无法复现。

```go
// forge-core/cmd/forge/engine_build.go:76-80
func claudeArgv(o runOpts, isClaude bool, tier string, p asset.Phase) []string {
    // 构建 --model、--permission-mode、--max-budget-usd 等参数
    // 但从不设置 --temperature 或 --seed
}
```

### 边界场景

| 场景 | 问题 | 严重度 |
|------|------|--------|
| loop-back 重跑 implementer 产生完全不同代码 | `FileDelta` 可能变小（新实现改更少文件）而误判进展正常 | 收敛误判 |
| CI 中的 `forge accept` 和本地的 `forge accept` 结果不同 | 因为 memory/trace 状态不同导致 gate 行为不同 | 不可复现 |
| `--resume` 恢复后重跑 phase N，agent 给出不同输出 | checkpoint 恢复的假设（「相同工作相同结果」）不成立 | 数据不一致 |
| 两次 `forge run build` 用相同输入得到不同产出 | 用户无法判断哪个产出是「正确的」 | 信任危机 |
| evolve 迭代中 memory 增长→prompt 内容变化→agent 决策漂移 | 即便目标没变，agent 的「理解」因更多上下文而变化 | 收敛不稳定 |

### 建议方向

1. **Prompt 指纹与版本锁定**: 为每次 prompt 装配产出一个内容哈希（SHA-256），记录在 trace 事件中。提供 `--lock-prompt` 开关：打开后 `Gather` 从缓存读取（与 `ContextCache` 协作），确保同一 phase 的 prompt 在单次 run 内可复现。
2. **输出稳定性契约**: 定义一个接口 `StabilityChecker`，对同一个 phase 的两次输出执行结构化比较——不只是 git diff 行数，还包括：导出符号集是否一致、API 签名是否兼容、测试覆盖率是否在同一量级。非确定性波动在可接受范围内时标记为「稳定通过」，超出范围时告警。
3. **Seed/Temperature 传播**: `routing.TierFor` 的返回增加可选参数字段 `temperature`（haiku=0.7, sonnet=0.5, opus=0.3 等默认值可配），`claudeArgv` 传播 `--temperature` 和 `--seed`（基于 run ID 哈希）。使重跑相同的 `(run_id, phase)` 得到尽可能一致的输出。
4. **实验性「双轨验证」**: 对关键 phase（reviewer/architect 裁决），用相同 prompt 连续调用两次 model（相同 tier），比较两个输出的裁决一致性。如果两次裁决不同（如一次 APPROVE、一次 REQUEST_CHANGES），触发人工审查。

### 代码入口点

| 文件 | 行号 | 改动点 |
|------|------|--------|
| `cmd/forge/prompt_context.go` | `Gather` / `buildPrompt` | prompt 指纹计算 + 版本锁定 |
| `cmd/forge/engine_build.go` | 76-80 `claudeArgv` | temperature/seed 参数 |
| `internal/routing/routing.go` | `TierFor` 返回值 | 增加温度/seed 参数 |
| `internal/converge/converge.go` | `Signals` | 增加 `OutputStability` 信号 |
| `internal/trace/trace.go` | `Event` 结构 | prompt 指纹字段 |

---

## 方向三 · 知识组织架构：从追加日志到结构化语义层

> **优先级**: 🟠 **P1** | **类别**: 架构 · 知识管理 | **预估**: ~4 sprints（跨阶段）  
> **差异化证明**: 关键词 `knowledge graph`、`concept map`、`semantic network`、`ontology`、`knowledge schema` 在全部 85+ 篇已有分析文档中 **零命中**。已有 memory 分析文档（`second-order-architectural-gaps.md` 方向一「知识质量衰减」、`architect-product-perspective-five-directions.md` 方向一「Memory 知识生命周期」）聚焦于**数据层的质量衰减、去重、优先级排序**。本文方向三聚焦的**不是数据层，而是 schema 层**——当前没有任何结构化的知识表示，知识以纯文本嵌入在文件格式中（Markdown/JSONL），系统无法在概念层面推理。

### 问题描述

ForgeOS 积累了大量的项目知识，存储为三种不同的格式：

```
.agent/                     # 治理声明（YAML/Markdown 格式）
  ROADMAP.md                # ❌ 勾选框 → 只有 done/not-done 二元状态
  PROJECT.md                # ❌ 散文 → 无结构化提取
  
.forge/                     # 运行时知识（JSONL/Markdown 格式）
  memory.jsonl              # ❌ 追加 JSONL → Entry Kind/Topic/Detail，无关系
  trace.jsonl               # ❌ 时序事件流 → 无状态聚合
  scorecards.json           # ✅ 有 schema → 但只有模型评分
  
docs/adr/                   # 架构决策记录（Markdown 格式）
  0001-xxx.md               # ❌ 散文 → 决策未提取为结构化数据
```

当前的知识表示有四个结构性缺口：

**缺口 A：无概念/关系层**

```go
// forge-core/internal/memory/memory.go
type Entry struct {
    Format     string  // "forgeos.memory.v1"
    Kind       string  // "decision" | "gap" | "lesson" | "choice" | "insight"
    Topic      string  // free-text topic
    Detail     string  // free-text detail
    Confidence float64 // 0..1
    Iteration  int
    Timestamp  int64
    // ❌ 没有关系声明：Entry A 和 Entry B 是矛盾/互补/替代/因果的关系
    // ❌ 没有概念引用：Topic 是自由文本，无法链接到 ROADMAP 的某个 item 或 ADR 的某个 decision
}
```

Memory 条目之间没有关系。当 session 1 的 implementer 写 `kind:"decision", topic:"database", detail:"use PostgreSQL"`，session 2 的 engineer 写 `kind:"decision", topic:"database", detail:"use SQLite"`，两者在系统中共存，**没有任何机制检测到矛盾**。系统无法区分「知识积累」和「知识相互覆盖」。

**缺口 B：ADR 决策未被结构化提取**

```go
// forge-core/internal/adr/adr.go — 全部导出符号：
//   Validate(path) []Result      ← 只做格式校验
//   ValidateAll(dir) []Result    ← 批量校验
// 零函数从 ADR 提取结构化决策供路由或 gate 消费。
```

ADR-0001~0004 包含项目的核心架构决策：
- ADR-0001: v0-v1 骑 Claude Code
- ADR-0002: Go 核心 Polyglot 栈
- ADR-0003: agent-os submodule
- ADR-0004: REVIEW 段裁决

但这些决策只存在于人类可读的 Markdown 中。`internal/routing` 的模型选择逻辑、`internal/mode` 的治理策略、`internal/orchestrator` 的编排行为——**完全不参考 ADR 的内容**。

**缺口 C：ROADMAP 只有二元状态**

ROADMAP.md 的 `- [x]`/`- [ ]` 只有 done/not-done。没有 in-progress、blocked、superseded、deprecated 等状态。当 `computeFileDelta` 读 ROADMAP 时，它只能回答「声称做完 vs. 实际改了文件」，不能回答「声称做完 vs. 实际测试全绿 vs. 用户确认完成」。

**缺口 D：trace 事件丢失状态聚合**

Trace 是时序事件流（`seq` 单调递增），但没有聚合视图。要回答「当前系统知道什么」需要重放所有 trace 事件并自行推理出当前状态——没有任何机制维护「迄今为止的知识摘要」或「当前已知概念集合」。

### 边界场景

| 场景 | 问题 | 影响 |
|------|------|------|
| Session 1 说「用 PostgreSQL」，Session 2 说「用 SQLite」 | 两条目共存，无矛盾检测，后续 prompt 可能同时推荐两者 | 指导冲突 |
| ADR-0002 说「Rust forge-runtime v3」，但 routing 从未参考 | 架构决策与运行时行为漂移 | 治理退化 |
| ROADMAP 的 `[x] implement auth` 经 `computeFileDelta` 匹配为「实现完成」 | 实际的 auth 实现可能有安全缺陷，但 gate 只看文件改动有无 | 误判完成 |
| Memory 积累了 200 条 `kind:"lesson"`，但都是低价值状态快照 | 注入 prompt 的 memory 内容信噪比极低，稀释硬约束 | prompt 质量下降 |
| 新 agent 想查询「这个项目之前为什么选择了 Go？」 | 答案在 ADR-0002 的散文中，没有简单的方式查询 | 知识不可达 |

### 建议方向

1. **知识 item 结构化 schema**: 定义 `KnowledgeItem` 结构（独立于 `memory.Entry`），包含 `Subject`（概念）、`Predicate`（关系：`uses`/`rejects`/`depends_on`/`contradicts`/`supersedes`）、`Object`（另一个概念）、`Provenance`（来源 ADR/phase/trace seq）、`Status`（active/superseded/deprecated/contradicted）。使系统能回答「当前生效的数据库选型是什么？」
2. **ADR front-matter 结构化提取**: 为 ADR 定义 YAML front-matter schema（`decision:` / `status:` / `applies_to:` / `constraints:` / `supersedes:`），`forge-core/internal/adr` 扩展为结构化解析器，供 `internal/routing`、`internal/mode`、`internal/orchestrator` 查询当前生效的架构约束。
3. **ROADMAP 状态扩展**: 从二元 `[ ]`/`[x]` 扩展为 `[ ]`（未开始）、`[/]`（进行中）、`[x]`（完成）、`[~]`（已废弃）、`[!]`（阻塞）。`evaluateRoadmap` 消费扩展状态，使收敛判定可以区分「声称完成」和「验证完成」。
4. **概念索引层**: 新增一个轻量的概念倒排索引（独立于 memory 的 TF-IDF），维护 `concept → [knowledge items, ADR references, ROADMAP items, trace events]` 的映射。当 agent 进入新 phase 时，注入的不只是 memory 条目，还包括**该 phase 涉及的概念的完整知识摘要**。

### 代码入口点

| 文件 | 行号 | 改动点 |
|------|------|--------|
| `internal/memory/memory.go` | `Entry` struct | 增加 `Relations []Relation` 字段 |
| `internal/adr/adr.go` | `Validate` | 扩展为结构化解析器 |
| `cmd/forge/prompt_context.go` | `memoryContext` | 从追加注入变为概念摘要注入 |
| `internal/converge/converge.go` | `computeRoadmapCompletion` | 消费扩展 ROADMAP 状态 |
| `internal/prompt/retrieve.go` | `Retrieve` | 增加概念索引查询 |

---

## 方向四 · 多维资源核算：从孤立防护栏到统一容量规划

> **优先级**: 🟡 **P2** | **类别**: 运营 · 容量规划 | **预估**: ~2 sprints  
> **差异化证明**: 已有文档覆盖了「预算防卫」（`BudgetAdjustTier`/PR4 hard-stop）、「成本估算」（`five-systemic-oversights-v45.md` 方向四）和「资源安全护栏」（recursion/budget/output/timeout 四维）。但**没有一个文档覆盖多维度资源的统一核算体系**——能将 run 的 token 消耗、墙钟时间、磁盘 IO、内存占用、API 调用次数统一计入一个「资源账本」，并基于此回答「我能负担得起跑一次 `forge evolve` 吗？」。

### 问题描述

ForgeOS 有四个独立的资源护栏：

| 维度 | 机制 | 位置 | 独立工作？ |
|------|------|------|-----------|
| Agent spawn 深度 | `FORGE_AGENT_DEPTH` + `MaxDepth` | `command_executor.go:150` | ✅ |
| Agent 调用次数 | `Engine.MaxAgentCalls` | `orchestrator/budget.go` | ✅ |
| 单命令输出大小 | `cappedBuffer` + `MaxOutputBytes` | `command_executor.go:180` | ✅ |
| 单命令墙钟时间 | `Timeout` + process group | `command_executor.go:141` | ✅ |
| 总花费 | `--max-budget-usd` | `orchestrator/budget.go` | ✅ |

但这些护栏有四个结构性缺口：

**缺口 A：各自为政，无统一账本**

每个维度独立跟踪、独立封顶。系统能回答「是否超了 agent call 上限」——但不能回答「当前 run 累计消耗了 80% 的 budget、70% 的允许 agent call、60% 的墙钟预算，预测再跑 3 个 iteration 将触及哪个上限」。

```go
// forge-core/internal/orchestrator/budget.go
// checkAgentBudget: 只比较 agentCalls 和 MaxAgentCalls
// checkRunBudget: 只比较 totalSpent 和 maxBudgetUSD
// cap: 在 AgentBudget 侧有，但不参与 run-level 的预测
```

**缺口 B：磁盘空间无监控**

`.forge/` 目录中的 `trace.jsonl`、`memory.jsonl` 随使用时间单调增长。`checkpoint.json` 的每次 Save 覆盖旧文件（大小稳定），但 trace 和 memory 文件**从不轮转或压缩**：

```bash
# 当前工作树的 .forge/ 文件统计
$ wc -c .forge/trace.jsonl .forge/memory.jsonl .forge/checkpoint.json
# trace 和 memory 随每次 forge run 增长
# 无大小上限，无自动轮转，无磁盘满预检
```

在 24h 无人值守的 `forge evolve` 场景下，trace 文件可以增长到 GB 级别。如果 `.forge/` 所在的文件系统满了，`persist.Save` 的 `write + rename` 原子提交会失败——但不是「优雅失败」，而是 `write` 返回 `ENOSPC`，`Save` 返回 error。谁消费这个 error？当前只有 checkpoint 路径有错误处理，trace 的 `Emit` 错误没有被上游关注。

**缺口 C：无「预检」机制**

用户调用 `forge run build` 或 `forge evolve` 时，系统没有 `--dry-run` 级别的资源预检：
- 「这个 run 预计需要多少次 agent call？预算够吗？」
- 「预计写多少 trace 数据？磁盘空间够吗？」
- 「预计跑多少 phase？按当前 model tier 估算，总花费是多少？」

`forge route` 可以手动路由查询，`forge doctor` 可以做环境诊断——但没有任何一个命令可以**预先估算一次 run 的资源消耗并对照可用资源**。

**缺口 D：跨 run 资源累积无可见性**

```
Run 1: forge run build → 耗时 2m，花费 $0.18
Run 2: forge run build → 耗时 3m，花费 $0.22
Run 3: forge run evolve → ？？？
```

用户跑第三次时，不知道前两次已累计花费。`scorecards.json` 累积了历史模型调用信息，但没有「累计账单查看」功能——`forge status` 不回答「这个项目本月在 AI agent 上花了多少钱」。

### 边界场景

| 场景 | 问题 | 严重度 |
|------|------|--------|
| `.forge/trace.jsonl` 增长到 2GB | 读取 trace 的 `scorecard-update.mjs` 变慢，`prompt_context.go` 的 `Gather` 变慢 | 性能退化 |
| 磁盘满导致 `trace.Emit` 返回 error | 当前 error 被忽略，trace 数据静默丢失 | 数据丢失 |
| 用户设 `--max-budget-usd=10`，但第一个 `claude-opus-4` 调用就花了 $5 | 余量只够跑 1-2 个 phase，但系统不知道 | 预算浪费 |
| 团队共享项目目录，多人轮流跑 `forge run` | 累计花费分散在各自 session 中，总和不可见 | 成本失控 |
| `memory.jsonl` 在 evolve 中从 100KB 增长到 100MB | prompt 的 memory lane 注入时间线性增长 | 性能退化 |

### 建议方向

1. **ResourceAccount 统一账本**: 新增 `internal/account/` 包，定义 `ResourceAccount` 结构，记录 run 级别累计的：`agent_calls`、`wall_clock_seconds`、`token_count`（估计）、`cost_usd`、`disk_write_bytes`、`trace_events`。在 `Engine` 级别统一跟踪，每次 phase 完成后更新。`account` 包的唯一职责：加法和比较。
2. **Pre-flight resource estimator**: 扩展 `cmd/forge/route.go` 的 `forge route --cost` 为 `forge plan --resource-usage`：读 workflow YAML → 枚举所有 phase → 按 model tier 查 scorecard 的历史均值 → 输出估算。不加新的 YAML schema，只利用已有数据。
3. **`.forge/` 磁盘空间监控**: 在 `Engine` 启动前执行 `diskFreeCheck(path)`：如果 `.forge/` 所在文件系统的可用空间 < `MinFreeBytes`（默认 500MB），拒绝启动。在 run 过程中，每次 `trace.Emit` 后检查文件大小是否超过 `MaxTraceBytes`（默认 500MB），如果超过则轮转 trace 文件并归档旧文件。
4. **跨 session 累计视图**: `forge status --cost` 聚合 `scorecards.json` 中所有历史记录，输出按日/周/月的累计花费。`forge status --disk` 输出 `.forge/` 目录各文件大小。
5. **三者协同不叠加**: 已有预算防御（`BudgetAdjustTier`/hard-stop）保持不变——accounting 是只读的预测和观测层，不增加新的 fail-closed 点。`forge run` 的默认行为不变；新增的 pre-flight 和监控是 advisory。

### 代码入口点

| 文件 | 行号 | 改动点 |
|------|------|--------|
| `internal/orchestrator/budget.go` | 全部 | 重构为 `ResourceAccount` 的一部分 |
| `cmd/forge/evolve.go` | `buildLoop` | 启动前磁盘空间预检 |
| `internal/trace/trace.go` | `Emit` | 写入后文件大小检查 + 轮转 |
| `cmd/forge/route.go` | `--cost` | 扩展为 `forge plan` |
| `cmd/forge/main.go` | `status` 子命令 | 增加 `--cost`/`--disk` flag |

---

## 方向五 · 边界情况分类学：系统在极端条件下的行为映射

> **优先级**: 🟠 **P1** | **类别**: 可靠性 · 测试基础 | **预估**: ~2 sprints（分类框架 + 自动化测试）  
> **差异化证明**: 已有分析偶尔提及个别 edge case（如 `forgotten-five-foundations.md` 提及 fork-bomb 和并发写入，`expansion-production-readiness.md` 提及 529 过载），但**没有一个文档系统性地列出每个核心子系统的边界条件矩阵**。Sprint 演进中遇到 edge case 时逐个修复（如 Sprint 27 的 `yaml2json` block-scalar 损坏、Sprint 31 的 FIFO regulator），但修复模式是「撞到→修掉」，不是「预扫→覆盖」。本文提议的是**将 edge case 从被动发现提升为主动测试分类**。

### 问题描述

ForgeOS 的核心子系统已经进行了大量的正常路径测试（`go test -race` 全绿，707+ 测试用例）。但在极端、退化、空值、并发、资源耗尽场景下的行为，大部分未经过系统性验证。

以下五个维度构成了 edge case 矩阵的五轴，每轴在每个核心子系统上的行为大部份未被覆盖：

**轴 1：零值/空值输入**

| 子系统 | 场景 | 当前行为（推测） | 是否验证？ |
|--------|------|----------------|-----------|
| `internal/gate/resolve.go` | 空 `Gates` 列表 | `GatesGreen` 全绿？ | ❌ |
| `internal/converge/converge.go` | 空 `Signals{}` | `Met` 是 true 还是 false？ | ❌ |
| `internal/routing/routing.go` | 空 `agent` 参数 | `TierFor("", "engineering")` 返回 Sonnet（走 defaultFor） | ❌ 隐式依赖默认值 |
| `cmd/forge/prompt_context.go` | `Gather` 在空 ROADMAP 文件 | 空字符串 / error / agent prompt 中缺失 Roadmap 段？ | ❌ |
| `internal/memory/memory.go` | 在空 `memory.jsonl` 上 `Load` | 空切片，正常路径？ | ✅（tests cover empty file）? |
| `internal/trace/trace.go` | 在空 `trace.jsonl` 上 `Replay` | 空结果 / error？ | ❌ |

**轴 2：最大值/极限输入**

| 子系统 | 场景 | 边界 |
|--------|------|------|
| `internal/prompt/retrieve.go` | 1000 条 memory 条目上 `Retrieve` | O(kN) 复杂度，性能退化？ |
| `internal/orchestrator/parallel.go` | 100 个并行 phase | 并发 map 访问 + goroutine 泄漏？ |
| `cmd/forge/prompt_context.go` | 1MB 的 ROADMAP 文件 | OOM / 静默截断 / 异常慢？ |
| `command_executor.go` | 10GB stdout | `cappedBuffer` 限制为 10MiB，但 drain 部分仍需读完 10GB → 墙钟？ |
| `internal/persist/checkpoint.go` | 每 10ms 一次 Save | 写放大 + 文件系统磨损？ |

**轴 3：损坏/退化输入**

| 子系统 | 场景 | 当前行为 |
|--------|------|---------|
| `internal/yaml2json/` | 截断的 YAML 文件（`description: >\n  truncated`） | `parseBlockScalar` 会不会 panic？ |
| `internal/persist/checkpoint.go` | 手动编辑过的 `checkpoint.json`（格式错误） | `json.Unmarshal` 返回 error → `found=false` → 从头开始？ |
| `internal/trace/trace.go` | `trace.jsonl` 中间一行的 JSON 损坏 | `bufio.Scanner` 读到该行返回 error → 停止处理？跳过？ |
| `internal/memory/memory.go` | `memory.jsonl` 中包含二进制数据 | `json.Unmarshal` 失败 → 跳过该行？全部拒绝？ |
| `internal/gate/resolve.go` | `GatesGreen` 中某个 gate 返回未知字符串 | `resolveGate` 的 default 分支行为？ |

**轴 4：并发/竞争**

| 子系统 | 场景 | 当前防护 |
|--------|------|---------|
| 两个 `forge evolve` 写同一个 `trace.jsonl` | O_APPEND 写入交错，seq 重复 | ❌ 无防护 |
| 两个 `forge evolve` 写同一个 `checkpoint.json` | 最后写入者胜 | `write + rename` 保证原子性但不保证语义正确性 |
| `parallel.go` 的多个 phase 同时调 `checkAgentBudget` | `atomic.AddInt32`？mutex？ | 需验证 |
| `Engine.RunFrom` 在 `--resume` 恢复时和另一个 `forge run` 同时执行 | 两个 orchestrator 竞争 | ❌ 无防护 |

**轴 5：外部环境故障**

| 场景 | 当前行为 |
|------|---------|
| `ANTHROPIC_API_KEY` 未设置 | `claude` 命令 exit 非零 → `KindFailed` → 不可重试？ |
| 网络断开（agent CLI hang 直到 TCP 超时） | `Timeout` 默认 0（永不超时）→ 永久挂起 |
| 文件系统只读 | `persist.Save` error → `forge run` fail？ |
| `/tmp` 满 | `cappedBuffer` 内存分配 OOM？ |
| 系统时钟在 run 期间回跳（NTP 校正） | `time.Now()` 返回过去的值 → latency 为负？`duration_ms` 为负？ |
| `claude` 命令返回 exit code 137（SIGKILL/OOM） | `classifyRunErr` 分类为 `KindConfig`（不可重试）→ 实际上应是 `KindOverloaded` |

### 建议方向

不编写代码，而是建立 **edge case 矩阵**和一个系统化的主动测试框架：

1. **建立 edge case 矩阵文档**: 为每个核心子系统（orchestrator、converge、routing、memory、trace、persist、prompt、gate、yaml2json）建立 `EdgeCases.md`，列出上述五轴中的已知/推定行为，附代码链接。
2. **Fuzz 测试基础设施**: 为 `internal/yaml2json`、`internal/converge`、`internal/memory`、`internal/persist` 增加 go-fuzz 风格的模糊测试，用结构化/半结构化/随机的输入验证 panic 安全。不需要外部依赖（`testing/quick` + `math/rand` 即可）。
3. **注入式故障测试**: 为 `internal/trace` 和 `internal/memory` 增加可注入的文件系统故障模拟层（类似 `internal/orchestrator` 已有的 `Sleep`/`Now` 可注入时钟模式），验证写入失败 → Emit error → 上游处理的完整链路。
4. **并发安全测试**: 为 `trace.go` 的 `Emit`（多 goroutine 写同一文件）、`memory.go` 的 `Append`（多 goroutine）、`persist.go` 的 `Save`（多进程）增加 `-race` 覆盖下的并发压力测试，验证 lock 覆盖完整、data race 为零。
5. **边界条件 CI job**: 新增一个 `forge-test-edge` CI job（不阻塞 PR merge，advisory），每周跑一次 edge case 矩阵中的混沌场景，输出 edge case 覆盖率报告。gate 行为不变。

### 当前已验证的 edge case（不应重复覆盖）

Sprint 中已经修复并验证过的一些 edge case（本文不做重复提议）：

| Edge case | Sprint | 已覆盖 |
|-----------|--------|--------|
| 空 `test_acceptance` glob（zero-match exit 0） | 方向三 fix | ✅ |
| `childEnv` 重复键跨 libc 未指定 | Sprint 20 | ✅ |
| 负 `keepPerKind` 在 `Compact` 中 panic | Sprint 27 | ✅ |
| `FileDelta` 在 git 未初始化仓库中 panic | Sprint 29 | ✅ |
| 529 overload → `KindOverloaded` + backoff | 方向四 PR1 | ✅ |
| YAML block scalar 损坏 | Sprint 27 | ✅ |
| 递归 fork-bomb（`FORGE_AGENT_DEPTH` cap） | Sprint 20 | ✅ |

### 代码入口点

| 文件 | 行号 | 对应的 edge case 测试点 |
|------|------|------------------------|
| `internal/yaml2json/yaml2json.go` | 全部 | fuzz: 截断/嵌套/超大输入 |
| `internal/trace/trace.go` | `Emit` | 并发写 + 损坏恢复 |
| `internal/converge/converge.go` | `Converge` | 空 Signals + 负值字段 + NaN |
| `internal/persist/checkpoint.go` | `Save`/`Load` | 截断文件 + 权限错误 + 磁盘满模拟 |
| `internal/memory/memory.go` | `Append`/`Load`/`Compact` | 二进制数据 + 0 条目 + 超大条目 |
| `cmd/forge/prompt_context.go` | `buildPrompt`/`Gather` | 空 ROADMAP + 超长 ADR + memory 缺失 |

---

## 优先级收敛建议

| 方向 | 优先级 | 类别 | 一句话杠杆 |
|------|--------|------|-----------|
| **① OS 级安全边界** | P0 | 安全 | 同机器任何用户 `ps aux` 即可读取全部 prompt 和 API key——这是最直接的信任危机 |
| **② Agent 输出确定性契约** | P1 | 架构·信任 | 验证管线的根基假设（输出的可复现性）当前不成立，但不妨碍功能使用，属于长期信任债 |
| **③ 知识组织架构** | P1 | 架构·知识 | 系统当前只能「存和取」知识，不能「理解和关联」知识——semantic gap 限制系统真正变聪明的天花板 |
| **④ 多维资源核算** | P2 | 运营 | 已有护栏够用，缺少的是「可见性」和「预测能力」——但无人值守 24h 场景才暴露此缺口 |
| **⑤ 边界条件分类学** | P1 | 可靠性 | edge case 矩阵 + fuzz 是「被动撞 bug」到「主动防 bug」的测试基础设施升级，收益随仓库增长放大 |

### 实施建议

- **方向一（P0）建议立即启动**: 将 prompt 从 CLI arg 移到 stdin/tempfile 是纯机械改动（不改任何语义），但解决了一个真正紧急的安全缺口。`childEnv` 的 allowlist 同样如此。这两项可以并行进行，各自独立验证。
- **方向二（P1）和方向三（P1）可以渐进式进行**: 不需要大爆炸式重构。方向二可以先从 `--seed` 参数传播开始（最简单的改动，杠杆最高）；方向三可以先从 ADR front-matter 结构化解析开始（最小的 schema 增量）。
- **方向五（P1）可以作为一个 SRE/测试专项 Sprint**: 不添加功能，只添加 edge case 覆盖。建议在一个 Sprint 中集中完成所有核心子系统的 edge case 矩阵 + fuzz 入口点，后续作为持续维护的测试文化。
- **方向四（P2）建议随下次真点火部署准备**: 在用户开始 24h 无人值守 `forge evolve` 之前，确保 `--disk-space-precheck` 和对 `trace.jsonl` 增长的监控就绪。

---

*本文基于对 forge-core 18 Go 包 / ~35k LOC / 707+ 测试用例 / 31 轮 Sprint / 85+ 已有分析的全局扫描。每个方向经过交叉验证确认未被已有分析覆盖，附精确到 `file:line` 的代码级证据。*
