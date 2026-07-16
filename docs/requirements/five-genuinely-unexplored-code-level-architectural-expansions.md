# ForgeOS — 五个真正未覆盖的代码级架构扩展方向

> **声明**:本文基于对当前代码库 (`forge-core/`, `harness/`, `.agent/`) 的端到端扫描，
> 结合已存在的 85+ `docs/requirements/*.md` 分析和已解决的 `docs/FUNCTIONAL_REQUIREMENTS_AUDIT.md`
> 全部 GAP 列表。以下五方向是**经过交叉验证后确认未被已有文档覆盖**的高价值扩展方向。
> 每个方向都锚定在**代码中可定位的未被使用的接口/字段/模式**，而非泛泛的产品愿景。

---

## 方向一：Agent 阶段运行时监督树（Process Supervision & Watchdog）

**现状锚定**:`forge-core/internal/orchestrator/command_executor.go` 的 `Run()` 方法
执行 `cmd.Run()` 并等待 `cmd.Wait()`。一旦 agent 子进程 spawn，orchestrator 对它
没有任何中间可见性或控制——没有心跳、没有健康检查、没有阶段超时内的微超时。
`command_executor.go` 的 `interruptProcessTree`/`waitWithTimeout` 是唯一的监督机制，
且只在**整体超时**后触发（`command_executor_unix.go:40-55`）。

**方向**:在 agent 子进程和 orchestrator 之间插入一层监督者(Supervisor)，周期性地：

1. **存活探测**:对长时间运行(>30s)的 agent 命令发送`SIGINFO`/`os.FindProcess`探活，
   检测到僵尸或挂起时主动上报而非等待整体超时。
2. **阶段进度心跳**:agent CLI (`claude -p` 等)虽无原生进度回调，但可通过
   管道输出节流监控(`cappedBuffer` 的写入速率)推断是否活着的 agent 在做有用工作。
   持续 N 秒无输出 → 怀疑死锁 → 上报 trace 事件 + 可配置终止。
3. **资源使用量预监控**:在 `cappedBuffer` 侧跟踪**输出产生速率** + 累计 wall time，
   在接近 `--max-output-bytes` 或 `--timeout` 时**提前发出告警 trace 事件**
   (当前只在超标后 fail-closed，没有渐进式预警，这对无人值守运行是运营盲区)。

**为什么高价值**:
- 当前架构对 agent 子进程的模型是"发射后不管"——spawn、等待、要么成功要么超时。
  从真点火(Sprint 24-26, `examples/ignition.md`)可知，一次 `forge evolve` 的
  build 阶段可跑 5+ agent phases，每 phase 数十秒~数分钟，总计可达数小时。
  中间若一个 agent 进程挂起(不崩溃、不超时、只是不产生有用输出)，当前系统无法检测，
  直到消耗完 `--timeout`(`default=0`=永不超时，用户必须显式设)才察觉。
- 将监督逻辑与命令执行逻辑解耦为独立的 `Supervisor` 接口
  (`command_executor.go` 中本来已有空的 `SandboxConfig` 占位螺丝)，使 fork/exec 层
  和运行时策略层分离——这是向 north-star `Sandbox` 方向演进的真实一步。

**具体代码入口点**:
- `command_executor.go:102-118` `SandboxConfig` struct（当前是空壳 v1 placeholder）
- `command_executor.go:222+` `startCommand`/`cappedBuffer`（当前无输出速率监控）
- `loop.go:46-50` `OnIteration`——可为每次迭代注入监督检查点

**边界情况**:
- agent 进程因 OOM 被内核杀死(exit code 137 / SIGKILL) → 当前归类为 `KindConfig`(不可重试)
  → 若能检测到"OOM"信号特征，应降级为 `KindOverloaded`(可重试+退避)
- agent 进程在 `--max-output-bytes` 限流下仍在运行但输出被截断 → 当前静默截断，
  不会通知 agent 其实输出被截断了(agent 不知道自己的 stdout 不完整)
- 并发 phases (`RunParallel`, `parallel.go`)下多个子进程的监督需要**协调的取消树**
  (一个 phase 的超时不应牵连同 wave 的其他 phase)

**性能开销**:每 5-10s 一次 `Process.Signal(signal.SIGInfo)` 是常数级，对数百进程
  并行场景也安全。真正开销来自周期性地检查输出速率(读取 cappedBuffer 的写入计数器，
  原子 load，纳秒级)。

**与已有功能的关系**:
- 不替代 `--max-output-bytes`/`--timeout`(fail-closed 边界)，在它们之前加一层
  渐进式告警(approaching-boundary 事件)
- 不替代 `recursion guard`(`FORGE_AGENT_DEPTH`)，专注单进程健康而非进程树深度
- 接入现有的 `trace.Tracer` 事件通道(`ErrorEvent`/`OverloadEvent` 等已有
  constructor helper)，无需新增数据通道

---

## 方向二：离线回放引擎与故障取证（Replay Debugger / Session Forensics）

**现状锚定**:`forge-core/internal/trace/trace.go` 记录结构化的 JSONL 事件流，
`forge-core/internal/persist/checkpoint.go` 记录收敛状态快照，
`forge-core/cmd/forge/evolve.go` 中的 `--resume` 能从 checkpoint 恢复运行。
但**没有任何机制能让开发者"回放"一次已完成的运行**——检查每一步 agent 看到了什么 prompt、
产生了什么输出、为什么 gate 失败了、为什么收敛了。`forge doctor` (`doctor/anomaly.go`)
能检测异常模式(停滞 iteration、快速收敛等)，但异常检测只能"发现问题"不能"解释问题"。

**方向**:构建一个**离线回放引擎**，读取一次运行的完整 `.forge/` 目录
(checkpoint.json + trace.jsonl + memory.jsonl)并提供一个交互式/编程式视图：

1. **阶段级时间线**:遍历 trace 事件，按 seq 排序，渲染每个 phase/gate/iteration
   的耗时、状态、模型、成本。类似 `forge status --history` 但更细粒度。
2. **Prompt 重建**:从 `memory.jsonl` 和 checkpoint 的 iteration 上下文，
   重构出每个 agent phase 的 prompt（当前不保存 prompt 原文——这是最大取证缺口）。
   最小可行做法:启动 `--save-prompts` flag 将每 phase 的 `buildPrompt` 输出
   追加到 `.forge/prompts.jsonl`。回放引擎读取它。
3. **失败根因分类**:对失败的运行(trace 中存在 `"status":"failed"` 或收敛 false 的
   checkpoint)，自动进行**结构化根因分析**:
   - 是 gate 失败?哪个 gate?`trace.jsonl` 中每个 gate 事件已带 `Name` 和 `Status`
   - 是 agent 超时?哪个 phase?耗时多少?`trace.jsonl` 中 `Kind:"agent"` 带 `DurationMs`
   - 是 budget 耗尽?消费了多少?`checkpoint.json` 的 `SpentUsdMicros`
   - 是 reviewer 拒绝?`verdictLedger` 的 REQUEST_CHANGES 计数
4. **跨运行对比**:将两次运行的 trace/checkpoint 加载为两条时间线，并排对比
   (哪次更快?哪次更便宜?哪次通过了更多 gate?哪个 phase 的耗时差异最大?)

**为什么高价值**:
- 当前最让运营商不安的场景不是"运行失败"，而是"运行成功了但不知道它到底做了什么"。
  Sprint 24-26 真点火证实了 agent 自治运行可以长达数十分钟甚至数小时。
  `forge doctor` 的 anomaly 检测能找到"有问题"但不能回答"发生了什么"。
  离线回放是 ForgeOS 的**运营可观测性基座**——没有它，自治运行是一个黑箱。
- trace.jsonl 已有丰富的事件结构：6+ event kinds、`Seq`/`DurationMs`/`CostUsdMicros`/
  `Model` 字段全部就绪，`trace.GateEvent`/`DecisionEvent` 等 constructor helpers 也已存在。
  缺失的是**消费端**——目前没有任何代码读取 trace.jsonl 来做聚合呈现。
  这是现成的数据基础设施在等待一个消费者。
- `doctor/anomaly.go` 中的 `DetectAnomalies` 函数(异常检测启发式引擎)可以作为
  回放引擎的分析后端复用——检测到的异常再加时间线上下文就是完整的根因分析。

**具体代码入口点**:
- `trace.go:30-104` `Event` struct（完备的字段，等待消费）
- `doctor/anomaly.go:95-155` `DetectAnomalies` 函数（可复用的启发式分析引擎）
- `cmd/forge/status.go` 的 `Status` 命令路径（最自然的扩展点:加 `--replay` flag）
- `ScorecardWind` / `scorecard_wind.go`——已有从 trace JSONL 计算百分位的框架

**边界情况**:
- `.forge/` 目录不完整(只有 trace 没有 checkpoint，或反过来) → 回放引擎应
  诚实报告"partial replay"而非崩溃。trace 有 seq 排序后可独立于 checkpoint 重构时间线。
- 两次运行使用不同版本的 forge-core → 事件结构可能变化(trace 的 `_format` 字段
  专为此场景设计——version field + backward compat decode)。
- 超大型 trace(24h 运行产生 10,000+ 事件) → 回放引擎必须支持分页/过滤/聚合，
  不能全部加载到内存渲染。

---

## 方向三：跨会话知识传递与模式库（Cross-Session Knowledge Transfer & Pattern Library）

**现状锚定**:`forge-core/internal/memory/memory.go` 实现了**单项目内**的跨迭代知识存储
(`KindGap`/`KindDecision`/`KindLesson`，JSONL 格式，带 `Supersedes` 机制)。
但 `memory.go` 将 path 视为唯一键(第 30 行的 `loadCache` 按 `${path}` 缓存)，
`Append` 只写到 `path` 指向的 `.forge/memory.jsonl`。
**没有跨项目共享**:项目 A 的 `memory.jsonl` 永远只存在于项目 A 的 `.forge/` 目录下，
项目 B 的 loop 无法查询它，项目 A loop 的教训不能帮助项目 B 避免同类错误。

**方向**:构建**跨会话知识传递层**，让一个项目(或一个运行)学到的知识
可以被另一个项目(或同一项目的后续运行)查询和引用：

1. **共享知识存储（Shared Knowledge Index）**:
   在用户主目录下(如 `~/.forgeos/knowledge/`)创建一个**多 project 可追加的知识索引**。
   每个 `memory.Append` 在写入本地 `.forge/memory.jsonl` 的同时也**异步地**
   (可配置、可禁用)写一份到共享索引。共享索引按 Topic 去重 + 按 `CreatedAtUnix`
   保留最新的版本。查询时本地知识优先 + 共享知识填补缺口。
2. **可移植教训匹配（Portable Lesson Matching）**:
   当一个 evolve loop 产生一个 `KindLesson`(如"Go 结构体 JSON tag 与 snake_case 不匹配
   导致 gate 失败")，共享索引抽象出 lesson 的**模式标签**(如语言=Go、领域=serialization、
   工具=lint)而不是原文。新项目启动时若有相同标签组合，从共享索引注入相关 lessons
   到 agent prompt 中。
3. **组织级记分卡聚合（Organization-Level Scorecards）**:
   当前的 `scorecards.json` (`.agent/routing/scorecards.json`)只记录本项目的历史路由。
   跨会话聚合可将**多个项目**的成本/延迟/质量数据汇聚为**组织级**记分卡，
   使 `HistoryTiebreak` 的统计基础从"本项目 N 次运行"扩展到"整个组织的 10N 次运行"，
   让冷启动项目的路由更快收敛到最优模型选择。
4. **模式库自动更新（Pattern Library Auto-Maintenance）**:
   当一个 `KindLesson` 在跨会话索引中被多个项目"确认有用"(通过人工反馈或自动检测到
   lesson 推荐的模式减少了该项目的 gate 失败)，其置信度自动提升。
   累计达到阈值 → 提升为 `KindDecision`(project 级约束)→ 最终可自动生成
   一个 ADR 草案供 architect 审阅——实现教训→决策→制度化的知识飞轮。

**为什么高价值**:
- `memory.go` 的 `Kind` 枚举(`gap`/`decision`/`lesson`)、`Topic`/`Supersedes` 字段、
  `Confidence` 浮点都是为这个方向设计的——只是当前只在单项目范围内运行。
  跨会话传递是这些机制的**自然放大**，只需增加共享索引 + 可移植匹配层，
  不需要修改 memory 的核心数据结构或序列化格式。
- 知识飞轮(lesson→decision→ADR)是 ForgeOS 核心承诺之一(G5:持续演化)中隐含的
  但从未显式实现的能力。Sprint 17 的 `priorities` 处理决策(不链接独立加权语义)
  和 Sprint 30 的 G3 审计(多维路由不驱动执行)都指向同一个缺口:截止目前，
  学习闭环只能优化成本和延迟，不能提升代码质量——知识传递是补上质量维度的钥匙。
- 对多项目组织，跨会话索引的 ROI 随项目数线性增长(不是每个项目的孤立教训浪费时间)。

**具体代码入口点**:
- `memory.go:20-30` `loadCache`(按 path 缓存——这是单项目隔离的机制根源)
- `memory.go:64-94` `Entry`(Kind/Confidence/Supersedes 全部就绪)
- `cmd/forge/evolve.go` 的 `recordMemory`/`memoryHook`(知识产生点)
- `prompt_context.go` 的 `appendFeedbackLanes`(知识注入点)

**边界情况**:
- **隐私/隔离**:共享索引中的 lesson 绝不能泄露源代码或项目结构细节。
  `pattern_tags` 抽象层必须剥离所有项目特定文本，只保留**可泛化的模式签名**
  (如"Go struct field tag missing in `json:`")。User 主目录被多用户共享时
  需要文件权限隔离。
- **知识膨胀**:跨项目积累的 lessons 可能快速增长。需要 `kind=lesson` + `confidence<0.3`
  的自动淘汰机制，以及 `+kind:decision -kind:gap` 的分层保留策略。
- **冲突处理**:项目 A 的 lesson 说"用 errgroup"而项目 B 的 lesson 说"用 channel"——
  共享索引按 Topic 键保留最新(按 `CreatedAtUnix`)，但可能丢失历史分歧。
  需要"分歧检测":当一个 Topic 存在多个非 supersede 关系的高置信度 lesson 时，
  标记为"conflicting"并等待人工裁决。

**性能开销**:
- 共享索引写操作是本地 append 后的 **fire-and-forget 异步 IO**，不应阻塞主循环。
- 查询共享索引只在**启动阶段**执行一次，memory copy 约几十 KB——对运行时延迟零影响。

---

## 方向四：增量门评估与变化感知选检（Incremental Gate Evaluation & Change-Aware Audit）

**现状锚定**:`harness/acceptance.mjs` 和 `forge-core/internal/gate/gate.go` 的
`Gate`/`Check`/`Accept`/`ProbeAll` 函数**每次都是全量运行**——无论改动了 1 个文件
还是 100 个文件，都跑所有 gate(all 9 check.py checks + 6 gate.mjs checks +
8 arch-check.mjs checks + test/app-test + secret-scan + SCA + lint + coverage)。
`harness/select-tests.mjs` 已存在但**明确声明为 advisory only**(`select-tests.mjs:3-5`
"never replaces the full forge accept gate")，且仅用于 test 选择而非全 gate 选择。

**方向**:构建**增量门评估框架**(Change-Aware Evaluation)，让 forge-core 知道
一个运行作用于哪些 changed files，并据此动态决定哪些 gate 必须跑、哪些可以跳过、
哪些必须全量跑：

1. **更改清单注入(Change Manifest)**:
   在 `forge run`/`forge evolve` 的入口(in `main.go` 的 `runOpts` 附近或在
   `engine_build.go` 的 agent executor 构建路径中)增加 `--diff-files` flag，
   接受 `git diff --name-only` 的输出作为**本运行的已更改文件清单**。
   未提供时全量跑(向后兼容)。
2. **门-文件映射表(Gate-to-File Mapping)**:
   定义**每个 gate 关心的文件范围**的结构化表(在 `gate.go` 或在 `resolve.go` 中):
   - `gate.mjs`(体积):取决于所有文件 → 全量跑
   - `arch-check.mjs`(架构):取决于 Go/Python/TS 源文件但排除 generated → 若只改了
     markdown，可安全跳过
   - `check.py`(治理完整性):取决于 `.agent/` 和 YAML → 若改的不是这些，可跳过
   - `secret-scan`:取决于有 secret 风险的文件 → 只跑含 `.env`/`key`/`token` 模式的路径
   - `test`/`app-test`:取决于源文件和测试文件 → 用已有的 `select-tests.mjs` 建议机制
   - SCA (`sca.mjs`):取决于 manifest 文件 → 只当 `go.mod`/`requirements.txt` 等改变时跑
3. **增量裁决缓存(Incremental Verdict Cache)**:
   缓存上次全量运行时每个 gate 的裁决结果(状态+PASS/FAIL/NA+文件哈希)。
   当 changed files 不触及某 gate 的关心范围时，直接从缓存返回上次裁决。
   缓存失效条件:gate 关心范围中的任何文件被更改，或上次 FAIL 必须重跑确认 fix。
4. **渐进式强制报告**:
   输出中清晰标记哪些结果是增量复用(FRESH/INCREMENTAL-CACHED)，
   保持诚实——永不伪造"未执行的检查"为 PASS。

**为什么高价值**:
- Sprint 11 的 adapter lint 框架和 Sprint 12 的 adapter coverage 框架已确立了
  "按需检查、无工具则 N/A"的诚实模式。增量选检是这个模式的**逻辑延伸**——
  不仅是"无此语言→N/A"，而且是"此文件未变更→复用上次结果"。
- 当前 `ProbeAll` (`gate.go:110-145`，每次 `forge run`/`forge evolve` 的 iteration
  都跑一次)每次执行约 6-10 个子进程(node + python3 各 gate)，每次耗时 0.5-3 秒。
  对于 30-iteration evolve 循环，gate 检查本身就可占 15-90 秒。
  增量评估可将高频开发循环(改 1 个文件→跑 gate→feedback 循环)的 gate 开销
  从数秒压缩到亚秒级，显著影响开发者体验。
- Sprint 13 已建立了 `select-tests.mjs` 的 advisory 选择模式——这证明项目接受
  "启发式选择+全量 fallback"的哲学。增量门评估只是把这个模式从 test 扩展到
  所有 gate。

**具体代码入口点**:
- `gate.go:110-145` `ProbeAll` (当前每次全量执行所有 probe)
- `gate.go:36-40` `Result` struct (已有 `Name`/`OK`/`Status`/`Output`——可加
  `FileHash` 或 `Freshness` 字段)
- `acceptance.mjs` 的 `--json` 输出格式(`probeRow` + `status` + `category`)
- `harness/select-tests.mjs` 作为 advisory 参考实现

**边界情况**:
- **传递依赖**:改一个 shared Go interface 文件会影响所有实现者 gate。
  门-文件映射不能只做直接映射，需要**传递依赖解析**(Go 的 import graph)
  ——或保守地当传递依赖不确定时全量跑(诚实 fallback)。
- **缓存安全性**:缓存使用文件内容的 SHA256 哈希，而非 mtime。
  `git checkout` 切换到旧分支时 mtime 不可靠但内容可哈希验证。
- **首次运行**:无缓存时全量跑 + 填充缓存。缓存为空时不退化(与今天逐位一致)。
- **并发安全(`RunParallel`)**:多个 phase 可能同时调 `ProbeAll`——
  缓存读取必须并发安全(`sync.Map` + 读锁)，写入在首次全量跑后原子替换。
- **生命周期状态迁移后**:`forge migrate` 从 explorer→engineering 后，
  缓存应从`warn`模式下的记录升级为`block`模式下的强制验证——不迁移旧缓存。

---

## 方向五：量化 Agent 输出质量评估（Quantitative Agent Output Quality Evaluation）

**现状锚定**:`forge-core/internal/converge/converge.go` 的 `Signals` 包含
`RoadmapCompletion`/`GatesGreen`/`ReviewStatus`/`RequirementConfidence`/
`FileDelta`/`CodeTestRatio`。**没有一个信号衡量 agent 输出的内在质量**——
代码是否可维护、是否正确处理错误、是否遵循项目约定、是否有安全缺陷。
`Signal.Criteria` 接受 `test_pass`/`architecture`/`arch_violations`/`complexity_violations`
等准入标准，但这些全是**门式二元判决**(PASS/FAIL/NA)，不是**连续质量评分**。
`internal/routing/routing.go` 的 `TierForScore` 对 task 做多维评分
(complexity/dependency/context/business_impact)用于**路由**，但从不用于评估
**输出质量**——这些维度是**任务固有属性**而非**产出的质量属性**。

**方向**:引入**量化 Agent 输出质量维度**，作为 `converge.Signals` 的新维度
和 `trace.Event` 的新事件种类，创建一个持续追踪和比较质量的记分卡轴：

1. **轻量级质量探针集合**:
   a. **代码风格一致性**:重复运行 `lint` gate(已有 adapter 框架)，但不再是
      PASS/FAIL 二元判决，而是**统计违规密度**(每千行违规数)。改进→质量提升，
      恶化→质量下降。数据来自 `lint` gate 的 `Output` 文本，无损接入。
   b. **测试覆盖率趋势**:重复运行 `coverage` gate(已有 adapter 框架)，
      但追踪**覆盖率增量**而非是否达到阈值门槛。
      每个 iteration 的覆盖率增量 → 连续质量指标。
   c. **复杂度趋势**:重复运行 `complexity` gate(已有 arch-check.mjs 的
      function-length 检查)，追踪**超限函数数量变化**。
      Sprint 5 的真实 dogfood 事件(方向五开发中 113 行测试函数被 gate 捕获→强制重构)
      说明复杂度门是实际有效的质量信号。
   d. **(可选,非 v1)LLM 作为评判**:对高风险 phase(如 security-review)，
      用第二个独立的 LLM 评判 agent 输出质量(事实性、完整性、可操作性)。
      这需要额外成本但提供了无 gate 覆盖的质量维度。
2. **质量信号归一化与趋势化**:
   将上述探针的输出归一化到 [0,1] 的**质量分(Quality Score)**，每个 iteration
   追加到 `converge.Signals` 的 `QualityMetrics` 新字段(位置在 `converge.go` 的
   Signals struct 中)。LoopEngine 每轮报告"质量趋势"(↑ 稳定 / ↓ 恶化)，
   在质量连续 N 轮下降时触发**非收敛告警**(类似 stale-progress tripwire)。
3. **质量分驱动路由**:
   当历史质量分表明某个 agent+model 组合在特定 task_type 上的质量持续低于
   阈值时，routing 的 `HistoryTiebreak` 可**拒绝选该组合**，
   即使它更便宜或更快。这补上 Sprint 17 `priorities` 的缺口——
   "quality"优先级现在不是虚构字段而是可衡量的路由信号。
4. **质量回归检测**:
   跨会话(方向三的共享索引)追踪每个 agent 角色的质量基线。
   新运行的质量分显著低于基线(`-2σ`)→自动标记为**回归** → 生成 trace 事件
   + 选项式通知。

**为什么高价值**:
- 当前系统可以告诉你"gate 通过了"但无法回答一个更根本的问题:
  **agent 写的代码好吗?** Sprint 25 的真点火暴露了一个根本问题:agent 没写测试
  但自己诚实拒绝了勾 ROADMAP 复选框。这是 agent 诚实性的胜利，
  但系统层面没有机制**鼓励更好的输出**——只是对其诚实度被动反映。
- `converge.go` 的 `evalOne` 函数已经有了 `acceptanceMetrics` 集合
  (`test_pass`/`architecture`/`arch_violations`/`complexity_violations`)，
  每个指标从 `Signals.Criteria` 中提取。质量探针只需为这些指标增加
  **连续分值**(从"PASS/FAIL"扩展到"多少分")，就能在现有 convergence 框架
  内运行，不破坏已有边界。
- 质量维度加进 trace + scorecard 后，ForgeOS 的学习闭环就从
  **单目标(成本/延迟)优化**升级为**多目标(成本/延迟/质量)优化**——
  这是系统愿景中"持续演化"的实质含义，不是更多 gate 而是更好的 gate。

**具体代码入口点**:
- `converge.go:14-38` `Signals` struct——添加 `QualityMetrics map[string]float64`
- `converge.go:183-213` `Evaluate`/`evalOne`——为 quality metrics 增加新 Metric 分支
- `cmd/forge/gates.go` 的 `gatherSignals` 函数(信号收集的汇聚地)
- `internal/routing/routing.go` 的 `HistoryTiebreak` 函数(质量分的新消费者)
- `trace.go` 的 `Event`——可以加 `KindQuality` 新事件种类

**边界情况**:
- **探针本身的质量**:lint 和 coverage 是确定性工具，不存在"误判"问题。
  LLM-as-judge 探针有幻觉风险——必须归为可选(opt-in)并在 trace 中立标记
  `"judge_model":"claude-opus-4"`，让运营商可以审查评判质量。
- **短期波动 vs 真实衰退**:单次 iteration 的质量分下降可能是正常的
  (agent 在大重构，中间状态必然更差)。质量 tripwire 必须要求连续 N 轮下降
  才触发告警(类似 stale-progress: `noQualityProgress`)，避免大重构中误报。
- **质量分的新项目冷启动**:新项目无历史基线时，质量 trend 报告"insufficient data"，
  不触发任何告警也不提升违反——与 `HistoryTiebreak` 的冷启动策略一致
  (无历史时 fallback 到 tier default)。
- **与已有记分卡的关系**:质量分不是替代 `scorecards.json`，而是它的新维度。
  当前 scorecard 已有 `avg_cost_usd`/`p95_latency_ms`，新增 `quality_score` 维。
  不需要改变 scorecard schema，只需新增字段。

---

## 总结:五方向的依赖与优先关系

```
方向三(跨会话知识) ←── 方向五(质量评分)
       │                     │
       │                     ↓
       └──────────→ 方向四(增量门评估) ←── 方向一(监督树)
                            │
                            ↓
                    方向二(离线回放)——依赖全部其他方向的数据
```

- **独立快速取胜**:方向一(监督树)和方向四(增量门评估)不依赖其他方向，
  可独立开发且短期内产生可观测收益(运营安全感 + gate 速度)。
- **数据基础设施优先级**:方向二(离线回放)依赖方向一的 trace 丰富化
  和方向四的缓存结构，应稍后启动但一旦启动即解锁"可调试自治运行"。
- **长期竞争力**:方向三(跨会话知识) + 方向五(质量评分)共同构成
  ForgeOS 相对于"裸 Claude Code"的核心护城河——不是更快的 gate，
  而是组织级的学习飞轮，这是竞争对手最难以复制的部分。

创建日期:2026-07-10
基于代码库:forge-core (Go 18 packages) + harness (Node + Python)
审计基准:已与 `docs/FUNCTIONAL_REQUIREMENTS_AUDIT.md` 逐 GAP 核对，无重复
