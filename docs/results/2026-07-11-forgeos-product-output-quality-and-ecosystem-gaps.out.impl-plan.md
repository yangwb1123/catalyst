现在我已掌握完整的代码库背景。这是我的技术负责人分析：

---

# 技术负责人分析：ForgeOS 五个结构性扩展方向

## 执行摘要

我阅读了分析文档、验证报告，并通读了 `forge-core`、`harness`、`.agent/` 和 `examples/` 中涉及的所有 2000+ 行实现代码。分析文档的五个方向是**明智的**，纠正了三个事实错误后，其核心论点依然成立。以下是我的可执行分解——从方向（分析文档的结构）到任务（具体的代码变更），每个任务 2-4 小时，包括依赖关系和验收标准。

当前代码库状态（`forge-core`，13 个 Go 包，零依赖）意味着这些方向中的每一个都有**现成的数据输入**——scorecards、traces、risk signals、phase emits。缺失的是消费端。这是一件好事：可实现性的杠杆极高。

---

## 1. 任务分解

### TASK-001：risk.FromChangedPaths 内容嗅探（方向三，v1）

轻量级升级：在 `FromChangedPaths` 中从纯路径子串匹配过渡到正则模式 + 文件前 4KB 内容采样，查找敏感函数定义。不涉及完整的 AST——只是在打开的文件中搜索 `import "payment"` 或 `func.*processPayment`。

| 字段 | 值 |
|---|---|
| **ID** | `TASK-001` |
| **标题** | risk.FromChangedPaths 内容感知级别提升 |
| **方向** | 3 — 静态分析驱动的风险提取（P0） |
| **涉及文件** | `forge-core/internal/risk/risk_diff.go`（修改），`forge-core/internal/risk/risk_diff_test.go`（新增/修改），`harness/acceptance-quality.mjs`（更新诚实报告） |
| **前置依赖** | 无 |
| **预估工时** | 3 小时 |
| **验收标准** | ① 对不包含路径关键词但包含 `import "payment"` 的文件中的变更，能正确设置 `TouchesPayment=true`。② 纯路径变更（如仅修改注释）保持 `TouchesPayment=false`。③ 所有现有 `risk_diff_test` 通过。④ 从分析文档验证角度：原始的错误 3（javascript:alert(1) URL 验证删除）仍然由 url-shortener 模块覆盖，而非 risk 模块。 |

### TASK-002：复杂性维度信号实现（方向三，v2）

为圈复杂度给 `routing.Score()` 的 `complexity` 维度提供真实信号。通过 harness adapter 框架接入（`gocyclo`/`lizard`/`radon`），为已更改文件生成复杂度读数。

| 字段 | 值 |
|---|---|
| **ID** | `TASK-002` |
| **标题** | 复杂性维度：圈复杂度信号生产者 |
| **方向** | 3 — 静态分析驱动的风险提取（P0） |
| **涉及文件** | `forge-core/internal/routing/score_signals.go`（新增），`harness/adapters/go.yml`（新增复杂度工具注册），`harness/adapters/typescript.yml`（新增），`harness/adapters/python.yml`（新增） |
| **前置依赖** | `TASK-001`（相同的 risk/routing 内部模式，非阻塞） |
| **预估工时** | 4 小时 |
| **验收标准** | ① `forge route --diff-files` 输出包含 `complexity` 维度得分，不再是硬编码的 0.5。② `go.yml` 适配器在 `gocyclo` 可用时运行它，缺失时诚实报告 N/A（参见 lint/coverage 模式）。③ 复杂性得分归一化至 0..1 范围。④ 警告：`gocyclo` 需要可用二进制；在 GitHub Actions 上需安装 `go install golang.org/x/tools/cmd/gocyclo@latest`。 |

### TASK-003：合约 schema 定义与验证器（方向五，v1）

为 `emits:` 产物添加轻量级合约：在 agent 卡或 workflow 阶段中提供可选的 `schema:` 键，引用 YAML/JSON 断言。实现 `forge validate --contracts` 子命令检查合约是否存在并解析。

| 字段 | 值 |
|---|---|
| **ID** | `TASK-003` |
| **标题** | Agent 产出合约 schema 定义与 CLI 验证器 |
| **方向** | 5 — Agent 产出合约验证（P0） |
| **涉及文件** | `forge-core/internal/contract/schema.go`（新增核心合约类型与解析器），`forge-core/internal/contract/validator.go`（新增断言检查器），`forge-core/cmd/forge/validate.go`（修改——添加 `--contracts` 标志），`.agent/workflows/evolve.yml`（修改——在 gap-analysis 阶段添加 `schema:` 引用），`docs/contracts/gap-report.schema.yml`（新增示例合约） |
| **前置依赖** | 无 |
| **预估工时** | 4 小时 |
| **验收标准** | ① `forge validate --contracts` 验证所有 workflow `emits:` 产物是否有对应的 `schema:`，无 schema 的产物警告但不阻断。② 契约解析器在启动时能存活解析格式错误的 schema。③ 示例 gap-report 合约检查 gap-report.md 是否包含 `top_gaps`、`priority` 字段。④ 所有现有 validate 测试通过。 |

### TASK-004：运行时合约检查 gate（方向五，v2）

在 phase 执行后、进入下一 phase 前，插入一个可选的 `contract_check` gate。验证 agent 输出是否符合其在阶段 `emits:` 中声明的合约。失败会触发 trace 事件 + 减慢收敛（不硬阻断）。

| 字段 | 值 |
|---|---|
| **ID** | `TASK-004` |
| **标题** | 运行时合约检查 gate |
| **方向** | 5 — Agent 产出合约验证（P0） |
| **涉及文件** | `forge-core/internal/orchestrator/contract_check.go`（新增 gate 执行器），`forge-core/cmd/forge/gates.go`（修改——注册 `contract_check` gate），`forge-core/internal/asset/asset.go`（修改——在 Phase 中添加 `ContractCheck` 字段），`forge-core/internal/trace/trace.go`（修改——添加 `EventKindContractFail`） |
| **前置依赖** | `TASK-003`（合约 schema） |
| **预估工时** | 4 小时 |
| **验收标准** | ① 合约验证失败生成 `EventKindContractFail` 追踪事件。② 合约失败不会导致运行中止——会记录警告并继续，但会降低收敛收敛进度（参见 `converge.go` 中的 `no_progress` 逻辑）。③ dry-run 报告合约检查结果。④ 入口：如果 `schema:` 缺失，则跳过 gate（向后兼容）。 |

### TASK-005：将 ad-hoc VERDICT 解析迁移到合约（方向五，v2.5）

将 `cost.go:parseReviewerVerdict`、`parseExecutiveVerdict`、`parseConfidenceScore` 从脆弱的 `strings.Contains` 迁移到合约驱动的验证。Verdict 合约（`review-verdict.schema.yml`）定义确切的格式；解析器使用它，静默降级时显示告警。

| 字段 | 值 |
|---|---|
| **ID** | `TASK-005` |
| **标题** | 将 VERDICT 解析从 ad-hoc 字符串匹配迁移到合约 |
| **方向** | 5 — Agent 产出合约验证（P0） |
| **涉及文件** | `forge-core/cmd/forge/cost.go`（修改——替换 `parseReviewerVerdict`、`parseExecutiveVerdict`、`parseConfidenceScore`），`forge-core/cmd/forge/cost_test.go`（修改——更新测试以使用合约），`docs/contracts/review-verdict.schema.yml`（新增） |
| **前置依赖** | `TASK-004`（合约验证框架） |
| **预估工时** | 2 小时 |
| **验收标准** | ① 新解析器正确提取 `VERDICT: APPROVE`（内部代码块、额外空格附近）。② 严格模式下的格式错误的输出会记录告警追踪事件。③ old-behavior fallback：格式错误的输出返回空字符串（同前），因此现有下游调用者保持零行为变化。④ 删除分析文档中记录的所有三个事实错误。 |

### TASK-006：路由阈值可观测性（方向一，v1）

向 `forge scorecard rebuild` 添加 `--calibrate` 标志，输出从 scorecard 数据派生的建议路由阈值。此标志不会更改任何代码——它只报告「你的 scorecard 数据显示 HaikuMax 应为 0.42，而非 0.34」。

| 字段 | 值 |
|---|---|
| **ID** | `TASK-006` |
| **标题** | 路由阈值校准报告（只读） |
| **方向** | 1 — 路由阈值自校准（P1） |
| **涉及文件** | `forge-core/cmd/forge/scorecard_wind.go`（修改——添加 `--calibrate` 标志），`forge-core/internal/routing/routing.go`（修改——导出阈值常量以用于报告），`forge-core/internal/routing/calibration.go`（新增） |
| **前置依赖** | 无 |
| **预估工时** | 3 小时 |
| **验收标准** | ① `forge scorecard rebuild --calibrate` 输出类似 `suggested HaikuMax = 0.42 (current = 0.34, based on N=120 samples)` 的建议。② 建议基于第 X 百分位数（可配置，默认 p85）的 quality_score × latency 权衡。③ 零个文件被修改——纯报告。④ 冷启动（零 scorecard 数据）输出 `insufficient data`。 |

### TASK-007：自动阈值更新（方向一，v2）

引入 `CalibratedThresholds` 类型，在配置的 `min_samples` 满足后，自动调整 `BandForScore` 和 `TierForScore` 的分段边界。绑定到 scorecard 更新事件。

| 字段 | 值 |
|---|---|
| **ID** | `TASK-007` |
| **标题** | 自动路由阈值校准 |
| **方向** | 1 — 路由阈值自校准（P1） |
| **涉及文件** | `forge-core/internal/routing/routing.go`（修改——`HaikuMax`/`SonnetMax` 从 const 变为 var），`forge-core/internal/routing/calibration.go`（修改——主动更新逻辑），`forge-core/internal/routing/calibration_test.go`（新增） |
| **前置依赖** | `TASK-006`（阈值报告模式） |
| **预估工时** | 3 小时 |
| **验收标准** | ① 在 scorecard 重建时，收集到超过 N（默认 50）个样本后，阈值更新。② drift guard：如果新阈值与旧阈值偏离超过 0.15，则拒绝更新并记录警告。③ 所有现有路由测试通过。④ 所有更改在重新构建后必须可逆转（通过 `--reset-thresholds` 标志）。 |

### TASK-008：预测运行估算 v1（方向二，v1）

`forge run --dry-run --predict` 输出基于历史的预测：预计成本（USD 美元）、预计持续时间和预计迭代次数。从 `.forge/trace.jsonl` 和 `.agent/routing/scorecards.json` 聚合历史数据。

| 字段 | 值 |
|---|---|
| **ID** | `TASK-008` |
| **标题** | 预测运行估算——CLI 预测报告 |
| **方向** | 2 — 预测性运行估算（P1） |
| **涉及文件** | `forge-core/internal/orchestrator/predict.go`（新增），`forge-core/internal/orchestrator/predict_test.go`（新增），`forge-core/cmd/forge/run_budget.go`（修改——添加 `--predict` 标志） |
| **前置依赖** | 无 |
| **预估工时** | 4 小时 |
| **验收标准** | ① 对一个至少有 5 条记录的 trace 文件，`forge run build --dry-run --predict` 输出均值/中位数/p90 的成本和持续时间。② 离群值过滤：超过 3σ 的数据点被标记并从预测均值中排除。③ 零历史输出 `insufficient history (need ≥5 runs, got 0)`。④ 按 `(mode, workflow)` 分桶。 |

### TASK-009：预测注入运行预算（方向二，v2）

将预测整合到运行时预算守卫中。`checkRunBudget` 在成本超过预测均值的 2 倍时发出咨询警告，超过 3 倍时硬阻断。

| 字段 | 值 |
|---|---|
| **ID** | `TASK-009` |
| **标题** | 预算守卫预测异常检测 |
| **方向** | 2 — 预测性运行估算（P1） |
| **涉及文件** | `forge-core/internal/orchestrator/budget.go`（修改——添加预测交叉检查），`forge-core/internal/orchestrator/predict.go`（修改——导出 `PredictCost`、`PredictDuration`） |
| **前置依赖** | `TASK-008`（预测引擎） |
| **预估工时** | 2 小时 |
| **验收标准** | ① 当 `cumulative_cost > 2 × predicted_cost` 时，生成警告日志，运行继续。② 当 `cumulative_cost > 3 × predicted_cost` 时，运行中止并显示 `cost_drift_exceeded`。③ 预测异常检测器在零历史数据时可存活跳过（无中断）。 |

### TASK-010：跨运行失效分类 v1（方向四，v1）

`forge trace --summary` 子命令：读取 `.forge/trace.jsonl`，按 `(Kind, Status)` 聚合计数，按 `(model, Status)` 生成失败率排名。纯本地，零依赖。

| 字段 | 值 |
|---|---|
| **ID** | `TASK-010` |
| **标题** | 跨运行失效摘要 CLI |
| **方向** | 4 — 跨运行失效模式分类（P2） |
| **涉及文件** | `forge-core/cmd/forge/trace_cmd.go`（新增），`forge-core/internal/trace/summary.go`（新增），`forge-core/internal/trace/summary_test.go`（新增） |
| **前置依赖** | 无 |
| **预估工时** | 3 小时 |
| **验收标准** | ① `forge trace --summary` 输出一个表格，包含行：`review:gate_test FAIL: 12 occurrences (23.5%)`。② 如果一周内失败率超过 3σ 阈值，则高亮显示。③ 输出是纯文本，适合 CI 日志。④ 空 trace 文件输出 `no trace data` 并退出 0。 |

### TASK-011：Trace 归档与旋转（方向四，v2）

按文件大小（默认 10MB）或时间（默认 7 天）添加 trace 文件旋转。为归档查询添加子命令。

| 字段 | 值 |
|---|---|
| **ID** | `TASK-011` |
| **标题** | Trace 归档、旋转与查询 |
| **方向** | 4 — 跨运行失效模式分类（P2） |
| **涉及文件** | `forge-core/internal/trace/rotate.go`（新增），`forge-core/cmd/forge/trace_cmd.go`（修改——添加 `archive query`），`.agent/project.yml`（修改——添加 `trace.max_size_mb`、`trace.max_age_days` 选项） |
| **前置依赖** | `TASK-010`（trace 基础设施） |
| **预估工时** | 4 小时 |
| **验收标准** | ① 当 `.forge/trace.jsonl.gz` 超过配置大小或年龄时，自动旋转至 `archive/`。② `forge trace archive query --since=7d` 按时间段和状态过滤输出。③ 零行为变化： `--summary` 仍读取活动 trace + 可选的归档。④ 直接 gzip 操作——稳健且可预测。 |

### TASK-012：修复 examples/go-taskd/main.go（分析文档方向 ①）

对 `main.go` 的 20 行更改：添加 `signal.NotifyContext`、`http.Server.Shutdown` 和 `GET /health` 端点。这是验证报告同意的最高性价比改动。

| 字段 | 值 |
|---|---|
| **ID** | `TASK-012` |
| **标题** | go-taskd main.go 生产就绪性修复 |
| **方向** | 1 — 工厂产出质量（来自分析文档方向 ①） |
| **涉及文件** | `examples/go-taskd/main.go`（修改——添加优雅关闭） |
| **前置依赖** | 无 |
| **预估工时** | 1 小时 |
| **验收标准** | ① `SIGTERM`/`SIGINT` 触发 `http.Server.Shutdown`（5 秒超时）。② `GET /health` 返回 `200 {"status":"ok"}`。③ 所有 `go-taskd` 测试（4 个测试文件）通过。④ 分析文档中记录的 `log.Fatalf` 错误签名被处理，但 `log.Fatalf` 保留仅用于非关闭错误（FATAL 是 composition root 中正确的退出方式）。 |

---

## 2. 执行顺序

```
graph TD
    
    subgraph 阶段1-P0层[阶段 1：P0 层 — 核心安全与完整性]
        T001[TASK-001: risk 内容嗅探]
        T003[TASK-003: 合约 schema + 验证器]
        T012[TASK-012: go-taskd main.go 修复]
    end

    subgraph 阶段2-P0层增强[阶段 2：P0 层增强 + P1 起点]
        T002[TASK-002: 复杂性维度信号]
        T004[TASK-004: 运行时合约检查 gate]
        T005[TASK-005: VERDICT 解析迁移]
        T006[TASK-006: 阈值校准报告]
        T008[TASK-008: 预测运行估算 v1]
        T010[TASK-010: 跨运行失效摘要]
    end

    subgraph 阶段3-P1自动化[阶段 3：P1 自动循环]
        T007[TASK-007: 自动阈值更新]
        T009[TASK-009: 预算守卫异常检测]
        T011[TASK-011: Trace 归档与旋转]
    end

    T001 --> T002
    T003 --> T004
    T004 --> T005
    T006 --> T007
    T008 --> T009
    T010 --> T011

    T001 -.-> T005
    T003 -.-> T005
```

**可并行组**：

| 并行组 | 任务 | 共享依赖 | 风险评估 |
|---|---|---|---|
| **A**（P0 核心） | T001、T003、T012 | 无 | 低：全部是独立的增强，没有共享的代码路径 |
| **B**（P0 增强） | T002、T004、T005、T006、T008、T010 | T001→T002，T003→T004→T005 | 低至中：T004 依赖 T003，但 T001/T006/T008/T010 完全独立 |
| **C**（P1 循环） | T007、T009、T011 | T006→T007，T008→T009，T010→T011 | 低：每对严格顺序，但组间独立 |

---

## 3. 技术风险

### 3.1 高风险

| 风险 | 任务 | 缓解措施 |
|---|---|---|
| **TASK-001 内容嗅探的误报问题**：在文件中搜索 `func.*[Pp]ayment` 可能会匹配注释或日志字符串，导致 `TouchesPayment=true` 误报。 | T001 | 使用来自 `arch-check` 的 `extractJsImports`-style 解析器模式：仅检查实际导入语句，而非全文 grep。无 AST 要求——只需对 Go 导入、TypeScript 导入、Python 导入进行正则模式匹配。为路由风险分类器输出 `reason` 字段，因此用户如果不同意，可以通过 `--touches-payment=false` 进行覆盖。 |
| **TASK-004 合约检查对 workflow 收敛的意外影响**：如果合约过于严格，正常运行可能因格式的细微偏差而失败，这些错误本可以由 agent 自我修复。 | T004 | v1 必须**不阻断运行**——它记录、减慢收敛、但永不硬阻停，除非配置为 `blocking: true`。遵循 `workflow_depth`（explorer = 仅警告，engineering = 可能阻塞）。此外，从 sprint 5 的教训中吸取经验：**首次在 agent 上失败，然后自动在提示词中加强指令**，而不是强迫人类进行修复。 |
| **TASK-005 VERDICT 解析迁移可能引入回归**：当前的 ad-hoc 解析器虽然有缺陷，但包含 2 年的隐性边缘情况。基于合约的替换可能会破坏尚未测试过的格式。 | T005 | 保留旧解析器作为备选。合约解析器先运行；如果合约不存在或返回空，则降级到旧解析器并记录 `COMPAT_FALLBACK` 警告。在迁移周期（2 个 sprint）后移除旧路径。参考验证报告中对 url-shortener URL 验证的误判，确保先测试前移。 |

### 3.2 中等风险

| 风险 | 任务 | 缓解措施 |
|---|---|---|
| **TASK-002 通过适配器进行圈复杂度**：`gocyclo` 输出格式在版本之间可能会变化，或者可能不在 PATH 上（尤其是在 CI 中）。 | T002 | 遵循现有的 harness adapter 模式（`probeLint`、`probeAppTests`）：探测工具是否存在，存在则运行检查，缺失则 N/A。不通过未经检查的 `gocyclo` 输出破坏 CI。遵循 sprint 19 的 `sca.mjs` 模式——semver 匹配引擎有在缺失数据时降级的先例。 |
| **TASK-007 自动阈值校准产生振荡**：如果 scorecard 数据有噪声，阈值可能在每次重建时都会发生变化，导致路由不稳定。 | T007 | 漂移保护（如果 |new - old| > 0.15 则拒绝）+ 指数衰减（样本权重在 30 天后减半，遵循 `policy.yml` 的 `recency_half_life_days: 30`）。从 sprint 11 的 `decayWeight` 实施中学习。 |
| **TASK-008 冷启动预测**：首次运行的仓库没有历史数据。 | T008 | 回退到 `forge-core` 自身运行数据聚合的于 `(mode, lifecycle)` 的经验基线。从 S25-S26 文档化的 trace 数据中播种基线成本：`avg_cost_usd=0.1841`。 |

### 3.3 低风险

| 风险 | 任务 | 缓解措施 |
|---|---|---|
| **TASK-010 大 trace 文件**：长运行 trace 文件可能包含数十万行，导致读取整个文件变慢。 | T010 | v1 流式读取（逐行迭代器，无需将整个文件加载到内存中）。对于归档，使用 `gzip.Reader` 流式解压缩。 |
| **TASK-012 1 小时完成**：这非常简单——只有 20 行。零风险。 | T012 | 直接实现。遵循 `net/http` 的 `Server.Shutdown` 模式，参见 Go 文档。 |

---

## 4. 资源评估

### 4.1 人员规模与技能要求

| 角色 | 必要性 | 任务 | 评估 |
|---|---|---|---|
| **Go 工程师（初级至中级）** | **1 人** | T001、T007、T008、T009、T010、T011、T012 | 需要熟悉 `forge-core` 内部结构（`routing`、`trace`、`orchestrator`、`risk` 包）。纯标准库，零外部 Go 模块。 |
| **Go 工程师（高级）** | **1 人** | T002、T003、T004、T005、T006 | 需要理解 `harness adapter` 框架（shell 出+解析输出）+ `cost.go` 的脆弱的 verdict 解析。T004 需要深入理解 `orchestrator` 状态机。 |
| **审查员** | **每任务 1 人** | 全部 | 必须是 fresh-context 审查员（参考 `AGENTS.md` 门控）：每个任务由不同的工程师审查，而不是由实施者。对于 forge-core 更改，经验丰富的 Go 工程师最佳。 |

**总人员**：2 名开发人员 + 1 名每任务审查员（轮换）。考虑到节奏，**约 3-4 名工程师**可以舒适地并行覆盖。

### 4.2 关键里程碑

| 里程碑 | 时间线 | 交付物 |
|---|---|---|
| **M1：P0 安全层就绪** | 第 1-2 天 | T001 交付（内容感知风险）+ T003 交付（合约验证器）+ T012 交付（go-taskd 关闭） |
| **M2：P0 合约检查在运行中激活** | 第 3-5 天 | T004 交付（合约 gate 执行）+ T005 交付（VERDICT 迁移）+ T002 交付（复杂性信号） |
| **M3：P1 可观测性就绪** | 第 6-8 天 | T006 交付（阈值报告）+ T008 交付（预测估算）+ T010 交付（trace 摘要） |
| **M4：P1 自动循环激活** | 第 9-11 天 | T007 交付（阈值自动调整）+ T009 交付（预算异常检测）+ T011 交付（trace 归档） |
| **M5：治理整合** | 第 12-14 天 | 全任务 `forge accept` ACCEPTED、5 个方向全部功能绿、审查周期完成 |

### 4.3 阻塞点与解决策略

| 阻塞点 | 影响 | 解决策略 |
|---|---|---|
| **T002 需要 CI 中的 `gocyclo`** | 如果没有 `gocyclo` 二进制，复杂性维度信号在 CI 中保持 N/A | 在 `forge-core` 中实现轻量级圈复杂度计算器——纯 Go，零依赖，仅计算函数复杂度。20 行 AST 遍历。从现有的 `arch-check` AST 启发式中汲取灵感。 |
| **T006 缺少校准用的足够 scorecard 样本** | 阈值校准不可操作 | v1 播种合成基线数据——从 20 个模拟运行生成，均匀分布。如果校准没有真实历史，则无法推进。更好：用合成数据播种后立即进入生产环境。 |
| **T004 合约检查在 adapter test 修复前干扰 agent** | agent 输出不断失败合约检查，减慢了 dogfood | sprint 13 模式：`on_fail` 循环回 implement 阶段，而非 abort。合约失败触发定向 loop_back + 提示词加强。每次合约失败后，agent 会得到一条新指令：「你之前的输出缺少必要字段 X」。 |

---

## 5. 质量保证

### 5.1 单元测试覆盖要求

| 任务 | 所需新测试 | 覆盖目标 | 关键边界情况 |
|---|---|---|---|
| T001 | `risk_diff_test.go` 中的 8+ 个函数 | 输入解析的 95%+ | `import "payment"` vs `// payment not implemented`（注释 vs 代码）；Go、TypeScript、Python 导入语法；空文件；4KB 截断边界 |
| T002 | `score_signals_test.go` 中的 10+ 个函数 | 复杂性信号 90%+ | 空 diff、单文件、多文件、零复杂度、高复杂度（`gocyclo` 30+）、语言切换 |
| T003 | `schema_test.go` 中的 6+ 个函数 | 合约解析 95%+ | 格式错误的 YAML、缺失 `schema:` 字段、未知断言类型、循环引用（通过深度限制防止） |
| T004 | `contract_check_test.go` 中的 10+ 个函数 | gate 逻辑 90%+ | 合约通过、合约失败、阶段无合约（跳过）、紧接合约失败后循环回退、提示词加强检查 |
| T005 | `cost_test.go` 中的 6+ 个更新 | 回归覆盖 100% | 代码块内的 VERDICT、额外空格、缺失 VERDICT、双重 VERDICT（取最后一个）、大小写错误 |
| T006 | `calibration_test.go` 中的 6+ 个函数 | 校准计算 95%+ | 零样本、1 个样本、< min_samples、边缘百分位、scorecard 数据中的离群值 |
| T007 | `calibration_test.go` 中的 10+ 个函数 | 自主决策 95%+ | 节流（每次重建仅 0.15 步长）、对噪声的稳定性、可逆性（重置标志）、向后兼容 |
| T008 | `predict_test.go` 中的 10+ 个函数 | 预测引擎 90%+ | 空 trace、仅 1 个 trace、离群值过滤（3σ）、按 mode 的分桶、按 workflow 的分桶 |
| T009 | `budget_test.go` 中的 4+ 个函数 | 预测交叉检查 90%+ | 历史不足（跳过）、<2× 预测（通过）、>3× 预测（阻断）、稳定性测试 |
| T010 | `summary_test.go` 中的 8+ 个函数 | 聚合 95%+ | 空 trace、单事件、多事件、大量事件（10K+）、失败率排名、阈值高亮 |
| T011 | `rotate_test.go` 中的 6+ 个函数 | 旋转 90%+ | 大小旋转、年龄旋转、无旋转条件、归档查询、跨归档的边界情况 |
| T012 | 现有测试 | 100% 现有 | 信号中断、健康检查返回 200、超时发生 |

**所有任务**：对于 `forge-core` 更改，所有测试**必须**通过 `-race -count=5`（遵循 S22/S25 建立的模式）。对于 `harness` 更改，使用 `node --test` 测试，并且 `-count=5` 进行稳定性测试。

### 5.2 集成测试策略

| 集成测试 | 覆盖的任务 | 方法 |
|---|---|---|
| `forge route --diff-files` 集成 | T001、T002 | 在 `forge-core/cmd/forge/route_test.go` 中：创建模拟文件 diff（包含 `payment/handler.go` 的临时目录）→ 运行 `forge route --diff-files --from-git` → 签名输出现在包含 `complexity: 0.72` 和 `risk: critical`。 |
| 合约生命周期 | T003、T004、T005 | 从 `evolve.yml` 测试 `gap-analysis` 阶段：运行 `forge validate --contracts` → 报告合约就绪。运行 `forge run gap-analysis --executor dry` → 报告包含合约验证检查。 |
| 预测与预算 | T008、T009 | 用已知的 `trace.jsonl`（包含 10 个历史运行）播种临时 `.forge/` → 运行 `forge run build --dry-run --predict` → 断言输出匹配预期的预测。然后运行 `forge run build --max-budget-usd 0.01`（预算极低）→ 验证预算异常检测器触发。 |
| 端到端带 trace | T010、T011 | 生成跨越 2 个会话的 trace 文件 → 运行 `forge trace --summary` → 聚合正确。手动旋转归档 → `forge trace archive query --since=30d` → 报告正确。 |

### 5.3 代码审查要点

每个拉取请求的审查清单：

| 关注领域 | 要检查的内容 |
|---|---|
| **合规性** | `forge accept` 在本分支上通过。新代码的测试覆盖达到 ≥90% 是真的，而不仅仅是统计。 |
| **诚实性** | 没有任务声称它没有做的事情（Sprint 19 的教训：诚实性 > 功能）。审查员应检查 `N/A` 路径是否明确且可读，而不是静默回退。 |
| **单一职责** | forge-core 中的文件 ≤500 行，函数 ≤50 行。如果接近，则拆分（遵循 S23 的 acceptance.mjs 拆分）。 |
| **零外部依赖** | forge-core Go 包的 `go.mod` 中零新依赖。仅标准库。任何新的 harness Node 模块必须通过 `require()` 调用（无 npm install）。 |
| **架构分层** | `internal/` 包遵循 Clean Architecture：域层不导入编排层，编排层不导入 cmd/ 包。 |
| **冷静公开** | 审查员自己做。在子任务上独立工作（Sprint 29 的教训）。 |

### 5.4 性能测试需求

| 任务 | 测试 | 基准 | 通过标准 |
|---|---|---|---|
| T001（内容嗅探） | 嗅探 1000 个文件的微基准 | 每个文件 < 1ms | 线性扩展 |
| T004（合约检查） | 检查包含 100 个阶段的 workflow 的完整运行 | < 50ms 开销 | 在典型的运行时间（数秒）中可忽略不计 |
| T008（预测） | 在包含 10K 条记录的 trace 文件上运行预测 | < 100ms | 在 CLI 上下文中可接受 |
| T010（trace 摘要） | 在包含 100K 条记录的文件上运行摘要 | < 200ms | 流式读取——内存恒定，可以处理更大的文件 |

---

## 6. 实施计划

### 6.1 甘特图

```
gantt
    title ForgeOS — 五个结构性扩展方向实施计划
    dateFormat  YYYY-MM-DD
    axisFormat  %b %d

    section 阶段1 — P0 层（第 1-2 天）
    TASK-001 risk 内容嗅探           :a1, 2026-07-13, 1d
    TASK-003 合约 schema + 验证器     :a2, 2026-07-13, 1d
    TASK-012 go-taskd main.go 修复    :a3, 2026-07-13, 0.5d
    阶段1 审查时间                   :a4, after a3, 0.5d

    section 阶段2 — P0 增强 + P1 起始（第 3-7 天）
    TASK-002 复杂性维度信号           :b1, after a1, 1d
    TASK-004 合约检查 gate           :b2, after a2, 1.5d
    TASK-005 VERDICT 解析迁移        :b3, after b2, 1d
    TASK-006 阈值校准报告            :b4, 2026-07-15, 1d
    TASK-008 预测运行估算 v1        :b5, 2026-07-15, 2d
    TASK-010 跨运行失效摘要         :b6, 2026-07-16, 1d
    阶段2 审查时间                   :b7, after b6, 1d

    section 阶段3 — P1 自动循环（第 8-11 天）
    TASK-007 自动阈值更新            :c1, after b4, 1.5d
    TASK-009 预算守卫异常检测        :c2, after b5, 1d
    TASK-011 Trace 归档与旋转       :c3, after b6, 1.5d
    阶段3 审查时间                   :c4, after c3, 1d

    section 阶段4 — 整合与发布（第 12-14 天）
    全系统集成测试                   :d1, 2026-07-24, 2d
    docs/ignition.md 更新            :d2, 2026-07-24, 0.5d
    最终 forge accept 全绿         :d3, 2026-07-25, 0.5d
```

### 6.2 阶段交付物

#### 阶段 1：P0 核心安全与完整性（第 1-2 天）

| 日期 | 可交付物 | 验证 |
|---|---|---|
| 第 1 天上午 | `TASK-001`：`risk.FromChangedPaths` 对 Go/TS/Python 导入具有内容感知能力 | `forge route --diff-files --from-git` 针对包含 payment 导入的模拟目录 |
| 第 1 天下午 | `TASK-003`：合约 schema 解析 + `forge validate --contracts` | 针对 `evolve.yml` 运行 `forge validate`，显示 gap-analysis 的合约状态 |
| 第 2 天上午 | `TASK-012`：go-taskd 优雅关闭 + `/health` | `kill -TERM` → 5 秒内正常关闭；`curl /health` → `200` |

**退出标准**：`forge accept` 通过。T001 + T003 + T012 合并。每个都有 fresh-context 审查批准。

#### 阶段 2：P0 增强 + P1 起点（第 3-7 天）

| 日期 | 可交付物 | 验证 |
|---|---|---|
| 第 3 天 | `TASK-002`：圈复杂度适配器集成 | `forge route --diff-files` 输出包含非 0.5 的 `complexity` 分数 |
| 第 4-5 天 | `TASK-004` + `TASK-005`：合约 gate 执行 + VERDICT 迁移 | agent 输出缺少必要字段 → gate 记录 + 循环回退（不阻断）。格式错误的 VERDICT → 警告 + 旧解析回落 |
| 第 5 天 | `TASK-006`：`forge scorecard rebuild --calibrate` 报告 | 对 ~20 个样本运行，硬件水平输出汇总统计 |
| 第 6-7 天 | `TASK-008`：`forge run --dry-run --predict` | 播种 10 个 trace 记录；运行命令；输出显示 `predicted_cost_usd` 和 `predicted_duration_ms` |
| 第 6 天 | `TASK-010`：`forge trace --summary` | 播种 20 个 trace 事件；运行命令；输出显示失败排名 |

**退出标准**：所有 6 个任务合并。每个都带来价值（不仅仅是基础设施）。`forge accept` 通过。

#### 阶段 3：P1 自动循环（第 8-11 天）

| 日期 | 可交付物 | 验证 |
|---|---|---|
| 第 8-9 天 | `TASK-007`：自动阈值更新 | 驱动 50 个模拟 scorecard 更新 → 观察阈值调整（受漂移防护限制）。运行 `forge route --reset-thresholds` → 恢复出厂设置 |
| 第 9 天 | `TASK-009`：预算异常检测 | 设置 `--max-budget-usd 0.01` → 运行预测检查。如果 >3× 预测，则阻断并显示 `cost_drift_exceeded` |
| 第 10-11 天 | `TASK-011`：trace 归档 | 生成 11MB trace 文件 → 自动旋转。查询归档 trace：按日期和状态过滤 |

**退出标准**：所有 3 个任务合并。通过集成测试。审查周期完成。

#### 阶段 4：整合与发布（第 12-14 天）

| 日期 | 可交付物 | 验证 |
|---|---|---|
| 第 12-13 天 | 端到端集成测试 | 带有真 `--agent-cmd=claude`（可选）的完整 `forge evolve` 循环，包含所有 5 个方向 |
| 第 13 天 | 文档更新 | `docs/ignition.md`、`docs/contracts/`、`ROADMAP.md` 更新 |
| 第 14 天 | `forge accept` 通过 | 全绿——整个仓库处于 `block` 模式 |

---

## 7. 总结建议

### 执行顺序（按节奏）

1. **第 1-2 天**：T001 + T003 + T012（P0 修复 + go-taskd“快速取胜”）。这三者各自独立，交付后可合并。每个审查需要 2-3 小时。

2. **第 3-7 天**：T002 + T004 + T005（P0 完成）+ T006 + T008 + T010（P1 可观测性）。可并行实施（T002 独立，T004-T005 在前端验证阶段，T006/T008/T010 完全独立）。**关键依赖**：T004 必须等待 T003；其余可并行推进。

3. **第 8-11 天**：T007 + T009 + T011（P1 自动化）。全部线性依赖前面的任务。T007 具有最高的技术 ROI——自动校准是路由效率的**倍增器**。

4. **第 12-14 天**：集成测试、文档、发布。

### 需要立即作出的决定

1. **T001 的范围**：是否应该包含 AST（完整的文件解析），还是仅包含正则/关键词内容嗅探？我建议仅采用正则嗅探——它成本低，匹配 80% 的价值，且不会将重型 AST 解析引入风险路径。如果发现误报率 > 20%，可以在阶段 2 中升级。

2. **T003 合约格式**：使用现有的 YAML 还是自定义 DSL？坚持使用 YAML——它已经在整个 `.agent/` 中使用，现有的 `yaml2json.py` shim 可以处理它，并且操作者不需要学习新的 schema 语言。合约本身应该是简单的断言（`contains_field`、`matches_pattern`）。

3. **T008 预测粒度**：按 `(mode, workflow)` 分桶就足够了吗，还是需要 `(mode, workflow, model)`？从按 `(mode, workflow)` 分桶开始——它提供了 80% 的预测价值。如果模型差异很大（如在 S26 文档中看到的 latency 差异），之后升级为按 `(mode, workflow, model)`。

4. **T012 go-taskd 修复**：验证报告在 server.go 中发现了足够好的代码——main.go 修复应当被优先处理。这是阻力最小的路径。

### 拒绝的设计（反模式）

- **不要在 forge-core 中添加 YAML 库**：为了保持零依赖，坚持使用现有的 `python3 yaml2json.py` shim。合约解析可以在该 JSON 层面完成，而非在 Go 层面。
- **不要为校准建议添加单独的 API 端点**：`forge scorecard rebuild --calibrate` 是一个 CLI 标志，足够用了。不需要 HTTP API。
- **不要用合约检查来阻断关键路径**：合约验证必须永不中断正常运行（除非明确配置为 `blocking: true`）。静默降级 + trace 事件始终比硬阻断更安全。
- **不要从路径推断 ProdTraffic**：遵循 `risk_diff.go` 中的诚实性注释——这将永远是一种猜测。保持其显式性。
