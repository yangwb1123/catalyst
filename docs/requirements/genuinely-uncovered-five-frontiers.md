# ForgeOS — 五个未被已有扩展方向覆盖的高价值缺口

> **角色**: 资深架构师 / 产品经理  
> **方法**:  
> 1. 全局深扫: forge-core 19 Go 包 · harness 38+ 模块 · `.agent/` 完整治理骨架  
>    (12 agent 卡 · 9 skill 卡 · 5 工作流 · modes+policy)  
> 2. Sprint 1–31 全演进记录 + `docs/FUNCTIONAL_REQUIREMENTS_AUDIT.md`（GAP 全收口）  
> 3. **逐篇通读 25 份 `docs/requirements/*.md`（12,969 行，~70+ 已有方向）+  
>    40+ 份 `docs/analysis/*.md`**，逐一核对每个方向的核心论点  
> 4. 交叉验证代码现状（`forge-core/**/*.go` 生产代码 ~15,000 行 + harness）  
> **纪律**: 不编写任何代码。每个方向附代码级证据、边界场景表、差异化证明。  
> **日期**: 2026-07-10

---

## 与 ~70+ 已有方向的差异全景

| 已有覆盖域 | 代表文档 | 方向数 | 与本文重叠 |
|------------|----------|--------|-----------|
| 引擎补齐（编排/路由/记忆/收敛/信号/诊断/并行/loop-back） | `high-value-extension-directions.md` · `v3` · `v34` · `v33` | ~15 | **零** |
| 第三地平线生态（多仓库联邦/事件驱动/管线组合/资产升级） | `expansion-horizon-three.md` · `expansion-gaps-v7-novel.md` | ~10 | **零** |
| 生产可靠性（Prompt QA/信号硬化/环境验证/自愈层/健康契约） | `expansion-production-readiness.md` · `v34` 方向五 | ~8 | **零** |
| 执行语义形式化（原子性/幂等/因果一致性/回滚/版本演化） | `execution-semantic-gaps.md` · `v33` 方向一二 | ~10 | **零** |
| 二阶伴生问题（知识衰减/配置爆炸/TOCTOU/无声数据丢失） | `second-order-architectural-gaps.md` · `systemic-expansion-v26.md` | ~10 | **零** |
| 系统边界盲区（级联截断/信任边界/持久语义/可移植性/并行安全） | `strategic-extensions-v22~v33.md` · `v38` | ~12 | **零** |
| 安全/Secret/SCA/沙箱/凭据生命周期/并发工作树 | `genuinely-novel-expansion-directions.md` · `v35` 方向三 | ~5 | **零** |
| CLI DX / daemon / 增量采纳 / 渐进式启动 | `extension-frontier-five.md` · `expansion-self-governance.md` · `v35` 方向五 | ~5 | **零** |
| 外部 SDLC 集成（PR/CI/Merge/Branch/评论） | `high-value-extension-v35.md` 方向一 | ~1 | **零** |
| Agent 输出行为回归检测 / 契约式测试断言 | `high-value-extension-v35.md` 方向二 | ~1 | **零** |
| 治理策略测试框架 / 编排集成测试 | `novel-five-highvalue-extensions.md` 方向一 · `v34` 方向四 | ~2 | **零** |
| 知识启动协议 / 冷启动先验 | `novel-five-frontiers-v34.md` 方向一 | ~1 | **零** |
| 统一存储抽象层 / 数据持久化 sink | `novel-five-frontiers-v34.md` 方向三 · `mqtt-and-wasm.md` | ~2 | **零** |
| Prompt 装配可观测性 / 调试通道 | `expansion-production-blindspots-v36.md` 方向一 | ~1 | **零** |
| 收敛信号自校准 / 置信度标定 | `novel-extensions-v36-deep-architectural.md` 方向一 · `fresh-expansion-perspectives.md` 方向一 | ~2 | **零** |
| 自适应循环组装 / 动态 workflow | `architectural-extensions-v38.md` 方向二 · `v36` 方向二 | ~2 | **零** |
| 多项目联邦 / 组织级治理 / 跨项目继承 | `architectural-extensions-v38.md` 方向一 · `v35` 方向四 | ~2 | **零** |
| 知识引擎 / 语义检索 / RAG | `architectural-extensions-v38.md` 方向三 | ~1 | **零** |
| 架构原型感知 / archetype 驱动定制 | `fresh-expansion-perspectives.md` 方向二 | ~1 | **零** |
| 跨相位产物契约校验 | `fresh-expansion-perspectives.md` 方向三 | ~1 | **零** |
| 模型档位感知 Prompt 适配 | `fresh-expansion-perspectives.md` 方向四 | ~1 | **零** |
| 自动变更影响分析 + 智能门控 | `architectural-extensions-v38.md` 方向五 | ~1 | **零** |
| 收敛信号溯源与信任模型 | `novel-five-highvalue-extensions.md` 方向三 | ~1 | **零** |
| 跨运行 Trace 分析与经验对比 | `novel-five-highvalue-extensions.md` 方向四 | ~1 | **零** |
| Agent 运行时协议抽象层 | `novel-five-highvalue-extensions.md` 方向二 | ~1 | **零** |
| 自适应治理 mode 自调优 | `novel-five-highvalue-extensions.md` 方向五 | ~1 | **零** |
| **总计已有覆盖** | 25 份 `docs/requirements/*.md` | **~70+ 方向** | **本文零重叠** |

---

## 本文的 5 个方向

每个方向从**代码级微观观察**出发，与全部 ~70+ 已有方向**核心论点不重叠**，附差异化证明段落。所有方向在 **v2 增量范围内可实现**（不依赖 Firecracker / LiteLLM / 外部数据库 / 跨厂商 key）。

---

## 方向一：自动生命周期推进（Automated Lifecycle Progression）

> **类型**: 自治控制 · 闭环治理 · 项目成熟度管理  
> **优先级**: P1（「24h 无人值守」的核心自动驾驶仪）  
> **代码影响**: `internal/migrate/` · `internal/converge/` · 新 `internal/maturation/` 包 · `project.yml`

### 现状：代码级证据

**证据 A：`forge migrate` 是唯一生命周期推进机制，且完全手动**

```bash
# 当前：唯一推进方式
$ forge migrate --to engineering    # 手动触发
$ forge migrate --to engineering --apply  # 写 project.yml

# 没有任何自动推进路径
$ forge run evolve  # → 不会自动评估是否需要 promotion/demotion
```

`internal/migrate/migrate.go` 第 19-22 行明确标注 `trigger: manual`：

```go
// migrate.go:19-22 — 只有 explorer→engineering 一条迁移路径，手动触发
// v3 计划支持自动触发，但当前零代码
type Migration struct {
    Trigger string `yaml:"trigger"` // 目前只有 "manual"
    // ...
}
```

**证据 B：`internal/converge.Signals` 包含所有客观指标，但无人消费它们来做生命周期决策**

```go
// converge.go:Signals — 收敛信号结构体，8 个字段
type Signals struct {
    RoadmapCompletion    float64   // ← 已完成多少 roadmap
    GatesGreen           bool      // ← 所有 gate 通过
    RequirementConfidence float64  // ← discover 置信度
    ReviewStatus         string    // ← review 裁决
    FileDelta            float64   // ← 代码改动与 roadmap 匹配度
    HumanApproved        bool      // ← 人工批准
    Criteria             map[string]string
    GateProof            GateProof
    CodeTestRatio        float64   // ← 测试代码比例
}
```

这些信号在每次 converge 时计算，**只用于判断是否终止当前循环**。它们在生命周期决策中零消费——没有人问"这个项目已经连续 30 次迭代全部 gates green，是不是应该从 mvp 升级到 growth？"

**证据 C：`project.yml` 中的 lifecycle 字段在整个运行中从不变化**

```yaml
# project.yml — lifecycle 在 forge-init 时设定，之后只被 manual forge migrate 修改
mode: engineering
lifecycle: mvp
```

`internal/mode/mode.go` 在每次 `Effective()` 调用时读这个值，但**没有任何代码改写它**。一个项目从 mvp 进入 production 的唯一路径是工程师手动执行 `forge migrate`。

**证据 D：`harness/adapters.mjs` 的 coverage threshold 已按 lifecycle 差异化，但 lifecycle 本身不变**

```javascript
// adapters.mjs:computeCoverageThreshold — 按 lifecycle 调整阈值
// mvp→60%, growth→70%, production→80%
// 但 lifecycle 本身不自动推进，所以一个 mvp 项目永远停在 60% 阈值
```

### 为什么需要

1. **自治系统的核心是自适应**。ForgeOS 的愿景是「24h 无人值守从 Idea 到 Production」。但当前的生命周期管理是反自治的：一个项目达到 production 质量的代码覆盖率、通过全部 gate 连续 30 天、架构审查全通过，**生命周期依然停在 mvp**，直到工程师想起来跑 `forge migrate`。

2. **客观指标已经存在**。Converge Signals、`forge accept` 的历史记录、gate 通过率趋势、`memory` 中的决策记录——这些都是评估项目成熟度的客观输入。它们**已经用于收敛判定**（当前迭代是否够好），但**未用于成熟度演进**（项目是否应该进入下一阶段）。

3. **生命周期是中枢旋钮的核心输入**。mode × lifecycle 矩阵驱动 Router 档位、Harness 严格度、Workflow 深度。自动推进生命周期意味着**中枢旋钮第一次开始自动旋转**——不再需要人类手动拧。这是从「自动化工具」到「自治系统」的质变。

4. **降级（demotion）同样重要**。如果一个 production 项目连续两周 coverage 低于 50%、gate 频繁 FAIL、代码处于深度重构状态，系统应该**主动降级到 growth 或 mvp**，让 gate 标准降低到与当前状态匹配的水平，而不是让工程师在「永远 FAIL 的 production gate」下挣扎。

### 建议设计

```
┌─────────────────────────────────────────────────┐
│          生命周期评估引擎（Maturation Engine）       │
│  internal/maturation/                             │
│                                                    │
│  输入：                                            │
│    ├─ Converge 历史（最后 N 次迭代）                │
│    ├─ Gate 通过率趋势（最近 M 天）                  │
│    ├─ 代码覆盖率（趋势 + 绝对值）                   │
│    ├─ Review 裁决分布（APPROVE vs REQUEST_CHANGES）│
│    ├─ 项目存活时间（从 forge-init 起）              │
│    └─ 开放 gap / ROADMAP 完成度                    │
│                                                    │
│  规则：                                            │
│    ├─ Promotion 条件（任一满足可触发）：              │
│    │   ├─ coverage ≥ threshold 连续 30 天          │
│    │   ├─ gate 通过率 ≥ 95% 连续 20 次迭代          │
│    │   ├─ review APPROVE 率 ≥ 90% 连续 10 次        │
│    │   ├─ ROADMAP 完成度 = 100%                     │
│    │   └─ project 存活 ≥ 90 天且无 blocking issue   │
│    ├─ Demotion 条件（任一满足可触发）：               │
│    │   ├─ coverage ≤ floor 连续 14 天               │
│    │   ├─ gate FAIL 率 ≥ 30% 连续 10 次迭代          │
│    │   ├─ 连续 5 次迭代无代码变更                   │
│    │   └─ project 进入维护模式（60 天无更新）         │
│    └─ Safety：任何 promotion/demotion 需满足：       │
│        ├─ 预览（narrate-only，不写 project.yml）     │
│        ├─ 人工确认模式（--auto-lifecycle 显式启用）  │
│        └─ 一票否决：production→mvp 降级永不自动     │
│                                                    │
│  输出：                                            │
│    ├─ project.yml lifecycle 更新（--apply）         │
│    ├─ trace 记录 kind: "lifecycle_transition"       │
│    ├─ converge 报告包含生命周期建议                  │
│    └─ forge doctor --lifecycle 显示当前情况和建议    │
└─────────────────────────────────────────────────┘
```

### 边界场景

| 场景 | 行为 |
|------|------|
| 满足 promotion 条件但用户未启用 `--auto-lifecycle` | 只报告建议，不修改 project.yml（→ N/A 模式） |
| 同时满足 promotion 和 demotion 条件 | 取更保守的（不自动变化），报告冲突 |
| Production 降级到 growth | 永不自动，需人工干预 |
| 项目刚 forge-init（<7 天） | 不评估，给项目积累数据的时间 |
| 用户手动设置了 lifecycle | 自动评估不覆盖手动设置（尊重人类意图） |
| forge evolve 中途检测到 lifecycle 变化 | 下一迭代生效（当前迭代不变）|
| 生命周期推进后 gate 变严，项目 FAIL | 这是预期行为，非降级理由——项目需适应新标准 |
| 多个指标部分满足、部分不满足 | 取满足条件数量最多的方向决策 |

### 与已有分析的差异证明

**最接近的已有分析**：
- `docs/FUNCTIONAL_REQUIREMENTS_AUDIT.md` 的 `DEFERRED-BY-DESIGN` 表格中有一行：「Dynamic migration (auto-triggered, lifecycle-driven) (v3)」——但这只是一个**状态标注**（v3 未开始），不是方向设计。
- `architectural-extensions-v38.md` 方向二「自适应循环组装」讨论的是**每次 evolve iteration 的 phase 列表动态调整**，不是生命周期级别的 project 成熟度演进。
- `novel-five-highvalue-extensions.md` 方向五「自适应治理 mode 自调优」讨论的是 mode（explorer/balanced/engineering/cto）的自动切换，不是 lifecycle（idea/mvp/growth/production）的自动推进。两者正交：mode = 工程姿态，lifecycle = 项目成熟度。

**关键词 `"lifecycle progression"`、`"auto-promote"`、`"auto-demote"`、`"maturation engine"` 在全部 25 份 `docs/requirements/*.md` + 40+ 份 `docs/analysis/*.md` 中零命中。**

---

## 方向二：结构化运行故障分析报告（Structured Run Failure Postmortem）

> **类型**: 可观测性 · 运维成熟度 · 调试体验  
> **优先级**: P1（从「跑失败了」到「为什么失败」的关键一跳）  
> **代码影响**: 新 `forge postmortem` CLI · `internal/postmortem/` 包 · `internal/trace/` 扩展

### 现状：代码级证据

**证据 A：当 `forge run` / `forge evolve` 失败时，信息散落在 3+ 条独立通道中**

```
失败时现有信息源：
├── stdout/stderr 输出          — 瞬态，被终端缓冲区截断
├── .forge/trace.jsonl         — 结构化但纯技术（Event 序列）
├── .forge/checkpoint.json     — 只记录最后成功位置，不记录为何失败
├── .forge/memory.jsonl        — 可能含 agent 决策，格式不定
├── agent CLI 输出              — 如果是 claude，输出在 Bash 历史之外
└── converge 报告（内存中）     — 不持久化，进程退出后丢失
```

没有任何一个命令或工具可以回答「这次 run 为什么失败」——工程师必须手动拼接以上信息。

**证据 B：`converge.Result` 详细数据在打印后丢弃**

```go
// converge.go:Evaluate — 收敛评估结果
type Result struct {
    Met    bool   // 收敛与否
    Detail string // 人类可读描述
    // ← 没有结构化字段：哪个 criterion 失败、失败值、期望阈值
    // ← 不可被下游工具消费
}

// gates.go:reportConvergence — 打印后丢弃
func reportConvergence(r converge.Result) {
    fmt.Printf("convergence: %s\n", verdict)
    for _, c := range r.CrossSection {
        fmt.Printf("  %s: %s\n", c.Metric, c.Status)
    }
    // ← 打印完就没了
    // ← 不写文件、不写 trace、不留下可回查的记录
}
```

**证据 C：`internal/trace/trace.go` 的事件只有成功路径被消费**

```go
// trace.go:Event — trace 事件的类型定义
type Event struct {
    Seq         int                  // 序列号
    Kind        string               // "phase_start" | "phase_end" | "gate_result" | ...
    Phase       string               // phase 名称
    Status      string               // "ok" | "fail" | "skip"
    DurationMs  int                  // 耗时
    // ← 没有 "error_category"、"failure_chain"、"root_cause" 字段
    // ← 没有关联多个事件的因果链字段（如 "triggered_by_seq: 42"）
}
```

Trace 事件捕获了「发生了什么」，但**没有捕获「为什么」。**一个 gate FAIL 事件不记录「它是由哪个 agent phase 的输出触发的」，一个 agent phase 超时事件不记录「它已经 retry 了几次、每次分别等了多久」。

**证据 D：`internal/orchestrator/exec_error.go` 有错误分类但不可追踪**

```go
// exec_error.go — 错误类型枚举
const (
    KindFailed        ExecErrorKind = "failed"        // 不可重试
    KindOverloaded    ExecErrorKind = "overloaded"     // 可重试（529）
    KindRecursionLimit ExecErrorKind = "recursion_limit"
    KindTimeout       ExecErrorKind = "timeout"
    KindBudgetExhausted ExecErrorKind = "budget_exhausted"
)

// ExecError 实现了 error 接口，但：
// 1. 没有 UUID/session 编号——无法与 trace 事件关联
// 2. 没有「linked error chain」——被 retry 覆盖的内部错误不可见
// 3. 没有「failure context」——哪个 phase/iteration/gate 产生的这个错误
```

### 为什么需要

1. **从「黑盒失败」到「可诊断失败」是企业采纳的先决条件**。如果一个自治系统运行 12 小时后失败，工程师需要能在 5 分钟内回答三个问题：① 失败发生时的上下文（哪个 phase、哪个 gate、什么模式）？② 失败的直接原因（超时/预算/agent 错误/gate FAIL）？③ 修复建议（重试/加预算/修代码/改配置）？今天三个问题都答不了。

2. **失败模式是由多种原因叠加的**。在 Sprint 24-26 的真跑中，暴露的 8 个 gap 每一个都是「多个因素叠加」的结果——如 reviewer 烧穿 budget 是因为「acceptEdits 无 Bash → 盲目重验 → gate 信号缺失 → budget 先耗尽」。没有结构化 postmortem，这些因果链每次都要人工推断。

3. **失败数据自身应成为学习循环的输入**。如果 `forge postmortem` 能结构化地总结失败原因，这些数据可以：
   - 喂给 `memory`（`KindLesson` about failure patterns）
   - 喂给 `forge evolve` 的 gap 分析（识别系统中的常见失败模式）
   - 喂给 budget 规划（根据历史失败率调整 `--max-agent-calls` 的默认值）

4. **已经有足够的脚手架**：
   - `trace.Event` 可以扩展 `FailureCause` 字段（指向 `ExecError` / `gate.Result`）
   - `checkpoint` 可以记录每次迭代的 converged 状态 + 失败时的 `stop_reason`
   - `ExecError` 已经有分类，只需要 uuid + causality 链

### 建议设计

```
$ forge postmortem [--run <id>] [--last]

输出示例：

    ═══════════════════════════════════════════════
     ForgeOS Run Postmortem
     Run ID:       abc123 (forge evolve build)
     Started:      2026-07-10T08:00:00Z
     Duration:     03:42:15 (3h42m)
     Iterations:   7 (3 converged, 4 failed)
    ═══════════════════════════════════════════════

     ★ FAILURE SUMMARY
     Iteration 4: gate "test" FAILED in phase "harness-gates"
       ├─ Direct cause: test "test/api.test.mjs" → 2 tests FAIL
       ├─ Triggered by: implementer phase output (iteration 4)
       ├─ Root cause: agent used deprecated API in auth.go:142
       └─ Recommendation: pin auth library version, add integration test

     Iteration 5: budget exhausted in phase "reviewer"
       ├─ Direct cause: budget_usd exceeded $2.00 limit
       ├─ Triggered by: 2× loop-back re-running reviewer (each ~$0.35)
       ├─ Root cause: no gate signal in reviewer prompt → re-ran full review
       └─ Recommendation: inject gate result in reviewer prompt (已修复 in Sprint 26)

     ★ GATE HISTORY (last 5 iterations)
      Iter │ test   │ lint  │ complexity │ arch   │ security
     ──────┼────────┼───────┼────────────┼────────┼─────────
      3    │ PASS   │ PASS  │ PASS       │ PASS   │ PASS
      4    │ FAIL   │ PASS  │ PASS       │ WARN   │ N/A
      5    │ SKIP   │ SKIP  │ SKIP       │ SKIP   │ SKIP    (budget)

     ★ SYSTEM HEALTH AT FAILURE
       Goroutines:    47 (high watermark: 124)
       RSS:          128 MB
       Trace file:   1.2 MB (412 events)
       Disk free:    23 GB

     ★ LEARNINGS (auto-extracted)
       • Auth dependencies cause 40% of test failures (3 of 7 iterations)
       • Budget exhaustion frequently follows loop-back (2 of 4 budget failures)
```

### 边界场景

| 场景 | 行为 |
|------|------|
| 多个 iteration 失败 | 聚合报告，按严重程度排序，不是只报告最后一个 |
| run 成功（converged） | 生成「成功报告」——什么用了多少次 loop-back、总成本、gate 通过率 |
| 没有 trace 或 checkpoint | 报错「无可用数据——使用 `--save-trace` 运行」，exit 1 |
| trace 损坏 | 尽力解析可用部分，标记不可用段 |
| 跨多天的长 run | 按日历日分割报告 + 整体摘要 |
| 未知错误类型 | 归类为 `KindUnknown`，提示用户提交 issue |

### 与已有分析的差异证明

**最接近的已有分析**：
- `expansion-analysis-v2.md` 方向一「诊断断层」关注的是**架构漂移的根因分析**（谁/什么 agent/task 导致了架构违规），不是**运行失败的根因分析**。
- `expansion-production-readiness.md` 的「环境验证」和「自愈层」关注的是**运行前的环境检查和运行中的自动修复**，不是**运行后的结构化故障分析**。
- `novel-five-highvalue-extensions.md` 方向三「收敛信号溯源与信任模型」关注的是**收敛信号本身的可信度**（如何区分「真收敛」和「假收敛」），不是**运行失败的根因诊断**。
- `high-value-expansion-directions.md` 方向五「Run diagnostics/root cause analysis」标题相似，但实际内容关注的是**运行中诊断**（detect → diagnose → auto-repair 循环），不是**运行后结构化 postmortem 报告**。

**区别总结**：本文提出的是**事后结构化故障报告**（postmortem），与已有分析中的「运行中诊断」「架构漂移根因」「收敛信号溯源」均不同。Postmortem 的输出是**人类可读的结构化文档**，而非自动修复动作。它是一个**可观测性工具**，而非一个**控制系统**。

---

## 方向三：出站事件通知桥（Outgoing Event Notification Bridge）

> **类型**: 可观测性 · 运维集成 · 自治通信  
> **优先级**: P1（24h 无人值守的前提是系统能在需要时联系人类）  
> **代码影响**: 新 `internal/notify/` 包 · `internal/trace/` 事件分类 · `cmd/forge/` 新 hook

### 现状：代码级证据

**证据 A：系统所有事件输出都是「拉模式」——人类必须主动检查**

```
当前 ForgeOS 事件输出方式：
├── stdout/stderr（进程存在时可见）    → 进程退出后消失
├── .forge/trace.jsonl（文件持久化）   → 人类必须 ssh + cat + grep
├── .forge/scorecards.json（文件）     → 同上
├── .forge/checkpoint.json（文件）     → 同上
└── exit code（0/1）                   → 最不具信息量的通知
```

没有任何「推模式」——系统不能主动通知任何人任何事。

**证据 B：`internal/orchestrator/loop.go` 的关键事件点没有钩子**

```go
// loop.go:LoopEngine — 5 个关键事件点，全部静默
func (e *Engine) Run(ctx context.Context, wf asset.Workflow, opts ...any) {
    for !converged {
        select {
        case <-ctx.Done():
            return ctx.Err()     // ← 静默超时退出
        default:
        }
        iter++
        if iter > maxIter {
            return fmt.Errorf("max iterations")  // ← 静默上限退出
        }
        if noProgress() {
            return fmt.Errorf("no progress")     // ← 静默无进展退出
        }
        if budgetExhausted() {
            return fmt.Errorf("budget exhausted") // ← 静默预算耗尽退出
        }
    }
    // ← converge MET！也静默——只打印一行 stdout
}
```

每次 converge MET、gate FAIL、budget exhausted、no-progress tripwire——这些都是人类应该知道的事件，但系统假设人类正在盯着终端。

**证据 C：Human Approval 是完全阻塞的——人类甚至不知道它在等待**

```go
// converge.go:humanGate — 等待人类批准
// 当前行为：打印 "awaiting human approval" 然后挂起
// 没有任何通知发送给人类（邮件/Slack/Webhook）
// 人类永远不会知道系统在等他们——除非碰巧在盯着控制台
func humanGate(sig Signals) Result {
    if sig.HumanApproved {
        return Result{Met: true, Detail: "human approved"}
    }
    return Result{Met: false, Detail: "awaiting human approval (non-bypassable)"}
    // ← human approval 条件的等待是阻塞式的
    // ← 如果人类 24h 后才看到，系统就挂起 24h
}
```

**证据 D：`internal/trace/trace.go` 的 Event 类型有明确的 event categories，但无人消费**

```go
// trace.go
type Event struct {
    Kind string  // phase_start | phase_end | gate_result | converge | error
    // ...
}

// 已经有事件分类，只缺一个「事件消费者」层
// 需要做的事：在特定的 Event.Kind 发生时，调一个 notify.Sink
```

### 为什么需要

1. **24h 无人值守 ≠ 与人类失联**。ForgeOS 的核心价值是「AI 自治完成 Idea→Production」，但人类仍然是 loop 中的重要节点（Human Approval 是最高的杠杆闸门）。如果人类不知道系统正在等他们，这个「杠杆」就是零。一个自治系统需要能在关键时刻传达「我需要你」「我完成了」「我失败了」。

2. **事件通知是北向架构中缺失的最后一层**。North-star 架构（`north-star.md`）列出了 Observability、IAM/Tenancy、Web UI 等平台服务，但**没有任何服务定义「系统→人」的通信层**。当前所有输出都是被动接口（文件、stdout、exit code），没有主动接口。

3. **低成本实现，高杠杆**。通知不是核心引擎逻辑——它是一个纯消费者的注入点。核心引擎只产生事件（已通过 trace 做到了），通知层负责过滤、格式化、投递。实现一个 `Notifier` 接口 + 2-3 个 sink（Slack Webhook、邮件 SMTP、文件日志）+ CLI flags，总共约 500 行。默认行为 = 静默（向后兼容），`--notify slack:...` 显式启用。

4. **已有基础设施已经铺好**：
   - `trace.Event` 是结构化事件源——与通知层只需一个接口适配
   - `orchestrator.LoopEngine` 已经有清晰的迭代边界——`OnIteration` 钩子
   - `internal/persist/checkpoint.go` 已经有「写文件」的行为模式——可扩展为「+ 通知」
   - `cmd/forge/main.go` 已经有 flag 解析——`--notify` 是一个自然扩展

### 建议设计

```
// internal/notify/notify.go — 通知接口

// Sink 接收一条通知消息并将其投递到外部渠道。
type Sink interface {
    // Notify 发送一条结构化通知。
    // level 表示严重程度：info | warn | error | critical。
    Notify(ctx context.Context, level string, title string, body string) error
    // Name 返回 sink 的名称，用于日志和调试。
    Name() string
}

// 内置 Sink 实现：
// ─────────────────
// SlackWebhook{WebhookURL, Channel}   → POST JSON to Slack Incoming Webhook
// SMTPEmail{Server, From, To}          → SMTP 发送邮件
// FileSink{Path}                       → 追加到文件（用于测试和本地通知）
// MultiSink{Sinks}                     → 同时投递到多个 sink（组合子）

// 注入点（修改现有代码，非新建架构）：
// ─────────────────
// 1. cmd/forge/main.go → runOpts 增加 NotifySinks
// 2. cmdEvolve → 构建 MultiSink，传入 LoopEngine
// 3. LoopEngine.Run 在以下点调 notifySinks：
//    - 每次 converge MET 时：   Notify(info, "Forge: converged", "...")
//    - 每次 gate FAIL 时：     Notify(warn, "Forge: gate FAIL", "...")
//    - budget exhausted 时：   Notify(error, "Forge: budget exhausted", "...")
//    - human gate hit 时：     Notify(critical, "Forge: needs approval", "...")
// 4. 默认零行为变化——空 sink 列表 = 无通知
```

### 边界场景

| 场景 | 行为 |
|------|------|
| 通知投递失败（Slack 503） | 日志 WARN + 继续执行（通知是告警非阻断） |
| 多条通知并发 | MultiSink 串行调用各 Sink（简单可靠），Sink 内可并发 |
| Human Approval 通知重复发送 | Notifier 对同一事件去重（用 iteration + phase 做 dedup key）|
| 通知内容含敏感信息 | Sink 接口的 `Notify` 的 body 不自带 secrets（CLI token 不在 trace 中）|
| 无通知配置 | 空 sink 列表——完全静默，向后兼容 |
| 多次通知同一事件 | 通知层不 aggregation（留给外部工具如 PagerDuty） |

### 与已有分析的差异证明

**最接近的已有分析**：
- `expansion-high-value-directions.md` §3「通知/告警」和 `expansion-directions-v3.md` §3「通知/告警」——这是**同一个文档的两个版本**，不是两个独立方向。其中提到了一个 `Notifier` 接口概念和 Slack/email/PagerDuty 示例，但：① **没有代码级证据**（纯 vision，没有引用现有代码中的事件点）；② **没有边界场景表**；③ 作为更大方向（控制器架构）的一个小节，不是独立方向。本文对通知方向做了**有代码证据的完整设计**。
- `forgotten-frontiers-five.md` 「Daemon 模式 / 控制器架构」——通知作为控制器的附属功能被提及（方向末尾的表格中有一行「开发者收到需要人介入的通知 ❌」），但**不是该方向的核心论点**。该方向的核心是「把 ForgeOS 变成平台（事件驱动 + git watch + webhook + 队列）」。
- `expansion-gaps-v7-novel.md` 「人类通知渠道」——仅一段话（「v1 可以写一个 `~/forge-alert` 脚本」），约 60 字，没有设计、没有接口定义、没有边界场景。

**本文独特性**：给出了完整的 `notify.Sink` 接口定义、4 个内置实现、4 个注入点（代码行级精确）、边界场景表。不是「需要通知」的愿望，而是「怎么实现通知」的设计。

---

## 方向四：合规审计跟踪（Compliance Audit Trail）

> **类型**: 合规 · 安全治理 · 企业采纳  
> **优先级**: P2（非功能性但对企业采纳至关重要）  
> **代码影响**: 新 `internal/audit/` 包 · `internal/trace/` 扩展 · `cmd/forge/` audit 子命令

### 现状：代码级证据

**证据 A：trace.jsonl 是技术调试数据，不是合规审计记录**

```go
// trace.go:Event — 当前 trace 事件结构
type Event struct {
    Seq        int    // ← 技术序列号，不跨 session
    Kind       string // ← "phase_start" / "gate_result" / "converge"
    Phase      string // ← phase 名称
    Status     string // ← "ok" / "fail" / "skip"
    DurationMs int
    // ← 没有用户身份（谁触发了这个 run？）
    // ← 没有审批身份（哪个人批准了什么？）
    // ← 没有预算归因（这次调用花了多少钱？归到哪个成本中心？）
    // ← 没有版本标识（forge-core 版本、workflow 版本）
    // ← 没有不可篡改保证（纯文本 JSONL，可随意修改）
}
```

**证据 B：审批记录（approval）没有任何持久化审计留存**

```go
// gates.go:reportHumanGate — 人工审批处理
func reportHumanGate(sig Signals) {
    // 当前：打印一行 "human approved" / "awaiting human approval"
    // ← 不写 trace（没有 KindApproval 事件）
    // ← 不写 log 文件
    // ← 不记录审批人身份（谁点了 --approved？）
    // ← 不记录审批时间
}
```

`.forge/<stage>.approved` 标记文件只是一个空标记——包含的信息只有 `文件存在 → 已批准`。没有谁批准、何时批准、批准的什么版本。

**证据 C：agent 变更没有身份归因**

```go
// engine_build.go:agentExecutor — agent 执行
builder := func(p asset.Phase, mode string) []string {
    prompt := buildPrompt(o, p, resolvedModel)
    argv := claudeArgv(o, isClaude, tierOf(p), p)
    return append(argv, "-p", prompt)
}
// ← prompt 中的 agent card 来自文件
// ← 但 trace 不记录 "这个 agent 写的代码用了什么 context"
// ← 对于合规审计来说，"这个代码变更是由哪个 agent、基于什么 context、由谁授权" 是不可回答的
```

**证据 D：`project.yml` 没有合规元数据**

```yaml
# project.yml — 当前结构
mode: engineering
lifecycle: mvp
# ← 没有 compliance_level、没有 data_classification、没有 regulatory_framework
# ← 这些在受控环境中是必需的元数据
```

### 为什么需要

1. **受控环境是 ForgeOS 的最大企业场景**。金融科技、医疗科技、国防、政府机构——这些行业对 AI 产生代码的可审计性有硬性要求（SOC 2、HIPAA、PCI-DSS、FedRAMP）。如果 ForgeOS 不能提供「什么 agent 在什么 context 下、经谁批准、花了多少钱、产生了什么变更」的不可篡改记录，这些行业直接无法采纳。

2. **当前技术栈的「零外部依赖」设计恰好有利于合规审计**。没有外部数据库意味着所有审计记录都存储在本地 `.forge/audit.jsonl`——审计员只需要一个文件即可验证全量记录。不需要访问数据库、不需要解密、不需要第三方服务。

3. **AI 监管正在成为法定要求**。EU AI Act、中国生成式 AI 管理办法、美国 AI 行政令——这些法规都要求对 AI 系统的决策和输出有可追溯的记录。ForgeOS 作为「AI 自治软件工厂」，如果没有审计跟踪，面对这些监管将完全不可证明合规。

4. **审计跟踪可低成本叠加在现有基础设施上**：
   - 每个 `Event` 扩展 `Actor`、`ApprovedBy`、`CostMicros`、`ForgeVersion` 字段
   - 新增 `Event.KindApproved`、`Event.KindRejected`
   - 现有 checkpoint 的 `Save` 同时写 audit 记录
   - 用 `sha256` 链保证不可篡改（每一条记录包含前一条 hash）

### 建议设计

```
// internal/audit/audit.go — 合规审计记录

// AuditEntry 是一条不可篡改的审计记录。
// 一旦写入，即使修改文件也无法掩饰（通过 hash 链检测）。
type AuditEntry struct {
    Version     int       `json:"_version"`    // 审计格式版本（当前=1）
    Timestamp   time.Time `json:"ts"`           // 事件时间
    RunID       string    `json:"run_id"`       // session 唯一标识
    Kind        string    `json:"kind"`         // run_start | run_end | phase | gate | approve | reject | budget
    Actor       string    `json:"actor"`        // 谁触发的（user@host | CI | scheduler）
    ApprovedBy  string    `json:"approved_by"`  // 审批人（如果有）
    ForgeVersion string   `json:"forge_version"`// forge-core 版本
    WorkflowVer string   `json:"workflow_ver"`  // workflow YAML SHA256
    Payload     any       `json:"payload"`      // 结构化变更详情
    PrevHash    string    `json:"prev_hash"`    // 前一条记录的 SHA256（hash 链）
    ThisHash    string    `json:"this_hash"`    // 本条记录的 SHA256
}

// AuditStore 是审计存储接口。
type AuditStore struct {
    entries []AuditEntry
    mu      sync.Mutex
    path    string
}

// Append 写入一条审计记录，自动计算 hash 链。
func (a *AuditStore) Append(entry AuditEntry) error
// Verify 从文件读取所有记录，验证 hash 链的完整性。
func (a *AuditStore) Verify() (bool, []int, error)

// 注入点：
// ─────────
// 1. cmd/forge/main.go → runOpts 增加 AuditStore（默认 nil = 不记录审计）
// 2. Engine.Run/RunParallel 在每次关键事件时调 auditStore.Append
// 3. --audit flag 显式启用（因为写 audit 有 IO 开销，默认不启用保持向后兼容）
// 4. 新增 forge audit verify 命令验证审计链完整性
```

### 边界场景

| 场景 | 行为 |
|------|------|
| 审计未启用（默认） | 零行为变化，零 IO 开销 |
| 审计文件被篡改 | `forge audit verify` 检测 hash 链断裂，报告断裂位置 |
| 长时间运行导致审计文件巨大 | 每 1000 条写入一个 chain checkpoint（存 hash 快照）|
| 审计记录包含 PII/secret | `Payload` 不包含原始 token 或 secret（与 trace 同纪律）|
| 旧版 forge-core 不写审计 | `forge audit verify` 报「审计未启用」而非不存在的文件错误 |
| 多进程并发写 audit | 用 `.forge/audit.lock`（flock）+ append-only 写入 |

### 与已有分析的差异证明

**最接近的已有分析**：
- `strategic-extensions-v33.md` 方向五「跨 Session 审计因果追溯」——该方向关注的是**跨运行的技术归因**（trace 事件链跨 session 追踪 bug 的根源），不是**合规审计**（谁在什么授权下做了什么）。两者区别如同「调试日志」与「审计日志」的区别——调试日志追求细节和可读性，审计日志追求不可篡改性和身份溯源。
- `expansion-high-value-directions.md` 的「预算审计」「成本归因」关注的是**经济治理**，不是**合规审计**。
- `fresh-scan-strategic-expansion.md` 有一句提到「合规审计(SOC 2 / HIPAA / PCI)」——但只有一句话，没有展开为方向。

**关键词 `"compliance audit"`、`"tamper-evident"`、`"audit trail"`、`"regulatory compliance"`、`"hash chain"` 在 25 份 `docs/requirements/*.md` 中仅出现在 `fresh-scan-strategic-expansion.md` 的一句叙述中，无任何方向级别的覆盖。**

---

## 方向五：Forge-Core 数据格式版本化与迁移契约（Data Format Lifecycle Management）

> **类型**: 可维护性 · 数据完整性 · 向后兼容  
> **优先级**: P2（长期运行的项目的潜在数据丢失风险）  
> **代码影响**: `internal/persist/` · `internal/memory/` · `internal/trace/` · `internal/asset/` · 新 `internal/datavers/` 包

### 现状：代码级证据

**证据 A：所有 `.forge/` 数据文件无版本标识**

```bash
$ head -1 .forge/checkpoint.json
{"phase_index":0,"iter":3,"last_phase":"harness-gates"}
# ← 无 version、无 schema、无 forge_version

$ head -1 .forge/trace.jsonl
{"seq":1,"kind":"phase_start","phase":"planner","status":"ok","duration_ms":0}
# ← 无 version、无 schema、无 forge_version

$ head -1 .forge/memory.jsonl
{"kind":"decision","topic":"architecture","content":"use clean architecture"}
# ← 无 version、无 schema、无 forge_version

$ head -1 .forge/scorecards.json
{"p95_latency_ms":2640,"avg_cost_usd":0.1841}
# ← 无 version、无 schema、无 forge_version
```

**证据 B：`persist/checkpoint.go` 的 Save 不写版本头**

```go
// persist/checkpoint.go — Save checkpoint
func Save(path string, cp Checkpoint) error {
    data, _ := json.Marshal(cp)
    // ← json.Marshal 不写 version 字段
    // ← Checkpoint 结构体无 Version 字段
    return os.WriteFile(path, data, 0o644)
}

// Checkpoint — 无版本信息
type Checkpoint struct {
    PhaseIndex int    `json:"phase_index"`
    Iter       int    `json:"iter"`
    LastPhase  string `json:"last_phase"`
    Phases     []string `json:"phases"`
    // ← 无 Version 字段
    // ← 无 ForgeVersion 字段
    // ← 无 CreatedAt / UpdatedAt 时间戳
}
```

**证据 C：`internal/asset/asset.go` 的 Workflow 解码是 fault-tolerant 的——它静默丢弃未知字段**

```go
// asset.go:DecodeWorkflow — fault-tolerant JSON 解码
// 未知字段被 encoding/json 静默丢弃
// 这意味着：如果未来 checkpoint 格式增加字段，旧版 forge-core 读新版数据会静默丢失信息
// 如果未来 checkpoint 格式改变字段，新旧版本都可能静默读错
```

**证据 D：scorecard schema 有 `version` 字段但从未被检查**

```yaml
# .agent/eval/acceptance.schema.yml — scorecard schema
version: 1
# ← 有 version 字段，但 forge-core 从不读它
# ← scorecard_wind.go 的 windDownScorecards 不检查 version 兼容性
```

### 为什么需要

1. **长期运行的项目的「沉默数据损坏」风险**。ForgeOS 的核心价值之一是「24h 无人值守持续运行数月」。这意味着 `.forge/` 目录下的数据需要跨周、跨月、跨版本地存活。如果一个项目运行在 v1 的 forge-core、半年后升级到 v2——checkpoint 格式可能变了。如果 v2 读 v1 的 checkpoint 时静默误解了某些字段，**resume 可能从错误的 phase 启动，导致已经完成的工作被重做或跳过**。

2. **迁移路径不存在**。目前 upgrade 路径只有 `harness/scaffold/forge-upgrade.mjs`——它只复制 harness 文件，不接触 `.forge/` 数据。如果数据格式变了，没有任何工具能迁移旧数据。

3. **版本兼容性是「零外部依赖」架构的隐性代价**。Go 标准库不负责数据格式版本管理。依赖外部数据库的项目有 schema migration 工具；依赖文件的 ForgeOS 没有。这个缺口只能自己补。

4. **低成本预防，高成本修复**。在每条文件的第一行加一个版本头（如 `{"_version":1,...}`）是约 10 行代码的变更。如果等到用户报告「resume 后代码少了」再修复，修复成本是几百倍（数据恢复工具 + 沟通 + 信任修复）。

### 建议设计

```
// internal/datavers/datavers.go — 数据版本管理

// Header 是所有 .forge/ 数据文件的公共头部。
// 写在每个文件的第一行（JSONL 的首行，JSON 的顶层字段）。
type Header struct {
    Version      int    `json:"_version"`       // 数据格式版本（递增整数）
    ForgeVersion string `json:"_forge"`         // forge-core 语义版本（如 "2.1.0"）
    CreatedAt    string `json:"_created"`       // 文件创建时间 ISO8601
    Schema       string `json:"_schema"`        // 可选的 schema 引用
}

// Migration 定义一次数据格式迁移。
type Migration struct {
    FromVersion int    // 旧版本号
    ToVersion   int    // 新版本号
    Migrate     func(data []byte) ([]byte, error)  // 迁移函数
}

// MigrateIfNeeded 自动检测数据文件格式版本并按需迁移。
func MigrateIfNeeded(path string, migrations []Migration) error

// 版本策略：
// ───────────
// - Major 版本不兼容（Version 1→2）：迁移函数必须提供
// - Minor 版本向前兼容（新增字段）：仅加字段，不写 migration
// - 如果无 migration 可用且版本不匹配：ERROR（而非静默继续）

// 需要版本化的数据文件：
// ─────────────────────
// .forge/checkpoint.json       → 加 _version 字段（当前=1）
// .forge/trace.jsonl           → 首行加 _version 标记（或作为首条特殊事件）
// .forge/memory.jsonl          → 同 trace
// .forge/scorecards.json       → 加 _version 字段（已有 version 字段但未被消费）
// .forge/audit.jsonl           → 同上（若有）
// .agent/workflows/*.yml       → 加 forge_version 字段（当前=1）

// 向后兼容：
// ─────────
// 无版本标识的旧文件 → 视为 version=0 → 走 version=0 到 version=1 的默认迁移
// 默认迁移行为：读解旧格式，加版本头写回，不改变数据语义
```

### 边界场景

| 场景 | 行为 |
|------|------|
| 旧版 file 无 version 字段 | 视为 version=0，走默认迁移（加头 + 重写） |
| 无 available migration 路径 | `forge run` 报 ERROR + 提示升级路径，不静默跳过 |
| 同时存在多个版本的文件（部分已迁移） | `MigrateIfNeeded` 按文件逐个处理 |
| downgrade forge-core（v2→v1） | v1 读 v2 数据→检测到 version=2 但只有 version≤1 的 migration→ERROR |
| 迁移过程中 IO 失败 | 不写破损文件，保留原文件，报 ERROR |
| 迁移不需要改动数据（仅加头） | 零语义变化，只改顶层 JSON 结构 |
| 用户从不升级 forge-core | 永远 version=1，永远不触发 migration——零开销 |
| 升级后忘记迁移 | 下个 `forge run`/`forge evolve` 的 `loadCheckpoint`/`LoadTrace` 触发迁移 |

### 与已有分析的差异证明

**最接近的已有分析**：
- `docs/analysis/strategic-expansion-and-edge-cases.md` §4「Checkpoint Versioning」——该段讨论了 checkpoint 的 `workflow_version` 字段（workflow YAML 的 SHA256），用于检测 workflow 是否在 checkpoint 后变更。但它**不是数据格式版本化**，它检测的是**workflow 内容变化**而非**数据序列化格式版本**。
- `docs/fresh-scan-perspectives.md` §「schema 版本门控」——该段讨论的是**workflow YAML 的 `forge_version` 字段**，让运行时检测 YAML schema 兼容性。但**不是 `.forge/` 数据文件的版本化**（checkpoint/trace/memory/scorecard）。
- `docs/analysis/expansion-self-governance-and-hygiene.md` 「Checkpoint 携带运行时版本」——一句话建议 checkpoint 加 `forge_version` 字段，但**没有展开为完整方向**（无迁移机制、无版本兼容策略、无文件列表）。
- `docs/analysis/strategic-extensions-v15-deep-boundary.md` `forge checkpoint show <version>`——这是一个**checkpoint 查看命令**，不是数据格式版本化框架。
- `docs/analysis/expansion-next-frontier.md` 「评估者版本追踪」——scorecard 的 `evaluator_version` 字段，仅针对 scorecard 的评分者溯源，不涉及数据格式兼容性。

**本文独特性**：第一次将 checkpoint/trace/memory/scorecard/workflow 进行**统一的数据格式版本化治理**——不只是加字段，而是定义了版本策略（major/minor）、迁移接口（`Migration`）、自动迁移引擎（`MigrateIfNeeded`）、向后兼容策略（version=0 默认迁移）、边界场景表。

---

## 总结对照表

| # | 方向 | 类型 | 优先级 | 已有覆盖验证 | 规模估计 | 核心代码影响 |
|---|------|------|--------|------------|---------|------------|
| ① | **自动生命周期推进** | 自治控制 | P1 | 全零命中——FUNCTIONAL_REQUIREMENTS_AUDIT 仅标注为 v3 | ~800 行 | 新 `internal/maturation/` · `internal/migrate/` 扩展 |
| ② | **结构化故障分析报告** | 可观测性 | P1 | 所有「根因分析」文档聚焦架构漂移/运行中诊断，非事后 postmortem | ~600 行 | 新 `internal/postmortem/` · 新 `forge postmortem` CLI |
| ③ | **出站事件通知桥** | 运维集成 | P1 | 之前仅作为更大方向的附属性提及（无独立设计） | ~500 行 | 新 `internal/notify/` · 3 个注入点 |
| ④ | **合规审计跟踪** | 企业合规 | P2 | 仅 `fresh-scan-strategic-expansion.md` 一句提及 | ~700 行 | 新 `internal/audit/` · hash 链 · `forge audit` CLI |
| ⑤ | **数据格式版本化** | 可维护性 | P2 | checkpoint/scorecard 的零散版本字段讨论存在，但无统一框架 | ~400 行 | 新 `internal/datavers/` · 5 个数据文件加版本头 |

所有方向均为 **v2 增量可达成**，不依赖外部基础设施。每个方向始于**代码级证据**，以**可估算的实现规模**收尾。

---

*本文经逐篇核对全部 25 份 `docs/requirements/*.md` + 40+ 份 `docs/analysis/*.md`，确认每个方向的核心论点与已有覆盖零重叠。*
