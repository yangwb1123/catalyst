# ForgeOS — 高价值扩展方向 (全局代码库扫描)

> **视角**: 资深架构师 / 产品经理  
> **方法**: 全局源码扫描 + 运行路径追踪, 不编写任何代码  
> **日期**: 2026-07-10  
> **基础**: forge-core Go 运行时 18 包 + harness Node/Python 工具链 + .agent 声明式治理

---

## 概述

本次扫描基于对 `forge-core/` (Go, 18 包)、`harness/` (Node/Python, ~30 文件)、
`pi-batch.py` (Python 批处理)、`.agent/` 声明层和 `docs/` 已有分析的全量阅读。
现有文档已覆盖大量方向 (Sprint 1-31, 数百页分析), 但仍有几个**真正未被触及**
的高价值缺口。以下五个方向每个都是:

1. **当前代码库中存在可验证的"声明但未消费"或"部分实现"的证据**
2. **有明确的用户/系统价值 (避免损失或解锁新能力)**
3. **有可行的实现路径 (非镀金/非架构外)**

---

## 方向 1: 运行时自适应上下文窗口管理

### 现状

`internal/prompt` 包的 Context Engine 使用三个固定上限:

| 资源 | 上限 | 位置 |
|------|------|------|
| ADR 注入条数 | `adrTopK=6` | `prompt.go:39` |
| ROADMAP 注入长度 | `taskCap=4000` (rune) | `prompt.go:44` |
| AGENTS.md 约束行数 | `leadingBullets(..., 6)` | `prompt.go` |

这些上限是**诚实但静态的**: 项目只有 4 个 ADR 时 `adrTopK=6` 意味着"全注入",
但当项目增长到 20 ADR 时, 6 条 BM25 最相关可能不够传达上下文。
反之, 同为 6 条但实际只匹配到 2 条相关时, 注入窗口浪费了 4/6。

同时, `adrTitles` 在 `Gather` 中重新 `ReadDir` 并 `firstHeading` 读取每个 ADR 文件,
但 `cache.go` 的 `ContextCache` 只缓存了 `[]Doc` (标题), 没有缓存 ADR 正文内容。
当 `Retrieve` 做 BM25 时, 它只匹配标题——ADT 的正文内容从未参与相关性计算。

### 证据

```go
// forge-core/internal/prompt/prompt.go:39-44
const adrTopK = 6   // 固定, 不随项目 ADR 数量变化
const taskCap = 4000 // 固定, 不随可用上下文窗口变化
```

```go
// forge-core/internal/prompt/prompt.go:86-97
func relevantADRs(repoRoot, query string) []string {
    titles := adrTitles(repoRoot)   // 只读标题, 不读正文
    ...
    docs := make([]Doc, len(titles))
    for i, t := range titles {
        docs[i] = Doc{ID: t, Text: t} // Text = 标题, 非正文
    }
    ...
}
```

`Retrieve` (BM25) 因此只用标题做相关性评分, 正文中更详细的技术描述被浪费。
`ContextCache` 没有缓存 ADR 正文, 因此即使要升级也是每次重新读文件。

### 为什么需要

1. **项目规模扩展时的保真度**: 一个 20 ADR 的项目, `adrTopK=6` 且只匹配标题,
   意味着大量相关技术决策的正文内容不会进入 prompt。Agent 可能在不知道已有决策
   的情况下做出与之矛盾的实现——这直接违反 ForgeOS 的"不让架构腐化"核心承诺。

2. **上下文窗口的经济性**: Claude 的上下文有成本 (prompt tokens ≈ 价格)。
   注入不相关的 ADR 浪费 tokens; 漏掉相关的 ADR 浪费 agent 工时。
   没有自适应机制, 随着项目增长浪费呈线性增长。

3. **缺失的反馈闭环**: 系统无法知道注入的上下文是否被 agent 实际使用。
   `trace` 记录 gate 结果, `converge` 评估停止条件——但没有"prompt 效率"的度量。
   不知道"我们的 ADR 注入是有帮助还是噪声"。

### 建议方向

- ADR 正文全文索引 (不只是标题), 用正文做 BM25 相关性计算
- 动态 `adrTopK`: 根据项目 ADR 总数和可用的上下文预算计算 (而不是固定 6)
- 优先级分桶: 高影响 ADR (如架构决策) 强制注入, 低影响 ADR (如工具选择) 检索注入
- Agent 使用的隐式反馈: gate 结果好的迭代 → 当前上下文策略有效; gate 结果差的迭代 →
  上下文可能不足 → 下次扩大检索范围 (这需要对 `OnGateResult` 回调的增强)

---

## 方向 2: 运行时风险感知编排 (风险信号的主动消费)

### 现状

`internal/risk` 包包含完整的风险分类器:

- `risk.Classify(Signals)` — 将声明的特征映射到 `low|medium|high|critical` 四个等级
- `risk.FromChangedPaths(paths)` — 从文件路径启发式推导风险特征
- `risk.Higher(a, b)` — 取更严等级 (auto 与 manual 的合并)

但这些能力**只被 `forge route --from-git` 这个手动 CLI 命令消费**。
在真正的编排路径 (`orchestrator.Engine`, `LoopEngine`) 中, 风险信号完全不存在。

具体缺口:

| 位置 | 应该做 | 实际做 |
|------|--------|--------|
| `RunFrom` / `RunParallel` | 高风险 → 强制 reviewer 相位 + Opus tier | 完全忽略风险 |
| `buildPrompt` | 注入风险上下文 → agent 知道"你在改支付代码" | 完全没有风险信息 |
| `gatesFor` / mode gating | 高风险 → 不能跳过 reviewer (override `optional_for`) | mode gating 只看 mode+lifecycle |
| `ProcessRun` (cost.go) | 高风险 → 预算守卫更保守 | 预算守卫与风险无关 |

### 证据

```go
// forge-core/internal/risk/risk_diff.go:15-20 (header 自认)
// FromChangedPaths ... 它读取变化文件的路径, 用路径子串启发式推导风险特征
// ... 
// BY DESIGN THIS ONLY RAISES RISK, NEVER LOWERS IT.
```

但全仓搜索 `risk.FromChangedPaths` 的消费者:

```
$ grep -r "FromChangedPaths" forge-core/
forge-core/cmd/forge/route.go:   sig, reasons := risk.FromChangedPaths(paths)
forge-core/internal/risk/risk_diff.go: func FromChangedPaths(paths []string) ...
```

**只有一个消费者: `cmd/forge/route.go`**, 而且 `route` 是纯手动 CLI。
编排路径 (`cmdRun` / `cmdEvolve`) 完全不接触风险。

更根本的是, `asset.Phase` 没有任何风险声明字段。即使编排想用风险,
它也没有注入点——没有 "这个 phase 的变更是否触及 payment/auth/secret" 的标识。

### 为什么需要

1. **安全合规的根本要求**: 如果 agent 正在修改支付代码 (从文件路径可知),
   当前编排不会做任何特别处理。reviewer 可能被 mode 跳过 (explorer mode),
   tier 可能被预算守卫降级 (near-budget → Haiku), gate 可能被缩减。
   这是**真实的安全缺口**: 高风险变更在低戒备模式下通过。

2. **风险信号的生产者已经就绪, 消费者缺失**: `risk.FromChangedPaths` 是纯函数,
   零外部依赖, 已经测试覆盖。唯一缺失的是一根连线: 在 `cmdRun`/`cmdEvolve` 中
   调用它, 将结果注入 `Engine` 的 mode gating / tier 选择 / prompt 注入。

3. **预算守卫的盲区**: 当前 `BudgetAdjustTier` 只看 `spendRatio` 和 `opusFloorAgents`。
   一个 near-budget 的高风险非 floor phase (如 implementer 改支付代码) 会被降级到 Haiku,
   但 Haiku 做高风险工作的质量风险远高于省下的 $0.10。

### 建议方向

- 在 `cmdRun`/`cmdEvolve` 中集成: 跑 `risk.FromChangedPaths(git diff)` 并在构建
  `Engine`/`LoopEngine` 时将结果传入
- `Engine` 新增 `RiskLevel` 字段: 高风险时 `gatesFor` 不能缩减, `mode_gating` 的
  `optional_for` 被覆盖为全执行, `PhaseTier` 的 lower bound 被提升
- `buildPrompt` 注入风险上下文: "⚠ 本次变更触及支付代码, 请特别关注安全正确性"
- 远期: `asset.Phase` 增加 `risk_sensitive` 声明字段, workflow 作者可以标记
  哪些 phase 需要风险感知

---

## 方向 3: 并行 phase 产出的冲突检测与合并契约

### 现状

`RunParallel` (parallel.go + waves.go) 支持 dependency-wave 并发:

- 波内独立 phase 同时运行
- Fail-fast: 第一个失败取消整个波
- 无 directed loop-back
- 无 per-phase checkpoint

但有一个根本性的缺口:**两个 concurrent phase 写了同一个文件时, 没有检测, 也没有合并策略**。

具体场景: 假设波 2 包含 `market-research` 和 `capability-analysis` 两个 phase,
它们都声明了 `emits: [docs/discovery/analysis.md]` (同一路径)。
在 `RunParallel` 下:

1. Phase A 打开 `docs/discovery/analysis.md` 写入 10KB 市场分析
2. Phase B 同时打开同一个文件, 写入 8KB 能力评估
3. **无原子写入**: Go 的 `os.WriteFile` 不是原子操作, 两个进程的交织写入
   可能产生混合内容 (行交错、截断——取决于具体写入模式)
4. **即使不交织**: 最终结果是 A 或 B 之一完全覆盖另一个, 数据静默丢失
5. **下游 phase** (如 `product-designer`) 读到的分析文件是残缺的

### 证据

```go
// forge-core/internal/orchestrator/parallel.go:80-125
// runPhaseParallel 每个 phase 独立运行:
func (e Engine) runPhaseParallel(ctx context.Context, ...) error {
    ...
    return e.runAgentPhase(ctx, p, mode)  // agent 写文件
}
```

`asset.Phase.Emits` 已经在 schema 中 (asset.go:107):
```go
Emits []string `json:"emits,omitempty"`
```

但 `parallel.go` 完全不读也不消费 `Emits`。
没有冲突检测, 没有写入协调, 没有后验校验。

```go
// forge-core/internal/orchestrator/waves.go
// Waves 只做拓扑排序, 不做资源冲突分析:
func Waves(phases []asset.Phase) ([][]int, error) {
    // 只检查 depends_on 图的合法性
    // 不检查两个 phase 是否写同一个 emits 路径
}
```

### 为什么需要

1. **并行编排的正确性前提**: 如果并发 phase 不能安全地写文件, 并行模式的
   实际可用性为零——用户不敢打开 `--parallel`, 因为产出的损坏是静默的、
   非确定性的 (只出现在特定时序下 → Heisenbug)。

2. **Discovery 阶段的 fan-out 是天然并行**: `discover.yml` 的 `scan`/`market-research`/
   `capability-analysis` 三叉扇出是并行模式的首个真实用例。它们各自产出独立的
   `docs/discovery/*.md`。但如果两个 phase 意外声明了同一个输出路径,
   当前系统没有任何防御。

3. **数据完整性是 ForgeOS 的核心承诺**: ForgeOS 要运行 24h 自治软件工厂。
   如果自治模式下的并行执行会产生静默数据损坏, 用户不能信任系统。
   这个缺口使 `--parallel` 在生产使用中不可用。

### 建议方向

- **冲突检测**: `RunParallel` 在调度波前, 遍历波内所有 phase 的 `Emits` 列表,
  检查是否有交叉路径。发现交叉时, (a) fail-fast 拒绝执行 (安全), 或 (b) 序列化
  这两个 phase (退化为顺序执行但同时启动的是输出不可预测——不如序列化做明确)
- **原子临时文件 + 重命名**: 要求 agent 先把输出写入临时路径, 成功后原子重命名
  到目标路径 (类似 checkpoint 的 `writeSynced` + rename)。这样两个 phase 即使
  意外写了同一个文件, 后完成的那个会原子覆盖前一个——不会产生交织。
- **后验校验**: phase 完成后, 检查声明 `emits` 的文件是否确实存在且非空。
  不存在 → 记录 WARN (诚实: agent 声称产出但未实现)
- **远期**: 引入文件级锁定或每个 phase 独立的输出目录 (如 `.forge/outputs/<phase>/`),
  再在 phase 完成后合并到共享目录

---

## 方向 4: Checkpoint 工作流定义漂移检测 (Resume 安全)

### 现状

`internal/persist` 包的 checkpoint 系统支持:

- 原子写入 (temp+rename+fsync)
- 历史保留 (rotateRetain, up to 5 备份)
- phase 粒度恢复 (PhaseIndex)

但是 **`persist.Checkpoint` 不记录工作流本身的指纹**:

```go
// forge-core/internal/persist/checkpoint.go:60-85
type Checkpoint struct {
    FormatVersion     string  `json:"_format,omitempty"`
    Workflow          string  `json:"workflow"`           // 只是名字 (如 "build")
    Mode              string  `json:"mode"`
    Iteration         int     `json:"iteration"`
    RoadmapCompletion float64 `json:"roadmap_completion"`
    PhaseIndex        int     `json:"phase_index,omitempty"`
    ...
}
```

`Workflow` 只是一个名字字符串——不包含:
- 工作流的 content hash 或版本戳
- phases 列表的 fingerprint
- stop_condition 的 snapshot

当用户 `forge run --resume` 时:

```go
// forge-core/cmd/forge/checkpoint_reflect_test.go
// resumeStart 加载 checkpoint, 读 workflow 名, 把它传给 loadWorkflow
// 但如果 workflow YAML 在 checkpoint 写入后被修改过, 没有检测
```

结果是: 工作流定义在 checkpoint 保存后可能已变更 (phase 重排序、新 phase 插入、
stop_condition 修改), 但 resume 仍从记录的 PhaseIndex 继续。跳转到的位置可能
对应完全不同的 phase。

### 证据

具体危险场景链:

1. 用户启动 `forge run build` → 完成 implementer (phase 1/5) → checkpoint 写入:
   `Workflow="build", PhaseIndex=2`
2. 用户在另一个分支编辑 `.agent/workflows/build.yml`: 在 implementer 前插入
   一个新 phase "lint-setup" → phases 索引全部+1
3. 用户切回原分支 → `forge run build --resume` → checkpoint 读到 PhaseIndex=2
4. `RunFrom(wf, mode, 2)` → 但它指向的是原来的 harness-gates, 现在是 lint-setup
5. **gate 被跳过**: lint-setup 不是 gate phase, 运行了一个空 phase → 继续到
   真正的 harness-gates → 侥幸跑完但 lint-setup 从未运行

```go
// forge-core/cmd/forge/main.go 中的 resumeStart:
startPhase, err := persist.Load(...) // 读 checkpoint
// 没有将 checkpoint 的 workflow 指纹与当前 YAML 文件对比
```

### 为什么需要

1. **长期运行的安全性**: ForgeOS 的目标是 24h+ 自治运行。Resume 是这个愿景的
   关键能力——没有它, 一次崩溃就丢失数小时的工作。但如果有"resume 到错误 phase"
   的风险, 用户不敢在生产使用 resume。不安全的 resume 比没有 resume 更糟
   (它给你虚假的信任)。

2. **多人协作的必然冲突**: 在团队环境中, 工作流 YAML 在 git 中。开发 A 启动
   evolve, 开发 B 修改 workflow YAML (添加一个安全 gate)。A 的容器重启后 resume →
   新 gate 被无声跳过。这是**合规审计的噩梦**。

3. **已有恢复机制但缺乏验证**: checkpoint 的原子性和保留机制是优秀的,
   但它们解决的是"崩溃时文件不会损坏", 不是"恢复的目标仍然有效"。
   这是"基础设施正确"但"语义正确"的缺口。

### 建议方向

- `persist.Checkpoint` 增加 `WorkflowHash` 字段: 在 save 时对 workflow YAML 内容
  做 SHA-256 (或简化的内容指纹), 存入 checkpoint
- `resumeStart` 在加载 checkpoint 后, 计算当前 `loadWorkflow()` 结果的内容指纹,
  与 checkpoint 中的指纹对比
- 不匹配时的行为: (a) fail-closed 拒绝 resume + 打印 diff, 或 (b) 叙事性警告
  "workflow 已变更, resume 目标 phase 可能不正确" + 要求 `--force` 确认
- 远期: 同时记录 checkpoint 的 phases 列表 (至少每个 phase 的 name 和类型),
  恢复时验证当前 workflow 的 phases 列表是 checkpoint 记录的超集 (允许追加,
  不允许重排序/删除)

---

## 方向 5: Agent 产出的真实性交叉验证 (诚实性 V 2)

### 现状

当前的诚实性机制包括:

| 机制 | 位置 | 功能 |
|------|------|------|
| `FileDelta` | `converge.Signals` | 核对 ROADMAP 勾选与 git diff 的匹配程度 |
| `CodeTestRatio` | `converge.Signals` | 检测代码写得很多但测试很少的情况 |
| Gate 客观裁决 | `orchestrator` | gate 结果注入 reviewer prompt, 不靠 agent 自述 |
| `FreshContext` | `asset.Phase` | reviewer 不看到 implementer 的输出来保持客观性 |

但这些机制都是**被动**和**事后**的:

- `FileDelta` 和 `CodeTestRatio` 只在 converge 报告中生成警告, 不阻断收敛
- Gate 裁决是二元的 (PASS/FAIL), 不检查"代码质量"或"是否做了不该做的事"
- `FreshContext` 防止 reviewer 锚定偏差, 但不能防止 agent 谎报完成度

具体缺口——**Agent 可以 tick ROADMAP `[x]` 但实际产出的文件与声明的需求无关**:

```
ROADMAP 条目: "- [ ] 添加用户登录 API 的 rate limiting"
Agent ticks: "- [x] 添加用户登录 API 的 rate limiting"
Git diff:     "修改了 README.md 的拼写"  (FileDelta ≈ 0, 触发警告但不停)
Gate:         test all green              (因为没改业务代码, 测试保持通过)
Converge:     RoadmapCompletion=100%, GatesGreen=true → MET
```

这个场景虽然不是恶意 (可能只是 agent 的 honest mistake), 但当前系统会错误地
判定收敛——即使实际交付为零。

### 证据

```go
// forge-core/internal/orchestrator/loop.go:182-195
// reportConvergence 中:
if sig.RoadmapCompletion > 0.5 && sig.FileDelta < 0.3 {
    l.logf("  ⚠ honesty: roadmap=%.0f%% but file-change coverage=%.0f%%")
}
// ⚠ 这只是一个 log 警告, 不阻止收敛
```

`converge.Signals.FileDelta` 虽然被 `LoopEngine.reportConvergence` 检查,
但检查结果是纯日志——`evalOne` 中没有任何 criterion 消费 `FileDelta` 信号。
它不影响 `allMet` 的值。

```go
// converge.go:130-160
// evalOne 只识别 roadmap_completion, gates_status, requirement_confidence,
// review_status, 和 acceptanceMetrics
// FileDelta 不在这些 metric 中, 因此不能被 stop_condition 引用
```

更根本的是, `gatherSignals` (cmd/forge/gates.go) 中的 `computeFileDelta` 使用
**关键字子串匹配** (ROADMAP 条目的文字 vs git diff 的文件名),
这是一种诚实但极其粗糙的启发式——一个 `[x]` 的 tick 可以完全不对应任何代码。

### 为什么需要

1. **自治运行的核心信任问题**: 如果 ForgeOS 要在无人值守下运行 24h,
   它必须能区分"agent 真的做了"和"agent 声称做了"。当前的依赖是
   agent 的 honesty + harness gate 的二零检查——但 gate 可以通过
   不触碰业务代码的修改来保持绿色。这是一个**被忽略的假阳性收敛路径**。

2. **`FileDelta` 信号已声明但未接线**: `converge.Signals.FileDelta` 字段存在,
   `computeFileDelta` 实现运行, `LoopEngine.reportConvergence` 打印它——
   但没有任何 stop_condition 能引用它。这符合 FUNCTIONAL_REQUIREMENTS_AUDIT.md
   的"断信号"模式 (如 Sprint 29 修复前的 `ReviewStatus`), 是一个**活跃但未使用**
   的信号。

3. **从"警告"到"阻断"的可选升级路径**: 有些团队可能希望 `FileDelta < 0.5` 时
   阻止收敛 (强制 agent 要么写代码、要么撤 tick)。系统应该有这个选项,
   而不是把诚实性只作为"观察"。

### 建议方向

- `converge` 增加 `"file_delta"` metric 识别: 允许 stop_condition 的 criterion
  引用 `FileDelta`, 例如 `{metric: file_delta, operator: '>=', threshold: 0.5}`
- `evalFileDelta` 函数: 镜像 `evalRoadmap` 和 `evalRequirementConfidence` 的模式,
  将 `FileDelta` 纳入收敛判定——选择权在 workflow 作者
- **进阶**: `computeFileDelta` 的精度提升——从关键词子串匹配升级为:
  (a) 读取 ROADMAP 条目的 `[x]` 项, 用 LLM 提取"应修改的文件"的期望模式;
  (b) 或用 harness 级别的文件修改追踪 (如 `forge diff --check-roadmap` 命令)
- **监管模式**: `production` lifecycle 下, `file_delta` 默认为 `>= 0.3` 的收敛条件,
  防止 agent 在无人值守时虚报完成——即使用户忘记在 workflow 中声明

---

## 优先级评估

| 方向 | 价值 | 实现难度 | 风险 | 推荐时间 |
|------|------|----------|------|----------|
| 1. 自适应上下文窗口 | 中 (长期项目质量) | 中 (需重构 prompt 包) | 低 | Sprint 33+ |
| 2. 风险感知编排 | 高 (安全合规) | 低 (已有 producer, 缺 consumer) | 中 (改变既有行为) | **Sprint 32** |
| 3. 并行冲突检测 | 高 (使 --parallel 生产可用) | 中 (需协调写入) | 低 (新功能, 不影响串行) | Sprint 33 |
| 4. Resume 工作流漂移检测 | 中 (长期运行安全) | 低 (加 hash + 校验) | 低 (fail-closed) | **Sprint 32** |
| 5. 真实性交叉验证 | 高 (自治核心信任) | 低-中 (接已有信号) | 低 (可选升级) | Sprint 33 |

**Sprint 32 建议**: 方向 2 (风险感知, 安全价值最高) + 方向 4 (resume 安全, 改动最小),
因为两者都是"已有基础设施缺接线"的模式, 实现成本低、可独立验证。

**Sprint 33 建议**: 方向 3 (并行安全, 解锁能力) + 方向 5 (自治信任, 核心价值),
需要更多设计但长期回报最高。

**方向 1** (上下文窗口) 可弹性安排在任何 sprint, 因为它是渐进改善而非功能缺口——
但它对项目扩展的长期影响最大, 不应无限期推迟。

---

*本文件基于 2026-07-10 的 `forge-core` 全量源码扫描产出,
覆盖 18 Go 包、~200 源文件、5 个 workflow YAML、~12 个 agent 卡。*
