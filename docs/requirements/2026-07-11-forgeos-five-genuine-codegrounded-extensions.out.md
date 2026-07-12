现在我拥有完成验证所需的所有数据。让我给出全面评估。

---

## 评估：五个扩展方向验证报告

### 方法论

我已对每个方向做了以下验证：
1. **代码证据**：读取所有引用的 Go 文件，验证变量、函数和行号的存在性
2. **已有覆盖率**：在 `docs/requirements/`（229 篇）+ `docs/analysis/`（40 篇）中搜索每个方向的核心命题，检查是否存在相同的系统化展开
3. **边界场景**：确认每个方向的边界场景是否源自实际代码约束

---

### 验证结果

| # | 方向 | 代码证据准确性 | 已有覆盖率 | 真实性判断 |
|---|------|--------------|-----------|-----------|
| 1 | 执行溯源与可回放 | 🟡 位置有误，概念正确 | 🟡 部分覆盖 | 方向概念已被展开 |
| 2 | 状态外部队改防护 | 🟢 准确 | 🟢 新 | 真实系统性缺口 |
| 3 | 外向通知总线 | 🟢 准确 | 🟢 新 | 真实系统性缺口 |
| 4 | 策略合规漂移检测 | 🟢 准确 | 🟢 新（不同角度） | 真实系统性缺口 |
| 5 | 多仓库工作区编排 | 🟢 准确 | 🔴 已有覆盖 | 已被系统化展开 |

---

### 详细分析

#### 方向一 · 执行溯源与可回放 → 🟡 **差异化验证未通过**

**代码证据问题**：
- `observeFor` 位于 `cmd/forge/prompt_context.go:180`，**非** `engine_build.go`
- `reportConvergence` 位于 `cmd/forge/main.go:399`，**非** `gates.go`
- `Event` 结构体中确实缺少 PromptText/OutputText/FileDelta 字段 —— 此观察准确
- `AgentExecutor.Execute` 返回 `error` 而非结构化执行证据 —— 此观察准确

**已有覆盖率：该方向已被系统化展开**

`docs/requirements/five-genuinely-unexplored-code-level-architectural-expansions.md` 中有一个完整章节 **「方向二：离线回放引擎与故障取证（Replay Debugger / Session Forensics）」**，涵盖：
- 阶段级时间线重建
- Prompt 保存（`--save-prompts` 标志写入 `.forge/prompts.jsonl`）
- 失败根因分类（gate / 超时 / 预算 / reviewer 拒绝）
- 跨运行对比
- 具体代码入口点（`trace.go:30-104`, `doctor/anomaly.go:95-155`, `cmd/forge/status.go`）

`docs/analysis/seventh-wave-data-realism.md` 中也有 **「方向 5：真实数据回放（Replay Testing from `.forge/` 目录）」**。

用户将 PromptText/OutputText/FileDelta 加入 Event 结构体的切入点更细粒度，但**整体方向**（可回放的执行溯源）已在之前的两篇文档中作为系统性扩展展开过。已有文档明确指出了相同的缺口：「当前不保存 prompt 原文——这是最大取证缺口」。

---

#### 方向二 · 状态外部队改与防护 → 🟢 **差异化验证通过**

**代码证据**：全部准确
- `persist/checkpoint.go`：使用 `rename(2)` 原子性，但无 fcntl `flock`、无跨文件一致性校验
- `memory/memory.go`：使用 `O_APPEND`，但 Load 时不校验交错写入损坏
- `trace/trace.go`：`sync.Mutex` 仅防护单进程并发，不防护多进程写冲突

**差异化验证**：`genuine-uncovered-five-binary-state-output-session-datalifecycle.md` 和 `five-foundational-resilience-gaps.md` 讨论了状态文件的生命周期管理（轮转、压缩、格式兼容性），但**从未**讨论外部篡改检测（`flock`、文件校验和、多进程冲突检测、跨文件时间偏差验证）。此角度确实新颖。

---

#### 方向三 · 外向通知总线 → 🟢 **差异化验证通过**

**代码证据**：全部准确
- `reportConvergence` 使用 `fmt.Printf`（`main.go:409`）—— 只有 stdout
- 全系统无 `EventSink` 接口、无 webhook 注册、无事件总线
- `gates.go` 和 `evolve.go` 中的所有输出都是 stdout 或本地 JSONL 文件

**差异化验证**：`five-architectural-product-expansion-directions.md` 在一行中提到了 "事件总线抽象" 作为告警集成的一部分，而 `expansion-horizon-three.md` 方向三讨论了事件驱动调度，但**两者都没有将外向通知作为完整的系统性方向展开**，包含 EventSink 接口设计、多种 Sink 实现（stdout / JSONL / Webhook / Slack）、以及产品化 CI/CD 集成场景。此文对此方向提供了更全面的分析。

---

#### 方向四 · 策略合规漂移检测 → 🟢 **差异化验证通过**

**代码证据**：全部准确
- `routing.go:139-140`：`HaikuMax = 0.34`, `SonnetMax = 0.69`
- `routing.go:145-157`：`TaskTypeFloor` 映射
- `routing.go:162-166`：`SafetyForceOpus` 映射
- `attribution.go`：`AgentTaskType` 映射（硬编码 agent→task_type 关系）
- YAML 声明：`.agent/routing/policy.yml` 和 `.agent/policies/modes.yml` 定义了相同的概念

**差异化验证**：`novel-five-perspectives-2026-07-10.md` 中的 **「方向一 · 跨项目治理策略漂移检测与调和」** 讨论了**跨项目**治理漂移（origin vs child），关注的是项目间策略版本锁定问题。此文讨论的是**项目内** YAML vs Go 实现的不一致性 —— 两个完全不同的域。前者关于治理分发，后者关于声明-实现合规。跨项目分析中提到的 `five-genuinely-uncovered-frontiers.md` 方向五（"声明 vs 实现"一致性）仅局限于单一项目范围内，且作为治理测试框架的一部分提及，而非作为独立的合规漂移检测方向。

---

#### 方向五 · 多仓库工作区编排 → 🔴 **差异化验证未通过**

**代码证据**：全部准确
- `gate.go:RepoRoot` 返回单一根目录
- `main.go:loadWorkflow` 所有路径相对于单根目录
- `memory.go` 为每个仓库使用独立路径

**已有覆盖率：该方向已被系统化展开**

`docs/requirements/expansion-horizon-three.md` 中有一个完整章节 **「方向二：多仓库联邦治理 —— 从单仓到多仓生态」**，涵盖：
- 每个仓库各自独立 evolve，无跨仓库视角
- 共享库变更断裂 / API 契约演进 / 聚合预算 / 统一策略继承 / 跨仓库问题追踪
- 代码证据：`grep` 显示零跨仓库概念；`asset.Workflow` 无 `depends_on_repo` 字段；每个仓库独立的 ROADMAP
- 边界场景：共享库 breaking change、跨仓库 API 契约演进、聚合预算耗尽、安全策略统一升级、跨仓库问题追踪
- 产品价值：真实世界适配、依赖感知编排、统一策略管控

多份其他文档中也广泛覆盖了多仓库/跨仓库概念。

---

### 关于代码级证据准确性的说明

| 用户声明 | 实际位置 | 准确性 |
|---------|---------|--------|
| `trace.go:36-57` Event 结构体 | 正确 | 🟢 |
| `orchestrator/executor.go:20-22` Execute 接口 | 正确 | 🟢 |
| `observeFor` 在 `cmd/forge/engine_build.go` | 实际在 `cmd/forge/prompt_context.go:180` | 🟡 文件名错误 |
| `reportConvergence` 在 `cmd/forge/gates.go` | 实际在 `cmd/forge/main.go:399`（`orchestrator/loop.go:346` 尚有另一个版本） | 🟡 文件名错误 |
| `cost.go:unwrapClaudeResult` | 正确（第 420 行） | 🟢 |
| `routing.go` 中的 `HaikuMax` / `SonnetMax` / `TaskTypeFloor` / `SafetyForceOpus` | 正确（第 139-166 行） | 🟢 |
| `policy.yml` 和 `modes.yml` 存在 | 正确 | 🟢 |
| `gate.go:RepoRoot` | 正确（第 37-44 行） | 🟢 |
| `evolve.go` 中的 `LoopEngine.Run` | 该文档中是组合/合成引用，非逐字引用 | 🟢 概念正确 |

少量文件名不准确（`reportConvergence` 和 `observeFor`）不会影响整体的架构分析 —— 代码行为和缺失之处描述正确。

---

### 优先级框架评估

| 方向 | 优先级 | 与工程红线的关联 |
|------|--------|-----------------|
| 方向一 · 执行溯源 | 🔴 P0 | 事故复盘 = 故障排除；未来部署时运维不可缺失 |
| 方向二 · 状态防护 | 🔴 P0 | 多进程/多机器数据完整性 —— 数据损坏导致错误决策 |
| 方向三 · 外向通知 | 🟠 P1 | 产品化/CI-CD 集成门槛 |
| 方向四 · 策略漂移 | 🟠 P1 | 安全合规：执行漂移是最难发现的 bug |
| 方向五 · 多仓库 | 🟡 P2 | 高价值，但已有覆盖，且不阻碍单仓库用户 |

我同意用户的优先级排序，但需说明方向一作为新方向的有效性较弱，原因在于前三波扫描中已有大量覆盖。真正的 P0 新方向是**方向二**。

---

### 总结

| 维度 | 评分 | 说明 |
|------|------|------|
| 代码证据质量 | 🟡 良好 | 存在少量文件位置不准确，但行为观察正确 |
| 差异化验证 | 🟡 3/5 通过 | 方向二、三、四确实新颖；方向一和五已有大量覆盖 |
| 分析深度 | 🟢 强 | 边界场景、产品价值和实现量级预估扎实 |
| 工程完整性 | 🟢 强 | 与现有架构及工程红线（纯 stdlib、零外部依赖）一致 |

**结论**：此文围绕 3 个真正新颖的方向（方向二→四）提供了扎实的分析，这些分析建立在准确的代码证据基础上，并在 269 份已有文档的交叉引用中经得起检验。方向一和五展示了强大的一致性遵守，但其差异化主张在现有语料库中已有充分展开。
