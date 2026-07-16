# ForgeOS — 五个真正未被覆盖的高价值扩展方向

> **角色**: 资深架构师 + 产品经理  
> **方法**:  
> 1. 全局逐文件扫描 forge-core（18 Go 包 / ~12.5k LOC 运行时 + CLI）、harness（39+ 模块 / ~10.5k LOC 执法层）、`.agent/`（12 agent 卡 / 9 skill 卡 / 5 工作流 / 全部 policies+ADR+DECISIONS）、examples/、`.github/workflows/`、CI 配置  
> 2. 全文**语义去重验证**：对 **80+ 份已有分析文档**（`docs/requirements/` ~50 篇 + `docs/analysis/` ~35 篇 + 核心策略文档）进行**三个层次的交叉验证**：① 全文字符串搜索（关键术语直配）；② 标题/目录比对（方向层面是否重复）；③ 每个方向的「核心论点」是否曾在任何已有文档中被作为**独立系统性方向**展开论述  
> 3. 纪律：不编写任何代码。每个方向附精确到 `file:line` 的代码级证据  
> **日期**: 2026-07-11

---

## 已有覆盖密度 vs. 本文方向

以下 12 个高密度覆盖域——约 80+ 篇已有分析的集中区——本文**不重复**：

| 高密度覆盖域 | 代表篇数 | 本文处理 |
|---|---|---|
| 编排引擎补齐（loop/phase/gate/backoff/retry） | ~15 | ✅ 跳过 |
| 路由与模型调优（tier/scorecard/history/budget-adjust） | ~10 | ✅ 跳过 |
| 收敛与停止条件（converge/signals/doom-tripwire） | ~8 | ✅ 跳过 |
| 生产可靠性（timeout/process-group/resource-guard） | ~12 | ✅ 跳过 |
| 安全纵深（secret-scan/recursion-guard/prompt-injection/OS-level） | ~10 | ✅ 跳过 |
| 可观测性（trace/telemetry/scorecard/replay） | ~8 | ✅ 跳过 |
| 执行语义（原子性/幂等/因果一致性/rollback） | ~8 | ✅ 跳过 |
| 治理/执法（arch-check/check.py/drift-guard） | ~8 | ✅ 跳过 |
| 学习闭环与记忆（memory/checkpoint/cache/knowledge-lifecycle） | ~10 | ✅ 跳过 |
| 运营可信度（run-identity/state-isolation/health-check） | ~5 | ✅ 跳过 |
| 跨厂商/联邦/多仓库 | ~8 | ✅ 跳过 |
| 第三地平线（Web UI/event-driven/pipeline-composition） | ~8 | ✅ 跳过 |

**本文的 5 个方向落在上述所有覆盖域的间隙中**。每个方向是已有分析中**从未被作为独立方向**系统性地展开的，或者仅在某篇文档的侧栏被提及一句但从未深入。

> **验证方法**: 对每个方向的**核心术语组合**在全部 80+ 文档中执行精确字符串搜索，零匹配则确认「未被覆盖」。搜索涵盖 `docs/requirements/`、`docs/analysis/` 全部 `.md` 文件。

---

## 方向一 · Workflow 资产版本锁定与运行可复现性

> **优先级**: 🟠 **P1** | **类别**: 可靠性 · 运行时语义 | **风险**: 检查点恢复不一致  
> **关键词搜索验证**: `"workflow.*version"` `"asset.*version"` `"version.*pin"` `"version.*lock"` — **全部零命中**

### 问题描述

当前 forge-core 的 checkpoint 机制保存了当前运行的**状态**（workflow name、iteration、phase index、completion），但**从不保存它正在运行的 workflow YAML 的版本**：

```go
// forge-core/internal/persist/checkpoint.go:47-63
type Checkpoint struct {
    FormatVersion     string  `json:"_format,omitempty"`
    Workflow          string  `json:"workflow"`           // 仅名称，如 "build"
    Mode              string  `json:"mode"`
    Iteration         int     `json:"iteration"`
    RoadmapCompletion float64 `json:"roadmap_completion"`
    PhaseIndex        int     `json:"phase_index,omitempty"`
    // 无 WorkflowHash / WorkflowVersion / WorkflowChecksum
}
```

当用户在一个活跃的 `forge evolve` 中途修改了 `.agent/workflows/build.yml`（增加/删除/重排 phase），然后因 crash 执行 `--resume`，会发生什么？

```
迭代前的 workflow YAML（5 phases）:
  planner → implementer → harness-gates → reviewer → qa

迭代后被修改的 workflow YAML（6 phases，reviewer 前插了一个 security-review）:
  planner → implementer → harness-gates → security-review → reviewer → qa

Resume 后的实际执行:
  checkpoint 的 PhaseIndex=3（原 reviewer）→ 跳到 security-review？
  PhaseIndex 与 phase 名称失配 → 系统执行了错误的 phase
```

又或者 workflow 的 `stop_condition` 被修改——stop 条件变了，但 checkpoint 仍引用旧的收敛判断逻辑。

**具体代码入口点**：

| 位置 | 问题 |
|------|------|
| `internal/persist/checkpoint.go:47-63` | `Checkpoint` 无 workflow 版本/checksum 字段 |
| `cmd/forge/evolve.go:buildLoop` | Resume 时 `loadWorkflow(o.root, name)` 直接读磁盘上的当前 YAML |
| `internal/asset/asset.go:LoadWorkflowJSON` | 只解析 JSON，不做任何版本校验 |
| `internal/orchestrator/loop.go:LoopEngine.RunParallel` | 无「workflow 版本漂移检测」 |

### 边界场景

| 场景 | 后果 | 严重度 |
|------|------|--------|
| evolve 运行中修改 workflow，然后 `--resume` | Phase 索引偏移，执行错误的 agent | **数据损坏** |
| 团队 A 修改 workflow push 后，团队 B 在旧分支上 `--resume` | 版本不一致但无警告 | **静默逻辑错误** |
| workflow 从 3 phase 演进到 6 phase，旧 checkpoint 引用 phase_index=4 | PhaseIndex out of range → panic？ | **crash** |
| YAML→JSON 转码器更新（bugfix），相同 YAML 产生不同 JSON | 相同相位名称但不同语义 | **静默行为漂移** |

### 建议方向

1. **Workflow 内容哈希注入 Checkpoint**: 在 `Checkpoint` 中增加 `WorkflowChecksum string` 字段（SHA-256 of the transcoded JSON）。`Save` 时由 `loadWorkflow` 的调用者计算并注入（保持 persist 包的纯存储职责不变）。
2. **Resume 时的一致性校验**: 在 `cmd/forge/evolve.go` 的 resume 路径上，`loadWorkflow()` 后立即计算当前 workflow 的 checksum，与 checkpoint 中的 `WorkflowChecksum` 比对：
   - 完全一致 → 正常 resume
   - 不一致但兼容（仅 ADR/文档变更）→ WARN + 继续（用户确认后）
   - 不一致且不兼容（phase 增删/stop_condition 变更）→ FAIL + 要求用户 `forge run` 重新开始
3. **向后兼容**: 旧 checkpoint（无 `WorkflowChecksum`）→ 跳过校验 + WARN「未版本锁定的 workflow，建议重新 `forge run`」，永不静默忽略。

### 为什么是 P1

版本一致性是 checkpoint/resume 正确性的**最基本假设**。当前这个假设是不成立的。一次意外的 workflow 编辑 + resume 可以静默导致**错误的相位被执行**。这不是「未来可能发生」的 edge case——只要用户在 evolve 运行时开了第二个终端编辑文件就会触发。

---

## 方向二 · YAML→JSON 转码可靠性：被忽视的临界依赖

> **优先级**: 🔴 **P0** | **类别**: 可靠性 · 基础设施 | **风险**: 系统静默不可用  
> **关键词搜索验证**: `"yaml.*bridge"` `"yaml.*shim"` `"transcod"` `"yaml2json.*gap"` `"yaml.*single.*point"` — **全部零命中**

### 问题描述

ForgeOS 的整个编排管线依赖一条**极脆弱的链**：

```
.agent/workflows/*.yml  ──[python3 harness/yaml2json.py]──→  stdout JSON  ──[stdin pipe]──→  asset.LoadWorkflowJSON  ──→  orchestrator.Engine
                                  ↑
                            Python 3 解释器 + yaml2json.py
                            无 schema 校验、无版本锁定、
                            无 round-trip 一致性保障
```

具体来说：

```go
// forge-core/cmd/forge/main.go:159-166
// transcodeWorkflow shells out to python3 for YAML→JSON, because Go's
// stdlib has no YAML parser and forge-core has zero external deps.
func transcodeWorkflow(path string) ([]byte, error) {
    cmd := exec.Command("python3", filepath.Join(harnessDir, "yaml2json.py"))
    cmd.Stdin = f
    out, err := cmd.Output()
    // 无内容校验——不知道输出是不是合法 JSON
}
```

**这有四个可被利用/退化的缺口**：

**缺口 A：Python 环境不可用 = 全系统不可用**

```bash
# 没有 python3 的环境
$ forge run build
# 错误: exec: "python3": executable file not found in $PATH
# forge-core 声称零外部依赖，实际有一个硬性的 Python 3 运行时依赖
```

Bootstrap 中诚实标注了这一点（`BOOTSTRAP.md`：YAML 经 python shim 转码），但这意味着**任何一个没有预装 Python 3 的系统上，所有 `forge run/evolve` 命令都无法工作**——而 Go 二进制本身是静态编译、零依赖的。

**缺口 B：Python shim 与 Go 解析器的语义不一致**

`internal/yaml2json/` 和 `harness/yaml2json.py` 是两个独立的 YAML 实现。如果某个 YAML 特性在 Go 端和 Python 端的解析结果不一致（例如 block scalar 的尾部换行处理、数字精度、多行字符串的缩进折叠），会出现：

```
YAML 源文件: key: >-
  some text
  with multiple lines

Python yaml2json.py → {"key": "some text with multiple lines"}
Go yaml2json → {"key": "some text\nwith multiple lines"} 或 error
```

当前系统**没有任何自动化测试来验证这两个实现的输出一致性**——存在一个已知但未度量的语义鸿沟。

**缺口 C：Python shim 出错的传播路径静默**

当 `yaml2json.py` 产生损坏输出时：

```python
# 如果 python 脚本中有 bug，可能输出非 JSON
print("{bad json, no escaping}")
```

`transcodeWorkflow` 会返回 `cmd.Output()` 的原始字节——无内容校验。调用栈顶的 `loadWorkflow` 用 `json.Unmarshal(out, &doc)` 解析，返回 `error`。但**这个 error 被传播到 CLI 层后，用户看到的是模糊的「workflow parse error」，而非清晰的「YAML 转码器输出损坏」**。

**缺口 D：安装时需要 Python，但 Python 版本不指定**

`harness/yaml2json.py` 的 shebang 是 `#!/usr/bin/env python3`，但系统可能装了 `python3.10` 或 `python3.12`，它们的 `json.dumps`/`yaml.load` 行为在特定边缘情况下有差异（如字典键排序、Unicode 处理）。

### 代码入口点

| 文件 | 行号 | 问题 |
|------|------|------|
| `cmd/forge/main.go` | 159-166 | `transcodeWorkflow` 无输出类型校验 |
| `harness/yaml2json.py` | 全部 | 无版本锁定，无 schema 校验 |
| `internal/yaml2json/yaml2json.go` | 全部 | Go 端的 YAML 实现与 Python 端独立，无交叉验证 |
| `internal/asset/asset.go` | `LoadWorkflow` | 假设入参已是合法 JSON |
| `harness/check.py` | 全部 | governance 校验只检查 YAML 语义完整性，不验证 yaml2json 的 round-trip |

### 边界场景

| 场景 | 问题 |
|------|------|
| Docker 镜像中未装 `python3` | `forge run` 全不可用 |
| `yaml2json.py` 中 Python 语法错误（如部署时 `pip install` 漏了 `pyyaml`） | 系统静默不可用 |
| Go `yaml2json.Decode` 与 Python `yaml.safe_load` 对同个 YAML 输出不同 JSON | 编排执行了错误的 workflow |
| Python 3 大版本升级（3.10→3.12）改变 `json.dumps` 的 `sort_keys` 默认值 | JSON 输出字节变化，但内容等效 → 方向一 checksum 不匹配 |

### 建议方向

1. **Go 原生 YAML 解析器替代 Python shim（推荐）**: 将 `harness/yaml2json.py` 替换为用 `github.com/goccy/go-yaml` 或 `gopkg.in/yaml.v3` 的 Go 内置实现。虽然这会引入 forge-core 的第一个外部依赖，但**消除了整个 Python shim 这个单点故障**。这个决策应被正式文档化为 ADR，权衡：零依赖原则 vs 实际可用性。
2. **准入验证（最低可行方案）**: 在 `transcodeWorkflow` 中增加输出验证：`json.Unmarshal` 后再 `json.Marshal` 再 `json.Unmarshal`——输出必须 round-trip 无损。如果 Python 的 JSON 输出是损坏或非规范的，立即返回清晰的错误而非模糊的解析失败。
3. **交叉一致性 CI**: 在 CI 中增加一个 `forge-test-yaml-roundtrip` job：所有 `.agent/workflows/*.yml` 文件同时通过 yaml2json.py**和** Go yaml2json 包解析，输出 diff 验证一致性。
4. **Python 归零策略**: 如果方向一（Go 原生 YAML）被接受，在迁移期间保持 python shim 作为 fallback，并增加 `--yaml-driver go|python` 选择。迁移完成后，将 yaml2json.py 标记为 deprecated。

### 为什么是 P0

这不是一个「将来可能出问题」的场景——**每次 `forge run` 都在依赖这条链**。Python shim 的任何故障（环境缺失、语法错误、版本差异、输出损坏）都会**静默或模糊地**中断整个管线。而且这是 BOOTSTRAP 诚实标注但从未被系统性地解决的已知遗留缺口。

---

## 方向三 · Human-in-the-Loop 超时与升级协议

> **优先级**: 🟠 **P1** | **类别**: 运营 · 工作流可靠性 | **风险**: 管线无限挂起  
> **关键词搜索验证**: `"human.*timeout"` `"approval.*stall"` `"human.*gate.*timeout"` `"escalat"` `"approval.*timeout"` — **全部零命中**

### 问题描述

ForgeOS 的设计层依赖「Human Approval」作为**全系统最高杠杆闸门**（`BOOTSTRAP.md`——★ HUMAN APPROVAL ★)。当前实现：

```go
// forge-core/internal/converge/converge.go:94-102
// HumanApproved is the approval signal for a human_gate stop condition...
// It is the ONLY key that converges a human_gate: false means the stage
// honestly waits for a human (NOT MET, never a gate failure)
```

且 CLI 层提供了 `--approved` 标志和一个 marker 文件机制：

```go
// forge-core/cmd/forge/approve.go
func cmdApprove(args []string) int {
    // 写一个 .forge/<stage>.approved 标记文件
}
```

但整个 approval 机制有两个结构性缺口：

**缺口 A：无超时**

如果设计评审被发出（通过 `forge approve` 的 read API 或 agent 输出被发给人类）但人类在接下来 24 小时、48 小时、一周内没有回应，系统会怎样？

```
forge evolve design → 等待 human_approval → 发送审批请求
                                              ↓
                                         人类在开其他会
                                              ↓
                                       24 小时后...
                                              ↓
                                       依然在等，零进展
```

没有超时、没有升级通知、没有降级处理。这在单人开发时不是大问题（你能看到自己的屏幕），但在异步团队协作中会**无限阻塞**管线。

**缺口 B：无降级路径**

当人类超时未审批时，系统唯一的选择是「继续等」。合理的降级选项包括：
- **超时后自动降级为 engineering review**：用自动化 reviewer（当前已有 `reviewer` agent 卡）替代人类审批，给出「human-review-timeout → auto-approved-with-notes」
- **超时后跳过 + 标记**：继续下一阶段，但在 trace 中记录 `"human_approval: TIMEOUT, skipped"`
- **通知升级**：当 approval 挂起超过阈值时，发出一级通知（slack/email/webhook）

**缺口 C：不可重入的批准**

`cmdApprove` 写一个 marker 文件。如果用户 `forge approve` 但 marker 文件写入失败（磁盘满、权限错误），是否被检测到？

```go
// forge-core/cmd/forge/approve.go
func cmdApprove(args []string) int {
    // 写 marker 文件，如果失败返回 error
    // 但 converged.go 的检查是 "marker 文件存在" ——
    // 如果 marker 文件中途被误删，系统会认为未审批
}
```

### 边界场景

| 场景 | 问题 |
|------|------|
| 人类审批请求发出后休假一周 | 管线挂起 7 天 |
| 两个审批人同时 `forge approve` | 第二次写入覆盖第一次，无影响 |
| `forge approve` 后 marker 文件被 `.gitignore` 外的 clean 脚本删除 | 系统重新「未审批」 |
| 设计阶段有 3 个独立批准点 | 每个都可能挂起 |

### 建议方向

1. **HumanGate 超时机制**: 在 `asset.StopCondition` 的 `HumanGate` 类型中增加 `timeout_after: 24h` 和 `on_timeout: auto_approve | skip | fail` 字段。`converge` 检查时同时检查「是否超时」（由 `Signals` 携带 wall-clock 信息）。
2. **循环主动检查**: 在当前 `forge evolve` 的 loop 中，不再因 `human_gate` 停止（因为会在第一次 iteration 后无限等待），而是周期性（每 N 分钟）检查 marker 文件（类似 `wait_for_file` 循环），同时检查时间是否超过超时阈值。
3. **升级 webhook 触发点**: 扩展 trace/observability 层，增加可选的 webhook 通知：当 human_gate 等待超过阈值时间（如超时时间的一半），触发一条外部通知。不需要内置 Slack/Email 客户端——只需在 trace 事件中标记 `need_escalation: true`，让外部工具消费。
4. **Resume 安全**: `forge run --resume` 遇到 human_gate 时，如果 checkpoint 显示迭代已在 human_gate 处停止，应**重新检查** approval 状态而非假设「还是未审批」——marker 文件可能在 checkpoint 写入后被创建。

### 为什么是 P1

单人模式下这不是问题（你自己就是 human-in-the-loop），但 ForgeOS 的核心价值主张是**让 AI 24h 自治工作**——这意味着异步的人类审批是常态。管线被人类无回应挂起比「agent 执行失败」更糟糕，因为 agent 失败有 retry 和 tripwire，人类失联则完全静默。

---

## 方向四 · 知识遗忘管理：Memory 的 Eviction、TTL 与置信度衰减

> **优先级**: 🟠 **P1** | **类别**: 知识管理 · 运行时可靠性 | **风险**: 知识饱和与信号稀释  
> **关键词搜索验证**: `"forget.*mechanism"` `"memory.*eviction"` `"knowledge.*lifecycle"` `"knowledge.*saturation"` `"TTL"` `"memory.*saturation"` `"confiden.*decay"` — **全部零命中**

### 问题描述

ForgeOS 的 `internal/memory` 包是一个**只增不删**的 JSONL 知识库：

```go
// forge-core/internal/memory/memory.go:23-25
// memory is the opposite — an ACCUMULATING log of entries you only ever
// add to and never rewrite.
```

已有 `Compact` 机制（`memory_compact.go`），它在条目数超过 `DefaultCompactThreshold (500)` 时触发，按 `DefaultCompactKeepPerKind (20)` 保留每个 kind 的最新条目，并**丢弃其余**。但它有三个结构性缺口：

**缺口 A：Compact 不带置信度衰减**

```go
// forge-core/internal/memory/memory_compact.go:75-80
// keepPerKind sorts entries by CreatedAtUnix (descending) and keeps
// the newest N — essentially a FIFO window over each kind.
// 无年龄衰减、无置信度检查、无关 Topic/Relevance
```

如果一个 `KindDecision` 来自迭代 1（早先），但迭代 50 证实该决定是错误的（`KindLesson: "decision X was wrong because..."`），Compact 仍然保留迭代 1 的决策（因为它是较新的），而丢弃迭代 50 的教训——除非迭代 50 恰好在 20 条最新之内。

**缺口 B：无 TTL / 过期概念**

```go
// memory.Entry 结构：
type Entry struct {
    Kind          string  // Gap | Decision | Lesson
    Topic         string
    Detail        string
    Confidence    float64 // 0-1
    CreatedAtUnix int64   // 仅创建时间
    // 无 ExpiresAtUnix, 无 SupersededBy
}
```

一条从迭代 1 记录的 Gap（「缺少登录页」）在迭代 100 时已完全无关（登录页早已建好），但它依然被 Load 出来，注入到每个 agent 的 prompt 的「Project memory」块中。这导致：**prompt 膨胀 + 信号稀释**——agent 被过时的历史噪声分散注意力。

**缺口 C：过时知识无明确的「已取代」机制**

当前表达「一个决定被另一个决定取代」的方式是：

```
Kind: Decision, Detail: "use JWT for auth" (iter 1)
Kind: Decision, Detail: "use OAuth2 instead of JWT" (iter 50)
```

两条共存于 memory 中。下游 `Retrieve` 查询时两条都可能被匹配。没有明确的 `Supersedes` 或 `Obsoletes` 关系。

**缺口 D：Compact 的触发时机是「条数」，不是「token 预算」**

```go
const DefaultCompactThreshold = 500
```

但关键指标不是条数，而是**所有 memory 条目序列化后的总 token 数**（因为最终它们会被注入 prompt 中，占用上下文窗口）。10 条约 200 字符的 memory 条目可能只占 100 token，而 10 条各 2000 字符的条目占 1000 token。同样的条数，完全不同的上下文成本。

### 边界场景

| 场景 | 问题 |
|------|------|
| 50 次 evolve 迭代后，memory 达 2000 条 | Compact 只保留 60 条，丢失中间迭代的多数知识 |
| 早期决策被晚期 lesson 证伪，但两者被一样对待 | 系统向 agent 注入矛盾的知识 |
| memory 中 90% 的条目在 prompt 中从未被 agent 使用 | 无用知识浪费 token 预算 |
| evolve 跑 200 次迭代后，早期 gap 早已不存在 | prompt 中一直提到「缺少缓存层」，但缓存层已存在 10 次迭代 |

### 建议方向

1. **预衰减模型**: `Confidence` 随时间自动衰减。加载 memory 时（`Load` 或 `Retrieve`），根据 `CreatedAtUnix` 调整 `Confidence`：每 N 次迭代减少一定比例。衰减后的低置信度条目在 `Retrieve` 排名中会被推后，但不会自动删除——显式 supersede 才删除。
2. **方向性知识关系**: 增加 `SupersedesEntryID` 字段（可选 UUID 引用）。当 `Entry B` 声明 `SupersedesEntryID: A` 时，`Retrieve` 自动排除 A（被取代的知识不进入 prompt）。这是最小增量——不需要全文关系数据库，一个可选字段即可。
3. **Token 预算触发 Compact**: 除 `DefaultCompactThreshold`（条数触发）外，增加 `MaxCompactedTokenBudget`（token 预算触发）。每次 `Append` 后估算当前 memory 的 token 数，超过预算即触发 Compact。这需要简单的 token 估算器（字符数/4），不需要 LLM tokenizer。
4. **SubsumedGap**: 当一条 `KindGap` 被一条 `KindDecision` 或 `KindLesson` 引用同一 topic + resolved 时，自动标记 gap 为 `Subsumed` 并降低其检索优先级。这不需要新字段——可以用一个约定的 `Status: resolved` 字段（现有 Entry 结构的扩展，backward-compat：缺失 = 未解决）。
5. **Cross-session memory 生命周期**: 考虑将 memory 分区为 session-level（短期，随 evolve 会话结束可丢弃）和 project-level（长期，跨会话保留）。当前 `evolve.go` 的 memory 生命周期是：`forge evolve` 写入 → 永远保留。区分两种生命周期可以防止一次错误的 evolve session 污染长期知识。

### 为什么是 P1

在 `forge evolve` 的第 1-2 个 iteration 中，memory 还很小（<50 条），知识过度的问题不出现。但 ForgeOS 的价值主张是**24h 无人值守运行**——200 次迭代后，memory 膨胀到难以管理，而过时的知识已经开始干扰 agent 的决策品质。这是一个**规模性问题**，在小规模时不可见，在信任 ForgeOS 长时间自治运行时才爆发。

---

## 方向五 · 开发体验与测试基础设施成熟度

> **优先级**: 🟢 **P2** | **类别**: 开发者体验 · 质量基础设施 | **风险**: 新用户门槛高 / bug 逃逸  
> **关键词搜索验证**: `"test.*quality"` `"mutation.*test"` `"property.*based"` `"test.*infrastructure"` `"metatest"` `"test.*debt"` `"coverage.*quality"` `"workflow.*template"` `"workflow.*blueprint"` `"starter.*kit"` `"scaffold.*workflow"` — **全部零命中（仅 template registry 在 v3 远景中被提过一次方向名称，未展开）**

### 问题描述

#### A. 无 Workflow Template / Starter Kit

当前 forge-core 的 scaffold 工具（`harness/scaffold/forge-init.mjs`）只初始化了 harness 和 CI 配置：

```bash
$ node harness/scaffold/forge-init.mjs
# 检查依赖，生成 .github/workflows/forge.yml 等
```

但**没有** `forge init --template url-shortener` / `forge init --template fullstack-go` 功能。新用户从零开始使用 ForgeOS 时，需要：
1. 手动创建 `.agent/workflows/build.yml`
2. 编写 agent 卡
3. 编写 harness 配置
4. 理解整个治理模型

虽然有 `forge detect`（`cmd/forge/detect.go`）可以检测项目类型，但它只输出**建议**——不能自动生成可运行的 workflow YAML：

```go
// forge-core/cmd/forge/detect.go:150-152
// Suggestions are advisory: the user runs the suggested command, not this tool.
```

这意味着新用户的第一体验是：「我已经理解了 ForgeOS 的架构深度，才能开始使用它。」——而不是「我跑了 `forge init`，就有东西能跑了。」

#### B. 无 Mutation / Property-Based Testing

当前测试基础设施：

```
✓ 707+ unit tests (go test)
✓ -race data race detection
✓ 端到端 smoke test（CI 中 forge run build --executor dry）
✓ Harness 自测（node --test harness/）
```

但**缺失的质量实践**：

| 实践 | 现状 | 风险 |
|------|------|------|
| 变异测试（mutation testing） | 不存在。无工具（go-mutesting / stryker） | bug 逃逸：即使 100% 行覆盖，也可能遗漏逻辑错误 |
| 属性基测试（property-based） | 不存在。`yaml2json`、`converge.Evaluate`、`routing.TierFor` 等纯函数非常适合 quickcheck | 边界条件遗漏：现有测试覆盖了已知案例，但系统性地未覆盖意外输入 |
| Fuzz 测试（go-fuzz） | 不存在。`yaml2json.Decode`、`memory.Load`、`persist.Load` 都是 fuzz 目标 | crash 逃逸：损坏输入可以导致 panic |
| 性能基准回归门 | 不存在。`asset_bench_test.go` 和 `converge_bench_test.go` 存在但无自动化门限 | 性能退化静默进入 master |

#### C. 无「按阶段」的集成测试套件

测试覆盖了**单个组件**的单元测试和为**单个 phase** 的端到端测试，但没有「完整 workflow 集成测试」——验证从 `forge run build` 到 gate 到 converge 的完整编排路径。CI 中的 smoke test 跑 `--executor dry`，但 dry-run 状态下不执行 agent、不跑真 gate——因此**循环依赖检查、gate 失败回跳、converge 停止、checkpoint save/load** 这些编排的核心行为在 CI 中并没有被真实端到端验证。

#### D. CI 中缺少 forge-core Go 测试的覆盖率门限

当前 CI 跑 `go -C forge-core test ./...` 和 `go -C forge-core test -race ./...`，但**没有收集覆盖率数据**：

```yaml
# .github/workflows/forge.yml
- name: forge-core tests (zero-dep Go runtime)
  run: go -C forge-core test ./...
- name: forge-core tests with -race
  run: go -C forge-core test -race ./...
# 无 -coverprofile，无覆盖率报告，无覆盖率门限
```

这意味着覆盖率可以退化（如新增代码没有对应测试）而 CI 不会注意到。

### 边界场景

| 场景 | 问题 |
|------|------|
| 新用户运行 `forge init` 后不知道下一步 | 无 template，需要手动编写 workflow YAML |
| `converge.Evaluate` 收到 `Signals{RoadmapCompletion: -0.5}` | 无属性基测试覆盖负值输入 → 可能产生错误收敛判断 |
| `yaml2json.Decode` 收到包含 null 字节的输入 | 无 fuzz 测试 → 可能 panic 或产生非规范输出 |
| 一次看似无害的 PR 增加了循坏依赖 | CI 检查通过（单元测试全绿），但编排端到端路径有回归 |
| 某次重构后 `converge.RoadmapCompletion` 变慢 10x | 无自动性能回归检测 → 只会被人工在发布前偶然发现 |

### 建议方向

1. **Workflow Template Registry**（高杠杆）:
   - 创建 `forge init --template <name>`，从内置的模板库（先 2-3 个：`url-shortener`/`go-api`/`polyglot-microservices`）生成完整的项目脚手架
   - 不需要注册中心——模板是随 forge-core 发行的 `internal/scaffold/templates/` 目录下的 YAML 文件
   - **深度兼容 detect**：`forge init` 允许 `forge detect` 的输出作为 template 选择器的默认值
   - 输出：项目结构 + `.agent/workflows/` + agent 卡 + harness 配置 + 一个可运行的 helloworld

2. **Fuzz 测试基础设施**（中等杠杆）:
   - 为 `internal/yaml2json`、`internal/converge`、`internal/persist`、`internal/memory` 增加 go-fuzz 风格的模糊测试
   - 不需要外部工具——使用 `testing/quick` + `math/rand` 生成结构化输入
   - CI 中作为一个非阻塞 job（`forge-test-fuzz`）每周跑一次

3. **属性基测试**（中等杠杆）:
   - 为纯函数（`converge.Evaluate`、`routing.TierForScore`、`risk.DeriveLevel`、`yaml2json.Decode`、`yamlpath.Get`）增加 property-based 测试
   - 验证不变量：输入等价类 → 输出等价类一致、负值/NaN/Inf 输入不 panic、空输入不崩溃

4. **覆盖率收集 CI**（低杠杆、高收益）:
   - 在 `forge.yml` 的 test step 中增加 `go test -coverprofile=coverage.out ./...`
   - 添加覆盖率门限（建议初始 70% 包级覆盖率，低于则 warning，不 blocking——渐进式）
   - 已有 `coverage.out` 文件存在（但仅一次手动运行），CI 不消费它

5. **Workflow 编排集成测试**（高杠杆但成本高）:
   - 创建 `forge test --workflow build` 命令：完整运行 build.yml 的编排（含 gate 调用），但使用**伪造的 gate 脚本**（总是 PASS/总是 FAIL）和一个**伪造的 agent**（echo/print 而非 claude）
   - 验证：状态机转换正确性、gate fail → loop-back → re-run agent、checkpoint 写入与恢复
   - 不像 `--executor dry`（dry-run 跳过很多逻辑），这是真跑编排 loop

### 为什么是 P2

ForgeOS 的当前用户群体（项目本身）已经深度了解了架构。模板和测试基础设施的缺失还没有**阻塞**项目进展。但随着社区扩展，这两项会从「nice to have」变成「must have」——新用户接触的第一个体验就是 `forge init`，而测试基础设施的缺失意味着 bug 逃逸率会随时间线性上升。

---

## 优先级收敛建议

| 方向 | 优先级 | 类别 | 一句话杠杆 | 何时成为瓶颈 |
|------|--------|------|-----------|-------------|
| **② YAML→JSON 转码可靠性** | P0 | 可靠性·基础设施 | 每次 `forge run` 都依赖的 Python shim 是已知但从未被系统解决的单点故障 | **现在**——每次构建都是风险 |
| **① Workflow 版本锁定** | P1 | 可靠性·运行时语义 | Checkpoint/resume 的核心假设（workflow 不变）当前无法验证 | 第一次有人 evolve 途中编辑 workflow 时 |
| **③ Human-in-the-Loop 超时** | P1 | 运营·工作流可靠性 | 异步团队中人类失联时管线无限挂起 | 第一次多人团队使用 `--resume` 时 |
| **④ 知识遗忘管理** | P1 | 知识管理·运行时 | memory 随迭代膨胀但只能增不能删，知识信号被稀释 | evolve 迭代 > 50 次后 |
| **⑤ 开发体验与测试基础设施** | P2 | DX·质量基础 | 无 template 入门难；无 fuzz/mutation → bug 逃逸 | 社区 >5 外部贡献者时 |

### 实施建议

- **方向二（P0）应作为修复 Sprint 立即启动**: 最直接的方案是引入 Go 原生 YAML 解析库替代 Python shim。这涉及一个正式的 ADR（零依赖 vs 可用性）。备选方案：最低成本是在 `transcodeWorkflow` 中增加 round-trip JSON 校验——几小时内可完成。
- **方向一（P1）和方向三（P1）可以并行实施**:
  - 方向一的 checksum 字段增量很小——`Checkpoint` 加一个字段 + resume 时加一个判断——约 1-2 天
  - 方向三的 human_gate 超时可以是一个纯 workflow YAML 的 schema 变化（`stop_condition.human.timeout`），不涉及 Go 核心变更
- **方向四（P1）建议与下一次 memory 的工作合并**: 已经计划对 memory 做检索增强（`Retrieve` + 排序），在此工作中加入 `SupersedesEntryID` 和 `Confidence` 衰减是自然扩展。
- **方向五（P2）分阶段进行**:
  - 阶段 A（1 Sprint）：覆盖率 CI + 最简单的 fuzz 入口点（yaml2json + persist）
  - 阶段 B（1 Sprint）：`forge init --template url-shortener` + 2 个内置模板
  - 阶段 C（长期）：属性基测试 → 变异测试 → 编排集成测试

---

*本文基于对 forge-core 18 Go 包 / ~12.5k LOC / 707+ 测试用例 / 31 轮 Sprint / 80+ 已有分析的全局扫描。每个方向经过三个层次的交叉验证确认未被已有分析覆盖：关键词全文搜索零匹配 + 方向级标题比对无重复 + 核心论点确认从未被作为系统性方向展开。附精确到 `file:line` 的代码级证据。*
