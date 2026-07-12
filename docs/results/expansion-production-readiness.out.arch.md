我现在对整个语境有了完整的理解。下面是我的架构分析。

---

# 架构分析报告：ForgeOS — 从 95% 功能完备到生产就绪的桥梁工程

## 1. 架构评估

### 1.1 当前架构的优势

**1.1.1 精炼的脊柱架构（Spline Architecture）**
ForgeOS 的脊柱架构（DISCOVER → DESIGN → REVIEW → BUILD → EVOLVE）是一个经过实战检验的核心抽象。它并非简单的管线（pipeline），而是一个带反馈回路的闭环系统——每个阶段都有停止条件（stop condition）和回退机制（loop-back）。这种设计使得 31 轮 sprint 以来所有功能引擎（编排、路由、记忆、收敛、信号、并行）都能在脊柱上「挂载」，而无需改动脊柱本身。这是架构弹性（architectural elasticity）的实证。

**1.1.2 中枢旋钮设计（mode × lifecycle）**
七个维度（mode × lifecycle）构成一个正交控制平面，同时驱动机器人路由器档位、Harness 严格度、工作流深度。这是一个高度精简的「旋钮抽象」——从 explorer→engineering 的迁移只需一次状态变更，自动派生补测试/CI/监控任务。这种设计在同类 AI-SDLC 工具中极为少见，是 ForgeOS 最核心的架构创新。

**1.1.3 带外执法层（Out-of-Band Enforcement）**
「真相之源 = 带外执法层」这一决策是务实的架构现实主义。Sandbox/CI runner 独立于任何宿主 CLI，hook 适配器（PostToolUse/Stop）仅为加速器而非地基。这避免了与宿主工具（Claude Code、Cursor 等）的紧耦合，使得 ForgeOS 的治理模型不受宿主 CLI 版本升级的影响。

**1.1.4 严格执行零外部依赖**
`forge-core` 全部 18+ 包纯 Go 标准库、`harness` 零外部 npm/PyPI 依赖。这在 31 轮 sprint、~32k LOC 的规模下仍然保持，是纪律性架构治理的成果。直接收益：无供应链攻击面、无版本冲突、CI 安装时间 ≈ 0。

**1.1.5 完备的治理执法层**
8 项架构检查（layering/包/扇入/认知/反模式命名/函数长度/循环依赖/drift-guard）+ 10 项治理检查（check.py），加上硬编码 secret 扫描。这套体系已经超越了大多数开源项目，接近企业级治理成熟度。

### 1.2 关键架构债务与技术债

**1.2.1 运行时状态的无隔离共享（P1）**
`.forge/` 目录是所有运行时状态的单一交汇点——checkpoint、trace、memory、scorecards、approval markers 全部共享同一目录，无 run-ID 隔离，无并发锁，无垃圾回收。证据（来自 `docs/requirements/forges-five-hidden-product-quality-gaps.md`）：

- `trace.go:105-106`：Tracer 的 `seq` 是进程内变量，跨运行重置 → seq 冲突实证 91 行中 84 个重复
- `checkpoint.go`、`memory.go`、`scorecard_wind.go` 全部使用硬编码路径，无 session 前缀
- 并行 `forge evolve` 会导致 checkpoint/trace/memory 静默覆盖/交错

**这是最紧急的架构债务**——在宣称「24h 无人值守自治运行」下，它构成了静默数据损坏的定时炸弹。

**1.2.2 默认执行器是 DryRunExecutor（P1）**
`main.go:242` 的 `--executor dry` 默认值意味着：首次接触 ForgeOS 的用户看到的输出全是叙述性的——没有真实 trace、没有 memory 条目、没有 scorecard 更新、没有任何收敛信号。整个学习循环（`memory.Append` / `trace.Emit` / `converge.Converge`）在默认配置下**永远不会被执行**。

这是一个**架构契约违反**（architectural contract violation）：产品承诺「24h 自治学习循环」，但默认行为是「叙述性演练」。更根本的是，这种默认值使得核心代码路径在用户环境中从未被测试过——是一种隐蔽的「假阴性通过」。

**1.2.3 审批协议的二元性限制**
当前 `approve.go` 只有二元 `list` 能力，审批状态是简单的 `.forge/<stage>.approved` / `.rejected` marker 文件。在需要条件批准、委派、部分批准等协作场景下，这种二元协议严重限制了 ForgeOS 在多人团队中的应用。

**1.2.4 记忆系统的信任扁平化**
`memory.go` 的 `Query` 对所有条目一视同仁——来自 `planner` 和来自 `implementer` 的 entry 权重相同，置信度（Confidence）虽然被记录（`prompt_memory.go:178-185` 有展示层消费），但在查询层未被用于加权排序。这导致记忆质量无法自我净化。

**1.2.5 缺乏补偿撤销（Compensation Rollback）**
当前，当 `loopBackTo` 耗尽且 gate 仍 FAIL 时，系统直接 abort——不撤销已产生的副作用（git commit、checkpoint 写入、trace 记录）。对于「迭代是不可逆的」这一生产信任前提，这是一个硬缺口。

### 1.3 架构债务的根本原因分析

| 债务项 | 根本原因 | 引入阶段 | 偿还成本 |
|--------|---------|---------|---------|
| 状态无隔离 | 早期 sprint 为简化实现选择了单目录设计 | Sprint 1-5 | ~1 sprint（目录重构 + 迁移脚本） |
| 默认 dry-run | 为确保安全默认行为，但未考虑产品价值体验 | Sprint 1-3 | ~2 sprints（执行器选择逻辑 + 引导流程） |
| 审批二元性 | MVP 阶段只识别「批准/拒绝」二元状态 | Sprint 8-12 | ~2 sprints（数据模型 + 迁移 + CLI） |
| 记忆信任扁平 | 未预见多 agent 来源的信任差异 | Sprint 15-20 | ~1 sprint（查询层加权 + 配置） |
| 无补偿撤销 | 未将「迭代失败」视为运营事件 | Sprint 20-25 | ~2 sprints（git tag + 补偿引擎 + rollback CLI） |

---

## 2. 扩展方向

基于全部阅读的文档，我分析出以下 5 个高价值架构扩展方向，按**业务价值 × 技术可行性 × 现有基础**三维排序。

### 方向一：运行时状态生命周期管理（P0）

**为什么需要**：这是生产就绪的**前提条件**而非功能增强。数据完整性是 24h 自治运行的非可协商基础。

**核心挑战**：
- 向后兼容：现有单目录状态文件必须能无缝迁移到新结构
- 并发模型：需要文件级锁（`flock`）而非进程级锁，以避免跨进程竞争
- GC 策略：需要确定「何时保留/何时清理」的决策标准——按运行次数、按时间、按大小、还是按模式

**架构变更**：
```
当前：.forge/{checkpoint.json, trace.jsonl, memory.jsonl, scorecards.json, *.approved}
目标：.forge/
  ├── runs/
  │   ├── <run-id>/
  │   │   ├── checkpoint.json
  │   │   ├── trace.jsonl
  │   │   └── memory.partial.jsonl
  │   └── latest -> <run-id>  (symlink)
  ├── memory/ (跨运行持久知识)
  │   └── store.jsonl
  ├── approvals/
  │   └── <stage>.json
  ├── trace/ (汇总 trace，可选)
  │   └── archive/
  └── LOCK
```

**设计决策权衡**：

| 选项 | 优点 | 缺点 |
|------|------|------|
| **A: 目录重构 + 软迁移** | 完全向后兼容；可逐步迁移 | 迁移逻辑增加复杂度；旧路径需要兜底 |
| **B: 增加 run-ID 前缀（`checkpoint.<run-id>.json`）** | 变更最小；无需目录结构重构 | 文件数量爆炸；无法 group cleanup |
| **C: 嵌入式 SQLite** | 强一致性；查询能力 | 违反零外部依赖红线；二进制体积增大 |
| **推荐：A** | — | — |

**对现有系统的影响**：
- `persist.Load/Save` 需要新增 `RunID` 参数
- `trace.NewTracer` 需要 `RunID` 而非纯 `io.Writer`
- `memory.go` 需要区分「运行时临时 memory」和「持久 memory store」
- 对外的「最新状态」通过 `latest` symlink 保持兼容

---

### 方向二：补偿撤销与编排级自愈（P0）

**为什么需要**：AI 编排的不可逆性是采用的最大心理障碍。用户需要确信「如果这次 evolve 搞砸了，我能回到之前的状态」。这是从「玩具」到「生产工具」的门槛。

**核心挑战**：
- Git 操作的自动化不确定性——merge conflict 无法自动解决
- 不可逆操作的检测（DB migration、secret 轮换、breaking schema change）
- 补偿链的定义：一个 phase 的撤销可能导致上游 phase 状态不一致

**架构变更**：
```
当前：evolve → gate FAIL → abort（无撤销）
目标：evolve → pre-phase git tag → phase execute → post-phase git tag
       → gate FAIL → compensate chain → converge → trace
```

**关键设计决策**：

| 决策 | 选项 | 推荐 | 理由 |
|------|------|------|------|
| 回滚机制 | git tag vs backup dir vs checkpoint | **git tag** | 复用 git diff/merge 能力；可见性好；事后审计自然 |
| 补偿执行方式 | 同进程 vs 子进程 | **同进程** | 不需要拉起新进程；补偿 phase 只是特殊标记的 phase |
| 不可逆检测 | 静态规则 vs 动态分析 | **静态规则（硬编码已知模式）** | 动态分析复杂度高且误判成本不可接受 |

**对现有系统的影响**：
- `asset.Phase` 需新增 `CompensatePhase *string` 和 `CompensateAction *CompensateAction`
- `orchestrator/engine.go` 需在 phase 生命周期中插入 `pre/post` 钩子
- 新增 `internal/orchestrator/compensator.go`（约 200 行核心逻辑）
- 新增 `cmd/forge/rollback.go`（CLI）

---

### 方向三：记忆系统的信任加权与自净化（P1）

**为什么需要**：当前 memory store 是「写后即忘」模式——所有 agent 写入的条目具有同等权重。随着系统运行时间增长，噪声条目会淹没高信号条目。更根本的是，没有机制检测不同 agent 间的矛盾信息。

**核心挑战**：
- 信任权重的声明模型：谁来定义权重？用户？系统？还是数据驱动？
- 矛盾检测的假阳性率：gate 结果为 false 可能是代码未完成，不意味着记忆是错误的
- 知识验证（verify‑knowledge）工作流的触发策略：每次 evolve 都跑？定时跑？手动触发？

**架构变更**：

```
当前：memory.Append(entry) → store.jsonl → Query(kind, topic) → 平等排序
目标：memory.Append(entry, source, confidence) → store.jsonl
       → QueryWeighted(kind, topic, policy) → 按 source×weight×confidence 排序
       → DetectContradictions → AnnotateStaleness → 注入 prompt
```

**关键设计决策**：

| 决策 | 选项 | 推荐 | 理由 |
|------|------|------|------|
| 权重声明位置 | YAML vs 内嵌 vs env | **YAML**（`.agent/policies/memory.yml`） | 声明式可版本化；agent 卡可覆写 |
| 矛盾检测粒度 | entry 级 vs topic 级 | **topic 级** | 同一 topic 下矛盾才有意义 |
| 知识验证策略 | 被动（触发）vs 主动（定时） | **被动**（`forge evolve --verify-knowledge`） | 主动验证引入非确定性行为 |

**对现有系统的影响**：
- `memory/memory.go` 的 `Query` 保留不变（向后兼容），新增 `QueryWeighted` 方法
- `cmd/forge/prompt_memory.go` 的 `boundMemory` 改为调用 `QueryWeighted` 排序
- 新增 `internal/memory/contradiction.go`（矛盾检测核心）
- 新增 `internal/workflow/verify_knowledge.go`（验证工作流）

---

### 方向四：协作人机决策协议增强（P1→P2）

**为什么需要**：当前审批协议仅支持二元「批准/拒绝」。在团队协作场景中，需要条件批准（「批准，但使用 PostgreSQL」）、委派（「这个交给 Alice 审核」）、部分批准（「原型模式批准，生产闸门保留」）、超时升级（「48h 无人审批 → 自动升级到 TL」）。

**核心挑战**：
- 与现有 marker 文件的向后兼容：`.forge/<stage>.approved` 文件必须能被新系统识别
- 条件批准的复杂工作流状态爆炸：N 个条件在不同 lifecycle phase 的组合达 2^N
- 委派链的循环检测：A→B→C→A 的委派链需要提前阻断

**架构变更**：

```
当前：.forge/<stage>.approved / .rejected（二元 marker）
目标：.forge/approval/
       ├── <stage>.json（完整 ApprovalRequest）
       ├── <stage>.conditions.json（条件清单）
       └── <stage>.delegation.json（委派链）
    旧格式自动识别为 APPROVED（迁移桥接）
```

**关键设计决策**：

| 决策 | 选项 | 推荐 | 理由 |
|------|------|------|------|
| 持久化方式 | marker 文件 vs 独立 store | **marker 文件** | 无外部依赖；原子写入复用 `persist` 的 `temp+rename` |
| 条件类型数量 | 不做限制 vs 固定 3 种 | **固定 3 种**（approve_with_changes / redirect / partial） | 限制状态空间避免组合爆炸 |
| 通知方式 | 内建 vs 插件 | **接口 + 默认 stdout** | `Notifier` 接口可扩展；初始实现 log-only |

**对现有系统的影响**：
- `internal/approval/` 全新包，约 5 个文件
- `cmd/forge/approve.go` 重构（新增 `--condition`、`--delegate`、`timeout` flags）
- `gate.go` 读取统一改为 `internal/approval/marker.go`

---

### 方向五：多仓联邦编排（P3）

**为什么需要**：单体仓库是起点，但真实世界的组织有 N 个仓库，依赖关系错综复杂。ADR 0003 已讨论过 submodule，但无编排引擎。多仓编排是 ForgeOS 从「单项目工具」到「组织级平台」的跃迁。

**核心挑战**：
- 拓扑依赖的循环检测和死锁预防
- 跨仓原子演化：N 仓中 1 仓失败，其余 N-1 仓需自动回滚
- 全局信号聚合：各仓的 gate 信号如何聚合法决策级信号

**架构变更**：

```
当前：--root <single> → orchestrator.Run()
目标：--topology topology.yml → 多仓依赖图
       → 拓扑排序 → 并行无依赖仓 → 串行依赖链
       → 全局信号聚合 → 原子回滚（如有失败）
```

**关键设计决策**：

| 决策 | 选项 | 推荐 | 理由 |
|------|------|------|------|
| 拓扑格式 | YAML vs TOML vs Go struct | **YAML**（`.forge/topology.yml`） | YAML 已在 check.py 中使用；团队熟悉 |
| 跨仓回滚 | git tag vs backup | **deferred-revert**（记录旧 HEAD，失败时 `git reset --hard`） | 避免 merge conflict 风险 |
| 信号聚合 | 全局 AND vs 加权投票 | **全局 AND**（AllGatesGreen && NoPendingP0） | 安全优先；加权可后续版本扩展 |

**对现有系统的影响**：
- `orchestrator/orchestrator.go` 的 `--root` 抽象为 `RootResolver` 接口
- 新增 `internal/orchestrator/polyrepo/` 包（~5 个文件）
- 依赖方向二的回滚能力（`R-006`）

---

## 3. 接口设计建议

### 3.1 关键接口设计原则

**原则 I: 策略模式（Strategy Pattern）优先于条件分支**
Memory 的选择策略（衰减/语义/紧凑）应当使用策略接口：

```go
type SelectStrategy interface {
    Select(entries []MemoryEntry) []MemoryEntry
    Name() string
}
```

三种实现（`TimeDecayStrategy`、`RelevanceStrategy`、`CompactStrategy`）可组合使用。新增策略不需要修改核心逻辑。这是当前 `memory.go` 中已经部分体现的模式（有 `Compact` 接口），需要扩展到策略层。

**原则 II: 接口隔离（Interface Segregation）**
`orchestrator` 包中对 `Topology` 的依赖应当抽象为最小接口：

```go
type RootResolver interface {
    RootFor(repo string) (string, error)
    AllRoots() []string
}
```

而非在 `orchestrator` 中直接引用 `Topology` 结构体。这保持了单根模式（当前行为）和 MultiRoot 模式的一致抽象。

**原则 III: 向后兼容优先于完美设计**
所有数据模型变更必须保留旧格式的读写能力，周期至少为 2 个 sprint：

- `memory.Query` 保持原语义，新增 `QueryWeighted` 而非修改行为
- approval marker 旧 `.forge/<stage>.approved` 自动识别为 APPROVED
- `--executor dry` 保持为默认值，但新增 `forge init --guided` 引导用户启用真实执行器

### 3.2 是否需要新的抽象层

**需要：一个 `internal/runtime/` 包（或类似）作为运行时状态管理中心**

当前，运行时状态管理散落在多个包中——`persist` 负责 checkpoint、`trace` 负责日志、`memory` 负责知识。没有一个统一的「运行时上下文」来协调 run-ID 分配、目录初始化、GC 触发。

```
internal/runtime/
├── context.go     // RunContext: RunID, Mode, Lifecycle, StartTime
├── directory.go   // .forge/ 目录管理，版本迁移，GC
├── lock.go        // 文件级锁（flock）、死锁检测
└── identity.go    // Run ID 生成（ULID）、跨运行一致性
```

这个包不代替各功能包的职责，而是提供它们共同需要的「运行时身份和生命周期上下文」。

### 3.3 保持向后兼容性的具体策略

| 变更类型 | 兼容策略 | 转换期 |
|---------|---------|--------|
| `.forge/` 目录结构变更 | 读取时同时检查新旧路径；自动 symlink `latest` | 4 sprints（永久保留旧路径兜底） |
| approval marker 升级 | 旧 `.approved` 文件自动识别为 APPROVED；写入时双写 | 2 sprints（sprint 后只写新格式） |
| `Trace` seq 连续性 | `trace.jsonl` 头记录 last-seq；新 Tracer 从 last-seq 继续 | 即时（不设转换期） |
| Memory Query 加权 | `Query` 保持原行为，新增 `QueryWeighted`；`prompt_memory.go` 改为调用 `QueryWeighted` | 无需转换期（纯新增） |

---

## 4. 技术选型

### 4.1 是否需要引入新技术栈

**结论：不需要。** 所有五个方向都可在现有技术栈内完成：

| 需要的能力 | 实现方式 | 是否引入新依赖 |
|-----------|---------|--------------|
| Git 操作（tag/revert） | `os/exec` 调用 git CLI | 否（外部 git CLI） |
| 文件级锁 | `golang.org/x/sys/unix.Flock` 或 `os.OpenFile` + 自旋锁 | 否（golang.org/x 已隐式依赖） |
| 目录自动清理 | `os.RemoveAll` / `filepath.Walk` + 时间判断 | 否 |
| YAML 解析 | `gopkg.in/yaml.v3` 或 `encoding/json` + 结构体标记 | **需评估** |
| 策略模式 | Go 接口（无需新依赖） | 否 |
| 定时/超时 | `time.After` / `time.Ticker` | 否 |

**关于 YAML 解析的决策**：

| 选项 | 优点 | 缺点 |
|------|------|------|
| 使用 `gopkg.in/yaml.v3` | 标准 YAML 解析；团队熟悉 | 引入外部依赖（违反零外部依赖信用） |
| 使用 `encoding/json` + JSON Schema | 零新依赖 | JSON 在配置文件中不友好（无注释、多行困难） |
| 自定义解析器（极简子集） | 零外部依赖；可精确控制语法 | 实现成本 0.5-1 天；边缘情况多 |
| **推荐方案：混合** | 配置使用 JSON 格式（`.forge/safety.json`、`.forge/topology.json`），用 `encoding/json` 解析 | 零外部依赖；JSON 在 VSCode 中已有 schema validation 支持 |

**红线重申**：`forge-core` 必须保持零外部 Go 依赖。如果 YAML 需求变得不可回避，应在 `harness/` 层（Node.js，已有 `js-yaml`）解析后生成 JSON 传递给 forge-core。

### 4.2 自建 vs 采购的决策依据

| 需求 | 选项 | 推荐 | 理由 |
|------|------|------|------|
| Git 操作 | 自建（`os/exec`）vs libgit2 绑定 | **自建** | libgit2 引入 C 依赖和构建成本；`os/exec` 覆盖全部需求 |
| 通知发送（Slack/Email） | 自建 vs 第三方 SDK | **接口抽象 + 初始 stdout** | 依赖外部 SDK 违反零依赖；通知逻辑本身简单（HTTP POST） |
| 调度队列 | 自建 vs Redis/NSQ | **自建** | daemon 是本地调度，不需要分布式队列；goroutine + channel 足够 |

### 4.3 评估标准（用于未来技术选型）

```
1. 是否符合零外部依赖红线（forge-core）？
2. 是否需要在用户环境安装/编译（增加用户摩擦）？
3. 是否在 CI 环境中可用（sandbox/无网络）？
4. 是否有替代的纯标准库实现？
5. 是否可被泛化为接口供将来替换？
```

---

## 5. 实施路线图

### 5.1 优先级总排序

| 优先级 | 方向 | 原因 | 预估工作量 |
|--------|------|------|-----------|
| **P0** | 运行时状态生命周期管理 | 数据完整性前提；无此方向其他所有方向都在不稳定地基上 | ~1 sprint |
| **P0** | 补偿撤销与编排级自愈 | 生产信任前提；方向一（多仓）和方向三（daemon）依赖此方向的回滚能力 | ~2 sprints |
| **P1** | 记忆系统信任加权与自净化 | 记忆质量决定系统长期价值；已有半成品代码可复用 | ~2 sprints |
| **P1** | 审批协议协作增强 | 团队协作场景必备；当前二元协议过于局限 | ~2 sprints |
| **P2→P3** | 多仓联邦编排 | 平台级能力但依赖 P0 方向完成；单仓场景仍可满足 80% 用户需求 | ~2-3 sprints |

### 5.2 阶段划分与里程碑

```
Phase 0（Sprint 32-33）: 地基加固
  ├── 运行时状态生命周期管理（完整）
  ├── 默认执行器引导（forge init --guided）
  └── 里程碑 M0: .forge/ 目录隔离完成，无回归

Phase 1（Sprint 34-35）: 可撤销编排
  ├── 补偿撤销方向（完成）
  ├── 记忆信任加权（数据模型 + 查询层）
  └── 里程碑 M1: forge rollback 可用，记忆带信任排序

Phase 2（Sprint 36-37）: 协作与净化
  ├── 审批协议增强（完整）
  ├── 记忆系统自净化（矛盾检测 + 知识验证）
  └── 里程碑 M2: 条件批准 + 委派 + 超时可用，记忆带自净化

Phase 3（Sprint 38-40）: 平台化
  ├── 多仓联邦编排（V1）
  ├── 跨方向集成测试 + 性能 benchmark
  └── 里程碑 M3: 多仓编排 V1 可用，全部方向集成测试通过
```

### 5.3 风险点与缓解策略

| 风险 | 概率 | 影响 | 缓解策略 |
|------|------|------|---------|
| 运行时状态迁移导致现有用户数据丢失 | 低 | **极高** | 双写 4 sprints；迁移前自动备份；`forge doctor` 健康检查 |
| Git revert 在 CI 浅克隆中不可用 | **高** | 高 | CI 检测到浅克隆时回退到文件级 diff（`risk_diff.go` 的 `FromChangedPaths`） |
| Memory namespace merge 冲突 | 中 | **高** | 后写入优先 + confidence: 0.5；`FORGE_MEMORY_MERGE_STRATEGY=manual` 逃生口 |
| 审批条件状态组合爆炸 | 中 | 中 | 限制条件类型为 3 种；不支持嵌套或连锁条件 |
| 多仓编排的循环依赖 | **高** | 中 | `Topology.Validate()` DFS 循环检测；`--force` 仅标记 unstable |
| 五方向同时开发导致 orchestrator.go 修改冲突 | **高** | **高** | 提前提取接口（`MemoryStore`、`RootResolver`、`PhaseHook`）；严格 feature branch |
| 团队对零依赖红线产生压力 | 中 | 中 | 文档明确例外条件（如 YAML 解析 → harness 层处理）；红线不松动 |

### 5.4 非功能性需求

| 维度 | 目标 | 衡量标准 |
|------|------|---------|
| 向后兼容 | 新旧格式并行 ≥ 4 sprints | 所有旧 `.forge/` 结构用户可无缝升级 |
| 性能 | 架构变更导致的基准退化 < 5% | `go test -bench=.` 对比 sprint 前后 |
| 正确性 | 所有方向集成测试 100% 通过 | CI gate 强制执行 |
| 文档 | 每个方向有独立 ADR + 用户文档 | `docs/adr/` + `docs/` 分别记录 |
| 外部依赖 | 0 新外部 Go 依赖 | `go.mod` diff 检查 |

---

## 总结

ForgeOS 已经完成了最困难的部分：构建了一个功能完备的 AI-SDLC 编排引擎，拥有治理执法层、学习闭环、中枢控制平面。剩下 5% 的差距不是「更多功能」，而是**从功能完备到生产就绪的桥梁工程**——运行时数据完整性、可撤销性、记忆可信任性、协作可扩展性。

这五方向的共同主题是**信任基础设施**（Trust Infrastructure）：用户需要信任系统不会丢失数据（方向一）、信任错误可以回滚（方向二）、信任记忆是有质量筛选的（方向三）、信任审批反映了团队意图（方向四）、信任跨仓编排是一致的（方向五）。

建议在实施开始时，先完成**一个最小垂直切片**来验证桥梁工程的可执行性——选择方向二的补偿撤销（因为其复用性最高）中的一个核心任务（git tag + rollback CLI），在 1 周内完成端到端交付。这个垂直切片一旦通过，剩余四个方向的可执行性就得到了实证验证。
