现在我已彻底阅读了所有相关代码。以下是深度 Tech Lead 分析。

---

# Tech Lead 分析：五个方向的技术实施计划

## 0. 分析基础

**代码库背景**：`forge-core` 是一个 Go 单体应用（`cmd/forge` 为 CLI 入口，`internal/` 为库包），外加一个 `/harness` Node.js 门禁系统。所有五个方向都基于对实际代码的深入审查，而非理论推测。

**关键架构边界**：
- `internal/routing` — 已有 `ModelMap`/`ResolveModel()` 厂商路由骨架 ✅
- `cmd/forge/engine_build.go` — `claudeArgv()` 硬编码 `--model`，没有厂商选择 GAP 🔴
- `cmd/forge/prompt_context.go` — `buildPromptWithEmits()` 无总 token 检查 GAP 🔴
- `internal/memory/memory.go` — `Confidence` 字段已存在，但仅用于显示前缀，2 个消费者 GAP 🔴
- `internal/trace/trace.go` — `Event` 无 `PromptText` 字段 GAP 🔴

---

## 1. 任务分解

### 方向一：Prompt Token 预算管理（P1）

| 任务 ID | 标题 | 涉及文件 | 前置依赖 | 预估工时 | 验收标准 |
|---------|------|---------|---------|---------|---------|
| **TASK-001** | 实现 `EstimateTokens()` 估算函数 | `cmd/forge/prompt_budget.go` (新文件) | 无 | 2h | `EstimateTokens(s string) int` 对纯文本返回 `len([]rune(s))/4` + 对中英文混合分别估算；对空字符串返回 0；有表驱动测试覆盖边界情况（空、纯英文、纯中文、纯代码、混合） |
| **TASK-002** | 定义模型感知的 Token 预算常量 | `cmd/forge/prompt_budget.go` (TASK-001 同时) | TASK-001 | 1h | 导出 `ModelBudget map[string]int` = `{"haiku": 8000, "sonnet": 16000, "opus": 200000}`；向后兼容：未知模型默认 16000；编译时常量检查 |
| **TASK-003** | 在 `buildPromptWithEmits()` 中注入总 token 检查 | `cmd/forge/prompt_context.go` | TASK-001, TASK-002 | 3h | `buildPromptWithEmits` 在所有 context lane 追加*之后*，但在 `prompt.Build()` 调用*之前*，计算估算的 token 总数；超出预算时记录警告（`logln`）；不中断流程（Phase A 仅监控） |
| **TASK-004** | 实现预算降级逻辑（降级到更便宜的模型） | `cmd/forge/prompt_budget.go` | TASK-003 | 4h | `DegradeForBudget(tier string, estimatedTokens, budget int) string` 在超出预算时返回下一档更低模型；Haiku 降级返回 `"haiku"` 不变；已有 `DowngradeOne()` 可用，但需 token 预算感知；有测试验证边界情况 |
| **TASK-005** | 将 token 预算降级接入 `phaseTierResolver` | `cmd/forge/engine_build.go` | TASK-004 | 3h | 在 `phaseTierResolver` 的*主* `--model` 路由链中添加 token 预算降级作为最终防线（`BudgetAdjustTier` 之后）；确保 `opusFloorAgents` 豁免于 token 预算降级；trace 事件记录降级决策 |

**方向一 Phase A 总计：13h（~2 个开发日）**

---

### 方向二：语义验证管线（P1）

| 任务 ID | 标题 | 涉及文件 | 前置依赖 | 预估工时 | 验收标准 |
|---------|------|---------|---------|---------|---------|
| **TASK-006** | 实施 Go 编译门禁 gate | `harness/gate.mjs`, `harness/gates/go-compile.mjs` (新文件) | 无 | 3h | 新 `go-compile` gate 对 Go 项目运行 `go build ./...`；非 Go 项目跳过（N/A）；在 `go-compile` 之前缓存 `go vet` 产出；返回机器可读的 PASS/FAIL + stderr 输出；有 `test_gate.mjs` 单元测试 |
| **TASK-007** | 实施 Node.js 编译门禁 gate | `harness/gate.mjs`, `harness/gates/node-compile.mjs` (新文件) | 无 | 2h | 新 `node-compile` gate 对存在 `package.json` 的项目运行 `node --check` 扫描（`find . -name '*.js' -not -path '*/node_modules/*'`）；非 JS/TS 项目跳过；返回 PASS/FAIL + 输出 |
| **TASK-008** | 在 `build.yml` 中注册编译门禁 | `.agent/workflows/build.yml` | TASK-006, TASK-007 | 1h | 将 `compile` 添加到 `harness-gates` 阶段的 `required_gates`：`[lint, test, build, complexity, arch, security, compile]`；确保 `compile` 被 mode gating 正确过滤（explorer 模式豁免）；向后兼容：现有 `policies.yml` 中未定义 mode 时默认启用 |
| **TASK-009** | 扩展 `test` gate 以检查新代码编译 | `harness/gate.mjs` | 无 | 2h | 在 `test` gate 的 `spawn` 之前新增一个预检查步骤，对 `git diff --name-only` 中检测到的任何新 `.go`/`.js`/`.ts` 文件运行 `go vet`/`node --check`；保留避免重复工作的编译结果缓存；对无变更项目为零开销 |

**方向二 Phase A 总计：8h（~1 个开发日）**

---

### 方向三：确定性重放（P2）

| 任务 ID | 标题 | 涉及文件 | 前置依赖 | 预估工时 | 验收标准 |
|---------|------|---------|---------|---------|---------|
| **TASK-010** | 向 `trace.Event` 添加 `PromptText` 字段 | `internal/trace/trace.go` | 无 | 1h | 向 `Event` 结构体添加 `PromptText string \`json:"prompt_text,omitempty"\``；确保 `omitempty` 使旧 JSONL 向后兼容；更新现有测试以验证 `prompt_text` 为空时省略 |
| **TASK-011** | 向 `persist.Checkpoint` 添加 `MemorySnapshot` 字段 | `internal/persist/checkpoint.go` | 无 | 1h | 向 `Checkpoint` 添加 `MemoryPath string \`json:"memory_path,omitempty"\`` 和 `MemoryEntryCount int \`json:"memory_entry_count,omitempty"\``；在 `--resume` 上，`PhaseIndex` 恢复使用现有快照；零值 = 无快照（向后兼容） |
| **TASK-012** | 录制 prompt 文本以供重放 | `cmd/forge/prompt_context.go`, `cmd/forge/engine_build.go` | TASK-010 | 4h | 向 `buildPromptWithEmits` 添加可选 `promptSink func(string)` 参数；在 `engine_build.go` 中，从 `observeFor` 或 `agentExecutor.Build` 内部将 prompt 文本传递到 `tracer.Emit()`；`prompt_text` 出现时附带 `prompt_snapshot` 字段标记；Phase A 默认*不*录制 prompt（opt-in：`--trace-prompt` flag） |
| **TASK-013** | 实现重放 bundle（最近 5 轮） | `cmd/forge/replay.go` (新文件) | TASK-010, TASK-011 | 4h | 新 `ReplayBundle` 结构体 = `{Traces []trace.Event, Memory []memory.Entry, Checkpoint persist.Checkpoint}`；存储最近 5 轮（`Iteration` > current-5）；bundle 目标存储路径为 `<.forge>/replay/iter-<N>.json`；每个 bundle < 1MB（5 轮 ~80KB + prompt ~10KB）；有测试验证轮换和大小边界 |
| **TASK-014** | 添加 `--replay <iter>` CLI 命令 | `cmd/forge/main.go`, `cmd/forge/replay.go` | TASK-013 | 3h | 新 `forge replay <iter>` 子命令：加载 bundle、打印 prompt 文本和 memory、可选择重放执行（dry-run 模式）；输出人类可读的完整 prompt + agent 输出摘要；通过 `--replay-only`（仅打印）和 `--replay-exec`（重放，干燥）进行测试 |

**方向三总计：13h（~2 个开发日）— 优先级较低，可在后续 sprint 完成**

---

### 方向四：多厂商模型路由（P2）

| 任务 ID | 标题 | 涉及文件 | 前置依赖 | 预估工时 | 验收标准 |
|---------|------|---------|---------|---------|---------|
| **TASK-015** | 在 CLI 级别添加 Provider 配置格式 | `cmd/forge/config.go` (新文件), `routing.go` | 无 | 3h | 新增 `ProviderConfig` 结构体：`{Name string, DefaultTier string, EnvVar string}`；添加`--provider` flag（默认 `"anthropic"`）；读取 `FORGE_PROVIDER` 环境变量；验证提供者存在于 `routing.Providers()` 中，否则失败并给出清晰错误信息 |
| **TASK-016** | 将提供者选择接入 `claudeArgv` | `cmd/forge/engine_build.go` | TASK-015 | 4h | 修改 `claudeArgv` 以读取 `o.provider`；将 `--model` 切换为使用 `routing.ResolveModel(provider, tier)` 而非硬编码 tier；更新 `tierOf` 闭包以传递提供者；确保 `opusFloorAgents` 保持 Opus 等级*无论提供者如何*——在提供者未知时降级失败（fail-closed） |
| **TASK-017** | 扩展 `ModelMap` 以支持额外提供者 | `internal/routing/routing.go` | 无 | 2h | 向 `ModelMap` 添加 `"openai"`：`{Haiku: "gpt-4o-mini", Sonnet: "gpt-4o", Opus: "o3"}`；添加 `"google"`：`{Haiku: "gemini-2.0-flash-lite", Sonnet: "gemini-2.0-flash", Opus: "gemini-2.5-pro"}`；为未知提供者提供清晰的编译时/测试错误信息 |
| **TASK-018** | 实现提供者感知的等级解析 | `internal/routing/routing.go` | TASK-017 | 3h | 新增 `ResolveTier(provider, baseTier string) string`：为每个提供者映射等级等价；新增 `CrossProviderDowngrade(tier, provider string) string`：在提供者间保留下级语义；按 `routing_test.go` 中的提供者分组测试；验证提供者切换时的等级一致性 |
| **TASK-019** | 添加提供者集成测试 | `cmd/forge/route_test.go`, `internal/routing/routing_test.go` | TASK-015—TASK-018 | 3h | mock `ModelMap` 驱动测试：验证提供者切换正确地改变 `--model`；验证未知提供者失败；验证提供者回退到 anthropic（v1 默认行为）；集成测试在无 API key 时跳过 |

**方向四总计：15h（~2 个开发日）— 需要仔细设计；安全部署建议 feature-flag 保护**

---

### 方向五：Memory 价值感知管理（P1）

| 任务 ID | 标题 | 涉及文件 | 前置依赖 | 预估工时 | 验收标准 |
|---------|------|---------|---------|---------|---------|
| **TASK-020** | 实现 Confidence 感知排序 | `cmd/forge/prompt_memory.go` | 无 | 3h | 修改 `boundMemory()` 以接收 `entries` 和 `query`，输出按 `confidence * relevance` 排序的 entries；在 `memoryCap` 下保留全量；超过 `memoryCap` 时：丢弃价值最低的 entry（`confidence * relevance` 最低者）；原有的 `memoryRecencyFloor` 保留最新 8 条（confidence 无关）；已有 `prompt_memory_test.go` 测试验证排序行为 |
| **TASK-021** | 实现 Confidence 加权相关性评分 | `cmd/forge/prompt_memory.go`, `internal/prompt/retrieve.go` | TASK-020 | 4h | 修改 `relevantOlder` 以将 `confidence` 合并到 BM25 相关性评分中：`adjustedScore = bm25Score * confidence`；在 `prompt.Retrieve` 中添加 `WeightedRetrieve(docs, query, k, weights []float64)` 签名；为 `prompt.Doc` 添加可选的 `Weight` 字段；文档：confidence 加权是*相对*排序，而非绝对排名 |
| **TASK-022** | 实现源权重方案 | `cmd/forge/prompt_memory.go` | TASK-021 | 2h | 定义 `sourceWeight` 映射：`{"implementer": 0.5, "reviewer": 1.0, "planner": 0.7, "qa": 0.8, "product-manager": 0.6}`；在 entry 加载时：`effectiveConfidence = entry.Confidence * sourceWeight[entry.Source]`；未知 source → weight = 1.0（默认信任但记录警告）；在 `prompt_memory_test.go` 中测试 |
| **TASK-023** | 实现随迭代/时间的价值衰减 | `cmd/forge/prompt_memory.go` | TASK-022 | 3h | 添加 `decayFactor`：`decayedConfidence = effectiveConfidence * (1.0 - decayRate)^(currentIteration - entry.Iteration)`；`decayRate = 0.05`（每次迭代衰减 5%，20 次迭代后 decayed 至 ~36%）；对于 `iteration == 0`（旧格式 entries），使用 `CreatedAtUnix` 做基于时间的衰减（`decayPerDay = 0.1`）；在 `prompt_memory_test.go` 中使用模拟迭代值测试 |
| **TASK-024** | 将 Confidence 接入 Memory Compaction | `internal/memory/memory_compact.go` | TASK-022 | 3h | 修改 `compactByKind()` 以按 `confidence` 排序：保留 confidence 最高的 entries，而非简单地保留最近 N 条；为低 confidence entries 添加专门的 early-expiry 路径（`confidence < 0.2` → 有资格在任何 age 被 compaction）；更新 `summarizeBlock()` 以报告被 compaction 的 entries 的平均 confidence；添加 `TestCompactByConfidence` |

**方向五 Phase A 总计：15h（~2 个开发日）**

---

## 2. 执行顺序

```mermaid
graph TD
    %% Phase A 基础设施（高优先级，可并行）
    subgraph PhaseA["Phase A — 基础观测与防护"]
        T001[TASK-001: EstimateTokens]
        T002[TASK-002: Token Budget 常量]
        T003[TASK-003: Build 中注入 Token 检查]
        T020[TASK-020: Confidence 排序基础]
        T006[TASK-006: Go Compile Gate]
        T007[TASK-007: Node Compile Gate]
    end

    T001 --> T002
    T002 --> T003
    T020 --- T021[TASK-021: Confidence 加权评分]
    T006 --> T008[TASK-008: Build.yml 注册 compile]
    T007 --> T008

    %% Phase B — 降级与衰减
    subgraph PhaseB["Phase B — 主动降级与衰减"]
        T003 --> T004[TASK-004: Token Budget 降级]
        T004 --> T005[TASK-005: 接入 tierResolver]
        T021 --> T022[TASK-022: Source 权重方案]
        T022 --> T023[TASK-023: 迭代衰减]
        T023 --> T024[TASK-024: Confidence 接入 Compaction]
    end

    %% 低优先级 —— 方向三 & 四
    subgraph PhaseC["Phase C — 可观测性与可重放性"]
        T010[TASK-010: Trace.Event 加 PromptText]
        T011[TASK-011: Checkpoint 加 MemorySnapshot]
        T010 --> T012[TASK-012: 录制 Prompt 文本]
        T012 --> T013[TASK-013: 重放 Bundle 5 轮]
        T013 --> T014[TASK-014: --replay CLI]
    end

    subgraph PhaseD["Phase D — 多厂商路由（Feature-flag 保护）"]
        T015[TASK-015: Provider Config]
        T015 --> T016[TASK-016: Provider 接入 claudeArgv]
        T017[TASK-017: 扩展 ModelMap]
        T017 --> T018[TASK-018: Provider 感知等级解析]
        T016 --> T019[TASK-019: 集成测试]
        T018 --> T019
    end

    %% 方向间依赖
    T003 -.-> T020 样式:虚线,颜色:gray
    T008 -.-> T012 样式:虚线,颜色:gray
```

**可并行任务组**：

| 组 | 任务 | 并行原因 |
|----|------|---------|
| **G1** (基础设施) | TASK-001, TASK-002, TASK-020, TASK-006, TASK-007 | 零代码共享；分别涉及不同的包 |
| **G2** (核心实现) | TASK-003, TASK-021 | TASK-003 仅依赖 TASK-001+002；TASK-021 仅依赖 TASK-020 |
| **G3** (降级) | TASK-004→005, TASK-022→023→024 | 各自在同一方向内顺序，但方向间无共享数据 |
| **G4** (低优先级) | TASK-010→011→012→013→014, TASK-015→016→017→018→019 | 不共享代码，无数据竞争 |

---

## 3. 技术风险

### 风险矩阵

| # | 风险 | 方向 | 可能性 | 影响 | 缓解策略 |
|---|------|------|--------|------|---------|
| R1 | **Token 估算不准确**：`EstimateTokens()` 可能在中文、代码、JSON 块上偏差 2-3 倍 | D1 | 高 | 中 | 采用保守估算（上取整到 `len/3`）；在 Phase A 中仅打日志不截断；积累真实数据后用模型感知计数器替换 |
| R2 | **Confidence 不可靠**：Confidence 是 Agent 自报告的，天然高置信偏差 | D5 | 高 | 高 | `sourceWeight` 方案（implementer=0.5）是主要缓解手段；在 Phase A 中仅用 confidence 做排序提示而非硬截断；需要外部验证信号（v2） |
| R3 | **Provider 切换改变模型行为**：OpenAI GPT-4o 在同一个 prompt 上的表现不同于 Claude Sonnet，破坏确定性 | D4 | 中 | 高 | Feature-flag `--provider` 默认为 `anthropic`；provider 切换在 `routing.go` 中记录 trace `decision` 事件；集成测试要求 provider=openai 时同一 prompt 的输出为同一 schema |
| R4 | **Test gate 仍不覆盖未测试代码**：即使加了 compile gate，新函数若无测试调用，test gate 仍 PASS | D2 | 中 | 中 | compile gate 只检查语法不检查可达性；Phase B 可加 `unused-function` gate（基于 `go vet -unused` / `ts-prune`）；文档明确标注局限 |
| R5 | **Trace/prompt 录制放大写放**：每轮录制 ~10KB prompt 文本，5 轮 = ~50KB，长时间运行下积累显著 | D3 | 低 | 低 | bundle 上限为 5 轮；opt-in `--trace-prompt`；超过 1MB 时自动 GC 旧轮 |
| R6 | **Confidence 衰减过大导致记忆过早丢失**：`decayRate=0.05`×20 次迭代 = 36%，对长运行可能丢弃仍有价值的旧条目 | D5 | 中 | 中 | 保留 `memoryRecencyFloor=8` 最新条目不受衰减影响；decayRate 可在运行时配置（环境变量 `FORGE_MEMORY_DECAY`） |

### 性能瓶颈分析

| 瓶颈点 | 位置 | 当前成本 | 优化后成本 | 备注 |
|--------|------|---------|---------|------|
| Token 估算 | `buildPromptWithEmits` | O(0) | O(n) 其中 n=prompt 长度（~20KB） | 可忽略；`len(runes)/4` 是 O(n) 但 n 很小 |
| Confidence 加权 BM25 | `relevantOlder` → `prompt.Retrieve` | O(d·t) d=文档数 t=词项数 | O(d·t) 相同，常数因子增加 | 加权是乘法，不影响算法复杂度 |
| Replay Bundle 序列化 | 迭代结束写入 JSON | O(0) | O(e+m) e=events m=memory entries | 5 轮 ~80KB，< 5ms；可忽略 |
| Compile Gate | `harness/gate.mjs` → `go build ./...` | 0（未运行） | 首次 5-30s，缓存后 <1s | 缓存 go build 产物；增量编译显著加速 |

---

## 4. 资源评估

### 人员技能矩阵

| 角色 | 技能要求 | 负责方向 | 建议人数 |
|------|---------|---------|---------|
| **Go 后端工程师** | 精通 Go、并发、CLI 架构 | D1, D4（routing 层）, D5（memory 层） | 1-2 |
| **全栈工程师** | Go + Node.js，熟悉 CI/CD | D2（harness gates） | 1 |
| **质量/测试工程师** | 测试策略、集成测试、CI pipeline | D2（集成）, D4（集成测试） | 1（可与上者合并） |
| **Tech Lead / 架构师** | 系统设计、跨方向协调 | 全部 | 1（负责 review、设计决策） |

**建议团队规模**：2-3 名开发 + 1 名 TL（或 TL 兼任开发）

### 关键里程碑

| 里程碑 | 时间（自 sprint 开始） | 产出 | 依赖 |
|--------|----------------------|------|------|
| M1：编译门禁绿灯 | Day 3 | TASK-006, -007, -008 通过验收，CI 中 `compile` gate 运行 | G1 |
| M2：Token 预算可观测 | Day 4 | TASK-001, -002, -003 完成，`forge run` 在 prompt 超预算时记录警告 | G1+G2 |
| M3：Confidence 排序上线 | Day 5 | TASK-020, -021, -022 完成，`boundMemory` 按加权 confidence 排序 | G1+G2 |
| M4：主动降级就绪 | Day 7 | TASK-004, -005, -023, -024 完成，token 和 memory 降级都在生产中 | Phase B |
| M5：Provider 路由验证 | Day 10 | TASK-015—TASK-019 通过，`--provider openai` 在集成测试中有效 | Phase D |
| M6：Replay 工具可用 | Day 12 | TASK-010—TASK-014 完成，`forge replay` CLI 可用 | Phase C |

### 阻塞点与解决策略

| 阻塞点 | 描述 | 严重性 | 解决策略 |
|--------|------|--------|---------|
| **B1**：D4 中无 OpenAI API key | 集成测试需要真实的非 Anthropic API key | 高（阻塞 TASK-019） | 为 openai 测试提供 `FORGE_OPENAI_KEY` 环境变量（可选）；无 key 时标记为 skip；单元测试使用 mock `ModelMap` |
| **B2**：compile gate 的平台差异 | `go build` 在 CI vs 本地可产生不同结果 | 中 | gate 应在 `forge-core` 仓库根目录运行以保证一致性；CI 应当预装最新 Go 工具链 |
| **B3**：Memory confidence 在初始冷启动期无意义 | 前 5-10 次迭代没有足够数据做价值排序 | 低 | `memoryRecencyFloor=8` 在不依赖 confidence 的情况下保留最新 entries；冷启动期降级为纯按 recency 排序 |
| **B4**：Prompt 录制与大 prompt 的交互 | `prompt_text` 字段可使 JSONL 膨胀 5x | 中 | `omitempty` + `--trace-prompt` opt-in flag；长 prompt 的 JSON 转义可能损坏；在 `encode()` 中使用 `json.Marshal`（安全） |

---

## 5. 质量保证

### 单元测试覆盖要求

| 方向 | 文件 | 最低覆盖行 | 关键测试场景 |
|------|------|-----------|------------|
| D1 | `prompt_budget.go` | 95% | 空/纯英文/纯中文/代码/混合 prompt；未知模型默认 16K；Haiku 降级返回 "haiku"；降级环形保护 |
| D2 | `harness/gates/go-compile.mjs` | 90% | 有效 Go 项目 → PASS；语法错误 → FAIL；非 Go 项目 → N/A；缓存命中 → 2x 加速 |
| D2 | `harness/gates/node-compile.mjs` | 90% | 有效 JS 文件 → PASS；语法错误 → FAIL；非 JS 项目 → N/A |
| D3 | `replay.go` | 90% | Bundle ≤1MB；轮换 5 轮 → 丢弃最旧轮；空 trace → 优雅处理；不完整 bundle → 错误信息 |
| D4 | `routing/routing.go` | 95% | 已知 provider+tier → 正确模型；未知 provider → 回退到 tier 名称；`ResolveModel("", tier)` → anthropic |
| D5 | `prompt_memory.go` | 90% | Confidence 排序：高 > 低；sourceWeight：reviewer=1.0 > implementer=0.5；衰减：newer > older；早期淘汰：confidence < 0.2 |

### 集成测试策略

| 测试套件 | 位置 | 运行时机 | 描述 |
|---------|------|---------|------|
| **CI Gate 集成** | `.github/workflows/forge.yml` | 每次 push | `harness/acceptance.mjs` 包含新 compile gate 检查 |
| **Token Budget E2E** | `cmd/forge/prompt_budget_test.go` | `node --test cmd/forge/` | 用真实 prompt 构建 `buildPrompt` → 验证估算和降级决策 |
| **Provider 切换** | `cmd/forge/route_test.go` | `go test ./cmd/forge/` （有 API key 时） | `--provider openai` → `--model` 正确切换；无 key 时跳过 |
| **Memory 排序回归** | `cmd/forge/prompt_memory_test.go` | `go test ./cmd/forge/` | 固定 64 条 entry 的语料库 → 验证 top-32 包含正确的 mix |

### 代码审查要点

每个 PR 必须审查以下内容：

1. **向后兼容性**：新字段是否标记 `omitempty`？零值是否等于旧行为？
2. **降级安全性**：token/memory/confidence 降级是否尊重 `opusFloorAgents`？
3. **锁安全性**：`runBudget`、`trace.Tracer`、`phaseOutputLedger` 中的新并发访问是否需要 `sync.Mutex`？
4. **资源泄露**：compile gate 中的 `spawnSync` 是否清理子进程？bundle 写入是否关闭文件？
5. **日志诚实性**：降级/截断决策是否产生清晰的 `logln` 消息（`forge: ⚠ ...` 格式）？
6. **测试覆盖**：新函数是否包含表驱动测试？边界情况（空、最大长度、未知值）是否覆盖？

### 性能测试需求

| 测试 | 方向 | 测试工具 | 阈值 | 频率 |
|------|------|---------|------|------|
| Prompt 构建延迟 | D1 | `go bench ./internal/prompt/` | 每次构建 <50µs | 每次 commit |
| Memory 排序吞吐 | D5 | `go bench ./cmd/forge/` | 64 entries 排序 <10µs | 每次 commit |
| Compile gate 缓存效率 | D2 | `hyperfine 'node harness/gate.mjs compile'` | 缓存后 <1s | 每周 |
| Provider 切换开销 | D4 | `go bench ./internal/routing/` | 等级解析 <1µs | 每次 commit |

---

## 6. 实施计划

### Sprint N 时间线（建议 2 周）

```
阶段 | 活动                              | Day1 Day2 Day3 Day4 Day5 Day6 Day7 Day8 Day9 Day10 Day11 Day12
-----|-----------------------------------|--------------------------------------------------------------
G1   | EstimateTokens + Budget 常量      | ██
G1   | Confidence 排序基础                | ██
G1   | Go + Node Compile Gate            | ██   ██
G2   | Build 中注入 Token 检查            |           ██   ██
G2   | Confidence 加权评分               |           ██   ██
M1   | 🏁 编译门禁绿灯                    |                     █
M2   | 🏁 Token 预算可观测                |                     █
M3   | 🏁 Confidence 排序上线             |                     █
PB   | Token Budget 降级 + Resolver 接入 |                     ██   ██
PB   | Source 权重 + 迭代衰减            |                     ██   ██   ██
PBC  | Confidence 接入 Compaction        |                               ██
M4   | 🏁 主动降级就绪                    |                                     █
PD   | Provider Config + claudeArgv 接入 |                                        █   ██
PD   | ModelMap 扩展 + 等级解析           |                                           ██
PD   | 集成测试                          |                                              ██
M5   | 🏁 Provider 路由验证               |                                                █
PC   | Trace/Checkpoint + 录制 Prompt    |                                                    ██
PC   | Replay Bundle + CLI               |                                                       ██
M6   | 🏁 Replay 工具可用                 |                                                         █
```

**图例**：G=并行组，M=里程碑，PB=Phase B，PC=Phase C，PD=Phase D

### 详细三阶段路线图

#### 阶段 1：基础设施与观测（Day 1-5）

目标：建立观测能力（token 超限警告 + confidence 排序）和防线（compile gate 回归）。

| Day | 工作时 | 任务 | 负责人技能要求 |
|-----|--------|------|---------------|
| 1 | 6h | **TASK-001**: 实现 `EstimateTokens()` + 测试 | Go 后端 |
| 1 | 3h | **TASK-006**: Go compile gate 基础版本 | 全栈 |
| 1 | 1h | **TASK-002**: Token 预算常量定义 | 任何开发者 |
| 2 | 6h | **TASK-003**: Build 中注入 token 检查 + 日志记录 | Go 后端 |
| 2 | 4h | **TASK-007**: Node compile gate | 全栈 |
| 2 | 2h | **TASK-020**: Confidence 排序基础 + boundMemory 修改 | Go 后端 |
| 3 | 3h | **TASK-021**: Confidence 加权 BM25 评分 | Go 后端（检索经验） |
| 3 | 1h | **TASK-008**: Build.yml 注册 compile gate | 任何开发者 |
| 3-4 | 4h | 集成测试 + CI 配置 compile gate | 质量工程师 |
| 4-5 | 6h | **TASK-022**: Source 权重方案 | Go 后端 |
| 5 | 2h | 阶段 1 回归测试 + 修复 | 全部 |

**交付物**：
- ✅ `forge run` 在 prompt 超出模型 token 预算时发出警告
- ✅ `compile` gate 阻止语法错误的代码
- ✅ `boundMemory` 按 confidence × relevance 排序
- ✅ CI pipeline 包含 compile gate

#### 阶段 2：主动降级与衰减（Day 6-10）

目标：将观测转化为行动——token 超限时降级到更小模型，memory confidence 按时间和迭代衰减。

| Day | 工作时 | 任务 | 风险点 |
|-----|--------|------|--------|
| 6 | 4h | **TASK-004**: Token budget 降级逻辑 | R1：不准确估算导致过早降级 |
| 6-7 | 3h | **TASK-005**: 降级接入 tierResolver | 需要仔细测试与现有 `BudgetAdjustTier` 的交互 |
| 7-8 | 5h | **TASK-023**: 迭代衰减 + 时间衰减 | R6：衰减率需要校准 |
| 8-9 | 4h | **TASK-024**: Confidence 接入 Compact | 验证 compact 不会丢弃仍有价值的旧条目 |
| 9-10 | 4h | 阶段 2 集成测试 + 端到端回归 | 全部 |

**交付物**：
- ✅ prompt 超 budget 时自动降级到下一档更小模型
- ✅ memory confidence 随迭代衰减
- ✅ compact 按 confidence 保留最高价值的条目
- ✅ 所有降级决策在 trace 中记录

#### 阶段 3：Provider 路由 + Replay（Day 10-14，低优先级）

目标：扩展系统以支持多个 LLM 提供者；添加确定性重放能力。

| Day | 工作时 | 任务 | 备注 |
|-----|--------|------|------|
| 10 | 4h | **TASK-015**: Provider config + flag | Feature-flag 保护 |
| 10-11 | 4h | **TASK-016**: Provider 接入 claudeArgv | 核心风险 B1（无 API key） |
| 11-12 | 3h | **TASK-017**: 扩展 ModelMap（openai, google） | 纯数据添加，无代码风险 |
| 12 | 3h | **TASK-018**: Provider 感知等级解析 | 需要跨 provider 等级等价验证 |
| 12-13 | 3h | **TASK-019**: 集成测试 | 无 API key 时跳过 |
| 13 | 2h | **TASK-010**: Trace.Event + PromptText 字段 | 向后兼容 |
| 13 | 5h | **TASK-012**: 录制 prompt 文本 | 默认 `--trace-prompt` opt-in |
| 14 | 4h | **TASK-013**: Replay bundle 5 轮 | 成本估算已验证 5 轮~80KB |
| 14 | 3h | **TASK-014**: `forge replay` CLI | 用户命令可用 |

**交付物**：
- ✅ `--provider openai` 可用（实验性）
- ✅ `forge replay` 可重放过去 5 轮中的任何一轮
- ✅ 新 checkpoint 格式向后兼容

---

## 总体总结

| 维度 | 评估 |
|------|------|
| **总估算** | 方向一 13h + 方向二 8h + 方向五 15h +（方向三 13h + 方向四 15h 低优先级）= **Phase A+B 核心 36h（~5 开发日，2 开发者）** |
| **推荐执行顺序** | G1 并行 → G2 并行 → Phase B（降级）→ Phase D（provider）→ Phase C（replay） |
| **最大风险** | R2（self-reported confidence 不可靠）+ R3（provider 切换改变模型行为） |
| **关键约束** | 保持向后兼容（所有新字段 `omitempty`）；`opusFloorAgents` 不可降级；compile gate 必须考虑无 git 仓库的情况 |
| **推荐团队** | 2 Go 开发者 + 1 TL（审查所有降级安全性逻辑） |
| **快速收益** | compile gate（TASK-006→008）可在 3 天内上线，立即可防止无法编译的代码 |
