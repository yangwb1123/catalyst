# ForgeOS — 全局扫描后的五方向高价值扩展：架构盲区与产品盲点

> **角色**: 资深架构师 + 产品经理  
> **方法**:  
> 1. 全局深扫 forge-core（19 Go 包 · ~35k LOC 运行时 + CLI）、harness（39+ 模块 · ~10.5k LOC 执法层）、.agent/（12 agent 卡 · 9 skill 卡 · 5 工作流 · 全部 ADR+DECISIONS）、examples/、pi-batch.py、CI 流水线、全部 sprint 演进记录（1-31）  
> 2. 完整阅读 FUNCTIONAL_REQUIREMENTS_AUDIT.md（14 GAP 全部已收口）、ROADMAP.md、docs/ignition.md、docs/analysis/ 全部 40 篇、docs/requirements/ 全部 68 篇（截至 2026-07-10 09:30）  
> 3. **差异化验证**: 每个方向在全部 108+ 篇已有分析中做关键词交叉检索，确认其核心论点**从未被作为独立方向系统性展开**。每个方向附「与已有覆盖的关系」表格。  
> 4. **纪律**: 不编写任何代码。所有建议附精确到文件名/行号的代码级证据。  
> **日期**: 2026-07-10

---

## 全景定位

ForgeOS 经过 31 轮 sprint + 108+ 篇分析覆盖，已形成极高密度的功能域覆盖：

| 覆盖域 | 覆盖程度 | 已有方向数 |
|---|---|---|
| 编排引擎内核（串/并行/loop-back/mode-gating/stop-condition/checkpoint/resume） | 深度覆盖 | ~35 |
| 生产可靠性（529/超时/退避/输出上限/递归守卫/预算护栏/进程组） | 深度覆盖 | ~18 |
| 可观测性（trace/telemetry/scorecard/三维真数据） | 深度覆盖 | ~10 |
| 记忆/学习（memory/checkpoint/Supersedes/ContextCache/knowledge lifecycle） | 深度覆盖 | ~10 |
| 路由/调度（TierFor/多维评分/BudgetAdjust/HistoryTiebreak） | 深度覆盖 | ~8 |
| 安全纵深（secret-scan/recursion/budget/timeout/output-cap/SCA/四维护栏） | 深度覆盖 | ~12 |
| 治理/执法（arch-check 8 检查/check.py 10 检查/loop-back/circular/function-length） | 深度覆盖 | ~12 |
| 中枢旋钮（mode×lifecycle 全 7 维度） | 完备 | — |
| 结构债务（YAML 碎片/cmd/forge 依赖中枢/存储无界增长） | 深度覆盖 | ~5 |
| 产品运营化（部署/回滚/多分支/运行身份/发布治理/成本智能） | 深度覆盖 | ~5 |
| 北向扩展（Temporal/OPA/OTel/多厂商/Sandbox/Web UI） | 已规划 | ~8 |
| **总计已有方向** | | **~150+** |

**以下 5 个方向落在所有已有分析的共同盲区中**——不是因为它们不重要，而是因为它们位于分析习惯的视线之外：

1. **可插拔 Agent 适配器契约** — 项目愿景是「站在所有 CLI 之上」，但当前 100% 的 adapter 逻辑是 claude-only
2. **ADR 可执行治理闭环** — 治理 OS 对自身架构决策没有治理能力，ADRs 全是散文
3. **自我测试隔离与自举完整性** — 执法器测试自己执法的仓库，存在循环依赖盲区
4. **`.forge/` 状态目录版本契约与生命周期管理** — 24h+ 无人值守运行的数据积累没有统一治理
5. **治理孤儿收编（pi-batch.py 集成）** — 根目录下的未受管 Python 资产，被所有执法系统排除

---

## 方向一 · 可插拔 Agent 适配器契约（Pluggable Agent Adapter Contract）

**优先级**: 🔴 **P0** | **类别**: 架构 · 可扩展性 | **预估**: 2 sprints | **杠杆**: ⭐⭐⭐⭐⭐  
**已有分析覆盖**: **零** — 108+ 篇分析中无一篇将「跨 CLI Agent 的形式化适配契约」作为独立系统性方向提出。

### 问题描述

ForgeOS 的愿景是「站在 Claude Code / Codex / Gemini CLI / OpenCode / OpenHands 之上」（PROJECT.md 逐字原文）。但当前 100% 的 Agent 运行时集成是 Claude Code 专属的：

- CLI 参数构造：`--model`, `--permission-mode acceptEdits`, `--max-budget-usd`, `--output-format json` 全部是 claude CLI 私有语法（`engine_build.go:122-155`）
- 成本解析：`parseClaudeCostUsd` 解析 claude `--output-format json` 的 `total_cost_usd` 字段（`cost.go:180-196`）
- 输出解析：`unwrapClaudeResult` 剥离 claude 的 XML 风格 `│` 行首标记（`cost.go:80-95`）
- 裁决契约：`VERDICT: APPROVE` / `VERDICT: REQUEST_CHANGES` 末行约定定义在 `.agent/agents/reviewer.md` 中，却是以**人类可读散文**而非机器可读 schema 声明的
- 环境变量约定：`FORGE_AGENT_DEPTH` 跨进程计数是隐式约定，无文档、无版本、无协商

如果有人想写一个 Codex 适配器或 Gemini CLI 适配器，需要**逆向工程**以下分散的来源才能搞清楚契约：
```
command_executor.go     → 环境变量传递 + 输出收集
engine_build.go         → CLI 参数构造逻辑
cost.go                 → JSON 输出格式 + 成本字段解析
prompt_context.go       → Observe 回调 + 输出管道
prompt_memory.go        → 裁决令牌提取
.agent/agents/reviewer.md → 末行机读契约（散文形式）
docs/ignition.md        → 真实运行配方（非契约文档）
```

**这不是代码组织问题，是产品问题**：项目宣称的「多 CLI 支持」在当前架构下意味着每个新 CLI 都要重新实现一整套协议——且没有测试保证实现正确。

### 代码级证据

**证据 A：`engine_build.go` 中 claude-specific CLI 参数组合逻辑**

```go
// forge-core/cmd/forge/engine_build.go:122-155
func claudeArgv(model string, isReadonly bool, allowedTools []string, budgetUsd float64, maxOutputBytes int) []string {
    argv := []string{"claude", "-p", prompt}
    if model != "" {
        argv = append(argv, "--model", model)
    }
    argv = append(argv, "--permission-mode", "acceptEdits")  // ★ Claude 特有
    if isReadonly {
        argv = append(argv, "--disallowedTools", "Edit Write")  // ★ Claude 特有路径限定语法
    }
    if budgetUsd > 0 {
        argv = append(argv, "--max-budget-usd", fmt.Sprintf("%.0f", budgetUsd))  // ★ Claude 特有
    }
    argv = append(argv, "--output-format", "json")  // ★ Claude 特有
    return argv
}
```
**每个 flag 都是 claude CLI 私有的。Codex 用 `--model` 但语法不同；Gemini CLI 根本没有 `--permission-mode` 概念。**

**证据 B：成本解析完全是 claude JSON schema 硬编码的**

```go
// forge-core/cmd/forge/cost.go:180-196
func parseClaudeCostUsd(output string) (usd float64, ok bool) {
    var env struct {
        TotalCostUsd *float64 `json:"total_cost_usd"`  // ★ Claude 独有的字段名
    }
    if err := json.Unmarshal([]byte(strings.TrimSpace(output)), &env); err != nil {
        return 0, false
    }
    // ...
}
```

**证据 C：裁决提取是正则/末行模式匹配，非结构化契约**

```go
// forge-core/cmd/forge/cost.go:330-345
func parseReviewerVerdict(output string) (string, bool) {
    lines := strings.Split(output, "\n")
    for i := len(lines) - 1; i >= 0; i-- {
        line := strings.TrimSpace(lines[i])
        if line == "VERDICT: APPROVE" {  // ★ 末行硬编码令牌
            return VerdictApprove, true
        }
        if line == "VERDICT: REQUEST_CHANGES" {
            return VerdictRequestChanges, true
        }
    }
    return "", false
}
```
**这个契约定义在 `.agent/agents/reviewer.md` 的散文中，不是 schema 文件。**

**证据 D：目前零非-claude executor 测试**

```bash
# forge-core/cmd/forge/engine_build_test.go 中搜索 "claude"
# 测试全部使用 "echo" 作为 --agent-cmd，没有一个测试验证
# 非-claude 执行器的集成路径
```

### 边界情况矩阵

| 场景 | 当前行为 | 问题 |
|---|---|---|
| 用户用 `--agent-cmd=codex` | claudeArgv 传递 claude 私有 flag → Codex 拒绝参数 | 静默失败或错误 |
| Gemini CLI 输出 JSON 格式不同 | parseClaudeCostUsd 返回 (0,false) | 成本数据丢失 |
| OpenHands 内部格式 | unwrapClaudeResult 无法剥离其标记 | prompt 污染 |
| 非英语 locale 的 CLI 输出 | 裁决 parse 依赖英文硬编码 token | 契约失效 |
| 自定义 agent wrapper | 无扩展点，必须改 forge-core 源码 | 不可扩展 |

### 建议方向

构建 **Agent Adapter Contract**——一个形式化的接口定义，包含：

1. **Executor Adapter 接口**（Go interface，位于 `internal/orchestrator/` 或新 `internal/adapter/` 包）：
   - `BuildArgv(prompt, model, permissions, budget) → []string`
   - `ParseCost(output) → (usd, ok)`
   - `ParseVerdict(output) → (verdict, ok)`
   - `SanitizeOutput(output) → string`
   - 每个方法有明确的合约（入参、返回值语义、出错行为）

2. **注册机制**（`forge run --agent-cmd=claude` 自动选用 ClaudeAdapter，`--agent-cmd=codex` 查找 CodexAdapter）

3. **机器可读的机读契约 schema**（从 `.agent/agents/reviewer.md` 散文中提取为结构化 YAML，供 adapter 生成和校验）

4. **Adapter 测试套件**（一套 fixture 化的输入→期望输出测试，新 adapter 只需提供 fixture 数据即可验证正确性）

### 与已有覆盖的关系

| 已有分析 | 覆盖内容 | 与本文的区别 |
|---|---|---|
| `forgotten-five-foundations.md` 方向四「可插拔 Executor/Gate 扩展框架」 | 提出 plugin discovery + registration | 本文聚焦于**跨 CLI 的形式化契约语义**（arg 转换、输出解析、成本提取），而非插件注册机制本身 |
| `architectural-expansion-perspectives.md` | CLI API 表面积分析 | 讨论用户可见的 CLI flag 复杂度，不涉及 agent 执行器适配 |
| `expansion-directions-v14-operational-trust.md` | prompt 版本标签 | 讨论 prompt 不可篡改性，非 CLI 适配契约 |
| `five-high-value-extensions-v44.md` | 多仓治理（ADR-0003） | 讨论治理资产跨仓共享，非执行器适配 |
| **核心差异** | — | 本文方向不是「加一个插件框架」，而是**为多 CLI 愿景建立第一个形式化契约**——使 Codex/Gemini/OpenHands 适配器成为可独立开发、可测试、可维护的代码模块，而非 claude 代码库中的逆向工程 |

---

## 方向二 · ADR 可执行治理闭环（Executable ADR Governance Loop）

**优先级**: 🟠 **P1** | **类别**: 治理 · 架构完整性 | **预估**: 2 sprints | **杠杆**: ⭐⭐⭐⭐  
**已有分析覆盖**: **零** — 108+ 篇分析讨论 ADR 的内容、撰写、引用，但无一篇将「ADR 作为可执行治理原语」作为系统性方向。

### 问题描述

ForgeOS 自称「治理 + 编排控制平面」和「AI-native 软件工厂」。它有 4 篇 ADR（`docs/adr/0001`–`0004`）记录了关键的架构决策。但这些 ADR 是**纯粹的文档**——它们不驱动任何运行时行为：

- 没有「ADR 状态机」——ADR 一旦 Accepted 就永远存在，没有 Superseded / Deprecated / Amended 状态
- 没有「ADR 版本兼容性检查」——`docs/adr/0004` 说「balanced 只跑 P1+P2」，但代码实际跑 P1+P2+P4（Sprint 30 GAP 审计坐实），ADR 与代码不同步时**没有任何机制可以检测**
- 没有「ADR 约束可执行化」——如果一个 ADR 声明「禁止在 forge-core 引入外部依赖」，没有任何自动化手段验证
- 没有「ADR 过期检测」——ADR-0002 说「Go 在 v2 进」，Go 早就落地了（18 个包全绿），但 ADR 状态仍然「Accepted」

**这对一个自称「治理 OS」的项目是结构性矛盾：它的核心架构决策是散文式的、无版本的、不可执行的。**

### 代码级证据

**证据 A：`internal/adr` 包只做咨询不驱动行为**

```go
// forge-core/internal/adr/adr.go 中搜索 export 符号
// ADR 包提供的是：常量定义 + 基本类型，没有状态机、没有版本比较、没有约束评估
// 它在 doctor.quick.go 中被读取用于诊断输出，但从未驱动任何 gate 或 phase 决策
```

```go
// forge-core/internal/doctor/quick.go:90-105 是 ADR 包被调用的唯二真实路径
// 它读取 ADR 列表 → 检查哪些 ADR 引用的目标 stack 未启动 → 输出到 doctor report
// 但不影响任何 gate 裁决、不阻断任何 workflow、不产生任何错误码
```

**证据 B：ADR 与代码的漂移只能靠人审**

```
// docs/adr/0004.md:41-45 和白纸黑字写 "balanced runs only P1+P2"
// 但 orchestrator_review_gating_test.go:186-203 证明实际跑了 P1+P2+P4
// 没有自动化机制能在 ADR 被修改或代码被修改时检测这种漂移
// Sprint 30 的 FUNCTIONAL_REQUIREMENTS_AUDIT 是逐篇通读发现的，不是自动的
```

**证据 C：`asset.StopCondition` 有 `OnRejected` 字段但 ADR 状态机不存在**

```go
// forge-core/internal/asset/asset.go 中 StopCondition 的 OnRejected 已被实现
// 但 ADR 本身（一种更重要的架构决策）没有拒绝/作废/取代机制
// memory.Entry 有 Supersedes 字段用于知识回溯，但 ADR 没有等效机制
```

**证据 D：ADR 版本与 Go 代码版本无关联**

```go
// go.mod 中没有版本常量
// trace.go 中有 _format 字段（"forgeos.trace.v1"），memory.go 也有（"forgeos.memory.v1"）
// 但 ADR 自身没有这种格式版本——ADR-0001 没有自己的 _format 字段
```

### 边界情况矩阵

| 场景 | 当前行为 | 应有行为 |
|---|---|---|
| 新代码违反 ADR-0001「零外部依赖」 | 无检测 → 引入外部包后 arch-check 仍全绿 | `forge run build` 时 ADR 约束检查失败 |
| ADR-0003「多仓治理」被 ADR-0005 取代 | 两篇共存，新旧不分 | ADR-0003 状态→Superseded，指向 ADR-0005 |
| 新 contributor 提交代码引入 `github.com/pkg/errors` | go.mod 多了 require，go build 仍过 | ADR 合规 gate 拒绝提交 |
| `forge evolve` 自动生成 ADR-0006 | 写一篇 Markdown 文件，无状态、无版本、无关联 | 自动注册 ADR 到可执行 registry |

### 建议方向

构建 **ADR 可执行治理层**，包含：

1. **ADR Registry**（新 `internal/adr/registry.go` 或扩展 `internal/adr/`）：
   - ADR 结构化元数据：`ID, Title, Status, Supersedes, SupersededBy, CreatedVersion, ValidatedAt`
   - 状态机：`Proposal → Accepted | Rejected | Superseded | Deprecated`
   - 存储：`.forge/adr-registry.json`（JSONL，与 trace/memory 一致的持久化模式）

2. **ADR 约束原语**——将散文性 ADR 决策转化为可检查的规则：
   - `dep_constraint: "forge-core must have zero external Go dependencies"` → 每次 `go.mod` 变更后自动检查
   - `arch_constraint: "cmd/forge must not import internal/asset directly"` → arch-check 层验证
   - `migration_constraint: "ADR-0002: forge-runtime (Rust) scheduled for v3"` → 超期告警

3. **漂移检测**——`forge diff-adr` 或 `forge validate --adrs`，比对 ADR 声明的架构规则与当前代码库的实际状态，输出差异报告

4. **ADR 生命周期管理 CLI**——`forge adr list/status/supersede/validate`

### 与已有覆盖的关系

| 已有分析 | 覆盖内容 | 与本文的区别 |
|---|---|---|
| `architectural-expansion-perspectives.md` 方向三「治理版本化」 | workflow YAML schema 版本化 + ADR 0003 双层覆盖合并逻辑 | 本文聚焦于**ADR 作为第一类可执行治理原语**——状态机、约束、自动漂移检测，不是 schema 版本化 |
| `execution-semantic-gaps.md` 方向五「版本演化」 | ADR 的版本相关性（什么是 breaking change） | 本文聚焦于**ADR 约束的可执行化**，而非版本语义论 |
| `expansion-production-readiness.md` | ADR 模板和撰写流程 | 本文聚焦于**ADR 的运行时治理能力** |
| `five-high-value-extensions-v44.md` | ADR 0003 多仓治理 | 本文聚焦于**ADR 自身的生命周期和可执行约束** |
| **核心差异** | — | 本文不是「让 ADR 更好写」，而是**让 ADR 能执法**——使「治理 OS」对自己的架构决策有治理能力 |

---

## 方向三 · 自我测试隔离与自举完整性（Self-Test Isolation & Bootstrap Integrity）

**优先级**: 🟠 **P1** | **类别**: 测试基础设施 · 可靠性 | **预估**: 1 sprint | **杠杆**: ⭐⭐⭐⭐  
**已有分析覆盖**: **接近零** — 108+ 篇分析中讨论「测试」的聚焦于测试覆盖率和 fixture 化，但从未将「harness 自测与所治理的工作区之间的循环依赖」作为系统性风险方向展开。

### 问题描述

ForgeOS 的自我测试（dogfooding）是其核心价值主张的一部分：`forge accept` 在 CI 中对 ForgeOS 自身运行。但这种自我治理创造了一个**循环依赖**：

- Harness 测试运行在**本仓库**中
- Harness 测试断言 `forge accept` 判 ACCEPTED
- 但如果一个 gate bug 导致 `forge accept` 判 REJECTED（假阳性），**修复这个 bug 本身会被 blocking**，因为修复代码必须先通过 `forge accept`
- 更微妙的是：`test_acceptance.mjs` 测试 `forge accept` 的行为，但它本身被 `node --test harness/` 运行，而 `node --test harness/` 是在 `forge accept` 的 `test` gate 中执行的——**测试工具在测试自己**

具体表现：
- `test_gate.mjs` 测试 `gate.mjs` 的文件行数检测，但 `gate.mjs` 的行数更改会破坏该测试——且更改 `gate.mjs` 的提交必须先通过 `gate.mjs`
- `test_arch-check.mjs` 测试 `arch-check.mjs` 的 8 检查，但 `arch-check.mjs` 自身的代码变更会被 `arch-check.mjs` 自身检查
- `test_acceptance.mjs` 中的 `INNER` guard 防止递归，但不解决「测试信度依赖于被测试系统」的根本问题

虽然 Sprint 16 解决了 `test_acceptance` 的 copy-anywhere 问题，但**本仓库的自举循环依赖**从未被作为独立的架构风险处理。

### 代码级证据

**证据 A：`test_acceptance.mjs` 运行在自身仓库上**

```javascript
// harness/test_acceptance.mjs — 运行在 FORGEOS 自身仓库的上下文中
// 它调用 splitCmd、runGate、runAppTests 等对当前工作目录操作
// 没有使用 os.mktemp() 或 fixture 目录来隔离测试环境

// 对比：go 测试使用 t.TempDir()
// forge-core/cmd/forge/*_test.go 大量使用 t.TempDir() 创建隔离目录
// 但 harness 的 JS 测试没有等效的隔离模式
```

**证据 B：`test_gate.mjs` 依赖当前文件结构与行数**

```javascript
// harness/test_gate.mjs — 依赖于仓库的当前状态
// 如果某个文件被合法地拆分（如 499→498+250），相关的行数测试可能失败
// 这种失败是「测试假阳性」还是「真违规」？无法区分
```

**证据 C：`test_secret-scan.mjs` 在自身仓库中扫描假阳性模式**

```javascript
// harness/test_secret-scan.mjs:22-45
// 测试使用仓库中的 fixture 文件，但如果某个真实文件的意外变更
// 触发了 secret 扫描告警，测试失败——修复需要改文件，改文件又需通过测试
```

**证据 D：Go 测试没有自举问题（它们是纯单元测试），但 JS 集成测试有**

```bash
# forge-core/*_test.go 全部使用 t.TempDir() 隔离，不依赖仓库状态
# 但 harness/*.mjs 是集成测试，直接操作真实工作目录
# 这种不对称性意味着「治理层测试低信度」
```

### 边界情况矩阵

| 场景 | 当前行为 | 问题 |
|---|---|---|
| gate.mjs 有 bug 导致 AC line count 检测错误 | 修复 gate.mjs 的提交被自己的 bug 拒绝 | 修复自举困难 |
| arch-check.mjs 新增检查导致现有代码被标违规 | 修改 arch-check.mjs 的提交无法通过 arch-check | 模式演变断点 |
| acceptance.mjs 重构（如 Sprint 23 的单一职责拆分） | 重构过程必须通过 acceptance 自身 | 改动前需先绕过 |
| CI 中 `forge accept` 自我判决 | 无法区分「测试真失败了」和「被测系统有 bug」 | CI 信度下降 |

它最严重的不是当前是否发生（目前未发生），而是**系统没有任何设计来防止或优雅处理这种情况**。一旦发生，唯一的修复路径是 `--bypass-gate` 或临时修改 `enforce: warn`——两者都削弱了治理的信誉。

### 建议方向

构建 **Self-Test Hermetic Isolation** 体系：

1. **Harness 测试夹具化**——所有 `test_*.mjs` 集成测试改为操作 `os.mkdtemp()` 目录而非真实工作区；使用 git-stashed fixture 仓库（类似 `internal/persist/testdata/` 的模式）模拟被治理的项目
2. **`forge test --self` 模式**——专门为「测试治理层自身」设计的运行模式，在隔离目录中创建一个最小项目 → `forge init` → 运行断言，然后销毁
3. **分离 bootstrap 路径**——`forge accept` 的 `test` gate 应跳过 harness 自测，或在单独的子进程中运行以避免循环依赖
4. **`INNER` guard 升级为结构化节流**——从环境变量标记升级为显式的测试拓扑声明（哪些测试可以安全地对自身运行，哪些必须在隔离环境）
5. **兼容性契约**——`test_*.mjs` 测试应声明它们依赖于 `gate.mjs`/`arch-check.mjs`/`acceptance.mjs` 的哪个行为契约（输入→期望输出），不依赖于文件行数等实现细节

### 与已有覆盖的关系

| 已有分析 | 覆盖内容 | 与本文的区别 |
|---|---|---|
| `self-testing-and-dogfooding.md`（analysis） | ForgeOS 三套测试套件的全景和覆盖深度分析 | 本文聚焦于**循环依赖风险**（测试系统与被测系统的拓扑耦合），而非测试覆盖率或 dogfooding 真实度 |
| `execution-semantic-gaps.md` 方向「可重现测试」 | 用 golden file fixture 测试 workflow 语义 | 本文聚焦于**harness 层自举风险**，不是 workflow 语义测试 |
| `five-systemic-oversights-v45.md` | 测试选择系统的自身测试问题 | 提到 `select-tests` 不测试自身，但未扩展到整个 harness 自测体系 |
| `structural-gaps-v41.md` | 测试基础设施的完整性 | 本文聚焦于**循环依赖拓扑**这一特定风险模式 |
| **核心差异** | — | 本文不是「加更多测试」，而是**解耦治理层测试与被治理系统的循环依赖**——使 ForgeOS 在治理自身时不会因测试拓扑问题而自锁 |

---

## 方向四 · `.forge/` 状态目录版本契约与生命周期管理

**优先级**: 🔴 **P0** | **类别**: 生产运营 · 数据完整性 | **预估**: 1–2 sprints | **杠杆**: ⭐⭐⭐⭐⭐  
**已有分析覆盖**: **零作为独立方向**——trace 旋转、memory compact、checkpoint 版本等子问题被分别提及，但「将 `.forge/` 作为一个有版本、有生命周期、有契约的状态目录」从未被提出。

### 问题描述

`.forge/` 是 ForgeOS 的持久化核心，包含 4 种不同类型的状态：trace（审计轨迹）、memory（跨会话知识）、checkpoint（恢复快照）、scorecards（路由评分）。每种状态有不同的增长模式、不同的容错需求、不同的保留策略——但**没有任何统一的版本契约和生命周期管理**：

- **版本不兼容检测为零**——一个 sprint-30 构建的 `forge` 二进制读取 sprint-31 写入的 checkpoint，没有任何机制检测格式变化。`persist.Checkpoint` 没有 `_format` 字段
- **trace 只旋转一次**——`openTracer` 在启动时如果旧 trace > 10MB 就备份为 `.1`，然后从此永不旋转（`evolve.go:469-476`）。一个 100 迭代的 evolve 运行产生 ~500 条 event + gate 裁决 + converge 事件，日增量 1-5MB，数天后无上限增长
- **checkpoint 不保留历史**——`persist.Save` 的 `retain` 参数在 `evolve.go` 中硬编码为 0（`checkpointRetain` 从未出现在 flag 集合中）
- **memory 有 compact 但无 TTL**——`Compact` 按数量阈值 + 时间年龄分组压缩，但没有 TTL 机制删除真正过时的知识（比如 30 天前的 gap 条目）
- **scorecards 只增不减**——每次 `forge run/evolve` 写入一个 JSON 文件，永远不清理
- **没有 `forge cleanup` 命令**——用户无法主动管理 `.forge/` 目录大小

**这不只是「运维方便」问题**：在一个 24h+ 无人值守的 evolve 运行中，`.forge/` 的无限制增长是一个**可靠性风险**——磁盘满会导致整个循环异常终止。

### 代码级证据

**证据 A：checkpoint 无格式版本**

```go
// forge-core/internal/persist/checkpoint.go:25-45
type Checkpoint struct {
    Engine      orchestrator.Engine      `json:"engine"`
    Loop        LoopState                `json:"loop"`
    Signals     converge.Signals         `json:"signals"`
    // ★ 没有 _format 字段，没有 Version 字段
    // 对比：trace.go 有 _format:"forgeos.trace.v1"，memory.go 有 _format:"forgeos.memory.v1"
}
```
**不一致性**：trace 和 memory 都有 `_format` 版本标识，checkpoint 没有——但 checkpoint 是恢复路径的关键数据，版本不兼容的后果最严重（恢复静默失败）。

**证据 B：checkpointRetain 硬编码为 0**

```go
// forge-core/cmd/forge/evolve.go — checkpointRetain 从未出现
// persist.Save 的 retain 参数传 0（不保留历史）
// 对比：persist.Save 的 retain 机制已实现但未被消费
// forge-core/internal/persist/checkpoint.go:102
func Save(path string, cp Checkpoint, retain int) error {
    if retain > 0 {
        rotateRetain(path, retain) // 已实现但从未被调用时传 >0
    }
    // ...
}
```

**证据 C：trace 旋转只发生一次（启动时），之后永不处理**

```go
// forge-core/cmd/forge/evolve.go:469-476
func openTracer(root string) (*trace.Tracer, func(), error) {
    tp := filepath.Join(forgeDir(root), "trace.jsonl")
    // 如果文件 > 10MB，重命名为 trace.jsonl.1，开始新文件
    // 这是启动时执行一次，然后永不
    // 100 次迭代后 trace.jsonl 会重新增长到超过 10MB
    // 但永远不会再次旋转
}
```

**证据 D：四种状态四种策略，无统一视图**

```go
// .forge/ 目录结构
// .forge/
//   trace.jsonl      ← 仅启动时旋转一次
//   trace.jsonl.1    ← 旧备份（仅一份）
//   memory.jsonl     ← 每 10 迭代 compact，但无 TTL
//   checkpoint.json  ← 原子覆盖，不保留历史
//   scorecards/      ← 只增不减
//     <mode>/
//       <ts>.json    ← 每个 run 一个文件，永不删除
```
**没有统一的生命周期策略，没有配额，没有 `forge cleanup`。**

### 边界情况矩阵

| 场景 | 当前行为 | 问题 |
|---|---|---|
| forge-core 升级后 checkpoint 格式变化 | `forge resume` 静默加载已损坏的数据 | 恢复路径无声失败 |
| 连续 evolve 运行 7 天 | trace.jsonl + trace.jsonl.1 各~10MB + memory 数 MB + scorecards 累计 | 磁盘占用不可预测 |
| DevOps 清理磁盘 | 不知道 `.forge/` 中哪些可删、哪些不可删 | 运维不确定性 |
| CI 中每个构建产生一个 checkpoint | 30 天 CI 后 ~600 个 checkpoint | 存储膨胀 |
| 回滚 forge-core 到旧版本 | 已读取过新格式 checkpoint 的数据无法回退 | 版本降级路径缺失 |

### 建议方向

构建 **`.forge/` 状态目录统一生命周期管理**：

1. **格式版本契约**——为 `.forge/` 中每种持久化数据添加统一的版本标识：
   - `checkpoint.json` 加 `_format: "forgeos.checkpoint.v1"`（与 trace/memory 对齐）
   - 新 `internal/state/` 包，定义 `StateDir` 接口（`Version() string, Cleanup(policy) error, Migrate(fromVersion string) error`）
   - 二进制启动时自动检测版本兼容性：不能识别的新格式 → fail-closed（不是静默加载）

2. **统一生命周期策略**——新 `project.yml` 字段或 `.forge/policy.yml`：
   ```yaml
   state_management:
     trace:
       max_size_mb: 50
       rotation: size
       retention_days: 30
     memory:
       max_entries: 10000
       ttl_days: 90
     checkpoint:
       retain_count: 5
     scorecards:
       max_files: 500
       retention_days: 60
   ```

3. **`forge cleanup` 命令**——手动或自动（`forge evolve --cleanup`）触发的状态目录整理，按策略清理过期数据

4. **配额告警**——`forge doctor` 或 evolve 日志在 `.forge/` 超过配置阈值的 80% 时发出警告

5. **迁移路径**——`forge migrate --state` 将旧格式的 checkpoint/memory/trace 升级到新版本

### 与已有覆盖的关系

| 已有分析 | 覆盖内容 | 与本文的区别 |
|---|---|---|
| `five-product-operational-gaps.md` 方向三「checkpoint 版本兼容性」 | checkpoint 版本化 + 备份建议 | 本文将其扩展为**整个 `.forge/` 目录的统一生命周期框架**，不限于 checkpoint |
| `expansion-production-perspectives.md` 方向一「持久化数据安全」 | trace/memory/checkpoint 的数据安全 | 本文聚焦于**版本契约 + TTL + 大小配额 + 统一清理**，而非数据加密/权限 |
| `execution-semantic-gaps.md` 方向二「确定性回放」 | trace 数据用于重放 | 本文关注的是**存储管理**（版本、大小、保留），而非重放语义 |
| `structural-gaps-v41.md` | 存储无界增长问题 | 从结构债务角度提问题，本文提供完整的生命周期管理方案 |
| `expansion-five-uncovered-2026-07-10.md` 方向一「确定性回放」 | Hermetic replay 机制 | 回放需要数据完整性和可追溯性，本文提供其前提条件（版本契约 + 保留策略） |
| **核心差异** | — | 本文不是「加一个 cleanup 脚本」，而是**将 `.forge/` 从「无管理的二进制产物目录」升级为「有版本、有策略、可维护的状态有向图」**——这是生产化部署的前提条件 |

---

## 方向五 · 治理孤儿收编：pi-batch.py 与 Examples 的治理集成

**优先级**: 🟢 **P2** | **类别**: 一致性 · 完整性 | **预估**: 1 sprint | **杠杆**: ⭐⭐⭐  
**已有分析覆盖**: **零作为独立方向**——pi-batch.py 在结构债务表中被列为「根目录文件」之一，但从未被作为「功能完整的资产需要收编」提出。

### 问题描述

ForgeOS 有两个「治理孤儿」——功能完备但被排除在治理体系之外的资产：

#### 孤儿 A：`pi-batch.py`

`pi-batch.py` 是一个 470 行 / 17KB 的独立 Python 批处理执行器，位于根目录，有完整的 CLI、YAML 任务文件解析、串行/并行执行模式、超时机制——但：

- **完全在治理体系之外**：`gate.mjs` 的行数检查覆盖它但 `arch-check.mjs`（限于 Go 源文件）不检查，`check.py`（限于 `.agent/`）不检查，`acceptance.mjs` 的 app-test 不检查
- **零测试覆盖**：`harness/` 中没有 `test_pi-batch.py`，`forge-core/` 中没有相关测试
- **已知 bug 未修复**：Sprint 27 诚实记录的超时延迟偏差（~2×配置值）和 `FileNotFoundError` 误报「pi not in PATH」问题
- **违反根目录文件数红线**：`project.yml` 规定 `max_root_files:15`，不包括该文件（BOOTSTRAP.md 宣誓的纪律自己没遵守）
- **无 forge-core 依赖也不被 forge-core 依赖**——唯一的集成点是 `engine_build.go` 中搜索 `pi` 作为可能的 executor

#### 孤儿 B：`examples/`

`examples/url-shortener` 和 `examples/go-taskd` 是完整的应用程序——但：
- **没有自己的 `.agent/` 目录**——无法展示 `forge-init` 产生的完整治理脚手架
- **没有独立的 CI**——没有 `.github/workflows/forge.yml`，无法演示治理循环
- **没有独立的 `harness/`**——被治理但不展示治理
- **`go-taskd` 没有测试文件在其目录内**——`forge accept` 的 app-test 如何验证它？

### 代码级证据

**证据 A：pi-batch.py 被根文件数规则豁免**

```yaml
# .agent/project.yml:14
overrides:
  max_root_files: 15
# pi-batch.py 是根目录的第 16 个文件（不计 .gitignore 中的 .pyc 和 __pycache__）
# 它在 gate.mjs 的行数检查范围内，但不被 max_root_files 门控
```

**证据 B：pi-batch.py 的已知 bug 从未被跟踪修复**

```python
# pi-batch.py:180-200 附近
# FileNotFoundError 分支无法区分「二进制在 PATH 中缺失」和「cwd 不存在」
# 超时机制对混合 stdout+stderr 的输出流实际延迟 ~2× 配置值
# 这些在 Sprint 27 诚实记录但从未进入 issue 跟踪或功能需求清单
```

**证据 C：examples/ 没有自己的治理层**

```bash
ls -la examples/url-shortener/.agent/  # 不存在
ls -la examples/go-taskd/.agent/       # 不存在
ls -la examples/go-taskd/test/         # 存在但无独立 CI
# 对比：forge-init 创建的新项目有完整的 .agent/ + harness/ + CI
# 表明 examples 是 pre-init 的展示品，不是 post-init 的示范
```

**证据 D：没有集成测试验证 forge-init 创建的示例与 examples/ 等价**

```bash
# 没有任何测试断言：
# forge init my-project && tree my-project/.agent/ ≈ tree examples/url-shortener/.agent/
# 因为后者根本没有 .agent/
```

### 边界情况矩阵

| 场景 | 当前行为 | 问题 |
|---|---|---|
| 用户检查根目录文件数 | pi-batch.py 不被计数，违反纪律 | 治理盲区 |
| 修改 pi-batch.py 后提交 | 无 gate 检查该文件 | 破损无声 |
| 用户看 examples/ 作为上手模板 | 没有 .agent/，无法直接感受治理 | 展示价值下降 |
| `go-taskd` 被修改 | 无独立测试捕捉回归 | 依赖外部不注意 |
| `forge --help` 输出 CLI 命令 | pi-batch.py 功能与 forge 功能重叠 | 用户困惑 |

### 建议方向

1. **pi-batch.py 收编**：
   - 整合到 `forge-core/cmd/forge` 作为 `forge batch` 子命令（复用现有的 flag 和 executor 基础设施）
   - 或迁移至 `harness/pi-batch.shim.py`（作为 harness 工具而非根目录独立脚本）
   - 补测试：`harness/test_batch.py`（镜像 `test_sca.mjs` / `test_secret-scan.mjs` 模式）
   - 修复已知 bug（超时偏差 + FileNotFoundError 区分）

2. **Examples 治理化**：
   - 为 `examples/go-taskd` 生成 `forge-init` 产物（.agent/ + harness/ + CI）
   - 为 `examples/url-shortener` 补充独立的 `forge accept` 演示
   - 添加集成测试：`forge init` 生成的脚手架 ≈ examples 的治理结构

3. **增加 `forge accept` 对根目录的交叉检查**——验证 `forge-core/` 以外的资产（根目录文件、`examples/`、`pi-batch.py`）也被纳入治理范围

### 与已有覆盖的关系

| 已有分析 | 覆盖内容 | 与本文的区别 |
|---|---|---|
| `four-truly-unexplored-architectural-gaps.md` | 在结构债务表中将 pi-batch.py 列为根目录文件之一 | 本文聚焦于**功能完整的资产需要收编**（测试、修复 bug、治理集成），不是仅列为债务项 |
| `structural-gaps-v41.md` | cmd/forge 包内聚性债务 | 提到结构孤儿概念但不特指 pi-batch.py |
| `five-systemic-oversights-v45.md` | 结构缺口表中出现 pi-batch | 作为已识别项列出但不提供收编方案 |
| `forgotten-five-foundations.md` | 「被遗忘的基础设施」概念 | 从架构基础缺失角度讨论，本文从产品一致性角度 |
| **核心差异** | — | 本文不是「承认有孤儿存在」，而是**为每个孤儿提供具体的收编路径和验证标准**——使 ForgeOS 的治理范围与代码库实际范围一致 |

---

## 总结：五方向全景

| # | 方向 | 优先级 | 类型 | 杠杆 | 预估 | 核心价值主张 |
|---|---|---|---|---|---|---|
| 1 | 可插拔 Agent 适配器契约 | 🔴 P0 | 架构·可扩展 | ⭐⭐⭐⭐⭐ | 2 sprints | 使多 CLI 愿景从散文变为可执行的架构决策 |
| 2 | ADR 可执行治理闭环 | 🟠 P1 | 治理·完整性 | ⭐⭐⭐⭐ | 2 sprints | 治理 OS 治理自己的架构决策 |
| 3 | 自我测试隔离与自举完整性 | 🟠 P1 | 测试·可靠性 | ⭐⭐⭐⭐ | 1 sprint | 使治理层不因自举循环而自锁 |
| 4 | `.forge/` 版本契约与生命周期 | 🔴 P0 | 运营·数据完整性 | ⭐⭐⭐⭐⭐ | 1–2 sprints | 使 24h+ 无人值守可预测、可运维 |
| 5 | 治理孤儿收编 | 🟢 P2 | 一致性·完整性 | ⭐⭐⭐ | 1 sprint | 使治理边界与代码库实际一致 |

**五个方向的共同主题**：ForgeOS 已经是一个高度复杂的系统，其核心引擎（编排、路由、收敛、记忆）经过 31 轮 sprint 已非常健壮。但系统的**外围**——多 CLI 适配、ADR 治理、自举测试、状态目录管理、根目录资产——尚未达到与核心同等的工程成熟度。这 5 个方向代表了从「功能性原型」到「生产级平台」必须克服的最后几个结构性障碍。
