现在我掌握了全部上下文。以下是完整的 Tech Lead 分析报告。

---

# Tech Lead 分析报告：ForgeOS 生产级扩展方向

> **分析范围**: `expansion-production-perspectives.md`（5 个新方向）及其交叉验证评审 `expansion-production-perspectives.out.md`
> **交叉参考**: 已交付的 ROADMAP.md（5 方向已落地）、ARCHITECTURE.md、代码结构（forge-core/internal/）
> **分析日期**: 2026-07-12

---

## 0. 前置观察：执行的门槛条件

在分解任务之前，必须记录一个关键的**团队状态信号**：

| 指标 | 值 | 含义 |
|------|-----|------|
| `docs/requirements/` 文档数 | **400 份** | 分析产出严重过剩 |
| 其中 2026-07-10 ~ 07-12 产生 | ~50+ 份 | 三天内密集生成，主题高度重叠 |
| 原文档 (`expansion-production-perspectives.md`) 本身 | 第 5-6 轮迭代产物 | 已经是充分提炼后的结果 |
| 交叉验证 (`out.md`) | 3 处引用偏差 + 5 处盲点补充 | 修正很精准，但不改变核心论点 |

**团队当前最大的风险不是「遗漏了某个方向」，而是分析瘫痪**。400 份文档意味着每一份新文档的边际价值已经接近零。作为 Tech Lead，我的首要建议是**冻结分析阶段，进入执行阶段**。以下任务分解基于这个前提——每个任务都是 2-4 小时可完成的可执行单元，不再是分析提案。

---

## 1. 任务分解

### 1.1 方向 0：写时快照（架构分析建议的新增方向）

原 5 个方向均未涉及状态持久化的安全网。在实现任何涉及文件修改的新功能之前，应先建立回滚能力。

| 任务 ID | 标题 | 涉及文件 | 前置依赖 | 工时 | 验收标准 |
|---------|------|---------|---------|------|---------|
| TASK-000 | 定义 Phase 声明式写集/读集 Schema | `internal/asset/phase.go` | 无 | 2h | `Phase` struct 新增 `ReadFiles []string` / `WriteFiles []string` 字段；序列化/反序列化测试通过 |
| TASK-001 | 实现 phase 边界 git stash snapshot | `internal/orchestrator/orchestrator.go` | TASK-000 | 3h | phase 启动前对 `WriteFiles` 内文件执行 `git stash push`；phase 成功时 `git stash drop`；失败时 `git stash pop`；单元测试覆盖 stash/pop 路径 |
| TASK-002 | 为并行 wave 模式添加快照支持 | `internal/orchestrator/parallel.go` | TASK-001, TASK-000 | 2h | 并行 wave 中每个 phase 独立执行快照；wave 内一个 phase 失败不滚其他 phase |

**方向 0 总计: 7h (~1 人日)**

### 1.2 方向 1（P0）：多 Agent 冲突检测与操作排序

| 任务 ID | 标题 | 涉及文件 | 前置依赖 | 工时 | 验收标准 |
|---------|------|---------|---------|------|---------|
| TASK-010 | 构建干涉图数据结构 | `internal/orchestrator/interference.go`（新建） | TASK-000 | 4h | `InterferenceGraph` 支持构建 phase-phase 冲突边；`ConflictKind` 枚举（NoConflict/ReadWrite/WriteWrite）；单元测试覆盖 3 种冲突 |
| TASK-011 | 实现干涉图调度器 | `internal/orchestrator/scheduler.go`（新建） | TASK-010 | 4h | `ScheduleOrder()` 返回 `[]PhaseWave`；基于干涉图将冲突 phase 串行化；无冲突 phase 分入同一 wave；与现有 `parallel.go` WAVE 排序兼容 |
| TASK-012 | 声明式文件集注入 + 干涉图构建 | `internal/asset/workflow.go`, `internal/orchestrator/orchestrator.go` | TASK-010, TASK-011 | 3h | 每个 phase 启动前将其 `ReadFiles`/`WriteFiles` 注入干涉图；干涉图在 `Run`/`RunParallel` 入口处构建 |
| TASK-013 | 后验冲突检测（写后快照比对） | `internal/orchestrator/detect.go`（新建） | TASK-001 | 4h | phase 完成后对 `WriteFiles` 做 diff（对比 stash 前的状态 vs 完成后状态）；记录不匹配到 trace event |
| TASK-014 | LLM 辅助三路合并（可选） | `internal/orchestrator/merge.go`（新建） | TASK-013 | 4h | 检测到写-写冲突后，用 Haiku 生成合并提案；合并结果保存为 `.forge/merge-proposal/` 下供人工审查 |

**方向 1 总计: 19h (~2.5 人日)**

### 1.3 方向 2（P0）：实时可观测性与 Event Bus

| 任务 ID | 标题 | 涉及文件 | 前置依赖 | 工时 | 验收标准 |
|---------|------|---------|---------|------|---------|
| TASK-020 | Event Bus 核心接口 + 进程内实现 | `internal/bus/bus.go`（新建包） | 无 | 4h | `EventBus` 接口支持 `Publish` / `Subscribe` / `Unsubscribe`；非阻塞 Publish（满队列丢）；基于 Go `chan` 实现，零外部依赖 |
| TASK-021 | trace.Event 扩展 Payload 字段 | `internal/trace/trace.go` | 无 | 1h | `Event` 新增 `Phase string` / `Agent string` / `Payload []byte` 字段；向后兼容（omitempty）；序列化测试 |
| TASK-022 | Event Bus → trace.jsonl sink | `internal/bus/trace_sink.go`（新建） | TASK-020, TASK-021 | 2h | 订阅所有事件并写入 trace.jsonl；复用现有 `trace.Tracer` 的互斥锁保证 |
| TASK-023 | CLI 实时输出 --verbose / --tail | `cmd/forge/main.go`, `internal/bus/cli_sink.go`（新建） | TASK-020, TASK-021 | 4h | 新增 `--verbose` 标志；CLI sink 实时渲染 PhaseStarted/PhaseOutput/PhaseEnd 事件；进度条（已耗成本 + 预估剩余）；agent stdout 关键行提炼显示 |
| TASK-024 | CappedBuffer 改为 ring buffer + stream tee | `internal/orchestrator/command_executor.go` | TASK-020 | 3h | `cappedBuffer` 保留截断保护（上限 ~10MB）；同时通过 `EventBus.Publish` 转发原始输出流到总线；输出不翻倍（ring buffer 复用现有内存） |
| TASK-025 | 实时成本预估仪表 | `internal/bus/cost_dashboard.go`（新建） | TASK-020 | 2h | 订阅 `agent` 类事件；按 phase 已用量 + 历史单价估算剩余成本；显示在 CLI 进度条中 |

**方向 2 总计: 16h (~2 人日)**

### 1.4 方向 3（P1）：验证工厂

| 任务 ID | 标题 | 涉及文件 | 前置依赖 | 工时 | 验收标准 |
|---------|------|---------|---------|------|---------|
| TASK-030 | 声明式产出规格 Schema | `internal/asset/verification.go`（新建） | 无 | 2h | 在 agent card schema 中新增 `emits:` 验证规格字段：文件存在性/格式类型/必要 section 列表；Go struct + YAML 定义 |
| TASK-031 | 语法验证引擎（零 LLM 成本） | `internal/verify/syntactic.go`（新建包） | TASK-030 | 4h | 检查文件存在性、JSON/YAML 可解析、Markdown 必要 section 存在；纯 Go 标准库实现；`VerificationReport` 结构 |
| TASK-032 | 结构验证引擎（规则引擎 + AST） | `internal/verify/structural.go`（新建） | TASK-031 | 4h | 检查 ROADMAP 条目是否被代码改动覆盖（复用 `file_delta.go`）；ADR 是否包含架构决策段；go 代码改动是否对应测试文件；纯机械检查 |
| TASK-033 | 语义验证引擎（LLM-as-judge） | `internal/verify/semantic.go`（新建） | TASK-032 | 4h | 用最便宜模型评估 ADR/PRD 内容质量；结果写入 `VerificationReport`；遵守 `BudgetAdjustTier`（预算紧时跳过）；**咨询性，不阻断** |
| TASK-034 | 验证工厂注入 phase 生命周期 + 事件总线集成 | `internal/orchestrator/orchestrator.go`, `internal/bus/verify_sink.go` | TASK-031, TASK-020 | 3h | 验证工厂在 phase 完成后异步执行；验证报告通过 Event Bus 推送到 CLI + trace；写入 `phaseOutputLedger` 供下游 agent 消费 |

**方向 3 总计: 17h (~2 人日)**

### 1.5 方向 4（P1）：跨项目模式记忆

| 任务 ID | 标题 | 涉及文件 | 前置依赖 | 工时 | 验收标准 |
|---------|------|---------|---------|------|---------|
| TASK-040 | 组织知识库数据模型 + schema | `internal/memory/knowledge.go`（新建） | 无 | 3h | 定义 `Insight` 结构（content/type/confidence/effective_until）；`InsightType`（exact/heuristic）；序列化/反序列化测试 |
| TASK-041 | `forge learn push/pull` CLI | `cmd/forge/learn.go`（新建） | TASK-040, 现有 `cmd/forge/route.go` | 4h | `forge learn push` 将本地洞察推送到知识库 Git 仓；`forge learn pull` 拉取远程洞察到本地；采用 Git 做版本控制 + 分发 |
| TASK-042 | `forge init --org` 集成 | `cmd/forge/init.go` | TASK-041 | 3h | `--org` 标志从组织知识库拉取模式模板；覆盖默认 `examples/starter` 模板 |
| TASK-043 | 路由历史跨项目汇聚 | `internal/routing/cross_project.go`（新建） | TASK-041 | 4h | 聚合多个项目的 scorecard 数据；产生 `(task_type, model)` 评分表；写入知识库供其他项目消费 |
| TASK-044 | 洞察隐私过滤器 | `internal/memory/sanitizer.go`（新建） | TASK-040 | 2h | 默认拒绝列表（`API_KEY`/`SECRET`/`password` ENV 等）；模式级清理；默认不推送任何带项目名/路径的信息 |

**方向 4 总计: 16h (~2 人日)**

### 1.6 方向 5（P2）：预算治理与成本智能

| 任务 ID | 标题 | 涉及文件 | 前置依赖 | 工时 | 验收标准 |
|---------|------|---------|---------|------|---------|
| TASK-050 | `project.yml` budget schema | `internal/asset/project.go` | 无 | 2h | 定义 `Budget` 配置（monthly_usd/sprint_usd/hard_stop）；YAML 序列化；schema 验证 |
| TASK-051 | `forge cost report` CLI | `cmd/forge/cost.go` | TASK-050 | 3h | 从本地 `trace.jsonl` 聚合成本数据；按 phase/model/project 分组统计；输出表格 |
| TASK-052 | 运行中预算检查 + 告警 | `internal/orchestrator/budget.go` | TASK-050, TASK-020 | 4h | 每次 agent 调用后检查累计成本 vs 预算上限；超过 80% 告警（CLI warning）；hard_stop 在 run 起点检查（exit 1） |
| TASK-053 | 成本归因 + 历史追踪（复用已有 trace） | `internal/attribution/cost.go`（新建） | TASK-051 | 2h | 从 trace 已有的 `cost_usd_micros` + `model` 字段做归因；持久化到 `.forge/cost_history.json` |
| TASK-054 | 异常检测 + 优化建议规则引擎 | `internal/attribution/anomaly.go`（新建） | TASK-053 | 3h | 简单规则引擎（本周 cost/phase > 上周 2x → 告警；某 model 使用率下降档可替代 → 优化建议） |

**方向 5 总计: 14h (~1.75 人日)**

---

## 2. 执行顺序与依赖图

```
                       ┌─────────────────┐
                       │    Sprint N      │
                       │  (基础设施搭建)   │
                       └────────┬────────┘
                                │
              ┌─────────────────┼──────────────────┐
              ▼                 ▼                   ▼
     ┌────────────────┐ ┌────────────────┐ ┌────────────────┐
     │ TASK-000       │ │ TASK-020       │ │ TASK-021       │
     │ Phase 读/写集   │ │ Event Bus 核心  │ │ trace Event    │
     │ Schema         │ │ 接口+实现      │ │ 扩展 Payload   │
     └────────┬───────┘ └────────┬───────┘ └────────┬───────┘
              │                  │                   │
              ▼                  ▼                   │
     ┌────────────────┐ ┌────────────────┐           │
     │ TASK-001       │ │ TASK-022       │           │
     │ git stash      │ │ trace.jsonl    │           │
     │ snapshot       │ │ sink           │           │
     └────────┬───────┘ └────────┬───────┘           │
              │                  │                   │
              │                  ▼                   │
              │         ┌────────────────┐           │
              │         │ TASK-023       │           │
              │         │ CLI --verbose  │           │
              │         │ 实时输出       │           │
              │         └────────┬───────┘           │
              │                  │                   │
              │                  ▼                   ▼
              │         ┌────────────────┐ ┌────────────────┐
              │         │ TASK-024       │ │ TASK-025       │
              │         │ CappedBuffer→   │ │ 实时成本预估    │
              │         │ ring+tee       │ │               │
              │         └────────────────┘ └───────┬────────┘
              │                                    │
              ▼                                    │
     ┌────────────────┐                            │
     │ TASK-010       │                            │
     │ 干涉图数据     │◄────────────────────────────┘
     └────────┬───────┘      Event Bus 就绪后
              │              方向 1 才开始调度
              ▼              以利用可观测性调试
     ┌────────────────┐
     │ TASK-011       │
     │ 干涉图调度器    │
     └────────┬───────┘
              │
              ▼
     ┌────────────────┐  ┌────────────────┐
     │ TASK-012       │  │ TASK-013       │
     │ 文件集注入      │  │ 后验冲突检测    │
     └────────────────┘  └────────┬───────┘
                                  │
                                  ▼
                         ┌────────────────┐
                         │ TASK-014       │
                         │ LLM 三路合并   │
                         └────────────────┘

     ════════════════════════════════════════════════
            Sprint N+1    核心功能实现阶段
     ════════════════════════════════════════════════

     ┌────────────────┐  ┌────────────────┐  ┌────────────────┐
     │ TASK-030       │  │ TASK-050       │  │ TASK-040       │
     │ 验证规格 Schema │  │ budget schema  │  │ 知识库数据模型  │
     └────────┬───────┘  └────────┬───────┘  └────────┬───────┘
              │                   │                   │
              ▼                   ▼                   │
     ┌────────────────┐  ┌────────────────┐           │
     │ TASK-031       │  │ TASK-051       │           │
     │ 语法验证引擎    │  │ forge cost     │           │
     └────────┬───────┘  │ report CLI     │           │
              │          └────────┬───────┘           │
              ▼                   │                   ▼
     ┌────────────────┐          │           ┌────────────────┐
     │ TASK-032       │          │           │ TASK-041       │
     │ 结构验证引擎    │          │           │ forge learn    │
     └────────┬───────┘          │           │ push/pull      │
              │                  │           └────────┬───────┘
              ▼                  ▼                    │
     ┌────────────────┐  ┌────────────────┐          │
     │ TASK-033       │  │ TASK-052       │          ▼
     │ 语义验证引擎    │  │ 运行中预算检查  │  ┌────────────────┐
     └────────┬───────┘  └────────┬───────┘  │ TASK-042       │
              │                   │          │ forge init     │
              ▼                   ▼          │ --org          │
     ┌────────────────┐  ┌────────────────┐  └────────┬───────┘
     │ TASK-034       │  │ TASK-053       │           │
     │ 验证工厂注入    │  │ 成本归因/历史   │           ▼
     │ phase 生命周期  │  └────────┬───────┘  ┌────────────────┐
     └────────────────┘           │          │ TASK-043       │
                                  ▼          │ 跨项目汇聚      │
                         ┌────────────────┐  └────────┬───────┘
                         │ TASK-054       │           │
                         │ 异常检测/优化   │           ▼
                         └────────────────┘  ┌────────────────┐
                                             │ TASK-044       │
                                             │ 隐私过滤器      │
                                             └────────────────┘
```

```mermaid
graph TD
    %% Sprint N — 基础设施
    subgraph Sprint_N["Sprint N - 基础设施 (7人日)"]
        T000["TASK-000<br/>Phase 写集 Schema<br/>2h"]
        T020["TASK-020<br/>Event Bus 核心<br/>4h"]
        T021["TASK-021<br/>trace 扩展 Payload<br/>1h"]
        T001["TASK-001<br/>git stash snapshot<br/>3h"]
        T002["TASK-002<br/>并行 wave 快照<br/>2h"]
        T022["TASK-022<br/>trace.jsonl sink<br/>2h"]
        T023["TASK-023<br/>CLI --verbose<br/>4h"]
        T024["TASK-024<br/>ring buffer + tee<br/>3h"]
        T025["TASK-025<br/>实时成本预估<br/>2h"]
    end

    T000 --> T001
    T001 --> T002
    T020 --> T022
    T020 --> T023
    T020 --> T024
    T020 --> T025
    T021 --> T022
    T021 --> T023

    %% Sprint N+1 — 冲突检测
    subgraph Sprint_N1["Sprint N+1 - 核心功能 (13人日)"]
        T010["TASK-010<br/>干涉图数据结构<br/>4h"]
        T011["TASK-011<br/>干涉图调度器<br/>4h"]
        T012["TASK-012<br/>声明式文件集注入<br/>3h"]
        T013["TASK-013<br/>后验冲突检测<br/>4h"]
        T014["TASK-014<br/>LLM 三路合并<br/>4h"]
    end

    T000 --> T010
    T010 --> T011
    T011 --> T012
    T012 --> T013
    T013 --> T014
    T022 -.-> T013  <!-- Event Bus 用于调试 -->

    %% Sprint N+2 — 验证工厂 + 预算 + 记忆
    subgraph Sprint_N2["Sprint N+2 - 扩展功能 (40人日)"]
        T030["TASK-030<br/>验证规格 Schema<br/>2h"]
        T031["TASK-031<br/>语法验证引擎<br/>4h"]
        T032["TASK-032<br/>结构验证引擎<br/>4h"]
        T033["TASK-033<br/>语义验证引擎<br/>4h"]
        T034["TASK-034<br/>验证工厂注入<br/>3h"]
        
        T050["TASK-050<br/>budget schema<br/>2h"]
        T051["TASK-051<br/>forge cost report<br/>3h"]
        T052["TASK-052<br/>运行中预算检查<br/>4h"]
        T053["TASK-053<br/>成本归因/历史<br/>2h"]
        T054["TASK-054<br/>异常检测/优化<br/>3h"]
        
        T040["TASK-040<br/>知识库数据模型<br/>3h"]
        T041["TASK-041<br/>forge learn CLI<br/>4h"]
        T042["TASK-042<br/>forge init --org<br/>3h"]
        T043["TASK-043<br/>跨项目汇聚<br/>4h"]
        T044["TASK-044<br/>隐私过滤器<br/>2h"]
    end

    T023 --> T030
    T030 --> T031
    T031 --> T032
    T032 --> T033
    T034 --> T033
    T022 --> T034

    T000 --> T050
    T050 --> T051
    T051 --> T052
    T052 --> T053
    T053 --> T054
    T022 --> T054

    T040 --> T041
    T041 --> T042
    T041 --> T043
    T041 --> T044
    T043 --> T044
```

### 并行组

| 并行组 | 包含任务 | 理由 |
|--------|---------|------|
| **组 A** | TASK-000, TASK-020, TASK-021 | 三个独立的基础设施任务，代码变更区域完全不重叠（asset/trace/bus） |
| **组 B** | TASK-022, TASK-023, TASK-024, TASK-025 | 都基于 Event Bus，但互为独立 sink 实现 |
| **组 C** | TASK-031, TASK-050, TASK-040 | 三个方向的 Phase A 并行启动——验证引擎/budget schema/知识库模型互不依赖 |
| **组 D** | TASK-033, TASK-052, TASK-041 | 语义验证 + 预算检查 + forge learn CLI 可并行（但需各自 Phase A 就绪） |

---

## 3. 技术风险

### 风险矩阵

| # | 风险 | 概率 | 影响 | 方向 | 缓解措施 |
|---|------|------|------|------|---------|
| R1 | **Event Bus 成为性能瓶颈**：`Publish` 在高频 agent 输出下阻塞 phase 执行 | 低 | **高** | 2 | 对 `chan send` 使用 `select { case ch <- ev: default: /*drop*/ }`；所有 Publish O(1) 零分配；sink 失败不传播 |
| R2 | **干涉图调度器过于保守**：因写集重叠串行化过多 phase，抵消并行收益 | 中 | **中** | 1 | 从粗粒度文件级声明开始；用后验检测数据 feedback 调优；实现"乐观并行，冲突回退串行"启发式 |
| R3 | **git stash 在高频 phase 边界下性能崩坏**：100+ phase 的 run 产生 100+ stash | 中 | **中** | 0 | git stash 并非为秒级高频设计。进程内内存 blob 快照作为 fallback；仅对 agent phase 做（gate phase 不需要） |
| R4 | **语义验证的 LLM 成本吞噬预算** | 中 | **高** | 3 | 语义验证默认关闭（`--verify`）；预算紧张时自动跳过（`BudgetAdjustTier`）；只用 Haiku/最便宜模型 |
| R5 | **组织知识库泄露敏感信息** | 低 | **极高** | 4 | TASK-044 隐私过滤器是强制前置依赖；默认不推送任何数据；由 user 显式选择加入 |
| R6 | **验证工厂与 gate 产生验证结果冲突**：gate PASS 但验证 FAIL，造成用户困惑 | 中 | **中** | 3 | 验证工厂永远是 advisory（不阻断）；gate 永远是 authoritative；文档明确说明两者的关系 |
| R7 | **干涉图与 `parallel.go` 的 LOCK ORDER CONTRACT 冲突**：调度器改变了 phase 执行顺序，暴露未覆盖的锁顺序 | 低 | **高** | 1 | 干涉图调度器**不取代** LOCK ORDER CONTRACT，而是位于其**上层**；`-race` 测试覆盖所有新调度路径 |
| R8 | **`forge evolve` 2-3 iteration 中验证工厂延迟从未生效** | 高 | **低** | 3 | 这是该方向的已知诚实边界但非风险——短迭代中验证工厂确实不会触发。不影响架构决策，文档注明即可 |

### 最高风险项分析

**R5（机密泄露）** 是风险矩阵中唯一打"极高"影响的风险。方向 4 的跨项目记忆如果泄漏 API keys 或项目名到公开仓，会造成不可逆的品牌和信任损失。

**缓解方案（没有商量余地）**：
1. TASK-044 必须是 TASK-041 的硬前置依赖（`forge learn push` 之前必须经过隐私过滤器）
2. 隐私过滤器默认拒绝所有 ENV 名、路径名、文件名、域名、IP 的推送
3. 知识库仓建议默认私有（`.github/forgeos-knowledge` 的默认配置为 `private: true`）
4. 首次 `forge learn push` 必须打印警告并需要 `--ack-risk` 确认

---

## 4. 资源评估

### 4.1 人员技能矩阵

| 角色 | 所需技能 | 分配方向 | 人数 |
|------|---------|---------|------|
| **Go 运行时工程师** | Go 并发、接口设计、文件系统操作 | 方向 0/1/2 核心 | 1-2 人 |
| **可观测性工程师** | CLI 渲染、流式处理、事件驱动架构 | 方向 2 CLI sink + 成本仪表 | 1 人 |
| **验证/治理工程师** | YAML schema、规则引擎、Go/AST 解析 | 方向 3 | 1 人 |
| **Memory/CLI 工程师** | TF-IDF、git 操作、CLI 设计 | 方向 4 | 1 人 |
| **成本/分析工程师** | 聚合查询、启发式规则、CLI 报表 | 方向 5 | 0.5 人（可与方向 2/3 复用） |

**最小团队**: 2 人（1 Go + 1 全栈）可并行推进方向 2 + 方向 0
**推荐团队**: 3 人（2 Go + 1 可观测性/CLI）可并行推进 Sprint N 全部三个基础设施
**最大团队**: 4-5 人（含方向 4/5 专项）可全面铺开

### 4.2 关键里程碑

| 里程碑 | 时间 | 交付物 | 验证方式 |
|--------|------|--------|---------|
| **M1: Event Bus 可用** | Sprint N 结束 | `internal/bus/` 包全绿；`trace.jsonl` 沿用现有格式 + 扩展字段；`--verbose` 可显示实时输出 | `go test ./internal/bus/...` + 手工 `forge run --verbose` |
| **M2: 写时快照可用** | Sprint N 结束 | 所有 agent phase 边界执行 git stash；失败 phase 文件自动回滚 | 手动制造 phase 失败验证回滚 |
| **M3: 干涉图调度器可用** | Sprint N+1 结束 | `RunParallel` 使用干涉图调度；冲突 phase 被串行化 | `go test -race` + 冲突注入测试 |
| **M4: 验证工厂可用** | Sprint N+2 第 1 周 | 语法+结构验证在 phase 后执行；验证报告出现在 trace 和 CLI | `forge run --verify` 验证 ADR/ROADMAP |
| **M5: 预算治理可用** | Sprint N+2 第 2 周 | `forge cost report` 产生分项报表；运行中预算超过 80% 告警 | 手工运行并检查告警触发 |
| **M6: 跨项目记忆可用** | Sprint N+2 第 2 周 | `forge learn push/pull` 可推送和拉取洞察 | 双项目测试：项目 A push → 项目 B pull 可见 |

### 4.3 阻塞点与解决策略

| 阻塞点 | 描述 | 解阻塞策略 | 责任方 |
|--------|------|-----------|--------|
| B1 | 方向 1 的干涉图调度器需要方向 2 的 Event Bus 来调试 | 方向 2 Phase A（Event Bus 核心）提前到 Sprint N；方向 1 Phase A 在 Sprint N+1 | 团队 Lead |
| B2 | git stash 在 Windows 下的兼容性 | TASK-001 专注于 Unix（项目已知运行在 Linux/macOS）；Windows 持后补 | Go 工程师 |
| B3 | `forge learn` 需要外部 Git 仓，但组织可能没有统一的 Git 基础设施 | 默认使用 `github.com/org/forgeos-knowledge` 仓名；支持通过 env `FORGEOS_KNOWLEDGE_REPO` 自定义 | 团队 Lead 决策 |
| B4 | 语义验证（TASK-033）需要 LLM 调用，但 forge-core 零外部依赖承诺 | 语义验证作为可选项（`--verify` flag），默认关闭；其 `go.mod` 保持零新增依赖（用现有 executor 做 LLM 调用） | 架构师/Lead |

---

## 5. 质量保证

### 5.1 单元测试覆盖要求

| 包 | 行覆盖率目标 | 关键测试场景 |
|----|------------|-------------|
| `internal/bus/` | ≥90% | Publish 非阻塞（queue full drop）；Subscribe/Unsubscribe 生命周期；并发 Publish 不 panic；sink 失败不传播 |
| `internal/orchestrator/interference.go` | ≥95% | 3 种 ConflictKind 正确判定；空/单 phase 边界；大 phase 图构建性能基线 |
| `internal/orchestrator/scheduler.go` | ≥95% | 冲突串行化正确性；无冲突 phase 全并行；混合冲突图拓扑排序 |
| `internal/orchestrator/detect.go` | ≥90% | diff 正确性；无变更 phase pass；变更 phase 正确报告 |
| `internal/verify/*.go` | ≥85% | 语法验证（文件不存在/格式错误/缺少 section）；结构验证（ROADMAP-代码关联）；语义验证 mock LLM 返回 |
| `internal/attribution/*.go` | ≥80% | 成本归因正确性；异常检测（阈值触发/不触发） |

### 5.2 集成测试策略

| 测试类型 | 描述 | 触发条件 |
|---------|------|---------|
| **`-race` 全包测试** | 所有涉及并发的新包必须通过 `go test -race` | CI 每次提交 |
| **冲突注入集成测试** | 两个 agent phase 声明写同一文件 → 干涉图正确串行化 | Sprint N+1 交付前 |
| **Event Bus 压力测试** | 100 goroutine 同时 Publish，验证无阻塞无 panic | Sprint N 交付前 |
| **`forge run --verbose` E2E** | 端到端运行 echo agent，验证 CLI 实时输出 | Sprint N 交付前 |
| **`forge cost report` 数据正确性** | 手工构造已知成本的 trace.jsonl，验证报表数字一致 | Sprint N+2 交付前 |

### 5.3 代码审查要点

| 审查领域 | 审查重点 | 审查者角色 |
|---------|---------|-----------|
| **并发安全** | Event Bus `Publish` 非阻塞保证；干涉图调度器锁顺序（不违反 `parallel.go` 的 LOCK ORDER CONTRACT）；所有共享变量的 mutex 覆盖 | Go 运行时工程师 |
| **接口设计** | Event Bus 接口最小化；不暴露不必要的内部状态；sink 接口可测试性 | Tech Lead |
| **零外部依赖** | `go.mod` 无新 require；无 CGO；无网络调用（知识库 Git 仓的 `git` 是外部命令调用，非 Go 依赖） | 架构师 |
| **向后兼容** | trace.Event 新增字段使用 `omitempty`；现有 trace.jsonl 消费工具不受影响；`project.yml` 新字段为可选 | Tech Lead |
| **错误处理** | 所有 `Publish` 错误被 sink 内部处理，不传播到 phase 执行路径；验证工厂失败不阻断 phase；预算告警不阻断执行（除非 hard_stop） | 团队成员交叉审查 |

### 5.4 性能测试需求

| 测试 | 场景 | 通过标准 |
|------|------|---------|
| Event Bus 吞吐 | 1000 msg/s Publish，3 sink 订阅 | Publish 延迟 < 100μs p99；无 goroutine 泄露 |
| 干涉图构建 | 100 phase 的 workflow，平均每 phase 5 文件 | 构建时间 < 50ms |
| git stash 开销 | 100 文件变更的 phase | stash + diff + pop 总耗时 < 500ms |
| `forge cost report` | 10000 event 的 trace.jsonl | 聚合耗时 < 1s |

---

## 6. 实施计划（详细时间表）

### 阶段 1：基础设施搭建（Sprint N · 5 个工作日 · ~7 人日）

**目标**: Event Bus 可用 + 写时快照就绪 + trace 扩展

| 天 | 任务 | 交付物 | 负责人 |
|----|------|--------|--------|
| Day 1 | TASK-000（Phase 写集 Schema）+ TASK-021（trace 扩展）+ TASK-020 接口定义 | asset/phase.go 修改；trace.go 扩展；bus 包接口 | Go 工程师 A |
| Day 2 | TASK-020 实现（chan-based event bus）+ TASK-022（trace.jsonl sink） | bus.go 完整实现 + trace_sink.go | Go 工程师 A |
| Day 3 | TASK-023（CLI --verbose）+ TASK-024（ring buffer + tee） | cli_sink.go + command_executor.go 修改 | 可观测性工程师 |
| Day 4 | TASK-001（git stash snapshot）+ TASK-025（实时成本预估） | orchestrator.go 快照逻辑 + cost_dashboard.go | Go 工程师 B |
| Day 5 | **集成测试 + 性能基线**：`go test -race ./internal/bus/...` 全绿；E2E `forge run --verbose` 手工验证；性能测试数据记录 | 测试报告 + 性能基线 | 团队全体 |

**验证标准**:
- [ ] `go test -race ./internal/bus/...` 全绿
- [ ] `forge run --verbose` 显示 phase 级实时输出 + 进度 + 预算消耗
- [ ] 手动制造 phase 失败，验证 git stash pop 回滚
- [ ] trace.jsonl 包含新的 Phase/Agent/Payload 字段（向后兼容）

### 阶段 2：核心功能实现（Sprint N+1 · 5 个工作日 · ~13 人日）

**目标**: 干涉图调度器就绪 + 后验冲突检测

| 天 | 任务 | 交付物 | 负责人 |
|----|------|--------|--------|
| Day 1 | TASK-010（干涉图数据结构 + 3 种冲突类型） | interference.go + 单元测试 | Go 工程师 A |
| Day 2 | TASK-011（干涉图调度器 + PhaseWave 排序） | scheduler.go + `parallel.go` 集成 | Go 工程师 A |
| Day 3 | TASK-012（声明式文件集注入干涉图） | orchestrator.go Run/RunParallel 入口注入 | Go 工程师 B |
| Day 4 | TASK-013（后验冲突检测：写后快照比对） | detect.go + Event Bus 集成（验证报告流） | Go 工程师 B |
| Day 5 | TASK-014（可选）LLM 三路合并 + **集成测试** | merge.go；冲突注入 E2E 测试；`go test -race` 全绿 | 团队全体 |

**验证标准**:
- [ ] 两个 phase 声明写同一文件 →干涉图正确串行化
- [ ] 无冲突 phase 保持并行（wave 对比前后吞吐无退化）
- [ ] 后验冲突检测在 phase 完成后 200ms 内产生结果
- [ ] `-race` 测试通过，LOCK ORDER CONTRACT 未违反

### 阶段 3：扩展功能实现（Sprint N+2 · 10 个工作日 · ~40 人日）

**目标**: 验证工厂 + 预算治理 + 跨项目记忆（三个方向并行）

| 周 | 方向 3（验证工厂） | 方向 5（预算治理） | 方向 4（跨项目记忆） |
|----|--------------------|--------------------|--------------------|
| **Week 1** | TASK-030（验证规格 Schema）→ TASK-031（语法验证引擎） | TASK-050（budget schema）→ TASK-051（forge cost report） | TASK-040（知识库数据模型）→ TASK-044（隐私过滤器） |
| **Week 2** | TASK-032（结构验证引擎）→ TASK-034（验证工厂注入 phase 生命周期） | TASK-052（运行中预算检查+告警）→ TASK-053（成本归因+历史追踪） | TASK-041（forge learn push/pull）→ TASK-042（forge init --org） |
| **Week 3-4** | TASK-033（语义验证引擎）→ 集成测试 | TASK-054（异常检测+优化建议）→ 集成测试 | TASK-043（跨项目汇聚）→ 集成测试 |

**注意**: 三个方向在 Week 1-2 可完全并行（团队拆分 3 人或 2 人串行），Week 3-4 各自深化后统一集成测试。

**验证标准**:
- [ ] 语法验证：缺失文件/格式错误/缺少 section 均被正确检测（100% 机械检查）
- [ ] `forge cost report` 输出与手工计算一致
- [ ] 项目 A `forge learn push` → 项目 B `forge learn pull` 可见洞察
- [ ] 运行中超过 80% 预算 → CLI warning 显示

### 阶段 4：集成测试 + 优化 + 上线（Sprint N+3 · 5 个工作日）

**目标**: 全量集成测试 + 性能调优 + 文档 + 上线

| 天 | 任务 | 交付物 |
|----|------|--------|
| Day 1 | **全量集成测试**：五个方向的交叉集成测试（Event Bus × 干涉图 × 验证工厂 × 预算治理 × 记忆） | 集成测试报告 |
| Day 2 | **`-race` 全量 + 性能调优**：解决并发问题；性能回归基线 | `-race` 全绿报告；性能基线数据 |
| Day 3 | **文档更新**：更新 `.agent/` 下的 AGENTS.md/PROJECT.md/ARCHITECTURE.md；新增 `docs/event-bus.md`/`docs/conflict-detection.md`/`docs/budget-governance.md` | 文档 PR |
| Day 4 | **边界情况处理**：验证工厂在 `--verify` 关闭时的空走路径；ghost phase（gate-only 无 agent）不触发快照；预算 hard_stop `exit 1` 路径 | 边界测试用例覆盖 |
| Day 5 | **上线前审查 + `forge accept`**：完整 Stop 闸门（gate.mjs + arch-check + check.py + secret-scan + test + app-test）；上线清单签署 | 上线清单 ✅ |

---

## 7. 汇总：投入产出分析

| 方向 | 总工时 | 人日 | 优先级 | 为什么这个优先级 |
|------|--------|------|--------|----------------|
| **方向 0**（写时快照） | 7h | ~1 | **前置条件** | 是所有方向的安全网；只有 1 人日，杠杆极高 |
| **方向 2**（Event Bus） | 16h | ~2 | **P0** | 解耦可观测性；是方向 3/4/5 的前置依赖；方向 1 的调试依赖 |
| **方向 1**（冲突检测） | 19h | ~2.5 | **P0** | 生产 multi-agent 硬要求；但依赖方向 2 的 Event Bus 调试 |
| **方向 3**（验证工厂） | 17h | ~2 | **P1** | 降低对 agent 自报告的依赖；可选语义验证受成本控制 |
| **方向 4**（跨项目记忆） | 16h | ~2 | **P1** | 团队 >5 人时启动；与方向 5 可并行 |
| **方向 5**（预算治理） | 14h | ~1.75 | **P2** | 组织级运营时才需要；单人开发用 `--run-budget-usd` 已够 |
| **集成+上线** | ~20h | ~2.5 | — | 必需的收尾阶段 |

**总计**: ~109 工时 ≈ 14 人日 ≈ **3 个 Sprint（含集成）**

---

## 8. 对团队的决策建议

### 8.1 执行路径选择

| 情景 | 建议路径 | 说明 |
|------|---------|------|
| **1 人开发** | 方向 0 → 方向 2 Phase A → 方向 1 Phase A → （按需选方向 3/5） | 单人无法并行，串行走优先级最高的三个方向 |
| **2 人团队** | 方向 0 + 方向 2 并行 → 方向 1 + 方向 3 并行 → 方向 4 + 方向 5 并行 | 2 人可完美拆分基础设施和核心功能 |
| **3 人团队（推荐）** | 上述三阶段按并行组拆分 | 核心团队 3 人可覆盖所有方向 |

### 8.2 快速出击选项（「最小可行生产级」）

如果团队只有 **2 个 Sprint** 的时间，只能做 3 件事：

```
必须做（没有这些不上生产）:
  1. 方向 2 Phase A（Event Bus + CLI --verbose）— 可观测性是心理安全底线
  2. 方向 0（写时快照）— 回滚安全网，仅 1 人日

强烈推荐:
  3. 方向 1 Phase A（干涉图调度器）— 并行安全，仅 2.5 人日
```

这三个方向合计约 42 工时 / 5.25 人日，2 人并行可在 **1.5 Sprint** 内交付。

### 8.3 停止分析，开始执行

作为 Tech Lead，我的核心建议：

```
┏━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━┓
┃   立即停止生成新的分析文档。                          ┃
┃   docs/requirements/ 已有 400 份文档。               ┃
┃   `expansion-production-perspectives.md`            ┃
┃   已经充分覆盖了下一阶段的方向。                      ┃
┃   Last commit wins. Start coding.                   ┃
┗━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━┛
```

---

## 附录 A：关键接口草案（供实现参考）

### A.1 Event Bus 接口

```go
package bus

import (
    "context"
    "time"
)

type EventKind string

const (
    PhaseStarted EventKind = "phase_started"
    PhaseOutput  EventKind = "phase_output"
    PhaseEnd     EventKind = "phase_end"
    GateResult   EventKind = "gate_result"
    VerifyResult EventKind = "verify_result"
    CostWarning  EventKind = "cost_warning"
)

type Event struct {
    Kind         EventKind
    Name         string   // phase/gate/agent name
    Phase        string   // phase ID（新增字段，omitempty for non-phase events）
    Agent        string   // agent ID（新增字段，omitempty for non-agent events）
    Timestamp    time.Time
    DurationMs   int64
    CostUsdMicros int64
    Model        string
    Detail       string
    Payload      []byte   // agent stdout / heavy payload（可 nil）
}

type EventFilter struct {
    Kinds  []EventKind
    Phases []string
    Agents []string
}

type SubscriptionID uint64

type EventBus interface {
    Publish(ctx context.Context, ev Event) error   // 必须非阻塞
    Subscribe(filter EventFilter, h EventHandler) SubscriptionID
    Unsubscribe(id SubscriptionID)
}

type EventHandler func(ctx context.Context, ev Event) error
```

### A.2 干涉图接口

```go
package orchestrator

type ConflictKind uint8

const (
    NoConflict        ConflictKind = iota // 无冲突，可并行
    ReadWriteConflict                     // 一个读一个写，可并行但需读前快照
    WriteWriteConflict                    // 都写同一文件，必须串行化
)

type PhaseID string

type PhaseWave struct {
    Phases       []PhaseID
    ConflictFree bool   // wave 内无写-写冲突
}

type InterferenceGraph struct {
    edges map[PhaseID]map[PhaseID]ConflictKind
}

func NewInterferenceGraph(phases []asset.Phase) (*InterferenceGraph, error)
func (g *InterferenceGraph) Conflict(a, b PhaseID) ConflictKind
func (g *InterferenceGraph) ScheduleOrder() ([]PhaseWave, error)
```

### A.3 验证工厂接口

```go
package verify

type CheckLevel int

const (
    Syntactic  CheckLevel = iota // 零 LLM 成本，纯机械
    Structural                    // 规则引擎 + AST
    Semantic                      // LLM-as-judge，可选
)

type CheckResult struct {
    CheckName string
    Level     CheckLevel
    Passed    bool
    Detail    string
}

type VerificationReport struct {
    PhaseID string
    Verdict string   // "pass" | "fail" | "advisory"
    Checks  []CheckResult
}

type VerificationFactory struct {
    syntactic  SyntacticChecker
    structural StructuralChecker
    semantic   SemanticChecker
}

func (vf *VerificationFactory) Verify(ctx context.Context, phase asset.Phase, artifacts []Artifact) VerificationReport
```

---

**结论**: `expansion-production-perspectives.md` 及交叉验证已经完整定义了下一阶段的 5 个方向。分析阶段结束，执行阶段开始。以上任务分解、依赖图和实施计划可以直接作为 Sprint 计划的基础。建议团队在下一个 Sprint 规划会中使用本分析作为起点，分配前 1-2 个 Sprint 的任务。
