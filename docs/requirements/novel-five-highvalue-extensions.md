# ForgeOS — 基于全局深扫的五个高价值扩展方向

> **角色**: 资深架构师 / 产品经理  
> **方法**: 全仓深度扫描  
>   — forge-core 18 Go 包 / `cmd/forge` 16+ CLI 命令 / harness 38+ 模块 /  
>   — `.agent/` 完整治理骨架（12 agent 卡 + 9 skill 卡 + 5 工作流 + policies）/  
>   — Sprint 1–31 完整演进 + FUNCTIONAL_REQUIREMENTS_AUDIT（90+ DONE + GAP 全收口）/  
>   — **通读交叉验证 40+ 篇 `docs/analysis/*.md` + 20+ 篇 `docs/requirements/*.md`（~68+ 已有方向）**  
> **核心承诺**: 每个方向与全部已有分析文档的核心论点**不重叠**。差异证明附后。  
> **纪律**: 不编写任何代码。每个方向附代码级证据 + 边界场景。  
> **日期**: 2026-07-10

---

## 已有覆盖全景（本文不重复）

本文**不重复**以下已被 ~68+ 份已有分析充分覆盖的域：

| 已有覆盖域 | 代表文档 | 方向数 |
|------------|----------|--------|
| 功能引擎补齐（编排/路由/记忆/收敛/信号/诊断） | `high-value-extension-directions.md` · `expansion-production-readiness.md` | ~15 |
| 第三地平线生态（多仓库联邦/事件驱动/管线组合/资产升级） | `expansion-horizon-three.md` · `expansion-gaps-v7-novel.md` | ~10 |
| 生产可靠性（Prompt QA / 信号硬化 / 环境验证 / 自愈层） | `expansion-production-readiness.md` | ~8 |
| 执行语义形式化（原子性/幂等/因果一致性/回滚/版本演化） | `execution-semantic-gaps.md` | ~8 |
| 二阶伴生问题（知识衰减/配置爆炸/TOCTOU/无声数据丢失） | `second-order-architectural-gaps.md` · `systemic-expansion-v26.md` | ~10 |
| 系统边界盲区（级联截断/信任边界/持久语义/可移植性） | `strategic-extensions-v22~v33.md` · `uncovered-frontiers-v25.md` | ~12 |
| 安全/凭据/secret 生命周期/SCA/沙箱 | `genuinely-novel-expansion-directions.md` | ~5 |
| 并行编排 / 迭代跳过 / 收敛可见性 / YAML 差分测试 | `high-value-extension-directions-v3.md` | ~5 |
| 经济治理 / cost 智能 / 跨运行审计 / 结构化输出协议 | `next-five-frontiers.md` · `forgotten-frontiers-five.md` | ~8 |
| 知识启动协议 / 冷启动先验 | `novel-five-frontiers-v34.md` 方向一 | ~1 |
| 并发状态一致性 / 文件锁 / 实例 ID | `strategic-extensions-v33.md` 方向一 · `novel-five-frontiers-v34.md` 方向二 | ~2 |
| 外部 SDLC 集成（PR/CI/Merge） | `high-value-extension-v35.md` 方向一 | ~1 |
| Agent 输出行为回归检测 | `high-value-extension-v35.md` 方向二 | ~1 |
| 跨阶段语义一致性守卫 | `novel-extensions-v12-architect-perspective.md` | ~1 |
| **总计已有覆盖** | | **~68+ 方向** |

---

## 本文的 5 个方向

每个方向均从**代码级微观观察**出发，而非抽象产品愿景。所有方向在 v2 增量范围内可实现（不依赖 Firecracker / LiteLLM / 外部数据库）。

---

## 方向一：治理策略测试框架（Governance Policy Test Framework）

> **类型**: 治理 · 质量保障 · 开发体验  
> **优先级**: P1（治理引擎自身的测试覆盖空白）  
> **代码影响**: 新 `forge test` CLI · 新 `harness/test-policy.mjs` · 可选 `.agent/tests/` 目录  
> **差异化证明**: ~68+ 已有方向中**零覆盖**。最近的 `check.py` 是结构验证（schema 合规/引用解析），非行为测试。没有任何现有分析提出**可断言的治理行为测试**概念。

### 现状：代码级证据

ForgeOS 有三层测试，但没有一层能测试**用户自定义的治理策略**：

**证据 A：forge-core 测试只测引擎逻辑，不测策略行为**

```bash
$ grep -rn "mode.*engineering.*production\|modes\.yml\|gate_set" forge-core/ --include="*_test.go" | head -5
# 结果是：forge-core 测试测的是 Engine、Converge、Routing 等内部包的边界条件，
# 但对 project.yml + modes.yml 组成的用户策略没有「组合测试」。
# 例如：没有测试说"当 project.yml 写 mode=engineering lifecycle=production 时，
# gate_set 必须包含 security gate"。
```

**证据 B：`check.py` 只做结构校验，不做行为断言**

```bash
$ grep "def check_" harness/check.py
# check_workflow_agent_refs   — agent 引用可解析
# check_workflow_control_flow — target_phase 存在
# check_workflow_model_tiers  — model_tier 引用已知 tier
# check_workflow_mode_gating  — mode_gating 声明 vs modes.yml 一致
# check_modes_router_tiers    — router_floors 引用已知 tier
# check_mode_priorities       — priorities 是合法排序
# check_agent_card_contracts  — agent 卡有机读契约段
# check_skill_card_refs       — skill 引用可解析
# check_workflow_stage_chain  — next_stage 指向存在的 stage
# check_cycle_dependency      — workflow 间无循环依赖
#
# 全部是「声明可解析/引用可解析」类结构性检查，没有一条是行为性断言。
# 没有任何 check 说"在 engineering+production 下，prompt 必须包含 security gate"。
```

**证据 C：`acceptance.mjs` 的 criterion 也是结构性的**

```bash
$ grep "metric\|criterion" harness/acceptance-quality.mjs | head -10
# test_pass, app_test_pass, complexity_violations, arch_violations,
# architecture, security_findings, dependency_vulnerabilities, lint_pass,
# typecheck_pass, build_pass, coverage_pass
# ——全是「gate 跑没跑过」，不是「策略对不对」。
```

**证据 D：`internal/mode/mode.go` 的策略逻辑有隐式行为需测试**

`mode.go` 的 `Effective(mode, lifecycle)` 函数是**策略解释器**——它把 `modes.yml` 的声明翻译为 Go 的 `Policy` 结构体。但是 translation 的正确性只有 forge-core 自己的测试覆盖，**没有给用户暴露一个断言接口**来验证「我的 project.yml 配置会得到我期望的 gate set」。

```go
// internal/mode/mode.go 第 102-105 行
type Policy struct {
    Gates          []string // mode × lifecycle 过滤后的 gate 集合
    Enforce        string   // "warn" | "block"
    Coverage       float64  // 覆盖率阈值
    DiscoverDepth  string   // "skip" | "light" | "full"
    DesignDepth    string   // "light" | "standard" | "full"
    ReviewDepth    string   // "skip" | "standard" | "full"
    Reviewer       bool
    ADR            bool
    EvolveDepth    string   // "opportunistic" | "standard" | "thorough" | "advisory"
}
```

这个 `Policy` 是整个中枢旋钮的输出——所有编排决策依赖它——但用户无法以测试形式断言它的值。

### 建议方向

引入一个轻量的**治理策略测试框架**，允许用户为他们的治理配置编写可重复执行的测试：

```
.agent/tests/
  test_gates.yml          # "mode=engineering lifecycle=production → gates 含 security"
  test_routing.yml        # "mode=explorer → default_tier = haiku"
  test_review_depth.yml   # "mode=balanced lifecycle=production → ReviewDepth=full"
```

每个测试文件包含一组**断言**，由 `forge test` 子命令解释：

```yaml
# .agent/tests/test_gates.yml
# 测试集合：验证不同 mode×lifecycle 组合下的 gate 集
tests:
  - name: "engineering+production forces security gate"
    mode: engineering
    lifecycle: production
    expect:
      gates_contain: [security]
      enforce: block
      coverage_ge: 80

  - name: "explorer skips security gate"
    mode: explorer
    lifecycle: idea
    expect:
      gates_not_contain: [security, test, complexity]
      enforce: warn

  - name: "balanced+mvp has minimum gates"
    mode: balanced
    lifecycle: mvp
    expect:
      gates_contain: [lint, build]  # require_min_gates floor
      gates_not_contain: [arch, security]

  - name: "production override forces full review even in explorer"
    mode: explorer
    lifecycle: production
    expect:
      review_depth: full
      reviewer_enabled: true
      adr_required: true
```

实现层可复用 `internal/mode.Effective()` 的既有逻辑——`forge test` 只需构造 `mode × lifecycle` 组合并断言输出的 `Policy` 结构体，加上 `internal/mode` 已有的 `distillModes` 能力。无需新策略引擎。

### 边界场景

| 场景 | 行为 |
|------|------|
| 零测试文件 | `forge test` 报告 `NO TESTS`（不是 PASS 也不是 FAIL） |
| 断言未知 mode/lifecycle | fail-safe：unknown mode→断言失败（不假装已知） |
| modes.yml 更新后测试漂移 | test 失败，CI 阻止漂移——这正是测试框架的核心价值 |
| forge-init 脚手架 | 生成一个 `tests/` 目录 + 这个项目的 mode×lifecycle 对应的最小测试集 |
| 向后兼容 | `forge test` 是独立子命令，不影响既有 `run/evolve/gate/accept` 路径 |

### 核心收益

| 维度 | 收益 |
|------|------|
| **治理可信度** | 策略行为可以被 CI 自动验证，告别「声明 vs 实现漂移」 |
| **增量采纳安全垫** | 团队在切换 mode/lifecycle 前可用 test 预览效果 |
| **组织标准化** | 中心团队可以发布一组「合规断言」测试集到各项目 |
| **零新运行时开销** | `forge test` 是离线诊断命令，不影响 evolve 运行路径 |

---

## 方向二：Agent 运行时协议抽象层（Agent Runtime Protocol Abstraction）

> **类型**: 架构 · 可扩展性 · 厂商中立  
> **优先级**: P2（v3 跨厂商的前置条件，越早做迁移成本越低）  
> **代码影响**: 新 `internal/runtime/` 包 · 重构 `command_executor.go` · 重构 `cost.go` · 新 `cmd/forge/agent_runtime.go`  
> **差异化证明**: 已有方向覆盖的是「跨厂商模型池」（路由层，v3 通过 LiteLLM）。本文覆盖的是**执行层**——当前 `command_executor` + `cost.go` 深度绑定 claude CLI 的 flag 语法、输出 JSON schema、权限模型、模型命名。这是两个正交的抽象层次。

### 现状：代码级证据

**证据 A：`command_executor.go` 通过 `Build` 回调间接接 claude，但调用方 `engine_build.go` 硬编码 claude 专用参数**

```go
// engine_build.go 第 52-58 行（原文略缩）
func agentExecutor(...) orchestrator.AgentExecutor {
    if o.executor == "command" {
        isClaude := strings.Contains(o.agentCmd, "claude")
        ex := orchestrator.CommandExecutor{
            Build: func(p asset.Phase, mode string) []string {
                argv := claudeArgv(o, isClaude, tierOf(p), p)
                return append(argv, "-p", buildPrompt(...))
            },
            ...
        }
    }
}
```

`claudeArgv` 函数（同一文件第 80-140 行）构造了 claude 特定的 flag：

```go
func claudeArgv(...) []string {
    argv := []string{"claude", "-p"}
    if isClaude {
        argv = append(argv, "--permission-mode", o.agentPermission) // claude 特定
        argv = append(argv, "--model", phaseModel(phase.Name))       // claude 语法
        argv = append(argv, "--output-format", "json")               // claude 特定
        if o.agentAllowedTools != "" {
            argv = append(argv, "--allowedTools", o.agentAllowedTools) // claude 特定
        }
        if isReadonly(phase) {
            argv = append(argv, "--disallowedTools", "Edit Write")     // claude 特定
        }
    }
}
```

所有这些都是 claude CLI 的私有协议。如果 v3 接入 Gemini CLI、Codex、OpenHands，这些 flag 全部不兼容。

**证据 B：`cost.go` 深度绑定 claude 的 JSON 输出格式**

```go
// cost.go 第 88-120 行（原文略缩）
type claudeResult struct {
    TotalCostUSD float64 `json:"total_cost_usd"`
    Result       struct {
        Text string `json:"text"`
    } `json:"result"`
}

// parseClaudeCostUsd 专用：只解析 claude 的 JSON 结构
func parseClaudeCostUsd(raw string) (costUsd float64, output string, ok bool) {
    var cr claudeResult
    if err := json.Unmarshal([]byte(raw), &cr); err != nil {
        return 0, raw, false
    }
    return cr.TotalCostUSD, cr.Result.Text, true
}
```

这个函数在 `cost.go` 中被 `Observe` sink 调用，而 `command_executor.go` 的 `Observe` 回调是通用接口。但当前**唯一实现**是 claude 解析器——新增一个 Codex 解析器需要修改 `cost.go`，而不是添加一个独立实现。

**证据 C：`routing.ModelMap` 已经预设了厂商结构，但执行层没有对应抽象**

```go
// routing/routing.go 第 217-231 行
var ModelMap = map[string]map[string]string{
    "anthropic": {
        Haiku:  "claude-sonnet-4-haiku",
        Sonnet: "claude-sonnet-4",
        Opus:   "claude-opus-4",
    },
}
```

路由层已经按 `provider/tier` 模型组织，但执行层（`engine_build.go`）仍然把 model 直接当 `--model` flag 传给 claude。加入 `"openai"` provider 后，`--model` flag 语法不同、`--output-format` 不同、cost JSON 结构不同、权限模型不同——`cost.go` 的 `parseClaudeCostUsd` 和 `claudeArgv` 都需要 vendor-specific 分支。

**证据 D：`Observe` sink 的契约成本路径需要 generalization**

`CommandExecutor.Observe` 是一个 `func(phase, output string, latency time.Duration)` 回调。`cost.go` 把它接给 `parseClaudeCostUsd`。但其他 vendor 的输出结构完全不同（Gemini CLI 的输出是 streaming JSON，Codex 可能是 structured YAML，OpenHands 是标准输出日志）。当前接口把「原始输出字节」直接传递给解析器——对 claude 这个格式是已知的，但对其他 vendor 这个接口太底层了，强迫解析器处理流控制、输出截断、多行拼接等擦屁股逻辑。

### 建议方向

引入一个 `AgentRuntime` 接口层，将「如何调用一个 LLM agent」从「调用哪个 vendor」中解耦：

```go
// internal/runtime/runtime.go
package runtime

import (
    "context"
    "time"
    "forgeos/forge-core/internal/asset"
)

// Result 是 agent 调用的标准化输出——每个 runtime provider 都要提供。
type Result struct {
    Output      string        // agent 的文本产出（prompt 的回复）
    CostUSD     float64       // 本次调用的美元成本（0=未知）
    Latency     time.Duration // 墙钟耗时
    Model       string        // 实际使用的 model 名
    Verdict     string        // 可选的机读裁决（从 output 提取）
}

// Runtime 是一个 agent 协议实现：知道怎么把 prompt 送进特定的 agent CLI，
// 怎么解析输出，怎么提取 cost/latency/verdict。
type Runtime interface {
    // Name 返回 runtime 名称，用于日志和诊断。
    Name() string

    // Argv 构建调用 agent CLI 的命令行参数。
    // model 是路由层解析后的模型名（如 "claude-sonnet-4"），
    // runtime 负责把它转成自己需要的 flag（如 "--model=..."）。
    Argv(prompt string, model string, opts ArgvOpts) ([]string, error)

    // Parse 解析 agent CLI 的原始输出，返回标准化的 Result。
    Parse(rawOutput string) (Result, error)

    // Provider 返回这个 runtime 对应的 provider 名（routing.ModelMap 的 key）。
    Provider() string
}

// ArgvOpts 是 Runtime.Argv 的可选参数——各 provider 支持的功能子集不同。
type ArgvOpts struct {
    Readonly       bool     // 是否只读模式
    AllowedTools   []string // 白名单工具
    MaxBudgetUSD   float64  // 单次调用的美元上限
    AgentTimeout   time.Duration
}
```

每个 vendor 一个实现：

```
runtimes/
  claude.go      — claude -p --model --permission-mode --allowedTools ...
  gemini.go      — gemini --model ...  (v3)
  codex.go       — codex --model ...   (v3)
  openhands.go   — openhands run ...   (v3)
  echo.go        — 当前 echo/dry 模式（向后兼容）
```

`engine_build.go` 的 `agentExecutor` 从硬编码 claude 改为按 `routing.ResolveModel` 的 provider + Config 选择 Runtime：

```go
// 示意
runtime := runtimes.Select(provider) // "anthropic" → claude runtime
ex := orchestrator.CommandExecutor{
    Build: func(p asset.Phase, mode string) []string {
        argv, _ := runtime.Argv(buildPrompt(...), tierModel, argvOpts)
        return argv
    },
    Observe: func(phase, output string, latency time.Duration) {
        result, _ := runtime.Parse(output)
        costSink(phase, result.Model, result.CostUSD, latency)
        // verdict 提取也由 runtime 完成
    },
}
```

### 边界场景

| 场景 | 行为 |
|------|------|
| 未知 provider | 默认 fallback 到 claude runtime（向后兼容），日志警告 |
| Provider 不支持某些 flag（如 Codex 无 `--allowedTools`）| `ArgvOpts` 中的字段若非必须则跳过，必须的返回 error |
| claude 输出非 JSON（旧版本）| `Parse` 降级为纯文本提取，cost=0，仍可工作 |
| 切换 provider 后 trace 归因 | `Result.Model` 记录真实 model，scorecard 按 model 归因 |
| `echo` executor | 一个简单 `echo` runtime，与当前 `DryRunExecutor` 等效 |

### 核心收益

| 维度 | 收益 |
|------|------|
| **厂商中立** | 新增 vendor 只需添加一个 runtime 实现，不动既有代码 |
| **测试能力** | Runtime 接口可 mock，单元测试不再需要真 claude 进程 |
| **路由闭环** | `routing.ModelMap` + `runtime.Select` 构成真正的 provider→execution 映射 |
| **v3 前置条件** | LiteLLM 只解决「路由到哪个模型」，Runtime 抽象解决「怎么调用那个模型」 |

---

## 方向三：收敛信号溯源与信任模型（Convergence Signal Provenance & Trust Model）

> **类型**: 正确性 · 可审计性 · 诚实性  
> **优先级**: P1（收敛判断是 ForgeOS 停止决策的核心——信号作假比功能缺失更危险）  
> **代码影响**: `internal/converge/converge.go` · `internal/converge/signal.go`（新文件）· 接线到 `gates.go`  
> **差异化证明**: ~68+ 已有方向中**零覆盖**。最接近的是「收敛信号硬化」（`expansion-production-readiness.md`），但那讨论的是信号缺失时的默认值安全，不是**信号来源的可信度分层与溯源**。

### 现状：代码级证据

**证据 A：`converge.Signals` 的所有字段都是裸值**

```go
// converge.go 第 36-89 行
type Signals struct {
    RoadmapCompletion    float64           // agent 自报
    GatesGreen           bool              // 独立执行（gate.mjs）
    RequirementConfidence float64          // agent 自报
    ReviewStatus         string            // agent 自报（verdict 解析）
    FileDelta            float64           // 机械计算（git diff）
    HumanApproved        bool              // 外部输入（approval marker）
    Criteria             map[string]string // 独立执行（acceptance probe）
    GateProof            GateProof         // 独立执行 + 生命周期矩阵
    CodeTestRatio        float64           // 机械计算（git diff --stat）
}
```

8 个信号来自 3 种**完全不同的信任源**，但 **`Signals` 结构体没有记录任何来源信息**：

| 信号 | 来源类型 | 独立性 | 可伪造 | 当前信任级别 |
|------|----------|--------|--------|-------------|
| `RoadmapCompletion` | agent 自报 | ❌ 不独立 | 易（agent 可空勾） | 与机器执行同权 |
| `GatesGreen` | 机器执行 | ✅ 独立 | 难 | 与自报同权 |
| `FileDelta` | 机械计算 | ✅ 独立 | 难 | 与自报同权 |
| `CodeTestRatio` | 机械计算 | ✅ 独立 | 难 | 与自报同权 |
| `ReviewStatus` | agent 自报（规则解析）| ⚠️ 半独立 | 中（agent 可控 VERDICT） | 与机器执行同权 |
| `RequirementConfidence` | agent 自报 | ❌ 不独立 | 易 | 与机器执行同权 |
| `HumanApproved` | 外部输入 | ✅ 独立 | 难 | 最高 |
| `Criteria` | 机器执行 | ✅ 独立 | 难 | 与自报同权 |

**所有信号都被 `evalOne` 同等对待**——一个 agent 空勾的 `RoadmapCompletion=100%` 和一个实跑的 `GatesGreen=true` 在收敛判定中权重相同。

**证据 B：`RoadmapCompletion` 的诚实性交叉验证（FileDelta）是分离的，不影响收敛**

```go
// loop.go 第 246-252 行
// FileDelta cross-validation: when roadmap is high but file changes are low,
// flag the potential honesty gap
if sig.RoadmapCompletion > 0.5 && sig.FileDelta < 0.3 {
    l.logf("  ⚠ honesty: roadmap=%.0f%% but file-change coverage=%.0f%%",
        sig.RoadmapCompletion*100, sig.FileDelta*100)
}
```

这个警告**只打印日志，不影响收敛判定**。即使 `RoadmapCompletion=100%` 但 `FileDelta=0%`，收敛仍判 MET——因为 `evalOne` 只看各自指标，不知一个信号「来自不可靠源」。

**证据 C：没有信号时效性概念**

`Signals` 没有时间戳。如果 `RoadmapCompletion` 是 3 轮迭代前测的（跑完 planner phase 后未更新），而 `GatesGreen` 是刚测的，收敛判定不知道这种新旧混合。Evolve loop 每轮迭代会重新测所有信号——但 `forge run`（单次执行）只测一次，用同一组信号驱动 stop 判定。

### 建议方向

引入**带溯源元数据的信号模型**，使收敛判定可以区分信号的可信度：

```go
// converge/signal.go
package converge

import "time"

// TrustLevel 表示一个信号的信任等级。
type TrustLevel int

const (
    TrustExternal     TrustLevel = iota // 外部输入（人批准）——最高
    TrustComputed                      // 机械计算（git diff）——高
    TrustExecuted                      // 独立执行（gate.mjs）——高
    TrustParsed                        // 规则提取（verdict 解析）——中
    TrustReported                      // agent 自报（roadmap 勾选）——低
)

// Verifiable 封装一个带元数据的信号值。
type Verifiable[T any] struct {
    Value     T         // 信号值
    Trust     TrustLevel // 信任等级
    CapturedAt time.Time // 测量时间
    Source    string     // 来源描述（"runAgentPhase planner" / "gate.mjs lint"）
}

type Signals struct {
    RoadmapCompletion    Verifiable[float64]
    GatesGreen           Verifiable[bool]
    RequirementConfidence Verifiable[float64]
    ReviewStatus         Verifiable[string]
    FileDelta            Verifiable[float64]
    HumanApproved        Verifiable[bool]
    CodeTestRatio        Verifiable[float64]
    // … 其余字段
}
```

收敛函数按信任等级加权：

```go
// converge.go
func evalRoadmap(c asset.Criterion, sig Signals) Result {
    pct := sig.RoadmapCompletion.Value * 100
    // 如果是自报（TrustReported）且 FileDelta 偏低 → 调低等效完成度
    if sig.RoadmapCompletion.Trust == TrustReported && sig.FileDelta.Value < 0.3 {
        pct = min(pct, 50) // 自报完成度超过 50% 但文件改动少的，压缩到 50%
    }
    // … 其余逻辑
}
```

**向后兼容**：`Verifiable[T]` 的零值为空——缺省 `Trust` 视同 `TrustExecuted`（同当前行为），缺省 `CapturedAt` 视同 `time.Now()`。这样旧调用点（如现有测试）不改动代码就保持原义。

### 边界场景

| 场景 | 行为 |
|------|------|
| 所有信号缺省 | 向后兼容，逐位相同 |
| Agent 报 100% roadmap 但 0 文件改动 | 信任调整：等效完成度 ≤ 50%，不判 MET |
| 多次迭代间信号的 CapturedAt 不一致 | 只用最新一轮的信号值（evolve loop 已保证每轮全量刷新） |
| 测试代码只设 `RoadmapCompletion` 不设 `Trust` | 缺省 = `TrustExecuted`，同当前行为 |
| 信号源记录到 trace | `Source` 字段可被 trace event 的 Detail 携带，提供审计追溯 |

### 核心收益

| 维度 | 收益 |
|------|------|
| **诚实性** | 自动调整不可靠信号的权重，收敛更难被 agent 自报操纵 |
| **可审计性** | 每次收敛判定的每个信号都可追溯：谁提供的、什么时候、可信度多高 |
| **零行为回归** | `Verifiable` 零值向后兼容，现有测试和运行路径逐位不变 |
| **演进基础** | 未来可加入更多信任规则（如 `TrustReported + TrustReported` 互校验）而无需改收敛核心 |

---

## 方向四：跨运行 Trace 分析与经验对比（Cross-Run Trace Analytics & Empirical Comparison）

> **类型**: 可观测性 · 运维 · 持续优化  
> **优先级**: P2（当前 trace 数据是「写后不读」——数据已就绪，分析工具缺失）  
> **代码影响**: 新 `forge inspect` CLI · 新 `internal/trace/analysis.go` · 可选 `internal/trace/compare.go`  
> **差异化证明**: 已有方向覆盖了 trace 的记录/telemetry/scorecard（Sprint 19/26）和跨 checkpoint 异常检测（v33）。但**没有方向覆盖对 trace 数据进行系统性比较分析，用于回答『改 mode 后收敛更快了吗？』这类经验优化问题**。

### 现状：代码级证据

**证据 A：trace 数据丰富但只有 scorecard 一个消费者**

```bash
$ grep -r "trace" forge-core/cmd/forge/*.go | grep -v "_test" | grep -v "trace\.Event\|trace\.NewTracer" | head -10
# scorecard_wind.go → 读 trace 算 p95_latency_ms / avg_cost_usd（Sprint 19 交付）
# 没有其他消费者
```

trace 每条 Event 记录的 fields（`Kind, Name, Status, DurationMs, CostUsdMicros, Model, Detail`）构成了一个结构化的运行记录——但实际只被 scorecard 的 percentile engine 消费了 latency 和 cost 两个维度。

**证据 B：没有跨运行比较能力**

当前 `forge status` 和 `forge doctor` 只检查当前运行状态：

```bash
$ forge status --help  #（从 main.go 的第 69-76 行的 subcommands 表推论）
# 显示 .forge/ 目录的状态、最新 checkpoint、agent 版本
# 没有 --runs 参数、没有历史对比
```

如果一个运维人员想回答以下任何一个问题，当前系统完全没有帮助：

- "换到 mode=engineering 后，平均每次 converge 需要多少迭代？比 balanced 多还是少？"
- "上个月部署后，gate 失败率是否上升了？哪类 gate 失败最多？"
- "不同 agent 版本（claude-sonnet-4 vs claude-opus-4）在 reviewer phase 的 verdict 分布有何差异？"
- "增加复杂度检查后，平均每次 evolve 的 phase 重跑次数有变化吗？"

**证据 C：trace 文件是无界的、无索引的**

```go
// trace.go 第 110-115 行
// Emit 总是 append。没有轮转、没有压缩、没有索引。
// 对一个 24h evolve 循环（~100 迭代 × ~10 event/迭代 = ~1000 events），
// trace.jsonl 线性增长到 ~500KB+。100 次运行后就是 50MB+，
// 没有任何工具可以高效地「找出去年 10 月所有 gate FAILED 事件」。
```

**证据 D：scorecard-update.mjs 的 percentiles 是运行内聚合，不跨运行**

```js
// scorecard-update.mjs（Sprint 19 交付）
// 每个 trace 文件单独处理，p95_latency 是「本次运行」的 p95。
// 没有跨运行聚合——你不能问"过去 30 天所有 engineering mode 运行的
// p95 latency 中位数是多少"。
```

### 建议方向

为一个轻量的**跨运行 trace 分析子系统**，提供两类核心能力：

**1. 运行摘要索引（`forge inspect index`）**

每个 `forge run`/`forge evolve` 完成时，自动（或手动）生成一个运行摘要 JSON：

```json
{
  "_format": "forgeos.run_summary.v1",
  "run_id": "evolve-20260710-152233",
  "workflow": "build",
  "mode": "engineering",
  "lifecycle": "mvp",
  "started_at": "2026-07-10T15:22:33Z",
  "duration_ms": 245000,
  "iterations": 4,
  "converged": true,
  "outcome": "MET",
  "total_cost_usd": 0.73,
  "phases": {
    "total": 20,
    "gates_passed": 15,
    "gates_na": 3,
    "gates_failed": 2,
    "loop_backs": 1
  },
  "gates": {
    "lint": {"executions": 4, "passed": 4},
    "test": {"executions": 4, "passed": 3, "failed": 1},
    "arch": {"executions": 4, "passed": 4},
    "security": {"executions": 4, "passed": 4}
  },
  "errors": [
    {"phase": "implementer", "kind": "timeout", "count": 1}
  ]
}
```

摘要写到一个索引目录 `.forge/runs/`（每个 run 一个 JSON 文件），用于快速检索而不需解析完整 trace。

**2. 跨运行查询与对比（`forge inspect compare`）**

```bash
# 对比两种 mode 的收敛效率
forge inspect compare --filter 'workflow=build' --group-by mode \
  --metric iterations --metric total_cost_usd --metric duration_ms

# 输出:
# mode=balanced   (12 runs): avg_iter=3.2  avg_cost=$0.51  avg_dur=182s
# mode=engineering (8 runs): avg_iter=5.8  avg_cost=$1.24  avg_dur=410s
# 结论: engineering 比 balanced 多花 2.6 迭代 / 2.4x 成本 / 2.3x 墙钟

# 查看某类 gate 的通过率趋势
forge inspect trend --metric test_pass_rate --window 7d
# 输出: 07-04: 100%  07-06: 75%  07-08: 67%  07-10: 100%
```

**3. 告警规则（可选扩展）**

基于查询结果定义告警：`if avg_gate_fail_rate > 0.2 over last 30d then warn`。可单独交付。

### 边界场景

| 场景 | 行为 |
|------|------|
| 无历史运行 | `forge inspect` 报告 `NO RUN DATA`，不是崩溃 |
| 索引目录被手动删除 | 自动重建：扫描 `.forge/trace.jsonl` 重新索引 |
| 运行中途 crash 无 trace | 运行摘要缺失，索引中不包含该 run（从 checkpoint 恢复时补创建） |
| 大量历史运行（1000+） | 按时间分片索引，按月归档旧摘要 |
| 对比的组大小差异过大（1 vs 100） | 报告样本数，让用户判断统计显著性 |

### 核心收益

| 维度 | 收益 |
|------|------|
| **经验决策** | 从「我认为 engineering 更好」到「数据显示 engineering 收窄了 40% 的迭代方差」 |
| **退化检测** | 新 agent 版本/新 policy 部署后自动发现 gate 失败率上升 |
| **成本优化** | 按 mode、workflow、phase 分维度的成本分析，指导路由和预算配置 |
| **文档自动化** | 运行摘要本身就是「这次分娩的客观记录」——可提交到文档、PR 评论 |

---

## 方向五：自适应治理——基于历史结果的 mode 自调优（Adaptive Governance — Mode Self-Tuning）

> **类型**: 治理 · 学习闭环 · 自治  
> **优先级**: P2（长期 vision G3/G4 的关键增量——让 ForgeOS 不仅执行治理，还优化治理）  
> **代码影响**: 新 `internal/adaptive/` 包 · 修改 `forge migrate` · `internal/mode/mode.go` 新增 `AdaptiveOverride`  
> **差异化证明**: 已有方向覆盖了**模型路由的学习闭环**（Eval→记分卡→Router，G3）。本文覆盖的是**治理策略本身的学习闭环**——当前 mode×lifecycle 是静态的。没有任何现有分析提出让 mode 参数根据历史执行结果自动调整。

### 现状：代码级证据

**证据 A：`modes.yml` 是静态声明，永不自动调整**

```yaml
# modes.yml 第 62-67 行
balanced:
  router_default_tier: sonnet
  harness:
    gates: [lint, test, build, complexity]
    coverage_threshold: 60
    enforce: warn
```

如果一个团队长期工作在 `balanced` mode 下，他们的代码质量和团队纪律可能已经演进——但 mode 不会。所有 gate 阈值、档位、深度从创建项目那天起保持不变，除非人手动改。

**证据 B：`forge migrate` 是手动触发的单次升级**

```yaml
# modes.yml 第 124-130 行
migrations:
  explorer_to_engineering:
    trigger: manual
```

当前只有 `explorer→engineering` 一条迁移路径，且必须人决定、人执行。没有「当项目的测试覆盖率连续 30 天 >80% 时，自动把 coverage_threshold 从 60 抬到 80」这样的渐进适应。

**证据 C：历史数据已就绪但未反馈到治理**

Sprint 26 已验证 `forge evolve` 真跑后产生的 trace 包含 quality/latency/cost 三维真数据。`internal/routing/scorecard.go` 已经实现了 `HistoryTiebreak` 用于路由择优。但这些数据**只喂给模型路由**（G3），不喂给治理策略本身。

具体来说，每次 evolve 完成后系统知道：
- 本次 evolve 用了多少次迭代才 converge
- 哪些 gate 反复 FAIL、哪些一次过
- 平均每次 agent call 的成本
- reviewer 的 verdict 分布

但这些知识**全都没有被用来优化治理参数**——`mode: balanced` 的 `coverage_threshold: 60` 从不因团队持续超额达标而自动提高。

**证据 D：`project.yml` 有 metadata 字段但无进化记录**

```yaml
# project.yml（虚构派生的 schema）
# mode: balanced  # 人设，从不自动变
# lifecycle: mvp  # 人设，从不自动变
```

没有运行历史、没有模式切换记录、没有建议升级的指标。

### 建议方向

引入一个轻量的**自适应治理层**，让 mode 参数可以根据历史运行数据渐进调整，而不需要人手动介入：

**1. 可调优参数白名单**

定义哪些治理参数可以自适应调整，每个参数的调整范围和默认值：

| 参数 | 当前 | 可自适应 | 调整方向 | 依据指标 |
|------|------|----------|----------|----------|
| `coverage_threshold` | 60 (balanced) | ✅ | 向上调整（不超过 80）| 连续 N 次运行覆盖率平均值 |
| `enforce` | warn (balanced) | ✅ | warn → block | gate FAIL 率低于阈值时收紧 |
| `gates` | 4 gates (balanced) | ✅ | 增加（如加 arch）| 架构违规出现频率 > 阈值 |
| `router_default_tier` | sonnet (balanced) | ✅ | 双向 | 历史 scorecard 质量分 |
| `max_file_lines` | 500 (balanced) | ⚠️ 暂不 | — | — |

**2. 自适应触发条件**

`forge evolve` 每次收敛后，自适应层检查历史数据的多个滚动窗口：

```yaml
# 设想的新配置段（在 project.yml 中，可选）
adaptive:
  enabled: true
  policies:
    - metric: coverage_avg     # 过去 20 次运行的覆盖率均值
      window: 20
      threshold: 75
      duration: 3              # 连续 3 个窗口都超标才触发
      action:
        set: coverage_threshold  # 把阈值调到
        to: 80
        max: 85                 # 上限，不超过此值
    - metric: gate_fail_rate   # 过去 30 次运行的 gate 失败率
      window: 30
      threshold: 0.05          # 失败率低于 5%（说明团队成熟了）
      duration: 2
      action:
        promote_enforce: block  # 把 enforce 从 warn 升到 block
    - metric: arch_violations_per_run
      window: 20
      threshold: 3             # 每运行超过 3 次架构违规
      duration: 2
      action:
        add_gate: arch          # 自动往 gate set 中加入 arch gate
```

**3. 自适应输出：渐进升级（非迁移）**

与 `forge migrate`（大版本升级，一口气改所有参数）不同，自适应治理是**渐进式微调**——每次调整一个参数，记录在 `.forge/governance-journal.jsonl`：

```json
{"date":"2026-08-01","action":"coverage_threshold 60→70","reason":"coverage_avg=78% over 20 runs","triggered_by":"adaptive"}
{"date":"2026-08-15","action":"enforce warn→block","reason":"gate_fail_rate=2% over 30 runs","triggered_by":"adaptive"}
```

每次调整是可追溯的、可回滚的。如果调整后某项指标恶化（如 `deploy_fail_rate` 上升），自适应层可以触发自动回滚。

### 边界场景

| 场景 | 行为 |
|------|------|
| 自适应关闭（默认） | `enabled: false`（默认）——当前行为不变，零回归 |
| 无历史数据 | 自适应等待最少窗口数量（如 10 次运行）才计算 |
| 调整后指标恶化 | 自动回滚到调整前值，在 journal 中记录 `reverted` |
| 与生命周期冲突 | `production` 的 `enforce_floor: block` 压过自适应的 `warn` 建议 |
| 团队手动设值 | 手动设置的值的优先级 > 自适应建议值（人始终可以 override） |
| 多项目共享 mode | 自适应只影响当前项目的 `project.yml`，不影响 `modes.yml` 全局声明 |

### 核心收益

| 维度 | 收益 |
|------|------|
| **渐进式严格** | 治理随团队成熟度自动收紧，不依赖人手动跃迁 |
| **风险规避** | 过度严格可通过自动回滚解除，不怕提错阈值 |
| **无需运维干预** | 「set and forget」——自适应层处理常规微调，只留非常规升级给人判 |
| **数据驱动治理** | 阈值不再凭「感觉」，而是基于团队自己的历史表现数据 |
| **G4 增量交付** | 自动 Roadmap（G4）的第一步——系统自己发现「该做什么」的治理版本 |

---

## 优先级建议

| 方向 | 优先级 | 类别 | 杠杆 | 依赖 |
|------|--------|------|------|------|
| ① 治理策略测试框架 | **P1** | 治理/质量 | 最高——让治理本身可测试，防止声明-实现漂移 | 无 |
| ② Agent Runtime 抽象 | **P2** | 架构 | 高——v3 跨厂商前置条件，越早做迁移成本越低 | 无 |
| ③ 收敛信号溯源与信任模型 | **P1** | 诚实性/审计 | 高——收敛信号的分层可信度直接影响自治决策质量 | 无 |
| ④ 跨运行 Trace 分析 | **P2** | 可观测性/运维 | 中——数据已就绪，工具缺失；增值 > 必需品 | trace 系统已有 |
| ⑤ 自适应治理 | **P2** | 学习闭环 | 高——G3/G4 增量，让治理系统自优化 | 方向④的 trace 索引不是必须的，但可增强 |

### 实施路线建议

**Phase 1（下一个 Sprint）**：方向① + 方向③
- 治理测试框架是最高杠杆——让治理本身可被测试，是避免「声明 vs 实现漂移」的最佳投资
- 信号溯源与信任模型是中等改动（新增一个 `Verifiable` 包装类型 + 调整 `evalOne` 的权重），可与测试框架并行

**Phase 2（下一轮）**：方向②（Agent Runtime 抽象）
- 这是纯粹的重构——不改变行为，只从 claude 特定代码中提取接口
- 重构完成后应有零行为变化的测试保障（类似 Sprint 23 的 `acceptance.mjs` 拆分）

**Phase 3（后续）**：方向④ + 方向⑤
- Trace 分析工具独立可用，不依赖任何其他方向
- 自适应治理可以从方向④的数据基础上受益，但不强依赖
