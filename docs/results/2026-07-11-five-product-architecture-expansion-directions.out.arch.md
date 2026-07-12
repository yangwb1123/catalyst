# 架构分析报告：ForgeOS 五个前瞻扩展方向

> **分析基准**: `docs/requirements/2026-07-11-five-novel-perspectives-product-architect.md` (32.7 KB)
> **交叉引用**: `.agent/ARCHITECTURE.md` · `AGENTS.md` · `2026-07-10-five-operational-frontiers.md`

---

## 1. 架构评估

### 1.1 当前架构的核心优势

ForgeOS 当前的架构（脊柱：Idea→Production + 五引擎落地）已展现出深思熟虑的架构决策：

| 优势 | 证据 | 为什么重要 |
|------|------|-----------|
| **引擎化设计** | Orchestrator / Model-Router / Memory / Context / Evaluation 五引擎独立封装 | 单一职责、可独立演化；是方向三（Adaptive Workflow）和方向五（Federation）的架构前提 |
| **纯标准库零依赖** | Go 运行时无第三方依赖 | 方向五（Federation）的 peer-to-peer 分发不引入供应链风险 |
| **治理机制先行** | AGENTS.md 三档红线（闸门/规范/纪律）先行于功能 | 方向一（Shadow）的信任梯度可以嫁接在已有闸门体系上 |
| **Checkpoint/Trace 基础设施已就绪** | `.forge/trace.jsonl` + `checkpoint.json` + memory.jsonl 三件套落盘 | 方向四（Replay）的核心数据管道已竣工 80% |
| **模式/生命周期组合设计** | mode×lifecycle 二维旋钮驱动 Router/Harness/Workflow | 方向三（Adaptive）的注入条件可以复用这个决策空间 |

### 1.2 架构债与技术债

分析文档揭示了三个层级的架构债：

**第一级：概念级债（最严重）**

1. **执行语义的三元缺口**: `dry` → `live` 之间缺少 `shadow`。这是使用模型的设计缺失——当前架构的「执行」抽象层次以 `CommandExecutor` 为核心，但该接口的设计假设每一次执行都是**最终性的**（要么干跑不调 LLM，要么真调真落盘）。缺少「调 LLM 但捕获产出」的中间语义。

2. **产出物模型单一**: 架构的产出层只有两级——原始 git diff（机器粒度）和 convergence report（聚合粒度）。中间缺少**结构化变更叙事**（方向二的核心命题）。这不是「少个字段」的缺口，而是架构层次中缺失了一个图层。

3. **编排静态性**: `RunFrom` 的线性遍历模型（`for i := start; i < len(wf.Phases); i++`）是当前编排层的核心抽象。这个抽象在遇到需要「运行时响应信号注入新相位」的场景时，需要根本性的重构——不仅是加一个 hook，而是要改变相位集合的生命周期语义。

**第二级：接口级债**

| 债项 | 位置 | 影响范围 |
|------|------|---------|
| `Phase` 结构是纯数据容器 | `internal/asset/asset.go` | 无法表达动态/瞬时相位；无法附加生命周期方法 |
| `Event.Detail` 是自由文本 | `internal/trace/trace.go` | 方向二（Narrative）和方向四（Replay）需要结构化字段 |
| `memory.Entry` 无跨实例字段 | `internal/memory/memory.go` | 方向五（Federation）新增字段时需向后兼容 |

**第三级：构建/分发级债（已被 operational-frontiers 覆盖）**

无版本号注入、无预编译二进制、无自更新——这些是运维债但不属于架构缺陷。

### 1.3 关键设计决策的合理性评估

| 决策 | 当时合理 | 现在是否仍然合理 | 备注 |
|------|---------|----------------|------|
| `CommandExecutor` 二元语义（dry/live） | ✅ | ⚠️ 不再充分 | 需引入 shadow 语义，但可以在不破坏现有接口的情况下通过适配器模式扩展 |
| 线性相位遍历 | ✅ | ❌ 成为瓶颈 | 静态相位序列在 31 轮 sprint 内足够，但自适应编排需求已超出其能力边界 |
| Trace 自由文本 Detail | ✅ | ⚠️ 需要结构化扩展 | 初期迭代快速，现在需要增加结构化字段，保留 detail 作兼容 |
| Memory 单实例作用域 | ✅ | ✅ | 单实例跑通 v1 是正确的；联邦化应作为 v2 增量，不改现有 Entry 语义 |
| Go 纯标准库 | ✅ | ✅ | 方向五的零外部依赖原则正是由此决策支撑 |

---

## 2. 扩展方向分析（架构视角）

文档已给出五个方向。以下从架构角度深化每个方向的实现挑战和架构变更模式。

### 2.1 方向一：Shadow-Mode "Propose-Only" Execution

**架构本质**: 引入一个新的执行语义层，介于 dry-run 和 live-execution 之间。

#### 为什么需要（架构视角）
现有的执行抽象树缺少一个**预览节点**。在 CI/CD 的类比中：`dry` 等于 `--dry-run`（只解析 YAML，不执行），`live` 等于 `deploy`，`shadow` 等于 `terraform plan`——它是人类审批环中不可跳过的步骤。没有 shadow，ForgeOS 的编排能力永远与人类的信任能力间存在鸿沟。

#### 核心架构挑战

1. **工作目录隔离策略（选项分析）**:
   - **选项 A: git worktree add** — 零拷贝、共享 object store、快速。但受限于 git 仓库（非 git 管理的目录不可用）。
   - **选项 B: tmpfs 快照（cp -r）** — 适用于任何目录，但大仓库拷贝 O(n) 文件、慢。
   - **选项 C: 写时复制（COW 文件系统）** — 最优方案但依赖内核支持（overlayfs / btrfs snap），跨平台问题。
   - **推荐**: 选项 A（主）+ 选项 B（fallback for non-git），v2 考虑 overlayfs。

2. **Phase 间产物传递**:
   Shadow 不是单 phase 执行，而是整条 pipeline 的执行预览。前一 phase 的产出（task-plan.md）作为后一 phase 的输入。这意味着临时工作目录不是独立的，而是 phase-by-phase 继承的版本化序列。

3. **Output capture 契约**:
   ```go
   // 需要的新接口契约
   type ShadowResult struct {
       PhaseResults []PhaseShadow
       CumulativeDiff string     // unified diff of all changes
       Narrative     Narrative   // 方向二的叙事结构
       CostSummary   CostSummary
   }
   ```

#### 架构变更范围

| 变更 | 影响 | 风险 |
|------|------|------|
| `CommandExecutor` 新增 `shadow` mode | 低——适配器模式，不改变现有接口 | 低 |
| `orchestrator.RunFrom` 增加 shadow 分支 | 中——主循环增加一个条件分支 | 中（需确保 resume/checkpoint 正确处理） |
| 新增 `git.WorktreeManager` 模块 | 低——新模块，不依赖现有代码 | 低 |
| 产出格式（JSON diff） | 低——序列化契约 | 低 |

### 2.2 方向二：Semantic Change Narrative Pipeline

**架构本质**: 在「原始数据层」（git diff / trace）和「人类消费层」（convergence report）之间插入一个**结构化语义层**。

#### 架构模式选择

| 模式 | 描述 | 适合度 |
|------|------|--------|
| **管道（Pipeline）** | Phase 执行 → diff capture → LLM 摘要 → narrative 序列化 → 聚合 | ✅ 最佳——每个阶段职责明确、可独立替换 |
| **事件驱动（Event-Driven）** | Phase 完成后 emit event → listener 生成 narrative | 🟡 可行但过度——目前不需要解耦到事件粒度 |
| **装饰器（Decorator）** | 在现有 phase executor 外包装 narrative 生成 | 🟡 适合过渡期，但装饰器嵌套不可组合 |

#### 关键设计决策

1. **LLM 生成内容的诚信锚**:
   - `files_changed` / `loc_delta` 必须由 `git diff --stat` 机械计算
   - LLM 只贡献 `summary` 字段，且标注 `generated_by: model_name`
   - 这个分割线是架构级的：它决定了叙事管道的**可信根基**

2. **增量 Diff 的版本化**:
   在 loop-back 场景中，第二次 implementer 执行后的 diff 包含了第一次的变更+修正。叙事需要区分：
   - `diff_from_baseline`: 相对于 phase 开始时的 diff
   - `diff_from_checkpoint`: 相对于 workflow 开始时的累积 diff
   - `delta_changes`: 相对于前一次实现

3. **持久化策略**:
   ```
   .forge/narratives/
   ├── <run-id>.json        # 单次 run 的完整叙事
   ├── <run-id>.signed.json # GPG 签名的审计版本（合规）
   └── index.json           # run-id → summary 索引
   ```

#### 与方向一的复用关系

方向一的 shadow mode 需要 capture diff 作为其核心产出。方向二的叙事管道可以复用 shadow 的 diff capture 基础设施。**产品组合中，方向一是「怎么预览」，方向二是「预览和结果的描述是什么」**——两者共享 diff_capture 模块。

### 2.3 方向三：Adaptive Workflow Composition

**架构本质**: 从「静态相位序列」到「响应式相位图」的范式迁移。

#### 当前架构的核心限制

```
// 当前：线性静态
phases := [P1, P2, P3, P4, P5]
for i := 0; i < len(phases); i++ { execute(phases[i]) }

// 目标：响应式图
phases := [P1, P2, ...]
for {
    signal := evaluate(context)
    if signal.trigger { inject(signal.phase) }
    next := resolve(phases, current, signal)
    if next == nil { break }
    execute(next)
}
```

#### Phase 生命周期模型扩展

当前 `Phase` 是纯数据容器。自适应编排需要引入生命周期接口：

```go
type PhaseLifecycle interface {
    BeforeExecute(ctx *ExecutionContext) (shouldRun bool, err error)
    AfterExecute(ctx *ExecutionContext, result *PhaseResult) (injections []Phase, err error)
    CanResume(ctx *ExecutionContext) bool
}
```

这个接口不破坏现有 Phase 结构（现有 Phase 作为纯数据容器保持不变），而是通过 PhaseRegistry 的装饰来实现。

#### 注入规则引擎的设计选择

| 选项 | 描述 | 评估 |
|------|------|------|
| **A: YAML 内嵌规则** | workflow.yml 内 `phase_injection` 块 | ✅ 声明式、审计友好；❌ 规则复杂后 YAML 膨胀 |
| **B: 独立规则 DSL** | 单独的 policies/rules.yml | ✅ 关注点分离；❌ 增加了学习和接线成本 |
| **C: Hook 脚本** | phase 前后可执行外部脚本检查条件 | 🟡 灵活但不可审计、不安全 |
| **推荐**: A（v1 五条常用规则）+ 路线图支持 B（v2 复杂编排） |

#### 架构关键边界

- **Safety floor 不可绕过**: 注入可以加相位（更多检查），不能跳过已有的 safety floor（Opus-only reviewer）
- **Checkpoint 兼容**: 注入的相位需要 `PhaseID`（非索引）来定位，因为索引在线性遍历中会偏移
- **Loop-back 交互**: 重跑注入前的相位时，注入的相位应保留还是重新注入？v1 规则：保留已注入相位，不重复注入

### 2.4 方向四：Convergence Replay & Forensic Analysis

**架构本质**: 在「原始可观测性数据」（trace/checkpoint/memory）之上构建一个**查询/分析层**。

#### 数据依赖分析

| 输入数据 | 当前状态 | 对 Replay 的适配度 |
|---------|---------|------------------|
| `trace.jsonl` | 已落盘，Event 结构带 kind/name/status/duration/cost | ⚠️ 缺少 `phase_index` 和 `iteration` 的外键字段 |
| `checkpoint.json` | 已落盘，保留历史版本 | ✅ 版本链完整 |
| `memory.jsonl` | 已落盘 | ✅ 但缺少与 run-id 的关联 |
| narrative（方向二）| 尚未实现 | 一旦实现将极大丰富 replay 的语义层 |

#### Replay 引擎架构

```
┌─────────────┐   ┌──────────────┐   ┌───────────────┐
│ Parser Layer │ → │ Timeline     │ → │ Analysis Views│
│ · reader     │   │ Builder      │   │ · cost        │
│ · validator  │   │ · phase seq  │   │ · loop-back   │
│ · versioner  │   │ · loop graph │   │ · convergence │
└─────────────┘   │ · cost trace  │   │ · what-if     │
                  └──────────────┘   └───────────────┘
```

#### What-if 模拟器的两个诚实声明

1. **非线性提示**: 改变 mode 后的实际行为可能完全不同——`forge what-if` 的输出是回归/统计推断，必须标注「基于 trace 数据的估算值」
2. **因果不可逆**: trace 记录了「在给定 prompt 下 agent 的行为」，what-if 无法模拟「换了一个 prompt 后 agent 会说什么」

### 2.5 方向五：Multi-Instance Knowledge Federation

**架构本质**: 从单实例知识存储演进为**联邦知识网络**。

#### 一致性模型选择

| 模型 | 描述 | 适合度 |
|------|------|--------|
| **最终一致性** | 实例间异步同步，冲突后发版裁决 | ✅ 唯一可行的选择——无中央协调器 |
| **强一致性** | 所有实例实时一致 | ❌ 离线场景不可行，且没有中央服务 |
| **因果一致性** | 有因果关系的操作按顺序同步 | 🟡 理想但实现复杂，v3 目标 |

#### 知识去重与冲突解决策略

```
Entry A: "Use SQLite" (confidence=0.8, time=T1, origin=repo-a)
Entry B: "Use PostgreSQL" (confidence=0.9, time=T2, origin=repo-b)
→ 裁决规则（优先级自上而下）:
  1. Supersedes 显式覆盖
  2. 同 Topic: 高 Confidence 覆盖低
  3. 同 Confidence: 新时间覆盖旧时间
  4. 同时间: Origin lexicographic 排序（确定性）
```

#### 联邦协议设计原则

- **无中央依赖**: 纯 P2P，基于 Git 或文件交换。CI/CD 是自然的传播通道
- **推拉分离**: 导出（push）异步无确认，导入（pull）有过滤和冲突解决
- **安全自治**: 实例控制自己的 `share_level`，无法技术强制防止泄露（类似 git commit 的本地签名）
- **渐进采用**: 单个实例零配置（`share_level: local`）即可运行，导入联邦知识是 opt-in

---

## 3. 接口设计建议

### 3.1 关键接口原则

**原则一：向后兼容是硬约束**

ForgeOS 31 轮 sprint 后已有用户。所有扩展必须在现有接口不变的前提下增量引入。

```go
// 不改变现有接口，通过适配器/装饰器扩展
type ShadowExecutor struct {
    delegate  CommandExecutor   // 包装现有的 CommandExecutor
    worktree  WorktreeManager
    diffCache DiffCapturer
}
```

**原则二：CLI 命令的三段式演化**

```
v1: forge run --executor live          # 现有
v2: forge run --executor shadow        # 方向一
v3: forge run --profile shadow --narrative --output json  # 组合
```

每个 flag 独立、正交、可组合。

**原则三：结构化产出是增长飞轮**

当前所有子命令缺少 `--output json` 是方向二/四/五的共同阻塞依赖。建议：

```go
type OutputFormatter interface {
    FormatJSON(v interface{}) ([]byte, error)
    FormatText(v interface{}) (string, error)
    FormatMarkdown(v interface{}) (string, error)
}
```

所有子命令的 `print*` 函数统一走这个接口。

### 3.2 是否需要新的抽象层

| 抽象层 | 必要性 | 引入时机 |
|--------|--------|---------|
| **Phase 生命周期接口** (`PhaseLifecycle`) | 必需——方向三的前提 | 方向一/二之后（~sprint 33-34） |
| **Diff Capture 抽象** (`DiffCapturer`) | 必需——方向一和二的共享基础设施 | 立即（sprint 32） |
| **Narrative Schema** (`Narrative`) | 必需——方向二的 Schema 定义 | 方向一之后（sprint 33） |
| **Replay 引擎** (`Replayer`) | 推荐——方向四的核心抽象 | 方向二之后（sprint 34-35） |
| **知识联邦协议** (`KnowledgePeer`) | 前瞻——方向五的核心抽象 | 产品三件套完成后（sprint 36+） |

### 3.3 保持向后兼容的具体策略

1. **Phase 结构**保持不变，新增 `PhaseLifecycle` 作为可选接口类型
2. **Event.Detail** 保留 `string` 类型，新增 `Changes []ChangeEntry` 可选字段
3. **memory.Entry** 新增 `Origin`/`ShareLevel`/`Namespace` 作为 omitempty 可选字段
4. **所有新功能默认关闭**：`--shadow`、`--narrative`、`--federate` 都是 opt-in flag
5. **Trace 格式版本化**：新增 `_version` 字段，Replay 引擎按版本解析

---

## 4. 技术选型

### 4.1 需要引入的新技术或库

| 方向 | 推荐选型 | 理由 | 替代项评估 |
|------|---------|------|-----------|
| 方向一 COW 快照 | **overlayfs**（Linux）/ 无额外依赖 | 零拷贝、内置于 Linux 内核 | tmpfs cp -r（简单但慢，适合 fallback） |
| 方向二 JSON Schema 验证 | **Go 标准库 `encoding/json` + 手动 schema 检查** | 零依赖原则；schema 很小不需要框架 | 如果复杂度增长，可考虑 `go-json-schema`（外部依赖评估中） |
| 方向三 规则引擎 | **声明式 YAML + Go `case/switch` 求值** | 规则是简单布尔表达式，不引入规则引擎框架 | 引入规则引擎（`gengine`/`expr`）是过度设计 |
| 方向四 终端 UI | **Go 标准库 `tablewriter` 或 `charmbracelet/lipgloss`** | Replay 输出是终端表格 + 进度条 | 未来可升级到 `bubbletea` TUI，但不急 |
| 方向五 知识序列化 | **JSON + 可选的 MessagePack** | JSON 可读、git diff 友好；MsgPack 用于大体积传输 | Protocol Buffers（强力但增加 schema 管理成本，v2 考虑） |

### 4.2 第三方依赖的评估标准

ForgeOS 目前**零外部依赖**（Go 运行时）。以下评估框架：

```
能否用标准库+现有基础设施实现？
  ├── 是 → 零依赖。结束。
  └── 否 → 评估外部依赖的必要性：
       ├── 依赖的成熟度（GitHub stars > 1000? 有 maintainer?）
       ├── 是否纯 Go?（无 CGO 要求）
       ├── 是否引入传递依赖？是否增加超过 5 个传递包？
       ├── 协议是否与 ForgeOS 许可兼容（MIT/Apache2）
       └── 如果该库今天消失，能否 fork 替代？
```

**当前五个方向的推荐：零外部依赖**。所有功能均可由 Go 标准库 + 现有代码实现。

### 4.3 自建 vs 采购/继承

| 功能 | 选择 | 理由 |
|------|------|------|
| 规则引擎 | **自建** | 方向三的注入规则是简单布尔表达式，不值得引入规则引擎框架 |
| Git 操作 | **继承 `go-git` 的已有代码** | forge-core 已有 `internal/git` 模块 |
| 终端输出 | **自建 table/barchart 格式化** | 输出量很小，不需要引入 `lipgloss`/`tablewriter` 依赖 |
| 知识序列化 | **自建 JSON 格式 + 版本号** | schema 很小，JSON 可读，git diff friendly |
| P2P 传输 | **基于 Git（零基础设施方案）** | 不需要 gRPC、NAT 穿透、节点发现——Git 是已有基础设施 |

**唯一值得考虑的外部库**: `charmbracelet/bubbletea` 为 Replay 工具的 TUI 模式。但 v1 可以只输出文本 table，TUI 是 v2 选项。

---

## 5. 实施路线图

### 5.1 优先级排序

基于**产品影响力 × 技术依赖关系**的二维排序：

```
       影响力高
        │
   P1 方向四 │ P0 方向一
   (Replay)  │ (Shadow-Mode)
        │    │
   ─────┼────┼──── 技术依赖度低────技术依赖度高
        │    │
   P2 方向五 │ P1 方向二
   (Fed)     │ (Narrative)
        │    │
   P3 方向三 │ P1 方向一→二(连续)
   (Adaptive)│ (共享diff capture)
        │    │
       影响力低
```

**排序结论**:
- **P0（最高）**: 方向一（Shadow-Mode）——信任基础，解锁企业采用，零架构前置依赖
- **P1**: 方向二（Narrative）+ 方向四（Replay）——可读产出 + 事后分析，可同步推进
- **P2**: 方向三（Adaptive Workflow）——架构突破，但需要 Phase 生命周期重构
- **P3**: 方向五（Federation）——高价值但无紧迫性，依赖 memory schema 扩展

**但文档推荐的「一+二+四」产品三件套顺序才是最优**——技术依赖图如下：

```
方向一 Shadow-Mode
  └── diff capture 基础设施 ──→ 方向二 Narrative（复用 diff capture）
                                  └── 结构化 trace data ──→ 方向四 Replay（复用 trace）
```

### 5.2 阶段划分

#### Phase 1（Sprint 32-33）: 信任基础设施 — 方向一 + 方向二基础

| 周 | 里程碑 | 产出 |
|----|--------|------|
| W1 | `DiffCapturer` 模块 | 可独立测试的 git diff capture + 结构化 diff 定义 |
| W2 | `ShadowExecutor` 原型 | `--executor shadow` 对单 phase 生效，输出 unified diff + JSON |
| W3 | WorktreeManager | `git worktree add` + tmpfs fallback，phase 间产物传递 |
| W4 | Narrative schema v1 + phase 级摘要 | 机械数据 100% 真实 + LLM 摘要标记 `generated_by` |

**门禁标准**:
- `forge run --executor shadow` 产出可消费的 unified diff + JSON
- 原始工作目录零修改（`git status` 应为 clean）
- 叙事文件存活于 `.forge/narratives/<run-id>.json`

#### Phase 2（Sprint 34-35）: 可观测性 — 方向四 + 方向二完整

| 周 | 里程碑 | 产出 |
|----|--------|------|
| W5-W6 | Replay 引擎 | `forge replay --latest` 显示时间线/成本/循环分析 |
| W7 | Flow-level narrative | 多 phase 叙事合并为 workflow changelog |
| W8 | `forge diff-runs` v1 | 对比两个 run 的元数据 |

**门禁标准**:
- `forge replay` 输出包含：时间线 + 成本分解 + loop-back 分析
- 数据全部来自已落盘的 trace/checkpoint，不调 LLM
- Narrative output 与 git diff 内容可交叉验证（机械字段 vs LLM 摘要）

#### Phase 3（Sprint 36-38）: 架构升级 — 方向三

| 周 | 里程碑 | 产出 |
|----|--------|------|
| W9-W10 | PhaseLifecycle 接口 + PhaseRegistry | 相位注入引擎原型 |
| W11 | 规则系统（YAML）+ 5 条内建规则 | 动态注入 pci-compliance / escalation / cost-optimization |
| W12 | Checkpoint 兼容 + safety floor 守卫 | 注入相位可审计可重放，不破坏现有 phase 执行 |

**门禁标准**:
- 规则系统 5 条注入规则全部通过端到端测试
- 所有注入相位在 trace 中可审计
- 已有 workflow YAML 零改动

#### Phase 4（Sprint 39-42）: 知识网络 — 方向五

| 周 | 里程碑 | 产出 |
|----|--------|------|
| W13 | Entry 扩展（Origin/ShareLevel）| memory 向后兼容 |
| W14 | `forge knowledge export/import` | 基于文件的 P2P 知识交换 |
| W15 | Scorecard 联邦聚合 | `forge scorecard --federate` |
| W16 | 冲突解决 + 去重 | Confidence/Supersedes 裁决引擎 |

**门禁标准**:
- 两个实例之间可完成 export → copy → import 的知识交换
- 导入方不会因低质量知识退化（`--min-confidence` 阈值生效）
- 单实例无联邦配置时行为零变化

### 5.3 风险点与缓解策略

| 风险 | 概率 | 影响 | 缓解 |
|------|------|------|------|
| **Shadow 保真性不足**（临时副本与真实运行不符） | 中 | 高——用户对 shadow 的信任如果被破坏，整个方向一失效 | 依赖外部网络的服务自动标注 "network side effects not shadowed" |
| **LLM 叙事摘要编造** | 高 | 中——编造的变更描述误导用户 | 机械字段硬锚定，LLM 只贡献 `summary` 且标记生成源 |
| **相位注入打破 safety floor** | 低 | 极高——安全灾难 | 编译时检查器禁止覆盖 Opus-only reviewer；运行时门禁验证 |
| **联邦知识污染**（低质量知识毒化其他实例） | 中 | 高 | `--min-confidence` 阈值 + 人工审核 hook + 置信度衰减 |
| **Narrative 管道增加 wall-clock 时间** | 高 | 中——用户可能因此关闭它 | opt-in（`--narrative`），加入 wall-clock budget 守卫 |
| **方向一/二/四的产出被用户忽略**（人类不审查 diff） | 中 | 中——信任机制设计得再好，用户不参与也是白费 | Integration with CI: shadow diff 自动贴在 PR 评论上；不审核则 merge blocked |

### 5.4 收敛选项总结

| 决策 | 选项 A（激进） | 选项 B（保守—推荐） | 选项 C（极简） |
|------|---------------|-------------------|---------------|
| 范围 | 五个方向并行 | **一+二+四产品三件套 → 三 → 五** | 只做方向一 |
| 时间 | ~6 sprints（并行 3 个 stream） | ~10 sprints（串行 + 保护期） | ~2 sprints |
| 风险 | 高——上下文切换成本超过并行收益 | 中——方向三可能依赖方向一/二的 diff 基础设施 | 低——但遗落方向二/四的信任完整性 |
| 信任闭环 | ⚠️ 部分完成需要跨 stream 协调 | ✅ 完整：「预览→产出→追溯」闭环 | ❌ 只有预览，没有「产出可读」和「事后分析」 |

**推荐选项 B**: 先完成产品三件套（方向一+二+四），在 sprint 35 后重新评估方向三和方向五的优先级。理由是方向三（Adaptive Workflow）的 Phase 生命周期重构会触及编排核心，应该在产品体验验证之后、确保方向正确的前提下再动手。方向五（Federation）的价值随用户量增长而增长——在还没有 50 个生产实例之前，其投资回报率低于产品体验三件套。

---

## 6. 清理建议：重复文件归一化

确认 `/home/u1/catalyst/docs/requirements/2026-07-11-five-novel-perspectives-product-architect.md` 与 `2026-07-11-five-product-architecture-expansion-directions.md` 内容完全一致（diff 空）。

**建议操作**: 删除后者（文件名较长，且方向编号不准确——文档列了 5 个方向但文件名称 "five-product-architecture"），保留前者作为 canonical 文件。或者更名前者为更明确的名称。

**不删风险**: 低——但后续查找时会造成困惑（两个文件名不同的语义相同的文件）。

---

**总结**: 这五个方向是对 ForgeOS 当前成熟基础设施的自然延伸。它们不修复 bug 而是**打开新的架构象限**。以「产品三件套（方向一+二+四）」为第一阶段，可以在 6 周内将一个已具备技术能力的系统转型为一个**可信任、可审计、可解释**的产品。方向三和方向五在信任基础建立之后跟进，形成完整的架构演进版图。
