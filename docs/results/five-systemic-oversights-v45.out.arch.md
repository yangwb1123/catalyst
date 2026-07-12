现在我有了足够的信息来提供一份深入的架构分析。基于验证报告的发现、已确认的事实修正，以及三个真正原创的方向，以下是分析：

---

# 架构分析：验证报告驱动的 ForgeOS 扩展方向评估

> **角色**：资深架构师  
> **分析基线**：代码验证报告（2026-07-12）+ `forge-core` HEAD b0c80e4 + 全部 31 个 Sprint 演进记录  
> **输入**：验证报告（5 个方向，3 个确认为原创，2 个需事实修正）  
> **约束**：不编写具体代码，只提供架构层面的设计和建议  
> **日期**：2026-07-12

---

## 1. 架构评估

### 1.1 报告揭示的系统成熟度信号

验证报告最重要的贡献不是发现了五个新方向，而是**修正了关于系统现状的过时或不准确的声明**。这些修正揭示了一个比外部文档（分析报告）所描述的**更成熟**的系统：

| 领域 | 分析报告声明 | 验证报告修正 | 架构含义 |
|------|------------|------------|---------|
| 成本预估算 | "零覆盖" | `checkCostEstimate()` 已在 `preflight.go:172-196` 实现 | 方向三不应作为"新方向"，而应作为"已有功能的增强" |
| ScorecardPair 数据结构 | 包含 `AvgCostUsd`/`SampleCount`/`LatencyMs` | 实际结构只有 `{Model string; TaskType string}` | 成本数据在 trace 的 `cost_usd_micros` 事件中，不在 scorecard 中 |
| 自测试覆盖 | —— | `self-testing-and-dogfooding.md` 已 300+ 行覆盖 | 方向四不应作为"空白"，而应作为"已分析但未 CLI 化" |
| 外部工具健康检查 | "没有统一健康检查" | `ProbeAll`/`ProbeTool` 存在但无 CLI 绑定 | ✅ 真缺口确认 |
| gitChangedPaths 门禁作用域 | —— | 用于风险分析 + 路由，不为门禁缩减 | ✅ 真缺口确认 |

**架构评估**：系统在三个维度上比分析报告描述的更健康（成本预估算、测试覆盖、依赖注入），但纠正后的三个原创方向（死代码治理、增量门禁执行、治理产物卫生）指向一类更深层的架构缺口——**元治理（Meta-Governance）**缺口。这不是"引擎还缺什么功能"，而是"治理系统本身缺少对自己治理状态的治理能力"。

### 1.2 元治理缺口的架构分类

验证报告确认为真正原创的三个方向，可以放入一个统一的架构框架：

```
┌──────────────────────────────────────────────────────────────┐
│                    ForgeOS 治理层                              │
│                                                              │
│  1. 代码治理 (arch-check)   ─── 检查代码结构健康               │
│      ▲                            ▲                          │
│      │ 缺少                         缺少                      │
│      │ 方向一                       方向二                    │
│  2. 包生命周期治理        3. 执行效率治理                      │
│     (死代码/孤包检测)       (增量门禁,不改的不检)               │
│                                                              │
│  4. 治理资产本身的生命周期治理 (方向五)                         │
│     ─── 治理系统管理自己的配置漂移                              │
└──────────────────────────────────────────────────────────────┘
```

方向一（死代码治理）发生在**代码级别**——检查包是否仍被需要。方向二（增量门禁）发生在**执行级别**——优化门禁的检查范围。方向五（治理产物卫生）发生在**元治理级别**——管理治理配置本身。这三个方向的共同模式是：它们都关注**治理系统的治理能力本身**，而非具体的引擎功能。

### 1.3 关键事实修正在架构层面的影响

验证报告修正的两个事实具有架构级含义，不只是文档勘误：

**修正 1：`ScorecardPair` 不含成本字段**

这意味着成本路由回灌（history tiebreak）需要的不是读取一个已存在的字段，而是建立一条**新的数据管线**：trace `cost_usd_micros` → scorecard 聚合 → 路由读取。当前 `checkCostEstimate()` 使用硬编码单价（Sonnet $0.08、Opus $0.35），而真正的成本数据在 trace 的事件流中。这是一个**数据流不完整**的架构问题——成本数据存在但未到达需要它的消费者（路由器和预估算器），中间缺失了"从 trace 事件到 scorecard 聚合"的管道段。

**修正 2：成本预估已经实现但使用硬编码单价**

`checkCostEstimate()` 的实现是"按 phase 数 × 单价"的线性模型，与实际 trace 中记录的真实成本无关。这意味着存在两条平行的成本认知路径：预估算使用静态单价、事后跟踪使用动态 trace 数据。架构上，这两条路径最终应收敛到同一个数据源——scorecard 中按 model+task_type 聚合的历史平均成本。当前它们相互独立，且预估算路径缺少数据回灌机制。

### 1.4 架构债务总结

| 债务项 | 类型 | 严重度 | 来源 |
|--------|------|--------|------|
| 成本数据流不完整（trace → scorecard → router/preflight 断开） | 数据流 | 中 | 验证报告修正 1+2 |
| 包生命周期元数据缺失（无"包是否被引用"的检测） | 治理 | 高 | 方向一确认 |
| 门禁执行作用域与改动范围解耦缺失 | 性能 | 中 | 方向二确认 |
| 治理配置无熵管理（无"定义→引用"的逆检查） | 元治理 | 中 | 方向五确认 |
| 外部工具健康检查有 ProbeAll 但无 CLI 绑定 | 可观测性 | 低 | 验证报告确认 |

---

## 2. 扩展方向

基于验证报告的事实修正和真正的原创缺口分析，我重新定义五个扩展方向及其优先级。

### 方向一：死代码与孤包元数据治理（P1）

**为什么验证报告确认它是原创的**：`arch-check.mjs` 的 8 个检查中没有包引用检测。`checkFanin` 检查的是"过多消费者→高耦合"，而不是"零消费者→死代码"。`internal/adr`（只有测试文件，零 Go 引用）和 `internal/yamlpath`（完整生产代码，零 Go 引用）是真实案例。

**为什么是元治理而非代码治理**：这不是向 `arch-check.mjs` 添加第 9 个检查那么简单。核心问题是：**Go 没有"包被谁引用"的元数据格式**。`go list -json ./...` 不输出 importers 列表，`grep` 式的文本搜索会忽略测试文件引用（包 A 被包 B 的测试文件引用时，它真的是"被引用"的吗？）。需要的是一个**包引用图**，而不仅仅是一个 grep 命令。

**架构方案选择**：

| 选项 | 方法 | 优点 | 缺点 |
|------|------|------|------|
| A（推荐） | 在 arch-check 中实现 import 图推理 | 复用现有框架，零新增外部依赖 | 需要解析 Go import 路径映射 |
| B | 新增 `forge doctor --orphans` 命令 | CLI 体验清晰，与现有 doctor 管道一致 | 重复 arch-check 的部分能力 |
| C | 在 CI 中定期运行 `go list -json` + 图分析 | 简单，无架构变更 | 每次 CI 需要全仓构建 |

**推荐选项 A**：在 `arch-check.mjs` 中新增 `checkOrphanPackages` 函数，通过分析 `go list -json -deps ./...` 的输出构建包依赖图，标记零入边的包为 "orphan"（允许例外：声明为 `library` 类型的包）。(~1 sprint)

**边界条件和对抗模式**：
- 测试文件引用不算"生产引用"——`internal/adr` 只有测试文件，应报告为 orphan
- 间接引用（A → B → C，C 无人引用但 A 间接依赖它）——C 不应报告为 orphan，因为它通过 B 被 A 间接需要
- 过渡期孤儿包——标记为 `adopted` 豁免（如 `internal/yamlpath` 被标记为 "planned for future use"）

### 方向二：增量门禁执行——从"全量检查"到"改动范围门禁"（P2）

**为什么验证报告确认它是原创的**：`gitChangedPaths` 存在于 `route.go` 和 `engine_build.go`，但只用于风险分析和路由，从来不用于门禁范围缩减。所有 harness 闸门（`gate.mjs`、`arch-check.mjs`、`check.py`、`secret-scan.mjs`）始终运行全量扫描。

**为什么是架构问题而非优化问题**：这听起来像性能优化，但更深层的问题是：**全量检查在 10 文件仓库和 10000 文件仓库上的语义相同吗？** 不是。全量检查假设检查成本与仓库大小无关，但这不是真的——`arch-check.mjs` 的全量扫描随源文件数线性增长。当 forge-core 从 18 个 Go 包增长到 180 个时，"每次 commit 都跑完整架构检查"将成为开发阻力。

**核心挑战**：
1. **依赖图传播**：改动文件 A，但 A 的消费者 B 可能因此违反扇入约束。增量检查必须跟踪改动的影响传播，而不仅仅是改动文件本身。
2. **门禁类型差异**：有些检查天然可增量（体积检查——只检查改动的文件），有些不能（循环依赖检查——需要全图）。架构需要区分"可增量"和"不可增量"的检查。
3. **诚实降级**：增量模式下的"通过"不改变全量模式下的"通过"语义——增量通过的 commit 在全量模式下也必须通过。

**架构方案**：

```
┌──────────────────────┐    ┌────────────────────────────┐
│ gitChangedPaths       │───▶│  ScopeResolver              │
│ (已存在于 route.go)   │    │  ├─ resolves changed files  │
└──────────────────────┘    │  ├─ resolves affected files  │
                            │  │  (consumers of changed)   │
                            │  └─ collapses to gate scope  │
                            └──────────┬─────────────────┘
                                       │
                    ┌──────────────────▼──────────────────┐
                    │   GateScheduler                      │
                    │   ├─ full-scan (circular/arch)      │
                    │   ├─ delta-scan (volume/lint)       │
                    │   └─ skip (no affected files)       │
                    └──────────────────┬──────────────────┘
                                       │
                    ┌──────────────────▼──────────────────┐
                    │   existing harness gates             │
                    │   (unchanged execution)              │
                    └─────────────────────────────────────┘
```

**边界条件矩阵**：

| 检查类型 | 增量可行？ | 传播范围 | 示例 |
|---------|-----------|---------|------|
| 文件体积 | ✅ 是 | 仅改动文件 | `gate.mjs` 行数检查 |
| 反模式命名 | ✅ 是 | 仅改动文件 | 测试不是上帝文件 |
| 认知复杂度 | ✅ 是 | 仅改动文件 | 函数复杂度检查 |
| 扇入/扇出 | ⚠️ 有条件 | 改动 + 消费者 | 新增导出会增加消费者扇入 |
| 分层架构 | ⚠️ 有条件 | 改动 + 所有依赖方向 | 新增 import 可能违反分层 |
| 循环依赖 | ❌ 否 | 全图 | 新增边可能在远端点形成环 |
| 全仓治理完整性 | ❌ 否 | 全仓 | `check.py` 声明 vs 实现 |

(~2-3 sprint，因为需要设计 ScopeResolver 的传播算法 + GateScheduler 的调度逻辑)

### 方向三：成本预估算增强——从硬编码单价到历史驱动（P1 增强）

**验证报告修正的核心**：这个方向上，分析报告声称"零覆盖"是不准确的——`checkCostEstimate()` 已经存在于 `preflight.go`。但系统存在更深层的架构问题：**两条平行的成本认知路径**。

```
         ┌──────────────────────┐         ┌──────────────────────┐
         │  preflight.go        │         │  trace/cost.go       │
         │  checkCostEstimate() │         │  cost_usd_micros     │
         │                      │         │                      │
         │  硬编码单价          │         │  真实交易成本        │
         │  Sonnet=$0.08        │         │  per-phase 记录      │
         │  Opus=$0.35          │         │                      │
         └─────────┬────────────┘         └──────────┬───────────┘
                   │                                 │
                   │  ✗ 不连接                        │
                   ▼                                 ▼
             用户看到"估算"                      scorecard 看到"实际"
```

**架构目标**：消除"估算 vs 实际"两条路径的差异，将预估算管道接上 scorecard 中按 `model+task_type` 聚合的历史平均成本。

**为什么是 P1（与原分析报告一致）**：成本可观测性是 ForgeOS 自治运行的核心信任机制。用户授权烧真实预算前（Sprint 24-26），需要知道"这次 evolve 会花多少钱"。当前硬编码单价给出的是线性估算，与真实成本可能存在偏差。

**架构变更范围**：
- 不需要新数据结构——scorecard 的 cost_usd_micros 已落盘
- 不需要新 CLI——`--dry-run` 已输出预估算
- 需要的只是：在 `checkCostEstimate()` 中，从 scorecard 读取历史平均值（如果存在），否则降级到硬编码单价

(~0.5 sprint，纯数据管线接线)

### 方向四：`forge self-test --ci`——从分析文档到可执行 CLI（P2）

**验证报告修正的核心**：分析报告声称方向四是"盲区"不准确——`self-testing-and-dogfooding.md` 已经 300+ 行分析了 3 个测试套件的覆盖、`forge accept` 的自我治理、Go 测试深度挑战、测试选择系统的自测试、dogfooding 真实性。真正缺失的（也被验证报告确认的）是一个**统一的 CLI 命令**将现有散布的多套测试聚合成一条可执行的命令。

**架构分析**：这不是"缺少测试基础设施"（测试已经存在），而是"缺少测试编排"。`go test ./...` 覆盖 forge-core 的 18 个 Go 包，`node harness/acceptance.mjs` 覆盖 harness 闸门，但不存在一个入口让用户或 CI 一次性验证 "ForgeOS 本身是否工作正常"。

**已有的碎片化入口**：

| 入口 | 覆盖范围 | CLI 路径 |
|------|---------|---------|
| `go test ./...` | 所有 Go 包 | 手动 |
| `node harness/acceptance.mjs` | 所有闸门 | `forge accept` |
| `node harness/gate.mjs` | 体积闸门 | `forge gate` |
| `python3 -m pytest harness/` | check.py 测试 | 手动 |
| `forge validate --models` | 所有 agent 卡模型引用 | `forge validate` |

**核心挑战**：如何聚合这些碎片化入口，使得 `forge self-test --ci` 的退出码可靠反映系统健康状态，而不因为某个子测试 N/A 而被误判。

**架构方案**：
- 新增 `cmd/forge/self_test.go`：编排所有自测入口
- 使用与 `acceptance.mjs` 相同的 PASS/FAIL/NA 裁定逻辑
- 输出格式与 `forge accept` 相同（JSON 模式供 CI 消费）
- 测试选择：`--ci` 运行全部，`--ci --quick` 只运行快速子集（跳过完整的 Go 测试套件）

(~1 sprint，主要是 CLI 编排，不需要新的测试基础设施)

### 方向五：治理产物熵管理——从"定义→引用"到"引用→定义"的逆向审计（P3）

**为什么验证报告确认它是原创的**：`check.py` 做的是"引用→存在"检查（workflow 引用的 agent 卡必须存在），但从不检查"定义→引用"方向（agent 卡存在但从未被任何 workflow 引用）。没有 `forge governance report`、`forge governance tidy`、或 `forge prune`。ADR 新鲜度/过时跟踪不存在。

**架构分析**：这是五个方向中最深层的元治理问题。代码的"死代码检测"（方向一）是包级别的，治理产物的"死资产检测"是文件级别的。当 `.agent/` 目录从 12 个 agent 卡增长到 50 个、从 5 个 workflow 增长到 20 个时，"有哪些东西已经没人用了"会成为治理噪声的来源。

**治理资产的生命周期阶段**：

```
CREATED → REFERENCED → UNREFERENCED → OBSOLETE → DELETED
                                    ↘
                              ARCHIVED (保留但标记)
```

当前架构缺少 `UNREFERENCED → OBSOLETE` 和 `UNREFERENCED → ARCHIVED` 两条路径。死治理资产永远不消失。

**边界条件矩阵（治理资产类型 × 检测维度）**：

| 治理资产 | 定义→引用检测 | 引用→存在检测（已有） | 新鲜度检测 |
|---------|-------------|-------------------|-----------|
| Agent 卡 | ✅ 可行（`check.py` 反向操作） | ✅ `check.py` | ✅ 可行（ADR 时间戳） |
| Skill 卡 | ✅ 可行 | ✅ `check.py` | ⚠️ 可行但需要引入修改日期 |
| Workflow 文件 | ✅ 可行（`next_stage` 引用链） | ✅ `check.py` | ⚠️ 慢速演化，不易过期 |
| Policy 文件（modes.yml） | ⚠️ 困难（所有 workflow 隐式引用） | ⚠️ 隐式引用无法静态验证 | ⚠️ 策略不常变化 |
| ADR | ✅ 可由 `forge governance` 跟踪 | ✅ `forge validate` | ✅ 核心功能 |
| Eval 计分卡 | ✅ 可通过路由历史推断 | ⚠️ 无检查 | ✅ 可检查 recency 衰减 |

**架构方案**：
- 扩展 `check.py` 新增 `check_unreferenced_assets` 检查
- 新增 `forge governance tidy` 命令（删除孤立的治理资产，需 `--apply`）
- 新增 `forge governance report` 命令（输出治理资产的引用覆盖率）

(~1-1.5 sprint，基于现有 `check.py` 基础设施扩展)

---

## 3. 接口设计建议

### 3.1 关键模块的接口设计原则

**原则 1：门禁作用域作为一等参数**

当前所有 gate 函数签名都假设"全量扫描"。为了支持增量执行（方向二），门禁的作用域必须被提升为接口的一等参数：

```
// 当前模式（隐式全量）：
function checkLayering(files, rules) → Result[]

// 未来模式（显式作用域）：
function checkLayering(files, rules, scope: { changed: Set<Path>, affected: Set<Path> }) → Result[]
```

没有作用域参数的门禁函数永远无法增量执行。这是所有 gate 函数从现在起就应该遵守的签名约定——即使传入 `{ changed: null, affected: null }` 表示全量。

**原则 2：治理资产元数据接口**

为了让治理产物卫生（方向五）成为可能，每个治理资产需要携带元数据：

| 元数据 | 来源 | 用途 |
|--------|------|------|
| `id` | 文件名 → 自动推导 | 引用解析 |
| `type` | 目录/命名约定 | 分类 |
| `created` | 文件创建时间 | 新鲜度评估 |
| `last_referenced` | 引用扫描缓存 | 孤资产检测 |
| `status` | 手动/自动 | active/archived/deprecated |

当前治理资产（agent 卡、skill 卡）没有 `id` 和 `status` 元数据。文件名被用作隐含的 id，但这种隐含契约不支持弃用（deprecated）状态。一个简单的 frontmatter 格式可以解决：

```yaml
---
id: security-engineer
status: active
superseded_by: security-architect  # 当 status=deprecated 时可选
---
```

这不是增加复杂性——接口的简洁性不是在"没有字段"而是在"默认值可用"。所有字段可选，零值 = 当前行为。

**原则 3：检查类型的分类元数据**

为了实现增量调度（方向二），每个检查需要声明其增量能力和传播范围：

```javascript
// 当前模式：检查是匿名函数
{
  name: "checkFanin",
  run: (files, rules) => { ... }
}

// 未来模式：检查携带执行语义元数据
{
  name: "checkFanin",
  incremental: "dependent-scan",  // "file-only" | "dependent-scan" | "full-scan"
  propagation: "consumers",       // "none" | "consumers" | "transitive-deps" | "full-graph"
  run: (files, rules, scope) => { ... }
}
```

### 3.2 是否需要新的抽象层

| 抽象层 | 需要？ | 理由 |
|--------|--------|------|
| GateScope 类型 | ✅ 需要 | 方向二增量门禁的基础 |
| 治理资产元数据 schema | ✅ 需要 | 方向五治理产物卫生的前提 |
| 检查分类元数据 | ✅ 需要 | 增量调度的路由表 |
| 成本数据管线接口 | ⚠️ 需要但可简单 | 从 trace → scorecard → preflight 的管道 |
| 统一自测编排器 | ⚠️ 需要但可简单 | 方向四的 CLI 命令 + 编排逻辑 |
| OPA/Rego 策略引擎 | ❌ 不需要 | 超出当前范围，v3 再评估 |
| LiteLLM Provider 接口 | ❌ 不需要 | 方向三只需要成本数据管线，不需要完整的 Provider 抽象 |

### 3.3 向后兼容性

所有新增接口的默认值必须与当前 v2 行为逐位一致：

- `scope = null` → 全量扫描（与当前行为一致）
- `incremental = "full-scan"` → 检查被调度为全量执行
- 治理资产无 frontmatter → 被视为 `status: active`
- 无成本数据管线 → `checkCostEstimate()` 继续使用硬编码单价

---

## 4. 技术选型

### 4.1 新增技术需求的评估

所有五个方向的实现都可在**零新外部依赖**下完成：

| 方向 | 需要的技术 | 现有基础设施是否足够 |
|------|-----------|-------------------|
| 方向一：死代码检测 | Go import 图解析 | ✅ `go list -json -deps` + 简单的图遍历 |
| 方向二：增量门禁 | 作用域传播 + 调度 | ✅ 已有 `gitChangedPaths`，扩展即可 |
| 方向三：成本数据管线 | trace → scorecard 聚合 | ✅ trace 数据已存在，scorecard 已聚合 |
| 方向四：自测试 CLI | 测试编排 + PASS/FAIL/NA | ✅ 与 `forge accept` 复用裁定逻辑 |
| 方向五：治理熵管理 | 引用扫描 + 报告 | ✅ 基于 `check.py` 基础设施 |

**技术选型决定**：保持 forge-core 的零外部依赖策略。Harness（Node.js/Python）也不需要新依赖——`check.py` 已经证明了纯标准库的治理检查是可行的。

### 4.2 自研 vs 集成的决策

| 组件 | 决策 | 理由 |
|------|------|------|
| `GateScope` 类型 | 自研 | 核心编排逻辑，ForgeOS 差异化 |
| 治理资产 frontmatter | 自研 | 简单的 YAML 元数据，不需要框架 |
| 成本数据聚合器 | 自研 | trace 格式是 ForgeOS 自定义的 |
| 自测试编排器 | 自研 | 与 `forge accept` 复用裁定逻辑 |
| SCA/CVE 漏洞库 | 集成（OSV） | Sprint 19 已实现适配器模式 |

### 4.3 做出"不做"的决定

以下在架构层面被**明确排除**在当前范围外：

1. **不引入通用工作流语言**（如 Temporal WfC、BPMN）。方向一（工作流组合原语）的原子原语（NextStage + WorkflowInclude + OnFail 跳转）足以覆盖管线化需求。

2. **不实现完整的 Provider 抽象层**。方向三（成本预估算增强）只需要一条数据管线将 trace 成本事件连接到 preflight。完整的 Provider 注册/发现/健康检查留给 v3。

3. **不重构 `forge-init` 的采纳模型**。方向五（治理产物卫生）针对的是已存在治理资产的生命周期管理，不改变如何创建初始治理资产。

4. **不自研沙箱或 Web UI**。Firecracker 和 Web UI 仍然留给 v3 北极星。

---

## 5. 实施路线图

### 5.1 优先级排序

| 方向 | 优先级 | 风险调整后优先级 | 理由 |
|------|--------|-----------------|------|
| 方向一：死代码治理 | P1 | **P1** | 防止代码库无声膨胀的自动防线，每个 sprint 都产生收益。改动最小（~1 sprint） |
| 方向三：成本预估算增强 | P1（修正后） | **P1** | 验证报告确认已有基础，只需要接线（~0.5 sprint） |
| 方向四：`forge self-test --ci` | P2（修正后） | **P1** | 解锁 CI 一体化验证（~1 sprint） |
| 方向二：增量门禁 | P2 | **P2** | 价值巨大但需要设计作用域传播算法（~2-3 sprint） |
| 方向五：治理产物卫生 | P3 | **P3** | 价值会随治理资产规模增长而增大，当前阶段 .agent/ 资产数还不算多 |

**优先级修正说明**：与原始分析报告相比，我将方向三和方向四上调优先级（因为它们已有基础设施，实现成本低于原始分析报告的估计），方向二保持在 P2（因为它的算法复杂度高于其他方向）。

### 5.2 阶段划分

#### 阶段一：快速收益（1-2 Sprint）

> **目标**：三件低投入、高可信度的事

- **Sprint A**：方向一（死代码检测）——在 `arch-check.mjs` 中新增 `checkOrphanPackages`
  - 实现 Go import 图推导（基于 `go list -json -deps`）
  - 标记 `internal/adr` 和 `internal/yamlpath` 为 orphan
  - 添加例外机制（`adopted` 标记）
  - 更新 `docs/FUNCTIONAL_REQUIREMENTS_AUDIT.md` 的死代码相关条目

- **Sprint B**：方向三（成本数据管线）——将 trace 成本事件连接到 preflight
  - 在 `scorecard` 聚合中添加按 `(model, task_type)` 的平均成本计算
  - 在 `checkCostEstimate()` 中读取 scorecard 历史平均值
  - 无历史数据时降级到当前硬编码单价
  - 在 dry-run 输出中显示"基于历史"vs"基于估算"的标注

- **Sprint C**：方向四（`forge self-test --ci`）——编排已有的碎片化入口
  - 新增 `cmd/forge/self_test.go`
  - 编排 `go test ./...` + `node harness/acceptance.mjs` + `forge validate --models`
  - 复用 `acceptance.mjs` 的 PASS/FAIL/NA 裁定逻辑
  - 新增 `--quick` 模式（仅运行快速子集）

**验收标准**：
- `forge doctor` 输出包含 "orphan packages: 2" 行（与 `checkOrphanPackages` 一致）
- `forge run --dry-run` 的成本估算行标注"基于历史"或"基于估算"
- `forge self-test --ci` 在零外部依赖环境下退出码可靠
- `go test ./...` 全绿，`forge accept: ACCEPTED`

#### 阶段二：基础架构（2-3 Sprint）

> **目标**：设计增量门禁的 ScopeResolver 算法并实现 MVP

- **Sprint D**：设计 `GateScope` 类型 + 检查分类元数据
  - 定义 `{ changed: Set<Path>, affected: Set<Path> }` 作用域类型
  - 为 `arch-check.mjs` 的 8 个检查添加增量能力声明
  - 实现 `ScopeResolver`：计算 changed + affected 文件集
  - 先验证设计：将 `checkOrphanPackages` 作为"不可增量"检查的参考实现

- **Sprint E**：实现 `GateScheduler` + 集成到现有 gate 执行流程
  - 调度器根据检查类型分发到 full-scan/delta-scan/skip
  - 集成到 `forge gate`（`--scope` flag）
  - 诚实降级：增量模式下的"通过"不保证全量下的"通过"

**验收标准**：
- 改动 1 个文件时，体积检查和反模式命名检查只扫描 1 个文件
- 改动 1 个导出函数的文件时，扇入检查扫描该文件 + 所有引用方
- 改动任何文件时，循环依赖检查始终全量扫描
- `forge gate --scope auto` 的耗时与改动文件数大致成正比

#### 阶段三：治理深化（1-2 Sprint）

> **目标**：扩展治理产物卫生能力

- **Sprint F**：在 `check.py` 中新增 `check_unreferenced_assets`
  - 对 agent 卡、skill 卡、workflow 文件、ADR 进行引用图分析
  - 输出未引用资产的列表（按最后修改日期排序）
  - 基于 Sprint 31 的 `check_workflow_mode_gating` 检查实现模式

- **Sprint G**：新增 `forge governance report` 命令
  - 输出治理资产的引用覆盖率（引用/总数）
  - 按类型分类（agent/skill/workflow/policy/adr）
  - 标记状态为 `deprecated` 的资产

**验收标准**：
- `check.py` 在新项目上运行时不产生假阳性（全为零引用因为项目尚未连入任何 workflow）
- `forge governance report` 输出包含引用覆盖率行
- 覆盖率不产生千分位误报（1/3 = 33%，而非 33.333%）

### 5.3 风险点和缓解策略

| 风险 | 概率 | 影响 | 缓解策略 |
|------|------|------|----------|
| Go import 图推导忽略了 conditional build tags | 中 | 低 | 使用 `go list -json -tags` 覆盖常见标签组合 |
| 增量门禁 ScopeResolver 的 affected 集合膨胀到接近全量 | 中 | 高 | 设置 affected 膨胀警戒线（超过全量的 80% 自动降级为全量） |
| 成本历史数据稀疏导致预估算波动大 | 低 | 中 | 在 scorecard 中设置最小样本数阈值（`min_samples: 3`），不足时使用硬编码单价 |
| `forge self-test --ci` 被误认为权威测试入口 | 低 | 低 | 文档明确标注"编排测试，非替代品"；GO 测试和 acceptance 仍独立可运行 |
| 治理资产引用检测误报高 | 中 | 中 | 使用"建议"模式（warn）而非"阻断"模式（fail）运行第一个版本，收集反馈 |

### 5.4 诚实标注

作为架构纪律，我标注以下不可验证的假设：

1. **方向二的时间与复杂度估算假定 ScopeResolver 的 affected 传播算法与现有的风险分析传播算法（`risk.FromChangedPaths`）类似**。如果实际 implementation 发现 affected 传播需要进行完整的 AST 分析（而非启发式文件名匹配），则方向二的 sprint 估算应加倍。

2. **方向五的治理产物引用检测假定 agent 卡和 skill 卡的文件名是稳定的标识符**。如果未来引入文件名重命名 → 所有引用的 workflow YAML 都需要更新——但这已被 `check.py` 的引用存在性验证覆盖。

3. **方向四的 `forge self-test --ci` 在 CI 环境中的行为依赖本环境可运行的测试命令（`go test`、`node`、`python3`）。** 如果未来引入需要外部资源（如 SCA 数据库）的测试，`--ci` 路径应自动降级这些为 N/A（`acceptance.mjs` 已有先例）。

---

## 总结

验证报告的核心贡献是确认了三个真正原创的方向（死代码治理、增量门禁、治理产物卫生），并纠正了关于系统现状的两个不正确声明（成本预估算已存在、自测试已分析）。这些修正揭示了一个重要事实：**系统在常规引擎能力上的成熟度高于外部分析描述，但元治理能力——治理系统治理自身的能力——存在空白**。

五个方向按此顺序实施：

1. **方向一（死代码治理，P1）** → 最低成本、最高杠杆的元治理增强
2. **方向三（成本预估算增强，P1）** → 基于现有基础设施的快速接线
3. **方向四（`forge self-test --ci`，P1）** → 从分析文档到可执行 CLI
4. **方向二（增量门禁，P2）** → 高价值但需要设计作用域传播算法
5. **方向五（治理产物卫生，P3）** → 价值随治理规模增大而增长

关键架构选择：所有方向都可以在**零新外部依赖**下完成——保持 forge-core 的纯标准库纪律和 harness 的 host-independent 执法原则。
