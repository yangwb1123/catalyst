# ForgeOS — 可信运营成熟度：全局扫描后的高价值扩展方向

> **角色**: 资深架构师 / 产品经理  
> **方法**:
> 1. 全局深扫 forge-core（140+ Go 源文件 · 18 Go 包 · ~32k LOC 运行时 + CLI）、
>    harness（39+ 模块 · ~10.5k LOC 执法层）、`.agent/`（12 agent 卡 · 9 skill 卡 ·
>    5 工作流 · 全部 ADR+DECISIONS+architecture）、examples/、`.forge/` 运行态数据
> 2. 完整阅读 31 轮 sprint 演进记录（CURRENT_SPRINT.md）、FUNCTIONAL_REQUIREMENTS_AUDIT.md
> 3. DevOps 纪律：不编写任何代码。所有建议附代码级证据。
> **日期**: 2026-07-10
> **与众不同的视角**: 已有 60+ 份 `docs/requirements/` 分析文档均聚焦于「缺少什么新功能」或
> 「什么边界没处理」。本文关注的不是「加什么新引擎」，而是——**ForgeOS 作为 24h 无人值守的治理系统，
> 其运营可信度（Operational Trust）存在哪些结构性缺口？** 不是功能缺失（feature gap），
> 而是信任缺失（trust gap）：用户凭什么相信这个系统不会在半夜静默出错？
> 每一方向都直接回答「为什么没有这个特性，24h 运行就可能出信任危机」。

---

## 全景概览

| 已有覆盖域 | 已有分析数 | 本文不重复的内容 |
|---|---|---|
| 引擎补齐（编排/路由/记忆/收敛/并行/loop-back） | ~20 篇 | ✅ |
| 生产可靠性（529/超时/退避/护栏/进程组） | ~18 篇 | ✅ |
| 可观测性（trace/telemetry/scorecard/三维真数据） | ~10 篇 | ✅ |
| 记忆/学习（memory/checkpoint/Supersedes/ContextCache） | ~10 篇 | ✅ |
| 执行语义（原子性/幂等/回滚/因果一致性） | ~10 篇 | ✅ |
| 系统边界盲区（并发隔离/TOCTOU/信任边界/数据生命周期） | ~10 篇 | ✅ |
| 测试元治理（执法器盲区/自我测试隔离） | ~8 篇 | ✅ |
| 第三地平线（多仓库联邦/事件驱动/Web UI） | ~10 篇 | ✅ |
| **本文：运营可信度（Trust & Operational Maturity）** | **~0 篇** | **独有视角** |

---

## 方向一 · Run Identity 与状态隔离

> **优先级**: 🔴 **P1** | **类别**: 运营 · 数据完整性 | **风险**: 数据静默损坏  
> **代码证据**: `.forge/trace.jsonl` · `.forge/checkpoint.json` · `.forge/memory.jsonl`

### 问题描述

ForgeOS 的所有运行态数据存储在 `.forge/` 目录下。但**没有任何 run/session 标识符**来区分哪条 trace 事件
来自哪次运行。以下是真实数据中的活体证据（来自当前工作树 `.forge/trace.jsonl`）：

```json
{"_format":"forgeos.trace.v1","seq":1,"kind":"doctor","name":"checkpoint","status":"ok",...}
{"_format":"forgeos.trace.v1","seq":2,"kind":"doctor","name":"trace","status":"ok",...}
```

trace 事件有 `seq`（单调递增的数字），但没有 `run_id`、`session_id`、`workflow_name`＋`started_at`
的组合键。多个 `forge doctor` 调用的输出被追写到同一个文件，事件交错排列——追溯「这些事件是哪次 `doctor`
产生的」只能靠人工猜测时间戳间隔。

更严重的是 checkpoint.json（当前内容）：
```json
{
  "workflow": "evolve",
  "mode": "balanced",
  "iteration": 1,
  "roadmap_completion": 1,
  "gates_green": true,
  "reason": "iteration complete",
  "updated_at_unix": 1782516099
}
```

没有 `run_id`，没有 `hostname`，没有 `agent_version`。如果两个 `forge evolve` 同时运行在同一个项目上
（例如一个在 CI 中触发，另一个是开发者本地的手动运行），它们会：
1. 向同一个 `trace.jsonl` 追加事件（O_APPEND，行交错，不可分离）
2. 向同一个 `memory.jsonl` 追加条目（知识污染——一个运行的「发现」变成另一个运行的「前提」）
3. 覆盖同一个 `checkpoint.json`（最后写入者胜，状态丢失）
4. 共享 `scorecards.json` 的读写（并发更新静默丢失）

### 为什么这是信任缺口

ForgeOS 的 vision 是 24h 无人值守自治运行。在这样的系统中，**数据污染是不可检测的**——没有 operator
盯着控制台去发现「这些 trace 怎么看起来像是两股交织的线程」。更糟糕的是，memory 的跨会话污染会让系统
学到的「知识」混入另一个运行产生的错误结论，而 checkpoint 的静默覆盖会让灾难恢复路径指向错误的状态。

### 边界场景

- **CI + 本地同时运行**: 开发者本地 `forge run build` 和 CI 中 `forge accept` 同时写 `.forge/`，
  CI 的结果被本地运行的 trace 事件稀释，本地运行的 memory「发现」混入了 CI 的上下文
- **SIGKILL 后残留**: 一个 run 被 `kill -9` 后，checkpoint.json 处于中间状态，下一个 run 通过
  `--resume` 读到残缺的 checkpoint，从错误状态恢复
- **SSH 重连后重复执行**: 用户在 tmux 中启动 `forge evolve`，SSH 断开后重连，忘了已经在跑，
  又启动一次——两股进程在同一个 `.forge/` 下争夺同一份状态

### 建议方向

- 为每个 `forge run`/`forge evolve`/`forge accept` 调用分配一个 UUID `run_id`，注入到所有子进程
  环境变量和 trace/memory/checkpoint 的每条记录中
- `.forge/` 目录添加命名空间层：`.forge/runs/<run_id>/trace.jsonl`，`.forge/store/` 跨运行共享状态
  仍存在但需要显式声明
- 防止并发写入：启动时检测 `forge run`/`forge evolve` 的 LOCK 文件，已存在且有活进程则拒绝启动
- checkpoint 添加 `run_id` + `forge_version` + `git_sha` 字段，防止跨版本恢复

---

## 方向二 · 运行时依赖契约与启动预检

> **优先级**: 🟠 **P2** | **类别**: 运营 · 部署可靠性 | **风险**: 运行中错误而非启动时错误  
> **代码证据**: `forge-core/cmd/forge/main.go` · `forge-core/cmd/forge/preflight.go` · harness

### 问题描述

ForgeOS Go 二进制文件号称「零外部依赖」（`go.mod` 无 `require`），这**在编译时是成立的**。
但在运行时，`forge gate/check/accept` 会 shell 出 Node.js 脚本：

```go
// cmd/forge/gates.go (gate.go 包的 delegate 函数)
// 间接通过 internal/gate/gate.go 调用 harness 脚本
func delegate(fn func(root string) gate.Result, args []string) int {
    // ...
    res := fn(*root)  // 内部 shell 出 node <harness-path>/gate.mjs
```

而 `forge run`/`forge evolve` 需要通过 yaml2json 读取 workflow YAML：

```python
# harness/yaml2json.py — forge-core 启动时会 shell 出 python3 调用
python3 harness/yaml2json.py < workflow.yml
```

当前 `forge preflight` 命令做了一些检查，但它与主执行路径是分离的。实际执行路径中：

1. **Node.js 版本未检查**: `harness/*.mjs` 使用了 `node:test` 等 ES2022 特性，需要 Node 18+。
   如果系统只有 Node 16，错误信息不会说「需要 Node >= 18」，而是抛出一个 JavaScript 解析异常。
2. **Python 版本未检查**: `yaml2json.py` 使用了 Python 3.8+ 的特性（赋值表达式 `:=`、`match` 语法）。
   如果 `python3` 指向 Python 3.6，会报 `SyntaxError`。
3. **子命令级依赖不一致**: `forge gate` 需要 Node，`forge run` 需要 Node + Python，但没有任何依赖
   清单告诉用户「安装这个子命令需要哪些运行时」。
4. **版本漂移会静默降级**: 如果安装了新版 Node 但某个 API 被废弃，harness 脚本可能会静默降级
   （例如用 try-catch 兼容旧版），导致治理强度不知不觉下降。

### 为什么这是信任缺口

用户初次尝试 ForgeOS 时，最常见的失败模式不是「代码写错了」，而是「环境不对」。如果 `forge accept`
在安装后的第一分钟因为 Node 版本问题抛出一个 cryptic error，用户的信任就破裂了。
对于 24h 无人值守运行，运行时依赖的版本一致性更关键——你不能假设部署环境永远不变。
Docker 镜像重建、OS 升级、Node/Python 版本更新都可能在不修改 ForgeOS 配置的情况下改变行为。

### 边界场景

- **Python 3→Python 2 静默切换**: 某些 Linux 发行版 `python3` 可能指向 Python 2（极少数旧配置），
  yaml2json 直接崩溃
- **Node 版本升级行为变化**: Node 22 改变了 `node:test` 的某些行为，harness `test_pass` 指标
  的含义悄悄变化，但 `forge accept` 仍然 ACCEPTED——治理标准在漂移
- **Docker 多阶段构建**: 第一阶段装 Node 18+Python 3.10，第二阶段忘了复制 Python——`forge run`
  失败但 `forge gate` 正常，诊断困惑

### 建议方向

- 每个子命令声明其运行时依赖版本范围（`internal/boot/checks.go`），在 CLI 入口处统一执行
  `exec.LookPath` + 版本号正则匹配（类似 `node --version` 解析）
- 依赖检查失败时，输出清晰的人类可读修复指南（不是 cryptic 错误）
- 建立「运行时依赖锁定」机制：读取 `.forge/requirements.lock`（类似 `go.sum`），校验当前环境的
  Node/Python 版本与最后验证通过的版本一致
- `forge doctor` 增加 `--deps` 模式，全量检查运行时依赖

---

## 方向三 · 治理策略变更影响预览

> **优先级**: 🟠 **P2** | **类别**: 运营 · 安全工程 | **风险**: 策略变更的未知后果  
> **代码证据**: `forge-core/internal/mode/mode.go` · `harness/policies.yml` · `.agent/policies/modes.yml`

### 问题描述

ForgeOS 的核心价值是「治理即代码」。但治理策略的变更——例如把 coverage 阈值从 60% 提升到 80%、
把某个 gate 从 `warn` 改为 `block`、把路由下限从 Haiku 抬到 Sonnet——**没有任何影响预览机制**。

当前，用户需要：
1. 编辑 `policies.yml` 或 `modes.yml`
2. 运行 `forge accept` 看 PASS/FAIL
3. 如果 FAIL，回溯到步骤 1 调整

这相当于在 CI 中「先提交，等 CI 跑完才知道是否通过」。对于只影响单个项目的变更（如本仓自己的
`project.yml` 调整），这可能还能接受。但对于**组织级治理策略的变更**（影响数十个 repo），
这种试错模式是不可接受的：

- **不可回滚的强制执行**: 把某个 gate 从 `warn` 改为 `block` 后，如果下游项目还没有准备好，
  所有 CI 全部变红——但改策略的人无法预先知道哪些项目会受影响
- **阈值调整的真实成本不透明**: coverage 从 60% 抬到 80%，听起来是好事，但如果大量项目目前只有 50%，
  这个变更意味着要么投入大量开发时间补测试，要么接受长期 CI 红——哪个后果更合理取决于数据，但
  当前没有任何「预览」给出这些数据
- **路由下限变更的预算影响未知**: 把 `balanced` 模式的 router 下限从 Haiku 抬到 Sonnet 会影响全仓
  每 agent-call 的成本，但没有任何 way 在变更前看到预估的额外支出

### 为什么这是信任缺口

治理层的信任建立在其**可预测性**上。如果策略变更的效果像黑箱一样不可预知，组织就不敢轻易调整策略——
这恰恰与「策略即代码，持续改进」的 DevOps 理念背道而驰。用户需要像 `terraform plan` 一样的能力：
在变更生效之前，看到变更影响的完整报告。

### 边界场景

- **级联失败**: 把 `blocking: true` 加到 SCA gate 后，所有未集成 OSV DB 的项目都过不了
  `forge accept`——但改策略的人并不知道哪些项目没配 SCA DB
- **阈值跨项目分布未知**: coverage 阈值抬到 80%，想确认哪些项目低于 80%，需要逐个项目手动跑
  `forge doctor`——没有聚合视图
- **回退路径不清晰**: `forge migrate --to engineering` 把整个治理严格度提升了，但执行后才发现
  项目无法通过新 gates。没有 `forge migrate --undo` 或 `--preview` 来先看一眼后果

### 建议方向

- `forge policy plan <file>` —— 分析策略文件的变更（对比当前生效策略与 proposal），输出：
  - 哪些 gate 的 enforce 级别会变化（warn→block 或反之）
  - 哪些阈值会变化（当前值 vs 新阈值，以及当前实际测量值的分布概况）
  - 哪些 router 下限会变化（以及预估的成本增量）
  - 受影响的 mode×lifecycle 组合列表
- `forge gate --dry-run --future-policy <file>` —— 用提案中的策略而不是当前策略跑一次 gate，
  报告差分
- 策略变更的版本化：支持 `policies.yml` 的 `canary` / `enforce` 两阶段部署，
  `warn` 模式报告违规但不阻断，`block` 模式才真正强制执行

---

## 方向四 · Agent 执行契约的 Schema 化与校验

> **优先级**: 🔴 **P1** | **类别**: 质量 · 管道完整性 | **风险**: agent 输出的机读标记被静默忽略  
> **代码证据**: `forge-core/cmd/forge/cost.go` · `.agent/agents/*.md` · `orchestrator/verdict_loopback_test.go`

### 问题描述

ForgeOS 经过 31 轮 sprint 建立了基于「机读契约」的 agent→orchestrator 通信协议：
- Reviewer 输出末行 `VERDICT: APPROVE` / `VERDICT: REQUEST_CHANGES` → 触发 loop-back
- CTO executive-review 输出末行 `VERDICT: APPROVE` / `VERDICT: REDESIGN` → 驱动收敛信号
- Product-manager discover 输出末行 `CONFIDENCE: 85` → 计算 `requirement_confidence`

这套协议是 ForgeOS 质量闭环的核心创新——它让 agent 的文本输出不仅限于人类阅读，还产生可机读的
控制流信号。但**这份协议目前没有形式化的 schema 声明**：

1. **没有协议规范文档**：每个 agent 卡中的契约是散文描述的（如 reviewer.md 的
   `VERDICT: APPROVE or REQUEST_CHANGES`），但没有任何地方给出正式语法：
   - 允许多少空格？`VERDICT:APPROVE` 合法吗？
   - 大小写敏感？`verdict: approve` 行不行？
   - 合约出现在哪一行？最后一行？包含关键字的任意行？当前实现取最后一行
     （`unwrapClaudeResult` + 末行匹配），但没有任何文档说「只有末行有效」
   - 是否可以包含其他文本？`VERDICT: APPROVE — looks good` 会被正确解析吗？
     （`cost.go:330 parseReviewerVerdict` 用 `strings.HasSuffix` 所以可以，但这不是 guarantee）

2. **没有格式校验**：如果 agent 输出了 `VERDICT: APPROVED`（多了一个 D ），当前行为是**静默忽略**
   ——`parseReviewerVerdict` 没匹配到，返回 `""`，然后 verdictLedger 没有记录，loop-back 不触发，
   review_status 保持 empty → convergence NOT MET。系统不会报错，不会告警"这个 agent 的输出格式
   不符合契约"。它只是悄悄地不走 loop-back。

3. **没有扩展性保证**：目前 3 种机读标记分别在 cost.go 的三个独立解析函数中实现
   （`parseReviewerVerdict` / `parseExecutiveVerdict` / `parseConfidenceScore`），
   作为三个串行的 fallback 调用。如果要加第四种，需要在 `observeFor` 的 fallback 链上再加一层。
   没有一个注册式的、可扩展的契约解析架构。

### 为什么这是信任缺口

机读契约是 ForgeOS 质量闭环的「神经网络」——它把自然语言输出转化为控制流决策。如果这个协议可以
静默失效（agent 输出了错误格式→不回灌→控制流断掉→但没有报警），那质量闭环就变成了「看起来有，
实际上在裸奔」。Sprint 23 已经坐实过类似问题（reviewer 裁决曾被完全静默丢弃），
当前的设计仍然允许同样模式再次发生，只是换了一种失效方式。

### 边界场景

- **claude 版本更新改变输出格式**：claude 4 的 CLI 输出格式与 claude 3.5 不同，`unwrapClaudeResult`
  可以解析新格式但 `VERDICT:` 行被 claude 包裹在解释性文本中——标记仍在但位置不对
- **非英语 agent**: 用中文 prompt 的 agent 可能输出「裁决：批准」而非 `VERDICT: APPROVE`，
  当前实现完全没有 i18n 考虑
- **agent 模板更新漂移**: 修改了 agent 卡但忘了更新机读契约，agent 输出新格式但解析器不认识
- **多行 VERDICT**: agent 的思维链中写了一句 `VERDICT: APPROVE` 作为推理过程，又在最后一行写了
  真正的裁决——末行字符串匹配可能匹配到推理行而非裁决行

### 建议方向

- 定义一个形式化的机读契约 schema（如 JSON Schema 或简单的 EBNF）：
  ```ebnf
  machine_contract = marker_line ;
  marker_line     = marker_name ":" WS marker_value [WS comment] ;
  marker_name     = "VERDICT" | "CONFIDENCE" | "REVIEW_STATUS" ;
  marker_value    = "APPROVE" | "REQUEST_CHANGES" | "REDESIGN" 
                  | "DELAY" | "REJECT" | "APPROVE_WITH_SIMPLIFICATION"
                  | DIGIT { DIGIT } "." DIGIT { DIGIT } ;  (* for CONFIDENCE *)
  ```
- 解析器中增加格式校验：如果一行以 `VERDICT:` 开头但与合法值不匹配，输出 WARNING（不是静默忽略）
- 沉淀可扩展注册表：`internal/contract/` 包，支持 `RegisterParser("VERDICT", fn)` 模式
- `forge validate --contracts` 模式：读取所有 agent 卡的机读契约声明，验证它们被注册了解析器

---

## 方向五 · 长运行时存储生命周期管理

> **优先级**: 🔴 **P1** | **类别**: 运营 · 数据工程 | **风险**: 磁盘爆满 / 性能退化  
> **代码证据**: `.forge/*.jsonl` · `forge-core/cmd/forge/memory-prune.go` · `forge-core/internal/memory/memory.go`

### 问题描述

ForgeOS 在运行时持续向 `.forge/` 目录追加数据。当前工作树的真实数据：
```
.forge/
  trace.jsonl      91 events (11KB)
  memory.jsonl     14 entries (2KB)
  checkpoint.json   1 entry (184B)
```

这在当前阶段是小数据。但在 24h 无人值守运行多个迭代后：
- **trace.jsonl**: 每 iteration 约 20-50 事件，每个 agent phase 约 5-10 事件。在 24h 内跑 50-100
  iteration 很正常，数据量可达 1-5MB。如果每天跑，一年后 trace 文件可达 1-2GB，但 `forge doctor` 仍会
  尝试全量读取 trace 并解析（`trace.Load` 读取整个文件到内存）。
- **memory.jsonl**: 每一轮发现的知识以 O_APPEND 方式追加，不做压缩。当前 `memory-prune` 命令存在，
  但必须手动调用：
  ```go
  // memory-prune 子命令
  "memory-prune": cmdMemoryPrune,
  ```
  `cmdMemoryPrune` 通过 `memory.Supersedes` 模式做压缩，但**演化循环从不自动触发它**。
  `LoopEngine` 的迭代中没有一个 "after N iterations, auto-compact memory" 的钩子。
- **checkpoint.json**: 当前单条目，每次写覆盖。但如果 checkpoint 格式演化为支持多个恢复点
  （如 phase 级粒度），就会变成一个追加日志，同样面临增长问题。

更深层的问题是**没有数据生命周期策略**：
- 没有 retention policy：保留最近 N 次运行？保留最近 N 天？
- 没有归档/轮转机制：关闭当前 trace.jsonl，开启新文件
- 没有自动的存储健康告警：`.forge/` 目录占用了多少空间？增长速度是多少？预计何时达到磁盘上限？

### 为什么这是信任缺口

一个 24h 无人值守的系统，如果因为磁盘满了而崩溃，是运维的耻辱——尤其是当这个系统的职责是「治理」。
更隐蔽的问题是：trace.jsonl 增大后，`trace.Load` 的读取时间和内存占用会线性增长。
用户不会立刻发现「forge doctor 变慢了是因为 trace 文件太大了」——他们只会觉得「ForgeOS 越来越卡」，
然后归因于错误的原因。

### 边界场景

- **ETL 式的接入场景**: ForgeOS 管理了一个繁忙的代码仓，每天 50+ PR，CI 为每个 PR 跑
  `forge accept`。一个月后 trace.jsonl 达到数百 MB，`forge doctor` 读取超时
- **`forge evolve` 长时间运行**: 一次 48h 的无人值守运行，memory 从 0 增长到 2000 条，
  prompt 注入的 memoryContext lane 虽然已加硬 cap（`memoryCap = 32`），但磁盘上的 jsonl 文件
  仍然在增长，从未被压缩
- **多个项目共享磁盘**: 如果 `/home/user/projects/` 下有 10 个 ForgeOS 治理的项目，
  每个项目的 `.forge/trace.jsonl` 和 `.forge/memory.jsonl` 各自增长，管理员不知道总占用
- **灾难恢复时发现 checkpoint 太旧**: restore 操作读取最后写入的 checkpoint.json，
  但那是 2 周前的状态（因为后来的 checkpoint 被某次中断破坏了），2 周的知识在 memory.jsonl 中，
  但 checkpoint 没有指向——两者没有交叉引用

### 建议方向

- 存储文件大小自动监控：`internal/persist/size.go` 在每次 Append 操作后检查文件大小，
  超过阈值（如 10MB）时自动轮转（`trace.1.jsonl` → `trace.jsonl`）
- 演化循环中新增 `afterCompact` 钩子：每 N 次迭代自动调用 memory 压缩
  （`memory.Compact` 和 `memory.Prune`）
- 为 `.forge/` 目录建立数据生命周期配置：
  ```yaml
  # project.yml 中可选的 storage 段
  storage:
    trace_retention: 30d         # 保留 30 天的 trace
    memory_max_entries: 5000     # memory 条目数上限（超限自动压缩）
    auto_compact_memory: true    # evolve 循环中自动压缩 memory
    storage_warning_mb: 100      # .forge/ 超过此值输出 WARNING
  ```
- checkpoint 升级为「指针 + 日志」模式：checkpoint 指向某个 trace 位置（seq 号），
  使得恢复时可以重放到精确的事件边界

---

## 优先级建议

| 方向 | 优先级 | 类别 | 风险 | 一句话杠杆 |
|---|---|---|---|---|
| **一 · Run Identity & 状态隔离** | **P1** 🔴 | 数据完整性 | 静默数据损坏 | 没有 run_id，`forge evolve` 同时跑两次就静默破坏对方的状态——这是 24h 系统的基本防线 |
| **四 · Agent 执行契约 Schema 化** | **P1** 🔴 | 管道完整性 | 控制流静默断裂 | 机读契约是整个质量闭环的神经，一旦静默失效，reviewer 裁决和收敛信号都变成摆设 |
| **五 · 存储生命周期管理** | **P1** 🔴 | 运营 | 磁盘爆满 / 性能退化 | 24h 无人值守系统不能假设有人盯着磁盘；自动轮转和压缩是零信任运维的前提 |
| **二 · 运行时依赖预检** | **P2** 🟠 | 部署可靠性 | 运行时错误 | 初次体验的信任门槛：如果 `forge gate` 因为 Node 版本报 cryptic error，用户不会再试第二次 |
| **三 · 策略变更预览** | **P2** 🟠 | 安全工程 | 变更冲击 | 治理即代码需要 `terraform plan` 式的安全感：改策略前先看见影响，不是赌 CI 会不会红 |

### 收敛建议

- **如果只做两件**: 方向一（Run Identity） + 方向四（契约 Schema 化）。
  一个保护数据不被静默污染，一个保护控制流不被静默断裂。
  这两个都是「系统性信任」的基石——不依赖 operator 的警觉，而是从架构上保证不会静默出错。
- **如果做三件**: 上加方向五（存储生命周期）。让无人值守运行不因磁盘打满而崩溃。
- **方向二和三** 可以在后续 sprint 按需补入——它们的触发条件是「有外部用户首次部署」（二）和
  「有组织级策略变更」（三），不是当前单仓 self-governance 的急迫问题。

---

## 与已有分析的差异化总结

本文不讨论以下已被充分覆盖的方向：
- 并发安全 / PID 文件 / 锁机制（已有 5+ 分析覆盖）
- CLI `--json` 结构化输出（已有 3+ 分析覆盖）
- Daemon 模式 / 持久进程（已有 1 篇分析覆盖）
- Phase 级 checkpoint 粒度 / 幂等性 / 补偿（已有 5+ 分析覆盖）
- 测试覆盖率提升 / 执法器盲区修复（已有 8+ 分析覆盖）
- 跨项目联邦 / 多仓库治理（已有 10+ 分析覆盖）
- 跨厂商模型路由 / LiteLLM / Firecracker（已有 5+ 分析覆盖，且均标注 BLOCKED-EXTERNAL）

本文 5 个方向的共同特征是：**不增加新功能，而是在已有功能基础上建立信任基础设施**——
让 ForgeOS 的运营者（而不是开发者）有信心把它部署到 24h 无人值守的生产环境中。
