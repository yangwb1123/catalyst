# ForgeOS: 二阶架构缺口 —— 系统自身演化的伴生问题

> **视角**: 资深架构师 / 产品经理,关注的不是「加什么功能」,而是**系统因为自身存在而产生的新问题**。
> **方法**: 全局代码库扫描(forge-core 18 Go 包 · harness 39 模块 · `.agent/{WORKFLOWS,AGENTS,SKILLS}` 全部声明 ·
> 31 个 Sprint 完整演进记录 · `.forge/` 运行时产物分析 · 40+ 篇已有分析文档交叉验证)。
> **核心判断**: 已有 3 篇 `docs/requirements/*.md` + 40+ 篇 `docs/` 分析已覆盖了**第一阶缺口**——
> 「系统还缺什么功能」(15 个功能方向)。本文瞄准的是**第二阶缺口**——「系统因为有了这些功能,产生了什么新问题」。
> 这些问题不是功能缺失,而是**系统复杂度增长的副作用**,在 Sprint 31 之前尚不明显,但随系统进入持续自治运行后会迅速放大。
> **纪律**: 不写代码。每方向附具体代码位置 + `.forge/` 运行时数据证据。
> **日期**: 2026-07-09

---

## 前言:二阶思维 (Second-Order Thinking)

第一阶思维问:「我们还需要什么?」
第二阶思维问:「我们已有的东西,正在制造什么问题?」

ForgeOS 已有:

| 资产 | 数量 | 增长率 |
|------|------|--------|
| Go 包 | 18 | Sprint 5 时 11 个,27 个月翻倍 |
| Go 源文件 | ~195 | 持续增长 |
| 测试用例 | 707+ | 每次 Sprint 增加 |
| memory 条目 | 14(本仓) | 每个 evolve 循环增加 |
| trace 事件 | 91(本仓) | 每个 `forge run` 增加 |
| agent 角色卡 | 12 | 已稳定 |
| workflow 文件 | 5 | 已稳定 |
| docs/ 分析文档 | 40+ | 每次分析 session 增加 |

系统正在从「被构建的系统」变成「持续演化的系统」。这个转变本身产生了一系列**二阶问题**,是功能方向分析无法覆盖的——因为它们不是功能的缺失,而是**资产积累带来的维护负担**。

---

## 方向一:知识质量衰减 —— 记忆系统的「信噪比」危机

### 核心洞察

ForgeOS 的 Memory Engine (`internal/memory`) 是一个**追加日志**,其设计原则是「累积,从不删除」。这在系统早期是正确的——当知识很少时,任何信息都比没有信息好。但随 evolve 循环运行,这个假设开始反转。

### 代码级证据

**证据 1: 本仓的 memory.jsonl 已显示信号稀释**

当前 `.forge/memory.jsonl` 有 14 条记录:

```json
{"kind":"lesson","topic":"evolve","detail":"iter 1: roadmap=75%, gates_green=false (dry-run trajectory)","iteration":1}
{"kind":"lesson","topic":"evolve","detail":"iter 1: roadmap=92%, gates_green=false (dry-run trajectory)","iteration":1}
{"kind":"lesson","topic":"evolve","detail":"iter 1: roadmap=100%, gates_green=false (dry-run trajectory)","iteration":1}
{"kind":"lesson","topic":"evolve","detail":"iter 2: roadmap=100%, gates_green=false (dry-run trajectory)","iteration":2}
{"kind":"lesson","topic":"build","detail":"iter 1: roadmap=100%, gates_green=true (dry-run trajectory)","iteration":1}
...
```

**14 条记录中有 12 条是同一模式:「dry-run trajectory」**。它们记录了 `(iteration, roadmap%, gates_green)` 的瞬时状态,但**没有一条是真实的决策、架构推理、或学习到的教训**。每条记录的 `kind` 都是 `lesson`,但内容只是遥测快照。

这不是系统的 bug——Memory Engine 忠实记录了 agent 写入的内容。但它是**知识质量危机的直接证据**:存储层不区分「高价值信息」(架构决策、安全约束、回顾发现)和「低价值信息」(迭代状态快照),两者都用相同的 `lesson` kind 写入,相同的权重注入 prompt。

**证据 2: Memory 没有质量度量**
```
forge-core/internal/memory/memory.go:
  type Entry struct {
      Format     string  // "forgeos.memory.v1"
      Kind       string  // "decision" | "gap" | "lesson" | "choice" | "insight"
      Topic      string  // free-text topic
      Detail     string  // free-text detail
      Confidence float64 // 0..1, agent's self-assessed confidence
      ...
  }
```

- `Confidence` 字段存在,但它的值来自 agent 自评——不是从客观结果推导的。
- Memory 注入 prompt 时没有按 `Confidence` 加权:**所有条目按文件顺序注入,没有优先级排序**。
- 没有**去重机制**:同一个决策被两个 agent phase 写入两次,或者同一个 gap 在每个 iteration 都被发现一次,`Load` 返回所有副本。
- 没有**矛盾检测**:session 1 写 `"use PostgreSQL"`、session 2 写 `"use SQLite"`,两者共存,没有机制标记为互斥。

```
# Query 是按 topic 的全表扫描,不区分新旧、不区分置信度
func Query(entries []Entry, kind, topic string) []Entry {
    var out []Entry
    for _, e := range entries {
        if (kind == "" || e.Kind == kind) && (topic == "" || e.Topic == topic) {
            out = append(out, e)
        }
    }
    return out
}
```

**证据 3: Memory 注入 prompt 时没有选择性**

```
forge-core/cmd/forge/prompt_memory.go:
  // memoryContext injects ALL memory entries matching the phase's topic into the prompt.
  // As memory grows, this becomes a significant portion of the prompt budget.
```

代码加载**所有匹配 topic 的条目**。没有 Top-K 截断,没有新鲜度加权,没有置信度阈值过滤。如果 memory 增长到 100+ 条目(一次 24h evolve 循环的合理规模),`memoryContext` 会往 prompt 注入大量**低价值状态快照**,稀释关键决策信号。

### 为什么这是二阶缺口(而非已有分析覆盖)

已有分析覆盖了:
- **「内存存储规模演进」**(`high-value-expansion-directions.md §3`):关注的是 O(n)→O(log n) 的性能优化
- **「Partial Failure」**(`expansion-production-readiness.md §5`):关注的是 Load 部分损坏时的优雅降级
- **「Cross-session corrective learning」**(`expansion-horizon-three.md §5`):关注的是人类修正信号注入

**方向一不是上述任何一个。** 它问的是:当 Memory 已经存储了 500、1000、5000 条记录时,系统如何**区分信号与噪声**?如何**阻止低价值信息淹没高价值信息**?如何**在两条矛盾的决策记录中察觉并告警**?

这是「存储规模」和「加载性能」之外的问题:**已存储内容的质量管理**。性能和格式优化做得再好,如果存储的内容本身是噪声,系统也不会变得更聪明。

### 建议的工程方向

1. **知识条目质量评分**——建立一个客观的 `knowledge_score`(非 agent 自评的 `confidence`):基于(引用次数 × 交叉验证数 × 年龄衰减)的启发式。高分的条目优先注入 prompt,低分的延迟或跳过。
2. **去重与合并**——对相同 `(kind, topic, detail)` 的条目做去重;对同一 `topic` 下矛盾的多条 `decision` 标注 `contradiction_detected=true` 并通知人类。
3. **注入选择性**——`memoryContext` 从「注入所有匹配条目」改为「注入 Top-K 按分数排序的条目」。超过 K 的条目只保留计数摘要(`还有 N 条关于 <topic> 的旧记录未注入`)。
4. **知识价值遥测**——记录每条 memory 条目被 `Query` 命中的次数、被注入 prompt 后是否影响了 agent 输出(通过对比注入/不注入的 ROADMAP 完成率变化)。建立知识价值的反馈回路。

### 边界情况

| 场景 | 风险 | 当前处理 |
|------|------|----------|
| Agent 重复写入同一发现(每 iteration 都写「发现需要测试」) | 50 条相同条目,全部注入 prompt,浪费 token | 无去重 |
| 错误的高置信度条目(agent 自信但错了) | `Confidence=0.95` 的错误决策被优先注入 | 无客观验证 |
| 旧条目与新决策矛盾(「用 PostgreSQL」vs「用 SQLite」) | 两者同时注入,prompt 混淆 agent | 无矛盾检测 |
| 有价值条目被低价值条目淹没 | agent 收到 90% 的迭代状态快照 + 10% 的架构决策 | 无优先级排序 |
| 人类直接编辑 memory.jsonl 修正了一条记录 | 文件尾部追加修正行,但旧行仍在,Load 后两者并存 | 无修正传播 |

---

## 方向二:三层配置表面的漂移 —— 声明的 Go / 执行的 Node(Python) / 描述的 YAML

### 核心洞察

ForgeOS 的配置横跨**三个不同层**,各自独立演化:

| 层 | 语言 | 位置 | 职责 | 文件数 |
|------|--------|--------|----------|-------|
| **声明层** | YAML + Markdown | `.agent/policies/`, `.agent/agents/*.md`, `.agent/workflows/*.yml` | 人类可读的配置来源,定义「系统应该做什么」 | ~25 |
| **执行层(Go)** | Go | `forge-core/internal/...`, `forge-core/cmd/forge/...` | 运行时消费配置,定义「系统实际做什么」 | ~195 |
| **执法层(Node/Python)** | JavaScript, Python | `harness/*.mjs`, `harness/*.py` | 独立验证,定义「系统被强制做什么」 | ~25 |

这三层从同一组源文件派生,但**没有机械一致性检查确保它们始终对齐**。

### 代码级证据

**证据 1: `modes.yml` 声明 vs `internal/mode` 实现 vs `check.py` 验证是三条独立维护的路径**

```
# 声明层 — .agent/policies/modes.yml
workflow_depth:
  discover: full|skip       # <-- 这里定义 discover_depth
  design:   full|summary    # <-- 这里定义 design_depth
  review:   full|standard|skip  # <-- 这里定义 review_depth
  evolve:   opportunistic|standard|thorough|advisory  # <-- 这里定义 evolve_depth
  adr:      required|not    # <-- 这里定义 adr_required

# 执行层 — forge-core/internal/mode/mode.go
type Policy struct {
    DiscoverDepth string  // "full" | "skip"   <-- 手工维持与 YAML 一致
    DesignDepth   string  // "full" | "summary"
    ReviewDepth   string  // "full" | "standard" | "skip"
    EvolveDepth   string  // "opportunistic" | "standard" | "thorough" | "advisory"
    ADR           bool    // true | false
}

# 执法层 — harness/check.py
# check_workflow_mode_gating: 手动维护了 authority 值与 modes.yml 的对照表
# 新加一个 depth 维度,check.py 需要手工加一条规则
```

Sprint 31 为 `mode_gating` 加装了漂移守卫(`check_workflow_mode_gating`),但这只是一个**局部的、事后弥补**的检测点——它只检查 workflow YAML 的 `mode_gating:` 声明是否与 `modes.yml` 一致。它不检查 Go 结构体是否与两者一致。

**证据 2: agent 角色卡的职责声明在 markdown 中,Go 代码中无对应**

```
# .agent/agents/cto.md — 声明 cto 的 5 种裁决
## Review 阶段 · executive-review 相位
请在评审结束时输出机读裁决:
VERDICT: APPROVE
VERDICT: APPROVE_WITH_SIMPLIFICATION
VERDICT: REDESIGN
VERDICT: DELAY
VERDICT: REJECT
```

```
# forge-core/cmd/forge/cost.go — Go 代码中硬编码了同样的 5 个 token
func parseExecutiveVerdict(output string) (string, bool) {
    switch {
    case strings.Contains(lastLine, "APPROVE_WITH_SIMPLIFICATION"):
        return "approve_with_simplification", true
    case strings.Contains(lastLine, "APPROVE"):
        return "approve", true
    ...
}
```

两者之间的**契约是文本一致性**——如果 `cto.md` 新增一种裁决(如 `VERDICT: DEFER`),`cost.go` 不会自动感知。没有测试会失败。系统只是静默地不认识这个新裁决,当作 `unmatched` 处理。

**证据 3: harness gate 名称的散布**

```
# .agent/workflows/build.yml — 声明 required_gates
  - gate: lint
  - gate: test
  - gate: build
  - gate: complexity
  - gate: coverage
  - gate: secret
```

```
# forge-core/internal/gate/resolve.go — 同样的 gate 名硬编码在 resolve 逻辑
func GatesGreen(required []string, statuses map[string]string) bool {
    for _, name := range required {
        if statuses[name] != "PASS" { ... }
    }
}
```

```
# forge-core/internal/gate/gate.go — ProbeAll 中同样是这些名称
func ProbeAll(root string) (statuses, categories map[string]string, err error) {
    probeGate("lint", ...)
    probeGate("test", ...)
    probeGate("build", ...)
    probeGate("complexity", ...)
    probeGate("coverage", ...)
    probeGate("secret", ...)
}
```

如果 build.yml 新增一个 `gate: security-audit`,需要**同时修改**:
1. 声明层:build.yml 加一行
2. 执行层:gate.go 加 `probeGate("security-audit", ...)` 实现
3. 执法层:如有新 harness 工具需添加

这三个修改如果不同步,错误的表现是静默的——gate 名不在 `ProbeAll` 中意味着它永远不会被运行,但 `GatesGreen` 在遍历 required gates 时也会跳过未知名称,所以收敛判定可能错误地认为所有门都绿了。

### 为什么这是二阶缺口(而非已有分析覆盖)

已有分析覆盖了:
- **「Governance asset upgrade」**(`expansion-horizon-three.md §4`):关注的是**跨项目**的治理资产版本同步
- **「YAML dual-parser reliability」**(`expansion-production-readiness.md §4`):关注的是两条解析路径的一致性
- **「Configuration drift detection in CI」**:没有被任何已有分析覆盖

**方向二的焦点是逐层之间的静默漂移。** 这不是「项目 A 的 YAML 落后于 forge-core 版本 X」的**跨项目问题**,而是**同项目内三层之间的即时一致性**问题。Sprint 27 的 block-scalar bug 就是三层漂移的典型症状——YAML 解析器(执行层)与 Python 解析器(执法层)产生分歧,而声明层(memory 条目)记录了损坏的数据。三层中只有一层被修正(Go 解析器),但**没有机制能保证其他两层也一致**。

### 建议的工程方向

1. **声明层即真相源(Source of Truth)的强契约**——建立 `forge validate --consistency` 命令,从声明层(YAML)自动生成检查清单,验证执行层(Go struct 字段)和执法层(harness gate 名称)是否全部覆盖。不一致时告警。
2. **agent 角色卡机读契约的自动提取**——从 `.agent/agents/*.md` 中提取 `VERDICT:`/`CONFIDENCE:` 模式,自动生成解析器测试,确保 `cost.go` 中的每一条解析规则都有一条对应的角色卡声明。
3. **gate 注册表**——为所有 gate 建立一个 `internal/gate/registry.go`,包含名称和实现函数的映射。workflow 的 `required_gates` 通过注册表验证(而非字符串比对),未注册的 gate 名在 `forge validate` 时报错。
4. **三层一致性 CI 检查**——在 `.github/workflows/forge.yml` 中增加一个 job,对比 YAML 声明的 gate 列表与 Go 注册表的 gate 列表,不一致时标记 warning(非 blocking)。

### 边界情况

| 场景 | 风险 | 当前处理 |
|------|------|----------|
| 新 agent 角色卡增加了机读裁决但 cost.go 未更新 | 新裁决静默 unmatched,收敛不正确 | `parseExecutiveVerdict` 返回 `("", false)`,下游用默认值(可能不收敛) |
| build.yml 引用了一个 gate 名但 ProbeAll 不认识 | gate 静默跳过,收敛可能假阳性 | `GatesGreen` 只检查已知名,未知名不报错 |
| modes.yml 新增 workflow_depth 维度 | mode.go 未予建模,新模式维度不生效 | `mode.Effective` 不知道如何处理新维度,用默认值 |
| .arch/rules.yaml 的 package.max_files 与 Go 包实际文件数漂移 | 允许的文件数超出架构设计 | `arch-check.mjs` 会报,但只报违规不报漂移原因 |

---

## 方向三:Prompt 模板作为一等代码资产 —— 从硬编码到可管理

### 核心洞察

ForgeOS 的核心价值——操控 agent——完全通过 prompt 模板实现。但所有 prompt 模板是**硬编码在 Go 源文件中的字符串**,分散在三处:

| 文件 | 行数 | 职责 |
|------|------|------|
| `prompt_context.go` | ~500 | 主 prompt 构建:角色卡 + 约束 + 任务 + 上下文 + gate 状态 |
| `prompt_memory.go` | ~150 | memory 条目注入格式化 |
| `prompt_artifacts.go` | ~100 | `uses_template`/`secondary_template` 渲染 |

**约 750 行 prompt 逻辑与 Go 代码锁死在同一个二进制中。** 这意味着:

- 修改 prompt 模板 = 修改 Go 代码 = 重新编译 = 重新部署
- 无法对 prompt 做版本对比(「本次改了什么提示词?」)
- 无法让产品经理或 prompt engineer 独立修改 agent 指令
- 无法 A/B 测试不同 prompt 策略
- 无法在运行时切换 prompt 版本(「如果 agent 输出 quality_score 低,回退到上周的 prompt」)

### 代码级证据

**证据 1: prompt 是字符串拼接,无结构化表示**

```go
// prompt_context.go (约第 200 行)
func buildPrompt(roleCard, constraints, task, adrContext, memoryContext, gateLedger string) string {
    var b strings.Builder
    b.WriteString(roleCard)
    b.WriteString("\n\n")
    b.WriteString("## Constraints\n")
    b.WriteString(constraints)
    b.WriteString("\n\n")
    b.WriteString("## Task\n")
    b.WriteString(task)
    if adrContext != "" {
        b.WriteString("\n\n## ADRs\n")
        b.WriteString(adrContext)
    }
    if memoryContext != "" {
        b.WriteString("\n\n## Memory\n")
        b.WriteString(memoryContext)
    }
    if gateLedger != "" {
        b.WriteString("\n\n## Gate Status\n")
        b.WriteString(gateLedger)
    }
    return b.String()
}
```

这是**纯文本拼接**,没有模板引擎,没有转义,没有结构化表示。每个 lane 的内容预先格式化为字符串,然后按固定顺序拼接。想要调整 lane 顺序、按 mode 切换模板、或注入条件性内容,都需要修改 Go 代码。

**证据 2: lane 顺序是硬编码的,无法按角色/场景调整**

当前顺序是固定的:
```
角色卡 → 约束 → 任务 → ADRs → Memory → Gate状态
```

但不同角色可能需要不同顺序:
- **planner**: 约束优先(规则 > 背景)
- **implementer**: 任务优先(做什么 > 为什么)
- **reviewer**: Gate 状态优先(前序产出 > 规则)
- **explorer**: Memory 优先(已知什么 > 要探索什么)

当前所有角色用同一个 `buildPrompt` 函数。`prompt_context.go` 没有 per-agent 的 lane 排序配置。

**证据 3: 没有 prompt 版本标识**

每一条 trace event 记录了模型、成本、延迟、裁决,但**不记录所使用的 prompt 版本**:

```json
// 来自实际 .forge/trace.jsonl 的事件
{"seq":1,"kind":"iteration","name":"1","status":"ok","duration_ms":0,"detail":"roadmap=100% gates_green=false"}
```

没有 `prompt_hash` 或 `prompt_version` 字段。这意味着:
- 无法追溯「这个 agent 输出是在哪个 prompt 版本下产生的」
- 无法对比不同 prompt 版本对 agent 输出的影响
- Scorecard 的 quality_score 无法归因到 prompt 版本

### 为什么这是二阶缺口(而非已有分析覆盖)

已有分析覆盖了:
- **「Prompt construction pipeline QA」**(`expansion-production-readiness.md §1`):关注的是 prompt **正确性**——渲染结果是否正确、token 是否超限
- **「LLM output contract compliance」**(`expansion-production-readiness.md §2`):关注的是 agent **输出的履约**

**方向三是 prompt 本身的可管理性问题。** 不是「prompt 是否构建正确」,而是「prompt 作为一个代码资产,是否具备与代码同等的管理能力——版本控制、diff review、独立测试、渐进部署」。这是两个不同的关注域:QA 保证**质量**,可管理性保证**演化速度**。

### 建议的工程方向

1. **Prompt 模板外部化**——将 prompt 模板从 Go 字符串提取为独立的模板文件(如 Go `text/template` 或简单的 `.md` 模板文件),存储在 `.agent/prompts/` 目录中。`forge-core` 在启动时加载这些模板,而非编译时锁定。
2. **Prompt 版本指纹**——在 trace event 中增加 `prompt_hash`(模板内容的 SHA256),使每个 agent 输出可追溯到生成它的 prompt 版本。Scorecard 的 quality_score 按 prompt 版本分桶。
3. **Per-agent lane 配置**——为每个 agent 角色声明一个 `prompt_lanes` 配置(顺序 + 启用/禁用 + 截断策略),替代 `buildPrompt` 的固定顺序。这个配置可以放在 agent 角色卡 YAML 头部,或单独的 `prompt_config.yml` 中。
4. **Prompt 快照目录**——在每次 `forge run` / `forge evolve` 时,将最终渲染的 prompt 文本写入 `.forge/prompts/phase_<name>_<seq>.txt`,供事后审查 prompt 实际内容(这是生产就绪度分析的「golden file」方向的轻量替代方案)。

### 边界情况

| 场景 | 风险 | 当前处理 |
|------|------|----------|
| 模板文件缺失或格式错误 | `forge run` 使用上次编译的旧模板或宕机 | 无(当前 Go 硬编码不存在此问题) |
| 多版本模板共存(实验 A/B) | 两条路径用不同模板,产出不可比 | 无版本概念 |
| agent 角色卡更新了职责 | 需同步改 Go 代码中的 prompt 模板 | 契约在角色卡 markdown 中,但 prompt 在 Go 中,无连接 |
| prompt 模板被外部编辑 | 版本控制外修改,不可追溯 | 硬编码在 Go 中,外部无法编辑(但也无法不编译就修改) |
| 按 mode 切换 prompt 风格(explorer 简略 vs engineering 详细) | 当前所有 mode 用同一 prompt | 无 mode 感知的 prompt 配置 |

---

## 方向四:运行时行为的不可重复性 —— 自治循环的「再现性赤字」

### 核心洞察

ForgeOS 的自治循环引入了两层非确定性:

| 层 | 非确定性来源 | 影响 |
|------|------------------------|--------|
| **LLM 输出** | 即使同一 prompt,模型输出每次不同 | agent 产出、裁决,每次 run 不同 |
| **运行时环境** | `time.Now` 在 checkpoint/trace/scorecard 中引入时间依赖 | checkpoint 内容、trace 事件每次不同 |
| **文件系统状态** | `forge run` 前后 memory/trace 文件变化,改变后续 run 的输入 | 相同 Git 状态但 `.forge/` 内容不同,导致不同结果 |
| **迭代顺序** | 并行 phase 的完成顺序依赖系统调度 | 不同顺序导致 feed-forward 内容不同 |

**当前没有任何机制能让两次运行的对比具有可操作性。** 没有 run ID,没有输入固定,没有输出对比框架。

### 代码级证据

**证据 1: 没有运行标识(Run ID)**

```
# 搜索 runid / run_id / session_id
grep -rn "runId\|run_id\|sessionId\|session_id\|RunID" forge-core/ --include="*.go"
# → goroutine.go: RunID — 仅在并行 wave 内部使用,不是全局 run 标识
```

每次 `forge run build` 产生一组新的 `.forge/trace.jsonl` 行,不同运行的 trace 事件在同一个文件中**追加排列**:

```json
// 第一批 evolve 的 trace (seq 1-2)
{"seq":1,"kind":"iteration","name":"1","status":"ok","duration_ms":0}
{"seq":2,"kind":"iteration","name":"2","status":"ok","duration_ms":0}
// 第二批 evolve 的 trace (seq 1-2?? 或 seq 3-4??)
{"seq":1,"kind":"iteration","name":"1","status":"ok","duration_ms":4469}
```

注意第二个 run 的 seq 从 1 重新开始——因为 `Tracer` 是每次 `forge run` 新创建的,seq 不跨 run 持续。这使得 trace.jsonl 中的不同 run 只能靠上下文猜测,没有明确的 run 分隔标记。

**证据 2: checkpoint 不记录输入参数**
```
forge-core/internal/persist/checkpoint.go:
  type Checkpoint struct {
      FormatVersion     string
      Workflow          string
      Mode              string
      Iteration         int
      RoadmapCompletion float64
      GatesGreen        bool
      Reason            string
      UpdatedAtUnix     int64
      PhaseIndex        int
      SpentUsdMicros    int64
  }
```

Checkpoint 记录了**运行时状态**(iteration, completion, gates),但**不记录输入参数**(executor type, agent command, max-iter, max-agent-calls, model override)。因此,从 checkpoint resume 后,系统无法确认是否在用与原始 run **相同的配置**运行。如果用户 resume 时忘了加 `--agent-cmd claude`,resume 会变成 dry-run,但不会被检测或告警。

**证据 3: memory 的 append 模式使「回放」不可能**

即使两次 run 用同一 Git commit、同一命令行参数、同一 workflow 文件,第二次 run 的输入因为第一次 run 在 `.forge/memory.jsonl` 中追加了条目而不同。Memory 是**累积的**,不是**场景绑定的**。没有 `--reset-memory` 或 `--snapshot-memory` 机制来重现相同条件。

```
# 第一次 run: memory 有 14 条 → prompt 包含 14 条 memory 上下文
# 第二次 run: memory 有 14+N 条 → prompt 包含 14+N 条 memory 上下文
# 输出不可比
```

**证据 4: trace event 没有输入参数的记录**

Event 结构体有 `Kind`/`Name`/`Status`/`DurationMs`/`CostUsdMicros`/`Model`,但**没有命令行参数、没有 workflow 输入、没有 mode/lifecycle**。事后分析 trace 时,无法回答「这个 trace 是在什么参数下产生的」。

### 为什么这是二阶缺口(而非已有分析覆盖)

已有分析覆盖了:
- **「Run diagnostics/root cause analysis」**(`high-value-expansion-directions.md §5`):关注的是失败诊断——「为什么这次 run 失败了」
- **「Checkpoint/resume correctness」**(`expansion-production-readiness.md §3` 间接):关注的是 crash 恢复

**方向四是比诊断和恢复更基础的问题:能否在可控条件下复现一次 run。** 如果不能复现,就无法验证 bugfix;无法确认「上次跑出奇怪结果的问题修好了」;无法做 A/B 对比(「新 prompt 比旧 prompt 好多少?」)。诊断工具只能解释已经发生的事,但无法回答「如果我改 X,Y 会变吗?」

### 建议的工程方向

1. **Run ID 和运行记录**——为每次 `forge run` / `forge evolve` 分配 UUID。所有 trace event 携带 `run_id`。Checkpoint 记录 `run_id`。这样 trace 可以按 run 过滤,checkpoint resume 时可以校验 run_id 是否一致。
2. **输入参数 checkpoint**——Checkpoint 扩展 `InputSnapshot` 字段,记录 `(executor, agentCmd, maxIter, maxAgentCalls, modelOverride, agentMaxBudget, mode, lifecycle)`。Resume 时对比当前参数与原始参数,不一致时告警(不阻断)。
3. **Memory 快照/回放**——支持 `forge run --memory-snapshot <label>` 在 memory 中创建命名快照,以及 `forge run --memory-restore <label>` 恢复到快照状态。这使 A/B 测试成为可能:从同一 memory 基线开始,测试两种不同的 prompt 或路由策略。
4. **Trace 格式加入 run_fingerprint**——在 trace event 中增加 `run_fingerprint`,它是(输入参数 + workspace git hash + workflow 文件 hash)的摘要。两次 run 如果 fingerprint 相同,预期行为应该可比较;不同则已知不可比。
5. **`forge diff-runs <id-a> <id-b>`**——一个差异化分析命令,对比两次 run 的 trace/checkpoint/convergence 变化:iteration 数差、成本差、gate 结果漂移、模型分布变化。

### 边界情况

| 场景 | 风险 | 当前处理 |
|------|------|----------|
| Resume 时 executor 从 command 变成 dry | 系统静默地不再跑 agent,但报告「converged」 | 无检测 |
| 两次 run 在不同 Git commit 上 | trace 不记录 commit,无法关联代码变化与行为变化 | 无 |
| 用户想复现「上周三那个奇怪的收敛失败」 | 没有 run_id,只能从时间戳猜测 | trace 只有 seq + 时间,无 run 边界 |
| 并行 phase 的完成顺序不同 | feed-forward 内容不同,下游 phase 接收不同输入 | 无顺序记录 |
| 相同的 Git + 参数但 memory 不同 | 结果不同,但无法区分是因为「memory 积累」还是「LLM 随机性」 | 无 memory 版本绑定 |

---

## 汇总:四方向的价值评估

| # | 方向 | 核心问题 | 当前成熟度 | 实现量级 | 与已有分析的差异度 |
|---|------|----------|------------|----------|---------------------|
| 1 | **知识质量衰减** | 追加日志的「信噪比」危机——存储内容本身是噪声 | `memory.jsonl` 已显示信号稀释(12/14 条是 dry-run 快照) | 中(Query/注入加质量筛选,不改存储格式) | **全新的二阶问题**——不是性能/功能/降级,是内容质量管理 |
| 2 | **三层配置漂移** | 声明层(YAML) vs 执行层(Go) vs 执法层(Python/JS)独立演化 | 无一致性检查,Sprint 27 block-scalar 是典型症状 | 中(一致性校验框架,非大重构) | **全新的二阶问题**——不是跨项目升级,是同项目内的三层对齐 |
| 3 | **Prompt 模板可管理性** | 750+ 行 prompt 硬编码在 Go 二进制中,不可独立版本化/测试/部署 | 无模板外部化,无版本指纹,无 per-agent 配置 | 大(模板外部化 + trace 扩展 + 快照目录) | **全新的二阶问题**——不是 prompt 正确性,是 prompt 的工程化管理 |
| 4 | **运行时不可重复性** | 无 run ID、无输入参数记录、memory 累积不可回放 | trace 无 run 边界,checkpoint 无输入参数,memory 不可快照 | 中(Run ID + trace 扩展 + 参数 checkpoint) | **全新的二阶问题**——不是运行诊断(解释过去),是运行复现(重现过去) |

### 推荐优先级

1. **方向四(Run 可重复性)**——基础投资。没有 run ID 和输入参数记录,所有后续的 A/B 测试、bug 复现、行为审计都建立在猜测上。改动量小(run_id UUID 生成 + trace 扩展 + checkpoint 扩展),但解锁的能力最多。
2. **方向一(知识质量)**——紧迫性随时间线性增长。当前 14 条 memory 记录还不是问题,但一次 24h evolve 循环就能产生 50-100 条。质量门需要在达到「噪声漫灌」阈值之前就位。
3. **方向三(Prompt 可管理性)**——高 ROI 但大改动。prompt 是系统最关键的输入,但当前的管理方式(硬编码 Go 字符串)对 Prompt Engineer 不友好,也不支持渐进部署。模板外部化是架构级的改进,但可分期实施——先从 `prompt_hash` trace 开始。
4. **方向二(三层一致性)**——重要性高但触发频率低。三层漂移的问题在每次新增特性时发生(约每 sprint 0.5-1 次),可以通过纪律(开发者 checklist)在短期内缓解,自动检查可以稍后工程化。

---

## 附录:被排除的方向与理由

| 方向 | 排除理由 |
|------|----------|
| **跨项目治理资产升级**(`expansion-horizon-three.md §4`) | 已在已有分析中完整覆盖 |
| **多维模型路由自动化**(`high-value-expansion-directions.md §1`) | 已在已有分析中完整覆盖 |
| **并行调度引擎激活**(`high-value-expansion-directions.md §2`) | 已在已有分析中完整覆盖 |
| **运行诊断与根因分析**(`high-value-expansion-directions.md §5`) | 已在已有分析中完整覆盖 |
| **事件驱动触发**(`expansion-horizon-three.md §3`) | 已在已有分析中完整覆盖 |
| **多仓库联邦治理**(`expansion-horizon-three.md §2`) | 已在已有分析中完整覆盖 |
| **管线工作流组合**(`expansion-horizon-three.md §1`) | 已在已有分析中完整覆盖 |
| **Prompt 构建管道 QA**(`expansion-production-readiness.md §1`) | 方向三的姊妹问题——QA 是对的补充,不是替代 |
| **LLM 输出契约履约验证**(`expansion-production-readiness.md §2`) | 方向二的姊妹问题——履约验证是契约执行,方向二是契约本身的一致性 |
| **核心链路交互面测试**(`expansion-production-readiness.md §3`) | 独立的质量工程领域,非架构缺口 |
| **YAML 双解析器可靠性**(`expansion-production-readiness.md §4`) | 方向二的子问题——三层一致性的一种体现 |
| **部分故障与优雅降级**(`expansion-production-readiness.md §5`) | 独立的韧性工程领域,非二阶演化问题 |
| **Web UI** | 已确定为 v3,CLI 优先的产品定位不应在此阶段动摇 |
| **Firecracker 沙箱 / LiteLLM 跨厂商池** | 外部阻断(v3),框架已就绪,等触发条件 |
| **Embedding 语义检索** | 镀金,当前 TF-IDF 对现有 corpus 够用 |

---

## 结论:二阶问题的特殊性

ForgeOS 的功能完备性在 Sprint 31 已到达一个里程碑:所有已知的 GAP 已收口,核心循环已端到端坐实,治理工具链已完整。**但系统的「存在」本身开始产生新的问题**——memory 越多,信噪比越低;代码层越多,漂移风险越大;prompt 越复杂,维护成本越高;运行次数越多,不可复现的困惑越多。

这些问题不是「还没做的功能」,而是「做了功能之后自然产生的负产物」。它们不会通过增加功能来解决——恰恰相反,增加功能会让它们恶化。解决它们需要**二阶工程**:建立质量管理机制(方向一)、一致性守卫(方向二)、资产可管理性(方向三)、以及运行可重现性(方向四)。

这些方向的共同特征是:**它们不增加系统的能力边界,但确保系统已有的能力可持续地、可信赖地、可演化地运行**。
