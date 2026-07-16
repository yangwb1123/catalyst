# ForgeOS — 生产就绪的五个战略缺口

> **角色**: 资深架构师 / 产品经理  
> **方法**:  
> 1. 全仓逐文件深扫: forge-core（18+ Go 包 · ~45k LOC 总计）、harness（39+ 模块 · ~10.5k LOC）、  
>    `.agent/`（12 agent 卡 · 9 skill 卡 · 5 工作流 · 全部 policies/architecture/DECISIONS）、  
>    pi-batch.py、examples/（url-shortener · go-taskd）  
> 2. 逐篇通读已有分析: **全部 75+ 份 `docs/` 分析文档**（39+ `requirements/*.md` + 41+ `analysis/*.md` +  
>    核心文档 BOOTSTRAP / CURRENT_SPRINT / ROADMAP / FUNCTIONAL_REQUIREMENTS_AUDIT / ADR 0001-0004 /  
>    DECISIONS / loop-engineering / north-star / ha-security-rollout / ignition）— 合计 **~120+ 已有方向**  
> 3. **差异化证明**: 对每个方向用 `grep -rn` 在 75+ 份已有分析文档中验证核心关键词，  
>    确认该方向**从未作为独立方向展开**（最多作为其他方向的边缘子段落）  
> 4. **视角**: 不从「加什么新引擎」出发，而从「把 ForgeOS 从可运行的原型推向可信赖的生产治理平台」出发——  
>    关注的是长期运行、跨项目、跨环境的治理成熟度  
> 5. **纪律**: 不编写任何代码。每个方向附代码级证据、边界场景、与已有覆盖的差异化证明。  
> **日期**: 2026-07-10

---

## 全景：已有 ~120+ 扩展方向覆盖图

已有分析压倒性地覆盖了以下域（本文的 5 个方向全部落在这张图的**白色区域**）：

| 已被充分覆盖的域 | 覆盖量 | 代表性文档 |
|---|---|---|
| 引擎补齐（编排/路由/记忆/收敛/信号/诊断/并行/wave/loop-back/自适应装配） | ~25 方向 | 大部分 requirements + analysis |
| 跨项目/跨会话/联邦治理（知识迁移/漂移检测/多仓库编排/事件驱动/定时平面） | ~15 方向 | `novel-five-perspectives.md` · `expansion-horizon-three.md` |
| 生产可靠性（Prompt QA / 信号硬化 / 环境验证 / 自愈层 / 健康契约 / 多级熔断） | ~15 方向 | `expansion-production-readiness.md` · `novel-five-frontiers-v34.md` |
| 执行语义形式化（原子性/幂等/因果一致性/回滚/版本演化/session/事务性执行） | ~12 方向 | `execution-semantic-gaps.md` · `v33.md` |
| 系统边界盲区（级联截断/YAML 分歧/信任边界/跨进程锁/超时覆盖/并行安全） | ~15 方向 | `forgotten-five-system-boundaries.md` · `v25.md` · `v38.md` |
| 二阶伴生问题（知识衰减/配置爆炸/TOCTOU/无声数据丢失/数据生命周期） | ~12 方向 | `second-order-architectural-gaps.md` · `v26.md` |
| 收敛方法论/自诊断/停滞检测/治理测试/元治理 | ~10 方向 | `novel-five-perspectives.md` · `loop-engineering.md` |
| API 版本化/Schema 契约/产物格式/跨会话学习/RAG/自免疫测试 | ~10 方向 | `production-hardening-five-v42.md` · `structural-gaps-v41.md` |
| 安全/凭据/SCA/沙箱/注入防御/readonly 强制 | ~8 方向 | `security-review.md` · Sprint 31 |
| **总计已有覆盖** | **~120+ 方向** | **通过 ~75 份独立文档阐述** |

**本文的 5 个方向共同特征**: 不是「新引擎」「新架构层」或「已有方向的变体」，而是  
**当 ForgeOS 从单项目原型扩展到多项目生产部署时必然暴露的缺口**。每个方向在已有 ~120 个方向中  
**零独立覆盖**（最多为其他方向的边缘段落）。

---

## 方向一 · 生产事故响应工作流（Hotfix / Rollback / Post-mortem）

**类型**: 工作流 · 可靠性 | **优先级**: 🔴 P0（生产中必遇）  
**影响范围**: `.agent/workflows/`（新 workflow）· `internal/orchestrator/loop.go` · `internal/converge/converge.go`  
**代码证据**: 全仓零 hotfix/rollback 工作流 | **搜索验证**: 0 篇已有文档独立覆盖

### 现状

ForgeOS 的脊柱工作流（Discover→Design→Review→Build→Evolve）全是**前向推进型**：

```yaml
# 现有五个 workflow 的方向
discover.yml  →  从空白到需求
design.yml    →  从需求到架构
review.yml    →  从架构到批准
build.yml     →  从批准到实现
evolve.yml    →  从实现到持续改进
```

所有工作流假设**你有完整的时间走完治理流程**。当生产系统出问题时：

**证据 A: 没有"紧急 bypass"概念**

```yaml
# 所有 workflow 的 mode 维度:
#   explorer → 跳过评审、快速推进
#   balanced → 标准流程
#   engineering → 全闸门
#   cto → 只出文档
#
# 但没有任何 mode / lifecycle 代表"生产事故，需立即修复"。
# explorer 最接近（跳评审），但它没记录"为什么跳"；
# 事故修复需要事后约束：修复完成后必须补 post-mortem。
```

**证据 B: 没有回滚工作流**

```yaml
# evolve.yml 是持续前向推进，没有"回滚到已知好的版本"的概念。
# 当前环境:
#   git revert + git push → CI 触发 gate → gate 可能因 revert 后的
#   新差异而 FAIL（因为 revert 产生了新的 diff）
#   没有 forge rollback 命令来声明"这是回滚，请放行已知好的制品"
```

**证据 C: 没有 post-mortem 工作流**

```yaml
# 事故修复后，没有工作流要求:
#   - 记录事故时间线
#   - 分析根因（5 Whys）
#   - 生成预防措施任务
#   - 更新 runbook
# 所有知识停留在工程师脑子里，下次事故可能重犯
```

**证据 D: 没有"紧急闸门降级"的记录**

```go
// mode_gating.go — mode 决定 gate-set
// 如果 SRE 为修复生产事故而临时将 lifecycle 从 production 改为 mvp
// 以绕过严格的 production gate，当前没有任何审计记录:
//   - 谁在什么时间改了 lifecycle
//   - 为什么改
//   - 什么时候改回来
```

### 为什么需要它

ForgeOS 承诺「24h 无人值守软件工厂」。但生产事故是必然事件——不是「是否发生」的问题，而是「何时发生」的问题。没有事故响应工作流：

- SRE 会**绕过** ForgeOS 做紧急修复（手动改代码、手动部署），破坏治理一致性
- 事故修复后学到的教训**丢失**，下次同类事故再犯
- 紧急 bypass 没有审计线索，合规审计时无法证明"事故期间谁做了什么、为什么"
- 治理系统在压力最大的时刻（生产故障）被**主动规避**，信任被侵蚀

### 建议方向

1. **新 workflow `incident-response.yml`**:
   - 包含相位: `triage`（影响评估）→ `hotfix`（紧急修复，跳过评审但记录原因）→ `gate-lite`（最少必需闸门）→ `deploy` → `post-mortem`（48h 内必须完成）
   - `mode: incident` 新 mode：约束最小 gate-set、强制记录 bypass 理由、强制排期 post-mortem
   - post-mortem 产出自动派生出 evolve 循环的 gap 任务

2. **`forge rollback <target>` 子命令**:
   - 结合 git 操作 + 治理覆盖：回滚代码 + 标记对应 ROADMAP 条目为 rolled-back
   - 避免 ci gate 因 revert diff 而错误 FAIL
   - 生成 `docs/incidents/rollback-YYYY-MM-DD.md` 记录回滚原因和审批

3. **`lifecycle: incident` 临时状态**:
   - 在 `project.yml` 中允许 `lifecycle: incident`（临时覆盖 production）
   - 自动设置 48h TTL（到期自动恢复原 lifecycle）
   - 所有 gate bypass 记录在 trace 中，`forge doctor` 可报告"未关闭的事故状态"
   - 派生补 post-mortem 任务到 ROADMAP，未完成则 `forge status` 告警

4. **紧急 bypass audit trail**:
   - 每次 gate 降级/skip 记录 `who`（`$USER` 或 `--reason`）、`why`、`when`、`expected_duration`
   - 输入到 `docs/incidents/` 下的审计文件
   - `forge doctor --audit-bypass` 报告当前所有活跃 bypass

### 边界场景

| 场景 | 当前行为 | 应然行为 |
|------|---------|---------|
| P0 生产宕机，需 5 分钟内修复 | SRE 手动改代码、手动部署、绕过 forge | `forge run incident-response --mode=incident --bypass="P0 severity: payment gateway down"` |
| hotfix 部署后发现引入回归 | 手动 revert，ci gate 因 revert diff 而 FAIL | `forge rollback --revision=abc123 --reason="hotfix introduced regression"` → gate 放行 revert-only diff |
| 事故修复后团队忘记写 post-mortem | 无记录，下次同类事故再犯 | `forge status` 显示 "INCIDENT: post-mortem pending (due 2026-07-12)" |
| SRE 上周改了 lifecycle 但忘了改回来 | gate 松了一周未被发现 | `forge doctor --audit-bypass` 报告 "lifecycle=incident set 7 days ago (TTL expired)" |
| 紧急 fix 没有经过 security-review | 安全漏洞被引入 | incident mode 记录 bypass 理由，post-mortem 强制补安全评审 |

### 差异化证明

- `high-value-expansion-directions.md` 方向一提到 hotfix 场景，但聚焦于 **resume 配置漂移**（A 跑了 evolve → B 改了 project.yml → A resume 用了错误 lifecycle），不是**生产事故响应工作流**。该文档的 hotfix 用例是作为配置漂移的例子，不是作为独立方向展开。
- 所有已有分析中「工作流」讨论均假设**正常开发循环**。没有任何文档提出「事故 bypass」「回滚工作流」或「post-mortem 强制」。这是 ForgeOS 工作流族中唯一的空白阶段。

---

## 方向二 · Context Window 作为一等资源管理

**类型**: 资源管理 · 可靠性 | **优先级**: 🟠 P1（长运行演化循环必然会遇到）  
**影响范围**: `cmd/forge/prompt_context.go` · `cmd/forge/prompt_memory.go` · `cmd/forge/prompt_artifacts.go` ·  
`internal/prompt/cache.go` · `internal/routing/routing.go` | **代码证据**: 全仓零 token 预算逻辑  
**搜索验证**: 12 篇已有文档浅层提及，**0 篇**作为独立方向展开

### 现状

ForgeOS 对「资源」有精细的预算管理体系，但只覆盖了两个维度：

| 资源维度 | 守卫机制 | 对应代码 |
|---------|---------|---------|
| **Agent 调用次数** | `Engine.checkAgentBudget` | `internal/orchestrator/budget.go` |
| **运行预算（美元）** | `Engine.checkRunBudget` / `BudgetExhausted` | `internal/orchestrator/budget.go` |
| **进程递归深度** | `CommandExecutor.MaxDepth` | `internal/orchestrator/command_executor.go` |
| **子进程输出大小** | `CommandExecutor.MaxOutputBytes` | `internal/orchestrator/command_executor.go` |
| **Context Window（token）** | ❌ **不存在** | — |

**证据 A: prompt 构建没有 token 计数**

```go
// prompt_context.go:70-80 — Build 接收 Workflow + Phase，组装 prompt
// 当前组装过程:
//   1. 加载 ADR 标题集（cache.go）
//   2. 加载 AGENTS.md 硬约束
//   3. 加载 agent 卡全文
//   4. 加载 ROADMAP 当前状态
//   5. 加载 memory 条目（prompt_memory.go）
//   6. 加载 gate 裁决历史（gateLedger）
//   7. 加载评审发现（reviewFindingsLedger）
//   8. 加载前序 phase 输出（phaseOutputLedger）
//
// 没有任何一步问过: "现在 prompt 已经多大了？"
// 没有任何一步检查过: "目标模型的 context window 是多少？"
// 没有任何降级策略: "如果 prompt 太长，优先保留哪些 lane？"
```

**证据 B: cache.go 明确承认 token 问题**

```go
// cache.go:16-23
// ★ HONESTY — what v1 buys, and what it does NOT ★
//   - It saves LOCAL I/O only: a readdir of docs/adr + a firstHeading read per ADR...
//   - It does NOT save a single claude token. The prompt TEXT is byte-for-byte
//     the same as the uncached path...
//   - The REAL token saving is v2 work: wiring the claude API's prompt-caching
```

**证据 C: memory 有硬编码 cap 但无 token 感知**

```go
// prompt_memory.go:48
const memoryCap = 32  // 硬编码条目上限
```

`memoryCap = 32` 是凭经验猜测的「安全值」，不是基于 token 预算计算出来的。如果每条 memory 平均 200 token，32 条 = 6,400 token；但如果每条 500 token，32 条 = 16,000 token——差距 2.5 倍，全凭运气。

**证据 D: 不同模型的 context window 差异巨大**

```go
// routing.go — TierFor 返回 haiku/sonnet/opus，但不返回 context window 大小
// Claude 3 Haiku:   48k token
// Claude 3 Sonnet:  200k token
// Claude 3 Opus:    200k token
// Claude 4 Opus:    200k token
// 同一个 prompt 在 Haiku 上可能占 90% window，在 Opus 上只占 5%
// 但 forge 构建 prompt 时不区分 target model
```

### 为什么需要它

随着 sprint 27-31 添加越来越多的 prompt lane（gate 裁决历史、评审发现、前序输出、memory 条目），prompt 长度在持续增长。不管理的后果：

1. **长 prompt 吞噬 token 预算**：一次 agent 调用烧掉更多 token，`--run-budget-usd` 更快耗尽
2. **模型质量下降**：接近 context window 上限时，模型 recall 质量下降（"lost in the middle"），增加自动决策风险
3. **跨模型路由不安全**：当前强制 Opus 用于评审，但如果某天 budget 守卫将 reviewer 降级为 Sonnet，同样的 prompt 可能在 Sonnet 上爆 window
4. **memory 增长与 prompt 长度正相关**：`Compact` 有 keepPerKind 控制条目数，但无 token 预算控制——即使用了 20 条 memory，如果每条很长，prompt 仍然膨胀

### 建议方向

1. **Token 估算器**：`internal/prompt` 包增加 `EstimateTokens(text string) int`——轻量启发式（如 `len(text)/4` 英文近似）。不需要精确计数（精确计数需要 tokenizer，会增加依赖），但需要有预算级误差（< 20%）

2. **Context Budget 结构**：每个 phase 的 `BuildPrompt` 接收一个 `ContextBudget`：
   ```go
   type ContextBudget struct {
       ModelWindow  int // 模型 context window 大小（token）
       TargetUsage  int // 目标使用量（如 70% of window）
       CurrentUsage int // 当前已使用
   }
   ```

3. **降级策略（lane priority）**：显式定义每个 prompt lane 的优先级：
   - `MANDATORY`（task、gate 裁决）——永远包含
   - `HIGH`（hard constraints、agent card）——尽量包含，预算紧张时优先保留
   - `MEDIUM`（memory entries）——预算紧张时按 relevance 排序截断
   - `LOW`（历史前序输出）——预算紧张时摘要化或跳过
   - `OPTIONAL`（详细 ADR 文本）——预算紧张时用标题替代

4. **Context Budget 预警**：`forge preflight` 增加 context window 检查——报告当前 model 的 window 容量 vs 估计 prompt 大小。`forge status` 报告每个 phase 的 token 消耗

5. **Window-aware routing**：`routing.TierFor` 增加 context window 维度——当 prompt 估计超过 Sonnet window 的 80% 时，强制升 Opus（更大的 window），即使 agent 的 tier 是 Sonnet

### 边界场景

| 场景 | 当前行为 | 应然行为 |
|------|---------|---------|
| 24h evolve 积累 200 条 memory（虽被 compact 但仍有~60 条+摘要） | prompt 变长但无人知晓 | token 预算预警，自动截断 low-priority lane |
| implementer 在 Sonnet 上跑，prompt 含大量前序输出 | Sonnet 200k window，可能仍安全 | context budget 检查确保 < 80% window |
| reviewer（Opus）的 prompt 因 gate 裁决历史过多而接近 200k | 模型 recall 质量下降 | 自动摘要 gate 历史，或仅保留最近 3 轮 |
| 用户向 Haiku 路由的快速 phase 中意外塞了超大 memory | Haiku 48k window 爆掉 | token 估算器在 phase 启动前预警，自动降级 memory lane |
| 两个 prompt lane 各自合理，但加起来爆 window | 单个 lane 检查不出问题 | 总预算汇总检查 |

### 差异化证明

- 12 篇已有文档**浅层提及** context window 问题，但全部是其他方向的子段落（"当心 prompt 太长"），**没有一篇**提出资源预算、优先级降级、window-aware routing 的完整方案
- `second-order-architectural-gaps.md` 方向一「知识衰减」提到提示过长会导致 agent 忽视重要上下文，但聚焦于**语义层面**（agent 漏读），**不是 token 预算层面**（prompt 物理长度）
- `expansion-production-readiness.md` 方向三「Prompt QA」聚焦于 prompt 质量（是否引起幻觉）、不是 prompt 长度资源管理
- 本文方向二的独特定位：将 context window 从「已知但忽略的问题」提升为**与 agent-call budget、run budget 并列的一等管理资源**

---

## 方向三 · Prompt 内容可复现性（版本锁定与回归检测）

**类型**: 可观测性 · 可靠性 | **优先级**: 🟠 P1（AI 输出可审计性的前提）  
**影响范围**: `cmd/forge/prompt_context.go` · `cmd/forge/prompt_artifacts.go` · `cmd/forge/prompt_memory.go` ·  
`internal/prompt/retrieve.go` · `internal/trace/trace.go` | **代码证据**: 全仓零 prompt 版本标签  
**搜索验证**: 5 篇已有文档提及类似概念，**0 篇**作为独立方向展开

### 现状

ForgeOS 的 prompt 由**多个 lane 的动态组装**产生，但没有任何机制追踪"给 agent 的 prompt 长什么样"：

**证据 A: prompt 是 ephemeral 的——构建完即丢弃**

```go
// prompt_context.go — buildPrompt
// 构建 prompt → 发给 executor → executor 返回输出 → prompt 对象丢弃
// 没有任何地方将 prompt 的摘要/哈希写入 trace、checkpoint 或日志
func (e Engine) buildPrompt(wf asset.Workflow, p asset.Phase, mode string) string {
    // 1. 系统指令（agent card + AGENTS.md + phases_on_deck + ...）
    // 2. 任务（ROADMAP 项 + 约束 + 前序输出）
    // 3. 知识（ADR + memory + gate 结果）
    // 4. 输出格式要求（machine-readable verdict 契约）
    // → prompt 构建完，发送，然后丢弃
}
```

**证据 B: 无法回答"这次和上次的 prompt 有什么不同？"**

```
迭代 1: gate 裁决 = {test: PASS, lint: PASS}
记忆: [kind=lesson "testing config", kind=decision "use pg"]
→ prompt 版本 A

迭代 2: gate 裁决 = {test: FAIL, lint: PASS}
记忆: [kind=lesson "testing config", kind=decision "use pg", kind=gap "missing mock"]
→ prompt 版本 B（gate 裁决不同 + 记忆多一条）
```

当 agent 在迭代 1 表现良好，在迭代 2 表现异常，当前无法判断是 prompt 变化导致的还是 agent 自身的问题。**prompt 没有指纹**。

**证据 C: 没有"prompt 回归测试"概念**

```go
// 当前测试覆盖:
// - prompt_context_test.go: 测试 lane 组装逻辑（硬编码任务/约束）
// - prompt_memory_test.go: 测试 memory 注入格式
// - prompt_artifacts_test.go: 测试 artifact 引用解析
//
// 没有测试: 给定真实输入（ADR + AGENTS + ROADMAP + gate 结果），
// 产出的 prompt 是否与预期一致（黄金文件测试）
```

**证据 D: trace 事件不含 prompt 信息**

```go
// trace.go — Event 结构
type Event struct {
    Format     string // "forgeos.trace.v1"
    Seq        int    // 单调递增
    Kind       string // "iteration" | "agent" | "gate" | ...
    Name       string // phase/gate name
    Status     string // PASS|FAIL|ok|...
    DurationMs int64
    CostUsdMicros int64
    Model      string // 模型名称
    Detail     string // 自由文本
    // ⚠ 没有 PromptHash string
    // ⚠ 没有 PromptLaneCount int
    // ⚠ 没有 PromptLength  int (tokens)
}
```

### 为什么需要它

ForgeOS 的治理质量直接依赖于 prompt 质量。当 `VERDICT: REQUEST_CHANGES` 或 `CONFIDENCE: 95` 出现时，审计者需要知道"这个裁决是在什么提示信息下做出的"。没有 prompt 可复现性：

1. **无法重现 AI 行为**：某次评审裁决是 `APPROVE`，另一次是 `REDESIGN`——根本原因可能是提示信息不同，而非代码质量不同
2. **无法检测回归**：prompt 格式重构（如 sprint 27-31 多次拆分 prompt_context.go）可能静默改变 agent 行为，但无任何检测
3. **无法比较**：不同 mode/phase 的 prompt 差异巨大，但无法做 A/B 比较
4. **审计断层**：trace 记录 agent 输出了什么，但不记录 agent 被喂了什么——缺失了审计线索的一半

### 建议方向

1. **Prompt 指纹（hash）**：每个 phase 的 prompt 构建后，计算 `SHA256(prompt)`，写入 trace event 的 `detail` 字段或新增 `prompt_hash` 字段。`forge status --trace` 可显示每个 phase 的 prompt 指纹。

2. **Lane-level version stamp**：每个 prompt lane 输出前附加 lane 版本（如 `# LANE: memory (v2, compact=true, count=7)`），使 prompt 的结构变化在审计时可见。

3. **Golden prompt snapshot**：`forge validate --prompt <workflow> <phase>`——不执行 agent，仅输出构建后的 prompt 全文并可选保存到文件。用于 CI 中检查 prompt 结构变化：
   ```bash
   # CI 步骤
   forge validate --prompt build.yml implementer --out .forge/prompts/implementer-latest.txt
   diff .forge/prompts/implementer-golden.txt .forge/prompts/implementer-latest.txt
   ```

4. **Prompt diff 检测**：在 trace 分析阶段自动比较同类型 phase 的 prompt hash 变化。当 hash 序列出现断裂时，标记为"prompt 结构变更"事件。

5. **Prompt 产物的数据流审计**：在 `phaseOutputLedger` 中不仅记录前序 phase 的输出，还记录**输出对应的 prompt 指纹**。使得后续 phase 可以追溯"这个输出是基于什么 prompt 产生的"。

### 边界场景

| 场景 | 当前行为 | 应然行为 |
|------|---------|---------|
| sprint 27 重构 prompt_context.go 后，agent 行为异常 | 无法判断是 prompt 变化还是 agent 自身问题 | 检测到 prompt hash 变化，在 forge status 中告警 |
| 相同 git commit 在不同时间跑出不同评审结果 | 原因不明 | 对比两侧 trace 的 prompt hash——如相同则问题在 agent 端，如不同则问题在 prompt 组装 |
| 用户想验证"新的 memory 注入方式是否改变了 agent 行为" | 无法做 A/B 对比 | golden prompt snapshot + diff |
| QA 评审报告说"代码质量差"，但实际是 prompt 缺乏必要上下文 | 无法区分 | prompt hash 记录显示缺失 ADR 注入 → 定位到 cache 层问题 |

### 差异化证明

- `expansion-directions-v14-operational-trust.md` 方向四「记分卡校准」提到附加 `prompt_version` 标签到记分卡——但那是在**路由层面**用于多维择优（不同 prompt 版本下同一 model 的 quality score 分开统计），不是**prompt 可复现性框架**。路由层面的标签不解决"审计需要重建 prompt"的问题。
- `second-order-architectural-gaps.md` 方向三「提示衰减」讨论了知识随时间衰减导致 agent 输出质量下降，但聚焦于**memory 内容层面**（旧知识覆盖新事实），不是**prompt 结构可复现性**。
- 本文方向三的独特价值：将 prompt 从「运行时 ephemeral 产物」提升为「可审计、可回归、可版本对比的一等治理工件」。

---

## 方向四 · Dry-Run 语义完备性差距（Dry-Run vs Real-Run 行为鸿沟）

**类型**: 测试 · 工程化 | **优先级**: 🟠 P1（无真的 agent 无法验证工作流完整性的根本原因）  
**影响范围**: `internal/orchestrator/executor.go`（DryRunExecutor）· `internal/orchestrator/loop.go`（LoopEngine）·  
`internal/converge/converge.go` · `cmd/forge/evolve.go` | **代码证据**: DryRunExecutor 返回空输出  
**搜索验证**: 3 篇已有文档浅层提及回放能力，**0 篇**分析 dry-run vs real-run 语义鸿沟

### 现状

`forge run --executor dry` 是 ForgeOS 的默认模式——它叙述工作流会做什么而不真正执行 agent phase。

**证据 A: DryRunExecutor 产出的 agent 输出为空**

```go
// executor.go — DryRunExecutor 的 runAgentPhase
// 调用 Build() 生成 argv，输出 "would run: <argv>" 到日志
// 但返回的 output 是空字符串
//
// 后果:
//   - NOT MET ← 因为 converge 需要 review_status == "approved"
//     但 review phase 的 DryRun 没有产出 VERDICT 行
//   - CONFIDENCE: <N> ← product-manager 的输出为空
//     导致 requirementConfidence 永远是 0
type DryRunExecutor struct {
    Build func(p asset.Phase, mode string) []string
    Log   func(string)
}

func (d DryRunExecutor) Execute(p asset.Phase, mode string) (output string, err error) {
    argv := d.Build(p, mode)
    log := fmt.Sprintf("[dry-run] would run: %s", strings.Join(argv, " "))
    if d.Log != nil {
        d.Log(log)
    }
    return "", nil  // ← 输出为空，任何依赖 agent 输出的下游全部断裂
}
```

**证据 B: Gate phase 在 dry-run 下真实执行**

```go
// orchestrator.go — runAgentPhase
// 当 phase 是 gate type 时，gate 调用真实 harness（gate.mjs/check.py/...）
// 所以 gate 的 PASS/FAIL 在 dry-run 下是真实结果
```

这就产生了一个危险的差距：

```
dry-run 下:
  - gate phase → 真实执行（可 FAIL）
  - review phase → 无输出（VERDICT 永远丢失）
  
结果：dry-run review 永远 review_status="" → 永远 NOT MET
但 gate 可能是 green 的 → 用户看到 "gates green, convergence NOT MET"
并错误地认为"系统工作正常，只是收敛条件没满足" (实际上 review phase 从未产数据)
```

**证据 C: converge 在 dry-run 下不可测试**

```
dry-run review:
  review_status = ""  (非 approved)
  evalReviewStatus → false
  convergence → NOT MET

real review:
  review_status = "approved" (agent 输出了 VERDICT)
  evalReviewStatus → true
  convergence → MET

差别：dry-run 的收敛判定与 real-run 完全不同，无法用 dry-run 验证
      工作流的收敛逻辑是否正确。
```

**证据 D: loop-back 在 dry-run 下不可观察**

```go
// loop.go — LoopEngine.Loop
// 当 review phase 输出 VERDICT: REQUEST_CHANGES 时
// AgentVerdict 检测到该信号，触发 loop_back → implementer
//
// 但在 dry-run 下：
//   - review phase 无输出 → AgentVerdict 返回 ""
//   - 不触发 loop_back
//   - 工作流继续前进到 qa，而不是回到 implementer
//   → dry-run 的流控制路径与 real-run 完全不同
```

### 为什么需要它

ForgeOS 的 CLI 默认是 dry-run。用户通过 dry-run 了解 forge 会做什么。但如果 dry-run 的行为与 real-run 有系统性差异：

1. **用户信任被侵蚀**：dry-run 说"会跑 review"，但不告诉你 review 会不会产出 APPROVE——收敛判定在 dry-run 下永远是 NOT MET，用户无法预判 real-run 能否收敛
2. **CI 中无法验证工作流完整性**：CI 不会花真钱跑 agent，所以 CI 只能验证 gate 结果，无法验证收敛逻辑（review_status、loop-back、confidence 等）
3. **学习成本高**：新用户跑 dry-run，看到 NOT MET，不知道这是因为 dry-run 的固有限制还是工作流配置出了问题
4. **开发者无法做 TDD**：编写新工作流后，无法用 dry-run 验证"如果 reviewer 输出 REDESIGN，流程是否回退到 security-review"

### 建议方向

1. **FakeAgentExecutor**：新增执行器类型，接收一个脚本或固定回复映射：
   ```yaml
   # workflow 测试用例
   phases:
     security-review:
       verdict: VERDICT: APPROVE
     performance-review:
       verdict: VERDICT: REQUEST_CHANGES
   ```
   与 `forge validate --state-machine`（novel-five-perspectives-2026-07-10-deep.md 方向四）配合，允许用户在不花钱的情况下验证所有状态机路径。

2. **`forge simulate <workflow> --case <fixture>`**：基于 FakeAgentExecutor 的完整工作流模拟器。加载预定义的 agent 输出 fixture，跑完整的工作流（含 loop-back 和收敛判定），输出最终状态。

3. **Dry-Run 诚实性报告**：`forge run --executor dry` 的输出末尾增加一段诚实性声明：
   ```
   [HONESTY] dry-run convergence reflects GATE PHASES only.
            Agent phase outputs (verdicts, confidence scores) are simulated
            as empty → review_status is always "" and convergence will
            always report NOT MET if agent outputs are required.
            Use `forge simulate --case <fixture>` for end-to-end validation.
   ```

4. **收敛的二元诚实分类**：在 dry-run 报告中，区分「gate 收敛」（gate 全绿）和「agent 收敛」（含裁决信号），让用户明确知道什么在 dry-run 下被验证了什么没有。

5. **`forge diff --executor dry --executor command`**：比较同工作流在 dry-run 和 real-run 下的行为差异——揭示 dry-run 低估了什么、高估了什么，量化鸿沟。

### 边界场景

| 场景 | 当前行为 | 应然行为 |
|------|---------|---------|
| 新用户初次试用，跑 `forge run build --executor dry` | 看到收敛 NOT MET，以为是工作流配置错误 | 诚实报告 dry-run 的局限性 + 提供 simulate 路径 |
| CI 中验证 review.yml 的 REDESIGN 路径 | 无法验证（dry-run 不到 REDESIGN） | `forge simulate review.yml --case redesign.json` |
| 开发者新增一个 workflow，想测试所有 loop-back 路径 | 需要一个真 agent 来触发每种输出 | `forge simulate --all-cases` 在几秒内穷举所有 agent 输出场景 |
| 用户写了复杂的 `on_fail` 迁移逻辑 | 无法通过 dry-run 预检 | simulate 显示状态机完整路径+预期迁移 |
| forge 升级后，旧工作流的行为是否不变 | 只能用真 agent 跑一遍 | simulate + fixture 库在 CI 中验证回归 |

### 差异化证明

- `seventh-wave-data-realism.md` 方向一「真数据轨迹回放」讨论了**捕获真实运行轨迹并回放验证**（通过真 agent 跑一次，保存轨迹，后续用轨迹验证），不是**dry-run 与 real-run 的语义差距分析**。回放的前提是有一次真 agent 运行，而 dry-run 差距分析的目标是在**没有**真 agent 运行的情况下验证工作流正确性。
- 3 篇已有文档提及「模拟 agent 输出」但全部是其他方向的子想法：`expansion-production-readiness.md` 作为「生产就绪检查清单」的子项，`novel-five-frontiers-v34.md` 作为「离线工作流测试」的子想法。**没有一篇**将 dry-run 语义差距本身作为独立方向。
- 本文方向四的独特价值：首次将 dry-run 从「叙述者」审视为「与 real-run 有系统性行为鸿沟的测试替身」，并提出量化、弥合、诚实披露这一鸿沟的完整方案。

---

## 方向五 · 跨运行工作流行为分析（Trace 到 Insight）

**类型**: 可观测性 · 分析 | **优先级**: 🟠 P1（trace 数据已存在但从未被聚合分析）  
**影响范围**: `forge-core/internal/trace/trace.go` · `internal/orchestrator/loop.go`（OnIteration）·  
`.forge/trace.jsonl` · `internal/persist/checkpoint.go` | **代码证据**: 零跨运行分析逻辑  
**搜索验证**: 0 篇已有文档独立覆盖

### 现状

ForgeOS 有一个高质量的 trace 系统——每个 agent phase、gate 裁决、收敛检查都记录为结构化的 JSONL 事件：

```jsonl
{"seq":1,"kind":"iteration","name":"evolve","iteration":1,"duration_ms":15000}
{"seq":2,"kind":"agent","name":"implementer","duration_ms":32000,"cost_usd_micros":54403,"model":"sonnet"}
{"seq":3,"kind":"gate","name":"test","status":"PASS","duration_ms":4500}
{"seq":4,"kind":"agent","name":"reviewer","duration_ms":28000,"cost_usd_micros":88120,"model":"opus"}
{"seq":5,"kind":"gate","name":"lint","status":"FAIL","duration_ms":2300,"detail":"gofmt check failed"}
{"seq":6,"kind":"converge","name":"build","status":"NOT_MET","detail":"gates red"}
```

但 trace 系统是**面向单次运行的**：trace 数据用于事后审计单次 evolve/run，从未被聚合分析。

**证据 A: trace 文件只有写入端，没有读取端**

```go
// trace.go — Tracer.Emit 写入 JSONL 行
// 全仓搜索 trace 的消费者（除测试外）:
//   - LoopEngine.OnIteration → trace.Emit (写入)
//   - runAgentPhase → trace.Emit (写入)
//   - runGates → trace.Emit (写入)
//   - execEngine → trace.Emit (写入)
//   - checkpointHook → trace.Emit (写入)
//   - scorecard_wind.go → 只读 scorecards.json，读 trace 文件
//   - forge doctor → 只检查 trace 文件的存在性和可解析性
//
// 没有任何代码回答:
//   - "过去 7 天里，gate 的 PASS 率是多少？"
//   - "implementer phase 的平均耗时趋势？"
//   - "哪些 gate 是最慢的？"
//   - "循环迭代次数是否有增长趋势？"
```

**证据 B: scorecard 是 per-run 快照，非历史趋势**

```go
// scorecard_wind.go — windDownScorecards
// 每轮 evolve/run 结束时写入一个 scorecard.json
{
  "p95_latency_ms": 32000,
  "avg_cost_usd": 0.1841,
  "window": "2026-07-10T03:00:00Z/2026-07-10T04:00:00Z"
}
// scorecard 是当前运行的汇总，不包含:
//   - 与上一次运行的对比（趋势）
//   - 与一个月前的对比（季节性模式）
//   - phase-level 维度（gate/agent/iteration 单独统计）
```

**证据 C: 没有跨运行聚合查询能力**

```bash
# 用户想知道:
# 1. "过去两周 gate FAIL 率是多少？趋势是上升还是下降？"
#    → 无命令可答
# 2. "哪个 phase 平均花的时间最长？哪个 gate 最慢？"
#    → 无命令可答
# 3. "上周 review phase 的 APPROVE 率是多少？"
#    → 无命令可答
# 4. "最近 10 次 evolve 迭代的平均步长（roadmap completion 增量）是多少？"
#    → 无命令可答
```

**证据 D: 活跃数据无处体现**

```
.gforge/trace.jsonl 包含全部运行历史，但:
  - 没有索引，没有按时间/kind/name 的检索能力
  - 文件可能很大（24h evolve = ~2000 事件 = ~500KB），但 grep/jq 是唯一查询方式
  - 无自动聚合、无定期摘要
  - 无告警规则（"gate FAIL 率连续 3 次上升 → 告警"）
```

### 为什么需要它

ForgeOS 作为一个治理平台，核心价值主张是「让 AI 24h 自治地改进软件」。治理者的核心职责是**了解被治理系统的健康状况**。没有跨运行分析：

1. **看不到趋势**：gate FAIL 率是在改善还是在恶化？不知道。团队可能在逐渐引入技术债务而无人察觉
2. **看不到瓶颈**：哪个 gate 最慢？哪个 phase 最贵？无法针对性优化
3. **看不到异常**：本次迭代延迟比历史均值高 5 倍——是一个异常事件，但没人知道，因为没有历史基线
4. **无法做容量规划**：不了解 token 消耗趋势，无法预测下个月的 API 成本
5. **治理者自己被蒙在鼓里**：ForgeOS 管理被治理系统的质量，但它**自己的运行状况**无人管理

### 建议方向

1. **`forge analytics` 子命令族**：
   ```
   forge analytics gates                 # gate 通过率趋势（最近 N 次运行）
   forge analytics phases                # phase 耗时/成本分布
   forge analytics trends                # 关键指标趋势（迭代时间、gate 率、cost/run）
   forge analytics outliers              # 异常运行（耗时/成本超出历史基线 2σ）
   forge analytics report                # 完整分析报告（Markdown/JSON）
   ```

2. **Trace 轻量索引**：在 `.forge/` 下维护一个轮转的 `trace.idx`（SQLite 或简单二进制索引），按 `(kind, name, timestamp)` 索引事件 seq 号，支持快速范围查询而不扫描整个 trace。

3. **健康基线**：自动计算每个 phase/gate 的**历史均值和标准差**。当新值超过 `mean + 2σ` 时告警。基线随运行次数渐进更新（指数加权移动平均）。

4. **`forge status --analytics`**：`forge status` 增加聚合统计段：
   ```
   forge status --analytics --days 14
   
   Run Activity (last 14 days):
     Total runs:       23
     Total iterations: 47
     Total agent cost: $4.23
   
   Gate Health:
     test:        PASS 91% | FAIL 9%  (trend: stable)
     lint:        PASS 85% | FAIL 15% (trend: ↑ failing)
     secret-scan: PASS 100%
     arch-check:  PASS 100%
   
   Phase Latency (p50 / p95):
     implementer:  12s / 45s
     reviewer:     28s / 62s
     gate-phase:   8s  / 22s
   
   Anomalies Detected:
     - 2026-07-08 gate FAIL rate 33% (3× baseline)
     - 2026-07-09 implementer latency 89s (4× baseline, outlier)
   ```

5. **趋势告警 hook**：在每次 `forge run/evolve` 结束后，自动计算最近 N 次运行的关键指标，按阈值触发告警（写入 trace 或 stderr），让治理者自己也被治理。

### 边界场景

| 场景 | 当前行为 | 应然行为 |
|------|---------|---------|
| 团队想知道昨天下线的原因 | 翻 trace.jsonl + jq 人工分析 | `forge analytics outliers --date 2026-07-09` 显示异常 |
| CTO 问 "ForgeOS 跑了一个月，效果如何？" | 无数据可答 | `forge analytics report` 输出完整月报 |
| lint gate 连续 FAIL 3 次，趋势在恶化 | 无人察觉，直到有人手动查看 | `forge status --analytics` 显示 "trend: ↑ failing" |
| 测试工程师想了解哪些 gate 最慢以优化 CI | 需要手动 parse trace | `forge analytics phases --kind gate --sort duration` |
| 用户想比较 mode=explorer vs mode=engineering 的成本差异 | 无法按 mode 分组统计 | `forge analytics report --group-by mode` |

### 差异化证明

- 已有分析的「可观测性」相关方向全部聚焦于**单次运行**：trace 事件格式（`trace.go`）、trace 事件完整性（`forge doctor` 检查 trace 可解析性）、scorecard per-run 汇总、telemetry（p95 latency / avg cost per run）。**没有一篇**提出跨运行聚合分析。
- `strategic-extensions-v24-uncovered-frontiers.md` 提到 `cross.run` 关键词但仅作为子进程生命周期管理的上下文（「跨运行孤儿进程收集」），不是**运行数据交叉分析**。
- `five-genuinely-uncovered-frontiers.md` 的搜索表显示 `run.*identity|artifact.*lineage|cross.run` 被标记为「已被 five-uncovered-architectural-frontiers.md 方向一覆盖」——但阅读该方向发现它覆盖的是**运行身份与产物溯源**（artifact lineage），不是**跨运行行为趋势分析**。
- 本文方向五的独特价值：trace 数据已存在且高质量，但分析层完全缺失。这是「建了日志系统但没人看日志」的架构版本——修复成本低（纯查询层），杠杆极高。

---

## 汇总

| # | 方向 | 类型 | 优先级 | 代码证据 | 已有覆盖 |
|---|------|------|--------|---------|---------|
| 1 | **生产事故响应工作流**（hotfix / rollback / post-mortem） | 工作流/可靠性 | P0 | 全仓零 emergency bypass 流程、零回滚工作流、零 post-mortem 强制 | 0 篇独立（仅 1 篇作为 resume 配置漂移的子用例浅提） |
| 2 | **Context Window 作为一等资源管理**（token 预算 / lane 优先级 / window-aware routing） | 资源管理 | P1 | `prompt_context.go` 零 token 计数、`prompt_memory.go` 硬编码 cap、`routing.go` 零 window 感知 | 12 篇浅提「注意 context 长度」，0 篇作为独立方向 |
| 3 | **Prompt 内容可复现性**（hash 指纹 / lane 版本 / golden snapshot / 回归检测） | 可观测性/可靠性 | P1 | `buildPrompt` 无 hash、`trace.Event` 无 prompt 信息、无黄金文件测试 | 5 篇作为路由标签子方向提 `prompt_version`，0 篇作为复现性框架 |
| 4 | **Dry-Run 语义完备性差距**（fake agent / simulate / 诚实差距报告） | 测试/工程化 | P1 | `DryRunExecutor` 返回空输出、converge 在 dry-run 下永远 NOT MET、loop-back 不可达 | 3 篇浅提「轨迹回放」，0 篇分析 dry-run vs real-run 鸿沟 |
| 5 | **跨运行工作流行为分析**（trace → insight / 趋势 / 基线 / 异常检测） | 可观测性/分析 | P1 | `trace.go` 只有写入端、scorecard 是 per-run 快照、零跨运行聚合逻辑 | 0 篇独立（同领域文档聚焦单次运行可观测性，非跨运行分析） |

### 收敛建议

**若只做一件**：方向一（生产事故响应工作流）。当 ForgeOS 治理的生产系统第一次宕机时，没有 incident workflow 会让 SRE 绕开系统、破坏治理一致性、丢失事后教训。这是所有方向中唯一在**系统压力最大时**暴露的缺口，也是影响用户信任最直接的方向。

**若做前三件（全部 P0/P1）**：方向一 + 方向二 + 方向三。分别解决「生产事故时治理不崩溃」「长运行时 prompt 不爆 window」「AI 裁决可审计可复现」三个核心信任问题。这三者共同构成 ForgeOS 从「好用的开发工具」到「可信赖的生产治理平台」的跨越。

**全部五件**：方向四（dry-run 语义鸿沟）和方向五（跨运行分析）是锦上添花的工程化提升。方向四让工作流开发者能在不花真钱的情况下验证工作流行为；方向五让治理者能看到被治理系统的长期趋势，从「被动响应」提升到「主动治理」。
