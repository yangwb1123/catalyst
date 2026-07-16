# ForgeOS — 五个系统性声明与运维缺口（全局代码扫描）

> **角色**: 资深架构师 / 产品经理  
> **方法**: 完整通读 forge-core（18 Go 包 / 63 生产源文件 / ~35k LOC）· harness（39+ 模块 / ~10.5k LOC）·  
>   `.agent/`（5 workflow / 12 agent 卡 / 9 skill 卡 / 全部 ADR+DECISIONS / policies）·  
>   `pi-batch.py`（440 行）· `ai-dev/` · `.github/workflows/forge.yml` · `examples/`（go-taskd + url-shortener）·  
>   全部 31 个 Sprint 记录 · `docs/FUNCTIONAL_REQUIREMENTS_AUDIT.md`（123 条逐项分类 + 14 GAP 全部收口）·  
>   交叉验证 `docs/requirements/`（~134 篇）全部已有分析的核心理念组合词  
> **去重验证**: 对每个方向的**核心论点组合词**在全部已有文档中做全文精确检索，确认零篇系统性展开（见各方向首行的关键词命中统计）  
> **纪律**: 不编写任何代码。每个方向附带精确到 `file:line` 的代码级证据、边界情况（Edge Cases）、产品价值判断

---

## 阅读指引

经过 31 轮 Sprint 和 ~134 篇扩展分析，ForgeOS 的**运行时引擎层**已被深度覆盖。以下域不在本文讨论范围内：

| 域 | 覆盖状态 |
|---|---|
| 编排引擎（串行/并行/loop-back/mode-gating/checkpoint/resume/stop-condition） | 🟢 完备 |
| 安全护栏（递归深度/执行次数/墙钟超时/输出上限/run-level budget） | 🟢 完备 |
| 学习闭环（trace/scorecard/memory/converge 全 8 信号） | 🟢 完备 |
| 治理执法（arch-check 8 检查/check.py 10 检查/gate.mjs/secret-scan/SCA 框架） | 🟢 完备 |
| 模型路由（多维评分/Opus 安全下限/budget 降档/HistoryTiebreak） | 🟢 完备 |
| 真点火验证（multi-agent 端到端 + 8 真 bug + 三维成本遥测） | 🟢 完备 |
| 功能需求审计（DONE 90+ / BLOCKED-EXTERNAL 3 / DEFERRED-BY-DESIGN 15+ / GAP 14 → 全部收口） | 🟢 完备 |

本文聚焦于**五个在代码中有明确存在痕迹、关键概念被声明但从未系统性扩展的域**。它们不是增量功能缺口（那 14 个 GAP 已在 Sprint 30-31 全部收口），而是 **ForgeOS 作为治理平台的几个基础面缺层**——每一层决定了系统能否从「单项目自治编排工具」向「组织级 AI 软件工厂」跨越。

---

## 方向一 · 跨文件声明一致性校验 —— 治理平台的自洽性基础

> **类型**: 治理完整性 · 正确性 · **优先级**: P0（高）  
> **关键词命中**: `cross.file.*valid\|inter.doc.*consist\|multi.file.*valid\|declaration.*consist`  
> **已有分析覆盖**: **0 篇**。这是一个完全未被触及的方向。

### 问题

ForgeOS 的整个治理模型建立在**一组跨文件、跨格式的声明式资产**之上——但没有任何机制验证这些资产之间的**互一致性**。

当前一致性校验的边界：

```
# check.py 的 10 项检查全部是单文件内引用校验:
#   check_workflow_agent_refs      — workflow 中引用的 agent 卡存在 .agent/agents/ 下
#   check_workflow_control_flow    — workflow 的 target_phase/next_stage 指向已有 phase
#   check_workflow_model_tiers     — workflow 声明 model_tier ∈ {haiku,sonnet,opus}
#   check_mode_priorities          — mode priorities 无矛盾 (speed/quality/cost)
#   check_workflow_mode_gating     — workflow mode_gating 与 modes.yml 不漂移
#   ... 全部文件内/邻域引用
```

但**跨两个及以上文件的语义一致性**完全不受检：

**示例 1**: `workflow_depth` 的声明三元组互不一致

```
# modes.yml#workflow_depth 定义:
#   reviewer: {explorer: skip, balanced: standard, engineering: full, cto: full}
# review.yml#required_when  引用:
#   required_when: ../policies/modes.yml#workflow_depth.reviewer
# build.yml#required_when    引用:
#   required_when: ../policies/modes.yml#workflow_depth.reviewer
#
# 🚫 不存在任何校验确保:
#   - build.yml 和 review.yml 的 required_when 指向同一个 modes.yml 字段
#   - 该字段在 modes.yml 中确实存在 (`workflow_depth.reviewer`)
#   - 当前 mode 的深度值符合 workflow 的预期
#
# 当前状态:三个文件各自拼写正确但不验证互指一致性
```

**示例 2**: Workflow 的 `requires_tools` 声明的幻影依赖

```
# discover.yml 的 market-research phase 声明：
#   requires_tools: [web_search, web_fetch]
#   注释说「degrade to advisory + flag when the tool is absent」
#
# 🚫 不存在以下校验:
#   - web_search / web_fetch 在全局是否有定义（哪个 agent 提供了它们？）
#   - 当前 executor 是否支持这些工具
#   - "degrade + flag" 的行为是否真的实现了（代码搜索:零实现）
```

**示例 3**: Agent 卡的机读契约声明 vs 运行时消费者的不一致

```
# reviewer.md 声明:
#   VERDICT: APPROVE
#   VERDICT: REQUEST_CHANGES
# cto.md 声明:
#   VERDICT: APPROVE / APPROVE_WITH_SIMPLIFICATION / REDESIGN / DELAY / REJECT
# product-manager.md 声明:
#   CONFIDENCE: <0-100>
#
# cost.go 里分别有:
#   parseReviewerVerdict()    — 读 reviewer 卡
#   parseExecutiveVerdict()   — 读 cto 卡
#   parseConfidenceScore()    — 读 pm 卡
#
# 🚫 不存在以下校验:
#   - cost.go 的解析器 token 集与 agent 卡的声明 token 集是否一致
#   - 当 agent 卡被更新（新增 REDESIGN_WITH_SCOPE）而解析器未更新时，零告警
```

**示例 4**: Mode 切换成本的多维影响面

```
# forge migrate --to engineering 可以改变:
#   - gate-set（6 gates 全开）
#   - router floor（从 haiku 抬到 sonnet）
#   - workflow 深度（discover/review/evolve 全深度）
#   - coverage 阈值（到 80%）
#   - 派生任务（5 个补债任务注入 ROADMAP）
#
# 🚫 不存在以下校验:
#   - migration 前后所有引用的 modes.yml 字段名一致
#   - 不存在 migration 到新 mode 后某些 workflow phase 引用已不存在的字段
#   - 不存在 migration 产生的补债任务与已有 ROADMAP 条目的冲突
```

### 为什么需要

ForgeOS 的治理模型**本身就是声明式资产构成的图**。当这个图有断边（broken edge）或自相矛盾（self-contradiction）时：

1. **系统不可信**: 治理平台自己治理不了自己的声明资产。这是「治理 OS」的元层面自洽性问题。
2. **静默静默降级**: `required_when` 指向不存在的 mode 字段时，phase 可能意外地永远跑或永远不跑——没有告警。
3. **迁移不安全**: `forge migrate --to engineering` 是一次状态迁移，不验证迁移后全图的一致性就让一个项目静默地处于治理漂移状态。

### 代码证据

| 位置 | 证据 |
|---|---|
| `harness/check.py:12-15` | 10 项检查全部是单文件/邻域引用校验 |
| `internal/yamlpath/yamlpath.go:57` | `Parse` 只解析语法，不验证目标存在性 |
| `internal/yamlpath/yamlpath.go:139` | `Resolve` 在**运行时**才解析目标——不是 CI 时 |
| `.agent/workflows/build.yml:78` | `required_when: ../policies/modes.yml#workflow_depth.reviewer`——CI 零校验 |
| `.agent/workflows/discover.yml:44` | 同上模式 |
| `.agent/workflows/review.yml:80-101` | 三个 phase 各自引用 `workflow_depth.reviewer`——无三端一致性检查 |
| `cmd/forge/cost.go:300-400` | 三个解析器各自硬编码 token——与 agent 卡声明的契约无链接 |

### 边界情况

1. **软删除 vs 硬删除**: 当 `modes.yml` 中一个字段被删除，引用它的 workflow 在运行时由 `Resolve` 返回错误。但 CI 阶段无检查，错误只出现在 `forge run` 时——已太晚。
2. **别名与重命名**: 如果 `workflow_depth.reviewer` 被重命名为 `workflow_depth.review`，旧 workflow 静默走到运行时才报错。没有迁移路径。
3. **版本兼容性**: `forge-init` 从模板复制 workflow 后，模板引用的是模板时代的 `modes.yml` 字段。项目自定义 modes.yml 后这些引用可能已不适用。

### 推荐实现量级

2-3 个 Sprint。核心是新增一个 `harness/check_declarations.py` 或扩展 `check.py`，对跨文件引用做图遍历校验：
- 建立从 `modes.yml` 出发的字段引用图
- 验证 `required_when`/`requires_tools`/`uses_template`/`secondary_template` 等全部引用目标的端到端可达性
- 验证 agent 卡机读契约与解析器的 token 一致性

---

## 方向二 · 编排性能基准检测线 —— 24h 运行的无声退化风险

> **类型**: 性能 · 运维 · **优先级**: P1（中高）  
> **关键词命中**: `benchmark.*gate\|perf.*regress.*ci\|performance.*baseline\|benchmark.*ci.*pipeline`  
> **已有分析覆盖**: **1 篇**旁证句子（作为「CI 增强」的子句提及，未系统展开）。

### 问题

ForgeOS 的 CI（`.github/workflows/forge.yml`）跑 `forge run build --executor dry` 作为冒烟测试——但**什么也不测量**：

```yaml
# .github/workflows/forge.yml:51-55
- name: forge run build --executor dry
  run: |
    go -C forge-core build -o /tmp/forge-test ./cmd/forge
    /tmp/forge-test run build --executor dry --root $PWD
```

这个步骤验证的是「forge run 能跑到终点不崩溃」。但不验证：
- 它跑了多久（wall-clock time）
- 转码了多少 YAML 资产（`yaml2json.Decode`/python shim 调用次数）
- 内存峰值是多少（`RunFrom` 在一次循环中积累多少状态）
- trace 事件写入了多少条
- `checkpoint.go` 的 atomic write 开销

**代码中的孤立基准测试**：

```
# 三个微基准——各自测一个极窄的函数，无法反映端到端系统性能:
internal/trace/trace_bench_test.go        → 纯 encode/decode 微基准
internal/asset/asset_bench_test.go        → 纯 JSON 解析微基准
internal/converge/converge_bench_test.go  → 纯 evaluate 微基准
```

**三个微基准都在 `go test` 中跑但 CI 不报告它们的结果**。每次提交可能引入的性能退化完全不可见。

### 为什么需要

1. **24h 自治运行是卖点**: 性能退化的放大系数是轮次。一次 `forge evolve` 跑 50 轮迭代——每轮慢 2 秒就是 100 秒的墙钟浪费。
2. **子进程开销不透明**: `CommandExecutor` 每 phase spawn 一个进程。无基准线追踪，无法知道「是 agent 慢还是 forge 编排慢」。
3. **yaml2json 性能是隐藏瓶颈**: 手写 Go YAML 解析器（`internal/yaml2json`，~1100 行）和 python shim 两次解码路径——无性能基准线，无法评估替换 python shim 的收益。

### 代码证据

| 位置 | 证据 |
|---|---|
| `.github/workflows/forge.yml:51-55` | `forge run build --executor dry` 只冒烟不测性能 |
| `internal/trace/trace_bench_test.go` | 微基准存在但 CI 不消费 |
| `internal/asset/asset_bench_test.go` | 同上 |
| `internal/converge/converge_bench_test.go` | 同上 |
| `internal/yaml2json/yaml2json.go:70` | 手写 YAML 解析器，无性能基准 |
| `internal/yamlpath/yamlpath.go:139` | 每引用解析 spawn python 子进程——N+1 模式 |
| `cmd/forge/evolve.go:467-482` | trace 文件在 10MB 时轮转，但无时间维度的性能指标 |

### 边界情况

1. **基准测试的基准线漂移**: CI runner 的硬件规格变化会自然改变绝对时间。需要历史基线存储（如 `scorecards.json` 的 p95 模式），而非硬编码阈值。
2. **python shim 的延迟方差**: `yamlpath.resolveRef` 每调用一次 spawn 一个 python 进程。在容器化 CI 中首次 spawn 耗时与裸机差距很大。
3. **并行 vs 串行的性能对比**: `RunParallel` 当前 opt-in。需要能测出「相同 workflow 在串行 vs 并行下的 wall-clock 差异」的基准设置。

### 推荐实现量级

1-2 个 Sprint。CI 中新增一步 `forge run build --executor dry --bench`，让引擎输出结构化 benchmark 数据（phase 时长 / 子进程数 / 内存 / trace 大小），写入 `.forge/bench.jsonl`，并由一个轻量脚本计算与历史基线的偏差。

---

## 方向三 · 凭证与密钥生命周期管理 —— 自治运行的资源供给线

> **类型**: 安全性 · 运维 · **优先级**: P1（中高）  
> **关键词命中**: `credential.*rotat\|secret.*expir\|key.*rotat\|credential.*lifecycle\|api.*key.*manage`  
> **已有分析覆盖**: **2 篇**旁证句子（作为「真点火障碍」的子句提及，未系统展开）。

### 问题

ForgeOS 的`secret-scan.mjs` 阻止开发者把密钥提交到仓库，但 ForgeOS **自己作为一个自治运行时**需要一组不提交到仓库的密钥来工作：

```
# forge 运行时需要的凭证（全部由环境变量提供，无任何生命周期管理）:
#
# 1. LLM API Key: claude CLI 从 ~/.claude/credentials.json 或 $ANTHROPIC_API_KEY 读
#     → 过期后 forge run --executor=command 静默失败
#     → 无告警、无预过期检查、无自动轮转
#
# 2. Git Token: forge evolve 可能需要 push 分支 / 创建 PR
#     → 无 token 存在性检查、无 scope 验证
#     → 如果 token 只有读权限但 workflow 需要写 push——运行时才发现
#
# 3. CI 环境中的凭证: .github/workflows/forge.yml 使用 github.token（隐式）
#     → 无显式声明 forge 对 CI token 的权限需求
#     → 当 workflow 需要 security-events: write 权限时，CI 不报错但行为异常
#
# 4. SCA 数据库: sca.mjs 查询 OSV 数据库（无需密钥，但差本地 DB）
#     → 无「外部依赖健康检查」: 数据库不可用时 N/A 降级而不是告警
```

**当前系统的缺位**：

```go
// command_executor.go:51-80 中 agent 命令的构建
//   只有 agentCmd、agentPermission、agentAllowedTools、agentMaxBudgetUSD
//   没有任何凭证管理/注入/轮转的概念
```

```python
# pi-batch.py:220-240 中子进程 CLI 调用
#   只传了 prompt 和 model——凭证交给了子进程自己的配置
#   pi-batch.py 自己不做任何凭证检查
```

```javascript
// harness/secret-scan.mjs:65-80
//   扫的是硬编码 secret——但不扫环境变量是否就绪
//   「forge run 之前检查 API key 是否存在」——零实现
```

### 为什么需要

1. **自治运行需要自治凭证管理**: 24h 运行中间 API key 过期 = `forge evolve` 静默崩溃。`secret-scan` 管了静态的以防万一，但没管动态的以保证运行。
2. **真点火的准入检查**: `forge run --executor=command --agent-cmd claude` 需要 claude CLI 配置好 API key。当前唯一的检查是「claude 在 PATH 里」（Sprint 24 的 bug fix）。没有 key 是否有效、未过期、有够额度（rate-limit）的检查。
3. **最小非预期权限**: 当前 forge 不声明它需要什么外部权限。CI token 可能因为缺少 `contents: write` 而让 `forge evolve` 的 roadmap 更新功能静默降级。

### 代码证据

| 位置 | 证据 |
|---|---|
| `secret-scan.mjs` | 只扫仓库内硬编码 secret，不检查运行时凭证 |
| `command_executor.go:51-80` | agent 命令构建与凭证完全解耦——把问题全推给了外部 CLI |
| `cmd/forge/engine_build.go:120-130` | `claudeArgv` 构建只处理 permission/model/budget，不验证凭证 |
| `pi-batch.py:220-240` | 子进程 CLI 调用不做任何预检查 |
| `harness/sca.mjs` | OSV 查询无需验证，但外面差离线数据库——无健康检查 |
| `.github/workflows/forge.yml` | 隐式使用 `github.token`，无显式 permission 声明 |

### 边界情况

1. **多云策略**: 如果最终跨厂商（v3），每个 provider 各有自己的 key 过期周期和 rate-limit 限。需要统一凭证管理面。
2. **轮转中的并发运行**: 一个 `forge evolve` 会话中 key 被轮转。没有「凭证热加载」机制——当前设计假设运行中 key 不变。
3. **最小权限审计**: CI token 有超出 forge 所需权限的可能。没有工具可以验证「CI 的 GITHUB_TOKEN 权限 == forge 所需权限」。

### 推荐实现量级

2-3 个 Sprint。核心新增 `forge doctor` 子命令增强 + `forge preflight` 的凭证检查 + 一个 `forge secrets` 子命令：
- `forge doctor --credentials` 检查所有外部依赖的凭证是否就绪
- `forge preflight <workflow>` 检查当前环境的 credentials 是否满足 workflow 声明的 requires_tools
- `forge secrets rotate` 密钥轮转辅助

---

## 方向四 · "干跑 vs 真实"碰撞测试线 —— 回音谷验证假象的结构性风险

> **类型**: 正确性 · 测试基础设施 · **优先级**: P1（中高）  
> **关键词命中**: `dry.run.*vs.*real\|echo.*vs.*llm\|stub.*vs.*real\|dry.run.*collision\|simulat.*vs.*execut`  
> **已有分析覆盖**: **2 篇**次提及（作为「测试不足」的子句，未系统展开）。

### 问题

ForgeOS 的发布质量高度依赖 `--executor dry`（干跑/echo）下的测试。但「干跑全绿 → 真跑一样绿」的假设有**结构性风险**——因为干跑路径和真跑路径共享基础代码但不共享关键行为代码。

**关键分离点**：

```
# Engine 的 Exec 接口 (internal/orchestrator/executor.go:18-22):
type AgentExecutor interface {
    Exec(ctx context.Context, p asset.Phase, mode string) (string, error)
}

# 两个实现:
# - DryRunExecutor (executor.go:30-50): 只打印路由决策，不调 LLM
# - CommandExecutor (command_executor.go:30-310): spawn 真子进程
```

**风险在哪里**：

```go
// orchestrator.go:261-285 中 runAgentPhase 的判断逻辑:
func (e Engine) runAgentPhase(ctx context.Context, p asset.Phase, mode string) error {
    result, err := e.Exec.Exec(ctx, p, mode)
    // 这段代码对两个 Executor 实现是 SHARED 的
    // ——但真正跑通的测试只覆盖了 DryRunExecutor

    // 更微妙的是: agentOutcome (orchestrator.go:321-333)
    // 依赖 AgentVerdict puller——它在干跑下不被设置
    // 所以干跑跑通不等于真跑通
}
```

**具体碰撞案例（已真实发生）**:

```
Sprint 24-26 真点火暴露的 8 个 gap 全部是「干跑测不出」:
  ③ 模型路由 — Build 无 --model, routing 计算结果被丢弃
  ④ 工作目录 — CommandExecutor.Dir 未设置，agent 在错误目录写码
  ⑤ 成本第三维 — claude --max-budget-usd 未传递
  ⑥ trace latency — 干跑不产生墙钟耗时，duration_ms 恒 0
  ⑦ cost telemetry — 干跑不产生真实计费
  ⑧ reviewer 缺前序 gate 信号 — 干跑不出 gate 裁决
```

**但目前没有系统性的方法回答「一个变更在干跑下全绿，在真跑下出现什么新行为？」**。

### 为什么需要

1. **回音谷（echo chamber）模式是默认开发流速模式**: 几乎所有 Sprint 的测试都是在干跑下验证的。这不是 bug——但无系统的碰撞检测意味着每个新功能上线前都需要手动真跑一次才知道干跑没覆盖什么。
2. **行为分叉不被检测**: 当 `CommandExecutor` 一个新 bug 被引入（如进程组泄漏、timeout 不对），干跑测试完全看不见，直到真跑炸了才被发现。
3. **假阳性的反面**: 干跑可能过于悲观——`BudgetExhausted` 在干跑下永不为 true，但真跑下可能耗尽；`OnGateResult` 在干跑下可能产生不同于真跑的裁决序列。

### 代码证据

| 位置 | 证据 |
|---|---|
| `internal/orchestrator/executor.go:18-22` | `AgentExecutor` 接口有两个实现，但测试只覆盖其一 |
| `internal/orchestrator/orchestrator_test.go` | 测试全用 `DryRunExecutor`/`fakeExecutor` |
| `internal/orchestrator/command_executor_test.go` | `CommandExecutor` 的测试只测 spawn 过程本身，不集成到 `RunFrom` |
| `cmd/forge/evolve_test.go` | `TestRun_BuildWF_DryRun` 等全用 exec dry |
| `cmd/forge/main_agent_test.go` | fake-agent 测试能逼近真跑，但覆盖率有限 |

### 边界情况

1. **Partial Collision**: 不是所有行为都会 collision。需要自动化方法系统性地标注「干跑测试覆盖了」vs「只真跑才覆盖」的代码路径。
2. **Inject 模式**: 一个中间方案——在干跑下伪造 `CommandExecutor` 的行为特征（exit code、timeout、output），验证编排器的重试/backoff 逻辑是否在两类 executor 下都正确。
3. **真实成本**: 在 CI 中跑真 agent 需要 API budget。碰撞检测必须能 opt-in 且 budget-aware。

### 推荐实现量级

2-3 个 Sprint。核心思路不是「在 CI 里跑真 agent」，而是建立一个 `DryVsRealCollisionSuite`：

1. 枚举所有「干跑和真跑代码路径分叉点」（executor/executor.go 的 Exec + runAgentPhase + gates.go + timeout/backoff）
2. 对每个分叉点，在一个注入式测试中跑**完全相同的 workflow** 在两种 executor 下
3. 断言两者的 log 事件序列、phase 裁决序列、gate 结果序列在语义上等价
4. CI 中作为 `forge accept` 的一步

---

## 方向五 · 自描述能力与发现协议 —— 从"CLI 工具"到"可集成平台"的桥梁

> **类型**: 平台化 · 集成 · **优先级**: P1（中高）  
> **关键词命中**: `self.describ\|discover.*protocol\|capability.*discover\|forge.*describe\|metadata.*export\|machine.*readable.*workflow`  
> **已有分析覆盖**: **3 篇**旁证（作为「外部集成」的子句提及，未系统展开）。

### 问题

ForgeOS 对自己的能力没有**机器可读的描述协议**。当前，了解一个 ForgeOS 项目的唯一方式是：

```
# 人工读 .agent/ 目录（散文格式）:
ls .agent/workflows/         # 列出 5 个 YAML
ls .agent/agents/            # 列出 12 个 Markdown
cat .agent/project.yml       # 读 mode + lifecycle
cat .agent/policies/modes.yml  # 读模式定义，再手动映射到代码行为

# 或运行 CLI 子命令（有格式但不一致）:
forge run --help             # 分散到各个子命令
forge validate --models      # 只检查 model 引用
forge doctor                 # 诊断但不输出结构化元数据
```

**缺乏的协议面**：

```
# 不存在以下命令/端点:
#   forge describe               → 当前项目的完整元数据（JSON/YAML）
#   forge describe workflows     → 可用 workflow 列表 + 各自的 phase 拓扑
#   forge describe gates         → 可用 gate 列表 + 当前环境哪些能真跑
#   forge describe capabilities  → 当前 executor 能做什么
#   forge describe traits        → 项目声明了哪些需要外部资源
#
# 这导致外部工具（IDE 插件、CI 编排器、Dashboard）无法：
#   - 发现项目有哪些 workflow 可用
#   - 判断当前环境是否满足 workflow 的 requires_tools
#   - 在 CI 中自动选择合适的 forge accept 参数
#   - 跨项目聚合治理状态
```

**当前最接近机器可读的是 YAML 文件本身**——但它们的 schema 没有正式定义为机器可消费的格式：

```yaml
# .agent/project.yml:
#   mode: engineering        # 消费方需要知道这是固定的枚举值
#   lifecycle: mvp           # 同上

# .agent/routing/scorecard.schema.yml:
#   这是 schema 但 forge 不 export 它
#   外部工具不知道 ForgeOS 识别的 task_type 枚举值
```

### 为什么需要

1. **CI 集成需要发现**: `.github/workflows/forge.yml` 硬编码了 `forge run build`。如果项目自定义了 workflow 名称，CI 不知道。如果 `forge describe workflows` 返回结构化列表，CI 可以自动选。
2. **IDE 集成需要能力声明**: 一个 IDE 插件需要知道「这个 project 用了哪些 gate」、「当前的 mode 跳过了哪些 phase」、「reviewer 在当前 mode 下是 mandatory 还是 optional」——当前都要人工 parse 散文 YAML。
3. **跨项目聚合的基础**: 方向一（跨文件一致性）的验证范围可以扩展到跨项目——通过 `forge describe` 的输出比较两个项目的治理声明差异。
4. **自监控**: `forge doctor` 当前是文本诊断。可扩展为输出机器可读的健康状态，供集群监控系统（Prometheus）消费。

### 代码证据

| 位置 | 证据 |
|---|---|
| `cmd/forge/main.go:69-76` | 子命令表——没有 `describe`、没有 `capabilities` |
| `cmd/forge/status.go` | `forge status` 输出文本但无 JSON 模式 |
| `cmd/forge/doctor/doctor.go` | `doctor` 诊断文本输出 |
| `.agent/project.yml` | 结构有 schema 但 forge 不 export |
| `.agent/routing/scorecard.schema.yml` | schema 文件存在但不被 CLI 暴露 |
| `harness/check.py:10-15` | 治理检查全部是文本输出 |
| `internal/mode/mode.go:Effective()` | mode 政策逻辑在代码里硬编码——不通过 describe 输出 |

### 边界情况

1. **describe 输出的版本化**: `forge describe` 的输出本身需要版本化（`_format: forgeos.describe.v1`），这样未来的 schema 变更不会破坏已有消费者。
2. **动态能力 vs 静态声明**: `forge describe capabilities` 需要区分「声明的能力」（modes.yml 说这个 mode 应该开 6 gates）和「实际能力」（当前环境只有 3 个 gate 工具有效）。前者静态来自 `.agent/`，后者动态来自 `forge doctor`。
3. **输出格式的选型争议**: JSON 更通用但 schema 文件要维护；YAML 更接近本项目的 DSL。建议 JSON，因为消费者主要是 CI/IDE/监控系统而非人类。
4. **安全**: 暴露 `forge describe` 会不会意外泄露项目配置信息？需要能限制输出（如 mask `agent-cmd` 地址）。

### 推荐实现量级

2 个 Sprint。核心是新增 `cmd/forge/describe.go` 和 `internal/describe` 包，导出结构化 JSON：

- `forge describe project` — project.yml 的内容 + 解析后的 mode/lifecycle effective 值
- `forge describe workflows` — 每个 workflow 的 phases/agents/gates/stop-condition 的机器可读表示
- `forge describe gates` — 当前环境各 gate 的状态（可用/不可用/N/A）
- `forge describe --json` — 全量描述，供外部工具消费

---

## 优先级汇总

| 方向 | 优先级 | 类别 | 杠杆 | 代码面与影响面 |
|---|---|---|---|---|
| **一·跨文件声明一致性** | **P0** | 治理完整性 | 最大:治理平台的元自洽性 | 影响 5+ YAML × 12 agent 卡 × modes × routing 的全网拓扑 |
| **二·性能基准检测线** | P1 | 运维 | 中:预防无声退化 | 3 个现存微基准 + CI 1 步新增 + bench 框架 |
| **三·凭证生命周期管理** | P1 | 安全/运维 | 中:真点火的前置条件 | secret-scan 扩展 + doctor 增强 + preflight 凭证检查 |
| **四·干跑 vs 真跑碰撞检测** | P1 | 测试基础设施 | 高:结构性测试假象 | Executor 接口分叉点枚举 + 注入式碰撞测试 |
| **五·自描述与发现协议** | P1 | 平台化 | 中:外部集成的桥梁 | 新增 `describe` 子命令 + `internal/describe` 包 |

**收敛建议**: 如果只能做一件，选择**方向一（跨文件声明一致性校验）**。因为这是治理平台的元自洽性问题——ForgeOS 治理其他项目的代码质量，但自己的声明资产图没有一致性护栏。方向五（自描述协议）是方向一的自然输出面：一旦有了跨文件一致性验证，把验证结果以结构化格式输出就是一个小 step。

---

> 截至 2026-07-11，文档中所有方向的核心论点经 `docs/requirements/`（~134 篇）+ `docs/analysis/`（~40 篇）+ `docs/FUNCTIONAL_REQUIREMENTS_AUDIT.md`（123 条）全文交叉验证，确认均未被作为独立方向系统展开。代码级引用精确到当前代码库内的 `file:line`。
