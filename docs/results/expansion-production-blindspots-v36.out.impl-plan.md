现在我已完整阅读所有 5 个方向的全部内容以及验证报告。以下是我的 Tech Lead 深度分析。

---

# Tech Lead 分析：V36 生产盲区深扫扩展方向

## 📋 任务分解

根据验证报告的 5 个方向（3 个 P1 + 2 个 P2），拆解为 **32 个可执行任务**，每个 2-4 小时。

### 方向一：Prompt 装配可观测性（P1，6 任务）

| 任务 ID | 任务标题 | 涉及文件 | 前置依赖 | 工时 |
|---------|----------|---------|-----------|------|
| TASK-001 | `--dump-prompt` flag + `.forge/prompts/` 目录初始化 | `cmd/forge/main.go`, `internal/prompt/audit.go`, `internal/workspace/init.go` | — | 3h |
| TASK-002 | `buildPrompt` 中 prompt capture hook（输出写入 + argv 注入） | `internal/prompt/prompt_context.go`, `engine_build.go` | TASK-001 | 3h |
| TASK-003 | Prompt 文件格式设计 + 段标记（`<!-- #region -->`）+ 元数据头 | `internal/prompt/audit.go` | TASK-002 | 2h |
| TASK-004 | `forge inspect prompt` 子命令（离线分析 + token 估算 + context sources 报告） | `cmd/forge/inspect.go`, `internal/prompt/inspect.go` | TASK-003 | 4h |
| TASK-005 | Context-assembly 审计日志（结构化 JSON 探针） | `internal/prompt/audit.go`, `internal/trace/trace.go` | TASK-002 | 3h |
| TASK-006 | `forge diff-prompts` 子命令（跨运行对比） | `cmd/forge/diff_prompts.go`, `internal/prompt/diff.go` | TASK-003 | 3h |

### 方向二：Gate 结果持久化与趋势分析（P1，6 任务）

| 任务 ID | 任务标题 | 涉及文件 | 前置依赖 | 工时 |
|---------|----------|---------|-----------|------|
| TASK-007 | 结构化 Gate 结果 JSONL 格式设计 + `.forge/gates/` 日期分片 | `harness/gate-persist.mjs`, `internal/gate/format.go` | — | 2h |
| TASK-008 | `harness/gate-persist.mjs` logger 模块实现 | `harness/gate-persist.mjs` | TASK-007 | 3h |
| TASK-009 | Wire gate-persist 到 `gate.mjs` + `acceptance.mjs` + Go `HarnessRunner` | `harness/gate.mjs`, `harness/acceptance.mjs`, `internal/gate/resolve.go` | TASK-008 | 2h |
| TASK-010 | `forge gate-trend` CLI（基本趋势 + 失败归因 + mode/phase 过滤） | `cmd/forge/gate_trend.go`, `internal/gate/trend.go` | TASK-007 | 4h |
| TASK-011 | 保留策略实现（按天分片 + 可配置保留窗口 + `forge gate-prune`） | `internal/gate/prune.go`, `cmd/forge/gate_prune.go` | TASK-007 | 2h |
| TASK-012 | `forge status` 集成 Gate 健康度仪表盘 | `cmd/forge/status.go`, `internal/gate/health.go` | TASK-010 | 3h |

### 方向五：自治工作流超时与熔断体系（P1，8 任务）

| 任务 ID | 任务标题 | 涉及文件 | 前置依赖 | 工时 |
|---------|----------|---------|-----------|------|
| TASK-013 | 三层次熔断状态机设计 + `CircuitBreaker` 结构体（CLOSED/OPEN/HALF-OPEN） | `internal/orchestrator/circuit.go` | — | 4h |
| TASK-014 | Phase 级熔断器 + 连续失败计数 + 冷却超时 + 降级动作（skip/stub/fail） | `internal/orchestrator/circuit.go` | TASK-013 | 3h |
| TASK-015 | Workflow 级熔断器（连续不收敛检测） | `internal/orchestrator/circuit.go` | TASK-013 | 2h |
| TASK-016 | 系统级熔断器（磁盘/OOM/工具缺失） | `internal/orchestrator/circuit.go` | TASK-013 | 2h |
| TASK-017 | `.forge/circuit.json` 持久化实现（原子读写 + checkpoint 集成） | `internal/orchestrator/circuit_persist.go`, `internal/workspace/init.go` | TASK-013 | 3h |
| TASK-018 | `forge circuit-status` + `forge circuit-reset` CLI | `cmd/forge/circuit_status.go`, `cmd/forge/circuit_reset.go` | TASK-014,TASK-017 | 3h |
| TASK-019 | 熔断器与 backoff/retry/budget 三层集成（状态传递 + 熔断感知重试） | `internal/orchestrator/backoff.go`, `internal/orchestrator/budget.go`, `loop.go` | TASK-014 | 4h |
| TASK-020 | 每 phase 独立超时 + 熔断配置的 YAML workflow 扩展 | `.agent/workflows/build.yml`（schema）, `internal/asset/phase.go` | TASK-019 | 3h |

### 方向三：跨 Phase 语义一致性守卫（P2，6 任务）

| 任务 ID | 任务标题 | 涉及文件 | 前置依赖 | 工时 |
|---------|----------|---------|-----------|------|
| TASK-021 | Phase 输出 Schema 声明格式设计（`schema:` YAML 字段） | `internal/asset/workflow.go`, `internal/coherence/schema.go` | — | 4h |
| TASK-022 | `forge check-coherence` CLI 骨架 + 关键词追溯策略 | `cmd/forge/check_coherence.go`, `internal/coherence/keyword.go` | TASK-021 | 3h |
| TASK-023 | 文件变更断言策略（planner 声明的文件 → git diff 验证） | `internal/coherence/file_assert.go` | TASK-022 | 3h |
| TASK-024 | 结构一致性策略（schema 解析 + phase 上下游匹配） | `internal/coherence/structure.go` | TASK-021 | 4h |
| TASK-025 | Plan-Actual Diff 报告生成（planned vs modified 对比 + 覆盖率） | `internal/coherence/diff_report.go` | TASK-022,TASK-023 | 3h |
| TASK-026 | 集成到 gate phase 或 converge 前验证步骤 | `internal/gate/resolve.go`, `harness/acceptance.mjs` | TASK-022 | 2h |

### 方向四：Token-Aware 上下文预算管理（P2，6 任务）

| 任务 ID | 任务标题 | 涉及文件 | 前置依赖 | 工时 |
|---------|----------|---------|-----------|------|
| TASK-027 | `ModelBudget` 注册表设计 + 已知模型预算数据 | `internal/prompt/budget.go` | — | 2h |
| TASK-028 | Token 估算 v1（基于 char/token 经验比率，零依赖） | `internal/prompt/tokenizer.go` | TASK-027 | 3h |
| TASK-029 | 四优先级上下文装配队列（硬约束/直接相关/有益/锦上添花） | `internal/prompt/priority_queue.go` | TASK-027,TASK-028 | 4h |
| TASK-030 | `--max-context-tokens` CLI flag + 默认值从 model tier 推导 | `cmd/forge/main.go`, `internal/prompt/budget.go` | TASK-027 | 2h |
| TASK-031 | 预算超限告警 + 优先级降级裁剪逻辑（哪些内容被丢弃/缩写） | `internal/prompt/trimmer.go` | TASK-029 | 3h |
| TASK-032 | Wiring 到 `buildPrompt` + `GatherCached` + `memoryContext` | `internal/prompt/prompt_context.go`, `internal/prompt/cache.go`, `internal/prompt/prompt_memory.go` | TASK-030,TASK-031 | 4h |

---

## 🔗 执行顺序与依赖图

```
mermaid
graph TD
    %% Direction 1: Prompt Observability
    subgraph "方向一：Prompt 装配可观测性 (P1)"
        T001["TASK-001: --dump-prompt flag"]
        T002["TASK-002: buildPrompt capture hook"]
        T003["TASK-003: Prompt 文件格式"]
        T004["TASK-004: forge inspect prompt"]
        T005["TASK-005: 审计日志探针"]
        T006["TASK-006: forge diff-prompts"]
    end

    %% Direction 2: Gate Persistence
    subgraph "方向二：Gate 结果持久化 (P1)"
        T007["TASK-007: JSONL 格式设计"]
        T008["TASK-008: gate-persist.mjs logger"]
        T009["TASK-009: Wire 到 gate.mjs/acceptance.mjs/HarnessRunner"]
        T010["TASK-010: forge gate-trend CLI"]
        T011["TASK-011: 保留策略 + gate-prune"]
        T012["TASK-012: forge status 集成"]
    end

    %% Direction 5: Circuit Breaker
    subgraph "方向五：熔断体系 (P1)"
        T013["TASK-013: 三层次状态机设计"]
        T014["TASK-014: Phase 级熔断器"]
        T015["TASK-015: Workflow 级熔断器"]
        T016["TASK-016: 系统级熔断器"]
        T017["TASK-017: .forge/circuit.json 持久化"]
        T018["TASK-018: circuit-status / circuit-reset CLI"]
        T019["TASK-019: backoff/budget/loop 集成"]
        T020["TASK-020: YAML workflow 熔断配置"]
    end

    %% Direction 3: Coherence
    subgraph "方向三：跨 Phase 语义一致性 (P2)"
        T021["TASK-021: Schema 声明格式设计"]
        T022["TASK-022: forge check-coherence + 关键词追溯"]
        T023["TASK-023: 文件变更断言"]
        T024["TASK-024: 结构一致性匹配"]
        T025["TASK-025: Plan-Actual Diff 报告"]
        T026["TASK-026: 集成到 gate phase"]
    end

    %% Direction 4: Token Budget
    subgraph "方向四：Token 预算管理 (P2)"
        T027["TASK-027: ModelBudget 注册表"]
        T028["TASK-028: Token 估算 v1"]
        T029["TASK-029: 优先级装配队列"]
        T030["TASK-030: --max-context-tokens flag"]
        T031["TASK-031: 超限告警 + 裁剪"]
        T032["TASK-032: Wiring 到 buildPrompt"]
    end

    %% 方向内部依赖
    T001 --> T002 --> T003 --> T004
    T002 --> T005
    T003 --> T006

    T007 --> T008 --> T009
    T007 --> T010 --> T012
    T010 --> T011

    T013 --> T014 --> T018
    T013 --> T015
    T013 --> T016
    T014 --> T017 --> T018
    T014 --> T019 --> T020

    T021 --> T022 --> T026
    T021 --> T024
    T022 --> T023 --> T025
    T022 --> T025

    T027 --> T028 --> T029 --> T031 --> T032
    T027 --> T030 --> T032

    %% 跨方向信息流 (非阻塞，仅为集成点)
    T005 -.->|"审计日志可被 T031 消费"| T031
    T012 -.->|"gate 健康度可用于 T015"| T015
    T026 -.->|"一致性检查结果可入 T009"| T009

    %% 可并行组标注
    classDef parallelA fill:#e1f5fe
    classDef parallelB fill:#f0f4c3
    classDef parallelC fill:#fce4ec
    class T001,T007,T013,T021,T027 parallelA
    class T002,T008,T014,T022,T028 parallelB
    class T005,T009,T018,T023,T031 parallelC
```

### **可并行执行组**

| 并行组 | 任务 | 方向 | 说明 |
|--------|------|------|------|
| **G1（启动）** | T001, T007, T013, T021, T027 | D1/D2/D5/D3/D4 | 各方向基础设施设计 — **完全独立** |
| **G2（核心）** | T002, T008, T014, T022, T028 | D1/D2/D5/D3/D4 | 各方向核心实现 — **完全独立** |
| **G3（集成）** | T005, T009, T018, T023, T031 | D1/D2/D5/D3/D4 | 各方向扩展功能 — **完全独立** |
| **G4（收尾）** | T004+T006, T010+T012, T019+T020, T025+T026, T032 | 所有方向 | 各方向最终集成 |

> 所有 5 个方向**彼此零阻塞依赖**，文档确认了这一结论。跨方向的信息流（审计日志 → token 预算、gate 健康度 → workflow 熔断、一致性结果 → gate 持久化）都是消费式集成而非阻塞依赖，可在各方向独立交付后再对接。

---

## ⚠️ 技术风险

### R1：Token 估算精度（方向一、四）
- **风险等级**: 🟡 中
- **描述**: forge-core 禁止外部依赖，无法嵌入 tiktoken（C-based）。v1 基于 chars/token 经验比率（英文 ~3.5:1，CJK ~1.5:1）的估算法在混合语言 prompt 下误差可达 ±30-50%
- **缓解策略**:
  - v1 定位为「参考提示」而非精确计数，prompt 元数据头注明 `estimated`
  - 暴露估算方法为可扩展接口，后续可接入 RPC tokenizer service（`forge tokenize`）
  - 在文档中明确标注估算误差范围，避免用户产生 false confidence
- **验证报告确认**: 方向一证据 C/D 的函数签名有偏差（`Retrieve` 无 `tags` 参数、`memoryContext` 返回 `[]string` 而非 `string`），方向一实现时需以实际代码签名为准

### R2：JSONL 并发写入安全性（方向二）
- **风险等级**: 🟢 低
- **描述**: 当两个 forge 实例同目录并发运行时，JSONL append 可能交错（单行原子但组间交错）
- **缓解策略**:
  - v1 沿用 `trace.jsonl` 同级别的保证（单文件 append 在 POSIX 下是原子的）
  - 在同目录多实例场景的文档中注明「不支持并发写入同一个 `.forge/` 目录」
  - v2 可引入文件锁（`flock`）或按 `run_id` 分片

### R3：熔断 HALF-OPEN 探针安全性（方向五）
- **风险等级**: 🟡 中高
- **描述**: HALF-OPEN 状态下发送探针请求（试探 phase 是否恢复）如果仍然失败，可能破坏 workspace 状态或产生脏数据
- **缓解策略**:
  - Phase 级熔断的探针必须**只读**（dry-run / `--executor=echo`），不执行真实 agent
  - 探针结果仅用于判定 OPEN→CLOSED 或 OPEN→再 OPEN，不用于 checkpoint 更新
  - 探针失败后冷却时间加倍（exponential backoff on probe），避免频繁试探

### R4：关键词追溯的准确率（方向三）
- **风险等级**: 🟡 中
- **描述**: 关键词匹配有高 FP/FN 风险——planner 说 "rate limiter" 但 implementer 用 "throttle" 实现，关键词追溯会误报未完成
- **缓解策略**:
  - 关键词追溯定位为 **warning 而非 blocking**，不阻断 workflow
  - 支持关键词同义词/正则替换（`required_keywords: ["rateLimit|throttle|limiter"]`）
  - 结构一致性（TASK-024）是更可靠的策略，建议在关键词追溯之上优先投入

### R5：YAML workflow schema 向后兼容（方向三、五）
- **风险等级**: 🟡 中
- **描述**: 为 workflow YAML 增加 `schema:`（方向三）和 `circuit_breaker:`（方向五）字段，旧 workflow 文件如果不更新会缺失这些配置
- **缓解策略**:
  - 所有新字段均为 **optional**，缺省时方向三跳过一致性检查（N/A），方向五使用全局默认配置
  - 加载时对未知字段不报错（宽容解析）
  - 提供 `forge workflow upgrade` 命令自动补充默认值

### R6：`--dump-prompt` 敏感信息泄露（方向一）
- **风险等级**: 🟡 中
- **描述**: Prompt 中可能包含 API key、内部 URL、商业机密等敏感信息。落盘后可能被无意提交
- **缓解策略**:
  - 框架自动在 `.forge/` 根目录创建 `.gitignore`（已有机制），确保 `.forge/prompts/` 不被提交
  - `forge inspect prompt` 输出中提供 `--redact` 选项（正则替换敏感模式）
  - 文档警告用户「prompt 文件包含发给 agent 的完整指令，自行评估敏感性」

---

## 👥 资源评估

### 团队组成建议

| 角色 | 技能要求 | 数量 | 主要负责方向 |
|------|----------|------|-------------|
| **Go 后端工程师（Senior）** | Go 标准库、CLI 设计、结构化并发、文件系统 I/O | 1-2 人 | 方向一/四/五（Go 核心） |
| **全栈工程师（Senior）** | Go + Node.js、shell 集成、YAML schema | 1 人 | 方向二/三 + 跨方向集成 |
| **Tech Lead / 架构师** | 系统设计、代码审查、跨方向协调 | 0.5-1 人（兼职） | 全部 |

**最小可行团队**：2 名 Senior 工程师（均熟悉 Go + Node.js），1 名 Tech Lead（兼职审查）

### 关键里程碑

| 里程碑 | 时间 | 交付物 | 验收标准 |
|--------|------|--------|---------|
| **M1: 基础设施就绪** | Day 3 | `.forge/prompts/`、`.forge/gates/`、`.forge/circuit.json` 创建逻辑 | `forge status` 显示三个新子目录状态 |
| **M2: P1 核心可运行** | Day 7 | `--dump-prompt` 输出文件 + `gate-trend` 显示 7 天数据 + `circuit-status` 显示 CLOSED | 端到端跑通一条 evolve 工作流 |
| **M3: P2 核心可运行** | Day 11 | `check-coherence` 输出 Plan-Actual Diff + 自动 token 预算裁剪生效 | 5 个方向的 CLI 子命令全部可用 |
| **M4: 集成测试完成** | Day 14 | 跨方向集成场景全部通过 + 文档更新 + 旧兼容性验证 | `harness/acceptance.mjs` 全绿 |

### 阻塞点（Blockers）

| 阻塞点 | 影响方向 | 解决策略 |
|--------|---------|----------|
| forge-core 零依赖策略 vs token 估算精度 | 方向四 | v1 用 char-based 估算（可接受误差），v2 单独评估 tiktoken 引入的决策，不阻塞交付 |
| 纯 Go 无 tiktoken binding | 方向四 | 自研轻量 BPE 编码（~200 行）或提供 `--tokenizer-cmd` 外部调用接口 |
| `.forge/` 目录缺失时所有新功能的行为 | 全部 | 统一在 workspace init 中创建所有子目录；缺失时各自优雅降级 |
| 现有 workflow YAML 文件无 schema/circuit_breaker 字段 | 方向三、五 | 全部 default 为 optional，无 schema 时跳过（N/A），无 circuit_breaker 时使用全局默认值 |

---

## ✅ 质量保证

### 单元测试覆盖要求

| 模块 | 最低覆盖率 | 关键测试场景 |
|------|-----------|-------------|
| `internal/prompt/audit.go` | 85% | `--dump-prompt` 写入、段标记格式、元数据头、目录自动创建 |
| `internal/prompt/inspect.go` | 80% | 空目录、单 phase、多 phase、无元数据降级处理 |
| `internal/prompt/diff.go` | 85% | 相同 prompt、全不同 prompt、部分不同、文件缺失 |
| `internal/gate/trend.go` | 90% | 空数据、单日数据、多日数据、PASS/FAIL 统计、按 mode/phase 过滤 |
| `internal/gate/prune.go` | 90% | 保留窗口边界、空目录、保留过程中文件被删除 |
| `internal/orchestrator/circuit.go` | 95% | CLOSED→OPEN→HALF-OPEN→CLOSED 全状态转换、阈值边界、冷却超时 |
| `internal/orchestrator/circuit_persist.go` | 85% | 原子读写、并发安全（文件锁）、损坏文件恢复、空文件 |
| `internal/coherence/keyword.go` | 90% | 精确匹配、正则匹配、同义词展开、无 plan 降级、多文件扫描 |
| `internal/prompt/budget.go` | 90% | 多模型预算、未知模型降级、预算边界、零预算 |
| `internal/prompt/priority_queue.go` | 90% | 四优先级排队、预算内满载、预算超限降级、优先级内评分排序 |

### 集成测试策略

| 测试场景 | 涉及方向 | 方法 | 判定标准 |
|----------|---------|------|---------|
| `--dump-prompt` + `forge inspect prompt` | D1 | 跑 `forge run build --executor=dry --dump-prompt /tmp/prompts` 后用 inspect 读取 | inspect 输出包含 phase/ model/ token_count/ context_sources |
| 连续 3 次 gate FAIL 后熔断打开 | D5+D2 | mock gate runner 连续返回 FAIL；验证熔断状态 | `circuit-status` 显示 OPEN；`gate-trend` 记录 3 次 FAIL |
| Plan-Actual Diff 报告 | D3 | 有 schema 的 workflow + 故意让 implementer 漏改一个文件 | Diff 报告显示 coverage 不足 |
| Token 预算超限裁剪 | D4 | `--max-context-tokens 1000` 跑有 10 个 ADR 的 workflow | 最终 prompt 不超 ~1000 token（±20%），且 AGENTS.md 必在 |
| 跨 run 熔断状态持久化 | D5 | 第一跑 phase 熔断 OPEN → 第二跑 `forge circuit-status` | OPEN 状态跨进程保留 |
| 并发方向基础设施共存 | 全部 | 同时启用 `--dump-prompt` + 打开熔断 + 跑 gate-persist | 3 个 `.forge/` 子目录协调工作，不冲突 |

### 代码审查要点

| 审查维度 | 重点关注 |
|----------|---------|
| **零依赖纪律** | 方向四的 token 估算是否引入了新依赖？如有，是否违反了 forge-core 零依赖原则？ |
| **状态机正确性** | 方向五的熔断状态转换是否完整？是否有未覆盖的转换路径（如 HALF-OPEN→直接 CLOSED 超时）？ |
| **错误处理** | 所有新 CLI 子命令在 `.forge/` 子目录缺失时是否优雅降级还是 panic？ |
| **YAML 向后兼容** | 方向三/五新增的 schema / circuit_breaker 字段对旧 workflow 文件的加载是否无影响？ |
| **竞态条件** | 方向二的 JSONL 写入 + 方向五的 circuit.json 写入在并发场景下的安全性 |
| **CLI 命名一致性** | 新子命令是否遵循 `forge <noun> <verb>` 模式（`forge gate-trend`、`forge circuit-status`） |
| **日志 vs stdout** | 审计日志（方向一）写入文件，gate 趋势（方向二）写入文件 + CLI 输出，熔断状态（方向五）写入文件 + CLI 输出——三者日志风格是否一致 |

### 性能测试需求

| 测试 | 条件 | 指标 | 阈值 |
|------|------|------|------|
| Prompt dump 性能开销 | 带/不带 `--dump-prompt` 各跑 20 次 evolve | 额外延迟 | < 5% （disk write 不阻塞 agent 启动） |
| Gate trend 查询性能 | 模拟 365 天 × 100 gate/天 = 36,500 行 JSONL | 查询耗时 | < 500ms（无索引 grep） |
| 熔断状态机性能 | 1000 次状态转换循环 | 转换耗时 | < 1μs/次（纯内存操作） |
| Token 估算速度 | 1K/10K/100K token 文本各 100 次估算 | 吞吐 | > 1000 tokens/ms（char-based） |
| 多方向同时启用 | 全部 5 个方向开启下跑完整 evovle | 总延迟增加 | < 10% |

---

## 📅 实施计划

### 人员配置
- **Dev A**（Go Senior）：方向一 + 方向四（Go 核心）
- **Dev B**（Go + Node.js Senior）：方向二 + 方向三（Go + Node + YAML）
- **Dev A+B 协作**：方向五（最大、最复杂，需两人分担）
- **Tech Lead**：每日代码审查 + 方向间集成协调

```
gantt
    title V36 生产盲区扩展实施计划
    dateFormat  YYYY-MM-DD
    axisFormat  %m-%d

    section 阶段 1：基础设施 (Days 1-2)
    TASK-001: --dump-prompt flag           :a1, 2026-07-14, 1d
    TASK-007: JSONL 格式设计                :a2, 2026-07-14, 0.5d
    TASK-013: 熔断状态机设计                :a3, 2026-07-14, 1d
    TASK-021: Schema 格式设计               :a4, 2026-07-15, 1d
    TASK-027: ModelBudget 注册表            :a5, 2026-07-15, 0.5d
    `.forge/` 子目录统一初始化              :a6, 2026-07-14, 0.5d

    section 阶段 2：P1 核心实现 (Days 3-7)
    TASK-002: buildPrompt capture hook     :b1, 2026-07-16, 1d
    TASK-003: Prompt 文件格式               :b2, 2026-07-17, 0.5d
    TASK-008: gate-persist.mjs logger      :b3, 2026-07-16, 1d
    TASK-009: Wire 到 gate.mjs + acceptance :b4, 2026-07-17, 0.5d
    TASK-014: Phase 级熔断器               :b5, 2026-07-16, 1d
    TASK-015: Workflow 级熔断器             :b6, 2026-07-17, 0.5d
    TASK-016: 系统级熔断器                  :b7, 2026-07-17, 0.5d
    TASK-017: circuit.json 持久化           :b8, 2026-07-18, 1d
    TASK-005: 审计日志探针                  :b9, 2026-07-18, 1d
    TASK-010: forge gate-trend CLI         :b10, 2026-07-18, 1d

    section 阶段 3：P1 收尾 + P2 启动 (Days 8-10)
    TASK-004: forge inspect prompt          :c1, 2026-07-21, 1d
    TASK-006: forge diff-prompts            :c2, 2026-07-22, 1d
    TASK-011: 保留策略 + gate-prune         :c3, 2026-07-21, 0.5d
    TASK-012: forge status 集成             :c4, 2026-07-22, 1d
    TASK-018: circuit-status/reset CLI      :c5, 2026-07-21, 1d
    TASK-019: backoff/budget/loop 集成      :c6, 2026-07-22, 1d
    TASK-020: YAML workflow 熔断配置        :c7, 2026-07-23, 1d
    TASK-022: forge check-coherence + 关键词 :c8, 2026-07-23, 1d
    TASK-028: Token 估算 v1                :c9, 2026-07-23, 1d

    section 阶段 4：P2 完成 + 集成 (Days 11-14)
    TASK-023: 文件变更断言                  :d1, 2026-07-24, 1d
    TASK-024: 结构一致性匹配                :d2, 2026-07-24, 1d
    TASK-025: Plan-Actual Diff 报告         :d3, 2026-07-25, 1d
    TASK-026: 集成到 gate phase             :d4, 2026-07-25, 0.5d
    TASK-029: 优先级装配队列                :d5, 2026-07-24, 1.5d
    TASK-030: --max-context-tokens flag     :d6, 2026-07-25, 0.5d
    TASK-031: 超限告警 + 裁剪               :d7, 2026-07-25, 1d
    TASK-032: Wiring 到 buildPrompt         :d8, 2026-07-28, 1d
    跨方向集成测试 + Bug 修复                :d9, 2026-07-28, 2d
    文档更新 + 验收                         :d10, 2026-07-29, 1d
```

### 阶段划分总结

| 阶段 | 日期 | 天数 | 交付物 |
|------|------|------|--------|
| **S1: 基础设施搭建** | Day 1-2 | 2 | 5 个方向的模块骨架 + `.forge/` 子目录统一初始化 + 所有新 CLI 子命令 stub |
| **S2: P1 核心功能** | Day 3-7 | 5 | `--dump-prompt` 全链路 + gate 结果 JSONL 持久化 + 三层次熔断状态机 + `.forge/circuit.json` 持久化。**里程碑 M1 (Day 3), M2 (Day 7)** |
| **S3: P1 收尾 + P2 核心** | Day 8-10 | 3 | `forge inspect/diff-prompts` + `forge gate-trend/status` + `forge circuit-status/reset` + YAML 集成 + 关键词追溯 + char-based token 估算。**里程碑 M3 (Day 10)** |
| **S4: P2 完成 + 集成测试** | Day 11-14 | 4 | 文件变更断言 + 结构一致性 + Plan-Actual Diff + 优先级队列 + token 预算裁剪 + wiring + 全方向集成测试 + 文档。**里程碑 M4 (Day 14)** |

---

## 📌 关键建议

### 1. 交付顺序建议
文档的优先级建议（D1=D2=D5 > D3=D4）合理。但考虑到工程量，建议内部的交付顺序为：

```
方向二 (Gate 持久化) → 方向五 (熔断体系) → 方向一 (Prompt 观测) → 方向四 (Token 预算) → 方向三 (语义一致性)
```

理由：方向二是最薄、收益最快的（~5 天即可看到 gate 趋势数据）；方向五现状是工程痛点（三层孤立无协调）；方向一需要 UX 打磨最耗时；方向三/四依赖设计先行且投入产出周期最长。

### 2. 验证报告提到的 3 处不准确需修正
文档中 `Retrieve` 签名、`memoryContext` 签名、`GateProof` 结构体共 3 处代码引用不准确——**这些是方向一和方向二的实现细节偏差，不影响论证骨架**，但实现时务必以实际代码为准。建议在实现对应方向时先读实际代码再动工。

### 3. 跨方向集成点做接口预留（不要做紧耦合）
五个方向之间建议通过**文件系统事件**（写入 `.forge/audit/` → 方向四消费、写入 `.forge/gates/` → 方向五消费）松耦合，不做跨包函数调用。这保证每个方向可独立交付、单独测试。

### 4. `forge-core` 零依赖纪律不可违反
方向四的 token 估算是唯一可能引入外部依赖的地方。**v1 必须使用纯 Go char-based 近似方法**。如果后续需要精确 token 计数，提供 `--tokenizer-cmd` 外部调用接口而非内嵌 CGO。
