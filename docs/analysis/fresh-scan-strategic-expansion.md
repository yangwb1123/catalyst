# ForgeOS — 全局扫描后的战略扩展方向分析

> 扫描日期: 2026-07-01
> 扫描范围: forge-core (13 Go 包) · harness (gate/accept/arch/secret/SCA/scorecard) · workflows · 所有存量分析文档
> 视角: 资深架构师/产品经理,基于「已有代码」判断「增量最高杠杆」
> 规则: 不编写代码,只做判断。不重复已有 ROADMAP/docs 内容,聚焦代码扫描揭示的新盲区。

---

## 前置:代码库现状速览

| 维度 | 当前状态 |
|---|---|
| forge-core | 13 Go 包,纯 stdlib 零依赖,CLI `forge run/evolve/gate/check/accept/route/migrate/detect/validate/scorecard/memory-prune/status` |
| 中枢旋钮 | mode×lifecycle 完整驱动 Router + Harness + Workflow 深度 + migration |
| 真点火 | 真 claude 多 agent 已端到端坐实(Sprint 24-26,八个真实 gap 已修) |
| 安全护栏 | 四维完整:递归深度/agent 总量/输出内存/美元封顶 |
| 并行执行 | 存在但 opt-in:无 fail-fast / 无 loop-back / 无 per-phase checkpoint |
| 持久化 | checkpoint(JSON) + memory(JSONL 追加) + trace(JSONL 追加),全无旋转/压缩 |
| 资产加载 | 经 `yaml2json.py` Python shim —— 唯一外部语言依赖 |
| 模型池 | Claude-only,跨厂商池占位(v3) |
| 风险推断 | 规则驱动,非自动提取(需显式 flag) |
| 上下文检索 | 纯关键词 TF-lite,非语义 |
| 审计溯源 | trace 记录事件但无完整 prompt/context 快照 |

---

## 方向一:长运行时数据生命周期管理 (Knowledge Lifecycle Engine)

### 代码扫描发现

`forge-core/internal/memory/memory.go` (568 行) 和 `forge-core/internal/trace/trace.go` (207 行) 都是**纯追加、无旋转、无压缩、无 TTL**的 JSONL 存储。`.forge/` 目录下的三个持久化文件:

| 文件 | 格式 | 增长模式 | 压缩 | TTL | 旋转 |
|---|---|---|---|---|---|
| `trace.jsonl` | JSONL 追加 | 每次 phase/gate/converge 一行 | 无 | 无 | 无(仅有 `.1` 备份) |
| `memory.jsonl` | JSONL 追加 | 每次 lesson/decision 一行 | 无 | 无 | 无 |
| `checkpoint.json` | JSON 覆写 | 每次迭代/phase 覆盖 | N/A | N/A | N/A |

### 为什么需要

1. **24h 自治运行的磁盘爆炸**:假设每 10 秒产生一条 trace event(保守估计),24h = 8,640 行,每行 ~500 bytes ≈ 4.3MB。但真实场景:每 agent phase(30s) + 每 gate(2s) + 每收敛检查(500ms),并行 N phase → 线性增长。30 天不间断运行 → 数百 MB→GB 级 trace 文件,无旋转机制意味着 `forge doctor` 和 scorecard 读全文件 O(n)。

2. **Memory 膨胀稀释 prompt 质量**:当前 `prompt.Build` 将 memory entries 注入 agent prompt。当 memory 有 5,000 条 lessons 时,即使 Retrieve 做 top-K,注入窗口也是巨大浪费。当前无:
   - **重要性评分**:所有 lesson 平等,无法区分"关键架构决策"vs"琐碎观察"
   - **去重**:同一 gap 在 N 次迭代中被反复记录
   - **过期淘汰**:30 天前的 lesson 可能已不相关但仍被检索

3. **Checkpoint 无 schema 版本兼容**:`persist.Checkpoint.FormatVersion` 字段存在(`"forgeos.checkpoint.v1"`)但 loader 读旧格式后不做迁移。当 forge-core 升级到 v3 改了 checkpoint 结构 → 旧 checkpoint 静默解析失败 → 恢复路径断裂。

### 建议扩展

- **Memory 分级(短期/长期)**:短期(当前 run)→ 内存 map + 定期 flush;长期(跨 run)→ JSONL 文件 + 索引。引入 compaction 操作:合并相似条目、移除过期条目、重写稀疏文件。
- **重要性衰减**:已有 `recencyHalfLifeDays` 模式(scorecard),扩展到 memory:每个 lesson 带 decay 权重,Query 按权重排序,低权重自动淘汰。
- **Trace Rotation & 压缩**:JSONL 按大小/时间自动旋转(`trace.jsonl.1` → `trace.jsonl.2.gz`);scorecard 聚合时只读最近 N 个旋转段。
- **Checkpoint 版本迁移器**:`persist.Migrate(oldPath, newPath, fromVersion, toVersion)` 接口,启动时自动检测格式版本并迁移。

---

## 方向二:YAML-Shim 消除与 Go-Native Asset Pipeline

### 代码扫描发现

`forge-core/internal/yaml2json/yaml2json.go` (755 行,forge-core 最大文件) 是整个 forge-core 的**临界依赖单点**:

```
Python shim (harness/yaml2json.py)
        ↓ stdout pipe
yaml2json.go (Go 端反序列化)
        ↓ encoding/json
asset.Workflow (Go struct)
```

调用链路:每个 `forge run/evolve` 启动 → `yaml2json.Transcode` → `exec.Command("python3", "harness/yaml2json.py", ...)` → shell 出(读取 `.agent/workflows/*.yml`) → JSON 写 stdout → Go 端 ParseWorkflow。

该文件还有一行注释暴露了 BUG 历史: `seq = append(seq, val) // BUG FIX: was missing — simple scalar item never appended` —— Python↔Go 管道的手工序列化容易出错。

### 为什么需要

1. **运行时依赖**:forge-core 是"纯 Go 标准库、零外部依赖",但每个 CLI 调用都隐式依赖 `python3` 在 PATH 上且 `PyYAML` 可用。这与项目"零外部依赖"的宣称矛盾。

2. **性能**:每个 `forge run/evolve` 多一次进程 fork+exec(~20-50ms) + Python 解释器启动(~30-100ms) + YAML 解析(~5-20ms) = 每次 CLI 调用 ~55-170ms 固定开销,完全与 workflow 复杂度无关。

3. **错误处理断裂**:Python 端的 YAML 语法错误 → Go 端收到空 JSON → `ParseWorkflow` 输出来自 `decoder.Decode` 的泛泛错误(`"unexpected end of JSON input"`)。用户看到的是"JSON 解析失败",而非"workflow YAML 第 42 行有语法错误"。

4. **Go 生态已有成熟 YAML 库**:`gopkg.in/yaml.v3` 是 Go 标准信号,被广泛使用,不会引入传递依赖问题。架构中"零外部依赖"约束是 v0 设计决策,当 forge-core 已有 13 个 Go 包后,权衡需要重新审视。

### 建议扩展

- **引入 `gopkg.in/yaml.v3` 直接解析 YAML**:消除 Python shim,消除 `yaml2json.go`(或大幅缩减为纯 schema 校验)。
- **内联 schema 校验**:当前 `harness/check.py` 校验 YAML schema(悬挂引用等),移至 Go 侧,用 `encoding/json` 的 `json.Unmarshal` + 自定义 `json.Decoder.DisallowUnknownFields` + 补充校验。
- **丰富错误消息**:直接输出 `line X: unknown field "phse"` 而非 `JSON parse error`。

---

## 方向三:自适应风险推断引擎 (从声明到自动发现)

### 代码扫描发现

`forge-core/internal/risk/risk.go` 的 honesty 注释明确:

```go
// HONESTY — what this is and is NOT:
//   - This is a RULE-BASED classifier: it maps DECLARED feature flags to a level
//   - The features themselves are taken as EXPLICIT INPUT
//   - AUTOMATIC feature extraction — parsing a git diff to decide which files
//     changed ... is downstream wiring and is OUT OF SCOPE here
```

当前用户/编排器必须显式设置 `Signals.TouchesPayment`/`TouchesAuth`/`TouchesSecrets`/`ProdTraffic`/`BlastRadius`。`risk.FromChangedPaths` (risk_diff.go) 尝试从文件路径启发式推断,但只读路径名、不读内容、不分析调用图。这意味着:

- 如果 implementer 的改动涉及支付模块但用户没传 `--touches-payment` → 风险评级为 low → Router 选 Haiku → 关键代码被最便宜的模型处理。
- BlastRadius 基于**改动文件数**,而非**真正的调用依赖图扇出**——一个改了核心抽象层的文件可影响 50 个下游,但被算为 1 个文件(低风险)。
- Secret-scan 在 gate 阶段才跑(事后),而非 routing 阶段(事前)做风险预判。

### 为什么需要

安全下限(`risk==critical → Opus`)是系统的最高优先级规则,但**它的输入源是残缺的**。没有自动风险发现:
- 安全闸门只能在 agent 跑完后检测 secret→无法阻止 agent 在低档模型上写涉密代码
- 路由的 `safety_override` 只在"你告诉系统有风险"时才生效
- 真点火场景:agent 被 prompt 诱导去改 auth 代码 → Haiku 处理 → 引入漏洞 → gate 很可能过(测试绿) → 生产事故

### 建议扩展

- **Git-Diff 感知的风险特征提取器**:解析改动文件的 import graph,计算真实扇出(非文件计数);检测 import 了 `payment/`/`auth/`/`secret/` 等敏感包的文件;标记涉及 schema migration 的 SQL/DDL 语句。
- **事前 secret 扫描**:routing 阶段对改动文件做快速 secret 扫描(正则匹配),而非等到 gate 阶段。
- **风险聚合器**:`Risk.Infer(changedFiles []string, imports map[string][]string) Signals`——纯函数,输入改动列表+依赖图,输出完整的 Signals,无需人工标记。
- **Honesty 护栏**:自动推断只抬高风险(不压低),`--risk low` 可覆盖但必须有日志告警,且 `critical` 永不从自动推断降级。

---

## 方向四:确定性运行回放与可审计持久化 (Deterministic Replay & Audit Trail)

### 代码扫描发现

`forge-core/internal/trace/trace.go` 记录了事件序列,但**不保存每个 phase 的完整输入**(prompt + context + routing 决策)。`forge-core/internal/persist/checkpoint.go` 保存的是 resume 用最小状态(iteration/phase index),而非审计用全快照。

当前,如果一个生产环境运行产出了错误代码,你能回答的问题是:
- 哪些 gate 过了/没过? (trace → gate event)
- 用了哪个模型? (trace → agent event)
- 花了多少钱? (trace → cost_usd)

但你**不能**回答:
- 这个 phase 的完整 prompt 是什么?(含 context/ADRs/GateLedger 注入)
- Router 为什么选了这个 tier?(多维打分细节 vs 最终决策)
- 如果换成 Opus 而非 Sonnet,结果会不同吗?
- 这个 agent 的 output 是否与注入的 context 一致?(防 hallucination 审计)

### 为什么需要

1. **合规审计(SOC 2 / HIPAA / PCI)**:在受监管环境,你需要证明"LLM 产出的每一行代码,其输入上下文和路由决策都是可追溯的"。当前系统没有这种证明力。
2. **调试归因**:当 agent 产出了错误代码,无法分辨是 prompt 不对、context 不全、还是模型本身的问题。没有完整快照,只能盲猜。
3. **"What-If" 分析**:这是 v3 Router 学习闭环的核心输入:"如果我在这个 phase 用了 model X 而非 Y,结果会更差还是更好?"没有 replay 能力,无法做离线对比实验。
4. **Prompt 泄露检测**:当 agent 产出了包含敏感上下文的代码(如把 AGENTS.md 注入到产物中),需要事后追溯是哪个 phase 的 prompt 包含了该内容。

### 建议扩展

- **Per-Phase Snapshots**:每个 agent phase 执行前,将完整 prompt + context 快照写入 `.forge/snapshots/<iteration>-<phase>.json`(可选,`--audit` flag 开启,因为默认开启的 IO 开销显著)。
- **`forge replay <trace-id>` 命令**:读取 trace.jsonl + snapshot dir,重放一个 run 的决策序列(但跳过真 agent 调用),输出"如果 x 不同会怎样"的分析报告。
- **Hash Chain**:每条 trace 事件记录前一个事件的 SHA256,形成不可篡改的链。`forge audit verify` 验证链完整性。
- **Scorecard 自动填充**:replay 模式下,给定同一 prompt 但不同 model tier,自动产生对比评分,填充 scorecard 的 `quality_score` 维度(目前该维度恒 N/A)。

---

## 方向五:并行编排生产化与多 Agent 协调模式 (Parallel Orchestration Productionization)

### 代码扫描发现

`forge-core/internal/orchestrator/parallel.go` (137 行) 实现了 opt-in 并行执行,但文档明确标注了三条限制:

```
// SCOPE / HONESTY (v1):
//   - NO directed loop-back.
//   - NO per-phase checkpoint.
//   - LOAD-BEARING lock-order contract (8 mutex levels)
```

同步,`loop.go` 的 `LoopEngine` 与 `RunParallel` 共享同一 `Engine`,但 LoopEngine 未并行化 —— evolve 循环内每个 iteration 仍然是串行的。

`exec_error.go` 定义了 5 种 ExecKind,但 **overload backoff** 是简单固定等待,非自适应(无 exponential backoff + jitter)。

`engine_build.go` 中 `Build` 函数(324 行)的 agentExecutor 选择是串行的——每个 phase 逐个构建 prompt -> spawn claude -> 等返回 -> 下一个。

多 agent 协调当前只有"前传"(feeds_forward):planner 的输出 → implementer 的输入。没有:
- 并发 agent 之间的通信
- 任务分解后的并行实现(每个 implementer 做不同文件)
- 结果合并/冲突解决

### 为什么需要

1. **端到端耗时瓶颈**:串行 5-phase build(plan→impl→gate→review→qa) × 多轮 loop-back × 迭代 = 1 个 roadmap item 可能耗时 30-60 分钟(真实 claude 调用)。并行能显著压缩:
   - 多个 implementer 同时实现不同文件(当依赖图允许)
   - gate 检查与 reviewer 评审部分并行(复杂度算分 vs 函数长度检查)
   - multiple roadmap items 并行推进(fork/join)

2. **并行模式的脆弱的锁契约**:8 级锁顺序是**文档约束**非编译器约束。没有 `go vet` 规则能验证开发者是否正确遵循了锁顺序。一个不小心 → Heisenbug 死锁,只在生产高负载下重现。

3. **Fail-Fast 缺失**:如已存在分析(`docs/analysis/edgecases-and-perf.md`)指出,当前并行波内一个 phase fail 不会取消同一波内其他 phase——浪费计费时间(一次波可浪费 $2+)。

4. **LoopEngine 与 Parallel 的坐标**:evolve loop 中 scan/gap-analysis/roadmap-update 是完全可并行的(三者无依赖),但当前是串行的。实现"并行迭代"可大幅压缩 evolve cycle 时间。

### 建议扩展

- **并发安全锁顺序静态验证**:增加 `tools/lockcheck` 工具(或 harness 闸门),解析 `sync.Mutex` 获取顺序,验证所有代码路径遵守 `trace → runBudget → loopProbe → gateLedger → phaseOutputLedger → ContextCache → reviewFindingsLedger → verdictLedger` 顺序。
- **Fail-Fast 波取消**:引入 `errgroup.Group` 替代裸 `sync.WaitGroup`,第一个 failure 取消波内所有剩余 phase 的 context。需注意 CommandExecutor 的 `cappedBuffer` 在 `cmd.Cancel` 后的行为(当前可能留下 dangling read)。
- **自适应 Backoff**:`backoff.go` 的 backoff 策略从固定等待→exponential backoff with jitter(基值 1s,倍率 2x,上限 60s,±25% jitter),响应 API overload(529)而非固定等待。
- **Evolve Loop 的并行扫描**:`evolve.yml` 的 scan 与 gap-analysis 之间插入 `fork_join` 并行段——同时跑代码扫描 / 依赖扫描 / secret 扫描 / 性能扫描,结果合并后进 gap-analysis。

---

## 总结:优先级矩阵

| 方向 | 价值 | 实现成本 | 风险降低 | 性能影响 | 推荐优先级 |
|---|---|---|---|---|---|
| ① 知识生命周期管理 | ★★★★ | ★★(增量) | ★★★ | ★★★★★(磁盘/内存) | **高** |
| ② YAML-Shim 消除 | ★★★ | ★(低)yaml.v3成熟 | ★(消除单点) | ★★★(每次 CLI 节省~100ms) | **高** |
| ③ 自适应风险推断 | ★★★★★ | ★★★(调用图分析) | ★★★★★(安全底线) | ★(纯计算) | **最高** |
| ④ 确定性回放与审计 | ★★★★ | ★★★(快照 IO) | ★★★★(合规/调试) | ★★(可选,默认关闭) | **中**(合规需求驱动) |
| ⑤ 并行编排生产化 | ★★★★★ | ★★★★★(锁/死锁/竞态) | ★★★(并发 Bug) | ★★★★★(端到端耗时) | **高**(量变到质变) |

> **最高建议**:从 ③ 自适应风险推断开始——它填补了系统最高优先级规则(安全下限)的输入空白,且纯计算无外部依赖,是"最小改动最大安全收益"。其次是 ① 知识生命周期管理——这是 24h 自治运行的必备前提(不做好,跑 48h 后就退化了)。
