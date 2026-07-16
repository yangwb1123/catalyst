# ForgeOS — 高价值扩展方向分析

> 基于 `forge-core` (Go ~20k 行) + `harness` (Node/Python ~13k 行) 全局扫描。
> 角色：资深架构师 / 产品经理视野，不写代码，只做判断。

---

## 目录

1. [方向一：Agent 沙箱隔离层（安全基座）](#方向一agent-沙箱隔离层安全基座)
2. [方向二：语义知识引擎与 RAG Pipeline](#方向二语义知识引擎与-rag-pipeline)
3. [方向三：持久化人工审批与事件驱动通知](#方向三持久化人工审批与事件驱动通知)
4. [方向四：跨仓库编排与联邦治理](#方向四跨仓库编排与联邦治理)
5. [方向五：自演化质量体系——测试生成与变异测试](#方向五自演化质量体系测试生成与变异测试)

---

## 方向一：Agent 沙箱隔离层（安全基座）

### 现状

`CommandExecutor` 直接在宿主机的进程空间中执行 `claude -p`（或 `echo` 等 stub 命令）。虽然已有三重防护：
递归深度守卫（`FORGE_AGENT_DEPTH`）、输出大小上限（`cappedBuffer`）、超时终止（`context.WithTimeout`），
但这些只防止资源耗尽/死循环，**不防止恶意行为**。

一个 24h 自治循环运行的 agent 可以通过 claude 的 `Bash` 工具：
- 读取宿主机任意文件（`~/.ssh/id_rsa`、`/etc/passwd`、环境变量中的 API key）
- 写入恶意 payload 到相邻项目目录
- 消耗宿主机 GPU/NIC 等非预算资源
- 持久化后门至 `cron` / `systemd` / SSH authorized_keys

当前的风险分类器（`internal/risk/risk.go`）已经实现了信号驱动安全分级（`Critical` → `Opus`），
但**分级后的执行路径没有任何差异化的沙箱隔离**——Critical 任务跑 Opus 模型但仍在裸宿主机上。

### 为什么需要

| 维度 | 理由 |
|------|------|
| **安全使命** | ROADMAP v3 明确列出 "带外 Sandbox (Firecracker)"，这是整个系统向前沿生产环境演进的前提条件 |
| **真实威胁** | OWASP Agentic Top-10 2025-12 排名第一即为 "Sensitive Information Disclosure"——secret-scan 能扫描已提交的硬编码密钥，但无法防御运行时的凭据外泄 |
| **差异化执行** | `risk.Classify` 已经产出 `low`/`medium`/`high`/`critical` 四级风险，但执行层没有差异化的资源边界——高风险的 auth/payment 任务和低风险的文档任务在同一个进程空间跑 |
| **dogfood 扩展** | 如果 ForgeOS 要管理自己的生产数据库 schema 迁移（已有 `TouchesMigration` 信号），任何未沙箱化的 agent 都能在迁移脚本中嵌入恶意逻辑 |

### 建议的架构方向

```
Sandbox 接口 (internal/sandbox):
  interface Sandbox {
    Execute(ctx, Phase) → Result
    Capability() → IsolationLevel
  }

  实现:
    HostSandbox —— 零开销，用于 low/medium 任务（当前行为）
    ContainerSandbox —— OCI 容器，用于 high 任务
    VMSandbox —— Firecracker microVM，用于 critical 任务

  注入点:
    CommandExecutor 不直接 exec.Command，而是调用 Execute(ctx, argv) → sandbox
    risk.Classify(signals) 输出直接决定 Sandbox 选择
```

**关键约束**：零外部依赖的 `forge-core` 包（纯 Go 标准库）只定义接口和纯逻辑；
沙箱的具体实现作为 `cmd/forge` 里的构建标签文件（`sandbox_unix.go`、
`sandbox_darwin.go`、`sandbox_other.go`），沿用 `command_executor_unix.go` 的隔离模式。

### 边界情况

- **沙箱中运行 harness gate**：gate 脚本需要读取 `.agent/` 和源码，沙箱内外文件映射策略
- **跨沙箱内存/知识传递**：checkpoint 和 memory 目前写宿主 FS，沙箱内 agent 写入后如何持久化
- **预算 vs 沙箱成本**：启动 Firecracker microVM 的秒级延迟和内存开销对短 phase（文档生成）不可接受
- **Windows 兼容**：OCI 容器/Firecracker 均无 Windows 原生支持，降级行为定义

---

## 方向二：语义知识引擎与 RAG Pipeline

### 现状

`internal/prompt` 包中的 Context Engine 三条信息源：

1. **ROADMAP 当前任务**：全文注入（`taskCap: 4000 runes` 截断）
2. **ADR 检索**：BM25-lite 关键词打分（`prompt.Retrieve`，纯词频统计学），
   取 top-K（`adrTopK: 6`）。开发者自述「keyword retrieval, not semantic — 'vehicle' won't match 'car'」
3. **工程红线**：`AGENTS.md` 前 6 条 bullet，硬编码

跨 session 记忆（`internal/memory`）虽然不设 schema，但查询路径 `memoryContext()` 调用
的 `boundMemory()` 仍然是同一套 BM25-lite 关键词匹配 + recency-floor 混合策略。

这意味着：**Agent 无法做"跨文档的概念等价检索"**。当 ADR-0003 说 "submodule"、
`ARCHITECTURE.md` 说 "governance layer"、代码注释说 "agent-os" 是同一件事时，
关键词检索会认为它们是三个独立主题。

### 为什么需要

| 维度 | 理由 |
|------|------|
| **决策完整性** | 一个 reviewer agent 面对修改时，需要知道所有相关 ADR——不仅仅是标题含关键词的。当前的 BM25 在 ADR 数量增长后召回率会急剧下降（已标记 "earns its keep the moment the repo accrues more ADRs than fit a context window"） |
| **长期运行** | evolve loop 每轮写一条 memory，50 轮后 store 就有 50 条。关键词检索在高密度 topic 空间中退化为噪音。`memoryRecencyFloor=8` 保证最近 8 条始终可见，但 8 条之外的 "old but relevant" 策略是关键词检索——在密集记忆下几乎随机。 |
| **跨 session 推理** | 一个 gap（memory.KindGap）的描述可能是 "gate N/A matrix misclassifies no_tool as inapplicable"，下一次 agent 问的问题是 "why did architecture-check report N/A?"——BM25 匹配不上。 |
| **知识图谱潜力** | 所有 ADR、架构文档、决策记录、memory entries 之间存在显式或隐式引用关系（"ADR-0001 触发 ADR-0002"、"D1 被 D2 推翻"）。纯文本检索无法利用这种结构。 |

### 建议的架构方向

```
layer 1: Embedding 服务 (纯 sidecar，非 forge-core 内部)
  - 项目级 .agent/embeddings/ 缓存（JSONL: path → vector）
  - 用 OSS embedding API（如 Gemini / BGE-M3）或本地模型（fastembed-rs）
  - forge-core 保持零依赖：只读已缓存的 embedding，不调用模型

layer 2: 混合检索器 (internal/prompt, 纯 Go)
  type Retriever struct {
    Keyword   BM25     // 现有 BM25-lite
    Semantic  Semantic // embedding cosine-similarity
  }
  func (r Retriever) TopK(query string, k int) []Doc
  // RRF (Reciprocal Rank Fusion) 合并两个排序

layer 3: 记忆图 (internal/memory 增量)
  - Entry 增加 relation: []string 字段（ADR-0003 "references" memory-entry-5）
  - 图 DFS 检索：找到一条相关知识后，沿着 relation 遍历邻接条目
```

**最小可行路径**：不改 `memory.Entry` schema，仅加 `Embedding` 缓存 + RRF 混合检索。
memory 的 JSONL line 可扩展（`omitempty` 保持向后兼容）。

### 边界情况

- **Embedding 模型版本漂移**：旧 embedding 与新 query 在同一语义空间下吗？
  需要 `embedding_version` 字段，检测到版本不匹配时全量重建
- **本地项目不可外发**：如果项目数据不能外发（内部软件），Embedding 必须本地运行
- **增量更新**：新增一个 ADR 不需要重算所有旧 embedding，但 cold start 需要
- **context window 竞争**：语义检索可能召回到高度相关但很老的 ADR——和 recency-floor 策略冲突。
  需要权重调和

---

## 方向三：持久化人工审批与事件驱动通知

### 现状

人工审批（`human_gate`）目前是一个**轮询信号**：
- CLI `--approved` 标志
- 文件系统标记 `<root>/.forge/<stage>.approved`

`reportHumanGate` 的输出诚实标注了这是 v1 信号检查："HONESTY: the approval is a v1 signal check (--approved / on-disk marker), not a durable cross-process wait (durable_wait is v2, Temporal)."

从产品视角看，这个门将几乎不可用：

1. **无人值守循环不知道有人在等**：`forge evolve` 遇到 `human_gate` 会直接拒绝运行（`rejectHumanGate`），
   唯一正确路径是 `forge run`。但用户不盯终端输出时不知道 run 在等批准。
2. **零通知**：没有 Slack/Email/Discord Webhook 告诉「有人等你做架构批准」
3. **零持久化**：如果部署在 CI runner 上，runner 被回收后 pending 批准丢失
4. **无 API**：外部工具无法查询当前有哪些 pending 批准、批准历史

### 为什么需要

| 维度 | 理由 |
|------|------|
| **最高杠杆闸门** | `BOOTSTRAP.md` 和 `PROJECT.md` 都声明 "Human Approval (Design→Build 之间) 是全系统最高杠杆的闸门"。但最高杠杆的闸门用最简单脆弱的机制实现——这是能力和表述之间的 gap |
| **真实工作流** | 架构师不在终端前盯着 `forge run`，而是用 Jira/Slack/Teams。ForgeOS 需要融入这些流程，否则用户会绕过它 |
| **审计合规** | 生产环境要求 "who approved what and when" 的可审计记录。当前 `.forge/checkpoint.json` 记录 `approved:true` 但不记录批准人身份、时间、上下文 |
| **Evolve 循环断裂** | 当前 `forge evolve` 对 `human_gate` 是 "fail-closed"（直接拒绝启动），这意味着包含人工节点的多阶段工作流完全不可自动化——设计→审批→实现→测试→部署这个基本链路无法接入 evolve 循环 |

### 建议的架构方向

```
1. 审批服务 (轻量, 非 Temporal——Temporal 是 v2 的 durable_wait 候选)
   internal/approval/
     type ApprovalRequest struct {
       Stage     string   // "design"
       Prompt    string   // 架构方案摘要 + diff/context
       RequestedAt int64  // unix
       Status    string   // "pending" | "approved" | "rejected"
       ApprovedBy string  // 空 = pending
       DecidedAt int64    // 0 = pending
     }
     type Store interface {
       Create(req) error
       List(pending) ([]Request, error)
       Decide(id, verdict) error
     }

2. 通知适配器 (插件式, 接 cmd/forge)
   type Notifier interface {
     NotifyPending(req) error     // Slack/Email/Discord webhook
     NotifyResult(req) error      // 批准后通知回 workflow
   }

3. CLI/Wire 整合
   forge approve                  # 列出所有 pending 批准
   forge approve design --yes     # 批准 design stage
   forge reject design --reason "数据流风险未评估"

4. 与 `forge evolve` 整合
   - evolve 遇到 human_gate 时：提交 ApprovalRequest、发通知、暂停循环
   - 轮询（或 webhook）等待批准结果
   - 批准后继续下一阶段，拒绝时记录原因并 stop
```

**最小可行路径**：不引入外部数据库。`<root>/.forge/approvals/` 目录用 JSONL，Webhook 配置
在 `project.yml` 的 `notifications:` 下。与现有 `--approved` 标记兼容——检测到标记自动批准。

### 边界情况

- **超时**：批准请求 72h 无响应怎么办？默认超时视为拒绝？发送 escalation？
- **多人审批**：v1 支持单人即可，方向正确但不做多签
- **撤回**：误通过的批准能否撤回？在无状态轮询模型下很难
- **approval + 并行 wave**：如果两个独立阶段各有一个 human_gate，可以并行等吗？
- **Evolve 暂停/恢复**：等待批准的循环需要序列化 checkpoint 内的 pending 状态

---

## 方向四：跨仓库编排与联邦治理

### 现状

当前 ForgeOS 是**单仓库系统**。所有路径锚定在单一 `--root`：
- `internal/gate` 的 `RepoRoot()` 返回一个根
- `internal/prompt` 的 `Gather()` 从 `<root>/.agent/` 读上下文
- `internal/orchestrator` 的工作流加载自 `<root>/.agent/workflows/`
- harness acceptance 扫描 `<root>/examples/` 的应用

ADR-0003 提出了治理资产的 submodule 化（`.agent/` 全局基线从独立 `agent-os` 仓共享），
但那是**纵向共享**（中心→项目），不是**横向编排**（同时管理多个独立项目/仓库）。

现有模式注定了一个 evolve 循环只能管理一个仓库。如果一个产品由前端仓（React）+ 后端仓（Go）+ 共享
proto 仓三部分组成，ForgeOS：
- 无法在一个 worklow 中同时修改三个仓（有跨仓依赖的变更）
- 无法在多仓间传播 governance 更新（非 ADR-0003 主张的资产共享，而是同时升级所有项目依赖的同一治理规则）
- 无法在聚合视图上判断 "产品级" roadmap 完成度（`RoadmapCompletion` 只扫描一个仓的 `.agent/ROADMAP.md`）

### 为什么需要

| 维度 | 理由 |
|------|------|
| **真实架构** | 大多数中大型软件系统是多仓库（或 monorepo 分模块）。ForgeOS 作为软件工厂，不能假设产品只有一个仓 |
| **依赖编排** | 一个 API 变更需要同时修改 provider 仓和 consumer 仓——当前只能分两次 evolve，中间站产生 broken window |
| **治理统一** | ADR-0003 解决了资产分发（中心→项目），但没有解决"中心同时升级所有项目"的 rollout 场景 |
| **复杂度证明** | ROADMAP 已有 "独立 agent-os 仓库(submodule)" 和 "模板" 待项，多仓编排是它们的自然上层 |

### 建议的架构方向

```
1. MultiRoot 概念 (无害增量)
   type Workspace struct {
     Roots []RepoRoot    // 多个仓库的绝对路径
     Primary int         // 0 = 主仓库 (read ROADMAP/stop from this)
   }
   - 所有现有 internal/gate/prompt 接口默认取 Roots[0]（back-compat）
   - 工作流可以声明跨仓 agent 阶段：指向特定 root 执行

2. 跨仓上下文聚合 (prompt.Gather 扩展)
   - 读取每个 root 的 ROADMAP/ADR/AGENTS
   - 标记每个条目的来源 root
   - 提示 agent "你在修改 provider 仓，consumer 仓的 API 版本约束是……"

3. 跨仓 convergence
   - `GatesGreen` 需要每个 root 的 `forge accept` 都绿
   - `RoadmapCompletion` 可以聚合为主仓 + 子仓的加权组合
   - checkpoint 需要记录每个 root 的进度

4. Workspace 发现
   - 支持 <root>/FORGE_WORKSPACE.yml 声明其他 roots
   - 支持 --workspace ./forge-workspace.yml 从外部文件加载
```

**最小可行路径**：不引入新的 engine 概念。在 `runOpts` 加 `--multi-root` flag，
修改 `harnessRunner`/`gatherSignals` 使其对每个 root 调用一次 probe/read，
然后 AND 聚合。路径解析改造（ADR-0003 的 `FORGE_PROJECT_ROOT`）是前置条件。

### 边界情况

- **根之间切换工作目录**：agent 的 cwd 是哪个根？agent 需要同时读写多个根时怎么办？
- **跨仓 git 原子性**：一个 evolve 迭代修改了三个仓，回滚时如何协调？
- **权限差异**：某些仓只读（vendor fork）、某些仓可写
- **竞态 gate 信号**：`forge accept --json` 在 A 仓 PASS 瞬间 B 仓有人提交失败代码——读取时差导致的不一致
- **依赖波**：如果三个仓的 pipeline 有执行顺序依赖（先 release proto，再后端，再前端），parallel mode 如何处理？

---

## 方向五：自演化质量体系——测试生成与变异测试

### 现状

当前质量保证体系非常完善但**完全被动**：
- `harness/acceptance.mjs` 的 10 项 criterion 覆盖测试、架构、安全、SCA 等
- `arch-check.mjs` 的 8 项检查（layering / 包 / 扇入 / 认知 / 命名 / 函数长度 / 循环依赖 / drift-guard）
- `secret-scan.mjs` 和 `sca.mjs` 覆盖安全
- `probeAppTests` 确保每个 example app 的测试通过

但所有这些都**验证已有代码的质量**，不**主动创造质量**：

1. **测试覆盖率没有强制执行**：lifecycle `production` 的 `coverage_delta +20` 在代码
   中标注为 "coverage THRESHOLD adjustment…needs a coverage tool to mean anything — that lives
   in the harness coverage adapters…not in this pure gate-set distillation"。实际至今未接入。
2. **无变异测试**：不衡量测试本身的充分性——现有测试全部通过但可能一个 assertion 都没有（"empty test"）
3. **无自动测试生成**：evolve 循环添加代码，但没有人（没有 agent 阶段）写对应的测试，
   导致 `forge accept` 的 `test_pass` 永远只测试现有测试，不测试新增代码
4. **`select-tests.mjs` 只选已有测试跑**：不等于引导 agent 写新测试

### 为什么需要

| 维度 | 理由 |
|------|------|
| **G1–G5 完整性** | ForgeOS 宣称 G1–G5 要实现 Idea→Production 全自动。不测试的代码不是 "Production-ready"。当前 "Implementer 写代码 → Reviewer 审代码 → QA 验证" 中 QA 是 agent 阶段但 prompt 里没有"检查测试覆盖率"这一条 |
| **自验证循环** | 当一个 `forge evolve` 迭代新加了 200 行代码但添加了 0 行测试时，`RoadmapCompletion` 仍然前进，但代码质量在下降——系统没有捕捉到这个退化 |
| **coverage_delta 落地** | lifecycle_modifiers 中 `production` 要求 `+20%` 覆盖率，但没有任何机制保证。ROADMAP v2 的 "执法补完" 方向也提到了覆盖率 |
| **Agent 行为引导** | 当前 agent 没有 "写测试" 的压力。如果在 `roadmap_completion` 之外增加一个 `test_coverage` convergence 条件，代理的行为会自然演化——这是系统作为行为塑造者的角色 |

### 建议的架构方向

```
1. 覆盖率接入 (harness 增量)
   - 确认 coverage_delta 在 adapters/<lang>.yml 中的 runner（现有框架如 go test -cover、
     pytest --cov、c8/v8 coverage）
   - 在 acceptance.mjs 中启用 probeCoverage 的 load-bearing 判定（当前覆盖率检查
     在 production lifecycle 下应为必修，不被 N/A 豁免）
   - converge 信号增加 CoverageCompletion float64

2. 变异测试 (增量 harness 工具)
   - harness/mutation-test.mjs: 接受 repo 路径 + test runner，注入变异算子
     (常量翻转、运算符替换、条件反转、return null)，运行测试，计算 mutation score
   - 作为 acceptance.mjs 的选修 criterion（`mutation_score >= 0.8`），production lifecycle 必修
   - OSS 引擎: Stryker (TS), mutmut (Python), go-mutesting (Go)

3. 测试写入 agent phase (workflow 增量)
   - 在 build.yml 的 implementer phase prompt 中增加：
     "For every new function you write, you MUST also write a test. Verify with
      `node --test` before ticking [x]."
   - QA phase scope 从 "验证功能正确" 扩展到 "验证测试充分性"
   - 如果 implementer 没写测试但 ticked [x]，reviewer 应当 detect 并提出
```

**最小可行路径**：仅做第 1 步（覆盖率接入）+ 第 3 步（prompt 引导），不做变异测试。
变异的工具依赖和计算开销较大，可以放在 v3。

### 边界情况

- **IDE 项目不一定有测试框架**：如果项目是实验性代码，`coverage_delta` 可能恒为 0%
- **覆盖率虚高**：100% 覆盖率不意味测试质量好（可以只有一行 assertTrue(true)）
- **变异测试超时**：大型项目全变异耗时数小时，需要像 `select-tests.mjs` 一样的增量变异选择
- **Flaky tests**：变异测试中测试本身 flaky 导致误报 mutation score，需要 flaky test 检测器
- **Agent 写测试的能力**：不是所有 agent 模型都能写出有意义的测试。Claude Opus 可以，Haiku 写的测试可能等于 assertTrue——这需要一个质量门槛

---

## 总结优先级

| 方向 | 影响面 | 实施成本 | 依赖前序 | 推荐 |
|------|--------|----------|----------|------|
| 1. Agent 沙箱 | **安全**: 高 | 中 | 无（接口已耦合度低） | **最高优先**：任何生产运行的硬化前提 |
| 2. 语义知识引擎 | **质量**: 中-高 | 中 | 无（纯增量） | 第二优先：ADR/记忆检索质量随项目增长线性下降，越早做收益越大 |
| 3. 持久化审批 | **流程**: 高 | 低-中 | 无 | 第三优先：当前实现限制了人机协作场景，投入产出比高 |
| 4. 跨仓库编排 | **扩展**: 中 | 高 | ADR-0003（路径改造） | 暂缓至 ADR-0003 触发条件达成（被治理项目 ≥ 2~3 |
| 5. 自演化测试 | **质量**: 中 | 中-高 | 无（prompt 引导立即可做） | 第四优先：prompt 引导零代码成本，覆盖率接入是 harness 增量 |

**建议的方案**：

- **立即做**（0 代码改动，仅调 prompt + adapter）：
  在 implementer 和 QA 的 agent prompt 中增加测试质量引导

- **Sprint n**（2-3 人周）：
  方向 3（持久化审批）——`.forge/approvals/` JSONL + Slack webhook 通知

- **Sprint n+1 / n+2**（4-6 人周）：
  方向 1（沙箱接口定义 + OCI 容器实现）——`internal/sandbox` 接口 + `container_sandbox.go`

- **方向 2 持续积累**（每周 1 人日持续 3 周）：
  语义检索 RRF 融合 + embedding 缓存

- **方向 4 和 5** 作为 ROADMAP v3 的规划输入，不在当前 sprint 承诺

---

*分析日期：2026-06-29 | 基于 forge-core 9a1e0f 状态 + harness 最新 commit*
