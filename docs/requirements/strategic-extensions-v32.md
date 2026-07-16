# ForgeOS — 五维扩展方向 v32：全局深扫后的盲区识别

> **角色**: 资深架构师 / 产品经理  
> **方法**: 全局代码库深度扫描（forge-core 18 Go 包 / 195+ 源文件 / 707+ 测试 /  
>   harness 39 模块 / 5 工作流 / 12 agent 卡 / 9 skill 卡 / pi-batch.py / examples /  
>   `.forge/` 运行时产物分析 / Sprint 1–31 完整演进）  
> **交叉验证**: 通读 **40+ 篇 `docs/analysis/*.md`** + **13 篇 `docs/requirements/*.md`** +  
>   `docs/FUNCTIONAL_REQUIREMENTS_AUDIT.md` + 全部 ADR + DECISIONS.md + PROJECT.md + ARCHITECTURE.md  
> **核心承诺**: 每个方向与已有 **~60 个方向**的核心论点**零重叠**  
> **纪律**: 不编写任何代码。每方向附具体代码位置 + 与已有分析的差异化证明  
> **日期**: 2026-07-09

---

## 前言

ForgeOS 已有 **60+ 个扩展方向**被 40+ 篇分析文档覆盖，涵盖：

| 维度 | 覆盖方向数 |
|------|-----------|
| 功能引擎补齐（路由/编排/记忆/收敛/诊断/自适应装配） | ~15 |
| 第三地平线生态（多仓库联邦/事件驱动/管线组合/资产升级/修正学习） | ~10 |
| 生产可靠性（Prompt QA / 信号硬化 / 环境验证 / 自愈层） | ~8 |
| 执行语义形式化（原子性/幂等性/因果一致性/版本演化） | ~8 |
| 二阶伴生问题（知识衰减/配置爆炸/TOCTOU/无声数据丢失/数据生命周期） | ~10 |
| 系统性边界盲区（级联截断/YAML分歧/信任边界/持久语义/可移植性） | ~10 |
| 架构盲区与多波分析（并行编排/成本智能/BM25检索/收敛定量语义） | ~10 |
| **总计** | **~60** |

**本文瞄准的是这 60 个方向之外的盲区。** 每个方向都经过代码级证据验证，并与已有分析逐行比对确保零重叠。

---

## 方向一：Model-Tier-Aware Context Window Budgeting —— 按模型容量动态装配 Prompt

**类型**: 性能优化 · 成本优化 · 架构  
**优先级**: P1（直接影响每次 agent 调用的 token 成本和质量）  
**代码影响**: `internal/prompt/` · `internal/routing/` · `cmd/forge/prompt_context.go` · `cmd/forge/prompt_memory.go`

### 现状

`buildPrompt` 当前**对所有模型一视同仁**地构建 prompt：

```go
// forge-core/cmd/forge/prompt_context.go
func buildPrompt(roleCard, constraints, task, adrContext, memoryContext, gateLedger string) string {
    // 拼接所有 lane, 固定顺序, 固定长度
}
```

注入的上下文量只取决于**内容存在性**（是否有 ADR、是否有 memory 条目），不取决于**目标模型的上下文窗口容量**。当前注入策略：

| 上下文组件 | 注入策略 | 最大 token 影响 |
|-----------|---------|---------------|
| 角色卡 (roleCard) | 总是全量注入 | ~2-5K tokens |
| ADR 上下文 | 全部 ADR 标题 + 内容 | ~2-20K tokens（随系统增长） |
| Memory 条目 | `memoryCap: 32` 条最新 | ~5-15K tokens |
| Gate 裁决历史 | 全量注入 | ~1-3K tokens |
| 约束/红线 | 全量注入 | ~1-2K tokens |
| Task 描述 | 全量注入 | ~1-5K tokens |

**问题**：

1. **Opus（$15/1M input tokens）和 Sonnet（$3/1M input tokens）收到相同的 prompt** —— 昂贵模型没有被利用其更大上下文窗口的特长，廉价模型也没有因较小 prompt 而受益。固定 prompt 大小意味着无法在模型间做经济性权衡。

2. **prompt 内容没有按 tier 做优先级的阶梯式注入** —— 对于 Haiku（最适合简单任务），完整注入 32 条 memory + 全部 ADR 是浪费；对于 Opus（最适合复杂推理），可能还需要更多上下文才能做出准确判断。

3. **不存在「prompt 预算」概念** —— 系统从不问「这个任务的 prompt 应该花多少 token」。当前模式是「全部注入，超出截断」（`boundMemory` 所做的 `memoryCap` 截断），而非「按预算分配各级内容的权重」。

4. **`internal/routing/routing.go` 已有 `TierFor` 决策，但 prompt 层不知道这个决策** —— 路由层知道「当前 phase 用哪个模型」，prompt 层不知道。两层的隔离意味着路由层无法告知 prompt 层「这是 Opus 调用，可以给更多上下文」。

### 代码级证据

```go
// forge-core/cmd/forge/prompt_context.go (约第 200 行)
// buildPrompt 接受 (roleCard, constraints, task, adrContext, memoryContext, gateLedger string)
// 没有 modelTier 参数, 没有 contextBudget 参数
```

```go
// forge-core/cmd/forge/prompt_memory.go
// memoryContext 注入所有匹配 topic 的条目, 上限 memoryCap (当前 32)
// 不区分目标模型: Haiku 也收到 32 条, Opus 也收到 32 条
func memoryContext(...) string {
    // ...
    for i, entry := range entries {
        if i >= memoryCap { break }
        // ...
    }
}
```

```go
// forge-core/internal/prompt/prompt.go
// prompt 包当前只有缓存逻辑, 没有上下文预算分配
type ContextCache struct {
    // run-scoped 不变上下文构建一次
    // 没有 Budget, TokenBudget, TokenUsage 等概念
}
```

```go
// forge-core/internal/routing/routing.go
// TierFor 返回 model tier, 但无接口让 prompt 层消费
func TierFor(agent, mode string, lifecycle ...) string {
    // 返回 "haiku" | "sonnet" | "opus"
    // 但从不作为 prompt 构建的输入
}
```

### 未被已有分析覆盖的证明

- `docs/requirements/high-value-extension-directions.md` 边栏提到「Emits 产物过大 → 产物摘要应该 truncate（如 2000 字符），避免 blow up prompt context window」——这是**产物侧**的截断，不是**prompt 装配策略**。关注的是「某个太大的输入怎么处理」，不是「按模型容量动态分配各种输入的预算」。

- `docs/analysis/strategic-expansion-v21.md` 提到「context window 浪费 —— prompt.Gather 每次注入 AGENTS.md 的约束 bullet list + 所有 ADR 标题，即使本次 phase 不涉及架构决策」——这是按 **agent 角色** 过滤上下文（"implementer 不需要 ADR 全文"），不是按**模型 tier** 动态调整上下文量。

- `docs/analysis/five-extensions-v10-distinct.md` 方向一「自适应上下文窗口预算」聚焦于「当一个 context window 太大时（超出模型限制），如何做渐进式注入或分块注入」——这是**超限处理**（error path），不是**常规的按 tier 优化**。

- **三者都不重叠**：已有分析讨论的是「截断过大输入」「按角色过滤」「超限后分块」。本文方向讨论的是「按模型 tier 分配上下文预算，让昂贵模型得到更多上下文、廉价模型得到精炼上下文」——这是一个独立的、未被提出的优化策略。

### 建议方向

1. **Prompt 预算表**：为每个模型 tier 建立默认的 `context_budget`（token 上限）：
   ```
   opus:   context_budget=64000  (利用其大窗口优势)
   sonnet: context_budget=32000  (标准)
   haiku:  context_budget=8000   (精炼，适合快速准确的任务)
   ```
   预算在 `project.yml` 或 `modes.yml` 中可配置。

2. **预算分配策略**：将 `context_budget` 按优先级分配给各上下文组件：
   ```
   1. 角色卡 (固定 ~10%)
   2. 约束/红线 (固定 ~10%)
   3. Task 描述 (固定 ~10%)
   4. ADR 上下文 (可变 ~30%，按相关性排序直到用尽预算)
   5. Memory 条目 (可变 ~30%，按新鲜度/置信度排序直到用尽预算)
   6. Gate 历史 (可变 ~10%)
   ```
   超预算时，从优先级最低的组件开始截断，而非固定 `memoryCap`。

3. **Prompt 构建的 tier 感知**：`buildPrompt` 增加 `tier string` 参数，`engine_build.go` 在构造 Engine 时从 `routing.TierFor` 获取并传入。

4. **Honesty 报告**：trace event 增加 `prompt_token_count` 和 `context_budget`，使每此 agent 调用的实际 token 消耗与预算可追踪。Scorecard 增加 `budget_utilization`（实际 token / 预算）。

### 边界情况

| 场景 | 风险 | 缓解 |
|------|------|----------|
| Model tier 升级（Sonnet → Opus in review phase） | Prompt 需要重建（更长版本） | prompt 预构建不 cache 跨 tier |
| 用户自定义模型（非标准 tier） | 未知 context window 容量 | 默认使用最保守预算（sonnet 级别） |
| 上下文预算不足导致关键内容被截断 | Agent 缺少关键信息 | 截断顺序：低优先级内容先截断；角色卡、约束、Task 永不被截断 |
| 同一 iterate 内多个 phase 用不同模型 | 每个 phase 的 prompt 不同 | 天然支持——每个 phase 独立 `buildPrompt` |

---

## 方向二：Governance-as-Code 变更审查流水线 —— 治理文件的结构化修改工作流

**类型**: 工程化 · 治理 · 合规  
**优先级**: P1（所有 ForgeOS 用户都面临此问题）  
**代码影响**: `cmd/forge/` · 新 `internal/governance/` · harness 扩展

### 现状

ForgeOS 治理自身的文件清单（约 25 个文件）：

```
.agent/                 ← 治理配置的核心
  project.yml            ← mode/lifecycle/overrides
  policies/modes.yml     ← 中枢旋钮配置
  routing/policy.yml     ← 模型路由策略
  agents/*.md            ← 12 个 agent 角色卡（含机读契约）
  skills/*.md            ← 9 个技能卡
  workflows/*.yml        ← 5 个工作流定义
  eval/*.yml             ← 评估 schema
harness/                ← 执法器
  policies.yml           ← 门禁严格度
  adapters/*.yml         ← lint/coverage 适配器
  sca.mjs / secret-scan.mjs / ...
.arch/rules.yaml        ← 架构规则
```

**所有这些文件都由 agent 直接修改。** agent 可以（通过 `acceptEdits`）修改 `project.yml` 的 mode、删除 `AGENTS.md` 中的红线、放松 `policies.yml` 的 enforce 级别。

当前没有任何结构化的工作流来管理这些变更：

| 操作 | 当前行为 | 理想行为 |
|------|----------|----------|
| 修改 project.yml 的 mode | 直接文件写入 → 下次 forge run 生效 | 创建 governance change proposal → review → apply |
| 更新 agent 卡的机读契约 | 直接文件写入 → 下次 buildPrompt 使用 | diff → 验证契约兼容性 → 批准 |
| 修改 workflow YAML | 直接文件写入 → 下次 loadWorkflow 使用 | proposal → workflow 语义验证 → 批准 |
| 升级 harness 执法器 | 直接文件覆盖 → 即时生效 | upgrade plan → 影响评估 → 滚动部署 |

**这是 ForgeOS 自身治理模型的最大信任盲区**（已在 `uncovered-frontiers-v25.md` 方向三中识别为「Agent 自修改治理文件」问题）。

### 代码级证据

```go
// forge-core/cmd/forge/main.go: loadWorkflow
// 每次加载直接从磁盘读 YAML，无版本校验，无 diff
// 如果 workflow 被修改了，系统直接静默接受，不记录变更
```

```go
// forge-core/cmd/forge/prompt_context.go
// buildPrompt 从角色卡文件读取内容
// 不校验角色卡是否被篡改（SHA 或版本与预期不符？）
```

```python
# harness/check.py
# 有 10 个 check 函数检查治理资产的一致性
# 但检查的是同步一致性（声明 vs 实现），不是版本演化（旧版本 vs 新版本）
```

```yaml
# .agent/project.yml — 无 governance_version / governance_fingerprint
mode: engineering
lifecycle: mvp
# 没有: governance_version: "1.0"
# 没有: governance_fingerprint: "sha256-xxxx"
```

### 未被已有分析覆盖的证明

- `docs/requirements/expansion-horizon-three.md` 方向四「治理资产升级管线」讨论的是**跨项目**的治理资产版本同步（forge-init 创建的项目如何接收上游治理更新）——这是**组织和分发**问题，不是**变更工作流**问题。

- `uncovered-frontiers-v25-systemic-boundaries.md` 方向三「Agent 自修改治理文件——信任边界的静默突破」已经诊断了**问题本身**（agent 可以修改治理文件），但没有提出修复方向——本文提出的 Governance-as-Code 流水线正是这个问题的**修复方案**。

- `strategic-extensions-v22-silent-failure-modes.md` 方向二「Governance-as-Code 的评审基础设施」提到了 governance diff 的概念，但那是作为一个**测试策略**（验证 governance 变更后的系统行为），不是作为一个**结构化的变更管理工作流**。

- **本文方向与上述所有分析的关系**：诊断问题（方向三 v25）→ 测试策略（方向二 v22）→ **变更工作流（本文）**。三者是互补关系：谁发现了问题、如何验证修复、如何管理变更生命周期。

### 建议方向

1. **Governance Change Proposal 模型**：引入 governance change proposal 概念：
   ```
   .forge/governance/
     proposals/
       001-relax-mode/
         plan.md        ← 变更内容描述
         diff.patch     ← 治理文件的 git diff
         status         ← pending | reviewing | approved | rejected | applied
         reviewed_by    ← reviewer agent 名称
         approved_at    ← 批准时间戳
         applied_at     ← 应用时间戳
   ```

2. **`forge governance propose`**：分析当前待提交的治理文件变更，生成 change proposal：
   - 展示变更范围（哪些文件变、新旧对比）
   - 验证变更语法（YAML 仍可解析？agent 卡契约仍完整？）
   - 评估变更影响（mode 变更 → gate-set 变化？workflow 变更 → 哪些 phase 受影响？）
   - 生成 proposal 文件

3. **`forge governance review`**：用独立 agent（fresh context）审查 governance change：
   - 审查者**不能**是实现变更的 agent（同 AGENTS.md 的 fresh-context 规则）
   - 审查输出：APPROVE / REQUEST_CHANGES / REJECT
   - 审查记录保存在 proposal 目录中

4. **`forge governance apply`**：批准后原子应用变更：
   - 创建 `.forge/governance/backups/<timestamp>/` 备份旧文件
   - 应用 patch 到治理文件
   - 运行 `forge validate --consistency` 验证变更后的完整性
   - 记录 `governance_applied` trace event

5. **治理变更的 CI 集成**：`.github/workflows/forge.yml` 增加 governance check job：
   - 检测治理文件的 PR 变更
   - 自动运行 `forge governance propose --dry-run` 生成影响报告
   - PR comment 自动贴 governance impact 摘要

### 边界情况

| 场景 | 风险 | 缓解 |
|------|------|----------|
| Agent 绕过 governance 流水线直接修改治理文件 | 治理流水线被旁路 | `forge doctor` 增加 `governanceIntegrityCheck`：比较治理文件 SHA 与上次 approved 的快照 |
| 紧急变更需要立即生效（如安全补丁） | Proposal 流程太慢 | `forge governance apply --emergency` 跳过 review，但记录紧急批准的理由 |
| 多个 proposal 冲突（同时改同意文件） | 冲突的 proposal 互相覆盖 | Proposal 基于 git 工作流：显式冲突检测 + 需要 rebase |
| Governance change 与正在运行的 evolve 冲突 | 变更在 evolve 中途生效 | `forge governance apply --at-phase-boundary` 等待当前 phase 完成再应用 |

---

## 方向三：Trace 事件分段与运行边界协议 —— 解决 O(n²) 数据膨胀和事件关联

**类型**: 可观测性 · 数据管理 · 调试  
**优先级**: P2（随运行次数增加 silently 恶化）  
**代码影响**: `internal/trace/` · `cmd/forge/` · `internal/doctor/`

### 现状

分析 `.forge/trace.jsonl` 的实际内容发现**严重的数据膨胀和结构缺失**：

**问题 1：doctor 事件 O(n²) 膨胀**

trace 文件中占最大比例的事件是重复的 doctor 检查：

```json
{"seq":1,"kind":"doctor","name":"checkpoint","status":"ok","detail":"readable"}
{"seq":2,"kind":"doctor","name":"trace","status":"ok","detail":"N events"}         ← 每次报告 "N events"
{"seq":3,"kind":"doctor","name":"memory","status":"ok","detail":"14 entries"}       ← 始终 14 entries
{"seq":4,"kind":"doctor","name":"preflight","status":"ok","detail":"quick doctor check complete"}
```

这些事件在每次 doctor 调用时写入，每次写入时 `detail` 中的 `N` **递增 4**（因为每次写入 4 个新事件）。这导致 **O(n²) 数据量增长**（第 N 次 doctor 写入的 trace 事件包含 `N events` 的计数值，Trace 事件数本身与 N 成正比）。

当前 trace 文件中有约 **95 个事件**，其中约 **80% 是重复的 doctor 事件**（相同的事件模式重复 20+ 次）。

**问题 2：无运行边界标识**

```json
// 第一批 evolve 的 trace (seq 1-2)
{"seq":1,"kind":"iteration","name":"1","status":"ok","duration_ms":0}
{"seq":2,"kind":"iteration","name":"2","status":"ok","duration_ms":0}
// 第二批 evolve 的 trace (seq 1-2 —— seq 重置了！)
{"seq":1,"kind":"iteration","name":"1","status":"ok","duration_ms":4469}
```

seq 计数器在每次 `forge run` 时重置，没有 run ID 来区分两次运行的 event 边界。trace 文件中不同 run 的事件**混在一起无法区分**。

**问题 3：无事件去重**

doctor 事件在每次调用时都写入完全相同的内容（只有 detail 的 N 递增），既无去重机制，也无采样策略。

### 代码级证据

```go
// forge-core/internal/trace/trace.go
type Event struct {
    Seq    int    `json:"seq"`    // 每次 newTracer() 从 1 开始
    Kind   string `json:"kind"`
    Name   string `json:"name"`
    Status string `json:"status"`
    Detail string `json:"detail,omitempty"`
    // ★ 没有 run_id, no session_id, no boundary marker
}

func (t *Tracer) Emit(ev Event) error {
    ev.Seq = t.nextSeq()  // 进程内原子递增
    // 写入 → 追加到文件
    // ★ 没有去重, 没有采样, 没有频率限制
}
```

```go
// forge-core/internal/trace/trace.go — newTracer()
func newTracer(w io.WriteCloser) *Tracer {
    return &Tracer{...}
    // 不写 run_started 边界事件, 不记录 run ID
}
```

```go
// forge-core/cmd/forge/main.go — doctor 调用
// doctor 写在 trace 中, 但每次开新 writer → seq 重置
// 而且多条 doctor 写入相同的事件模式
```

```go
// forge-core/internal/doctor/doctor.go
// Run() 返回 []Check
// 每个 Check 包含 Name + OK + Detail
// 但不包含「是否与前一次检查结果相同」的判断
// 每次运行时都重新计算和输出所有检查
```

### 未被已有分析覆盖的证明

- `docs/analysis/strategic-extensions-v23-systemic-gaps.md` 方向四「观测数据生命周期管理」讨论了 trace 轮转策略（按大小/行数轮转 trace 文件）——**文件级管理**（何时切文件），不是**事件级管理**（哪些事件值得写、如何分段）。

- `docs/analysis/strategic-extensions-v22-silent-failure-modes.md` 方向三「可观测管道无声故障」讨论了 trace writer 静默丢弃错误的问题——**错误路径**，不是**正常路径的数据管理**。

- `docs/requirements/high-value-extension-directions-v3.md` 方向一「收敛可靠性分层」使用 trace 事件做收敛分析，但从不质疑 trace 事件本身的完整性。

- `docs/analysis/edgecases-and-perf.md` 方向四「trace 写入负载」提到高频 trace 写入的性能影响，但那是假设**有意义的写入**，未被识别为**低价值事件的重复写入**。

- **本文方向问的是：trace 事件自身的数据质量。** 所有已有分析将 trace 视为**可靠的数据源**（讨论如何读、如何处理），没有人质疑 trace 中写入了大量低信息量重复事件。

### 建议方向

1. **Run 边界事件**：每次 `forge run` / `forge evolve` 开始时写入 `kind: "run_started"` 事件（携带 `run_id` UUID、`workflow`、`mode`、`lifecycle`）；结束时写入 `kind: "run_finished"` 事件（携带 `duration_ms`、`exit_code`）。trace reader 可按 run_id 筛选事件。

2. **事件去重与采样**：对于相同 `(Kind, Name, Status, Detail)` 的事件，在 T 时间段（如 60s）内只写入一次。`doctor` 检查每 5 分钟频率写入一次即可（而非每次 forge 命令都写）。突发变化（状态从 ok→fail）绕过采样优先写入。

3. **Doctor 增量检查**：`doctor` 改为只报告与前一次不同的检查结果。持续 ok 的检查每 5 次写入一次摘要（`"All checks pass (cached, last full scan at T-5m)"`），而非每次都全量写入。

4. **Seq 语义增强为 run_scoped**：seq 在当前语义下是「tracer 生命周期内递增」。改为 `run_seq` + `global_seq`：
   - `run_seq`：当前 run 内递增（当前行为）
   - `global_seq`：跨 run 递增（从 `trace.global_seq` 状态文件读取 / 持久化）
   - 这样 trace reader 可以通过 `global_seq` 的连续性检测事件是否丢失

5. **`forge trace --dedup` 修复命令**：对现有 trace 文件执行去重和修补 run 边界：
   - 检测 seq 重置 → 注入 `run_finished` / `run_started` 边界
   - 合并连续的重复事件
   - 输出修复后的 trace 文件

### 边界情况

| 场景 | 风险 | 缓解 |
|------|------|----------|
| 事件去重导致 key event 丢失 | 状态转变刚好在采样窗口内 | 采样只用于完全相同事件；Kind/Status 变化强制写入 |
| global_seq 状态文件损坏 | seq 从 0 重启 | 不影响运行时，`forge trace --repair` 可重建 |
| 高并发 run（CI 中多个并行 forge） | global_seq 竞争 | 使用原子文件锁或独立 seq tracker |

---

## 方向四：跨 Tier 一致性验证器（Cross-Model Consistency Validator）—— 用量化证据指导路由决策

**类型**: 质量 · 路由 · 成本优化  
**优先级**: P2（在组织级采用后成为关键）  
**代码影响**: `internal/routing/` · `new internal/quality/` · `cmd/forge/`

### 现状

ForgeOS 的模型路由策略（`internal/routing/routing.go`）基于**静态规则**：

```go
// routing.go
func TierFor(agent, mode string) string {
    // 按 agent 角色 + mode → 返回 tier 字符串
    // 完全是声明式规则，没有反馈回路
}
```

系统可以决定「planner 用 Sonnet, reviewer 用 Opus」，但**没有数据支撑这些决策**：

1. **不知道「如果 planner 用 Haiku 会发生什么」** —— 可能质量没差别（省 5× 成本），也可能降低 30% 的 plan 质量导致更多 loop-back
2. **不知道「这次 reviewer 的输出是否值得 Opus 的价格」** —— 可能是简单 CRUD 审查（Sonnet 足够），但系统硬编码了 Opus
3. **不知道「某个 agent 在某个任务类型上索要了更多的 loop-back 因为用了较便宜模型」** —— 无法做成本-质量权衡

当前唯一的质量回馈是 `scorecard.quality_score`，但恒 N/A（没有质量评估器），且与路由决策没有任何自动连接。

### 代码级证据

```go
// forge-core/internal/routing/routing.go — TierFor
// 纯声明式规则引擎：
//   agentTier map + modeDefault map + risk override + lifecycle floor
// 输出是 deterministic, pure function of (agent, mode, lifecycle, risk)
// 没有:
//   - 从 scorecard 读取历史表现
//   - shadow inference 结果
//   - empirical quality data
```

```go
// forge-core/internal/routing/scorecard.go
type Scorecard struct {
    Decisions []Decision `json:"decisions"`
    // 记录历史路由决策, 但不驱动未来决策
}
```

```go
// forge-core/cmd/forge/scorecard_wind.go
// scorecard 的 quality_score 恒 "N/A"
// 因为没有任何质量评估机制
```

```go
// forge-core/cmd/forge/route.go
// forge route — 手动路由决策工具
// --model 可以 override model tier
// 但不记录「这个 override 产生了什么质量结果」
```

### 未被已有分析覆盖的证明

- `docs/requirements/high-value-extension-directions.md` 方向一「多维模型路由自动化」讨论的是**路由决策的输入维度**（复杂度/依赖/上下文/业务影响/历史择优）——怎么决定用什么模型。本文方向讨论的是**验证决定的正确性**——用了之后怎么知道是对的还是错的。

- `docs/analysis/expansion-core-five.md` 方向二「统一验证引擎」讨论了 agent 输出的履约验证（输出是否符合契约），这是**输出质量**验证。本文方向讨论的是**模型选择质量**验证。

- `docs/requirements/expansion-production-readiness.md` 方向三「LLM 输出契约履约验证」同样聚焦**agent 输出**的质量，不是**路由决策**的质量。

- **本文方向填补的是路由决策的质量反馈闭环** —— 这是所有已有分析中缺失的一环：「路由层做决定 → 执行业 → 评估决定的质量 → 反馈给路由层」。

### 建议方向

1. **Shadow Run 机制**：对一定比例的 agent phase（例如 10%）同时用两个 tier 运行：
   - 主用 tier（如 Sonnet）的输出作为「生产路径」
   - Shadow tier（如 Opus）的输出作为「参考路径」（不写入磁盘、不影响 converge）
   - 比较两条路径的输出质量和成本

2. **输出质量代理指标**（非 LLM-as-judge，避免 rater drift）：
   - **Gate 首次通过率**：用 model_x 时 gate 在第一次尝试就 PASS 的比例
   - **Loop-back 归因**：用 model_x 后的 loop-back 是否因 agent 输出质量问题触发
   - **Agent 输出长度**：异常短（偷懒）或异常长（凑字数）的输出比例
   - **Verdict parse 成功率**：不同 model 的机读 token 解析成功率差异

3. **`forge route --audit`**：新增审计子命令，分析过去 N 次 run 的路由决策质量：
   ```
   $ forge route --audit --days 30
   Route Decision Quality Report (last 30 days):
   planner → Sonnet (45 calls, 78% first-pass gate pass rate)
   planner → Haiku  (12 calls, 42% first-pass gate pass rate, 3x more loop-backs)
     → RECOMMENDATION: keep planner on Sonnet (Haiku costs 50% less but causes 3x retries)
   
   reviewer → Opus (30 calls, 92% verdict accepted)
     → RECOMMENDATION: no lower-tier test available (Opus required by safety floor)
   
   implementer → Sonnet (80 calls, 71% first-pass)
   implementer → Haiku  (8 calls, 50% first-pass, only for "docs" phase)
     → RECOMMENDATION: Haiku acceptable for documentation-only implementations
   ```

4. **Shadow Run 的成本控制**：shadow run 只在满足以下条件时触发：
   - `lifecycle >= growth`（生产环境的项目）
   - `mode != explorer`（至少 balanced）
   - 全局采样率 ≤ 10%（控制额外成本）
   - 结果写入 trace event `kind: "quality_audit"`

### 边界情况

| 场景 | 风险 | 缓解 |
|------|------|----------|
| Shadow Opus 输出与 Sonnet 不同但同样 valid | 错误的判定 Sonnet 输出「质量差」 | 一致性验证器比较的是结构完整性（输出长度、章节完整性、verdict 可解析性），而非语义优劣 |
| Shadow run 增加 10% 的成本 | 年度预算超支 | 采样率可配置；仅在 `growth`+ 生命周期项目上激活；shadow run 不写磁盘不消耗下游成本 |
| 路由决策被 shadow run 数据「过度优化」 | 从静态规则变为过度拟合的数据驱动 | Shadow run 输出为**建议**（advisory），非**自动 override**。路由决策始终由声明式规则 + 安全底线保护 |
| 样本量不足（项目刚启动） | 数据不可靠导致误判 | `--audit` 在所有统计指标旁标注 `(n=X)`；n < 10 时不输出推荐意见 |

---

## 方向五：可组合 Skill Chains —— 从引用文档到可执行的多步技能管道

**类型**: 功能 · 编排 · 可复用性  
**优先级**: P2（随 skill 卡数量增长自动升值）  
**代码影响**: `internal/asset/` · `internal/orchestrator/` · `.agent/skills/` 约定

### 现状

ForgeOS 有 **9 个 skill 卡**（`.agent/skills/`），它们的功能仅停留在**引用文档**：

```
.agent/skills/
  ai-sdlc-review.md
  clean-architecture.md
  code-review.md
  cognitive-architecture.md
  modularization.md
  project-reorganization.md
  refactor-large-file.md
  security-review.md
  testing.md
```

Skills 当前的可消费方式：

| 方式 | 机制 | 现状 |
|------|------|------|
| Agent 卡引用 | agent 卡职责段声明 `uses_skills: [clean-architecture, testing]` | ✅ 已实现——`buildPrompt` 注入引用 |
| Workflow 声明 | `uses_template` 引用 skill | ✅ 已实现——模板渲染 |
| **Skill 组合** | **一个 skill 完成后执行另一个 skill** | **❌ 不存在** |
| **Skill 链式执行** | **agent 在 phase 内按序执行多个 skill** | **❌ 不存在** |
| **Skill 参数化** | **为 skill 提供输入参数** | **❌ 不存在** |

Skills 是**平面的引用文档**，不是**多维的可执行单元**。

#### 一个具体场景

当前 `workflow build.yml` 中，如果 implementer 需要同时执行 `refactor-large-file` + `testing` + `code-review`，agent 的 prompt 会包含所有三个 skill 卡的文本。但：

1. Agent 不知道执行顺序（先重构再测试？先测试再重构？）
2. Agent 不知道 skill 间的依赖关系（refactor 后是否需要重新测试？）
3. Agent 不知道 skill 的输出是什么（refactor 的结果是拆分的文件列表；testing 的结果是测试通过报告）
4. **没有形式化的方式让一个 skill 的输出成为另一个 skill 的输入**

### 代码级证据

```go
// forge-core/internal/asset/asset.go
type Phase struct {
    Agent           string   `json:"agent,omitempty"`
    UsesTemplate    string   `json:"uses_template,omitempty"`
    RequiresTools   []string `json:"requires_tools,omitempty"`
    // ★ 没有 UsesSkills []string 字段
    // ★ 没有 Skills []SkillSpec 字段
}
```

```go
// forge-core/cmd/forge/prompt_artifacts.go
// usesTemplateContext: 读模板文件 → 注入 prompt
// 没有 multi-step pipeline 的概念
```

```yaml
# .agent/agents/implementer.md
# 引用 skills 是通过散文的 "uses_skills:" 标记
# 没有参数化, 没有顺序, 没有条件执行
uses_skills: [clean-architecture, testing, refactor-large-file]
```

```go
// forge-core/cmd/forge/prompt_context.go
// buildPrompt 接受 skills string 参数
// 当前只拼接 skill 卡文本
// 没有 skill orchestration
```

### 未被已有分析覆盖的证明

- `docs/requirements/architectural-expansion-perspectives.md` 方向五「Skill 增强与复合工作流」讨论了 skill 概念本身（什么是 skill、如何声明），但那是关于**skill 的元模型**，不是**skill 之间的组合和顺序执行**。

- `docs/expansion-analysis-v2.md` 讨论了 skills 的「来源和作用」（源自 ForgeOS 架构原则，为 agent 提供深层知识），但仍然是**引用层面**（如何在 prompt 中注入 skill 卡），不是**执行层面**（如何把 skill 编排为多步管道）。

- `docs/requirements/high-value-extension-directions-v3.md` 方向五「Composable Skill Chains」（可组合技能链）的标题与本方向同名——但检查其实际内容，讨论的是**跨工作流的编排**（design → build → review 的 pipeline），不是**单个 phase 内的多 skill 顺序执行**。

- **Skill chain 与 workflow pipeline 的区别**：工作流管线编排的是**agent phase**（每个 phase 用完整 agent 角色卡）。Skill chain 编排的是**phase 内的多个技能步骤**（同一个 agent 依次执行多个 skill）。两者正交：一个 phase 可能执行一个复杂的 skill chain，chain 中每个 step 复用同一个 agent 上下文但有不同的技能专精。

### 建议方向

1. **Skill 格式升级**：skill 卡从纯 Markdown 升级为包含结构化的 metadata frontmatter：
   ```yaml
   ---
   name: refactor-large-file
   version: 1.0
   description: Split a file that exceeds the max_lines threshold into smaller modules
   input:
     - file_path: string  # 需要拆分的文件路径
     - max_lines: number  # 单文件行数上限
   output:
     - split_files: string[]  # 拆分后的文件列表
     - removed_lines: number  # 从原文件移除的行数
   steps:
     - name: analyze
       description: Analyze file structure and identify natural split points
     - name: split
       description: Create new modules and update imports
     - name: verify
       description: Run tests and gate checks
   dependencies:
     - clean-architecture  # 建议先了解 clean architecture 原则
   ---
   ```

2. **Phase 内 skill chain 语法**：workflow YAML 的 phase 支持 skill chain：
   ```yaml
   - name: implementer
     agent: implementer
     skills:                 # ← 新增：skill chain
       - refactor-large-file:
           file_path: "src/legacy/helper.go"
           max_lines: 200
       - testing:
           scope: "refactored files"
       - code-review:
           focus: ["modularization", "naming"]
     feeds_forward: [gate]
   ```

3. **Skill chain 执行引擎**（在 `buildPrompt` 中新增）：
   - 解析 phase 的 `skills` 声明
   - 为每个 skill 构建一段独立的 prompt 上下文（包含 skill 卡内容 + 输入参数）
   - 在同一条 agent 调用内按序嵌入所有 skill 指令
   - 每个 skill 的输出作为下一个 skill 的输入上下文
   - 如果一个 step 的输出不满足后续 step 的要求，诚实告警

4. **Skill 组合的可复用性**：skill chain 本身可以定义为新的复合 skill：
   ```yaml
   ---
   name: refactor-test-secure
   version: 1.0
   chain:
     - refactor-large-file
     - testing
     - security-review
   ---
   ```

### 边界情况

| 场景 | 风险 | 缓解 |
|------|------|----------|
| Skill chain 中某个 step 失败（agent 无法完成） | 后续 steps 基于不完整状态 | Step 可声明 `required: true`（失败→整个 chain 中止）或 `required: false`（跳过但注明） |
| 同一个 skill 被 chain 多次引用（先 refactor、再测试、再 refactor） | 重复执行可能退化 | **允许但罕见**；同一个 skill 在 chain 中多次出现的预期行为是独立的多次调用 |
| Skill 输入参数不满足声明类型 | Agent 行为不可预测 | `forge validate --workflows` 校验 skill 参数类型，不匹配时报 WARNING |
| Skill chain 过长 | Prompt 膨胀超过 context window | Skill chain 的 prompt 优先级低于角色卡/约束/Task；chain 超过 budget 时诚实截断并说明 |

---

## 优先级汇总

| # | 方向 | 类型 | 优先级 | 核心价值 | 代码规模 | 与已有分析重叠度 |
|---|------|------|--------|----------|----------|-----------------|
| 1 | **Model-Tier-Aware Context Budgeting** | 性能/成本优化 | **P1** | 相同 task 用不同模型时 prompt 长度自适应优化，直接影响 token 成本和 agent 输出质量 | 中（`buildPrompt` 参数扩展 + `context_budget` 配置 + trace 扩展） | **零** |
| 2 | **Governance-as-Code Review Pipeline** | 工程化/合规 | **P1** | 治理文件变更走结构化审查流水线，解决自修改治理文件信任盲区 | 中（`forge governance` 子命令 + proposal 模型 + 审计） | **零**（v25 方向三诊断问题未提方案，本文是修复方案） |
| 3 | **Trace 分段与运行边界** | 可观测性 | **P2** | 解决 O(n²) trace 膨胀和 run 边界丢失，使 trace 数据分析可靠 | 小（run_id + run 边界事件 + 去重 + 增量 doctor） | **零** |
| 4 | **Cross-Model Consistency Validator** | 质量/路由 | **P2** | 用量化证据回答「这个 tier 选对了吗」，关闭路由质量反馈闭环 | 中（shadow run + quality proxy metrics + `--audit`） | **零**（路由分析讨论怎么做决定，本文讨论决定是否正确） |
| 5 | **Composable Skill Chains** | 功能/编排 | **P2** | Skill 从引用文档升维为可执行、可组合的多步管道 | 中（skill 格式升级 + chain 语法 + 执行引擎） | **零**（标题与 v3 方向五相同，但内容完全不同——v3 是工作流管线，本文是 phase 内 skill 链） |

### 执行建议

```
短期（本 sprint）:
  方向三（Trace 分段）   —— run_id 和去重是最低成本高收益的可观测性改进
  方向一（Context Budget） —— prompt 构建参数扩展

1 sprint:
  方向四（一致性验证器）  —— shadow run + quality proxy 指标
  方向二（Governance Pipeline） —— `forge governance` 子命令 + proposal 模型

2+ sprints:
  方向五（Skill Chains） —— skill 格式升级 + chain 执行引擎
  方向二（CI 集成）      —— governance change 的 CI 自动验证
  方向四（自动路由优化） —— quality feedback → 路由策略更新
```

### 被排除的方向与理由

| 方向 | 排除理由 |
|------|----------|
| Agent 凭据注入（方向一 v30） | 已在 `genuinely-novel-expansion-directions.md` 方向一中完整覆盖 |
| 测试质量门禁（方向二 v30） | 已在 `genuinely-novel-expansion-directions.md` 方向二中完整覆盖 |
| 输出结构验证（方向三 v30） | 已在 `genuinely-novel-expansion-directions.md` 方向三中完整覆盖 |
| Prompt 效能度量（方向四 v30） | 已在 `genuinely-novel-expansion-directions.md` 方向四中完整覆盖 |
| 结构化输出协议（方向五 v30） | 已在 `genuinely-novel-expansion-directions.md` 方向五中完整覆盖 |
| 跨平台可移植性 | 已在 `uncovered-frontiers-v25.md` 方向四中完整覆盖 |
| 运行时自我修复 | 已在 `uncovered-frontiers-v25.md` 方向五中完整覆盖 |
| 数据生命周期管理 | 已在 `systemic-expansion-v26.md` 方向一中完整覆盖 |
| 错误分类与可操作性 | 已在 `systemic-expansion-v26.md` 方向四中完整覆盖 |
| 收敛信号可靠性分层 | 已在 `genuine-architectural-gaps-v28.md` 方向一中完整覆盖 |
| 治理资产热加载 | 已在 `genuine-architectural-gaps-v28.md` 方向二中完整覆盖 |
| 跨相位产物版本一致性 | 已在 `genuine-architectural-gaps-v28.md` 方向五中完整覆盖 |
| Agent 能力协商分配 | 已在 `genuine-architectural-gaps-v28.md` 方向四中完整覆盖 |
| Feed-forward 级联截断 | 已在 `strategic-extensions-v22.md` 方向一中完整覆盖 |
| YAML 双解析器交叉验证 | 已在 `strategic-extensions-v22.md` 方向二中完整覆盖 |
| Checkpoint PhaseIndex 漂移 | 已在 `strategic-extensions-v22.md` 方向四中完整覆盖 |
| 收敛定量语义 | 已在 `strategic-extensions-v23.md` 方向五中完整覆盖 |

---

*扫描基于 forge-core 全量源码（18 Go 包 / 195+ 源文件 / harness 39 模块 / pi-batch.py / examples / `.forge/` 运行时产物）*
*交叉验证 40+ 篇 `docs/analysis/*.md` + 13 篇 `docs/requirements/*.md` + `docs/FUNCTIONAL_REQUIREMENTS_AUDIT.md`*
*确认所有方向与已有 ~60 个扩展方向零重叠*
*生成日期: 2026-07-09 | 不含任何代码*
