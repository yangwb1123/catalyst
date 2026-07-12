我已经完整阅读了全部关键架构文件（BOOTSTRAP、ARCHITECTURE、PROJECT、ROADMAP、CURRENT_SPRINT 31 个 sprint、DECISIONS、ADR-0003、north-star、project.yml、policies.yml、需求审计、五方向扩展文档）。下面是完整架构分析。

---

# 架构分析报告：ForgeOS 生产级扩展方向

## 1. 架构评估

### 1.1 当前架构的核心优势

**1.1.1 零依赖哲学带来的工程韧性**

`forge-core` 全部 18+ Go 包纯标准库、零外部依赖（`go.mod` 无 `require`），这不是自虐——这是有意识的架构决策（ADR-0002）。其价值在长生命周期项目中会被反复验证：

- **无供应链攻击面**：无 `left-pad` 等价物，无 transitive dependency 的 CVE 传播
- **二进制体积与启动延迟**：Go 静态二进制，CI 中 0 秒安装、0 秒解析依赖树
- **版本锁定成本为零**：不存在"go mod tidy 改变了间接依赖"的非功能性 diff

**1.1.2 多层闸门创造渐进式安全网**

三层执法构成决策漏斗：

```
edit-time (CC PostToolUse hook → gate.mjs 快速体积)
    → Stop (forge accept 聚合 8 检查 + test + secret-scan)
        → CI (.github/workflows/forge.yml 在远程重现 Stop 闸门)
```

这不是"防御深度"的空话——每一层有不同的响应者顺序：编辑者即时修 / agent 在循环中修 / PR 作者在合并前修。**三层共享同一真相源**（`policies.yml` + `acceptance-kernel.mjs` 的 PASS/FAIL/NA 裁决），确保偏移不会使闸门失效。

**1.1.3 `honesty N/A` 是架构级承诺，不是实现细节**

从 `sca.mjs` 到 coverage 适配器到 lint 适配器，"工具不存在则诚实 N/A，绝不伪造通过"的纪律是 ForgeOS 区别于大多数 CI 系统的关键差异。这不是"优雅降级"——这是**信任模型**的选择：

- 标准 CI：未配置 = 静默跳过 → 在安全审计中可能被误认为"已覆盖"
- ForgeOS：未配置 = 诚实标记 N/A → 在审计中明确该检查*未运行*

**1.1.4 中枢旋钮（mode × lifecycle）是真正可工作的架构抽象**

经 Sprint 7-15-18 验证，单一设置现在驱动 **4 个维度**：

```
mode × lifecycle → Router tier + Harness strictness + Workflow depth + Migration
```

这不是纸上设计——production lifecycle 的"一票否决强制全闸门"已通过注入测试实证。

---

### 1.2 当前架构的关键局限性

**1.2.1 成本控制是被动止损，前馈可见性为零**

`runBudget`（三层护栏：`--agent-max-budget-usd` / `--run-budget-usd` / `--max-agent-calls`）全是**后验**截断器。架构中没有起跑前的**预测**层：

```
现状：运行 → 花钱 → 若超限则停
理想：估算 → 显示预期范围 → 运行 → 若偏离则告警 → 写入记分卡 → 下次估算更准
```

`scorecard_wind.go` 的 `distinctScorecardPairs` 和 `HistoryTiebreak` 已构建历史基线，但至今无人读它来回答问题"这大概会花多少钱"。

**1.2.2 收敛信号依赖 agent 自评 + 粗糙启发式的混合体**

当前三个收敛信号中：

| 信号 | 信任模型 | 可伪造性 |
|---|---|---|
| RoadmapCompletion | 信任 agent（- [x] 勾选） | **高**（agent 只需写 `- [x] done`） |
| GatesGreen | 机械客观 | 零（闸门 PASS/FAIL 不受 agent 影响） |
| FileDelta | 启发式交叉验证 | **中**（通过选择匹配文件路径可以绕过） |

唯一的**行为验证**来自 harness 闸门（测试通过 / lint 无错误 / 复杂度在阈值内）。但这些验证的是*代码质量*，不是*功能正确性*。架构缺少面向*语义*的验证层。

**1.2.3 人审是二进制信号，缺乏真实评审所需的语义丰富度**

`.forge/<stage>.approved` 标记文件是**存在/不存在**信号。`on_rejected` loop-back（Sprint 31）提供跳转方向的能力，但二元"批准与否"的框架本身与协作团队的运作方式仍然脱节。架构层缺少结构化审批状态、条件批准、拆分建议、部分批准等模式。

**1.2.4 YAML 解析是临时 Python shim，已进入债务时间**

`harness/yaml2json.py` 通过 `python3` shell 调用来运行，是 S24 确定的临时脚手架。Sprint 30 的 `normalize.go` 修复了块标量损坏的 bug，但**核心事实不变**：

- forge-core 的零依赖纪律阻止引入 Go YAML 库
- PyYAML 是 `forge-go build` 链之外的运行时依赖
- Python shim 路径在容器化/沙箱化场景中成为脆弱点（Python 不可用时整个 workflow 编排中断）

这不是紧急危机（Python 几乎无处不有），但已越过"临时"的合理半衰期。

**1.2.5 可观测性是原始事件流的等价物**

`trace.jsonl` 记录了丰富的结构化事件，但诊断工具链停留在 `grep` / `jq` 级别。`forge scorecard --summary` 是唯一的聚合视图，但它仅限于记分卡数据，不涉及时序比较、失败根因分析或流程图。

---

### 1.3 架构债务与技术债

**真实架构债务（需结构重构）：**

1. **YAML 解析的 shim 边界**（严重程度：中等）。Python shim 违反"执行者依赖于零外部依赖"的合约。修复需要决策：是否为 forge-core 引入 Go YAML 依赖（打破零依赖链），或是保持多语言运行时占据。

2. **`cmd/forge` 包文件数预算反复接近上限**（严重程度：低）。S27/S29/S30 三次触碰 `max_files` 限制，每次靠"拆到 `internal/xxx` + 提升上限"的双步模式解决。这是一个**健康信号**（纪律在警察），但也表明 `cmd/forge` 的责任范围可能接近功能区分的自然边界。

3. **agent 执行器默认 dry-run **（严重程度：按设计存在）。这不是债务——这是有意识的 fail-safe 默认（S24 确认的 Eight Real Gaps）。但需要注意 `--executor command --agent-cmd claude` 模式在 S24-S26 之后仍然需要四维护栏明确启用。

**不是债务的有意限制（不应重构）：**

- `HistoryTiebreak` 在单候选架构下的有限召回：这是 v3 routing 就绪之前的架构边界
- `computeFileDelta` 的启发式性质：代码诚实宣称 "CHEAP HEURISTIC PROXY"，不是未完成的工作
- 缺少跨厂商模型池：由 D4 排到 v3 的明确决策

---

## 2. 扩展方向

以下方向基于原始文档的五个方向，但我从架构层面重新排序，并增加了我认为优先的一个跨切面方向。

### 🏆 方向 0（新增跨切面）：YAML 解析——用 Go 原生替换 Python shim

这不是原始五个方向之一，但它是方向 1-5 中**大部分基础设施变更的前提条件**，且是现存的已知技术债。

**为什么需要：**
- Python shim 是 forge-core "零外部依赖"承诺的裂缝
- 所有方向（尤其是 ② 语义验证和 ④ 异步人审）会增加 `yaml2json` 管线上的负载
- 容器化/沙箱化场景中 Python 不可用的风险随时间增长

**方案权衡：**

| 方案 | 零依赖保持 | 实现成本 | YAML 1.1/1.2 偏差 | 推荐 |
|---|---|---|---|---|
| A) Go 自研 YAML 极小解析器 | ✅ 是 | 高（2-3 sprint） | 控制中 | 架构主义首选 |
| B) Vendored `gopkg.in/yaml.v3`（Go 模块） | ❌ 否 | 低（1 sprint） | 标准 | 实用主义首选 |
| C) 保持 Python shim，加运行时检查 | ✅ 是 | 最低 | 无变化 | 不推荐（只是延迟决策） |

**我的建议**：方案 B，但有纪律——将 `gopkg.in/yaml.v3` 放到独立子包 `internal/yaml` 中，隔离解析逻辑。如果未来需要回到零依赖，只改这一个包。同时，在 `go.mod` 添加 `require` 是一个重要信号——值得为此写一篇 ADR-0002 修正案。

---

### 方向 ①：预测性成本估算与预算治理（P0 - Sprint A）

**为什么是 P0：** 这是 ROI 最高的方向。当前基础设施（trace、scorecard、HistoryTiebreak）**已存在**，只需增量构建查询层。没有新的 infra，没有新的依赖。

**核心挑战：**

1. **冷启动问题**：新项目无历史数据。解决方案：两层预测——如果有历史评分卡，使用评分卡数据；如果没有，回退到按 model tier 的表格定价 × 该 tier 的典型 token 消耗。输出始终包含置信度区间。

2. **聚合偏差**：不同 task_type 的成本分布高度偏斜（一个 security-review 可能比实现贵 5 倍）。算术均值有误导性。应报告 **P50 / P80 / P95**，而非均值。

3. **跨项目聚合**：单 `project.yml` 中的 budget 字段无法跨仓库工作。方向 ③（fleet）是自然宿主。

**预期架构变更：**

```
┌──────────────────────────────────────┐
│  cmd/forge/cost.go                    │
│  （当前：仅被动消耗跟踪）              │
├──────────────────────────────────────┤
│  新增：                                │
│  - costEstimator（读 scorecards.json）│
│  - forge cost CLI（聚合 trace.jsonl） │
│  - budgetGuard（起跑前检查）          │
└──────────────────────────────────────┘
```

**接口设计**：

```
forge cost [--since 7d] [--by phase|task_type|model] [--summary]
forge evolve --dry-cost   # 起跑前输出估算
```

`project.yml` 新增（可选）：

```yaml
budget:
  monthly_hard_cap_usd: 1000
  monthly_alert_at_usd: 800
  owner: team-alpha
```

**现有系统影响**：零。全部新增代码是只读查询，不改变现有执行路径。

---

### 方向 ⑤：自治运行可观测性与事后调试（P0 - Sprint A，与方向 ① 并行）

**为什么是 P0：** 与成本预测互补，共享同一数据基础设施（`trace.jsonl` + `scorecard`）

**核心挑战：**

1. **JSONL 的时序关联**：跨多次运行的 phase-to-phase 对比需要时间对齐。如果第一次 run 有 5 次迭代而第二次只有 3 次，如何比较？解法：按 `task_type + phase name` 键而非按迭代索引映射。

2. **失败根因的规则引擎**：不应使用 LLM 分析失败（那是自指循环）。应构建将 trace 事件映射到人类可读解释的模式匹配规则引擎。

3. **trace 膨胀管理**：100+ 次迭代的 evolve 运行可以在单个 JSONL 中产生 2000+ 事件。架构需要自动轮转（每 N 事件新文件）+ 索引（`trace.idx` 映射事件偏移 → 行号，用于 O(log N) 查找）。

**预期架构变更：**

```
internal/trace/
├── trace.go          # 现有：事件源
├── reader.go         # 新增：结构化读取（按 run 过滤、时间范围、phase 名）
├── compare.go        # 新增：两次运行的时序对比引擎
├── explain.go        # 新增：失败模式匹配 → 根因文本
└── rotate.go         # 新增：自动轮转 + 索引
```

**接口设计**：

```
forge log --timeline [--run <id>] [--last] [--phase <name>]
forge diff --runs <run-a> <run-b>
forge run --explain   # 在非零退出后分析 trace
forge replay --phase <name> --from-run <trace-id>   # 非确定性重播
```

**现有系统影响**：`trace.go` 需要新轮转钩子（每 5000 事件轮转），以及 `scorecard_wind.go` 的 `windDownScorecards` 需要知道读取最近的 trace 文件。

---

### 方向 ②：语义收敛验证（P1 - Sprint B）

**为什么是 P1：** 高价值但**需要方向 ① 的 CLI 基础设施**来处理验收脚本执行（复用 `CommandExecutor` 的超时 + 输出上限 + 进程组）。依赖是工具性的，不是架构性的。

**核心挑战：**

1. **验收可执行脚本的安全沙箱**：从 ROADMAP.md 执行 `node --test test/auth.test.mjs` 在权限上是侵入性的。架构需要只读文件系统 + 无网络 + 超时切断 —— 并非所有 CLI 框架都能轻易提供。

2. **agent 自我检查的诚实性**：如果 agent 被要求为每个已实现项目生成 `SELF_CHECK:` 命令，它可能选择琐碎的检查（`grep -q login` 而非完整集成测试）。架构解决：将 `self_check` 与 `acceptance` 分开为不同信号，具有不同的权重。

3. **验收脚本难以验证"副作用"**：如果验收标准说"系统应在登录后重定向到仪表板"，这是集成测试覆盖的范围，不是 grep 能验证的。架构应**不扩张语义范围**——验收脚本验证可机械检查的断言。不可机械检查的保留给人审。

**预期架构变更：**

```
internal/converge/
├── converge.go          # 现有：Evaluate → evalOne 分发
├── signals.go           # 新增：AcceptancePass 字段
├── eval_acceptance.go   # 新增：逐条 ROADMAP 验收评估
└── self_check.go        # 新增：SELF_CHECK 解析器

cmd/forge/
├── gates.go             # 现有：加 acceptance gate 消费者
```

**接口设计**：ROADMAP.md 新语法（纯扩展，不修改现有 markdown 结构）：

```markdown
- [x] Add user authentication
  - [accept: "node --test test/auth.test.mjs"]  → 可选机读验收
```

**现有系统影响**：低。`converge.Evaluate` 已经是分发函数，添加新 metric branch 的范式和已有分支（`evalRoadmap`、`evalGateStatus` 等）一致。

---

### 方向 ④：异步协作人审界面（P1 - Sprint B）

**为什么是 P1：** 高杠杆——它是从"单用户 CLI"到"团队平台"最关键的跃迁，但需要方向 ② 的条件批准执行机制来消费批准条件。

**核心挑战：**

1. **`.forge/<stage>/` 目录重塑**：从单一标记文件到可能包含多个 JSON 文件的目录，需要向后兼容。无目录时退回到二进制 approved/not。

2. **异步 `forge review` 模式**：当前 `forge run` 在 human_gate 阻塞，导致进程挂起。架构需要"暂停并持久化等待"语义——写入等待标记，exit 0，另一个进程中的 `forge review` 拾取它。这是 `durable_wait` 的轻量版，纯文件系统。

3. **条件批准的闭环执行**：人类说"批准，但必须添加认证测试"。系统需要在批准后*验证这些条件*。这就是方向 ② 的用武之地——条件变成通过 `converge.Signals` 反向注入的隐式验收标准。

**预期架构变更：**

```
cmd/forge/
├── approve.go        # 现有：扩展 approve/reject/status 子命令
├── review.go         # 新增：异步审查 CLI（读取等待标记，展示上下文）

internal/approval/
├── store.go          # 新增：JSON 审批状态的读写
├── types.go          # 新增：state、condition 的结构体
└── verify.go         # 新增：条件 → 验收标准的执行
```

**现有系统影响**：中等。`humanGate()` 函数需要了解结构化审批状态（而不仅仅是 `isApproved` 布尔值）。`converge.go` 的 `IsHumanGate` 谓词保持向后兼容——无结构化状态时退回到二进制标记。

---

### 方向 ③：多仓库舰队治理（P2 - Sprint C）

**为什么是 P2：** 高价值但**当前条件尚未完全触发**（与原始文件结论一致）。`examples/go-taskd` 和 `url-shortener` 是同一仓库内的种子应用，不是独立的被治理项目。

**核心挑战：**

1. **submodule 路径解析改造是硬核**：ADR-0003 的决策 3 准确地识别出这是最大的单点工作项。`acceptance.mjs` / `arch-check.mjs` / `scan.mjs` / `secret-scan.mjs` 的自身位置锚定 `ROOT = dirname(HARNESS_DIR)` 在 submodule 化后会计算错误的项目根。修复引入 `FORGE_PROJECT_ROOT` 环境变量回退到 `process.cwd()`。

2. **渐进式策略推广**：canary 团队 + 时效的语义增加复杂性。架构需要 `policies.yml` 中的版本戳和每个仓库的"接受的策略版本"。

3. **覆盖与审计**：本地覆盖声明在 `project.yml` 中，可通过 `forge fleet diff` 审计。这是 ADR-0003 提到的，但在架构层面仍然开放：覆盖是否应该可过期？到期后是否应阻止通过闸门？

**预期架构变更**：

```
forge fleet/
├── init.go          # 新增：初始化中央政策仓库
├── sync.go          # 新增：拉取 + 本地覆盖合并
├── diff.go          # 新增：本地 vs 中央政策 diff
├── scorecard.go     # 新增：跨仓库评分聚合
├── audit.go         # 新增：逐仓库合规状态表
└── canary.go        # 新增：阶段性政策推广（含时效）
```

**现有系统影响**：高。ADR-0003 的路径解析改造（决策 3）触及执法热路径，需要在原仓库经过完整的 dogfood 循环后再推广到子仓库。

---

## 3. 接口设计建议

### 3.1 通用原则

**3.1.1 所有新功能入口应该是 CLI first，不是 API first**

ForgeOS 的核心哲学是 CLI 原生的控制平面。方向 1-5 的所有新功能都应设计为通过 forge CLI 子命令暴露，不作为微服务或 webhook 发送。即使是方向 ④ 的异步审查也应驻留在 CLI 调用中（`forge review design`），而不是 Web UI。

**3.1.2 新增信号应遵循 `evalOne` 分发模式**

`converge.Evaluate` → `evalOne` 的分发链是经过验证的可扩展性模式。方向 ② 的 `acceptance_pass` 应作为新分支加入，遵循与 `evalRoadmap` 等相同的签名模式：

```go
func evalAcceptancePass(signals *Signals, data evalData) {
    // 读取验收脚本结果，写入信号
}
```

**3.1.3 trace 事件应遵循"永不破坏消费者"扩展**

现有 `trace.Event` 结构体有固定字段（`DurationMs`、`CostUsdMicros`、`Model`）。新增字段（如 `PhaseTier`、`LoopBackReason`）应可选且从未知消费者零值安全。

**3.1.4 新配置应进入 `project.yml` 的 `overrides` 系列**

现有 `overrides` 段已经承载 `max_file_lines` 和 `max_root_files`。项目级配置的扩展（budget、fleet、approval defaults）应遵循此模式以避免新的配置根键和新的解析函数：

```yaml
overrides:
  budget_monthly_usd: 1000
  approval_expiry_hours: 72
  policy_canary: team-alpha
```

`modes.yml` 仍是全局策略的来源；`overrides` 是项目级局部覆盖。

---

### 3.2 关键接口的变更设计

**3.2.1 `converge.Signals` 的扩展**

当前结构体：

```go
type Signals struct {
    RoadmapCompletion float64
    GatesGreen        bool
    GateProof         []GateResult
    ReviewStatus      string
    HumanApproved     bool
    RequirementConfidence float64
    FileDelta         float64
    Criteria          []Criterion
}
```

扩展后（方向 ②）：

```go
type Signals struct {
    // ... 现有字段不变 ...
    AcceptancePass    bool        // 新增：方向 ②
    SelfChecksPassed  int         // 新增：方向 ② agent 自检计数
    AcceptanceDetail  []AcceptanceResult // 新增：逐项验收结果
}
```

关键设计决策：**不要为每个新信号添加布尔值**。应使用结构体化的结果枚举值（`Pass` / `Fail` / `NA` / `Skipped`）来避免"这个信号是 false，是因为失败了还是还没运行"的歧义。

**3.2.2 `trace.Event` 的扩展**

当前：

```go
type Event struct {
    Kind            string
    Phase           string
    Model           string
    DurationMs      int64
    CostUsdMicros   int64
    // ...
}
```

扩展后（方向 ⑤）：

```go
type Event struct {
    // ... 现有字段不变 ...
    LoopBackCount   int         `json:"loop_back_count,omitempty"`   // 方向⑤：迭代分析
    RejectionReason string      `json:"rejection_reason,omitempty"`  // 方向⑤：失败模式匹配
    CheckpointID    string      `json:"checkpoint_id,omitempty"`     // 方向⑤：可恢复性
    BudgetBeforeUsd int64       `json:"budget_before_usd,omitempty"` // 方向①：预算消耗分析
}
```

`omitempty` 对于向后兼容性至关重要——较旧版本的 ForgeOS 读取较新版本的 trace 文件时不会因缺少字段而崩溃。

**3.2.3 human_gate 的结构化状态模式**

方向 ④ 需要从二进制标记文件转变为目录结构：

```
.forge/
├── design/                          # 设计阶段的审批状态
│   ├── approval.json               # 结构化元数据
│   ├── approval.sig                # GPG/minisign 签名（舰队模式）
│   └── conditions/                  # 批准条件 → 验收标准
│       ├── 001-add-auth-tests.json
│       └── 002-caching-redesign.json
├── review.approved                  # 向后兼容二进制标记
└── review.rejected                  # Sprint 31 的反向标记
```

关键约束：`approval.json` 必须在其模式中包含 `api_version` 字段，以便未来的模式演进不会破坏旧的审批状态。

---

## 4. 技术选型

### 4.1 引入新依赖的门槛

ForgeOS 有**零外部依赖**的现有纪律。任何新增依赖都应满足以下标准：

```
必要条件（全部满足）：
  □ 该功能无法在合理时间内用标准库实现（≥ 2 sprint 的纯自研）
  □ 该依赖本身零传递依赖（不引入依赖树）
  □ 纯 Go 实现（无 CGo，无 cgo 子进程）
  □ 许可证兼容（MIT / BSD / Apache 2.0，非 GPL）
  □ 该功能被外部策略明确要求（例如：企业安全策略说"必须用 TPM 签名"）

理由例外（满足之一即可）：
  □ 安全审计显式要求（例如：FIPS 140-2 验证的加密）
  □ 被 100+ 项目使用且在过去 12 个月内未发现关键 CVE
```

**具体建议**：

| 依赖 | 引入 | 理由 | 判断 |
|---|---|---|---|
| `gopkg.in/yaml.v3` | 替换 Python shim | 不允许零依赖理想阻碍安全运行时架构 | **推荐**，有隔离纪律 |
| `go.opentelemetry.io/otel` | 方向⑤ 结构化可观测性 | 不推荐。当前 `trace.jsonl` 模式已经足够，且 OTel 引入大量传递依赖。**否决** |
| `github.com/google/uuid` | 方向④ 审批 ID | **否决**。`crypto/rand` + 类型包装器就足够了。不值得为了 UUID 格式打破零依赖 |
| `github.com/vektra/mockery/v2` | 测试替代 | **否决**。ForgeOS 的测试结构偏好显式 fake 而非生成 mock。更符合 Go 哲学 |

### 4.2 自研 vs 采购评估

方向 1-5 中，所有扩展都应是**自研**：

| 方向 | 自研/采购 | 理由 |
|---|---|---|
| ① 成本估算 | 自研 | 基础数据已在 `trace.jsonl` / `scorecard` 中；没有适合此领域的现成库 |
| ② 语义验证 | 自研 | 验收脚本执行 = 方向⑤ 的 `CommandExecutor` 的泛化；不需要 cucumber/SpecFlow |
| ③ 舰队治理 | 自研 + git | 中央仓库协议是 git；不需要 Chef/Puppet 之类的配置管理系统 |
| ④ 异步人审 | 自研 | CLI 原生的审批流程没有合适的外部选择。不要将 ForgeOS 的审批耦合到 GitHub 审查 API |
| ⑤ 可观测性 | 自研 | `forge log --timeline` 是专门的；Grafana 的方向不同（仪表板 vs CLI 诊断） |

**一个例外**：如果方向 ③ 中的跨仓库审计需要中央"舰队管理器"端点（不仅仅是 git push/pull 模型），则应考虑在轻量级 SQLite 或 BoltDB 之上构建，而不是引入 Postgres。

### 4.3 推荐的技术决策

**决策 1：YAML 解析——按自己的节奏打破零依赖纪律**

**推荐**：使用 `gopkg.in/yaml.v3` 但有结构性边界。将其封装在 `internal/yaml` 包中，只导出项目所需的函数。写一篇 ADR-0002 修正案，明确记录这就是决定破裂的地方，以及为什么——不是因为"需要库 X"，而是因为"零依赖只是满足可靠性的约束，不是目的本身"。

**不推荐**：构建自研 YAML 解析器（方案 A）。YAML 规范因歧义而声名狼藉——自研解析器将在边界情况中无限期丢失。

**决策 2：方向 ⑤ 的可观测性——JSONL 足够好，坚持使用**

**推荐**：不要引入 structured logging 库或 OTel。`trace.jsonl` 的每行 JSON 模式已经涵盖所有需要的事件。方向 ⑤ 的读端（`trace/reader.go`、`trace/compare.go`）应构建在现有的 JSONL 格式之上。如果稍后需要基于 trace 的指标，将 JSONL 导入 Prometheus 是通过 `json_exporter` 完成的外部集成。

**决策 3：方向 ④ 的异步审查——文件系统够用，不要 Temporal**

**推荐**：`durable_wait`（由 north-star 架构指定）的 MVP 应基于文件系统："暂停并持久化等待" = 写入 `.forge/<stage>/awaiting-review.json`，exit 0。"恢复" = `forge review <stage>` 读取等待标记，展示上下文，收集输入。这种"纯文件系统的两步协议"在性能达到 Temporal 之前就已足够。

---

## 5. 实施路线图

### 5.1 优先级排序

| 方向 | 优先级 | Sprint | 用户价值 | 技术风险 | 核心依赖 |
|---|---|---|---|---|---|
| ① 预测性成本估算 | **P0** | A | 高（省钱透明） | 低 | 已有 telemetry |
| ⑤ 自治运行可观测性 | **P0** | A | 高（信任） | 低 | 已有 trace 框架 |
| ④ 异步人审界面 | **P1** | B | 高（团队协作） | 中 | 方向② 的条件验证（选） |
| ② 语义收敛验证 | **P1** | B | 高（质量保证） | 中 | 方向① CLI 基础设施（选） |
| ③ 多仓库舰队治理 | **P2** | C | 中（规模） | 高 | ADR-0003 待拍板 |
| 0️⃣ YAML 原生解析 | **P1** | A+B | 中（基础设施） | 中 | 打破零依赖纪律决策 |

### 5.2 路线图

**Sprint A（可并行，建议持续 1-2 周真实时间）：**

```
方向①：forge cost CLI（基于 trace.jsonl 的只读查询）
      ├── trace/reader.go（按阶段/模型/任务类型结构化读 JSONL）
      ├── cost.go 扩展（增量聚合 + 冷启动回退）
      ├── forge cost [--since N] [--by phase|model] CLI 胶水
      └── scorecard_wind.go 扩展（成本预测层）
      
方向⑤：forge log --timeline + forge diff --runs
      ├── trace/compare.go（两次运行的时序对比引擎）
      ├── trace/explain.go（基于规则的失败模式匹配）
      ├── forge log --timeline CLI 胶水
      └── forge diff --runs CLI 胶水

方向⑤.1：trace 轮转 + 索引（预防 100 次迭代膨胀）
      ├── trace/rotate.go（每 5000 事件自动轮转）
      └── trace/index.go（事件偏移映射，用于 O(log N) 查找）

架构内务：将 yaml2json.py 的解析错误收集成聚合报告
         而非第一个错误就静默失败（为方向 B 的 yaml 替换做铺垫）
```

**Sprint A 验收标准**：
- `forge cost --since 7d --summary` 输出表格（即使无数据，诚实显示 N/A 或零）
- `forge log --timeline --last` 渲染最近的 trace 为时间线
- `forge diff --runs` 对两个 JSONL 输出结构化对比
- `forge accept: ACCEPTED`（全部现有闸门，新增命令行不变）

**Sprint B：**

```
方向④：forge approve 扩展 + 异步审查
      ├── internal/approval/（结构化审批状态存储）
      ├── approve.go 扩展（--with-conditions、--expires）
      ├── review.go 新增（异步审查 CLI，读取等待标记）
      └── converge.go 扩展（humanGate 理解结构化状态）

方向②：验收脚本执行 + 语义收敛
      ├── converge/eval_acceptance.go（从 ROADMAP 元数据执行验收脚本）
      ├── converge/self_check.go（解析 agent 自检）
      ├── gates.go 扩展（acceptance_pass gate 消费者）
      └── ROADMAP.md 格式扩展（[accept: "…"] 语法，可选）

方向0️⃣：YAML 解析替换（Sprint A 的架构内务交付）
      ├── 决策：选方案 B（gopkg.in/yaml.v3，有隔离纪律）
      ├── internal/yaml/ 封装包
      ├── 删除 harness/yaml2json.py
      ├── 写 ADR-0002 修正案，记录决策理由
      └── 更新 forge-init 复制的 COPIED_FILES（移除 yaml2json.py 的引用）
```

**Sprint B 验收标准**：
- `forge approve design --with-conditions "..." --expires 72h` 写入结构化 JSON
- `forge review design` 展示等待审查及其上下文
- `forge converge --verbose` 显示 `acceptance_pass` 状态
- 验收条件验证在人类批准后可作为收敛信号
- `forge accept` 在没有 Python shim 的情况下通过（原生 YAML 解析）
- `forge accept: ACCEPTED`

**Sprint C（候补，取决于方向③ 的触发条件）：**

```
方向③：舰队治理 MVP（ADR-0003 阶段 A — 本地原型，可逆）
      ├── 路径解析改造（FORGE_PROJECT_ROOT env var）
      ├── forge fleet init <url>（初始化中央策略仓库）
      ├── forge fleet sync（拉取 + 合并覆盖）
      ├── forge fleet diff（本地 vs 中央比较）
      └── forge fleet audit（跨仓库接受状态表）

方向①/⑤ 增强：
      ├── 方向①：project.yml budget 段 → 预算纪律闸门
      ├── 方向⑤：forge replay --phase（历史 prompt + 当前状态重播）
      └── 方向⑤：forge run --explain（非零退出后的根因分析）
```

**Sprint C 验收标准**：
- `forge fleet init` 在本地路径上创建可工作的中央策略仓库
- `forge fleet sync` 跨两个测试仓库复制策略
- `forge fleet audit` 输出表格
- 对方向 ① 和 ⑤ 的新增强可独立验证

### 5.3 里程碑

| 里程碑 | Sprint | 交付物 | 验证 |
|---|---|---|---|
| **M1：可观测闭环** | A | `forge cost` + `forge log --timeline` + `forge diff --runs` | 架构师演示：展示真实 trace 数据、成本趋势、两个运行对比 |
| **M2：人类协作层** | B | `forge approve/reject —with-conditions` + `forge review` + 验收脚本 | 模拟：设计→批准→条件验证→实施→收敛，人类只参与一次 |
| **M3：舰队准备** | C | `forge fleet sync` + `forge fleet audit` | 两个测试仓库从中央策略同步，audit 全部绿色 |

### 5.4 风险与缓解

| 风险 | 严重程度 | 可能性 | 缓解 |
|---|---|---|---|
| `forge cost` 在无历史的新项目上冷启动输出无意义 | 中 | 高 | 必须实现两层回退：有历史 → 基于评分卡预测；无历史 → model 定价表 × 典型 token 消耗。输出始终含置信度区间 |
| `forge log --timeline` 在 5000+ 事件的运行中表现不佳 | 中 | 中 | Sprint A 包含 trace 轮转 + 索引作为前置条件。在 Sprint A 末尾用 10000 事件的合成 trace 压力测试 |
| `forge approve --with-conditions` 的条件无法被强制执行 | 高 | 中 | 条件必须被视为方向② 的验收标准。Sprint B 将条件绑定到验收脚本执行。如果没有脚本可执行，条件变为"声明性备注"（协商，非阻断性） |
| YAML 原生解析替换出现边界情况且无 shim 回退 | 高 | 中-低 | 隔离策略：原有 `yaml2json.py` 在 2 个并发版本中保持可用，通过 `--yaml-backend` 标志切换。2 个 sprint 时间后，当证明了正确性，删除回退 |
| 方向③ 的路径解析改造意外破坏现有单仓行为 | 高 | 中 | `FORGE_PROJECT_ROOT` 默认值为 `process.cwd()` 时，在单仓库设置中完全向后兼容。Dogfood（ForgeOS 自身仓库先消费自己）是强制的验收门禁 |
| 人审子命令在条件批准后没有"提醒人类回来"的方法 | 中 | 低 | Sprint B 不包含 Slack/email 通知——这是 CLI 哲学边界。但 `forge status` 将列出所有等待的审批及其等待时长，以便 operator 可以扫描 |

---

## 总结

### 架构优先级排序（按实战影响）

1. **P0 方向 ① + ⑤**：成本可观测性 + 运行可观测性 —— 共享 trace JSONL 基础，可在 1-2 周内从"能够运行"提升到"可靠运行"，零架构债务
2. **P1 方向 ④ + ②**：人审结构化 + 语义验证 —— 从单兵工具到团队平台的跃迁；方向② 的条件执行反向依赖方向④，因此一起建设是合理的
3. **P1 方向 0（YAML 原生解析）**：清理临时 shim，为所有后续方向奠定安全运行时基础
4. **P2 方向 ③**：舰队治理——ADR-0003 设计就绪，但条件尚未触发，且路径解析改造是高风险的执法路径变更

### 验证报告的元评论

原报告对五个方向的验证总体上准确且彻底（26/26 基线声明经验证正确）。报告中我注意到的最关键细微差别：

- **方向 ③ 的条件确实是"触发条件尚未满足"**（`examples/go-taskd` 和 `url-shortener` 是同一仓库内的种子应用，不是独立的被治理项目）。这是正确的区分。
- **`forge scorecard` CLI 是跨方向未充分利用的资产**（方向 ① 的成本查询和方向 ⑤ 的有序观测都应该是 `forge cost` 和 `forge log --timeline` 而非试图将两者都塞进 `forge scorecard`）
- **目前 trace 中 event kind `"phase"` 不存在**——agent 阶段以 kind `"agent"` 记录，checkpoint 通过 `persist` 包而非 trace 管理。这是 event schema 中的一个小缺口，如果未来需要面向阶段的可观测性，可能需要用单独的 `"phase"` kind 显式标记阶段边界
