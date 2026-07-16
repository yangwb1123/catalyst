# ForgeOS — 基于代码扫描的扩展方向分析

> 本文基于对当前代码库(forge-core 18 Go 包 + harness 工具集 + pi-batch.py)的逐文件扫描,
> 从资深架构师/产品经理视角提出 **5 个高价值扩展方向**。
> 每个方向包含:为什么需要、当前代码中的具体缺失、边界情况(Edge Cases)、
> 以及性能考量。**不编写任何代码,只分析。**

扫描基准:2026-07-11,forge-core 零外部依赖,18 Go 包(12 internal + 6 cmd+internal 增量),
harness 约 20 JS/Py 文件,examples/ 两个 dogfood 应用。

---

## 方向一:跨会话记忆语义化 & 知识压缩引擎

### 为什么需要

当前 `internal/memory` 包是一个**纯追加 JSONL 日志**:每次 Append 写一行,Load 全量读到内存,
Query 做简单的 map 过滤。这有三个根本问题:

1. **无界增长** — memory.jsonl 只增不缩,没有 TTL、没有 compaction、没有 pruning 策略。
   `Compact` 函数存在(`memory_compact.go`)但语义是「保留每个 kind 最近 N 条」,不是
   按信息价值/时效性做分级压缩。一个 24h 自治运行的 memory 文件可达数万行,
   Load 会变慢,且大量低价值条目(如重复的「未找到 gap」)稀释了真正有用的知识。

2. **查询只有平面 map 过滤** — `Query` 函数接受 `filter func(Entry) bool`,
   没有语义检索。当前 `prompt/retrieve.go` 中有 TF-IDF 检索器,但 memory 包**不消费它**——
   这意味着 memory 中的知识条目无法按相关性排序/筛选注入 prompt,只能全部注入或什么都不注入。

3. **跨会话归零** — 当前 memory 在 `forge evolve` 每次迭代末尾 Append,但跨 session
   (一次 `forge run` 完成后,下一次从头开始)没有「加载历史 memory → 压缩提炼 →
   注入初始 prompt」的机制。LoopEngine 的 `LoopMemory` 字段(`orchestrator/loop.go`,
   行 80+)只是 `func() []memory.Entry` 的注入点,但 cmd/forge 的 `evolve.go` 中
   对应的接线(`storeEvolvingMemory`/`loadEvolvingMemory`)是每次迭代 Load 全量——没有增量提炼。

### 具体发现

| 位置 | 问题 |
|---|---|
| `internal/memory/memory.go` | `Load` 读取全部 JSONL,无 streaming/分页 |
| `internal/memory/memory_compact.go` | `Compact` 按 kind 保留最近 N 条,但不做语义去重 |
| `cmd/forge/evolve.go` `buildLoop` | 每次迭代加载全量 memory,不改 |
| `internal/memory/memory.go:150-170` | `loadCache` 每个 path 缓存一个条目,但 Append 后 invalidate,实际上每次迭代都重新读盘 |
| `internal/prompt/retrieve.go` | TF-IDF 检索器只用于 ADR,不通 memory |

### 边界情况

- **memory 文件损坏**:当前 Load 遇到一行 JSON 损坏直接抛错(诚实策略),但损坏行之后的
  所有知识都不可达。一个高价值扩展是「可恢复加载」:标记损坏行,返回剩余可解析条目
  + 告警,而不是全量拒绝。
- **超大 entry**:如果 agent 写了一个数万 token 的总结条目,全量 Load 可能导致 OOM。
  需要 entry-level 大小上限 + streaming 加载。
- **并发 Append**:当前用 O_APPEND + 单行写入,多进程并发 Append 不会交错行,但
  Load 可能在另一个进程写入半行时读到不完整 JSON——需要读时跳过不完整尾行(类似
  `trace/trace.go` 的 `lastLine` 检查,但 memory 没有这个守卫)。
- **memory 与 checkpoint 的相对时序**:如果进程在 Append memory 之后、Save checkpoint
  之前崩溃,恢复时会丢失 memory 条目(因为 checkpoint 回退到旧状态但 memory 文件已追加)。

### 性能考量

- **延迟**:每次迭代 Load 全量 memory(O(N))随着运行时长线性增长。N=10000 时一次 Load
  约 100-200ms,不影响 agent 延迟;N=100000 时约 1-2s 成为可感知开销。
- **I/O 放大**:`loadCache` 试图缓解但被每次 Append 的 invalidate 抵消——本质上是
  每次写后读缓存失效。
- **推荐方案**:分片存储(按 kind 分文件) + 增量加载(只加载上次 checkpoint 之后的条目)
  + 后台压缩合并(类似 LSM-Tree 的思路,但用 JSONL 格式)。

---

## 方向二:声明式弹性质量策略语言

### 为什么需要

当前质量策略(quality policy)分散在多个位置,且都是**硬数值**:

- `modes.yml` 的 `harness.coverage_threshold: 80` — 全项目单一阈值
- `harness/policies.yml` 的 `max_file_lines:500` / `max_function_lines:50` — 全局硬编码
- `internal/gate/resolve.go` 的 `resolveGate` 按 lifecycle 查 modes.yml 的 coverage 阈值,
  但不区分代码路径
- `internal/routing/routing.go` 的 `agentTier` — 按 agent 角色固定,不因代码语义变化

问题:所有质量策略是「全局一刀切」。没有:

1. **路径级策略**:`payment/*.go` 需要 90% 覆盖率但 `cmd/*.go` 只需要 50%
2. **生命周期演进策略**:覆盖率阈值随 lifecycle 上升(idea:0% → mvp:60% → growth:80% → production:90%),
  但函数长度/文件大小阈值应下降
3. **差异化回退**:不同 gate 对不同代码路径有不同的「最小通过标准」——安全 gate 对 payment
  代码是 blocking,对 CLI 代码是 advisory
4. **测试健康复合指标**:coverage + test-to-code-ratio + test-freshness 三者共同决定
  gate 是否通过,而非单一覆盖率数字

### 具体发现

| 位置 | 当前行为 | 缺失 |
|---|---|---|
| `internal/gate/resolve.go:120-150` | `ResolveGate` 查 modes.yml 的 coverage_threshold,单一值 | 无路径/领域维度 |
| `internal/converge/converge.go` | `Signals.CodeTestRatio` 被收集但不 gate-breaking | 已计算但无策略消费 |
| `cmd/forge/gates.go:290-310` | `computeCodeTestRatio` 全项目统一算 | 不能 per-path |
| `internal/doctor/doctor.go` | health check 不检查测试覆盖率趋势 | 无质量衰减探测 |
| `harness/policies.yml` | `max_file_lines` / `max_function_lines` 全局常数 | 不能按 lifecycle 衰减 |
| `internal/routing/routing.go` | `opusFloorAgents` 硬编码 | 不能按代码风险动态调整 |

### 边界情况

- **策略冲突**:如果 payment 路径要求 90% coverage 但安全 gate 在该路径上 N/A
  (工具不支持),哪个优先?需要声明式优先级语义。
- **策略衰减**:项目从 mvp→growth 时,覆盖率阈值从 60→80,但遗留代码不应该被新策略
  瞬间阻挡——需要「渐进式实施窗口」(例如:新策略生效前 3 个迭代为 warn 模式)。
- **跨语言策略**:Go 的覆盖率测量方式与 TypeScript 不同,策略语言需要感知语言。
- **策略可测试性**:策略本身应该可以被单元测试验证("assert that payment path
  coverage gate is blocking for production lifecycle")。

### 性能考量

- 策略解析本身不应该是性能瓶颈(O(1) 或 O(N_paths) 解析,缓存化)。
- 策略执行应在 gate 层,不在 phase 执行路径上。
- 策略 DSL 编译成 Go 代码或 JSON schema,避免运行时 YAML 解析开销。

---

## 方向三:项目级并发安全与状态锁

### 为什么需要

当前 forge-core **完全没有**跨进程并发安全防护:

1. **checkpoint.json 竞态**:两个 `forge run` 同时在同一个项目上执行,都调用
   `persist.Save` → 原子 rename 保证不会损坏文件,但第二个会静默覆盖第一个的
   进度——没有检测、没有隔离、没有告警。

2. **trace.jsonl / memory.jsonl 交错**:O_APPEND 写入保证单行原子性,但两个进程
   的 trace 事件会在同一个文件里交错出现,下游工具无法区分流。

3. **gate 缓存失效**:`cmd/forge/gates.go` 的 `loopProbe`(per-iteration probe cache)
   是进程内缓存,一个进程的 probe 结果不影响另一个。但同时跑两个 `forge run` 时,
   各自跑各自的 gate,互不知道对方的存在。

4. **无项目锁定机制**:没有 pidfile、没有 flock、没有 etcd lease——任何级别的
   互斥都不存在。

### 具体发现

| 位置 | 问题 |
|---|---|
| `internal/persist/checkpoint.go:Save` | 原子 rename 只防单进程崩溃,不防多进程覆盖 |
| `internal/trace/trace.go:Emit` | 无进程级锁,两个 forge 实例写同一 trace.jsonl |
| `internal/memory/memory.go:Append` | O_APPEND 安全但无归属标记 |
| `cmd/forge/gates.go:loopProbe` | 进程内 cache,无跨进程协调 |
| `internal/gate/resolve.go` | 纯函数,不感知调用者身份 |
| `cmd/forge/main.go:cmdRun` | 无 `--lock` / `--exclusive` 语义 |

### 边界情况

- **死锁**:如果进程 A 持有文件锁然后崩溃,锁需要超时释放(flock 在进程终止时自动释放
  但 NFS/分布式 FS 上行为不同)。
- **只读执行**:`forge status` / `forge doctor` 应该能**读**状态而不获取写锁。
  `forge run` / `forge evolve` 需要排他写锁。
- **GitOps 模式**:CI 中多个 job 可能同时对同一 repo 运行不同的 forge 命令(例如
  `forge gate` 和 `forge accept`),需要区分「读锁」和「写锁」。
- **跨容器/跨机器**:如果 forge-core 被容器化(未来 v3 Sandbox),项目状态可能挂在
  网络存储上——需要分布式锁(etcd/consul)或乐观锁(版本号检查)。

### 性能考量

- 文件锁(flock/LockFile)对单机场景足够,开销约 1ms。
- 分布式场景需要 lease-based 锁,开销约 10-50ms。
- 大部分 forge 操作是秒级的(agent 执行),锁开销可忽略。
- 关键是不在 hot path(内存操作)上加锁——仅在磁盘写前后加。

---

## 方向四:自适应 Prompt 预算与渐进式上下文披露

### 为什么需要

当前每次 agent phase 构建 prompt 时,**全量注入**所有上下文:

- `prompt.Build` 注入角色卡 + project context(ADRs + AGENTS.md 约束 + ROADMAP 任务)
- `prompt_context.go:buildPrompt` 额外注入 gate ledger + phase output + memory + artifacts
- `prompt/cache.go` 只缓存本地 I/O,不节省 API token

这意味着:

1. **每个 phase 支付全量 token 成本**——在真点火(`claude --model sonnet`)场景下,
   每次 prompt 都可能数千 token,对 5-phase build 流程 × 多迭代 = 数万 token 只是上下文。

2. **没有「已知道」优化**——phase 2 的 prompt 不需要重复 phase 1 已经注入的 ADR 全量。
   claude API 支持 `cache_control`(提示缓存),但 forge-core **没有利用**它。

3. **没有 prompt 分层**——高 tier 模型(opus)收到和低 tier 模型(haiku)**完全相同**的
   prompt 内容,只是模型不同。但 opus 的费用是 sonnet 的 ~5x、haiku 的 ~15x,
   应该收到更精简的高价值上下文(而 haiku 做简单 CRUD 不需要完整 ADR 清单)。

### 具体发现

| 位置 | 当前行为 | token 浪费估计 |
|---|---|---|
| `internal/prompt/prompt.go:Build` | 全量角色卡 + context | N 个 phase 重复 N 次相同内容 |
| `internal/prompt/cache.go` | 仅缓存文件读取,不缓存 prompt 文本 | 0 token 节省 |
| `cmd/forge/prompt_context.go:buildPrompt` | 每次重新拼接完整 prompt | 100% 重复 |
| `cmd/forge/prompt_memory.go:memoryContext` | 全量 memory 注入 | 随迭代增长 |
| `internal/prompt/retrieve.go` | TF-IDF 只用于 ADR 选取,不用于上下文裁剪 | ADR 部分节省但其他不 |
| `internal/routing/routing.go:TierFor` | 路由选模型但不影响 prompt 内容 | opus 得到和 haiku 相同的 prompt |

### 边界情况

- **prompt 缓存失效**:ROADMAP.md 被 agent 写入后,缓存必须立即失效。
  当前 `cache.go` 的 Rationale 段明确说了「no ROADMAP field」来防止缓存过期——
  这个设计是对的,但需要扩展到**所有** agent-writable 的上下文源。
- **cache_control 与 vendor 锁定**:claude 的提示缓存 API 是 Anthropic 特定的。
  跨厂商时(LiteLLM/v3)需要通用抽象层——缓存提示前缀但标记格式可能不同。
- **prompt 裁剪的 honesty**:必须让 agent 知道「部分上下文被裁剪了」,
  避免裁剪掉关键约束后 agent 做出违规决策。
- **渐进式披露的 round-trip**:如果 phase 1 没有披露某个 ADR,phase 2 发现需要它,
  那 phase 1 的产出可能需要重做——这引入了「撤回」语义。

### 性能考量

- **token 节省潜力**:5-phase build × 3 iterations = 15 次 prompt。如果每个 prompt
  节省 2K token(上下文重复部分),以 sonnet $3/M input token 计算,节省约 $0.09。
  看似小,但 24h 自治运行数百 iteration 时**累积显著**。
- **cache_control 的 API 开销**:设置 cache_control 标记是零额外延迟的;
  缓存命中的延迟节省通常 50-70%(API 侧)。
- **关键设计决策:token vs quality tradeoff**——裁剪上下文可能降低输出质量,
  需要接入 eval/scorecard 系统来验证裁剪策略的效果(引入实验框架)。

---

## 方向五:Gate 增量计算与分布式执行骨架

### 为什么需要

当前每次 `forge gate` / `forge run` 都**全量重跑所有 gate**:

- `gate.ProbeAll` 运行所有 probe(lint/build/test/complexity/arch/security)
- `harness/gate.mjs` 扫描**全部**源码文件
- `harness/arch/arch-check.mjs` 扫描全部源文件做 8 检查
- `internal/risk/risk_diff.go:FromChangedPaths` 每次 git diff 全量计算

这是**正确的**行为——全量验证确保没有遗漏——但对于大项目(数万文件)这成为瓶颈。

更根本的问题是:ForgeOS 缺乏**增量计算骨架**:

1. **没有 change-based gate memoization**——如果只有 `src/domain/` 下的文件变了,
   不需要重新 `npm run build` 或 `go build ./...`。
2. **没有 gate 依赖图**——某些 gate 可以并行(如 lint + arch-check 互不依赖),
   但有些 gate 依赖先跑 test 才能算 coverage。当前全部串行。
3. **没有 gate 缓存分层**——`harness/arch/scan.mjs` 的 AST 扫描结果应该可以跨
   invocation 缓存(与上次的 git diff 比较,只重扫有改动的文件)。
4. **secret-scan 可以增量**——只扫描新增/修改的文件(当前扫全部)。

### 具体发现

| 位置 | 当前行为 | 浪费 |
|---|---|---|
| `internal/gate/gate.go:ProbeAll` | 全量执行 6 个 probe | 无变更时 100% 浪费 |
| `harness/arch/arch-check.mjs` | 扫描全部源文件(约 200+ 文件) | ~100ms/次 |
| `harness/arch/scan.mjs:scanFiles` | 对所有 Go 文件做 AST 解析 | 无增量 |
| `harness/secret-scan.mjs` | 扫描全部文件 | 每次重扫 |
| `harness/gate.mjs` | 读取 policies.yml 后按时间排序最近修改的文件 | 只用于排序,不做增量 |
| `internal/risk/risk_diff.go` | 每次全量 git diff | O(N_changes)不节省 |

### 边界情况

- **变更检测的精确性**:git diff 只能检测 tracked 文件的变更。新文件(outside git tracking)
  或 `.gitignore` 变更后的文件可能被遗漏。需要额外扫描未追踪文件。
- **缓存失效的保守策略**:任何 `.agent/` 或 `harness/` 文件的变更应该 invalidate
  所有 gate 缓存(因为治理策略变了)。
- **分布式执行**:当 gate 可以并行时,未来的分布式执行器可以把独立 gate 派发到不同
   worker——但需要 gate 的依赖声明和结果聚合语义。当前 `orchestrator/parallel.go`
  的 `depends_on` 机制是精确的先例。
- **跨语言增量**:Go 的 test cache 是内置的,但 JS/Python/Rust 的 test runner 没有
   forge-core 级别的缓存协调。需要语言适配器报告「测试是否命中缓存」。
- **partial gate 结果语义**:如果 lint gate 增量执行(只扫 changed files)但其他 gate
  全量,`gatesGreen` 的语义需要精确定义——「所有已检查的 gate 全绿」vs.
  「所有 gate 全量全绿」。当前语义倾向于后者,安全但不经济。

### 性能考量

- **大项目实证**:在 10 万文件规模下,全量 arch-check (~1s)和全量 secret-scan(~5s)
  累积可达数十秒。增量可将 <=1s。
- **缓存存储**:增量计算需要持久化缓存(上次的 AST 快照、文件哈希等)。
  `internal/persist` 的原子写可以复用,但需要新的缓存数据结构和失效策略。
- **与 orchestrator 的集成**:增量 gate 不应该阻塞 phase 执行——可以在 agent 运行时
  后台预热 gate 结果,Sprint 22 的「真点火安全护栏」证明 agent 执行时间占主导,
  有足够的后台窗口做全量或增量 gate 计算。
- **建议渐进策略**:先做不需要状态缓存的「change-based gate skip」(如果只有 .md
  文件变更,不跑 build/lint/secret),再做带缓存的增量 AST 扫描。

---

## 总结:优先级建议

| 方向 | 价值 | 实现成本 | 建议时机 |
|---|---|---|---|
| ① 记忆语义化 & 知识压缩 | 中(自治运行耐力) | 中(需持久化格式变更) | 下一个大版本迭代 |
| ② 声明式弹性质量策略 | 高(治理层核心能力) | 中低(DSL + 新包,复用已有 gate 框架) | **建议立即启动** |
| ③ 项目级并发安全 | 高(生产就绪屏障) | 低(flock + pidfile,几小时实现) | **建议立即启动** |
| ④ 自适应 Prompt 预算 | 中高(token 成本节省) | 中(cache_control + 分层 prompt) | 真点火大规模采用后 |
| ⑤ Gate 增量计算 | 高(大项目扩展性) | 高(缓存层 + 变更检测 + 依赖图) | 作为性能工程专项 |

**最容易开始的方向**:③ 并发安全(低风险、高收益、独立可验证)
**最高长期杠杆**:② 弹性质量策略(对齐 ForgeOS 的声明式治理哲学核心)
**最需要谨慎**:④ prompt 预算(涉及输出质量与成本 tradeoff,需要 eval 验证)

> 本文档不编写任何代码,仅提供架构级分析和决策参考。
> 所有发现均基于 2026-07-11 代码库的实际实现状态。
