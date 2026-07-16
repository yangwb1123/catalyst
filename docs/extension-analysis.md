# ForgeOS 扩展方向分析

> 基于截至 2026-07-01 全局代码扫描的架构级审阅。
> 范围: `forge-core/` (13 Go pkg) · `cmd/forge` (CLI) · `harness/` (闸门框架) · `.agent/` (声明式资产)
> 角色: 资深架构师 × 产品经理

---

## 全局现状摘要

| 维度 | 状态 |
|------|------|
| 核心运行时 | ✅ 13 包全绿,纯 Go 零依赖 |
| CLI 子命令 | ✅ run / evolve / gate / check / accept / route / migrate / detect / scorecard / validate / doctor / approve — 12 个 |
| 中枢旋钮 | ✅ mode×lifecycle → Router + Harness + Workflow 深度 三维完整 |
| 学习闭环 | ✅ trace + memory + scorecard + converge 框架就绪,真 claude 成本/延迟数据已坐实 |
| 安全护栏 | ✅ 递归深度 + agent 调用总量 + 输出内存 + 超时 + 美元封顶 — 五维完整 |
| 真点火 | ✅ multi-agent → converge MET 已端到端坐实(8 个真 gap 已修) |

**核心发现**:ForgeOS 的「编排骨架」已非常健壮,但**面向真实企业级 AI 软件工厂的"血肉"仍有系统性空白**。以下五个方向按**ROI × 紧迫性**排序。

---

## 方向一:跨厂商模型池 + 学习闭环真接通

### 为什么需要它

当前 `routing.ModelMap` 只含 `anthropic` 一家 (`claude-sonnet-4-haiku / claude-sonnet-4 / claude-opus-4`),`routing.TierForScore` → `routing.HistoryTiebreak` → 到 `Eval → scorecard → Router 回灌` 的管线在代码中**完整存在但仅限单厂商内**。真 claude 跑出的成本/延迟/质量三维数据已落盘到 `scorecards.json`,但 Router 当前并不真正用它做多厂商择优——它只能在三个 Claude 档位间"折中选择",无法回答一个根本问题:**同一任务用 GPT-4o 是否比 Opus 更便宜且同样好?**

### 现状缺口

- `ModelMap` 硬编码,无厂商注册/发现机制
- `ResolveModel` 向 CLI 吐出的是模型名,但 CLI `--executor=command` 只执行了一个 `agent-cmd`,没有「根据 tier 选择不同 provider」的编排
- `LiteLLM` 集成在 ROADMAP 中标注为 v3,但 scorecard + trace + cost + latency 管道已备齐,**插上多厂商就是完整的 MLOps-for-LLM 闭环**,拖延等于浪费现有基础设施

### 影响

1. **成本浪费**:目前所有 work 走同一厂商,无法在质量容忍的任务(文档/测试/gate 裁决概要)上自动换用更便宜的模型
2. **供应商锁定**:不谈多厂商备份机制,单厂商挂了 -> 整个软件工厂停摆
3. **学习闭环空转**:scorecard 记录了 `(model, task_type, avg_cost_usd, p95_latency_ms, quality_score)`,但除了单厂商内 Haiku↔Sonnet↔Opus 之外没有消费者

### 建议路线

1. **ModelMap → ProviderRegistry**:`routing` 下加 `Provider` 接口(provider 名 · 模型列举 · tier→model 映射 · cost-per-token 表),`ModelMap` 变成注册表而非 map literal
2. **Executor 层多路**:CommandExecutor 的 `Build` 改造为接收 `(phase, mode, resolvedProvider)` → 输出对应 CLI 调用(今天 `claude -p`,明天 `litellm --model gpt-4o -p`);用 `--provider-override` 或 `TierForScore` 的扩展字段选择
3. **Scorecard→Router 喂入上线**:`routing.HistoryTiebreak` 目前是 `forge route` 可观测但**不在 CLI `--model` 选择管线中真实消费**(用于显示而非决策)。改 `phaseTierResolver` 在非 safety-floor agent 上让 scorecard 质量分真正影响 tier 选择
4. **Fallback 链**:主厂商超载 529 → 自动 fallback 备厂商同 tier,不降级

---

## 方向二:Agent-Runtime 引擎——从「Prompt 发射器」到「工具执行运行时」

### 为什么需要它

当前 `forge run` 的实际 agent 执行路径是:

```
CLI → Build(prompt string) → CommandExecutor(exec.Command(claude -p "prompt"))
```

这是一个直通式的「发射并遗忘」。Agent 没有自己的"运行时":没有工具调用管理器、没有对话状态机、没有任务计划的持久化、没有从失败中智能恢复(只有计数重试和 kill)。这与 `for` 的 `loopEngine` 的健壮性(检查点/断点续传/收敛检测)形成鲜明对比——agent 层是整个链条中最弱的一环。

真点火暴露的根本问题:agent 自己不会管理工具调用(acceptEdits 无 Bash 就无法 self-verify,依赖注入 gate 裁决)。这不是授不授权的问题,是 agent 没有"工具运行时"来编排 `claude` 的能力。

### 现状缺口

- `CommandExecutor.Observe` 是输出后回调——agent 已经运行完毕,你只能接收结果,无法干预其中
- 没有 `ToolExecutor` 层:Bash/文件编辑/代码搜索 等工具的策略(from agent 还是从 host?)未定义
- `RunAgentPhase` 的 retry 逻辑只有"重试 == 重新执行整个 prompt",没有"智能恢复"(换策略/换工具集/换上下文)
- 没有 agent 进程的 liveness/health check——一个僵死的 claude 进程只有超时才能被发现

### 影响

1. **agent 执行不可靠**:一次 transient 429,三十秒后重跑整个 prompt 烧全价 token
2. **无法编排工具**:agent 的能力完全取决于 CLI 给了什么工具。想加个 IDE 级别的 codebase 索引?需要重写整个执行器
3. **无法链接 agent**:A 的输出无法结构化为 B 的输入(当前靠 feeds_forward 传文本,没有结构化协议)

### 建议路线

1. **AgentRuntime 接口**:`internal/agentruntime/` 新包——定义 `Run(ctx, phase, toolset)` → `RunResult(output, cost, toolsUsed, structuredData)`
2. **Tool Manager**:声明式工具 schema(文件读/写,代码搜索,Bash,git),agent 不直接 `--allowedTools` 字符串,而是由运行时注册 + 权限策略决定
3. **Session 持久化**:agent 长期对话(multi-turn)的增量 checkpoint,不止于 CLI 的单次 `-p`
4. **智能重试**:不是全 prompt 重跑,而是把上次失败的 parsed 结果 + 错误 + 修正指令注入,增量修复而非全量重做

---

## 方向三:知识引擎——从 ADR TF-IDF 到结构化代码/决策知识库

### 为什么需要它

当前 `internal/prompt` 在构建 agent prompt 时注入三类上下文:

- **任务**:ROADMAP.md 体(截断 4000 rune)
- **ADR**:top-6 相关的 ADR 标题(靠 TF-IDF 关键词匹配)
- **约束**:AGENTS.md 前 6 条 bullet

这对一个几十个 ADR 的项目勉强够用。对于一个运行了 24h、积累 500+ 条 memory 条目、跨 5 个模块、有 200 个 ADR 的真实软件工厂,**绝对不够**。Agent 在做 implementer 任务时不知道「这段代码的架构约束是什么」「past decisions 为什么要用这种模式」「同模块有哪三个 ADR 与当前文件相关」。

### 现状缺口

- ADR 检索只用了标题+ TF-IDF,没有读取 ADR 正文内容来做语义匹配
- memory 存储是 JSONL append-only,查询只有精确的 `kind+topic` filter——没有"给我与 payment 模块相关的所有 decision"这种语义查询
- 没有代码结构索引——agent 自己靠文件系统探索,每次重新发现
- ContextCache 只是内存 cache,跨进程冷却后重建

### 影响

1. **Agent 决策质量上限低**:一个没见过相关 ADR 的 implementer 可能重复造一个已被否决的架构
2. **重复人工":reviewer 经常因为 agent「不知道上下文」而 REQUEST_CHANGES——这是整个 pipeline 最贵的环节(Opus 跑全上下文)
3. **长运行退化**:跑 100 轮后 memory 有 5000+ 条目,当前 compaction 策略(每 10 轮)不够频繁,容易撑爆

### 建议路线

1. **Memory 语义查询**:`internal/memory` 加 `Search(q string, kinds...)` 方法,用 retrieve.go 的 TF-IDF 检索 memory 条目而非仅精确匹配——RAG-on-memory
2. **ADR 正文全文索引**:当前 `adrTitles` 只取标题,改 `adrBody` 读整个文档(按章节),用同样的 retriever score 而非仅标题匹配;受 `adrTopK` 保护
3. **CodeContext 生产者**:`forge detect` 已经能做 project 级分析,扩展为按文件/模块生成结构化代码地图(imports · exports · test coverage · 依赖关系)——注入 implementer 的 prompt
4. **Memory 自动摘要**:超过 200 条时触发 LLM 调用(summarize)而非仅 trim;但这一条应标注为**需外部资源**(LLM 调用本身也要花钱)

---

## 方向四:多项目编排 + 中央控制面

### 为什么需要它

目前 `forge evolve` 是一个进程、一个工作目录、一个 project。如果组织有 5 个微服务需要同时演化——每个都有独立的 `.agent/` 树、独立的 ROADMAP、独立的 budget——只能开 5 个终端手动管理。没有中央仪表盘、没有全局预算分配、没有跨项目依赖协调,也没有"团队级别"的策略(所有 go service 必须过 security gate 才能 deploy)。

ForgeOS 的命名根源是"操作系统",但当前没有多进程调度。

### 现状缺口

- `LoopEngine` 是单实例的——一次一个 workflow
- 没有 `Scheduler` 层——谁先跑、谁可以跑、预算怎么分
- 没有全局 trace 聚合——每个 project 写自己的 `.forge/trace.jsonl`,无法看到组织级别 AI 成本
- `.forge/` 在项目根目录,不支持中央持久化(Temporal 声明的 `durable_wait` 是 v2)

### 影响

1. **扩展瓶颈**:5 个微服务=5 个 tmux 窗口,每个需要手动触发 `forge evolve`
2. **预算失控**:无中央 cap,每个服务独立设 `--run-budget-usd`,总和可能远超预期
3. **协调真空**:service A 的 API 改动影响 service B,A 的 pipeline 对此一无所知
4. **治理泄漏**:每个 project 可以自己设 `mode:explorer` 绕过严格闸门——中央无法强制执行最低 gate-set

### 建议路线

1. **ForgeOS Daemon**:`forge daemon` 后台驻留进程,管理 project 注册、调度、全局 trace 收集
2. **ProjectRegistry**:`$HOME/.forge/projects.yml` 列出所有已注册 project(每个带 root + lifecycle + 全局 min-gate-override)
3. **Scheduler**:**简单 FIFO 队列 + budget pool**(全局 `$budget / N project = 每 project cap`);不做复杂优先级调度(v2)
4. **全局 trace 聚合**:所有 project 的 trace.jsonl → 统一时序 DB(CloudWatch/Prometheus 不强制,先支持 `cat >> global/trace.jsonl`)
5. **跨 project 依赖**:声明 `depends_on_project: [service-b]`,当 service-b 的 ROADMAP 未满足时 service-a 的 deploy-gate 不绿

---

## 方向五:架构评估引擎——自动化设计治理

### 为什么需要它

当前的质量闭环止于 **Harness 闸门**(test/lint/complexity/arch/security) + **Human Approval**(design→build 闸门)。但中间有一个巨大的空白:对 AI 生成的代码**架构本身**缺少自动评估。

Reviewer agent 做代码审阅,但它是对一个 PR/MR 的**文本审阅**(读代码,提意见),不是**架构合规检查**(这个模块不应该依赖那个模块、这个接口的扇入超了、这里出现了循环依赖)。虽然 `arch-check.mjs` 有架构 8 检查,但那是**仓级别**的,在 commit 后才触发——太晚了。架构腐化在 AI 写代码的"三分钟"内就已经发生。

### 现状缺口

- 没有"架构预测"——改动前评估「这个文件增到 x 的扇出,会违反 layering 规则」
- 没有"演进轨迹"——5 轮迭代后模块 A 的依赖从 3 个膨胀到 17 个,需要自动标记
- `arch-check.mjs` 是**静态结构检查**(文件/包/导入),不是**架构风格合规**("这里应该用策略模式而非 switch"、"这个逻辑应该放在 domain 层而非 handler")
- reviewer 审代码,但每种语言、每个项目的架构规则不同,没有可定制的规则引擎

### 影响

1. **架构债务积累速度 > 人工审阅速度**:AI 24h 写代码,human reviewer 不可能每天审完所有变更
2. **Same mistake, next iteration**:一个被 reviewer 指出"不该依赖 infra 层"的 implementer,下一轮可能在其他模块犯同样的错——因为没有自动规则记住这个约束
3. **闸门盲区**:`forge accept` 通过但架构在崩溃——所有闸门都是 stateless 的

### 建议路线

1. **ArchRule Engine**:`.agent/architecture/rules/` 下声明式规则文件(类似 `.arch/rules.yaml` 的可扩展版本),支持 Layering · Package coupling · 命名约定 · 禁止模式
2. **Pre-commit architecture prediction**:`forge plan` 或 `forge run planner` 阶段,对 planner 的 task split + 受影响文件列表做架构影响分析——预测式而非反应式
3. **演进趋势仪表盘**:迭代间追踪:模块大小 · 依赖图熵 · 测试覆盖 delta;超过阈值 = stop condition 的额外 criterion
4. **规则模板库**:每个语言/框架的骨架规则集(`arch-rules/go-service.yaml` · `arch-rules/python-api.yaml`),forge-init 时按语言注入

---

## 边界情况与性能优化(横切关注)

### 已知边界风险

| 编号 | 问题 | 影响 | 修复方向 |
|------|------|------|---------|
| E1 | `parallel.go` LOCK ORDER CONTRACT 脆弱——新引入的 mutex 若顺序不对产生死锁(Heisenbug) | 并行模式下偶发死锁,极难复现 | 编译期 lock order 检查器(`internal/lockorder/`)或文档-测试-即合约 |
| E2 | Memory compaction 仅在每 10 轮触发——长时间运行(>50 轮)时 memory.jsonl 可膨胀到 >1000 行才触发 | prompt 注入 memory 上下文过大,浪费 token | 改为按**条目数**触发(>threshold 即 compact),轮次数作为 fallback |
| E3 | Checkpoint 保留 N 个历史版本(`rotateRetain`)——无上限,long-running evolve 可能积累上百个 `.forge/checkpoint.json.N` | 磁盘浪费,N 不合理大时 | 加 `--checkpoint-retain` 可配,默认 5 |
| E4 | `detect_parsers.go` 的语义探测(go.mod/package.json/pyproject.toml)读整个文件但无尺寸保护——一个 100MB `node_modules` 灰名单的 package.json 会撑爆 | 文件读入内存可能 OOM | 加 `maxDetectBytes`(默认 1MB) |
| E5 | `trace.jsonl` 旋转 10MB——rotate 时 `Rename` 非原子,同一进程可能写丢失 | 巧合下少 trace 行(非数据损坏) | `O_APPEND`+ 写前 rename、写后 rename + fsync |

### 性能热点

| 热点 | 原因 | 优化方向 |
|------|------|---------|
| ADR `firstHeading` 每 Gather 读所有 ADR.md 文件 | 即使 `adrTopK=6` 也从头读每个文件取标题 | 标题缓存(`.adr_titles_cache`)或单次扫描后缓存到 ContextCache |
| `LoadScorecards` 每次 `forge route` 全量读+decode | `forge route` 是快路径但每次读磁盘 | `scorecardCache` 加 mtime 监控(memory+persist 已用此模式) |
| `converge.RoadmapCompletion` 每次 gatherSignals 重扫 ROADMAP.md | 每轮多行扫描,evolve 中每 iteration 一次 | 利用 checkpoint 的 `RoadmapCompletion` 做惰性求值 |
| `gatherSignals` 每 iteration 读一次 ROADMAP.md + 跑 git diff | 文件 IO + `exec.Command("git", "diff")` | 可选的 `--skip-diff` flag(大型仓库中 git diff 慢) |

### 边角案例

- `forge run` 在没有 `.agent/ROADMAP.md` 时 `gatherSignals` 的 `RoadmapCompletion` 为 0,但 converge 可能仍然 `all_of` 未设定 roadmap_completion——导致 0% 收敛触发 `all_of` 零条件为空真(已被 `len(allOf) > 0` 保护)
- `forge evolve --resume` 从 broken pipe 恢复时,`prev RoadmapCompletion` 种子可能因 last checkpoint 是 phase 级别而略过迭代边界——正确性验证需增加对抗性故障测试
- `forge route --risk high --touches-payment` 等合并时,`mergeAutoSignals` 取 `OR` 只高不低,但 `ProdTraffic` 从自动推导永不设 true——意味着自动化变更永远不会标记为 `prod_traffic`,可能低估风险级别(已诚实标注在 `risk_diff.go`)

---

## 附录:方向优先级矩阵

| 方向 | 所需资源 | 现有基础设施 | 用户价值 | 风险(不做) |
|------|---------|------------|---------|-----------|
| ① 多厂商路由+闭环 | 高(新增 provider 适配器+CI 集成) | **高**(scorecard+trace+cost 管线就绪) | **高**(降 30-60% 成本) | 厂商锁定+成本浪费 |
| ② Agent-Runtime | 中-高(新包,重构执行管线) | 中(CommandExecutor 已有 Observe/Retry) | **高**(agent 可靠性提升 10x) | agent 不可靠=整个工厂不可靠 |
| ③ 知识引擎 | 中(纯 Go 增量) | **高**(memory+retrieve+ContextCache 就绪) | 中(随项目规模递增) | 大项目 agent 决策质量骤降 |
| ④ 多项目编排 | **高**(daemon+调度器+全局 state) | 低(目前单项目) | **高**(企业必须) | 无法扩展到 >1 项目 |
| ⑤ 架构评估 | 中(新规则引擎+已有 arch-check) | 中(arch-check 8 项+ `.arch/rules.yaml`) | **高**(防架构腐化) | AI 24h 写=24h 造债 |

**推荐执行顺序**:⑤→②→①→③→④

- **⑤ 架构评估**:ROI 最高。已有 `arch-check.mjs` 框架,加规则引擎成本低,产出是**每个 PR 都得到架构保护**。
- **② Agent-Runtime**:堵健壮性的最大洞。agent 不可靠则整个 pipeline 不可靠。
- **① 多厂商路由**:直接节省成本,scorecard 基础设施已是沉没成本,不加利用是浪费。
- **③ 知识引擎**:项目规模大了自然需要,prompt ContextCache 已准备好,但这属于*应需而建*而非*提前镀金*。
- **④ 多项目编排**:确认有多个项目需求时再启动 daemon 工作,过早建设增加维护成本。
