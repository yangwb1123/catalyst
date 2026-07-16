# ForgeOS — 战略扩展方向分析

> **视角**: 资深架构师 / 产品经理。基于全局代码扫描（16 Go 包 · 60+ Go 源文件 · 30+ harness 文件 ·
> 35+ 已有分析文档 · 完整 `.agent/` 治理资产），结合已有 8 轮扩展分析的盲区，提炼
> 5 个「当前未实现但一旦落地即解锁全新能力层级」的方向。
>
> **纪律**: 不与现有 ROADMAP 条目（v3 跨厂商池 / Sandbox / Web-UI）重复；
> 不凭空镀金 —— 每个方向有明确代码级证据和「为什么现在不做会限制下一量级」的推理。

---

## 目录

1. [方向一:Agent 身份、信誉与权限模型 (Agent IAM)](#方向一agent-身份信誉与权限模型-agent-iam)
2. [方向二:知识生命周期管理 (Memory RAG Pipeline)](#方向二知识生命周期管理-memory-rag-pipeline)
3. [方向三:确定性回放与回归测试框架](#方向三确定性回放与回归测试框架)
4. [方向四:多仓治理与组织级控制平面](#方向四多仓治理与组织级控制平面)
5. [方向五:并行编排的失败语义与资源核算](#方向五并行编排的失败语义与资源核算)

---

## 方向一:Agent 身份、信誉与权限模型 (Agent IAM)

### 代码级证据

- `cmd/forge/main.go` 的 `defaultAgentAllowedTools` 对所有 agent 一视同仁:
  reviewer 和 implementer 共享完全相同的 Bash 白名单,permission 唯一可选值是
  `acceptEdits` | `plan` | `default`, 无法按角色差异授权。
- `internal/asset/asset.go` 的 `Phase.Agent` 仅是一个 `string` 标签
  ("implementer" / "reviewer" / "planner"), 没有任何与之关联的身份元数据。
- `internal/orchestrator/command_executor.go` 的第 1–80 行: `CommandExecutor`
  完全不感知 agent 身份,它只运行 `Build(p, mode)` 返回的 argv。
- `cmd/forge/engine_build.go` 的 `agentExecutor`: 选择 exec 逻辑时只区分
  `o.executor == "command"` vs dry-run, 不在 agent 身份上做任何权限分流。
- `internal/routing/routing.go` 的 `opusFloorAgents` 是硬编码 map,不是从 YAML
  策略加载的动态身份声明——新增一个高安全 agent 需要改 Go 代码重新编译。

### 当前体系的能力边界

ForgeOS 有 14+ 个 agent 角色卡(`.agent/agents/*.md`),在文档层级做了详尽职责划分:
- `reviewer.md`: "fresh-context, 不审自己代码, VERDICT: APPROVE/REQUEST_CHANGES"
- `implementer.md`: "写代码,跑测试,自检 ROADMAP"
- `security-engineer.md`: "STRIDE 威胁建模"

但**运行时完全没有身份概念**。
- 一个 prompt-injected 的 implementer 可以自称是 reviewer 并输出 `VERDICT: APPROVE`
  ——系统没有机制验证 caller identity。
- 一个 implementer 被注入恶意指令后, 它的 `--allowedTools` 白名单和 reviewer 完全一致,
  没有最小权限原则。
- 没有「agent 行为信誉分」: 一个反复产出坏代码/测试失败的 implementer 在下一次迭代中
  被同等信任,直到 human review 介入。

### 为什么这是下一量级的瓶颈

ForgeOS 的长期价值主张是「24h 无人值守自治」。没有身份和权限模型:
1. **安全基线不成立**: 一个 agent 被 prompt injection 后可模拟任何其他 agent,
   包括具有高危操作的 fns (如果将来 Sandbox 接入了 `git push` / `deploy`)。
2. **审计链缺失关键维度**: trace 记录了 `kind:"agent"` 和 `name` (phase name),
   但不记录调用者身份,无法回答「谁发起了这次写文件操作」。
3. **责任分界不清晰**: reviewer 的 `VERDICT: REQUEST_CHANGES` 触发的 loop-back
   是纯文本契约,假如 implementer 的输出不小心包含了 `VERDICT:` 前缀,可导致
   评审循环误触发——因为没有机制验证该 verdict 的来源确实是 reviewer 角色。

### 建议切入路径(不做代码)

1. **Agent 身份令牌**: 每个 agent phase 启动时生成一个进程级 UUID (注入到 env),
   随 observe sink 传给 trace,使每条 agent 事件可追溯到具体角色。
2. **权限声明文件**: 在 `.agent/agents/*.md` 中增加 `allowed_tools:` 和
   `allowed_gates:` 元数据字段,运行时会按角色过滤工具白名单。
3. **信誉分聚合**: 在 scorecard 的 `/routing/scorecards.json` 中增加按 agent 角色
   而非仅按 model 聚合的 quality_score,让路由可拒绝低信誉角色。

---

## 方向二:知识生命周期管理 (Memory RAG Pipeline)

### 代码级证据

- `internal/memory/memory.go` 的 `Compact()` 和 `Prune()` 函数**完整实现但零调用**。
  从入口 `cmd/forge/evolve.go` 到 `cmd/forge/main.go`, 没有任何位置触发 compaction。
  只有 cmd/evolve 的 `cmdMemoryPrune` 是显式 CLI 子命令——意味着 operator 不手动
  运行 `forge memory-prune` 就不会有 compaction。
- `internal/prompt/retrieve.go` 的 `Retrieve()` 是纯关键词 TF-IDF 变体:
  - 没有语义检索
  - 没有跨 session 复用
  - 没有 relevance feedback 闭环
- 同一个文件的 `tokenize` 用 `FieldsFunc` 做 unicode 分词: 中文/日文等 CJK 文字
  不会被正确分词,因为 CJK 字符既是 letter 又没有空格分隔。这意味着所有中文 ADR
  标题对检索器完全不可见。
- `internal/memory/memory.go` 的 `filterSuperseded`: 依赖手动设置 `Supersedes` 字段,
  没有自动的近重复检测——迭代 5 发现的 gap 与迭代 3 相同的 gap 不会被自动标记为
  superseded,导致 memory 无限膨胀。

### 当前体系的能力边界

- memory.jsonl 是纯 append 日志,没有生命周期管理。
- 8 次 evolve 迭代后按 `DefaultCompactThreshold=500` 触发 compaction,但 Compact 函数
  从未被 loop 调用——意味着知识库只增不减,长期运行后 prompt 注入的 memory 条目越来越多,
  token 消耗线性增长。
- 知识条目没有 freshness 衰减: 迭代 1 的 `KindLesson`（"单元测试用 assert 不是
  should"）在迭代 50 仍然和最新条目一样权重,agent 被过时信息干扰。
- CJK 检索完全不可用。

### 为什么这是下一量级的瓶颈

ForgeOS 的核心差异化之一是 `memory` 包——号称「让 24h 运行不健忘」。但一个只增不减、
不衰减、不分层、不自动去重的知识库,在真实 24h + 100 次迭代的运行中会:
1. **prompt 膨胀失控**: 每次 gather signals 注入 knowledge, 500+ 条目中大部分是
  过时/重复/矛盾的噪声,浪费 token 预算。
2. **收敛困难**: 旧决策覆盖新决策但无 supersedes 标记,agent 同时看到两条冲突指令。
3. **CJK 项目无法使用**: 中文 ADR 标题的 TF-IDF 检索得分为 0——等价于没有检索。

### 建议切入路径(不做代码)

1. **Compaction 接入 LoopEngine**: 在 LoopEngine.Run 的 `onIteration` hook 中加入
   当 memory 条目数超过阈值时触发 `memory.Compact`——使 compaction 成为 loop 生命周期
   的一等行为,而非 operator 手动 CLI。
2. **Freshness 衰减 + 置信度衰减**: 为 `memory.Entry` 增加 `decayWeight` 方法,
   在 `memory.Query` 中对超过 `recency_half_life_days` 的条目自动降权(已有
   scorecard 的 `decayWeight` 模式可复用)。
3. **近重复检测**: 在 `memory.Append` 前用 edit distance / token overlap 扫描已有条目,
   命中相似度阈值自动设置 `Supersedes`。
4. **CJK tokenizer**: 在 `tokenize` 中 fallback 到 bigram (CJK 双字切分),
   使中文 ADR 标题获得非零检索分数。

---

## 方向三:确定性回放与回归测试框架

### 代码级证据

- `internal/trace/trace.go` 的 `Tracer.Emit` 写 JSONL 记录完整的事件流:
  iteration 边界 · gate 裁决 · agent phase · cost/latency · converge 检查。
- 但整个系统没有**回放引擎**: 无法读取一次 trace.jsonl 然后重放通过 Engine,
  以测试改了 policy/harness 后的行为是否一致。
- 当前回归测试套件 (`go test ./…` + harness 测试) 覆盖了:
  - 纯函数 (routing/risk/mode/memory/trace/converge)
  - 带 mock 的 engine (orchestrator/orchestrator_test.go 的 fake RunGate)
  - 集成测试 (test_acceptance.mjs 的 real ACCEPTED)
  - **但不覆盖**: 一个真实 trace 回放通过修改后的 gate 逻辑/loop 参数后的行为变化。
- `internal/persist/checkpoint.go` 已经保存了每次迭代的 `Reason` / `RoadmapCompletion`,
  它是回放恢复的起点——但缺少「用旧 checkpoint + 旧 trace 喂给新 Engine」的测试工具。
- `internal/orchestrator/orchestrator_test.go` 的 `fakeAgent` 返回固定输出,能够测
  engine 的状态机但测不到真实 agent 输出对下游相位的影响——因为没有一个回放/录制模式。

### 当前体系的能力边界

- 修改 `Engine.MaxLoopBack` 从 3→5,当前的测试只能验证「loop-back correctly jumps」,
  无法验证「在真实历史 trace 上 loop-back 3→5 是否会让一个原本失败的真实运行收敛」。
- 修改 `mode.Effective` 的 gate-set 过滤逻辑,没有 benchmark 测试「这个改动在过去
  10 个真实运行的 trace 上产生的 gate 裁决是否相同」。
- 增加新的 `proc.Signal` 类型到风险分类器,没有办法验证它对历史运行的路由决策影响。

### 为什么这是下一量级的瓶颈

ForgeOS 的一个核心主张是「策略即数据」——policy/modes/routing 都是声明式数据文件。
但策略的变更验证完全依赖昂贵的真实 agent 调用（`--executor=command --agent-cmd claude`
每次烧真实预算）。没有回放回退:
1. **策略变更风险高**: 改一行 modes.yml 的 `gates:` 子集,只能靠 code review 判断
   是否改变了预期行为——没有自动化回归套件。
2. **debug 困难**: 生产运行如果 gate 意外 FAIL,没有方法在本地重放那段 trace 来复现。
3. **收敛算法不可演进**: 修改 staleCount 的双轴逻辑(当前检查 roadmap 和 gatesGreen)
   无法在历史数据上验证不会造成 false-positive tripwire 触发。

### 建议切入路径(不做代码)

1. **Replay Engine 接口**: 一个 `Replay(tracePath string, eng Engine) (got, want []Result)`
   函数,读 trace.jsonl,依次调用 Engine.RunGate 和 DryRunExecutor,比对实际 gate 裁决
   与 trace 中记录的裁决是否一致。不一致处报告 diff。
2. **trace fixture 库**: 将 S25/S26 真 claude 运行的 trace.jsonl 作为版本化测试 fixture
   提交到仓库(`testdata/traces/`),作为 policy 变更的回归基线。
3. **Checkpoint + Trace 联合回放**: 从 checkpoint 恢复状态,从 trace 重放后 N 次迭代,
   验证「如果上次运行的 phase 3 用了更新后的 prompt,verdict 是否不同」。

---

## 方向四:多仓治理与组织级控制平面

### 代码级证据

- **每个项目完全自治**: `forge-init` 复制全套 `.agent/` + harness + CLAUDE.md 到新目录。
  治理资产(agents/skills/workflows/policies)是每个 repo 的本地拷贝,没有共享/继承机制。
- `internal/memory/memory.go` 的 `loadCaches` 是 `sync.Map` keyed by 文件路径:
  同一台机器上的两个不同项目的 forge 进程各自缓存各自的项目——没有跨项目记忆共享。
- `internal/routing/scorecard.go` 的 `scorecards.json` 是单仓的:
  - 没有跨仓聚合
  - 没有「来自全局经验」的冷启动推荐
  - 每个项目从零积累历史,冷启动时 `min_samples=20` ≈ 20 次 agent phase 后才有
    历史择优信号(约 $10–50 的磨损)
- `internal/prompt/cache.go` 的 `ContextCache` 只缓存本仓的 ADR + AGENTS.md:
  不能引用「另一个共享仓的架构决策」。
- `harness/scaffold/forge-init.mjs` 的 forge-init 复制文件时,所有路径都是相对于目标仓的
  本地路径——没有创建指向共享治理包的 symlink/submodule 的选项。
- `.agent/architecture/north-star.md` 第 6 条说「治理为独立平面(PDP/PEP 分离,OPA 式)」,
  但当前的 PDP (policies/modes.yml) 和 PEP (internal/mode + internal/routing) 完全
  耦合在单仓的本地路径下。

### 当前体系的能力边界

- 组织内有 10 个微服务,每个用 `forge init` 启动:
  - 修改 `AGENTS.md` 的一条红线需要在 10 个仓分别 PR
  - 没有全局的 `policy.yml` 统一推送
  - 每个服务的 scorecard 是孤岛,无法回答「哪个模型在 Python 微服务上表现最好」
- `forge migrate --to engineering` 只升级单仓——组织迁移需要逐个仓手动执行。
- ADR 跨仓引用不可追溯: 仓 A 的 ADR 不能被仓 B 的 prompt 检索到。

### 为什么这是下一量级的瓶颈

ForgeOS 的目标是「AI-native 软件工厂」,不是单项目管理工具。当组织超过 3–5 个仓:
1. **治理碎片化**: 各仓的 policy 快速漂移,失去统一标准。
2. **学习飞轮不转**: 仓 A 的经验(一个 crash 后发现的 prompt pattern)不会自动
   惠及仓 B——每个项目冷启动。
3. **运营成本线性增长**: 每个仓独立升级 forge-core / harness / policy / agent cards。
   没有控制平面,运维负担随仓数线性增长而不是收敛。

### 建议切入路径(不做代码)

1. **全局 Scorecard 聚合端点**: 一个 `forge scorecard --publish` 命令,将本仓的
   scorecards.json 推送到中央存储(以文件或 URL 形式),和 `forge route --global` 在
   本地样本不足时回退至全局冷启动数据。
2. **`.agent` overlay 机制**: `project.yml` 支持 `extends: git@github.com:org/shared-governance`
   字段,在本地 `.agent/` 未定义的键上 fallback 到远程仓(ADR 0003 设计就绪,等待
   触发条件和远程仓位置批准)。
3. **Policy 版本化契约**: `mode.Effective` 等核心策略函数增加版本号校验,
   当远程推下新 policy 时本地能检测版本不兼容并拒绝运行(而非静默错误解释)。

---

## 方向五:并行编排的失败语义与资源核算

### 代码级证据

- `internal/orchestrator/parallel.go` 的 `runWave` 用 `sync.WaitGroup` 等待
  波内所有 phase 完成后再检查 `firstErr`。这意味着:
  ```
  波 2 (5 并发 phase):
    phase A: gate FAIL @ 2s  →  waveCancel()  ✅ (context cancelled)
    phase B: agent 已跑 30s  →  `os/exec` 不会被 SIGKILL,会跑到超时 ❌
    phase C: agent 刚启动   →  同上 ❌
    浪费: 4 × 典型 phase 成本 + 等待时间
  ```
- 同一个文件的 `runPhaseParallel` 中,`checkAgentBudget` 和 `checkRunBudget`
  在锁内执行(`mu.Lock()`),但 agent spawn (`runAgentPhase`) 在锁外——这是正确的,
  但 budget 耗尽时已有 agent phase 在外运行,它们已消耗的成本无法回收。
- `LoopEngine.Run` 中 parallel 模式显式禁用 `OnPhase`(文件头注释):
  `forge evolve --parallel --resume` 从 crash 恢复时只会从 iteration 边界续跑,
  丢弃已完成的所有 agent phase 成果。
- `internal/orchestrator/parallel.go` 文件头已有完整的 **LOCK ORDER CONTRACT**
  (8 级锁顺序),但没有编译器/静态检查来强制执行——违反锁顺序会导致 schedule-dependent
  deadlock (Heisenbug)。
- `internal/orchestrator/waves.go` (并行依赖波计算) 没有依赖验证: 如果两个 phase
  互相声明 `depends_on: [each_other]`,`Waves` 会返回 err(在测试中),但生产环境
  的 `Waves` 没有熔断机制——一个循环依赖会直接 panic 或 OOM。

### 当前体系的能力边界

ForgeOS 的并行编排(方向五)是为 Discover fan-out / 多 implementer 而设计的核心性能特性,
但目前:
1. **Fail-fast 是伪语义**: context cancellation 只阻止**未开始的** phase,
   已经在运行的 agent 不会被中止。这浪费真实预算。
2. **Resume 退化为全量重放**: `--parallel --resume` 未实现 `RunParallelFrom`。
3. **锁顺序是纸契约**: 8 级锁的获取顺序写死在注释里,但 Go 编译器不检查。
4. **依赖图循环无熔断**: `Waves()` 的循环检测正确但缺少拓扑排序失败的降级策略
   (如 fallback 到串行)。

### 为什么这是下一量级的瓶颈

当 ForgeOS 真正并行跑 5 个 implementer 时(Discover 的 scan/market/capability 或
多模块并行编码):
1. **成本不可控**: 一个 gate FAIL 后,波内剩余 4 个 agent 继续跑满 30s+,
   每次浪费 $2+。当并行波每小时触发一次,每天 $48+ 消耗在丢弃的工作上。
2. **恢复时间长**: Crash 后 --parallel --resume 重跑整轮迭代(5 phase × 30s ≈ 2.5 分钟)
   而非从断点恢复(~0)。对于 24h 自治运行,每次 crash 损失 2.5 分钟 × 每日 crash 次数。
3. **死锁风险**: 锁顺序纸契约不可执行。引入新并发状态时(如第 9 级锁),只能靠
   `-race` 测试随机捕捉——不是 engineering 级别的保证。

### 建议切入路径(不做代码)

1. **真正的 fail-fast**: 将 `runAgentPhase` 接入 wave 的 `context.Context`——
   `CommandExecutor.Execute` 已经用了 `exec.CommandContext`,只需要把 waveCtx 传入
   agent phase 的引擎链,避免已 signal 的 agent 跑完剩余时间。
2. **RunParallelFrom**: 实现一个接受 `startPhase` 参数的并行入口——从 checkpoint
   恢复的 phase 索引转换为第一个需要运行的波索引,跳过后面的波。
3. **锁顺序静态验证**: 在 `sync.Mutex` 上加薄封装 `LeveledMutex`(带 int level),
   在 `Lock()` 时 assert 当前 goroutine 未持有更高 level 的锁——运行时(测试中)捕获
   违反而非等调度死锁。
4. **依赖图熔断**: `Waves()` 在检测到循环依赖时,不是 panic 而是返回一个
   `(waves, warnings)`——警告日志且 fallback 到串行执行,保证生产不崩溃。

---

## 汇总:优先级与依赖关系

| 方向 | 影响面 | 成本节约 | 安全影响 | 对自治运行的关键度 | 前置依赖 |
|------|--------|----------|----------|-------------------|----------|
| ① Agent IAM | 架构安全 | 中(防注入泄密) | 🔴 高 | 🔴 高(无身份则无审计) | 无 |
| ② Memory RAG | 运行质量 | 高(token 降耗) | 🟢 无 | 🟠 中(10+ 迭代后固化) | 无 |
| ③ 回放框架 | 开发效率 | 高(减少真 agent 测试) | 🟢 无 | 🟠 中(policy 演进依赖) | 无 |
| ④ 多仓治理 | 组织扩展 | 高(消除运维线性增长) | 🟠 中 | 🟢 低(单仓可用) | ADR-0003 |
| ⑤ 并行语义 | 执行效率 | 高(减少浪费开支) | 🟢 无 | 🟠 中(并发 workflow 时) | waves.go |

**推荐启动顺序**: ② Memory RAG compaction 接入 → ③ 回放框架 → ① Agent IAM →
⑤ 并行失败语义 → ④ 多仓治理。理由: ② 和 ③ 无前置依赖且直接提升现有单仓自治运行
的可靠性,是「止血型」性价比最高的起点。
