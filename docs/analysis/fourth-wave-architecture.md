# ForgeOS — 第四次架构扫描：被忽视的隐性能力缺口

> **扫描基准**：`b0c80e4`  
> **视角**：专门找「声明了但不可执行、存在了但不互联、能测量但不聚合」的隐性缺口  
> **方法论**：全量文件遍历 + 消费端链路追踪（从声明到执行的每条路径都走通一遍）

---

## 扫描发现：三个结构性的「有但没用」缺口

### 缺口 1：`forge doctor` 已存在但被完全孤立

`cmdDoctor` 在 `validate.go:290-405` 实现，功能完善：

```
forge doctor:
  [PASS] .forge/ directory exists
  [PASS] no .tmp residue
  [PASS] checkpoint.json — readable
  [PASS] trace.jsonl — 12 events, last line complete
  [PASS] memory.jsonl — 8 entries
  [PASS] python3 on PATH
  [PASS] harness/yaml2json.py — exists
forge doctor: all checks passed
```

但 **没有任何地方调用它**：
- `forge accept` 不调用它
- `forge run`/`evolve` 前不调用它
- CI 不调用它
- `main.go` 的 help 文本里甚至没有列出它（help 只列到 `validate|memory-prune|status`）

它是一个只存在于源码中的诊断工具，用户不知道它的存在，系统不自动利用它的输出。

### 缺口 2：Phase 输出内容无结构性校验

当前只有 `parseReviewerVerdict` 从 agent 输出中提取结构化数据（`VERDICT: APPROVE|REQUEST_CHANGES`）。其他所有 agent phase（planner、implementer、qa、architect…）的输出被**盲接收**：

```
planner 输出 → 无校验 → 存入 phaseOutputLedger
implementer 输出 → 无校验 → 直接写文件
qa 输出 → 无校验 → 丢弃
architect 输出 → 无校验 → 写入设计文档
```

没有检查：
- planner 是否真的输出了任务拆分（可能只写了「好，我来分配任务」）
- implementer 是否真的写了代码文件（可能只写了「正在实现中……」）
- qa 是否真的跑了验证（可能只写了「看起来没问题」）

依赖链是：**「agent 说它做了 = agent 做了」**，信任假设是全体 agent 绝对诚实。

### 缺口 3：`risk.FromChangedPaths` 完全依赖路径猜测，零内容分析

```go
paymentNeedles = []string{"payment", "billing", "charge", "invoice"}
```

一个 CSS 文件 `billing.css` → `TouchesPayment = true`  
一个 SQL 注入漏洞引入在 `formatter.go` → 无任何 needle 匹配 → `Classify` 返回 `Low`

这是文档中诚实声明了的限制（"coarse heuristic"），但意味着当前的 risk-based routing 在生产级安全场景下**不可信**。`Critical → Opus` 这个最硬的 safety floor 依赖的是一个路径子串匹配。

---

## 五个高价值扩展方向

### 方向 1：Agent 输出约束强制层（从信任到验证）

**当前状态**：
系统构建了极其丰富的 prompt 上下文（角色卡 + ADRs + 红线 + memory + gate 结果 + 前馈产出），但 agent 输出没有任何结构性校验。只有 reviewer 的 `VERDICT:` 行被解析；其他 phase 的输出被盲接收。

**建议方案**：

```go
// Phase 输出合约声明（在 workflow YAML 层）
// 让每个 phase 声明它应该产出什么格式的输出
phases:
  - name: planner
    agent: planner
    output_contract:              // 新字段
      format: markdown           // 期望输出格式
      must_contain:              // 必须包含的内容模式
        - "- task:"              // 任务拆分条目
        - "- acceptance:"        // 验收判据
      must_not_contain:          // 禁止包含的内容
        - "I cannot"             // 拒绝执行
        - "let me think"         // 空转
      min_lines: 5
      max_lines: 200
```

对应的校验引擎（`internal/contract`，纯 Go 标准库）：

```go
func Validate(output string, contract OutputContract) []Violation {
    // 格式检查（markdown/JSON/plain-text）
    // 内容模式检查（must_contain / must_not_contain）
    // 行数范围检查
    // 返回违反列表，非空则 phase status = "CONTRACT_VIOLATION"
}
```

校验失败的处理：
- `warn` 模式：记录 warning，继续
- `block` 模式：标记 phase 为 `CONTRACT_VIOLATION`，gate 变红，触发 on_fail 逻辑

**为什么需要**：

| 维度 | 理由 |
|------|------|
| **系统韧性** | 当前系统把 agent 的诚实性当基础设施——如果某个 agent 输出空内容（因 prompt 注入、模型退化、上下文超限），下游 phase 和 convergene 不知情，继续工作。一个输出校验层在 15ms 内就能拦截 |
| **收敛信号的可靠性** | `RoadmapCompletion` 是 agent 自报的。如果 implementer 的输出合约要求包含 `- [x] task` 才能算完成，自报的可靠性会自然提升 |
| **Debug 价值** | 合约违反是比「phase 失败」更细粒度的诊断信号——「planner 输出了 3 行而非预期的 5 行」告诉你 prompt 可能太短或 agent 不理解任务 |

**边界情况**：

1. **合约太严格导致 false positive**：planner 的输出格式正确但没包含 `must_contain` 中的精确短语。需要区分「格式违规」和「内容不完整」，前者 block，后者 warn
2. **合约随模型能力演进**：Haiku 的输出可能比 Opus 短，同样的 `min_lines` 对 Haiku 太严。需要 model-aware 的合约阈值
3. **合约本身需要测试**：合约写错了怎么办？`forge validate --contracts` 应该能对合约做冒烟测试

---

### 方向 2：`forge doctor` 的自动接入与主动诊断

**当前状态**：
`forge doctor` 已经是一个相当完善的诊断工具（检查 checkpoint/trace/memory/python3/shim/.tmp 残留），但**完全孤立**——没有在任何关键路径上被自动调用，用户也不知道它的存在（help 文本都没列出）。

**建议方案**：

**Phase 1（零架构变更，~20 行）**：
- 在 `forge run` / `forge evolve` 开始时自动调用 `forge doctor --quick`（只跑前 3 项检查，不扫文件完整性）
- 在 `forge accept` 前自动调用完整 `forge doctor`
- 在 help 文本加上 `doctor`
- 诊断结果写入 trace（作为 `kind: "doctor"` 的事件）

**Phase 2（结构化诊断报告）**：

```
forge doctor
  runtime:
    [PASS] .forge/ directory
    [PASS] checkpoint readable
    [WARN] trace.jsonl last line truncated (possible crash)
  tooling:
    [PASS] python3 on PATH
    [PASS] yaml2json.py exists
  governance:
    [PASS] agent cards all present (9/9)
    [PASS] workflows all parseable (5/5)
    [FAIL] policies.yml enforce=block, but max_function_lines missing
  performance:
    [INFO] memory.jsonl: 8 entries (2 KB) — healthy
    [INFO] trace.jsonl: 12 events (3 KB) — healthy
    [WARN] checkpoint age: 14 days — last run was 2 weeks ago
```

**Phase 3（持续健康评分）**：
- 每次 `forge doctor` 的结果写入 `.forge/health.jsonl`
- `forge status --trend` 输出健康变化趋势：
  ```
  forge status --trend
    last 7 days: 12 checks, avg score 92/100
    trend: ↓ (was 96 last week)
    top issues: checkpoint stale (x3), .tmp residue (x2)
  ```

**为什么需要**：

| 维度 | 理由 |
|------|------|
| **自治理** | ForgeOS 的治理哲学要求「系统自己知道自己的状态」。`forge doctor` 是自认知的载体，但当前它只是一个无人使用的独立命令 |
| **防御性运行** | 在 `forge run` 之前跑 doctor 可以预防「跑了 30 轮 evolve 后才发现 trace 文件损坏了」——运行时诊断比事后诊断节省成本 |
| **健康趋势** | 单个 doctor 输出是二元的（PASS/FAIL），跨时间的趋势能检测到退化——如 checkpoint 年龄增长表示 evolve 很久没跑了 |

**边界情况**：

1. **`--quick` vs `--full` 的语义**：quick 应该多快？只检查 `.forge/` 存在性 + checkpoint 可读性 ≈ 2ms。full 包括 agent 卡完整性 + workflow 可解析性 ≈ 100ms
2. **自检的自检**：如果 doctor 本身有 bug，输出错误的状态怎么办？`forge doctor --self-test` 跑 doctor 的单元测试
3. **CI 集成**：`forge doctor` 应该作为 CI 的一个 step 吗？如果 doctor FAIL（python3 不在 PATH），CI 应该 RED 吗？还是只 WARN？

---

### 方向 3：相位级健康画像（从「全局超时」到「自适应相位调度」）

**当前状态**：
`CommandExecutor.Timeout` 是一个全局值，所有 phase 共享同一个 timeout：

```go
type CommandExecutor struct {
    Timeout time.Duration  // 所有 phase 共享
    // ...
}
```

`backoff.go` 的 `overloadBackoff` 是固定指数退避，不感知 phase 类型。50 次迭代中每次的 planner 和每次的 implementer 都用相同的超时和退避。

**问题**：

```
实际场景：
  planner phase:    通常在 10-30s 内完成
  implementer phase: 通常在 60-180s 内完成（写代码 + 测试）
  reviewer phase:   通常在 30-90s 内完成

当前配置：
  --timeout 300s → 对所有 phase 统一
  如果 planner 在 15s 时就 hang 了，它还要等 285s 才超时
  如果 implementer 需要 200s 但 reviewer 只需要 40s，reviewer 拿到 300s 预算
```

**建议方案**：

**Phase 1（基于历史的动态超时）**：
```go
// 每个 phase 记录自己的历史耗时
type PhaseProfile struct {
    Name       string
    P50Ms      int64
    P95Ms      int64
    Samples    int
    LastRunMs  int64
}

// forge run 使用历史数据设置 per-phase timeout
// 而不是全局 --timeout
timeout := phaseProfile.P95Ms * 1.5  // 1.5x headroom
if timeout > globalMaxTimeout {
    timeout = globalMaxTimeout        // 绝对上限
}
```

数据来源：`trace.jsonl` 中 `kind: "agent"` 的 `duration_ms` 字段。按 `name`（phase 名）聚合。

**Phase 2（相位感知退避）**：
- `planner` 的 overload 退避：短（2s base，16s cap）—— planner 通常快，不需要长等待
- `implementer` 的 overload 退避：中（5s base，60s cap）—— implementer 成本高，值得等
- `reviewer` 的 overload 退避：长（10s base，120s cap）—— reviewer 是 Opus，最贵，最值得等

**为什么需要**：

| 维度 | 理由 |
|------|------|
| **成本效率** | 一个 300s 全局超时意味着每个 hang 的 phase 浪费最多 285s。50 次迭代 × 每个迭代 3 个 agent phase = 最多浪费 42750s（11.8 小时）的 wall clock |
| **自适应** | 系统已经有 `trace.jsonl` 记录了每个 phase 的 `duration_ms`。这些数据当前只用于 scorecard，不用于调度优化 |
| **差异化 SLA** | implementer（Sonnet，$0.015/调用）和 reviewer（Opus，$0.15/调用）的成本差 10 倍。reviewer 值得更长的退避和更耐心的重试 |

**边界情况**：

1. **冷启动**：第一次运行没有历史数据，用保守默认值（如 120s）
2. **Phase 耗时漂移**：项目变大后 implementer 从 60s 涨到 180s，历史 P95 需要滑动窗口
3. **`--parallel` 模式下的竞争**：多个 phase 同时运行，各自的 timeout 独立，但总的资源消耗可能超过宿主容量。需要总的并发预算

---

### 方向 4：配置漂移的自动修复（从「只读诊断」到「自修复」）

**当前状态**：
当前系统有多个「声明 vs 实现」一致性问题：

1. `requirement_confidence` 和 `review_status` 声明了但 converge 不消费（上轮分析的 bug）
2. `policies.yml` 的 `max_function_lines` 可能被编辑但 `modes.yml` 没同步
3. `project.yml` 的 `mode` 和 `lifecycle` 可能在多个地方重复（project.yml、modes.yml、workflow 引用的 fragment）
4. `forge doctor` 只报告问题，不修复
5. `forge validate` 只报告可解析性，不检查语义一致性

**建议方案**：

```bash
# 新命令或 forge doctor 的 --fix 模式
forge doctor --fix
  [FIX] checkpoint.json: repaired (last event truncated)
  [FIX] memory.jsonl: pruned to 500 entries (was 623)
  [FIX] policies.yml: max_function_lines restored (was deleted)
  [WARN] trace.jsonl: cannot fix (last line corruption is permanent data loss)
  [INFO] requirement_confidence metric: declared but unconsumed
         → 建议添加 requirement_confidence evaluator 或修改 discover.yml

forge doctor --diff
  checkpoint.json: iteration 3 → 7 (normal progress)
  memory.jsonl: 15 → 8 entries (pruned)
  scorecards.json: 7 → 9 entries (new model/task pairs)
```

**为什么需要**：

| 维度 | 理由 |
|------|------|
| **自治理闭环** | 当前系统只能检测问题，不能修复。「发现问题→修复问题」的闭环是自治系统的基础能力 |
| **guardian 模式** | 如果一个用户（或 agent 行为）错误删除了 `policies.yml` 的 `max_function_lines`，当前的系统会静默丢失函数长度执法。自修复可以恢复 |
| **dogfood** | ForgeOS 自己的仓库就发生过 `max_function_lines` 在重构中丢失的情况。如果有一个 `forge doctor --fix` 钩子在每次 evolve 前跑，这类问题不会累积 |

**边界情况**：

1. **修复的安全边界**：`--fix` 绝不能修改代码或业务文件。它只修复 `--forge/` 运行时状态和 `.agent/` 治理配置
2. **修复的幂等性**：重复跑 `forge doctor --fix` 不应产生副作用。第一次修复后，第二次应该 report clean
3. **不可逆修复的确认**：`trace.jsonl` 截断无法修复——数据永久丢失了。`--fix` 应该报告「无法修复」而不是静默跳过

---

### 方向 5：自举 / 自演化（ForgeOS 改进 ForgeOS 自身）

**当前状态**：
ForgeOS 可以 evolve 用户项目，但**不能 evolve 自己**。具体来说：

- `forge evolve build --executor command --agent-cmd claude` 可以改进 `examples/url-shortener` 的代码
- 但无法改进 `forge-core/` 自身的代码
- 无法改进 `.agent/workflows/` 的编排逻辑
- 无法改进 `harness/` 的 gate 逻辑

原因不是因为技术限制（`forge-core/` 也是代码，agent 可以读写），而是因为：
- 没有自我改进的 workflow（`self-evolve.yml` 不存在）
- RDADMAP 对于 forge-core 自身的改进方向没有结构化的差距分析
- 没有「改进 harness」的 agent 角色卡

**.ai/README.md 的 10 阶段 SDLC** 描述了如何对项目做结构化评审，但 ForgeOS 不对自己实施这套流程。

**建议方案**：

```yaml
# .agent/workflows/self-evolve.yml
# 让 ForgeOS 可以改进自己
stage: self-evolve
type: loop
stop_condition:
  type: external
  triggers: [human_pause, budget_exhausted]

phases:
  - name: self-scan
    agent: explorer
    prompt: |
      扫描 forge-core/ 和 harness/ 的代码质量。
      查找：函数长度超过红线、循环依赖、缺失的测试覆盖、
      TODO 注释、未使用的导出符号。

  - name: self-gap
    agent: architect
    feeds_forward: true
    prompt: |
      基于 self-scan 的发现 + 已有的 docs/analysis/* 分析文档，
      输出 ForgeOS 自身需要改进的差距列表。
      优先级排序：红线违规 > 已分析的缺口 > 代码异味。

  - name: self-roadmap
    agent: planner
    feeds_forward: true
    prompt: |
      将 self-gap 的差距列表转换为 .agent/ROADMAP.md 的 checklist 项。
      每项包含：差距描述、建议修复、预期影响、工作量估计。

  - name: self-implement
    agent: implementer
    required_gates: [lint, test, build, complexity, arch, security]
    model_tier: opus
    prompt: |
      实现 self-roadmap 中最高优先级的未完成项。
      你正在修改 ForgeOS 自身的代码。遵守 AGENTS.md 的所有红线。
      每次修改后跑 forge doctor。

  - name: self-review
    agent: reviewer
    fresh_context: true
    model_tier: opus

  - name: self-evaluate
    agent: qa
```

**为什么需要**：

| 维度 | 理由 |
|------|------|
| **狗食的终极形态** | 一个治理系统，当自己的治理出现 gap 时能自动发现和修复——这是自治闭环的完整形态。当前 ForgeOS 可以治理 url-shortener 但不能治理自己 |
| **持续质量** | ForgeOS 目前的代码质量已经很高（8 项 arch-check），但「高」是静态的快照。没有一个系统性的机制来防止质量退化——只有人工代码审查 |
| **产品验证** | 如果 `forge self-evolve` 真能跑通，它将是对 ForgeOS 产品价值的最强证明——系统在无人值守下改进自己 |

**边界情况**：

1. **自我修改的风险**：如果 agent 修改 `forge-core/` 时引入了 bug，下一个 iteration 的 `self-implement` 可能在损坏的系统上运行。「先测试再修改」是必须的纪律
2. **无限递归**：如果 agent 修改了 `harness/gate.mjs`，然后用修改后的 gate 来评估自己的修改——因果循环。需要独立的 gate 实例
3. **人类监督**：自我修改必须默认 `dry-run`，`--apply` 需要人类确认。同 `forge migrate` 的模式

---

## 优先级矩阵

| 方向 | 影响面 | 成本 | 前置依赖 | 推荐 |
|------|--------|------|---------|------|
| **1. Agent 输出约束强制层** | 系统韧性：高 | 中 | 无 | Sprint n+1 |
| **2. forge doctor 自动接入** | 自诊断：中-高 | **低**（~20 行接入 + help 文本） | 无（代码已有） | **Sprint n** |
| **3. 相位级健康画像** | 性能/成本：中-高 | 中 | trace 数据已存在 | Sprint n+2 |
| **4. 自修复（doctor --fix）** | 自治理：中 | 低-中 | 方向 2 先行 | Sprint n+1 |
| **5. 自演化（self-evolve）** | 产品力：高 | 高 | 方向 1、2、4 先行 | Sprint n+3 |

---

## 隐藏的模式：三个方向的通用基座

观察方向 1、3、5 可以发现一个共同的需求：**agent 输出需要结构化**。

- 方向 1 需要输出合约（`must_contain`、`format`）
- 方向 3 需要每个 phase 的耗时数据（`trace.jsonl` 已有，但需要 phase-level profile）
- 方向 5 需要结构化扫描结果（不只是「有违规吗」，而是「最多违规的类型是啥」）

这三个方向共享一个底层能力：**将 agent 输出从「自由文本」升级为「结构化报告 + 自由文本附件」**。

这不是一个单独的扩展方向，而是上述方向的共同基础设施。如果用一句话总结本次扫描的最大发现：

> ForgeOS 对 agent 输出的理解停留在「agent 说 OK 就是 OK」的信任模式。当输出合约、profile 数据和自扫描数据都变成结构化信号时，系统就从「信任」进化为「验证」。

---

*分析日期：2026-06-30 | 第四次全量扫描，主要发现已在 validate.go 中发现 `forge doctor` 的完全实现但零调用*
