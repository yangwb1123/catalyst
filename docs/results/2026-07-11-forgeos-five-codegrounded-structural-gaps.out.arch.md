Now I have a thorough understanding of the project. Let me synthesize my architectural analysis.

---

# 架构分析：ForgeOS 五个结构性扩展方向

> **分析基础**: `2026-07-11-forgeos-five-codegrounded-structural-gaps.md` 的原始分析（~36KB） + 基于 HEAD `b0c80e4` 的逐方向代码验证结果（.out.md） + 项目全貌（BOOTSTRAP/.agent/forge-core/harness/32 个 Sprint 演进记录）
>
> **角色**: 架构师视角,不写代码,只做结构决策评估

---

## 1. 架构评估

### 1.1 当前架构的优势

ForgeOS 是我见过自我治理最严格的 AI 工程系统。其架构优势与它自身的开发纪律高度耦合：

**a) 脊柱清晰度（Idea → Production）**
五个阶段（Discover→Design→Build→Evolve）被编码为可执行的工作流 YAML + Go 编排引擎。每个阶段有明确的 stop 条件和信号契约。这不是概念架构——是已经过 32 个 Sprint、真 Claude 端到端坐实的代码。

**b) 中枢旋钮（mode × lifecycle）**
一个设置同时驱动 Router 档位、Harness 严格度、Workflow 深度三处——这是极高杠杆的设计决策。`production` lifecycle 一票否决 override 机制防止宽松 mode 绕过安全执法，这是一个正确、务实的 fail-closed 设计。

**c) 载重墙模式（host-independent enforcement）**
不依赖任何特定 CLI 的 hook/plugin 机制做真相之源，而是用带外执法（Sandbox/CI runner）跑 harness 闸门。这是一个根本性的正确决策——它让 ForgeOS 的治理承诺不随宿主变更而减弱。

**d) 零外部依赖纪律（Go 标准库）**
`go.mod` 无 `require`——13+ Go 包全部纯标准库。这在开源项目中几乎独一无二，带来的好处是：
- 无 supply-chain 攻击面
- `go build/vet/test -race` 可在任意 Go 版本上瞬间完成
- 极大降低心智负担（新贡献者无需理解复杂依赖图）

**e) Fresh-context Reviewer 纪律**
「实现者不审自己的代码」被编码为不可绕过的架构约束。这是防止「自信虚构」的最有效机制——ForgeOS 的 32 个 Sprint 中，每一次多轮 fresh review 都揪出过真实 bug（block-scalar 损坏、production override 旁路、死代码标记等）。

### 1.2 当前架构的局限性

优势与局限是一体两面：

**a) 单实例假设（与 vision 的冲突）**
当前 forge-core 的所有持久化——checkpoint（`persist`）、trace（`trace.go`）、memory cache（`sync.Map`）——都假定**同一时间只有一个进程**在写 `.forge/`。而 vision 的「24h 自治工厂」天然需要多实例（CI + 开发者本地同时跑；或多个 evolve 并行探索不同方向）。这不是一个「可做可不做」的特性——vision 要求多实例，但代码层还没有为此准备。

**b) 信号神经的脆弱性**
整个自治循环的**决策转折点**——是否继续（VERDICT: APPROVE）、是否需要修改（REQUEST_CHANGES）、置信度是否足够（CONFIDENCE: ≥80）——依赖于三个独立的、精确字符串匹配的「末行解析器」。这是体系架构中信号最密集的神经中枢，但却是最脆弱的部分。一个模型升级（如 Claude 自动在输出末尾加 markdown 脚注或思考 token）就能让整个自治循环静默 fail-open。

**c) 工作流是一维列表**
虽然编排引擎在单工作流内极其成熟（loop-back/resume/parallel/wave/depends_on/human_gate），但工作流之间是**零组合能力**的。`design → build` 的演进要靠用户在外层 `forge run design && forge run build`，而不是工作流自身的语义。当前架构的扩展瓶颈不在「如何执行一个工作流」，而在「如何组合工作流」。

**d) 学习闭环只有任务层，没有行为层**
系统能学「哪些 gap 被发现了」（memory）、「ROADMAP 进展到哪」（converge）、「哪个模型对哪个任务类型性价比最高」（scorecard/HistoryTiebreak），但**不学「agent 卡是不是写得不好」**。agent 卡是纯静态的。这意味着如果实现者的 prompt 写得模糊，系统不会自我改进——它会在每次 loop-back 中给 agent 发完全相同的 prompt，期望非确定性 LLM 这次做得好一点。

**e) 治理的持久性盲区**
`N/A` 状态没有年龄/保鲜度概念。一旦某个 gate 报告 N/A（工具未安装），它会**永远**被当作「没问题」——即便下个月工具装好了，`ProbeAll` 只在每次 `forge run` 时才重新探测，而 operator 如果没有手动触发 run 就不会知道。这导致「诚实」退化为「永久忽略」。

### 1.3 架构债务与技术债

| 类别 | 描述 | 严重程度 | 修复成本 |
|------|------|---------|---------|
| **设计债** | 单实例持久化设计无法支持 vision 的多实例工厂 | 🔴 高 | Phase A ~200 行（检测锁）+ Phase B ~400 行（隔离子目录） |
| **设计债** | 工作流无组合原语，跨工作流靠复制粘贴 | 🔴 高 | ~500 行（include + next_stage 消费） |
| **实现债** | 三个独立末行解析器，重复模式，无契约声明 | 🟡 中 | ~400 行（统一解析器 + output_contract 声明） |
| **可观测债** | N/A 无保鲜度、无趋势、无审计报告 | 🟡 中 | ~200 行（age tracking + doctor 报告） |
| **架构债** | 元学习闭环缺失行为层，agent 卡纯静态 | 🟢 低 | ~500 行（prompt hash + 信号采集，但元学习建议本身是 P3） |
| **代码债** | `scoring/scoring.go` 已被确认不存在（`Score` 实际在 `routing/routing.go:177`）| 🟢 低 | 更新两篇分析文档中的文件引用 |

总的来看，技术债水平健康——这与 32 个 Sprint 中「先拆分、再继续」的纪律一致。但**设计债**（D1 单实例、D4 工作流组合）如果堆积到 v3 再修，重构成本会指数上升。

---

## 2. 扩展方向

### 2.1 方向一：多实例隔离（D1）

**为什么需要**:
这是**运维安全基线**。当前两个 `forge` 进程同时写同一 `.forge/` 目录会导致：
- checkpoint 静默覆盖（resume 到错误 iteration）
- trace.jsonl 行交错（telemetry 数据不可恢复）
- memory cache 跨进程不同步（加载过期数据）

对于 CI pipeline（`forge accept`）+ 开发者本地 + cron 定时 `forge evolve` 的三重场景，这不是理论问题——**迟早会撞见**。

**核心挑战**:
- `flock` 是建议性锁（防君子不防小人），SIGKILL 不清理锁文件
- 跨主机共享 `.forge/`（NFS）的分布式锁是 v2+ 问题，v1 不做
- 隔离子目录（`run-<ID>/`）需要保证主 checkpoint 仍可恢复（锁协调）

**预期架构变更**:
```
forge-core/internal/persist/
  ├── checkpoint.go          ← 原文件：增加 InstanceID 写入
  ├── lock.go                ← 新文件：flock 获取/释放 + 锁文件 TTL 检测
  ├── trace.go               ← 原文件：增加隔离目录感知（trace 写到 run-<ID>/）
  └── instance.go            ← 新文件：InstanceID 生成 + 活跃实例注册
```

**对现有系统的影响**:
- Phase A（检测锁）：`flock` 获取失败→打印 WARNING 不阻断→**零行为变化**（向后兼容）
- Phase B（隔离子目录）：默认关闭（`--isolate-runs`），开启后所有新 run 写 `run-<ID>/` 目录
- 影响面：主要影响 `forge doctor --concurrent` 和 trace 消费者（scorecard/telemetry）

**技术选项**:

| 选项 | 优点 | 缺点 | 建议 |
|------|------|------|------|
| A: 检测锁（仅 flock） | ~200 行，零运行时破坏，能覆盖 80% 场景 | 防不住恶意进程或 SIGKILL | **Phase A 首选** |
| B: 隔离子目录 + 符号链接 | 完全隔离，恢复时 symlink 指向最新 | 需要处理「哪个是最新版本的 checkpoint」 | Phase B 方案 |
| C: 嵌入式 etcd/raft | 真正分布式锁 | 违背零外部依赖原则，太重 | 明确排除 |

### 2.2 方向二：Agent 输出契约系统（D2）

**为什么需要**:
自治循环的「决策转折点」当前依赖三个精确末行解析器。模型升级（Claude 自动加 markdown 脚注、后置思考 token）会让整个系统静默 fail-open——reviewer 没批准但系统继续走。这是一个**信任基座问题**：如果 operator 不能信任系统正确理解了 agent 的裁决，那 24h 自治就是空中楼阁。

**核心挑战**:
- 向后兼容：现有 agent 卡格式不变，`output_contract` 是可选的
- 契约验证粒度：验证「最后一行是 TOKEN」容易，验证「JSON schema 格式的输出」需要 JSON Schema 引擎——后者超出当前 scope
- 双源一致性：agent 卡（human-readable 格式说明）和 `output_contract`（machine-readable 声明）可能 drift

**预期架构变更**:
```
forge-core/internal/asset/
  ├── asset.go               ← 增加 OutputContract 字段
  ├── output_contract.go     ← 新文件：统一解析器 ParseOutput

forge-core/internal/gate/
  └── resolve.go             ← 增加 contract_violation trace event

.agent/workflows/discover.yml 等 ← 增加 output_contract 声明（可选）
```

**对现有系统的影响**:
- `output_contract` 缺失→退回到当前三解析器行为→**逐字节向后兼容**
- 统一解析器 `ParseOutput(output string, contract OutputContract)` 替代三个重复实现→提升可测试性
- `forge validate --contracts` 增加新检查→零运行时影响

**技术选项**:

| 选项 | 优点 | 缺点 | 建议 |
|------|------|------|------|
| A: 最后一行 token 合同（当前建议） | ~400 行，覆盖现有 3 个解析器的全部场景 | 不覆盖「prompt 正文格式」的验证 | **Phase A 首选** |
| B: 完整 JSON Schema 验证 | 验证整个 agent 输出（不仅仅是最后一行） | 需要 JSON Schema 引擎（引入依赖 or 手写简化版） | Phase B 方向 |
| C: 保留现状 + 加一层「格式变化检测」 | 更小的改动 | 不解决根本问题（解析失败静默 proceed） | 不推荐 |

### 2.3 方向三：N/A 生命周期管理（D3）

**重要修正**:
代码验证发现，原始分析中的「全 N/A 静默视为 green」的前提已过时——`resolve.go:86` 的 `GatesGreen` 已包含 vacuous-green guard（`exemptsNoTool`），N/A 在 `production` lifecycle 下已经是 FAIL。因此**核心命题应从「N/A 算 green」修正为「N/A 无保鲜度/年龄追踪」**。但这一修正不影响方向本身的价值。

**为什么需要**:
- `forge init` 后大量 gate 是 N/A（coverage/lint/security 工具未装），operator 无法区分「正常的 N/A」（语言/框架不适用）和「应该配置工具的 N/A」
- `forge migrate --to engineering` 升级治理严格度时不知道当前有多少 N/A——迁移风险输入缺失
- Dogfood 自身纪律：如果 ForgeOS 允许自己的 gate 长期 N/A 也不觉得有问题，对外部项目的治理承诺就缺乏可信度

**核心挑战**:
- 「clean N/A」（Rust 项目无 Go coverage 工具）和「actionable N/A」（eslint 没装但应该装）的自动区分很难——需要白名单
- N/A 年龄是跨 session 的持久化数据（需要记录到 `.forge/` 或 memory）
- 自动复检（Phase B）不能变成噪音——如果每次 `forge run` 都报告「coverage 已 N/A 40 天」，operator 会忽略

**预期架构变更**:
```
forge-core/internal/gate/
  ├── gate.go                 ← ProbeAll 返回 N/A 时间戳
  ├── freshness.go            ← 新文件：N/A 年龄计算 + 预警阈值

forge-core/internal/converge/
  └── signals.go              ← 增加 NAGatesAge 信号

forge-core/cmd/forge/
  └── status.go               ← governance 报告增加 N/A 保鲜度行
```

**对现有系统的影响**:
- 只增加可观测性（报告）+ 可选预警（非阻断）→ **不对现有行为产生任何影响**
- `max_na_days` 默认 0 = 永不自动复检→向后兼容
- 白名单 `project.yml: na_permanent` 让用户自己标注哪些 N/A 是「故意如此」

### 2.4 方向四：工作流模块化（D4）

**重叠处理**:
代码验证已确认——`expansion-five-truly-uncovered-frontiers-v46.md` 的方向一「工作流组合代数」已经系统讨论了 include/import/next_stage 概念。本文（D4）的差异化角度是 **「治理一致性 vs 组合代数」**：
- v46：侧重编排语义（DAG、条件分支、子工作流）——这是**组合代数**
- D4：侧重跨工作流的治理一致性——build.yml 和 evolve.yml 的 phase 在手工维护，容易 drift

两者互补不冲突。D4 的 `include` 是实现治理一致性的原子步；v46 的 `combine`/`branch` 是更宏大的编排愿景。

**为什么需要**:
- 治理 drift：build.yml 和 evolve.yml 共享 phase 模式但各自独立维护，改进不传播
- 模板化：引入 deploy.yml、security.yml、compliance.yml 后，复制粘贴的维护成本线性增长
- 第三方扩展：`forge-init` 的治理继承是复制文件，不是 import
- 架构清晰性：10+ phase 的工作流已经很难阅读

**核心挑战**:
- 循环 import 检测（A→B→A）必须在加载时拒绝
- 参数化 override（A import B，想 override B 中某 phase 的 model_tier）是 v2 问题
- 条件化 import（production 下 import security-audit.yml，mvp 下跳过）
- 导入后的 phase 命名冲突（两 import 引入同名 phase）

**预期架构变更**:
```
forge-core/internal/asset/
  ├── workload.go             ← 增加 include 解析（递归加载 + 循环检测）
  ├── merger.go               ← 新文件：phase 合并逻辑（include + inline）

forge-core/internal/orchestrator/
  └── chain.go                ← 新文件：next_stage 消费（workflow 链）
```

**对现有系统的影响**:
- `include` 是编译时合并→下游代码无感知（合并后仍是扁平 `[]Phase`）
- `next_stage` 可选→不声明时逐字节不变
- 核心变更是 `asset.LoadWorkflowJSON` 的加载路径，不是执行路径

**技术选项**:

| 选项 | 优点 | 缺点 | 建议 |
|------|------|------|------|
| A: 编译时合并（include） | ~300 行，零运行时影响，解决复制粘贴问题 | 不支持条件式 import、参数化 | **Phase A 首选** |
| B: 运行时 DAG（v46 的 combine/branch） | 真正的工作流组合语义 | 需要重新设计编排引擎（LoopEngine 当前假设单一 workflow） | Phase B/v2 方向 |
| C: 仅 next_stage 消费 | ~200 行，解决「跑完 discover 自动接 build」 | 不解决跨工作流 phase 复用 | 做在 include 之后 |

### 2.5 方向五：元学习闭环（D5）

**代码修正**:
`scoring/scoring.go` 在代码库中不存在。原始分析引用的 `Score` 函数实际在 `routing/routing.go:177`。这一引用错误会导致后续实施时查不到文件，需同步修正。

**为什么需要**:
- agent 卡是纯静态的，但 prompt 质量决定了整个自治循环的效率（更精准的 implementer prompt = 更少 REQUEST_CHANGES = 更少循环 = 更少 token 成本）
- 当前系统不做 prompt 效果追踪——不能回答「这个 prompt hash 下的 approve rate 是多少？」
- 当 implementer 连续 3 次被 reviewer 要求修改，系统什么都不做（相同的 prompt、相同的 agent card、期望不同结果）

**核心挑战**:
- 归因困难：这次 REQUEST_CHANGES 是因为 implementer prompt 写得模糊，还是 task 本身太复杂？
- Prompt hash 是内容敏感的——加一个空格 hash 就变了，需要归一化才能做有意义的聚合
- 「approve rate」是代理指标，不是 ground truth——高 approve rate 不一定高质量（一个偏好 approve 的 reviewer 让坏代码通过）
- 冷启动问题：前 10-20 次运行样本太少，统计无效

**预期架构变更**:
```
forge-core/internal/trace/
  ├── event.go                ← 增加 PromptHash 字段
  └── prompt_hash.go          ← 新文件：sha256 prompt -> hash

forge-core/internal/orchestrator/
  └── scorecard.go            ← 扩展：按 prompt_hash 聚合 approve_rate/cost/latency

forge-core/cmd/forge/
  └── scorecard_wind.go       ← 增加 --by-prompt-hash 标志
```

**对现有系统的影响**:
- Phase A（可观测性）：只增加记录，不改变执行路径
- Phase B（信号采集）：memory 增加 `kind:"lesson"` 条目，不改变收敛逻辑
- Phase C（元学习建议）：只在 `forge doctor` 中输出建议→零自动修改
- **关键安全设计**：永远不自动修改 agent 卡——输出是 ROADMAP item，human-in-the-loop 不可绕过

---

## 3. 接口设计建议

### 3.1 关键模块的接口设计原则

**原则一：声明式契约优先于隐式解析**
当前系统隐含了三种「输出契约」在法律上（reviewer/executive/confidence 的末行格式），但没有显式声明。建议增加 `output_contract` 作为 `Phase` 的一等字段，让契约从「写在 agent 卡里后被代码硬编码」变成「声明在工作流 YAML 中，被统一解析器消费」。

**原则二：组合优先于复制**
工作流之间的 phase 共享不应该通过复制粘贴实现。`include` 机制应该是「声明式引用」而非「文件包含」——区别在于：前者支持解析时验证（目标文件存在、无循环引用），后者只是文本拼接。

**原则三：可观测性是功能**
N/A 保鲜度、gate 状态趋势、prompt hash 追踪——这些都是功能，不是锦上添花的仪表盘。它们直接决定了 operator 能否信任系统正在自治地做正确的事。

### 3.2 是否需要新的抽象层

**是，需要两个新的抽象层**：

**a) 工作流组合层（Workflow Composition Layer）**
当前 `orchestrator.Engine` 消费单一 `asset.Workflow`，工作流间的关系由 CLI（`forge run discover && forge run design`）管理。需要一个新的抽象层——`WorkflowChain` 或 `Pipeline`——来管理工作流间的组合、条件推进、多阶段收敛。

接口示意：
```go
type Pipeline struct {
    Stages []Stage
}

type Stage struct {
    WorkflowName string
    OnApproved   *NextAction  // converge → continue to next stage / stop
    OnRejected   *NextAction  // human_reject → loop_back / stop
}
```

**b) 输出契约层（Output Contract Layer）**
当前三个解析器直接耦合在 `cost.go` 中。需要一个新的 `contract` 包来处理所有 agent 输出的结构化解析——声明、验证、降级。

接口示意：
```go
func ParseOutput(output string, contract OutputContract) (token string, ok bool, violations []Violation)
```

### 3.3 如何保持向后兼容性

| 新机制 | 向后兼容策略 | 过渡方式 |
|--------|------------|---------|
| `output_contract` 声明 | 缺失→退回到三解析器行为 | 增量采用：新 workflow 可声明，旧 workflow 不受影响 |
| `include` 指令 | 不出现→当前加载行为不变 | workflow 文件出现 include 时才激活递归加载 |
| N/A 保鲜度 | 默认 `max_na_days=0` → 永不自动复检 | 用户显式配置后才生效 |
| 多实例隔离 | Phase A 仅检测 WARNING、不阻断 | Phase B 需要 `--isolate-runs` flag 才隔离子目录 |
| 元学习 Phase A | 不记录 prompt hash（不影响 trace 格式） | 记录 `PromptHash` 到 Event.Detail（可选字段） |

---

## 4. 技术选型

### 4.1 是否需要引入新的技术栈或框架

**不需要。** 这五个方向全部可以在当前技术栈内完成：

| 方向 | 所需技术 | 现状 | 说明 |
|------|---------|------|------|
| D1 多实例隔离 | `flock`/`LockFileEx` | Go 标准库 `golang.org/x/sys/unix` 或 `os` 包 | `syscall.Flock` 在标准库中，**不需要外部依赖** |
| D2 输出契约 | 结构体 + 字符串匹配 | 标准库即可 | 不需要 JSON Schema 引擎 |
| D3 N/A 保鲜 | time + map | 标准库即可 | 只需要时间戳比较 |
| D4 工作流模块 | YAML 解析递归 | 已有 `yaml2json` 转换器 | include 可以在 YAML 解析层处理 |
| D5 元学习 | SHA256 + 聚合 | 标准库 `crypto/sha256` | 不需要嵌入向量或 ML |

**唯一可能引入外部依赖的场景**：如果未来 D2 从「末行 token」扩展到「全面 JSON Schema 验证」，可能需要一个轻量 JSON Schema 解析器。但这至少是 Phase C 的事，当前不构成决策。

### 4.2 第三方依赖的评估标准

参考 forge-core 已有标准：

| 评估维度 | 当前要求 | 是否可放宽 |
|---------|---------|-----------|
| 外部依赖数 | 零（`go.mod` 无 require） | 仅当功能无法用标准库实现时才考虑 |
| 许可证兼容 | MIT/Apache 2.0 | 严格，不得引入 GPL |
| 审计要求 | 每引入一个依赖需要 ADR + 安全评审 | 当前无引入计划 |
| 源码尺寸 | 倾向自建轻度实现而非依赖重型框架 | 例如用 `flock` 而非 etcd |

**对于这五个方向，坚持零外部依赖是可行的**。如果未来 D4 的 `include` 需要「YAML 深度合并」，可以通过轻量递归实现（~100 行），不需要引入 `yaml.v3` 之外的东西（而 forge-core 当前通过 python shim 转码 YAML，零 Go YAML 依赖）。

### 4.3 自建 vs 采购的决策依据

这五个方向不涉及「采购」——它们都是 forge-core 自身能力的短板，只能自建。但有一个例外：

**D1 的跨主机分布式锁**（当前明确排除）——如果未来需要（多个 CI runner 共享一个持久卷），采购 etcd/consul 是合理选择。

**决策矩阵**：

| 能力 | 自建理由 | 自建成本 | 采购选项 | 结论 |
|------|---------|---------|---------|------|
| 文件锁 | 标准库 `flock` 即可，零依赖 | ~200 行 | 无 | ✅ 自建 |
| 输出契约解析 | 统一现有 3 个解析器 | ~400 行 | 无 | ✅ 自建 |
| N/A 保鲜度 | 时间戳 + map | ~200 行 | 无 | ✅ 自建 |
| 工作流 include | YAML 层递归合并 | ~300 行 | 无 | ✅ 自建 |
| Prompt hash 追踪 | SHA256 + 聚合 | ~300 行 | 无 | ✅ 自建 |

---

## 5. 实施路线图

### 5.1 优先级排序

基于四个维度评估：**风险降低**（数据损坏/安全问题的概率）、**未来扩展的制约**（如果现在不做，后续架构扩展被阻塞）、**实施成本**（代码行数 + 影响面）、**用户可见收益**。

| 优先级 | 方向 | 风险降低 | 扩展制约 | 成本 | 收益 | 综合判断 |
|--------|------|---------|---------|------|------|---------|
| **P0** | D1 Phase A（检测锁） | 🔴 高（数据损坏） | 🔴 阻碍多实例 vision | ~200 行 | 中等 | **立即做** |
| **P0** | D4 Phase A（include） | 🟡 中（治理 drift） | 🔴 阻碍工作流扩展 | ~300 行 | 高 | **冲刺内做** |
| **P1** | D2 Phase A（输出合同） | 🟡 中（静默 fail-open） | 🟡 阻碍新 agent 角色 | ~400 行 | 高 | 下一冲刺 |
| **P1** | D3 Phase A（N/A 保鲜） | 🟢 低（治理幻觉） | 🟢 低 | ~200 行 | 中 | 随 D1 一起做 |
| **P2** | D5 Phase A（prompt hash） | 🟢 低（效率优化） | 🟢 低 | ~300 行 | 长期高 | 按节奏安排 |
| **P3** | D1 Phase B + D3 Phase B（主动防护） | 🟡 中 | 🟡 中 | ~750 行 | 高 | 视 Phase A 效果决定 |

### 5.2 阶段划分和里程碑

```
Sprint N:   【安全基线 + 可观测性】
  - D1 Phase A: flock 检测锁 + InstanceID + forge doctor --concurrent
  - D3 Phase A: N/A age tracking + forge status 保鲜度报告
  → 里程碑：多实例检测就绪；N/A 治理可观测

Sprint N+1: 【信号神经加固】
  - D2 Phase A: OutputContract 结构体 + 统一解析器 + workflow YAML 声明
  → 里程碑：所有 agent 输出统一契约解析，信号脆弱性消除

Sprint N+2: 【工作流组合】
  - D4 Phase A: include 指令 + 循环检测 + next_stage 消费
  → 里程碑：工作流可组合，跨工作流复制粘贴消除

Sprint N+3: 【元学习观测基座】
  - D5 Phase A: prompt hash 追踪 + scorecard --by-prompt-hash
  → 里程碑：元学习数据管道就绪

Sprint N+4: 【主动防护 + 深度加固】
  - D1 Phase B: 隔离子目录
  - D3 Phase B: N/A 自动复检（max_na_days 配置）
  → 里程碑：多实例主动隔离；N/A 自动跟踪

Sprint N+5: 【收敛检查】
  - forge accept 全绿
  - 五个方向的代码库验证（逐方向 fresh-context review）
  → 里程碑：五个结构性扩展全部交付
```

### 5.3 风险点和缓解策略

| 风险 | 概率 | 影响 | 缓解策略 |
|------|------|------|---------|
| **D1 flock 在 NFS 上不可靠** | 🟡 中 | 可能导致跨主机多实例仍静默冲突 | Phase A 只在检测时给出 WARNING；Phase B 隔离子目录不需要锁协调（各自写自己的目录） |
| **D2 输出契约与现有 agent 卡漂移** | 🟡 中 | agent 卡描述 vs machine 声明不一致 | `forge validate --contracts` 作为 CI 闸门；不一致时 FAIL 构建 |
| **D4 include 循环引用** | 🟢 低 | 加载时无限递归 | 已加载路径 set → 发现循环时明确 error（不静默跳过） |
| **D5 元学习的冷启动噪音** | 🔴 高 | 前 10-20 次运行样本不足，统计不可靠 | 在 <10 个样本时不显示元学习指标；评分区间标注 confidence interval |
| **多个方向同时实施导致 merge 冲突** | 🟡 中 | Phase 结构体同时被 D2（OutputContract）和 D4（Include）修改 | D2 和 D4 共享 `asset.Phase` 的扩展——建议安排串行而非并行：D4→D2（或反向） |
| **D4 的 include 测试要求外部 YAML 文件** | 🟢 低 | 测试覆盖率依赖于 fixture 文件 | 用 embed.FS 嵌入测试 fixture（Go 1.16+, 标准库） |

---

## 总结

这五个结构性扩展方向落在同一个模式中：**ForgeOS 的垂直能力（单工作流编排、单实例运行、单轮静态 agent prompt）已经极其成熟，但水平断裂——跨工作流、跨实例、跨运行、跨层级的组合/协调/进化能力是缺失的。**

它们的共同特征是「低代码量、高架构杠杆」：
- 最重的方向（D2 输出合同）约 400 行
- 最轻的方向（D3 N/A 保鲜）约 200 行
- 全部五个 Phase A 合计约 1400 行

这不是巧合——说明这些方向不是「新造一个子系统」，而是**在当前架构的接口/边界处补一道薄抽象的接缝**。这正是架构师最有价值的工作：不是在现有体系上加新特性，而是在系统的自然断裂面插入薄抽象，让未来的扩展不再需要打破这些边界。
