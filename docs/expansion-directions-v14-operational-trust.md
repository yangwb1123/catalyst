# ForgeOS 高价值扩展方向分析 v14 — 运行信任与生产就绪

> **视角**: 资深架构师 / 产品经理  
> **方法**: 全局代码库深扫（forge-core 13 内部包 + cmd/forge 18+ CLI 命令 + harness 26+ 模块 + `.agent/` 完整治理骨架 + 27+ 份已有 docs/analysis/ 交叉核对）  
> **基线**: Sprint 26-27（真点火 multi-agent 端到端坐实、Learning loop 三维真数据、parallel 模式完整交付含锁顺序契约、四维资源安全护栏、signal handling 进行中）  
> **纪律**: 绝不与任何已有 27+ 份分析文档的核心论点重叠  
> **日期**: 2026-07-01

---

## 已有分析未覆盖的方向（确认清单）

以下方向已被深入覆盖，本文**绝不重复**：

| 已有覆盖域 | 对应文档 |
|---|---|
| 自适应工作流 / 信号驱动编排 | `high-value-extensions.md` 方向一 |
| 闸门自省 / 元学习 | `high-value-extensions.md` 方向二 |
| Phase 级文件系统隔离 / Git worktree | `novel-directions-v13.md` 方向一 |
| 配置面模式校验引擎 | `novel-directions-v13.md` 方向二 |
| 跨相位故障归因链路 | `novel-directions-v13.md` 方向三 |
| HTTP API / 执行器多样性 | `novel-directions-v13.md` 方向四 |
| 预算规划器 / 成本推演 | `novel-directions-v13.md` 方向五 / `novel-extensions-v12.md` 方向五 |
| 跨周期收敛状态机 | `expansion-core-five-2026-07-01.md` 方向一 |
| 自愈运行时 | `expansion-directions-v6-novel-perspectives.md` 方向四 |
| 确定性 Replay / 调试引擎 | `expansion-gaps-v7-novel.md` 方向三 |
| Memory 衰减 / 去重 / 数据生命周期 | `high-value-perspectives-v11.md` 方向四 / `fresh-scan-strategic-expansion.md` 方向一 |
| 收敛理论陷阱 / 并行 fail-fast | `edgecases-and-perf.md` §1.1 / §3 |
| 配置表面积 / 跨文件一致性 | `configuration-surface-and-adoption.md` |
| ADR 架构决策衰退审计 | `eighth-wave-adr-decay.md` |
| 交互式工作流 / 可暂停观察 | `five-extensions-v10-distinct.md` 方向一 |
| 检查点 Diff / 收敛回归浏览器 | `five-extensions-v10-distinct.md` 方向四 |
| 跨 Agent Prompt 注入防护 | `expansion-directions-v6-novel-perspectives.md` 方向一 |
| 架构度量趋势分析 / 早期预警 | `expansion-directions-v6-novel-perspectives.md` 方向五 |
| Workflow 版本化 / 灰度 / Rollback | `strategic-expansion-and-edge-cases.md` 方向 E |
| 长运行时资源泄漏 / 文件系统压力 | `edgecases-and-perf.md` §2 |
| 锁顺序契约 / 并行安全 | `edgecases-and-perf.md` §1.3 / `parallel.go` header |
| YAML-Shim 消除 / Go-Native Asset | `fresh-scan-strategic-expansion.md` 方向二 |
| 架构漂移诊断 / 修复 Pipeline | `expansion-analysis-v2.md` 方向一 |
| Git-native Rollback / Safe-Base | `expansion-analysis-v2.md` 方向二 |
| 多仓组合治理 / 组织控制面 | `expansion-high-value-directions.md` 方向二 |
| ForgeOS 自我测试 / Dogfood 缺口 | `self-testing-and-dogfooding.md` |

---

## 三个全新方向

经过全网扫描与交叉核对，以下方向在全部 27+ 份分析中**无核心重叠**。

---

## 方向一 · Trace 完整性链与闸门产出可验证证明

### 优先级: P1 — 生产信任基础设施

### 一句话

当前 trace.jsonl 是明文追加日志，**任何人都可以静默修改** trace 事件、删掉失败的 gate 记录、伪造成本数据。对于无人值守自治系统要可信地部署到生产，trace 必须可验证、不可篡改。

### 现状 (代码证据)

```
grep -rn "crypto\|sha\|hash\|sign\|attest\|hmac\|digest" forge-core/ --type go
→ 空 (零结果)
```

trace 写入 (`trace.Tracer.Emit`) 是纯 JSON 序列化 + `f.Write`，无 hash、无签名、无完整性保护：

| 文件 | 行 | 证据 |
|---|---|---|
| `internal/trace/trace.go` | 56-80 | `Event` 结构体只有业务字段，无 `hash_prev`、`signature` 等链式字段 |
| `internal/trace/trace.go` | 103-135 | `Emit` 序列化 JSON → 写入文件，无摘要计算 |
| `cmd/forge/scorecard_wind.go` | 全文件 | 回读 trace 时按字段解析，不做完整性验证 |
| `internal/persist/checkpoint.go` | 全文件 | Checkpoint JSON 无签名，恢复时不解签 |

Scorecard 的 `traceHasModelCost` 读取 trace 文件时，**无法检测**该文件是否被篡改。一个发生安全事故后的取证场景：trace 显示「所有 gate 全绿 → ACCEPTED」，但 trace 本身可能是伪造的。

### 为什么现在不做会限制下一量级

1. **合规基线**: 金融/医疗/受监管行业的审计要求「不可否认的操作记录」。若 ForgeOS 目标是 **AI 软件工厂**，它的产出（代码、部署决策）必须有可验证的来源（provenance）。
2. **生产事故归因**: 24h 无人值守运行时，若第 8 小时 agent 注入破坏性代码，trace 必须能无可辩驳地证明「这个 agent 在这个时间执行了这个命令」。篡改 trace = 毁灭证据。
3. **自治系统的信任根**: 一个能自己改代码的系统，也必须能自己证明它没有篡改审计日志。这是「谁审计审计者」问题的工程版本。

### 设计要点 (框架级，非代码实现)

**链式哈希**:
- 每个 `trace.Event` 增加 `HashPrev string` 字段 = 前一个事件的 SHA-256 摘要
- 写入时：`hash = SHA256(prevHash + json(event_without_hashprev))`，然后写 `HashPrev=prevHash, ...` + 自己的 hash
- 验证时：从头遍历，重算每个 hash 链，断裂即告警

**Gate 产出签名**:
- 每个 gate 执行器（`acceptance-kernel.mjs` 的 `Result`）增加 `Result.Signature` 字段
- harness gate 执行完一轮后，用自己的密钥对 `(gate_name, verdict, timestamp, output_hash)` 签名
- acceptance.mjs 聚合时收集所有签名，作为 `GateProof` 的一部分注入 converge 信号
- 验证者（后期 dashboard / audit 工具）用已知的公钥验证

**轻量级方案 (v1)**:
```
trace hash chain:
  Event[0]: {kind:"iteration", seq:1, hash_prev:"", hash:"abc123"}
  Event[1]: {kind:"gate", seq:2, hash_prev:"abc123", hash:"def456"}
  Event[2]: {kind:"agent", seq:3, hash_prev:"def456", hash:"ghi789"}

验证: 读到 Event[2] → 计算 SHA256(Event[2].hash_prev + body) == "ghi789"? → 验证 Event[1] → ...
```

**成本**:
- 每写入一次 trace event 增加一次 SHA-256（~1μs，相对 agent phase 的秒级耗时可忽略）
- 每个 event 增加 ~48 bytes（hash_prev 的 hex 编码）
- 不影响 run 的 exit code（hash 链断裂不阻断 run，仅产生 WARNING）

### 边界情况

| 情况 | 处理 |
|---|---|
| 首次写入（prev=空） | Event[0] 的 HashPrev = ""，hash = SHA256(json(body)) |
| 并发写入（parallel 模式） | 每个 goroutine 独立 Emit，hash 链保持顺序（Seq 保证全局序），但需要一个「上一个 hash」的原子读-算-写。可简化：parallel 下用 `sync/atomic` 的 `lastHash` 指针 |
| 文件截断/拼接 | 哈希链检测到断裂：报告 `trace integrity break at seq N`，记录为新的 doctor event |
| 合法的 checkpoint 恢复 | 新 iteration 追加到已有 trace 后，hash 链自动延续（读取最后一个事件 hash 作为 prev） |
| 性能 | SHA-256 纯 Go 实现 ~100MB/s；trace event 平均 <200 bytes，每一事件 ~1μs |

---

## 方向二 · 事件驱动触发层：Webhook / Cron / Git Hook 集成

### 优先级: P1 — 生产管道集成

### 一句话

当前的 `forge run` / `forge evolve` 是**手动启动**的 CLI 命令。ForgeOS 需要成为**被动触发的服务**——当 PR 合并时自动 `forge evolve build`，每早 9 点跑安全扫描，新版本发布后自动回归。

### 现状 (代码证据)

```
grep -rn "webhook\|trigger\|hook.*\|cron\|schedule\|event.*driven\|subscript" forge-core/ --type go
→ 空 (零结果)
```

所有入口均为 CLI：
- `cmdRun` / `cmdEvolve` / `cmdMigrate` — 全部从 `os.Args` 读取命令
- `execEngine` / `execLoop` — 同步执行、同步返回
- 无后台守护模式、无 HTTP 监听、无事件消费者

| 文件 | 行 | 证据 |
|---|---|---|
| `cmd/forge/main.go` | 66-99 | `run()` 是同步 CLI dispatcher，无服务器模式 |
| `cmd/forge/evolve.go` | 全文件 | `execLoop` 同步阻塞直到收敛/tripwire |
| `harness/scaffold/forge-init.mjs` | 全文件 | 生成的 CI `.github/workflows/forge.yml` 也只能通过 CLI 交互 |
| 整个代码库 | — | 无 HTTP/gRPC listener，无 message queue consumer |

### 为什么现在不做会限制下一量级

1. **CI/CD 集成是瓶颈**: 今天的 `forge evolve build` 在 PR merge 后自动触发（CI 里跑 `forge run build --executor command`），但 CI 作业有时间限制（GitHub Actions 6h），而一个真正的 evolve loop 可能跑 12-24h。需要异步触发模式：CI 触发 → ForgeOS 接受请求 → 后台执行 → 结果回调到 CI。
2. **事件驱动 = 管道自动化**: 真实软件交付不是「一个命令完成所有」，而是「PR merge → 构建 → 测试 → 预发布验证 → 金丝雀 → 全量发布」的事件链。每个阶段是独立触发条件，ForgeOS 需要能监听事件（webhook → 触发 run → 完成 → 触发下一个事件）。
3. **无人值守的天花板**: 当前 24h 自治的前提是人启动了 run。真正的「软件工厂」应该由外部事件（git push、cron、launchdarkly flag toggle）触发，不需要人 SSH 进去敲命令。

### 设计要点 (框架级)

**三层触发架构**:

```
触发源                          ForgeOS 调度器                 执行器
─────────                     ─────────────                ──────
Webhook (GitHub/GitLab)  →    HTTP Listener               Sandbox/Agent
Cron (定时)              →    Job Queue (内存/v2 → NATS)  Executor Pool
Git Hook (pre-push)     →    Rate Limiter + Auth          ...

每个触发 → 创建一个 RunRequest {workflow, params, callback}
RunRequest 入队 → 调度器分配 executor → 异步执行 → 结果回调
```

**v1 方案最小可行**: `forge serve` 命令 = 轻量 HTTP server：
- `POST /run` — 提交一个 workflow 执行请求，返回 `run_id`
- `GET /run/:id` — 轮询状态
- `GET /run/:id/trace` — 实时 trace 流 (SSE)
- 集成 webhook auth：`--webhook-secret` 验证 GitHub/GitLab 请求

**CI 集成协议**:
- `forge run build --wait` = 当前行为（同步等待）
- `forge run build --async --webhook-callback https://ci.example.com/job/123/done` = 异步触发
- CI 中的 workflow 变成:
  ```yaml
  - name: 触发 ForgeOS 构建
    run: forge run build --async --webhook-callback "${{ github.event.workflow_run.url }}"
  ```

### 边界情况

| 情况 | 处理 |
|---|---|
| 多个触发同时到达 | v1 顺序队列 + `--concurrency 1`，v2 用 job queue + worker pool |
| Webhook 认证 | `--webhook-secret`（HMAC-SHA256 验证 payload signature） |
| 回调失败 | 指数退避重试最多 3 次，然后 fallback 到 trace 中记录 `callback_failed` |
| Cron 表达式 | v1 简化：`--cron "0 9 * * 1"`（每周一 9am），用标准 cron 解析库 |
| 幂等性 | 每个 webhook 请求携带 `X-Idempotency-Key`，重复请求不重复执行 |
| 外部依赖 | 纯 Go 标准库可做 HTTP server（`net/http`），cron 表达式解析可用社区库 |

---

## 方向三 · ForgeOS 运行时自监控与 Day-2 运维命令

### 优先级: P2 — 长运行时可靠性

### 一句话

当前 `forge doctor` 和 `forge preflight` 检查的是**运行前环境**。运行中 forge 对自己的健康状态一无所知：trace 文件多大？memory 条目数？checkpoint 是否过期？没有「运行时健康检查」和「运维清理」命令。

### 现状 (代码证据)

现有健康相关命令：
- `forge preflight` — 跑前环境检查（CLI 存在、workflow 可解析、安全护栏就绪）
- `forge doctor` — 运行前诊断（stale checkpoint、trace truncation、tmp residue）

但运行中（尤其是 24h evolve loop），**没有任何自检**：

| 文件 | 行 | 证据 |
|---|---|---|
| `cmd/forge/preflight.go` | 全文件 | 只跑一次，不持续监控 |
| `cmd/forge/validate.go` | 全文件 | `forge validate` 是离线的 trace/memory 验证工具 |
| `internal/trace/trace.go` | 103-135 | `Emit` 无文件大小/行数追踪 |
| `internal/memory/memory.go` | 全文件 | `Append` 无限增长，无容量上限 |
| `internal/persist/checkpoint.go` | 全文件 | 只读/写，不检查 checkpoint age 是否合理 |

`forge validate` 虽然能离线检查 trace 和 memory，但它：
1. 不是运行时检查（需要停掉 evolve 才能跑）
2. 只检查格式正确性，不做语义检查（"这个 trace 的 seq 是否连续？"、"memory entry 数量是否合理？"）
3. 没有阈值告警——「trace 文件已 500MB」不阻断 run，也不报警

### 为什么现在不做会限制下一量级

1. **24h+ 运行的稳定性**: 一个 evolve loop 跑 48 小时后，trace.jsonl 可能达到数百 MB，memory.jsonl 数千条条目，checkpoint 文件可能因为某个 bug 被反复写入产生膨胀。没有运行时监控，这些故障只在「磁盘满了」时暴露为 OOM 或崩溃。
2. **运维盲区**: 生产环境部署 ForgeOS 后，运维人员需要 `forge status` 查看运行中实例的健康状态。当前连「forge 是否在运行」都无法通过 CLI 远程查询。
3. **升级风险**: 从 v2 到 v3 升级时，旧的 trace/memory/checkpoint 格式可能与新版不兼容。没有健康检查，升级后跑不起来才暴露问题。

### 方向分解

#### 3a. 运行时 Tracker (轻量级自监控)

在每个 `execEngine` / `execLoop` 中启动一个 goroutine，每 5 分钟检查一次：

```go
// 伪代码 — 设计方向示意，不实现
type RuntimeHealth struct {
    TraceSize       int64     // trace.jsonl 当前字节数
    MemoryEntries   int       // memory.jsonl 条目数
    CheckpointAge   time.Duration // 距上次 checkpoint 写入
    PhaseCount      int       // 已运行 phase 数
    IterationCount  int       // 已运行 iteration 数
    LastGateResult  string    // 最后 gate 状态（ok/FAILED/N/A）
    DiskUsage       float64   // .forge/ 目录占用
}
```

这些指标在每次 `OnIteration` / `OnPhase` 钩子中采样，记录为新的 `trace.Event.Kind = "health"` event。当超过阈值（`trace.jsonl > 100MB`）时，记录 WARNING 但不阻断 run。

#### 3b. `forge status` 命令

读取 `.forge/` 目录（无需运行），输出人类可读的健康报告：

```
$ forge status
forge runtime status:
  workflow:     build (iteration 7/12)
  state:        RUNNING (last activity 2m ago)
  trace:        12.4 MB, 2,847 events
  memory:       143 entries
  checkpoint:   3m old, iteration 7 phase 4/8
  gates:        test PASS · complexity PASS · lint N/A · secret PASS
  cost:         $3.42 (cumulative), $0.18/phase avg
  budget:       $10.00 (34% used)
```

已有数据全部可读（trace 扫描 + checkpoint 解析 + scorecard 回读），**只读操作，不修改任何状态**。

#### 3c. `forge cleanup` 命令

安全的运维清理：
- `forge cleanup --trace-keep-last 5` — 保留最近 5 条 iteration 的 trace，之前的内容轮转到 `.forge/trace.archive.1.jsonl`
- `forge cleanup --memory-merge` — 合并重复 memory entries（去重 key、保留最新值）
- `forge cleanup --checkpoint-stale-age 24h` — 删除超过 24h 的旧 checkpoint 文件
- `forge cleanup --all` — 清理所有可丢弃的运行时产物

### 边界情况

| 情况 | 处理 |
|---|---|
| cleanup 时 evolve 正在运行 | cleanup 只读/只归档非活动文件（trace 当前打开的文件不动），建议在 stop 后运行 |
| trace 文件锁 | Open trace 文件在 Unix 下可同时读写（O_APPEND），cleanup 读取 stats 不需要锁 |
| 误删 | `forge cleanup --dry-run`（默认！）打印计划操作不执行；`--confirm` 才真正执行 |
| checkpoint 过新 | `forge cleanup` 的默认阈值保守（24h），防止活跃 loop 被误清理 |

---

## 方向四 · Agent 能力感知自适应提示

### 优先级: P2 — 模型利用率

### 一句话

今天所有 agent 获得**同样结构的 prompt**，无论它跑在 Haiku（$0.0003/千 token）还是 Opus（$0.015/千 token）上。一个 Haiku 做文档更新拿到的是和 Opus 做架构设计一样长的 prompt——浪费了 50x 的成本差异带来的优化机会。

### 现状 (代码证据)

`prompt.Build` 和 `prompt.Gather` 不感知模型 tier：

| 文件 | 行 | 证据 |
|---|---|---|
| `internal/prompt/prompt.go` | 25-41 | `Build(agent, phase, mode, tier, card, ctx)` — tier 只出现在「你是…tier=%s」字符串中，不改变 prompt 结构 |
| `internal/prompt/prompt.go` | 66-116 | `Gather` — 固定三 lane：task + constraints + ADRs，无 tier 分支 |
| `cmd/forge/engine_build.go` | 53-64 | `buildPrompt(o.root, p, mode, tierOf, ctxCache, gates, phaseOut, findings)` — tier 传到 prompt 但只做身份声明 |
| `cmd/forge/prompt_context.go` | 全文件 | `context()`/`findingsContext()`/`verdictContext()` — 所有 context 块长度与 tier 无关 |

一个 Haiku 跑 `docs` 任务获得一个完整的 `feeds_forward` phase output + 6 个 ADR + AGENTS.md 约束 + gate ledger + findings ledger 的完整 prompt。而 Opus 跑 `architect` 设计任务获得**同样的 context 配比**。

理论上：
- Haiku 适合简短、确定性的任务（文档、简单 CRUD、测试补全），context 窗窄、注意力弱
- Opus 适合长上下文、深度推理（架构、安全、跨模块分析），需要更多 context 但也能消化

当前策略没有利用这个不对称性。

### 为什么现在不做会限制下一量级

1. **成本结构不对称**: 跨厂商池（v3）将引入成本差异更大的模型（DeepSeek 比 Opus 便宜 30x）。若不按能力调整 prompt 复杂度，低成本模型会因「需要处理过多无关 context」而实际效果低于预期，导致路由系统倾向于选贵模型——成本优势完全丧失。
2. **Haiku 的 token 预算**: Haiku 的 context window 默认比 Opus 小（128K vs 200K）。今天不给 tier 分支，未来当 ADR 积累到 50+ 时，`adrTopK = 6` 是固定值——对于 Haiku 太大（浪费），对于 Opus 可能太小（不够用）。
3. **质量可预测性**: 同样的 prompt，Haiku 和 Opus 的输出质量差距在不同任务上不同。对于简单的格式转换，差距小；对于架构决策，差距大。如果没有能力感知适配，路由系统无法准确预测「给 Haiku 这个任务能到什么质量」——记分卡的 (model, task_type) 二维数据会因 prompt 相同而高估/低估模型真实能力。

### 设计要点 (框架级)

**Tier 感知的 prompt 装配**:

```go
// 伪代码 — 设计方向示意
func BuildAdaptive(agent, phase, mode, tier string, card string, ctx []string) string {
    switch tier {
    case "haiku":
        // 轻量级 prompt：只有核心 role card + 最相关的 1-2 个 ADR
        // 跳过 feeds_forward 的冗长输出，用摘要替代
        // 跳过 AGENTS.md 的完整约束列表，只注入红线条款
        return buildHaikuPrompt(agent, phase, mode, card, compressContext(ctx, maxTokens=2000))
    case "sonnet":
        // 中等 prompt：完整 role card + top-4 ADR + AGENTS 红线 + gate ledger
        // feeds_forward 输出保留但摘要到 phaseOutputSummaryCap
        return buildSonnetPrompt(agent, phase, mode, card, compressContext(ctx, maxTokens=8000))
    case "opus":
        // 完整 prompt：所有 context 保留原样（当前行为）
        return buildOpusPrompt(agent, phase, mode, card, ctx) // = 现有的 Build
    }
}
```

**三条具体适配规则**:

1. **Context 注入量**: Haiku 的 context 注入量应为 Opus 的 25%（短 task + 2 个最相关 ADR + AGENTS 红线摘要），Sonnet 为 60%
2. **结构化输出要求**: Haiku 要求「只输出代码，不要解释」；Opus 要求「输出架构分析 + 理由 + 代码」
3. **Task 复杂度适配**: Haiku 获得拆解为极小子任务的 prompt（「改这一行」）；Opus 获得高阶抽象 prompt（「重新设计这个模块的接口」）

**记分卡校准**:
- 当 `windDownScorecards` 写入记分卡时，附加 `prompt_version` 标签
- 不同 prompt 版本下同一 model 的 quality_score 分开统计
- 路由层（`HistoryTiebreak`）按 `(model, task_type, prompt_version)` 三维择优，而非二维

### 边界情况

| 情况 | 处理 |
|---|---|
| 用户显式 `--model opus` + Haiku prompt | 不允许降级（prompt 复杂度按实际 tier，而非手动覆盖的 model） |
| ADR 数量很少（<6） | 所有 tier 都注入全部 ADR（因为总量很小，「压缩」没有意义） |
| 自定义 `--agent-allowed-tools` | 自适应 prompt 需保留所有注入的自定义工具列表（不因 tier 压缩而丢失） |
| 跨厂商池（v3）：不同厂商的 Haiku 级别模型 | 质量不同：Google Gemini 2.0 Flash vs Claude Haiku 3.5 vs Qwen 2.5-7B → 需要 **per-provider 的 prompt 适配**，不仅是 per-tier |
| backward compat | 现有 buildPrompt 签名不变；`BuildAdaptive` 作为可选升级路径，默认行为 = Opus 级别（完整注入），零改动 |

---

## 总结优先级矩阵

| 方向 | 影响范围 | 实现量级 | 前置依赖 | 与 v3 的关系 |
|---|---|---|---|---|
| ① Trace 完整性链 | 审计/合规/安全取证 | ~200 Go loC + 架构设计 | 无 | 独立；v3 沙箱可直接集成 |
| ② Webhook 触发层 | CI/CD 集成/无人值守自动化 | ~500 Go loC + HTTP server | 方向③（健康监控）弱依赖 | 独立；v3 分布式的基础 |
| ③ 运行时自监控 | 运维/长运行可靠性 | ~300 Go loC + 文档 | 无 | 独立；v3 分布式后更关键 |
| ④ 能力感知提示 | 成本优化/模型利用率 | ~150 Go loC + prompt 模板调整 | 跨厂商池（v3）弱依赖 | 跨厂商池的关键配套 |

**每方向独立的理由**: 四个方向没有阻塞依赖关系，可以独立推进、独立交付、独立验证。每个方向都解决一个「今天不做，到 v3 的某个阶段会系统性受限」的问题，而非镀金功能。
