# ForgeOS — 隐藏反馈回路、自认知能力与 CI/CD 盲区

> **第五次扫描**，这次跳出单个子系统边界，关注
> **子系统之间的交互反馈回路**，以及系统对自己的认知能力。
>
> 不写代码，只做判断。

---

## 目录

1. [三条隐藏反馈回路及系统性风险](#1-三条隐藏反馈回路及系统性风险)
2. [ForgeOS 对自己的认知能力](#2-forgeos-对自己的认知能力)
3. [CI/CD 管道的盲区](#3-cicd-管道的盲区)
4. [信号质量与垃圾进垃圾出](#4-信号质量与垃圾进垃圾出)
5. [长时间运行下的操作表面退化](#5-长时间运行下的操作表面退化)

---

## 1. 三条隐藏反馈回路及系统性风险

### 回路 A：Memory → Prompt → Agent → Convergence → Memory

**循环**：memory（跨 iteration 积累的缺口/决策/教训）→ 注入到下次 iteration 的 prompt →
agent 根据 prompt 行动 → convergence 衡量 RoadmapCompletion → 新发现写入 memory

**当前状态**：
- Memory 是**只增日志**，从不遗忘
- `boundMemory`（prompt_memory.go）在注入前做即时过滤（按新鲜度+相关性）
- `memory.Append` 在每个 agent phase 之后由 `cmd/forge` 调用，写入 `KindGap` / `KindLesson` / `KindDecision`
- 非测试代码只有 `memory.Append` 调用，`memory.Load` 只在 buildPrompt 时调用

**潜在问题**：

**a) Memory 的"自我强化循环"**：如果某个错误的 Decision 写入了 memory，它会在后续所有 iteration 中被注入到 prompt 中。Agent 读到这个 Decision，认为它是对的，可能基于它做进一步决策，可能写入更多确认它的 memory 条目。**错误越滚越大，没有纠正机制。**

```
Iteration 1: agent 误判架构方向 → 写入 KindDecision "we decided to use SQLite"
Iteration 2-50: 每次 prompt 都注入 "we decided to use SQLite" → agent 不会质疑它 → 越来越深入这条路径
→ 永远没有人说"这个决策可能不对"
```

当前没有以下机制：
- Memory 条目的**置信度**或**来源追踪**（哪个 agent、哪个 phase、什么 context 下写的）
- Memory 条目的**反驳/修正**机制（没有 `KindRetraction` 或覆盖写入）
- Memory 条目的**生命周期**（没有"如果超过 N 次 iteration 未被引用，降权"）

**b) boundMemory 的即时过滤**：`boundMemory` 在每次 prompt 构造时过滤 memory 条目。
它有 `memoryCap`（当前最多多少个条目注入）和 `recencyFloor`（保留最近多少个迭代的条目）。
但过滤算法只有**关键词精确匹配**——使用 `strings.Contains(text, topic)`：

```go
// prompt_memory.go 的 boundMemory
for _, e := range entries {
    if strings.Contains(e.Topic, topicLower) || strings.Contains(topicLower, e.Topic) {
        // 条件 1：按 topic 相关性
    }
    if capRecent > 0 && e.Iteration > maxIter-capRecent {
        // 条件 2：按 recency
    }
}
```

这意味着：
- Topic 匹配是**子字符串**，不是语义——"payment"会匹配"repayment"和"payment-processing"
- 一旦 memory 文件增长到几千条，`boundMemory` 每次加载全部、丢弃 99%——O(n) 扫描
- 没有**持久化索引**——每次 evolve loop 重启都重新扫描全部

### 回路 B：Gate Results → Prompt → Agent → Gate Results

**循环**：gate verdicts（test/lint 的结果）→ `gateLedger.record()` → `gateLedger.context()` →
注入到后序 agent 的 prompt → agent 根据已通过的测试信息行动 → agent 的代码改变测试结果

**当前状态**：
这是**设计的良好反馈**——让 reviewer 知道 test 已经通过了，不需要自己重跑。
但存在几个微妙问题：

**a) Feed-forward 可能导致"确认偏差"**：如果 implementer 的 test 通过了（但测试本身有缺陷），
reviewer 的 prompt 中看到了"test: ok"——这会潜意识地减少 reviewer 对测试质量的警觉。
真实的代码审查中，reviewer 应该质疑**测试是否覆盖了足够的边界**，而不仅仅看到"测试通过了"。

**b) gateLedger 不区分"N/A"和"PASS"**：`OnGateResult` 收到的是 `"ok" | "N/A" | "FAILED"` 三元组，
但 `context()` 的渲染是：

```
前序闸门结果：...
- lint: ok
- test: N/A
- app_test: N/A
```

N/A 被渲染为与 ok 相同的"闸门结果"。但 N/A 的真实含义是"这个闸门根本没跑"（因为工具不存在）。
如果 CI 中 `ruff` 不可用 → lint: N/A → reviewer 看到"lint: N/A"→ 可能误以为"lint 已通过"。
`converge.go` 的 `greenDetail` 已经修复了这个——它列出豁免的 gate 和原因——但 `gateLedger.context()`
没有这个级别的细节。

**c) Feed-forward 只在一次运行内有效**：`gateLedger` 是内存中的结构，每次 `forge run` 或
`forge evolve` 的每个 iteration 新建。它不持久化，不跨 session。因此，如果一个 run 中
gate 通过了，但 run 因为其他原因中断了（convergence 未达到），下次 run 的 reviewer 看不到
上一次通过的 gate 结果——需要全部重跑。

### 回路 C：Trace → Scorecards → Router → Model → Trace

**循环**：agent 执行产生 trace events（含 cost + latency + model）→ scorecard wind-down 聚合
→ scorecards.json 更新 → Router 在下一次路由时使用 scorecards → 选择不同的 model →
新 model 的执行产生新的 trace events

**当前状态**：这是一个**正确的、期望的学习循环**。但有几个问题：

**a) Scorecard wind-down 只在 evolve 的末尾执行**：`forge evolve` 结束后才调用 `windDownScorecards`。
在 50-iteration 的 evolve 中，前 40 个 iteration 的路由使用的 scorecards 是前一次 evolve 结束时的数据。
这意味着**scorecard 反馈延迟了整个 evolve 的长度**。

**b) Wind-down 的脆弱性**：`windDownScorecards` 只有在 trace 中有 model-stamped cost events
时才实际执行（`traceHasModelCost`）。在 `--executor=dry` 模式下，trace 中没有 cost events，
scorecards 永远不会更新。这本身是正确的（不伪造数据），但意味着在 dry-run 模式下测试路由循环是不可能的。

**c) Router 不验证 scorecard 数据的新鲜度**：如果 scorecards.json 因为某种原因被写入了过期数据
（例如回滚了文件、从备份恢复了），Router 会信任这些数据。没有 freshness check、没有 staleness 标记、
没有"如果数据比 N 天更旧，回退到默认"的机制。

### 回路 D：RoadmapCompletion 的自我报告风险

这是**最危险的反馈回路**：

```
Agent 被 prompt 注入 ROADMAP.md
→ Agent 自我判断 "我完成了哪些项？"
→ Agent 在输出的最后一行声明 VERDICT/完成状态
→ cmd/forge 解析该输出 → RoadmapCompletion%
→ convergence 判断是否达到 100%
→ 如果达到，循环结束；否则，继续下一次 iteration
→ 下一次 iteration 的 prompt 包含相同的 ROADMAP.md
```

问题：**RoadmapCompletion 是 agent 自我报告的**，没有独立验证。

agent 完成的真正"检查"是 **Git diff**（代码是否真的写了）和 **gate 结果**（test 是否通过了）。
但没有机制验证 agent 自报的 completion 是否诚实。一个 agent 可以报告"我完成了 100% 的 roadmap items"，
而代码中完全没有实现任何东西——只要 test gates 通过了，系统就会认为收敛了。

这在实际中没有发生（agent 没有动机撒谎），但重要的是：**系统的正确性依赖于 agent 的诚实**，
没有任何冗余或验证机制。如果未来有恶意 agent 或 prompt 注入攻击，这个回路可以被利用。

---

## 2. ForgeOS 对自己的认知能力

### 2.1 知道自己的架构吗？

ForgeOS 有 `arch-check.mjs` —— 它对 repo 的包结构、依赖方向、文件数量和导出数量进行扫描。
这些规则是在 `.arch/rules.yaml` 中声明的（如果有的话）——实际上是重用 `harness/policies.yml` 中的值。

但关键点：**ForgeOS 的运行时（Go orchestrator）不知道这些架构规则**。
引擎不知道"internal/ 不能导入 cmd/"——它只按 phase 顺序执行。架构规则是由 Node 的 arch-check 独立执行的。

这意味着 `forge run` 可以在一次 violation 违规存在的代码库上成功——arch-check 只在 CI 或
`forge accept` 时运行，不在 `forge run` 的运行前检查。

### 2.2 知道自己的测试覆盖率吗？

**部分知道**。`probeCoverage` 在每个语言 adapter 的 coverage 命令运行后解析覆盖率报告。
但 ForgeOS 的 **self-testing**（harness/test_*.mjs 和 forge-core/*/_test.go）和项目代码的测试
是同一个 gate 评估的。acceptance gate 的 `probeTests` 会运行两种测试：

1. `harness/test_*.mjs` —— 框架自身的测试
2. `<adapter-lang> test` —— 项目测试

在 ForgeOS 项目中，这意味着：`go test forge-core/...` 和 `node --test harness/test_*.mjs` 和
`ruff check harness/test_check.py` 都在同一个 gate 系统中运行。

但对于一个**使用 ForgeOS 的新项目**，测试覆盖率报告会告诉项目作者"你的测试覆盖了 67% 的代码"，
但它不会告诉 ForgeOS 自身的测试质量。

### 2.3 知道自己的文件是否遵循了自己的规则吗？

**非常有限的自我认知**：

| ForgeOS 规则 | 对自己强制执行？ | 执行方式 |
|-------------|----------------|---------|
| 文件 ≤ 500 行 | ✅ | arch-check 的 checkPackage |
| 函数 ≤ 50 行 | ❌ | 没有函数级检测 |
| 认知复杂度 | ❌ | 没有检测 |
| 循环依赖 | ❌ 但在 Go 中由编译器拒绝 | 通过编译 |
| 禁止的内→外依赖 | ✅ | arch-check 的 checkLayering |
| 扇入 ≤ 20 | ✅ | arch-check 的 checkFanin |
| 治理完整性（文件存在） | ✅ | check.py |
| Secret 硬编码 | ✅ | secret-scan.mjs |

缺少的：函数级度量（行数/复杂度）— Go 侧没有 gocyclo 的 runner，而且 arch-check
不解析 Go AST 中的函数边界。

### 2.4 能自我诊断吗？

**不能**。没有 `forge diagnose` 或 `forge doctor` 命令。当出现问题时（gate red、convergence 未达成、
trace 损坏），用户只能通过阅读原始日志来理解原因。

具体缺失的自我诊断能力：
- "为什么 gate 变成了 N/A？" → 需要手动检查工具是否安装
- "为什么 trace 是空的？" → 需要手动检查 JSONL 文件
- "scorecard 为什么不是最新的？" → 需要手动检查时间戳
- "checkpoint 损坏了？" → 运行时不会自动提示"你有一个损坏的 checkpoint，等待手动清理"
- "configuration 文件是否一致？" → 没有 forge validate

---

## 3. CI/CD 管道的盲区

### 3.1 CI 配置分析

`.github/workflows/forge.yml` 运行：

```yaml
- name: forge accept (Stop gate)
  run: node harness/acceptance.mjs
- name: forge-core tests (zero-dep Go runtime)
  run: go -C forge-core test ./...
```

`forge accept` 聚合：
- `gate.mjs` — lint（每种检测到的语言）、test、coverage、app_test
- `arch-check.mjs` — layering、package、fanin、cognitive、anti-pattern、function-length、cycle、drift-guard
- `check.py` — governance completeness
- `secret-scan.mjs` — hardcoded secrets

这些都运行。但以下**没有在 CI 中覆盖**：

### 3.2 不在 CI 中的东西

| 检查项 | 当前状态 | 理由 | 影响 |
|-------|---------|------|------|
| `go test -race forge-core/...` | ❌ 没有 | 只跑 `go test` | 数据竞争检测缺失 |
| `forge run build` (any workflow) | ❌ | CI 只运行 gate/check，不运行 orchestrator | 编排状态机从未在 CI 中测试 |
| `forge evolve` | ❌ | 同上 | 进化的 loop 从未在 CI 中测试 |
| `forge migrate --apply` | ❌ | 需要 git diff 验证 | 迁移路径从未在 CI 中验证 |
| yaml2json.py + Go 集成 | ❌ | 独立的 Python 测试存在（test_yaml2json.py），但 Go 加载编解码没有被集成测试覆盖 | Python 侧的 bug 可能无声地将空数据传递给 Go |
| scorecard-update.mjs + Go 集成 | ❌ | 见前述分析 | 数据完整性路径未被测试 |
| Node harness 完整性测试 | ⚠️ 部分 | `test_acceptance.mjs` 测试了 acceptance 的 pure 函数，但 `node --test` 在 CI 中不运行 | 好的一面：acceptance 的纯逻辑有测试；不足：不是所有 harness 文件都被测试 |
| Go 编译检查 | ✅ | CI 运行 `go test`，其中包含编译 | 好 |
| `forge --help` | ❌ | 没有 CI 检查 | 不起眼但重要：CLI 的变化不会因为文档过期而被捕捉 |

### 3.3 最令人担心的 CI 漏洞

**漏洞 1：编排状态机未在 CI 中测试**

当有人修改 `loop.go`、`backoff.go`、`parallel.go`、`converge.go` 或 `mode_gating.go`
时，CI 会运行它们的**单元测试**（loop_test.go、parallel_test.go 等），但不会运行
端到端的 `forge run build --executor dry` 来验证整个 workflow 的 phase 序列。

这意味着：一个在单元测试中绿了的 PR 可能在实际运行时出错——
例如 phase 过滤后序列索引不对、checkpoint 导致 phase 重复、mode gating 跳过了一个不应该跳过的 phase。

**漏洞 2：没有 CI 验证 `forge accept` 自身的正确性**

`forge accept` 是 ForgeOS 的 Stop gate，但在 CI 中 `forge accept` 本身**不是由 CI 验证其正确性**的。
如果有人破坏 `acceptance.mjs` 使其总是返回 OK（0 种 gate），CI 仍然会绿。没有"gate 的 gate"。

**漏洞 3：Go 的 `-race` 未在 CI 中运行**

`go test -race forge-core/...` 不在 CI 中。这在单线程的核心路径上不是问题，但 parallel.go 的
goroutines 共享 mutable state（cost、ledgers、trace）。race detector 目前只在有人手动
`go test -race` 时运行。

### 3.4 建议

```
# CI 增加的三个步骤（成本：每个约 10-30 秒）

# 1. Race 检测
go -C forge-core test -race ./...

# 2. Orchestrator 端到端测试（dry run）
forge run build --executor dry --root $PWD

# 3. 编译检查 + 验证 forge accept 至少有一个最低输出模式（防止无声回归）
node harness/acceptance.mjs --json  | jq -e '.verdict == "ACCEPTED" or .verdict == "REJECTED"'
```

---

## 4. 信号质量与垃圾进垃圾出

### 4.1 信号路径分析

ForgeOS 的"信号流"——从原始数据到决策——有多个质量衰减点：

```
Agent 输出文本
    │
    ├─ parseReviewerVerdict() → "APPROVE" 或 "REQUEST_CHANGES"
    │  （正则匹配：查找末尾的 "VERDICT: " 模式）
    │
    ├─ RoadmapCompletion()
    │  （统计 ROADMAP.md 中的 [x] / [ ] / [~]）
    │  （主观信号：agent 自己打勾）
    │
    ├─ memory.Append()
    │  （agent 输出中的发现 → 持久化 knowledge）
    │  （质量控制：没有置信度评分、没有冲突检测）
    │
    └─ cost.go readClaudeCost()
       （解析 claude JSON 中的 total_cost_usd → 微美分）
       （客观信号：来自 claude 的原始账单数据）
```

### 4.2 每个信号的质量问题

**a) Reviewer Verdict**：用 `VERDICT: APPROVE|REQUEST_CHANGES` 正则匹配。
如果 agent 的输出中有多行匹配，只有第一个匹配被采取（`parseReviewerVerdict`）。
但 reviewer 可能写出 "I APPROVE the changes to the VERDICT section..."——这会错误地匹配。
当前的实现取最后一个非空行的匹配，但正则 `(?m)^VERDICT:\s*(APPROVE|REQUEST_CHANGES)\s*$` 很精确，
误召回的概率低。

**b) RoadmapCompletion 计数**：计算 `- [x]` / `- [ ]` / `- [~]` 的数量。
这是一个清晰的计数，但如果 roadmap 包含非 checklist 的 `- [x]` 模式（例如示例代码），
会被错误计入。目前没有这种模式，但这是一个脆弱点。

**c) Memory 质量**：memory 条目的内容包括：
- `Gap`：agent 声明发现的短fall——但**没有验证这个 gap 是否真的存在**
- `Lesson`：agent 从失败中学到的教训——但**没有验证这个 lesson 是否正确**
- `Decision`：agent 做出的架构决策——但**没有仲裁或版本控制**

没有以下机制：
- Memory 条目的**peer review**——另一个 agent 能质疑一个 memory 条目吗？
- **Conflict detection**——两个 memory 条目说相反的事情（"use SQLite"和"use PostgreSQL"）
- **Quality scoring**——某些 agent phase 可能比其他的更可靠（reviewer vs implementer）

**d) Cost 信号**：这是**最客观**的信号——来自 claude JSON 的 `total_cost_usd`，经过 `cost.go` 的
严格解析（只认特定格式的 JSON 输出，多个 fallback 路径）。
这是唯一一个不能被伪造的信号。

### 4.3 "信号腐烂"的风险

```
高质量信号（cost, latency） → 客观、可验证、来自外部源
    ↓
中等质量信号（gate results） → 由 harness 独立执行，可重现
    ↓
低质量信号（RoadmapCompletion） → agent 自报，无独立验证
    ↓
最低质量信号（memory entries） → agent 自述，无法证伪
```

系统依赖所有信号来做出正确的收敛决策。如果低质量信号控制了高质量信号，
系统会做出错误的决定。

**具体场景**：memory 被污染（错误的 Decision）→ 每次 prompt 都注入该 Decision →
agent 按错误的架构方向行动 → agent 在 roadmap 上自我标记 [x] →
RoadmapCompletion 达到 100% → convergence 报告 "MET"

这行得通是因为：
1. Memory 没有错误的纠正机制
2. RoadmapCompletion 是自我报告的
3. Convergence 不看代码是否真的正确，只看 gate 和 self-report

**这个场景目前没有发生，因为 agent 是诚实的。但系统的荣誉盾越来越薄。**

---

## 5. 长时间运行下的操作表面退化

### 5.1 退化维度

ForgeOS 是为长时间运行设计的（`forge evolve` 可以在无人值守的情况下运行 24 小时）。
但多个子系统在长时间运行时表现出退化：

| 维度 | 退化模式 | 时间尺度 |
|------|---------|---------|
| memory.jsonl 文件大小 | 持续增长 → Load 变慢 | 1000+ entries |
| trace.jsonl 文件大小 | 持续增长 → wind-down 变慢 | 10000+ events |
| boundMemory 过滤 | 每次扫描全部条目 | 每次 prompt 构建时 |
| prompt 体积 | memory 条目不断增加 → token 消耗增加 | 每次 iteration |
| checkpoint 写入频率 | 每次 iteration + 每个 agent phase | 不变（O(1)） |
| scorecards.json | 合并后稳定 | 不变 |

**critical path**：最糟糕的情况下，prompt 构建（`buildPrompt`）执行 `memory.Load()`
读取 1000+ 行 JSON，然后 `boundMemory` 做 O(n) 的子字符串匹配——对于每个 iteration。
这不是今天的瓶颈（ForgeOS 刚起步，memory 很小），但如果不加限制，随着 memory 增长，每次
迭代都会越来越慢。

### 5.2 没有"分页"或"采样"

所有持久化文件都是**全量读取**：
- `memory.Load()` 读取整个文件
- `trace` 被 scorecard wind-down 读取整个文件
- `Gather()` 读取整个 ROADMAP.md（有 4KB cap，但文件可能更大）
- `adrTitles()` 读取整个 `docs/adr/` 目录
- `constraints()` 读取整个 AGENTS.md

在可预见的将来，这些文件都不会大到成为问题（几十 KB 最多）。但设计上没有任何分页或采样机制，
如果仓库长年积累，这些 O(n) 操作会线性增长。

### 5.3 memory 的"忘记"能力

当前唯一的内存遗忘机制是 `boundMemory` 的 `recencyFloor`：一个超过最近 N 次 iteration
但从未被检索到的内存条目不会被注入 prompt。但这只是在注射层面的过滤——**文件仍然包含它**，
下次 Load 仍然读取它。

一个 1000 条 memory 的文件，其中 800 条是旧的、不相关的、从未被检索的条目——每次
`boundMemory` 仍然扫描全部 1000 条，只是为了丢弃 800 条。

### 5.4 建议：Memory 压缩 / 归档

```
forge memory prune --keep-last 500 --remove-resolved-gaps --remove-obsolete-decisions
```

这个命令应该：
1. 加载全部 memory 条目
2. 移除标记为 "resolved" 的 gap（显示在 ROADMAP 上已完成的项）
3. 移除被后续 ADR 覆盖的 decision（如果存在 ADR 引用相同 topic 且更近）
4. 移除超过 N 次 iteration 未被检索到的 lesson（降权为零后再保留 N/2 次后删除）
5. 只保留最新的 500-1000 条
6. 用精简后的列表重写整个 memory.jsonl

---

## 总结：最应该担心的 5 个系统性风险

| 风险 | 根本原因 | 触发条件 | 影响 |
|------|---------|---------|------|
| **自我报告收敛** | RoadmapCompletion 无独立验证 | agent 错误标记完成 + gates 通过 | 虚假收敛 |
| **记忆污染** | Memory 只增不删，无仲裁 | 一个错误 decision 导致级联错误 | 反复做出错误决策 |
| **编排未测试** | CI 不运行 forge run/evolve | 状态机变更不被 CI 捕捉 | 无声回归 |
| **信号质量衰减** | 低质量信号控制高质量信号 | memory 污染 + self-report 收敛 | 错误但自信的收敛 |
| **长时退化** | 只增文件无 GC | 1000+ iteration 后 | 加载/过滤变慢 |

*分析日期：2026-06-29 | 基于第五次全量扫描（反馈回路 + 自认知 + CI 盲区视角）*
