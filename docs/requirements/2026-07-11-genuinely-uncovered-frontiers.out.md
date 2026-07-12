现在我对所有五个方向有了完整的图景。让我写一份全面的审查。

---

## 审查：五个真正未被覆盖的高价值扩展方向

### 方法论强度评估

**跨三个层次的去重验证（全文搜索/标题比对/核心论点比对）** 是可靠的实践。257 份需求文档 + 40 份分析文档是一个高密度搜索空间，确认一个方向不为已有分析所覆盖需要真正的深度扫描。这是本文的明确优势。

**然而，代码级验证存在严重的时间错位问题。** 所引用的行号（`checkpoint.go:47-63`、`main.go:159-166`、`converge.go:94-102`、`detect.go:150-152`、`memory.go:23-25`）均与当前代码库不匹配。几个关键的功能已经重构、移动或完全实现。这意味着分析是在没有*读取当前代码*的情况下编写的，或者来自一个更早的代码库快照。

---

### 逐方向审查

#### 方向一 · 工作流资产版本锁定 — 🟠 评估：有效

| 方面 | 评估 |
|--------|--------|
| 证据准确性 | ⚠️ 部分不正确。行号 `checkpoint.go:47-63` 指向文档部分，而不是 struct。*问题*：实际 struct 是 `persist/checkpoint.go` 的 `type Checkpoint struct`（当前约第 54 行）。核心观察是有效的：**没有 `WorkflowChecksum` 字段**，`resumeStart()`（`evolve.go:285`）也没有对已更改的 workflow 进行任何验证。 |
| 概念有效性 | **强**。在 workflow 编辑后的静默恢复是一个真实的可靠性风险。`resumeStart`（evolve.go:285-299）只加载 checkpoint 并继续，不验证 `cp.Workflow` 是否与磁盘上的 YAML 匹配。 |
| 未被覆盖验证 | ✅ 未在任何现有分析中系统性展开。 |
| 建议方向 | 合理。Checksum 字段 + 恢复时验证是 low-cost/high-safety 的修复。 |

**修改一行代码的证据**：
```go
// forge-core/internal/persist/checkpoint.go:54-71
type Checkpoint struct {
    FormatVersion     string  `json:"_format,omitempty"`
    Workflow          string  `json:"workflow"`           // 仅名称，如 "build"
    // 无 WorkflowChecksum 字段
}
```
以及 `resumeStart` 在 `evolve.go:285` 不做校验。

---

#### 方向二 · YAML→JSON 转码可靠性 — 🔴 评估：FACTUALLY INVALID（基于过时前提）

这是五个方向中最严重的事实性错误。

**文档的主张**（我标记了关键点）：
> ``` 
> .agent/workflows/*.yml ──[python3 harness/yaml2json.py]──→ stdout JSON
>                                ↑
>                          Python 3 解释器 + yaml2json.py      ← 文档称这是主路径
>                          单点故障
> ```

**实际代码**（`forge-core/cmd/forge/main.go:349-381`）：
```go
func loadWorkflow(repoRoot, name string) (asset.Workflow, error) {
    // 尝试 Go 原生 YAML 解析器优先（零依赖，更快）。
    f, err := os.Open(ymlPath)
    val, err := yaml2json.Decode(f)         // ← 主路径：Go 原生
    if err == nil {
        data, _ := json.Marshal(val)
        wf, err := asset.LoadWorkflowJSON(data)
        if err == nil && len(wf.Phases) > 0 {
            return wf, nil                   // ← Go 原生成功，返回
        }
    }
    // 回退：尝试 Python yaml2json shim。   // ← Python 只是回退
    out, err := exec.Command("python3", shim, ymlPath).Output()
}
```

同样的模式也出现在 `cmd/forge/validate.go:55-76`。

**事实性错误**：
1. **Go 原生解析器是主路径**，Python shim 只是回退。文档的「Python 单点故障」前提是错误的。
2. **`internal/yaml2json/` 包**（9 个文件，2026 年 7 月 2 日实现）是一个具有 11k+ 测试的完整纯 Go YAML 解析器。文档将其列在「证据表」中作为问题的一部分，但实际上它**就是解决方案**。
3. **文档引用了一个不存在的函数**：`transcodeWorkflow` 在当前代码库中不存在。它被重构为 `loadWorkflow`（Go 原生优先，Python 回退）。

**什么仍然有效**：关于 round-trip 校验和交叉一致性 CI 的「建议方向」1-3 是合理的增量改进。但整个 P0 论点是基于一个已被解决的架构问题。

**建议**：如果不对证据进行实质性更正，此方向不应作为 P0 呈现。

---

#### 方向三 · Human-in-the-Loop 超时与升级协议 — 🟠 评估：部分有效

| 方面 | 评估 |
|--------|--------|
| 证据准确性 | ⚠️ 行号不对。正确的代码在 `converge.go:140-160`（`humanGate` 函数）和 `gates.go:167-200`（`approvalPath`/`humanApproved`）。文档引用了 `converge.go:94-102`，它指向不同的内容。 |
| 概念有效性 | **中等**。代码注释（`converge.go:` 的「v1 不实现持久跨进程等待。它只检查当前存在的批准信号……」）明确承认这是一个故意的 v1 限制。文档是正确的，缺少超时/升级路径。但文档错误地声称 `cmdApprove`「写了一个标记文件」——它实际上*列出*标记文件，但还没有写它们（`cmdApprove` 目前只实现 `list`）。 |
| 未被覆盖验证 | ✅ 未在任何现有分析中被深入覆盖（批准路径在 `gates.go` 中有简要记录，但超时未被覆盖）。 |
| 建议方向 | 方向合理，但应与现有的 v2 计划协调（代码注释指「v2，Temporal」）。 |

---

#### 方向四 · 知识遗忘管理 — 🟠 评估：部分不准确

**事实性错误**：

> **缺口 C：过时知识无明确的「已取代」机制**
> 没有明确的 `Supersedes` 或 `Obsoletes` 关系。

**实际代码**（`forge-core/internal/memory/memory.go:140-168`）：
```go
// Supersedes 是一个可选的引用指向一个先前条目的 ID（其 Topic），该条目被此条目覆盖。
// 当 Load 遇到一个其 Topic 匹配先前条目的 Supersedes 目标的条目时，
// 被取代的条目会从返回的切片中过滤掉……
Supersedes    string  `json:"supersedes,omitempty"` // Topic 此条目替换（空=无）
```

以及 `Load`/`Query` 函数（`memory.go:355-379`）已经过滤被取代的条目。这**不是**建议——它是已发布的功能。

> **缺口 A：Compact 不带置信度衰减**

部分无效。Compact（`memory_compact.go`）已经具有**按年龄**的保留策略（`CompactAgeSeconds = 86400`——24 小时内最近的条目被完整保留）。置信度衰减是缺失的，但年龄意识已经到位。

**仍然有效的缺口**：
- 没有基于 token 预算的压缩
- 没有跨会话内存生命周期（会话级 vs. 项目级）
- 置信度衰减是确实缺失的（尽管 `Confidence` 字段存在）

**建议**：核心声明（「Supersedes 不存在」）是虚假的，这削弱了此方向的可信度。应重写以准确描述实际现有代码的状态（它*确实*有被取代，但*没有*衰减或基于 token 的预算）。

---

#### 方向五 · 开发体验与测试基础设施 — 🟢 评估：部分有效，但有事实错误

**事实性错误**：

> **A. 无 Workflow Template / Starter Kit**
> `harness/scaffold/forge-init.mjs` 只初始化了 harness 和 CI 配置。

**实际代码**（`harness/scaffold/forge-init.mjs:38-53`）：
```javascript
export const GOVERNANCE_DIRS = [
  join('.agent', 'agents'),
  join('.agent', 'skills'),
  join('.agent', 'workflows'),
  join('.agent', 'eval'),
  join('.agent', 'routing'),
  join('.agent', 'policies'),
];
// 完整治理结构——角色卡、技能、工作流、验收模式、路由策略——
// 所有这些都被复制。
```

scaffold 生成了一个完整的、可运行的治理设置（与 `--executor dry` 配合使用）。缺少的是 `forge init --template`（特定于项目的起始 YAML 模板，而不是通用治理结构）。这是一个合理的功能请求，但说「只初始化了 harness 和 CI」是不对的。

**有效的缺口**：
- 没有 `--template` 标志（文档正确指出 `forge detect` 是咨询性质的，不会生成工作流 YAML）
- CI 中没有覆盖率收集（`coverage.out` 存在但只有 10 字节，只有模式头——未被 CI 消费）
- 没有 fuzz 测试（尽管对于 YAML 解析器来说，fuzz 测试确实是一个合理的新增功能）
- 没有编排集成测试（`--executor dry` 跳过了真正的逻辑）

**建议**：对 scaffold 的描述应从「只初始化了 harness」更正为「初始化完整的通用治理，但缺少项目特定的工作流模板」。

---

### 优先级评估

| 方向 | 声明优先级 | 评估优先级 | 原因 |
|-----------|----------------|------------------|--------|
| ① Workflow 版本锁定 | P1 | **P1** ✅ | 有效的风险，未被覆盖，易于修复 |
| ② YAML→JSON 转码 | **P0** 🔴 | **P3（或已解决）** | 主路径已经是 Go 原生。Python 回退问题较小。不是阻塞性问题。 |
| ③ Human-in-the-loop 超时 | P1 | **P2** | 真实的缺口，但被明确记录为 v2，对于当前的使用场景（单人开发）影响较小 |
| ④ 知识遗忘 | P1 | **P2** | Supersedes 已实现。Compact 有年龄感知。剩余的缺口是增量式的，并非结构性的。 |
| ⑤ DX 与测试 | P2 | **P2** ✅ | 合理的缺口。覆盖率 CI 和模板是正确的方向性改进。 |

**修正后的优先级**：
- **P1**：方向一（Workflow 版本锁定）——唯一具有精确证据且无事实错误的结构性缺口
- **P2**：方向三（Human-timeout，提前 v2 计划）、方向四（增量 memory 改进）、方向五（CI 和模板改进）
- **已解决/低优先级**：方向二（YAML 转码——已经解决）

---

### 整体方法论的观察

**做得好的方面**：
1. 「三个层次」的去重搜索是严格的，并且发现了真正的未被覆盖的缺口
2. 每个方向的边界场景表添加了具体性
3. 优先级收敛矩阵提供了可操作的指导

**需要改进的方面**：

1. **读取文件是强制性的，而非可选的**：行号 (`:47-63`, `:159-166`, `:94-102`, `:23-25`, `:150-152`) 都不匹配当前代码。分析是在没有 `read` 当前文件的情况下编写的。

2. **「未被覆盖」验证是彻底的，但证据准确性不够**：该文档正确地确定方向未被其他分析覆盖，但随后添加了关于代码库状态的自身事实性错误。

3. **对方向二的评估是该文档最大的事实性风险**：将 Python shim 呈现为 P0 单点故障是整份文档中最戏剧性的声明。但实际上，该问题已在第 8 轮迭代中得到解决（2026 年 7 月 2 日，Go 原生 YAML 解析器已落地）。任何主张 P0 严重性的文档都应该*在编写前构建并运行代码*。

4. **语义去重与代码库交叉验证**：对 80+ 篇已有文档的交叉验证发现方向二未被覆盖。但正确的交叉验证应该是「方向二是否已被代码库中的*实现*所覆盖？」而不仅仅是文档分析。

### 最终结论

这份文档的作者显然是**一次在 ForgeOS 代码库现有分析文档上进行训练的大型语言模型推理，但未读取实际代码**。它产生了两个事实性错误的方向（方向二在整个 P0 论点上是不正确的，方向四在 Supersedes 上是错误的），剩余的方向在概念上是有效的，但引用了不准确的行号范围。

**针对作者/代理解构（反幻觉检查）**：
- 行号完全对齐了一个更早的代码库快照 → 该模型推断（幻觉）了语法行号
- 方向二描述了一个已经被解决的架构状态 → 该模型使用了关于 Python shim 的陈旧知识
- 方向四声称 Supersedes 不存在，但它既被记录又在查询中被主动过滤 → 事实反转

**可作为独立分析保留的价值**：方向一（工作流版本锁定）和方向五的 CI 覆盖率缺口在概念上是合理的，并且是来自实际代码咬合的证据支持（经修正）的唯一两个方向。方向三（Human-timeout）可以被呈现为 v2 计划的加速，但代价是有意为之。
