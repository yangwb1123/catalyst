# ForgeOS — 全局深扫后的高价值扩展方向

> **角色**: 资深架构师 / 产品经理  
> **方法**:  
> 1. 全局逐文件扫描完整代码库: forge-core(19 Go 包,~35k LOC 生产代码,纯 stdlib 零依赖)、
>    cmd/forge(17+ 子命令,~12k LOC)、harness(39+ Node/Python 模块,~10.5k LOC 执法层)、
>    `.agent/`(12 agent 卡,9 skill 卡,5 工作流,完整治理骨架)、
>    examples/(url-shortener, go-taskd)、pi-batch.py(499 行无治理脚本)
> 2. 阅读 FUNCTIONAL_REQUIREMENTS_AUDIT.md(全字段收敛信号审计,所有 GAP 已收口)
> 3. 阅读 CURRENT_SPRINT.md(31 轮 sprint 完整演进,真点火坐实 8 个真实 gap)
> 4. **差异化验证**: 对每个方向的核心关键词在 90+ 份已有 `docs/requirements/` 和
>    `docs/analysis/` 文档中做全文检索,确认该方向作为独立扩展方向是否已被深入覆盖
> 5. **纪律**: 不编写任何代码。每个方向附代码级证据、边界情况、与已有覆盖的关系说明。
> **日期**: 2026-07-10

---

## 全景定位:90+ 已有分析覆盖了极广的表面

ForgeOS 项目已积累约 90 份分析文档(其中仅 `docs/requirements/` 就有 56 篇,加上
`docs/analysis/` 约 40 篇),覆盖了几乎所有可触及的功能域:

| 覆盖域 | 代表性文档 | 方向数 |
|---|---|---|
| 引擎补齐(编排/路由/记忆/收敛/信号/诊断/并行/wave/loop-back) | 大部分 requirements 文档 | ~30 |
| 生产可靠性(Prompt QA / 信号硬化 / 环境验证 / 健康契约) | `expansion-production-readiness.md` | ~15 |
| 执行语义形式化(原子性/幂等/因果一致性/回滚/版本演化) | `execution-semantic-gaps.md` | ~10 |
| 二阶系统问题(知识衰减/配置爆炸/TOCTOU/数据生命周期) | `second-order-architectural-gaps.md` | ~10 |
| 系统边界(跨进程/信任边界/持久语义/并行安全/级联截断) | `v22~v33` | ~12 |
| 跨项目治理漂移 / 事件驱动 / 收敛诊断 / 自学免疫 | `novel-five-perspectives.md` | ~5 |
| 并行瓶颈 / 资源盲区 / git 降级 / 三存储一致性 / doctor 未融入循环 | `production-hardening-five-v42.md` | ~5 |
| 二进制分发 / 状态灾难恢复 / 结构化输出 / 多会话协调 / 数据生命周期 | `genuine-uncovered-five-binary-state-output-session-datalifecycle.md` | ~5 |
| 子进程错误协议 / 测试跳过侵蚀 / ContextCache 一致性 / pi-batch 失治 / 收敛预算分配 | `expansion-five-systemic-architectural-gaps.md` | ~5 |
| 跨会话可移植工作空间 / 自引用健康仪表盘 / 环境多态 / 自校准阈值 / Agent 连接池 | `truly-novel-five-directions.md` | ~5 |

**本文的 3 个方向落在上述密集覆盖域之外** —— 它们不是「缺什么引擎」或「架构新层」,
而是代码级观察结合系统性推理后发现的**结构性弱点**,且在已有文档中从未作为独立方向展开。

---

## 方向一 · 跨示例管线回归保险库

**优先级**: 🔴 P0 | **类别**: 治理 · 质量保证 | **预估**: ∼1.5 sprints | **杠杆**: ⭐⭐⭐⭐⭐

### 问题描述

ForgeOS 有两个端到端构建的真实示例应用:
- **`examples/url-shortener`** — 5 个源文件,39 个测试,经完整 architect→3×implementer→reviewer→fix 管线建成
- **`examples/go-taskd`** — 6 个源文件,Go 项目,经同样的管线建成

这两个示例是 ForgeOS「工厂已证可用」的唯一**外部可验证**证据。但当前 CI 中没有任何步骤
使用它们作为回归测试:

```
# .github/workflows/forge.yml (当前 CI)
步骤:
  1. node harness/acceptance.mjs        # 聚合闸门(检查 forge-core 自己)
  2. go -C forge-core build ./...        # 编译 forge-core
  3. go -C forge-core test ./...         # forge-core 单元测试
  4. go -C forge-core test -race ./...   # forge-core 竞态测试
  5. node --test harness/                # harness 自测(246 项)
  6. forge run build --executor dry      # 编排干跑(不调 LLM)
  # ❌ 没有任何步骤重建示例或验证管线完整性
```

这意味着:如果一次 forge-core 的提交破坏了工作流解析(例如 yaml2json 回退路径中断)、
相位路由逻辑、或 gate 执行契约,**没有任何自动化机制可以在 CI 中检测到**。
只有在某人手动重新运行完整的端到端管线时才会暴露。

### 代码级证据

**证据 A: CI 无任何示例引用**

```bash
$ grep -rn "url-shortener\|go-taskd\|examples" .github/workflows/forge.yml
# 空 —— 无引用
```

**证据 B: 示例使用独立的语言和测试框架,与 forge-core 自测无交集**

```
url-shortener: Node.js / node:test / 39 测试
go-taskd:      Go     / go test    / 4 测试文件

forge-core:    Go     / go test -race / 28+ 测试文件(全部在 forge-core/ 内)
```

它们唯一的交集是 `acceptance.mjs` 的 `app_test_pass` 检查,但该检查:
- 只运行已存在的单元测试(验证示例代码本身正确)
- 不运行 forge 管线(不验证 forge 能否从零构建示例)

**证据 C: 当前 `forge run build --executor dry` 步骤不触及示例逻辑**

```yaml
- name: forge run build --executor dry (end-to-end orchestration smoke test)
  run: |
    go -C forge-core build -o /tmp/forge-test ./cmd/forge
    /tmp/forge-test run build --executor dry --root $PWD
```

它用 `--executor dry` 运行 `build.yml` 工作流,所以:
- 不会真正调用 agent
- 不会创建或修改任何文件
- 只验证工作流解析和相位路由逻辑在本仓的 `.agent/workflows/` 上是否正常
- 完全不触及 `examples/` 目录

**证据 D: forge-core 的 `loadWorkflow` 路径存在双解析器分歧风险**

```go
// cmd/forge/main.go
func loadWorkflow(root, name string) (*asset.Workflow, error) {
    val, err := yaml2json.Decode(f)  // Go native hand-rolled parser
    if err != nil {
        // fallback to Python PyYAML
    }
}
```

如果一个语法在 Go 解析器中解析正确但在 Python 解析中产生不同结果(或反之),
没有示例管线回归测试来捕获这个分歧。yaml2json 包有 1565+ 行手写 YAML 解析代码,
其正确性只在 forge-core 自己的 7 个 YAML 文件上验证,从未在示例项目的 YAML 上验证。

### 边界情况

| 场景 | 影响 | 示例 |
|------|------|------|
| 工作流解析返回成功但行为有微妙变化 | 示例构建在旧版 fork 中以不同方式工作 | 某相位被静默跳过但 forge 自测仍 PASS |
| gate 契约参数变化 | 示例项目的 gate 配置与 forge-core 预期不匹配 | `--allowedTools` 格式变更导致 agent 无法自检 |
| 默认值变更 | 示例项目中未显式覆盖的配置行为改变 | `defaultAgentAllowedTools` 不包含示例所需的测试命令 |
| yaml2json 行为分歧 | forge-core 和 Python 在示例 YAML 上产生不同 JSON | 示例使用 `unwrap()` 标量但 Go 解析器处理不同 |

### 为什么需要它

ForgeOS 作为「AI-native 软件工厂」的核心承诺是:**从 Idea 到 Production 的全生命周期自动化**。
示例是这一承诺的具象化证据。如果 forge-core 的演进破坏了示例的构建能力,
这个承诺就失去了可验证性。没有回归保险库,每一次提交都是对「工厂能否造产品」的无声赌博。

### 实现思路

- 在 CI 中为每个示例运行 `forge run build --executor dry`(验证解析+路由+gate 编排完整)
- 增加每周/标签触发的**完整管线端到端重建**(真 agent,带四维资源护栏)
- 示例 YAML 文件纳入 yaml2json 差分测试的 fixture 集
- 为示例建立「从零构建」的 checkpoint:重建所需的最短时间/步骤/agent 调用数

---

## 方向二 · 零依赖约束的维护税

**优先级**: 🟠 P1 | **类别**: 架构 · 工程化 | **预估**: ∼3 sprints | **杠杆**: ⭐⭐⭐

### 问题描述

forge-core 的 `go.mod` 没有 `require` 块——**零外部依赖**,纯 Go 标准库。
这是一个了不起的工程成就,但它带来了一个隐藏的维护税:**任何本应由标准库或
成熟第三方库提供的功能,都必须由 forge-core 自己手写实现并持续维护**。

当前最显著的受税区域:

| 功能 | 手写代码 | 行数 | 成熟替代 | 维护风险 |
|------|---------|------|---------|---------|
| YAML 解析 | `internal/yaml2json/` (9 文件) | ~1565 | `gopkg.in/yaml.v3` | YAML 1.2 spec 持续演进;不支持 anchors/aliases/tags/multi-doc |
| 关键词检索 | `internal/prompt/retrieve.go` | ~130 | `bleve`/TF-IDF lib | 只有词频,无语义;IDF-lite 是近似算法 |
| 信号收敛 | `internal/converge/converge.go` | ~210 | 领域特定,无现成替代 | 可接受(领域逻辑) |
| 架构检查 | `harness/arch/arch-check.mjs` | ~370 + scan 模块 | `golangci-lint` 的部分检查 | 其实依赖 Node.js,不是纯 Go |
| SCA 扫描 | `harness/sca.mjs` | ~371 | `osv-scanner`/`snyk` | 其实依赖 Node.js |

最重的是 **yaml2json 包**:它是唯一完全驻留在 Go 运行时中的手写解析器(其他是 Node/Python 脚本)。
它必须处理:
- 块标量(`>`, `|`, `>-`, `|+` 等 + 缩进指示符)——已出过 bug(Sprint 27)
- 序列项——bare `-` 项空分支已出过 bug(Sprint 30)
- 映射键——冒号前后空白规则
- 行内 JSON/表格/YAML 混合
- 注释处理
- 多文档流
- 锚点/别名/合并键(**明确不支持,无运行时守卫**)

每次 YAML 规范更新或有新 YAML 功能需求,都要在这个手写解析器上追加代码,
而不是简单地升级一个 `go.mod` 版本号。

### 代码级证据

**证据 A: 解析器不支持的 YAML 功能清单(仅在注释中声明)**

```go
// forge-core/internal/yaml2json/yaml2json.go:27-41
// This is a hand-written YAML parser with limited scope:
//   - Scalars: plain, single-quoted, double-quoted, block (literal/folded)
//   - Collections: sequence, mapping (nested)
//   - Does NOT support: anchors (&) / aliases (*) / merge keys (<<:)
//     / tags / multi-document / directives / YAML 1.2 specific features
```

没有运行时守卫检查这些不支持的功能。如果一个工作流文件无意中使用了 `&anchor`,
Go 解析器会返回 error 或产生错误 JSON(取决于具体上下文),然后 fallback 到
Python PyYAML——两条路径产生不同结果,但没有告警。

**证据 B: 解析器已出过真实 bug**

```
Sprint 27: block-scalar indicator ("> " / "| ") 被拼入解码值
           → 每个真实 workflow 文件的 description:/note: 字段被注入前缀
           → 差分测试只 t.Logf 从不 t.Errorf——测试本身失效

Sprint 30: parseSeqItem 空分支不 append nil → bare `-` 序列项被静默丢弃
           → 对本仓 7 个真实 YAML 文件零影响(因为无一命中)
           → 但任何外部项目使用 bare `-` 就会出问题
```

**证据 C: 解析器的测试主要覆盖「本仓自己的 YAML」,非通用 YAML 测试套件**

yaml2json_test.go 的 `TestToJSON_MatchesPythonShim` 使用 `testdata/*.yml` 下的 fixture,
这些 fixture 是根据 forge-core 自己的 workflow 文件创作的。没有针对 YAML 规范
(YAML 1.2 Test Suite 或类似)的全面测试。所以解析器可能在其他合法 YAML 用法上失败。

### 边界情况

| 场景 | 当前行为 | 理想行为 |
|------|---------|---------|
| 用户 workflow 使用 YAML anchor `&defaults` | Go 解析报错 → fallback 到 Python | 明确告警:该 YAML 功能不被 Go 原生解析器支持 |
| 用户 workflow 使用 `!!str tag` | Go 解析可能遗漏 tag → 语义变化 | 同上 |
| 用户 workflow 使用多文档 `---\n...\n---` | Go 解析只读第一份 → 丢失后续 | 明确告警或支持多文档 |
| PyYAML 升级(yaml 6.0)改变某行为 | Python fallback 路径行为变化 | 两者都能解析的 subset 应有测试覆盖 |
| 新的 YAML 1.2 功能(YAML 1.3 设想中) | Go 解析器落后 → 不稳定 fallback | 架构上有清晰策略:升级第三方库或接受 zero-dep 的 subset |

### 为什么需要它

这不是「要不要加一个 YAML 依赖」的简单问题。这是 **zero-dep 作为架构决策的代价
需要被显式管理,而不是默默在每次 bugfix 中支付**。当前状态是:
- 技术上确实零依赖(go.mod 无 require)
- 但运营上依赖 Python PyYAML 作为 fallback
- 且双解析器路径不一致时无检测机制

三个可行的策略方向(需架构决策):
1. **接受 subset**:正式声明 forge-core 的 YAML subset,为不支持的语法加运行时错误检测
2. **解冻 zero-dep**:引入 `gopkg.in/yaml.v3`(标准库兼容许可),砍掉 1565 行手写代码
3. **硬化 fallback**:为双解析器建立持续的差分测试,确保每次构建都在所有已知 YAML 上一致

无论选哪条路,都需要架构级别的讨论和决策——这正是 ADR 机制存在的理由。

---

## 方向三 · 自治运行的监督盲区:人类如何「中途 check in」

**优先级**: 🟠 P1 | **类别**: 可靠性与可操作性 | **预估**: ∼2 sprints | **杠杆**: ⭐⭐⭐⭐

### 问题描述

ForgeOS 的核心场景是 24h 无人值守自治运行(`forge evolve`)。但一旦 evolve 循环启动,
人类监督者面临一个「单向玻璃」困境:

```
发起 evolve → [ 24h 自治运行 ] → 收敛/超时/失败 → 通知
                  ↑
           人类只能看终端日志滚动
           无法: 暂停 · 检查状态 · 调整方向 · 部分确认
```

现有的人机交互节点:

| 交互点 | 时机 | 能力 |
|--------|------|------|
| `--model` / `--mode` / `--lifecycle` | 启动前 | 设定运行参数 |
| `human_gate` (design.yml) | 阶段间 | 批准/拒绝设计(但 evolve 拒绝 human_gate workflow) |
| 终端 stdout | 运行中 | 只读日志 |
| SIGINT/SIGTERM | 运行中 | 粗暴终止(无 checkpoint) |
| `forge status` | 运行后 | 查看最终状态 |

运行中没有任何方式可以:
- 暂停 evolve 循环,检查当前状态,然后继续
- 对当前迭代的 agent 输出进行部分确认
- 调整剩余迭代的参数(如调低 max-iter、更换 model tier)
- 注入人类指导("这个方向不对,下一轮聚焦测试质量而不是新功能")
- 在收敛前收到渐进式通知("已完成 5 轮,roadmap 40%,继续")

### 代码级证据

**证据 A: LoopEngine 的信号通道是纯采集的,没有注入点**

```go
// internal/orchestrator/loop.go
type LoopEngine struct {
    Stop       asset.StopCondition  // 只读停止条件
    Signals    func() converge.Signals  // 纯采集:在每次迭代后测量信号
    OnIteration func(i int, sig converge.Signals, durationMs int64)  // 只写钩子
    // ❌ 没有任何 "OnHumanCheckin" 或 "InjectGuidance" 接口
    // ❌ 没有任何运行时暂停/恢复/调整的机制
}
```

LoopEngine 有三个回调(OnIteration, OnBeforeIteration, OnPhase),但都是**单向的**:
它们接受数据,不返回任何可以影响循环行为的指令。

**证据 B: `forge status` 是静态文件快照,不是运行时状态**

```go
// cmd/forge/validate.go
type statusJSON struct {
    Mode       string  `json:"mode"`
    Lifecycle  string  `json:"lifecycle"`
    Checkpoint *checkpointSummary `json:"checkpoint,omitempty"`
    // ❌ 没有运行中的进程信息
    // ❌ 没有"当前迭代"、"当前相位"、"已用时间"、"已耗预算"
    // ❌ 没有"是否有一个 evolve 循环在运行"
}
```

`forge status` 读取的是 `checkpoint.json`(最新持久化的快照)和 `.forge/` 目录标记,
不是正在运行的 evolve 进程的实时状态。

**证据 C: evolve 循环没有可配置的停顿点**

```go
// cmd/forge/evolve.go
func execLoop(...) {
    for iter := loop.StartIter; iter <= maxIter; iter++ {
        // 运行一轮 → 测量 → 判断收敛 → 继续
        // 没有任何等待外部输入的暂停点
        // 没有任何"等待用户确认后继续"的钩子
    }
}
```

对比 `human_gate()` 的实现:

```go
// internal/converge/converge.go
func humanGate(...) converge.Signals {
    // 检查 .forge/<stage>.approved 标记
    // ✅ 这是存在的,但它只在阶段间生效
    // ❌ 不存在运行中 evolve 循环的类似机制
}
```

`human_gate` 可以在阶段间暂停等待人类批准,但这个开关没有暴露给运行中的 evolve 循环。
`forge evolve` 在 Sprint 25 就拒绝了 human_gate workflow(`rejectHumanGate`),理由是
「循环中引入等待点会破坏无人值守语义」——但这恰恰是问题所在:无人值守不意味着「永远
不许人看」。

**证据 D: 没有通知机制**

```go
// 全仓搜索 "notify\|webhook\|alert\|callback.*url\|email\|slack"
// 零命中
```

即使 evolve 循环已收敛或已失败,运行者也无法在循环完成前收到通知。

### 边界情况

| 场景 | 当前行为 | 风险 |
|------|---------|------|
| evolve 在第 3 轮产生了一个危险的架构决策 | 继续到第 4 轮,不知道 | 浪费预算在错误方向上 |
| 用户通过 terminal 观察到方向错误,想纠正 | 只能 Ctrl-C → 丢失 checkpoint | 丢失已完成工作的进度 |
| 用户想「先看看第 2 轮的产出再决定是否继续」 | 不可能——一旦开始就无法暂停 | 要么全自动要么全手动 |
| 长时间 evolve(≥50 轮)后用户想检查进度 | 只能等终端变回提示符 | 无法渐进式监控 |
| 团队有多名成员,不同时间检查运行状态 | 无法异地查看 | 单点运行,单点失败 |

### 为什么需要它

ForgeOS 的愿景是「24h 无人值守自治运行」,但实际无人值守系统在任何行业都有一个
基本要求:**可监督性**(Supervisability)。人类不需要在每一步确认,但需要在关键节点
能「看一眼」而不必杀死进程。

这不是要引入「每一步都需要人确认」的繁重工作流——那是 `human_gate` 已经解决的问题。
这里需要的是**轻量级的、不阻塞运行的中途检查能力**:类似电梯里的「紧急通话」按钮,
它不影响电梯的自动运行,但让乘客知道自己不是完全被困。

### 实现思路

- `forge status --live`:连接运行中的 evolve 进程(通过本地 socket 或信号),返回实时状态
- `forge evolve --webhook <url>`:在每次迭代后推送状态变化到外部系统
- `forge evolve --pause-after <N>`:在第 N 轮迭代后暂停,等待用户 `forge continue` 或 `forge abort`
- `forge guidance "..." --attach <pid>`:向运行中的 evolve 注入人类指导(作为额外 context 注入下一轮 prompt)

---

## 汇总

| 方向 | 优先级 | 类别 | 预估 | 杠杆 | 已有覆盖 |
|------|--------|------|------|------|---------|
| ① 跨示例管线回归保险库 | 🔴 P0 | 治理 · 质量保证 | 1.5 sprints | ⭐⭐⭐⭐⭐ | **零覆盖**(确认) |
| ② 零依赖约束的维护税 | 🟠 P1 | 架构 · 工程化 | 3 sprints | ⭐⭐⭐ | 部分覆盖(YAML shim)、但未作为「零依赖的架构代价」量化 |
| ③ 自治运行的监督盲区 | 🟠 P1 | 可靠性与可操作性 | 2 sprints | ⭐⭐⭐⭐ | 部分覆盖(evolve 可观测性),但未聚焦「运行中人类交互」 |
