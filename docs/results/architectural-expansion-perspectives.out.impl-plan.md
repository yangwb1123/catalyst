Now I have a thorough understanding of the codebase and all relevant analysis documents. Let me compile the Tech Lead analysis.

---

# Tech Lead Analysis: 5-Direction Architectural Expansion

## Executive Summary

Based on the code-verified assessment, I've analyzed the input document against the actual codebase. Of the 5 proposed directions, only **3 have genuine novelty**—the other 2 are already fully covered by existing analysis documents. This analysis focuses implementation effort on the 3 real gaps, with explicit merge recommendations for the other 2.

| Direction | Novelty | Verdict | Implementation Priority |
|-----------|---------|---------|------------------------|
| D1: Phase-to-phase filesystem isolation | ✅ Fully novel | **Implement** | P1 |
| D2: Structured Agent output contract | ⚠️ Problem known, solution novel | **Implement** | P1 |
| D3: Workflow declaration versioning | ⚠️ Partial coverage, deeper solution | **Implement incrementally** | P2 |
| D4: Context budget management | ❌ Fully covered in `novel-expansion-directions-v19.md` | **Merge** → adopt existing doc's design | — |
| D5: Cross-run audit & replay | ❌ Fully covered in `seventh-wave-data-realism.md` | **Merge** → adopt existing doc's design | — |

---

## 1. 任务分解 (Task Decomposition)

### TASK-001：Phase 文件系统隔离原语
| 字段 | 内容 |
|------|------|
| **方向** | D1: Phase-to-phase filesystem isolation |
| **涉及文件** | `forge-core/internal/orchestrator/workspace.go` (新), `forge-core/internal/orchestrator/command_executor.go` (改) |
| **前置依赖** | 无 |
| **预估工时** | 4h |
| **验收标准** | 新 `Workspace` 类型提供 `Isolate()` → `root` 方法，支持 `workspace://<phase-name>` 路径解析；每个 isolation 策略（snapshot/cow/git-worktree）实现 `Isolator` 接口 |

**具体工作**：
- 定义 `Isolator` 接口：`Snapshot() (string, error)` → 返回隔离后的工作目录
- 实现 `PassthroughIsolator`（零隔离，当前行为，向后兼容默认）
- 实现 `GitWorktreeIsolator`（`git worktree add` 创建隔离副本，成本最低的 Go-stdlib-only 方案）
- 100% 单元测试覆盖三种策略

### TASK-002：CommandExecutor 集成隔离层
| 字段 | 内容 |
|------|------|
| **方向** | D1 |
| **涉及文件** | `forge-core/internal/orchestrator/command_executor.go` (改), `forge-core/internal/orchestrator/executor.go` (改) |
| **前置依赖** | TASK-001 |
| **预估工时** | 3h |
| **验收标准** | `CommandExecutor.Dir` 不再硬编码为 `o.root`，而是每个 phase 由 `Build` 闭包选择隔离目录；`DryRunExecutor` 报告隔离决策但不变更实际目录 |

**具体工作**：
- `AgentExecutor.Execute` 新增 `isolatedRoot string` 参数（或通过 context 传递）
- `CommandExecutor.Build` 闭包接入 `Workspace.Root`
- 向后兼容：未设置 isolator 时行为与当前 `Dir: o.root` 完全一致（byte-identical trace output）
- `DryRunExecutor` 诚实标注隔离类型和根目录

### TASK-003：Workflow 声明 phase 隔离策略
| 字段 | 内容 |
|------|------|
| **方向** | D1 |
| **涉及文件** | `forge-core/internal/asset/asset.go` (改), `.agent/architecture/phase-isolation.yml` (新) |
| **前置依赖** | TASK-001 |
| **预估工时** | 2h |
| **验收标准** | `asset.Phase` 新增 `Isolation string` 字段（""=继承workflow, "passthrough"/"git-worktree"/"snapshot"）；已有 workflow YAML 不修改时行为零变化；新增 `forge validate` 检查 isolation 策略名称合法性 |

### TASK-004：Phase 间 side-effect 传递契约
| 字段 | 内容 |
|------|------|
| **方向** | D1 |
| **涉及文件** | `forge-core/cmd/forge/engine_build.go` (改), `forge-core/cmd/forge/prompt_context.go` (改) |
| **前置依赖** | TASK-002, TASK-003 |
| **预估工时** | 3h |
| **验收标准** | 隔离后的 phase 真实文件变更不污染前序/后序 phase 的工作目录；`feeds_forward` 继续通过文本传递跨 phase 信息（不依赖共享文件系统）；git-worktree 策略下 `forge evolve` 每个 iteration 创建新 worktree，iteration 结束后自动清理 |

**具体工作**：
- `feeds_forward` 注入 `phaseOutputLedger` 时同时传递 phase 的产出文件列表（`emits:` 声明）
- 隔离 phase 完成后，`observeFor` 回调将产出文件复制到共享目录
- git-worktree 策略实现 `PostPhase` 清理钩子

### TASK-005：Agent 输出 Schema 注册中心
| 字段 | 内容 |
|------|------|
| **方向** | D2: Structured Agent output contract |
| **涉及文件** | `forge-core/internal/contract/schema.go` (新), `forge-core/internal/contract/registry.go` (新) |
| **前置依赖** | 无 |
| **预估工时** | 4h |
| **验收标准** | `Schema` 类型支持 JSON Schema 定义版本化；`Registry` 支持注册和查询 schema（按 agent × phase × version）；`Validate(output string, schema Schema) (bool, string[])` 校验输出是否符合 schema 并返回具体违规列表 |

**具体工作**：
- `Schema` 结构体：`Name, Version, Agent, Phase, Definition (JSON Schema string)`
- `Registry` 结构体：map 索引 `agent:phase` → `[]Schema`（多版本）
- `Validate` 函数：使用 Go 标准库 `encoding/json` 验证 + 字段级违规报告
- 内置注册 reviewer/architect/confidence 三种基础 schema
- 单元测试覆盖 schema 匹配、版本降级、违规检测

### TASK-006：Agent prompt 注入契约声明
| 字段 | 内容 |
|------|------|
| **方向** | D2 |
| **涉及文件** | `forge-core/cmd/forge/prompt_context.go` (改), `forge-core/cmd/forge/prompt_artifacts.go` (改) |
| **前置依赖** | TASK-005 |
| **预估工时** | 3h |
| **验收标准** | `buildPrompt` 在 agent prompt 中注入当前期契约声明（格式版本 + 预期输出 schema）；agent 输出被 `Observe` 捕获后自动校验 schema；校验失败输出详细违规原因并在 trace 中记录 `decision` 事件 |

**具体工作**：
- `buildPrompt` 新增 `contractLane`：输出格式版本号 + 预期字段 + 示例
- `observeFor` 中注入 schema 验证回调
- 验证失败记录 `trace.DecisionEvent` + 详细违规列表
- 向后兼容：无注册 schema 时跳过验证（行为零变化）

### TASK-007：替代 last-line 解析器为 schema 验证
| 字段 | 内容 |
|------|------|
| **方向** | D2 |
| **涉及文件** | `forge-core/cmd/forge/cost.go` (改), `forge-core/cmd/forge/prompt_context.go` (改) |
| **前置依赖** | TASK-005, TASK-006 |
| **预估工时** | 3h |
| **验收标准** | `parseReviewerVerdict`、`parseExecutiveVerdict`、`parseConfidenceScore` 三条解析路径优先走 schema 验证，验证通过后从结构化字段提取值；验证失败才 fallback 到末行解析；零值返回不再静默——改为 trace 记录 `decision` 事件说明解析降级 |

**具体工作**：
- 每条解析路径增加优先路径：先调 `contract.Validate`，成功则提取结构化字段
- 验证失败 + fallback 日志：`trace.DecisionEvent("contract_fallback", "schema vN validation failed for phase X, falling back to last-line parser: ...")`
- `parseConfidenceScore` 支持从 `confidence: 85/100` 字段统一提取
- 所有三个解析器保持向后兼容（fallback 路径的输入输出与当前完全一致）

### TASK-008：契约版本协商与 fail-closed
| 字段 | 内容 |
|------|------|
| **方向** | D2 |
| **涉及文件** | `forge-core/internal/contract/negotiation.go` (新) |
| **前置依赖** | TASK-005 |
| **预估工时** | 2h |
| **验收标准** | agent prompt 中声明 forge-core 支持的最新版本号；agent 输出在 `_format` 字段中声明使用的版本号；版本不匹配时执行 fail-closed（拒绝解析而非静默降级）；negotiation 过程记录到 trace |

**具体工作**：
- `Negotiate(supported []string, declared string) (string, bool)` — 声明的版本在支持列表中则通过
- `fail-closed` 行为：版本不匹配时，`Execute` 返回 `KindContractViolation` 而非完成
- `DryRunExecutor` 中报告「版本协商路径」
- trace 记录：`contract_negotiation` 事件

### TASK-009：Workflow SchemaVersion 声明
| 字段 | 内容 |
|------|------|
| **方向** | D3: Workflow declaration versioning |
| **涉及文件** | `forge-core/internal/asset/asset.go` (改) |
| **前置依赖** | 无 |
| **预估工时** | 2h |
| **验收标准** | `asset.Workflow` 新增 `SchemaVersion string` 字段，JSON 标签 `schema_version`；未声明时默认空字符串，运行时行为零变化；`forge validate` 输出当前 workflow 的 schema 版本号 |

### TASK-010：Migration engine（diff + verify）
| 字段 | 内容 |
|------|------|
| **方向** | D3 |
| **涉及文件** | `forge-core/internal/migrate/workflow.go` (新), `forge-core/internal/migrate/workflow_test.go` (新) |
| **前置依赖** | TASK-009 |
| **预估工时** | 4h |
| **验收标准** | `Diff(from asset.Workflow, to asset.Workflow) ([]Change, error)` 输出 phase 级别的增/删/改差异；`Verify(from asset.Workflow, to asset.Workflow) ([]Conflict, error)` 检测不兼容变更（删除 target phase 引用、重排导致 loop-back 断链）；`Safe(from, to) bool` 判断是否可以原地升级 |

**具体工作**：
- `Change` 类型：`PhaseAdded`、`PhaseRemoved`、`PhaseReordered`、`StopConditionChanged`、`LoopTargetChanged`
- `Conflict` 类型：`DanglingLoopBack`（on_fail 目标的 phase 不存在）、`DanglingDependsOn`、`VersionJump`（跳号）
- `Safe` 的规则：仅新增 phase = 安全；删除、重排、改 stop condition = 不安全
- 与 `strategic-expansion-and-edge-cases.md` 方向 E 的 `workflow_version` checkpoint 跨引用集成（TASK-012）

### TASK-011：`forge workflow migrate` CLI 命令
| 字段 | 内容 |
|------|------|
| **方向** | D3 |
| **涉及文件** | `forge-core/cmd/forge/main.go` (改), `forge-core/cmd/forge/migrate.go` (改) |
| **前置依赖** | TASK-010 |
| **预估工时** | 2h |
| **验收标准** | `forge workflow migrate --dry-run` 输出 diff + 兼容性报告但不写入；`forge workflow migrate --apply` 执行迁移；`forge workflow migrate --from-version X --to-version Y` 跨版本升级；迁移前自动执行 Verify |

### TASK-012：Checkpoint 绑定 workflow 版本
| 字段 | 内容 |
|------|------|
| **方向** | D3 |
| **涉及文件** | `forge-core/internal/persist/checkpoint.go` (改), `forge-core/cmd/forge/evolve.go` (改) |
| **前置依赖** | TASK-009 |
| **预估工时** | 2h |
| **验收标准** | `persist.Checkpoint` 新增 `WorkflowVersion string` 和 `WorkflowSHA string`；`forge evolve --resume` 检测 checkpoint 的 workflow 版本与当前声明版本不匹配时 warn + 可选拒绝；版本差异通过 TASK-010 的 `Safe` 判断是否可继续 |

### TASK-013：跨项目 schema 追踪
| 字段 | 内容 |
|------|------|
| **方向** | D3 |
| **涉及文件** | `forge-core/internal/asset/schema_tracker.go` (新) |
| **前置依赖** | TASK-009 |
| **预估工时** | 2h |
| **验收标准** | `forge-init` 新建项目时记录 workflow schema version；`forge doctor` 扫描当前项目 workflow 版本与全局声明版本的偏差；差异报告包含 compat 等级（safe/breaking/unknown） |

### TASK-014：D4 合并到 novel-expansion D1 的补充实现
| 字段 | 内容 |
|------|------|
| **方向** | D4 → Merge into `novel-expansion-directions-v19.md` 方向一 |
| **涉及文件** | `forge-core/internal/prompt/budget.go` (新) |
| **前置依赖** | 无（直接采用现有设计） |
| **预估工时** | 4h |
| **验收标准** | 实现 `novel-expansion-directions-v19.md` 方向一的 Budget struct + Allocate 方法；P0-P4 优先级模型；token 估算器（字符启发式）；连接 `GatherCached` 为第 4 步分配可用窗口 |

### TASK-015：D5 合并到 seventh-wave D5 的补充实现
| 字段 | 内容 |
|------|------|
| **方向** | D5 → Merge into `seventh-wave-data-realism.md` 方向五 |
| **涉及文件** | `forge-core/cmd/forge/replay.go` (新) |
| **前置依赖** | 无（直接采用现有设计） |
| **预估工时** | 3h |
| **验收标准** | 实现 `forge replay` 命令；`--save-trajectory` 保存代理轨迹；fixture 文件格式与第七波文档一致；trace 事件种类从 1 种扩充到 8 种 |

---

## 2. 执行顺序 (Dependency Graph)

```mermaid
graph TD
    %% D1: Phase isolation — 3 parallel entry points
    subgraph "D1: Filesystem Isolation"
        T001[TASK-001: Isolation Primitives<br/>Workspace interface + Impls<br/>4h]
        T001 --> T002[TASK-002: CommandExecutor Integration<br/>3h]
        T002 --> T004[TASK-004: Side-effect Transfer Contract<br/>3h]
        T003[TASK-003: Workflow Declaration<br/>2h]
        T003 --> T002
    end

    %% D2: Output contract — 2 parallel entry points
    subgraph "D2: Structured Output Contract"
        T005[TASK-005: Schema Registry<br/>4h]
        T005 --> T006[TASK-006: Prompt Injection<br/>3h]
        T005 --> T008[TASK-008: Version Negotiation<br/>2h]
        T006 --> T007[TASK-007: Replace Last-line Parsers<br/>3h]
    end

    %% D3: Workflow versioning — independent sub-tasks
    subgraph "D3: Workflow Versioning"
        T009[TASK-009: SchemaVersion Field<br/>2h]
        T009 --> T010[TASK-010: Migration Engine<br/>4h]
        T009 --> T012[TASK-012: Checkpoint Binding<br/>2h]
        T009 --> T013[TASK-013: Cross-project Tracker<br/>2h]
        T010 --> T011[TASK-011: CLI forge workflow migrate<br/>2h]
    end

    %% Merged directions
    subgraph "Merged Directions"
        T014[TASK-014: Budget Allocator<br/>4h<br/>(merge with novel-expansion D1)]
        T015[TASK-015: forge replay<br/>3h<br/>(merge with seventh-wave D5)]
    end

    %% Cross-direction integration
    T004 -.->|phase output feeds schema validation| T006
    T012 -.->|checkpoint version used in| T011
```

### 可并行执行的任务组

| 并行组 | 任务 | 预估总工时 | 条件 |
|--------|------|-----------|------|
| **组 A** | TASK-001, TASK-003, TASK-005, TASK-009 | 12h | 无前置依赖，可 4 人并行 |
| **组 B** | TASK-002, TASK-006, TASK-008, TASK-010, TASK-012, TASK-013 | 14h | 依赖组 A 完成 |
| **组 C** | TASK-004, TASK-007, TASK-011 | 8h | 依赖组 B 完成 |
| **组 D** | TASK-014, TASK-015 | 7h | 无前置依赖，可 2 人并行 |

---

## 3. 技术风险 (Technical Risks)

### 3.1 Phase Isolation (D1) — 高风险

| 风险 | 概率 | 影响 | 缓解策略 |
|------|------|------|---------|
| `git worktree` 在非 git 仓库中不可用 | 中 | 高（基础方案失效） | 提供 `PassthroughIsolator` 作为 fallback；检测 `git` 是否可用并诚实降级 |
| git-worktree 创建成本随仓库变大增加 | 高（大型 repo） | 中（iteration 间隔延长） | 添加 `ReuseIsolator` 对连续同隔离策略的 phase 复用 worktree；记录创建耗时到 trace |
| 隔离 phase 的产出文件被后序 phase 需要但不能访问 | 中 | 高（阻断 workflow） | 明确 `emits:` 声明作为「隔离破口」——声明文件从隔离目录复制到共享目录 |
| Git-worktree 清理失败导致 `.git/worktrees` 残留 | 低 | 中（需要手动处理） | 增加 `forge doctor` 检测残留 worktree；`prune` 清理命令 |
| Bash 命令绕过隔离（`cd /tmp && git clone`） | 中 | 高（隔离被打破） | 诚实标注：隔离防止意外污染，不防恶意绕过——这与 readonlyToolScope 的设计哲学一致 |

### 3.2 Output Contract (D2) — 中风险

| 风险 | 概率 | 影响 | 缓解策略 |
|------|------|------|---------|
| Agent 输出格式多样性超过 schema 覆盖范围 | 高（不同 agent 输出格式差异大） | 中 | schema registry 的版本号允许每个 agent/phase 独立演进；validator 可选 strict/relaxed 模式 |
| `lastNonEmptyLine` 并行路径与 schema 验证不一致 | 中 | 高（输出读错） | 并行双写验证：schema 验证和末行解析同时跑，验证结果不一致时记录 `decision` 事件 + 用 schema 结果覆盖 |
| JSON 输出嵌套过深超出 Go `encoding/json` 默认 Unmarshal 限制 | 低 | 低 | 在 schema 注册时检测并拒绝超过 maxNesting 的 schema |
| 大段代码输出中夹带 JSON 格式输出导致误解析 | 中 | 中 | prompt 中明确要求输出放在代码块外；parser 只取最后出现的 JSON block |

### 3.3 Workflow Versioning (D3) — 低风险

| 风险 | 概率 | 影响 | 缓解策略 |
|------|------|------|---------|
| SchemaVersion 语义不一致（semver vs 自增） | 中 | 低 | 采用 semver 格式，`Safe` 检查依赖 major version；major bump = breaking change |
| `forge workflow migrate --apply` 中途失败导致部分迁移 | 低 | 中 | 使用临时文件 + 原子重命名（与 `persist.Save` 同样的原子模式） |
| 与 `strategic-expansion-and-edge-cases.md` 方向 E 的 checkpoint 版本定义不一致 | 中 | 中 | 实现前与现有分析交叉确认；采用 CHECK 结构体复用 `persist.Checkpoint.WorkflowVersion` |

### 3.4 性能瓶颈预判

| 瓶颈点 | 触发条件 | 预估影响 | 优化方向 |
|--------|---------|---------|---------|
| git-worktree 创建（每次 phase → ~500ms） | 50 iteration × 5 phase = 250 次 | ~2min overhead | 策略复用：同一 iteration 的 phase 共享 worktree |
| Schema 验证（每次 agent 输出 → JSON unmarshal + validation） | 每次 agent phase | ~50μs | validator 可缓存编译后的 schema（内置 `schema.Precompile`） |
| Workflow diff（大 workflow） | migrate --dry-run 时 | ~1ms | diff 只涉及 phase 列表，不会大 |
| token 估算（budget allocator） | 每次 `buildPrompt` | ~10ms | 采用字符启发式而非 tiktoken |

---

## 4. 资源评估 (Resource Assessment)

### 4.1 开发人员需求

| 角色 | 技能要求 | 负责方向 | 建议人数 |
|------|---------|---------|---------|
| Go 基础架构工程师 | Go 标准库、文件系统操作、进程管理 | D1: Phase isolation | 1 人 |
| Go 中间件工程师 | JSON Schema、协议设计、版本化 | D2: Output contract | 1 人 |
| Go CLI 工程师 | CLI 设计、迁移策略、向后兼容 | D3: Workflow versioning | 1 人 |
| Go/Prompt 工程师 | Prompt 工程、token 估算、上下文管理 | D4: Budget allocator (merge) | 0.5 人 |
| 全栈测试工程师 | 集成测试、fixture 设计、回放引擎 | D5: Replay (merge) + 集成 | 0.5 人 |

**最小团队**: 2 人（1 人 D1, 1 人 D2+D3, 共享测试）
**理想团队**: 3 人（D1×1, D2×1, D3×1, 测试共享）
**时间窗口**: 3 周（理想团队全职）

### 4.2 关键里程碑

| 里程碑 | 时间点 | 交付物 | 验证方式 |
|--------|-------|--------|---------|
| **M1: 基础设施完整** | 第 5 天 | TASK-001, 003, 005, 009 全部完成 | 各组 A 任务 pass `forge accept` |
| **M2: 核心功能可运行** | 第 10 天 | TASK-002, 006, 008, 010, 012, 013 全部完成 | 各组 B 任务 pass `forge accept` + 集成测试 |
| **M3: 完整闭环** | 第 13 天 | TASK-004, 007, 011 全部完成 | 各组 C 任务 + 端到端 test |
| **M4: 合并方向补充** | 第 15 天 | TASK-014, 015 完成 | 与 novel-expansion/seventh-wave fixture 对齐 |
| **M5: 全闸门绿** | 第 15 天 | 全部 15 个 TASK 完成 | `forge accept` 全绿 + fresh-context review |

### 4.3 阻塞点与解决策略

| 阻塞点 | 阻塞路径 | 解决策略 |
|--------|---------|---------|
| git-worktree 需要 git ≥2.5 | TASK-001 的 GitWorktreeIsolator | fallback 到 `PassthroughIsolator` + warn；`forge doctor` 检测 git 版本 |
| JSON Schema 验证缺少 Go 标准库实现 | TASK-005 | 纯标准库验证：手动实现 subset（type/required/properties/enum），不引入外部依赖。fail-open：超出 subset 的 schema 规则诚实标注为 `unchecked` |
| `forge workflow migrate --apply` 的文件写入与 `forge run` 的 phase 写入冲突 | TASK-004, TASK-011 | migrate 在设计阶段运行，不在 build 阶段；与 parallel 模式的 `.forge` 目录锁一致 |
| YAML 转 JSON 的 python shim 无法传递 SchemaVersion | TASK-009 | 在 shim 中新增 `--schema-version` 参数或直接从 asset source 读取 version 字段 |

---

## 5. 质量保证 (Quality Assurance)

### 5.1 单元测试覆盖要求

| 包 | 要求覆盖率 | 关键测试用例 |
|----|-----------|-------------|
| `internal/orchestrator/workspace.go` | ≥90% | 三种隔离策略创建/销毁；非 git 仓库优雅降级；并发安全（并行 phase） |
| `internal/contract/` | ≥95% | schema 注册/查询/版本化；Validate 通过/拒绝对照表；大 schema 性能 |
| `internal/migrate/workflow.go` | ≥90% | 增 phase/删 phase/重排/改 stop condition 四种 diff；Safe 判断准确性；空 workflow |
| `internal/prompt/budget.go` | ≥95% | 优先级分配；窗口满降级；token 估算误差边界；不同模型窗口 |
| `cmd/forge/replay.go` | ≥80% | fixture 加载；事件重播；时间轴准确性 |

### 5.2 集成测试策略

| 测试场景 | 类型 | 方法 |
|---------|------|------|
| **D1: 隔离 phase 相互独立** | 集成 | 创建两个 phase：phase-A 写 `marker-a.txt`，phase-B 验证该文件不存在；分别在 passthrough 和 git-worktree 下跑 |
| **D2: Schema 验证拦截错误输出** | 集成 | 构造一个错误格式的 agent 输出 → `forge run` 记录 `contract_fallback` 决策事件；在 trace 中验证 |
| **D2: 版本协商拒绝** | 集成 | agent 输出声明 `_format: forgeos.output.v0`（不支持的版本）→ `forge run` 返回 `KindContractViolation` |
| **D3: Workflow migrate dry-run** | 集成 | 修改 workflow 删除一个 phase → `forge workflow migrate --dry-run` 报告 `PhaseRemoved` + `unsafe` |
| **D3: Checkpoint 版本失配** | 集成 | 手动修改 checkpoint.json 的 `WorkflowVersion` → `forge run --resume` warn |
| **D4+D5: 与现有分析对齐** | 集成 | 用 novel-expansion D1 fixture 验证 budget allocator；用 seventh-wave fixture 验证 replay |

### 5.3 代码审查要点

| 方向 | 审查焦点 | 审查人角色 |
|------|---------|-----------|
| **D1** | `Isolator` 接口的完备性——所有阶段（创建、使用、清理、失败）是否都有明确定义？bash 逃逸路径？ | 安全工程师 |
| **D2** | Schema 版本化策略——major bump 的触发条件是否明确？新版本 registry 是否有迁移脚本？ | 资深 Go 工程师 |
| **D3** | `Safe` 判断的完备性——是否覆盖了所有 workflow 变更类型？是否是 monotonic（safe + safe = safe）？ | 架构师 |
| **All** | 向后兼容——每个新字段的零值是否等于旧行为？`omitempty` 是否按需使用？现有 fixture 是否保持不变？ | Fresh-context reviewer |
| **All** | 红线遵守——文件 ≤ 500 行？函数 ≤ 50 行？循环依赖 = 0？ | CI 自动 + 手动 |

### 5.4 性能测试需求

| 测试 | 场景 | 通过标准 |
|------|------|---------|
| git-worktree 创建延迟 | 50 次交替 phase 隔离/销毁 | 平均每次 ≤ 1s（在 ext4/xfs 上） |
| Schema 验证吞吐 | 1000 次 Validate 调用 | 全部 ≤ 500ms |
| Budget allocation 延迟 | 6 lane 配置 | 每次 ≤ 10ms |
| Workflow diff 延迟 | 20 phase + 10 agent workflow | ≤ 5ms |
| Memory 压力 | 10000 条 memory 记录 | `boundMemory` 过滤 ≤ 100ms |

---

## 6. 实施计划 (Implementation Plan)

### 阶段 1: 基础设施搭建（第 1-5 天）

**并行轨道 A**（D1 基础）：
| 天 | 任务 | 产出 |
|---|------|------|
| 1 | TASK-001: 设计 `Isolator` 接口 + 实现 `PassthroughIsolator` | 接口定义 + 1 个实现 |
| 2 | TASK-001: 实现 `GitWorktreeIsolator`（创建/清理/复用） | 2 个实现 + 测试 fixture |
| 3 | TASK-003: Phase.Isolation 字段 + validate 检查 | Phase 扩展 + 验证规则 |
| 4-5 | Integration: D1 三件套集成 + `forge accept` | 通过闸门 |

**并行轨道 B**（D2 基础）：
| 天 | 任务 | 产出 |
|---|------|------|
| 1-2 | TASK-005: Schema + Registry + Validate | contract 包可用 |
| 3 | TASK-005: 内置 3 个 schema | reviewer/architect/confidence |
| 4-5 | TASK-009: Workflow.SchemaVersion 字段 | asset 扩展 |

**并行轨道 C**（D4+D5 合并）：
| 天 | 任务 | 产出 |
|---|------|------|
| 1-3 | TASK-014: Budget allocator | prompt/budget.go |
| 4-5 | TASK-015: forge replay skeleton | cmd/forge/replay.go |

### 阶段 2: 核心功能实现（第 6-10 天）

**轨道 A**（D1 集成）：
| 天 | 任务 | 产出 |
|---|------|------|
| 6-7 | TASK-002: CommandExecutor 接入 Workspace | 隔离 phase 可运行 |
| 8-10 | TASK-004: phase 间 side-effect 传递 | 隔离 + feeds_forward 协同 |

**轨道 B**（D2 集成）：
| 天 | 任务 | 产出 |
|---|------|------|
| 6-7 | TASK-006: Prompt 注入契约声明 | buildPrompt 含 contract lane |
| 7-8 | TASK-008: 版本协商 fail-closed | 契约版本不匹配拒绝 |
| 9-10 | TASK-007: 替代 last-line 解析器 | schema 优先 + 降级 trace |

**轨道 C**（D3 集成）：
| 天 | 任务 | 产出 |
|---|------|------|
| 6-7 | TASK-010: Migration engine | Diff + Verify + Safe |
| 8 | TASK-012: Checkpoint 绑定 | WorkflowVersion + WorkflowSHA |
| 9-10 | TASK-013: 跨项目 schema 追踪 | forge doctor 扩展 |

### 阶段 3: 集成测试和优化（第 11-13 天）

| 天 | 活动 | 产出 |
|---|------|------|
| 11 | TASK-011: forge workflow migrate CLI | 命令可用 |
| 11-12 | 跨方向集成测试 | D1-D3 联合 fixture 通过 |
| 12 | 性能测试（隔离延迟、schema 验证吞吐） | 性能基线数据 |
| 13 | 优化（git-worktree 复用、validator 缓存） | 优化数据 |

### 阶段 4: 发布准备（第 14-15 天）

| 天 | 活动 | 产出 |
|---|------|------|
| 14 | fresh-context reviewer 审查（D1, D2, D3 各 1 人） | 审查通过 |
| 14 | 文档更新：ARCHITECTURE.md、CLAUDE.md | 各方向使用说明 |
| 15 | `forge accept` 全绿 + 合并到 main | 发布就绪 |
| 15 | 更新 ROADMAP.md 标记交付 | 版本里程碑 |

### 甘特图

```
天:   1  2  3  4  5  6  7  8  9 10 11 12 13 14 15
     ┌──────────────────────────────────────────┐
D1   │─── T001 ───│── T002 ─│──── T004 ────│    │
     │  T003  │                                  │
D2   │─── T005 ───│── T006 ─│── T007 ─│         │
     │    │   │      T008 │                      │
D3   │  T009 │─── T010 ─│── T012 ─│── T011 ─│   │
     │         │    T013    │                   │
D4   │─── T014 ───│                              │
D5   │──── T015 ─────│                          │
     │         │         │   集成   │ 优化 │ 发布 │
     └──────────────────────────────────────────┘
     阶段1      阶段2       阶段3      阶段4
```

---

## 总结与建议

### 核心发现

1. **D1 (Phase isolation) 是最高价值的新方向**——填补了一个真实的安全/运维空白。建议作为 **P1 优先**。
2. **D2 (Output contract) 虽然问题已被识别，但方案级别的实现是全新的**——`internal/contract/` 包对代码库的「解析可靠性和可扩展性」有长期价值。建议作为 **P1 同等优先**与 D1 并行。
3. **D3 (Workflow versioning) 需要与现有分析对齐**——`strategic-expansion-and-edge-cases.md` 方向 E 已有浅层方案。本文案采集成方式，不推翻已有设计。建议作为 **P2**。
4. **D4 和 D5 不应作为独立方向提出**——直接合并到 `novel-expansion-directions-v19.md` 方向一 和 `seventh-wave-data-realism.md` 方向五 作为增量实现。提交前需要与那两份文档的作者协调。

### 最高风险行动项

- **TASK-001 的 git-worktree 可行性验证**：在第 1 天即验证目标 OS 上 git ≥2.5 是否可用，以决定是否需要降级策略。
- **TASK-005 的 JSON Schema subset 实现**：Go 标准库无 schema 验证器，需要手工实现一个 subset。第 1 天确定 subset 的范围（properties/required/enum/type/pattern）并编写原型。
- **TASK-014 与 novel-expansion-v19 的 D1 对齐**：现有的 `internal/prompt/` 目录下有 5 个文件（cache.go, prompt.go, retrieve.go 等），需要确认 budget.go 的插入位置不会造成循环依赖。

### 实施就绪度检查清单

每个 phase 在推进前应满足：

- [ ] 该 TASK 的 `forge accept` 可通过（新建/修改文件不超过 500 行）
- [ ] 与已有分析的交叉引用已记录在注释中
- [ ] 向后兼容方案已有文档（零值策略 + `omitempty` 使用）
- [ ] 审计性：关键决策有 trace 事件记录
- [ ] Fresh-context reviewer 已确认独立性（不与实现者同一人）
