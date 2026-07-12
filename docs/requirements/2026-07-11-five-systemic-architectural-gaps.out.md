# Architecture Review: Five Systemic Gaps Analysis

这是一个非常扎实的 deep-dive。我逐方向给出**架构评审反馈**——不是反驳,而是从不同视角补充一些可能被你低估或遗漏的维度。

---

## 总体评价

**亮点**: 代码证据引用精确到行级,工作量估算基于 sprint velocity 而非猜测,去重验证严谨。这是我见过的对 ForgeOS 体系最系统的缺口分析。

**一个结构性盲区**: 你独立分析了 5 个方向,但没有分析它们之间的**依赖关系**(dependency graph)。有些方向必须先于其他方向落地,否则后者要么做不了,要么做两遍。下面给出依赖图:

```
4. Artifact Verification (P0)
    ├─ 基础: Phase.PhaseOutput 需要有 schema 化的结构
    ├─ 为 3 (冲突检测) 提供 diff baseline
    └─ 为 1 (Workflow 组合) 的 phase 契约提供验证手段

3. 并发冲突检测 (P1)
    └─ 依赖 4: 冲突检测需要知道"每个文件属于哪个 artifact"
       否则无法区分"预期并行修改"和"意外冲突"

2. CLI 能力发现 (P1)
    ├─ 依赖 4: 能力声明的输出格式验证需要 artifact schema
    └─ 依赖 1: 不同 CLI 适合不同 phase 类型,需要 workflow 组合才能表达

1. Workflow 组合 (P1)
    └─ 相对独立,但模板中的条件语法会影响其他方向

5. 分布式骨架 (P2)
    └─ 依赖 3 + 4: 分布式环境下冲突检测和 artifact 验证是刚需
```

**正确的启动顺序**: 4 → 3 → (1+2 可并行) → 5

---

## 方向一 · Workflow 组合 —— 补充

### 一个被低估的复杂度:YAML 继承的语义问题

你提到循环引用检测,但还有更微妙的语义问题:**Phase 覆盖的叠加规则**。

```
# parent.yml
phases:
  - name: review
    required_gates: ["style", "unit"]

# child.yml
extends: parent.yml
phases:
  - name: review
    required_gates: ["security"]  # 是替换?追加?还是只允许减少?
```

三种语义各有使用场景:
- **替换(override)**: child 完全重定义 gate list——简单粗暴,但破坏复用
- **追加(append)**: `["style", "unit", "security"]`——灵活但可能导致 gate 膨胀
- **约束(refine)**: 只允许 child 减少 gate,不能增加——安全但不够灵活

### 另一个被低估的点:YAML 内嵌模板语言的选型

当前所有 workflow 是纯 YAML。如果引入参数化,有三种路线:

| 方案 | 例子 | 优点 | 缺点 |
|------|------|------|------|
| Go `text/template` | `{{.lifecycle}}` | 零依赖,ForgeOS 已有 | 非 YAML 原生,IDE 不友好 |
| YAML anchors/aliases | `<<: *review-template` | YAML 原生 | 不能跨文件,无逻辑 |
| CUE/DCL | `lifecycle: string` | 带类型约束 | 引入新语言,学习成本 |

我的判断:不做模板引擎,而是用**Go 结构体嵌入 + 代码生成**。即在 Go 中定义 workflow 组合逻辑,编译期或启动期生成 YAML。这样既有类型安全,又有逻辑能力,还不引入新的 DSL。

---

## 方向二 · CLI 能力发现 —— 一个被忽略的悖论

### 鸡和蛋问题

你的分析很准确,但有一个根本性的两难:

> ForgeOS 需要 CLI 声明自身能力,然后编排层据此匹配合适的工作。
> 
> 但**不是所有 CLI 都支持"声明能力"这个能力本身**。

一个 CLI 要声明能力,它必须先理解 JSON 格式 + 遵守某种协议。现在 Claude Code 就不支持 `--capabilities` flag——你需要在 ForgeOS 侧有一个静态映射表。

**我的建议**:分层方案——

```
Layer 0: 静态内置映射
  "claude" → {permission_mode: true, model: true, allowed_tools: true}
  "codex"  → {permission_mode: false, model: true, ...}
  "gemini" → {permission_mode: false, model: true, ...}

Layer 1: CLI 自省 (--help 解析)
  运行 `claude --help` → 解析 flag 列表 → 推断能力
  无需 CLI 主动配合

Layer 2: 能力声明文件
  CLI 安装目录下放一个 .capabilities.json
  Claude Code v2 可以附带这个文件

Layer 3: 运行时 negotiate
  CLI 进程启动后,通过 stdin/stdout 交换能力声明
```

Layer 0 可以 1 sprint 完成,Layer 1-2 是 2-3 sprints。你直接估了 2-3 sprints 对应 Layer 1-2 的工作量,这是合理的,但建议标注 Layer 0 作为可选的快速先行步骤。

### 一个被低估的产品问题:用户如何选择 CLI?

即使有了能力发现,用户怎么告诉 ForgeOS"这个 phase 用 Claude,那个 phase 用 Haiku"?

当前 workflow YAML 中 `agent: architect` 引用了一张 agent 卡。如果不同的 agent 卡对应不同的 CLI,那么 CLI 选择是通过 agent 卡间接实现的。这其实够用了:

```yaml
# agent 卡
agents:
  architect-agent:
    cli: claude     # 新增字段
    model: sonnet
  fast-reviewer:
    cli: gemini     # 另一个 CLI
    model: gemini-2.0-flash
```

不需要在 workflow 层暴露 CLI 选择——agent 卡做中间层就够了。这个观点你文档没有展开,我认为值得明确。

---

## 方向三 · 并发冲突检测 —— 最重要的补充

### 你低估了问题的难度

你说 3-4 sprints。我认为**仅文件级冲突检测就需要 3 sprints,再加上语义冲突(接口/实现一致性)可能再加 2-3**。原因:

**核心难点:Pre-execution snapshot 的粒度问题**

```go
// 方案 A: git-based snapshot (你隐含假设的方案)
git add -A && git stash  // 记录当前状态
// run agent
git diff --name-only     // 查看改动

// 问题:agent 自己也会 git add/commit/stash
// → snapshot 和 agent 的 git 操作互相污染
```

ForgeOS 的 agent prompt 中明确指示 agent 使用 `git commit`。这意味着不能依赖 git 做 snapshot——agent 的 git 操作和 ForgeOS 的 snapshot git 操作会冲突。

**正确的方案:文件级 sha256 快照**

```
执行前: walk 整个工作目录 → map[path]sha256
执行后: walk 整个工作目录 → 对比 sha256 → 发现新增/修改/删除
```

这样可以跟 agent 的 git 操作完全解耦。

### 一个被低估的"伪冲突"问题

假设:
- Agent A 修改 `auth/login.go`,添加了 `Login()` 函数
- Agent B 修改 `auth/middleware.go`,添加了 `AuthMiddleware()` 函数
- 两个文件都 import `"net/http"`——Agent A 加了,Agent B 也加了

文件级 diff 显示:两个文件都修改了,但 `"net/http"` 这个 import 两个 agent 都加了——这不算冲突,重复 import Go 编译器会处理。

但如果是 Python:
- Agent A 在 `__init__.py` 中添加了 `from .login import Login`
- Agent B 在同一文件同一行附近添加了 `from .middleware import AuthMiddleware`
- 如果 B 先写,A 后写,后写的 A 覆盖了 B 的改动——**这是真正的冲突**

**冲突检测需要理解文件格式**。Go 可以用 import 分区块来解决,但 Python 的 import 在同一区域内。这意味着:
- 对于 Go:冲突阈值较低(import 可追加)
- 对于 Python:冲突阈值较高(import 覆盖)
- 对于 YAML/JSON:完全不能并发写结构化文件

这不是纯文件级 diff 能解决的。我建议做成**插拔的冲突策略**:

```go
type ConflictStrategy interface {
    // 检测两个 diff 是否冲突
    Detect(base, a, b map[string][]byte) []Conflict
    // 如果可能,合并两个 diff
    Merge(base, a, b map[string][]byte) (map[string][]byte, error)
}

// Go 实现:宽松策略,只检测同一函数体的修改
// Python 实现:严格策略,检测 import 区域 + 函数体
// YAML 实现:最严格,任何修改都算冲突
```

这把你估计的 3-4 sprints 推到 5-6 sprints。

---

## 方向四 · Artifact 验证 —— P0 正确

### 唯一补充:验证的层次

你提到"不验证产物是否存在"。但验证不是二元的(存在/不存在),我认为有**五个层级**:

| 层级 | 验证内容 | 示例 | 现任谁做 |
|------|---------|------|---------|
| L0 | 进程 exit code | exit 0 | ✅ `CommandExecutor` |
| L1 | 末行 token | VERDICT: APPROVE | ✅ `cost.go` |
| L2 | 产物存在性 | `docs/adr/xxx.md` exists | ❌ 无人 |
| L3 | 产物结构 | ADR 包含 Status/Context/Decision | ❌ 无人 |
| L4 | 产物与声明一致 | ADR 中的 Decision 与 implementer 的承诺匹配 | ❌ 无人 |

当前系统覆盖 L0-L1。L2 是你说的"文件是否存在"。L3 是"格式是否正确"。L4 是"内容语义一致"——比如 implementer 承诺了三个功能点,ADR 中只写了两个。

**我建议 P0 只做 L2-L3**。L4 需要自然语言理解,用 LLM-as-judge 可以但会增加成本和延迟,建议留到 P1。

### 一个实际的验证案例

当前代码中,`asset.Phase.Emits` 的值是:

```go
// discover.yml 中的评审 phase
Emits: ["assessment-matrix", "faceted-classification", "dependency-map", "reference-architecture"]
```

这些字符串是自由文本。如果加上验证,需要将它们映射为具体的文件 glob 或目录结构:

```yaml
Emits:
  - name: assessment-matrix
    path: "docs/discovery/assessment-matrix.md"
    schema: "assessment-matrix"  # 引用 schema 定义
    required: true
  - name: faceted-classification
    path: "docs/discovery/faceted-classification.md"
    schema: "classification"
    required: true
```

这需要一个 `ArtifactSchema` 注册表——与 `harness/adapters` 模式一致。你提到"延续 adapters 模式",这是对的,但没有展开具体怎么做。我建议将 artifact schema 做成跟 gate adapter 类似的注册机制:

```go
// harness/adapters/artifact.go 新增
type ArtifactValidator interface {
    Name() string
    Validate(path string, content []byte) (bool, []Violation)
}
```

---

## 方向五 · 分布式骨架 —— 一个重要的渐进路径

### 你提到 4-6 sprints,但我认为可以更务实

你的 Phase 1-5 是连跨式升级。但如果组织只需要"进程 crash 不丢状态"和"远程执行",实际上可以跳过 Phase 4(coordinator/worker 拆分)直接走前三个阶段:

**最小可行分布式**:3 sprints

| Sprint | 交付 | 改动量 |
|--------|------|--------|
| 1 | 持久化抽象: `Store` interface + S3/GCS backend for trace + checkpoint | 新增 2 个文件,改 3-4 个现有文件 |
| 2 | Engine 状态序列化 + 原子 checkpoint(含 memory/trace 的快照) | 改 checkpoint.go 和 memory.go |
| 3 | 远程 executor: SSH backend for CommandExecutor | 新增 remote_executor.go,不破坏本地路径 |

Phase 4(coordinator/worker 拆分)是架构级别的变动,建议留到 v3 明确需要的时候再做。过早拆分会引入分布式状态管理、选举、心跳等复杂度,而你可能只需要"远程跑 agent"这个能力。

### 一个被低估的细节:`.forge/` 分支隔离

你提到"同一 repo 的两个分支不能并行运行 evolve,因为它们共享 `.forge/`"。这是一个很痛的现实问题,但不是分布式才能解决——**给 `.forge/` 加一个 workspace 命名空间**即可:

```go
func forgeDir(root string, workspace string) string {
    return filepath.Join(root, ".forge", workspace)
}
```

默认 workspace 是 `default` 或从 branch name 派生。这 50 行代码就可以解决,不需要分布式架构。建议纳入方向五的范围,作为一个 quick win。

---

## 对其他方向间连接的洞察

### 方向 2 与方向 4 的交集:capability 声明的 schema 复用

如果你的 artifact 验证框架定义了 schema 注册表,那么 CLI 能力声明可以用同一套机制:

```yaml
# claude-capabilities.yaml (由 ForgeOS 维护,非 CLI 提供)
schema: capability-declaration
capabilities:
  permission_mode: true
  model_flag: "--model"
  allowed_tools: "--allowedTools"
```

artifact schema 注册表 → 能力声明 schema → CLI 能力发现。同一套代码,两个用途。

### 方向 1 与方向 3 的交集:phase 契约

Workflow 组合中,如果 phase A 引用了一个模板,该模板 `Emits` 了某个文件——但并发执行时另一个 phase 也 Emits 了同一个文件——冲突检测需要知道"这两个 phase 是否共享预期的输出契约"。

这指向一个下层抽象:**Phase Contract**——一个 phase 声明了"我要读什么,我要写什么,我的成功标准是什么"。有了 Phase Contract,工作流组合、冲突检测、artifact 验证就统一了。

### 方向 3 与方向 4 的深层连接

如果方向 4(artifact 验证)先落地,方向 3 会从中受益:
- Artifact schema 可以提供**冲突检测的格式感知上下文**——知道哪些部分是安全并行修改的
- Artifact 的存在性检查本身就可以在并行执行中做预检

所以依赖顺序 `4 → 3` 不仅是架构层面的建议,也是实现层面的自然路径。

---

## 我对优先级的温和质疑

### P0 合理性确认 ✅

方向 4 是 P0。没有 artifact 验证,自治运行是自我循环的信任幻觉。我完全认同。

### P1 中,我建议调整方向 2 和 3 的优先级

你的表格中方向 2(P1)和方向 3(P1)是同一优先级。但我认为:

**方向 3(并发冲突检测)应该先于方向 2**:
- `--parallel` 已经存在并在文档中被宣传为功能,但它不安全——这是一个**断言的 bug**,不是增强
- 方向 2(CLI 适配)是一个"好要有"的功能——目前 Claude 跑得很好,多 CLI 接入是扩展性,不是正确性
- 依赖关系上,冲突检测需要 artifact schema(方向 4),CLI 适配也需要 artifact schema

建议:方向 4 → 方向 3 → (方向 1 + 方向 2 并行)

### P2 中的方向 5,应该拆出 quick win

方向 5 的 4-6 sprints 让整个方向看起来很难切入。但实际上可以拆两个增量:

```
Quick win (0.5 sprint): workspace 命名空间 + 原子 trace 写入
MVP (2 sprints):        Store interface + S3 backend + 远程 executor (SSH)
Future (3-4 sprints):   Coordinator/Worker 拆分 + 多 root 编排
```

建议标注 Quick win,让组织可以在 1 周内获得"进程 crash 不丢 memory"的能力。

---

## 总结:一个被忽略的"第零方向"

你的五个方向都 valid,但我认为存在一个**更基础的缺口**:Phase Contract 抽象。

当前:
- `asset.Phase` 定义了名字、agent、Emits——但没有定义"这个 phase 的输入是什么、成功标准是什么、它的副作用是什么"
- `converge.Signals` 有 `RoadmapCompletion` 和 `GatesGreen`——但没有"此 phase 是否完成了它承诺的输出"
- `prompt.Build` 构建 prompt——但没有"针对此 phase 的输入/输出契约做结构化注入"

如果把 Phase Contract 作为一个独立的设计文档先写出来,它会成为方向 1(组合)、方向 3(冲突)、方向 4(验证)的统一基础。这是一个**架构前置设计工作**,不是编码工作,但可以节省后续 3-5 sprints 的重构成本。

建议:在 sprint 规划中、在启动方向 4 之前,插入一个 **Phase Contract ADR** 作为方向 0。

---

以上就是我的完整评审。整体来说这是一份质量极高的分析——代码证据扎实,工作量估算有上下文,去重验证严谨。我的补充主要集中在:依赖关系梳理、复杂度低估、渐进路径、以及 Phase Contract 这个被我视为"方向 0"的缺失拼图。
