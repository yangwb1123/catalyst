这是一份非常高质量的架构分析——代码证据详实,跨包边界识别准确,而且每个方向都带着明确的 ROI 判断而不是泛泛的「应该做」。作为熟悉这个代码库的人,我逐方向给出我的审阅和补充。

---

## 总体评价

**最大的优点**:每个方向都找到了「代码证据→结构性风险→ROI 判断」的三段论证,而且不是扫缺口式的平铺,而是真的在找跨包、跨 sprint 的数据流缺口。这份文档本身可以作为架构 RFC 的起点。

**最大的盲区**(我认为):低估了 **LLM 行为不确定性**对方向①和方向③的约束,同时**高估了当前 cost 数据管道的统计显著性**对方向⑤的支持。

---

## 逐方向审阅

### 方向① · 确定性仿真测试

**分析准确度**:8/10。代码引用精确(`loop.go` 的 `OnRejected` 分支、`parallel.go` LOCK ORDER CONTRACT、`phaseCheckpointHook` 的不变量问题都真实存在)。

**但我补充一个重要的结构约束**:ForgeOS 的 orchestrator 复杂性来源和 FoundationDB/TigerBeetle 有本质区别:

| 维度 | FoundationDB / TigerBeetle | ForgeOS Orchestrator |
|---|---|---|
| 非确定性来源 | 调度交错 + 网络分区 + 磁盘故障 | **LLM 输出内容不确定性**(时序上大概率是确定性串行的) |
| 并发模型 | 多副本、多线程、复杂锁层次 | 单 goroutine 主循环 + `RunParallel` 用 `errgroup` 管理,锁合约只在 parallel path 存在 |
| 状态空间爆炸原因 | 副本数 × 消息类型 × 故障注入 | agent 返回值的组合(`REQUEST_CHANGES` / `REJECTED` / loop-back 计数相交) |
| 核心不变量 | 线性izability / 精确一次 | **budget 不超限 + checkpoint 不丢失已完工作** |

**这意味着什么**:真正的死锁风险其实很低——`parallel.go` 的锁顺序合约在两个 goroutine 之间(主循环 vs parallel agent caller),且加锁区间很短。Heisenbug 级的死锁概率存在,但不如你描述的高。**真正值得仿真的是「LLM 返回值序列 × budget 计数器交互」的组合爆炸**,而不是调度交错。

**建议调整**:不要以「并发死锁形式化证明」作为首要目标(那更适合作 Sprint 后的 hardening pass),而是以 **「checkpoint 崩溃一致性 + budget 不变量穷举」** 作为仿真框架的核心验证目标。具体:

- `SimulationHarness` 注入 LLM 返回值序列(模拟 `REQUEST_CHANGES` 在第 N 次、`REJECTED` 在第 M 次、`APPROVED` 最后),而不是注入 goroutine 调度交错
- 验证关键不变量:迭代结束时 `totalSpent <= MaxRetries × MaxAgentCalls × perCallBudget` 始终成立
- 验证 checkpoint 恢复后 `phaseIdx + spentMicros` 组合产生精确一次语义

这样 1-2 sprint 就能出价值,而不是 4 sprint 做一个通用仿真引擎。

---

### 方向② · 跨 Agent 信任链

**分析准确度**:9/10。这是五方向中最敏锐的。`memory.Entry.Confidence` 字段存在但从未自动衰减,以及 `prompt_memory.go` 无差别注入全部 `KindLesson`——这两个发现说明你真的读了每一行。

**但我发现一个你漏掉的攻击面**:`feedback_loop.go` 的 `collectPhaseFeedback` 把 reviewer 的原始输出写入 `memory` 且**不做任何结构化过滤**。reviewer 是最容易被 prompt 注入的 agent(它读大量代码),它的输出直接变成下一轮的 context——这比 `KindLesson` 写入的攻击面更直接,因为它**不经过 `Confidence` 过滤器**(feedback 走的路径是 `memory.FeedbackEntry` 而非 `memory.Entry`)。

**补充建议**:
1. 除了你提出的信任层级标签,建议增加一个 **provenance chain 字段**直接嵌入 memory/trace schema:
   ```go
   type Provenance struct {
       SourceAgent  string   // 哪个 agent 产生了这条信息
       SourcePhase  PhaseID  // 哪个 phase
       IterationNum int      // 第几次迭代
       HarnessGate  string   // 如果有带外验证,哪个 gate 确认了
   }
   ```
   这个字段的存在使得审计时可以直接问「谁说的」而不需要 parse trace。

2. **你低估了「gate 结果不可否认性」的工程成本**:要真正实现不可否认,需要外部签名密钥(不能在代码仓库里),需要在 CI/CD 中集成 KMS 或类似基础设施。对于 v1,一个更实际的起点是 **`gate.Result` 的 stdout 加上 commit hash + timestamp 做 SHA-256,存到 trace,并提供一个 `forge verify-trace --hash` 命令来重算验证**——不需要密钥,只需要确定性重放。

---

### 方向③ · Workspace 级多项目学习

**分析准确度**:7/10。代码引用正确,冷启动问题真实存在。但我觉得你**低估了架构复杂度**。

关键问题不在于 `scorecardPath(o.root)` 改为 `~/.forge/scorecards/`——那只是文件路径的改动。真正难的是:

1. **数据冲突**:项目 A 和项目 B 用同一个模型(CLaude Sonnet),但对项目 A 效果很好(因为项目 A 是 Python CRUD)而对项目 B 效果差(因为项目 B 是 Rust 内核编程)。全局 scorecard 如果简单地平均,会**对两个项目都给出更差的路由决策**。你需要**分层归因**——按 task type / language / framework 做条件聚合,而不是简单的全局合并。

2. **记忆冲突**:项目 A 的 `KindLesson`「用 `os.ReadFile` 比 `ioutil.ReadFile` 好」是通用的;项目 B 的 `KindLesson`「这个特定 API 的 auth 有坑」是项目特定的。没有 schema 区分这两者,跨项目导入会引入噪声。

3. **治理权**:谁来决定一条 memory 是否可跨项目共享?你提到了 CTO 角色,但当前没有这个 persona。这会引发一个 governance 设计问题——比文件路径复杂得多。

**务实建议**:与其做一个通用的 workspace 系统,不如先做 **`forge scorecard merge` 命令**,允许运维人员手动将一个项目的 scorecard 合并到另一个项目(作为冷启动种子),加一个 `--task-type-filter` 参数只合并同类任务的 scorecard。这样**不需要改变存储架构**,只需要一个 CLI 命令 + 合并逻辑,1 sprint 就能做出来验证价值。

---

### 方向④ · 执法器 SLO 监控

**分析准确度**:10/10。这是五方向中最扎实的,代码证据清晰,ROI 判断合理,工程规模估价准确。

**补充一个你提到的但可以更突出的点**:Sprint 27 的 `block-scalar 损坏` bug 在测试中隐匿了 6 轮 sprint——这是执法器退化最真实的危险信号。如果你的 SLO 监控系统当时存在,这个 bug 可以在第一个 sprint 就被发现(`arch-check` 的预期输出 vs 实际输出 mismatch)。

**最佳切入点**:`harness/arch/arch-check.mjs` 自带一个 `--self-test` 模式:读一个已知的架构违规样本文件,确认它输出 `ARCH_VIOLATION`。所有 harness 脚本都得通过这个测试,否则 `forge doctor` 报 WARN。这个**不需要新的基础设施,只需要给每个 gate 加一个 `--self-test` flag + 一个样本文件**。

我强烈建议这个方向作为**优先级最高的短期行动**:2-3 周可以完成,而且每修一个 gate bug 后可以立即验证执法器是好的。

```
forge doctor --gates
  ✔ harness/gate.mjs ........... PASS (0.12s, 7/7 probes OK)
  ✘ harness/arch-check.mjs .... FAIL (0.34s, self-test: expected ARCH_VIOLATION got PASS)
  ⚠ harness/secret-scan.mjs ... WARN (self-test skipped: no sample file)
  ✔ harness/check.py ........... PASS (0.08s, self-test OK)
```

---

### 方向⑤ · 成本优化引擎

**分析准确度**:7/10。成本数据管道描述准确,产品愿景清晰。但我认为你**低估了两个关键约束**:

1. **Scorecard 数据稀疏性**:当前 scorecard 是 per-project、per-model、per-task-type 的。对于一个冷项目,可能只有 3-5 条 scorecard 记录——不足以产生统计显著的 `avg_cost_usd` 估计。
   你提到的「Run 前成本预估器」在没有方向③(跨项目 scorecard 合并)之前,**预估的方差极大**,可能会导致用户不信任它。

2. **成本-质量帕累托边界需要用户定义「质量」**:你说「哪 3 个 phase 值得 Opus、哪 7 个 Haiku 就够了」,但系统怎么知道每个 phase 的「质量需求」?planner 可能需要高质量(reasoning-intensive),gate 需要精确(便宜模型可能漏检),reviewer 需要公正(中等即可)。**这需要用户声明 phase-level 的质量等级,或者从历史数据中学习 phase 质量敏感性**——两者当前都不存在。

**务实建议**:方向⑤的正确切入点是**方向③之后的第二阶段工作**。先建立跨项目 scorecard 的统计显著性,再做成本预测。在此之前可以做的是:

- 在 converge report 中加入 **cost-per-roadmap-point 指标**(已经有 roadmap completion 和 cost,就差一个除法)
- `forge run --budget` 的 help text 加上历史成本参考:「上一次类似 build: $1.42 (3 phases, 5 iterations)」

这些不需要方向③,但能为方向⑤积累需求信号。

---

## 优先级调整建议

你给的顺序是 ① → ④ → ② → ⑤ → ③。我认同④在最前面,但我认为你的排序忽略了**工程风险 vs 产品价值的时间维度**:

| 象限 | 方向 | 理由 |
|---|---|---|
| **立即做(2-4 周)** | ④ 执法器 SLO 监控 | 低风险,立即可见,工程纪律质变 |
| **短期(4-8 周)** | ② 信任链加固(memory provenance only,不做签名) | 中风险,高价值,企业采购前提 |
| **中期(8-12 周)** | ① 仿真测试(LLM 序列注入版本,不做调度交错穷举) | 中风险,长尾价值 |
| **长期(12-24 周)** | ③ Workspace + ⑤ 成本引擎(合并为"平台化轨道") | 高回报,但需要方向②和足够的 scorecard 数据做前提 |

**我的调整**:④ → ②(memory provenance 子集) → ①(裁剪版) → ③+⑤ 合并为平台轨道

理由:
- 方向④现在就可以开始,不需要任何依赖
- 方向②的 memory provenance 增强是方向③的前提——不做②直接做③,跨项目共享的知识可能是毒药
- 方向①裁剪后 2 sprint 能出价值,为 orchestrator 修改提供安全网
- 方向③和⑤绑在一起做,因为成本优化需要跨项目数据才有统计意义

---

## 我特别欣赏的一个观察

你注意到了 `loop.go:nextStartPhase` 的 `OnRejected` 分支——注释说「intentionally dormant」——并正确地指出这是**声明但未测试的契约**,未来代码演化可能激活它但缺乏行为验证。这种级别的观察(不是「这里缺测试」,而是「这里的代码声明了一个未被任何路径触发的行为契约,没有人在日后修改时会意识到它的存在」)正是架构师视角和普通代码审查之间的分界线。

这份文档值得被收录为 `.agent/ARCHITECTURE-DIRECTIONS.md` 作为规划参考。要我帮你把它格式化为架构 RFC 存入仓库吗?
