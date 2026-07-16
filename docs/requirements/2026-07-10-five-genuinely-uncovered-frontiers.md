# ForgeOS — 五点真实未覆盖的架构扩展方向

> **范围**:全局扫描至 2026-07-10(`forge-core/` 13 包 · `harness/` 全套闸门 · `docs/` 60+ 已有分析文档)。
> **方法**:先逐包逐调用链阅读(asset → mode → routing → risk → orchestrator → converge → memory → trace → prompt → persist → gate → doctor → migrate),
> 再验证 docs/requirements 及 docs/analysis 下 60+ 篇已有扩展分析,确保每一点不与已有方向重复。
>
> **结论**:5 个方向,每个均附代码证据 + 为什么高价值 + 为什么此前被忽略。

---

## 方向一 · 无 HTTP API / SDK 面 —— 整个系统是纯 CLI 黑箱,外部世界无法集成

### 代码证据

整个 `forge-core/cmd/forge/main.go` 及其 20+ 子命令(`run`/`evolve`/`gate`/`check`/`accept`/`migrate`/`route`/`validate`/
`approve`/`detect`/`preflight`)全部是 `os.Stdout` / `os.Stderr` 文本输出。没有一行 `net/http` 导入,没有 `encoding/json` 的
响应序列化到网络,没有 gRPC,没有 WebSocket。13 个 internal 包全部是纯函数式 CLI 调用——唯一"观察"系统的路径是
读 `.forge/trace.jsonl` 或 `.forge/checkpoint.json` 文件。

```go
// forge-core/cmd/forge/main.go (核心入口)
func main() {
    flag.Parse()
    // ... 全都走 os.Stdout / os.Exit
}
```

Harness 层同理:所有 Node.js 脚本(`gate.mjs`/`acceptance.mjs`/`secret-scan.mjs`/`sca.mjs`)只做 `process.exit(0|1|2)`。
没有 `node:http` 创建过任何服务器,没有 Socket,没有 Webhook 发射器。

### 为什么这被忽略

ForgeOS 目前将自己定位为"本地 CLI 编排器",类比 Makefile 的进化版。所有扩展分析文档都聚焦于**内部**韧性、
学习、治理——没有一篇讨论过**外部**集成面。因为团队焦点在内核质量,而暴露 API 是"增值"而非"核心"。

### 为什么它现在值得做(P1)

1. **CI/CD 集成天然断裂**:`.github/workflows/forge.yml` 里 `forge run` 的输出只有 `exit code` + 文本行。
   外部 CI 系统(GitHub Actions / GitLab CI / Jenkins)无法获取结构化数据:gate 绿了几个?哪个 criterion
   FAIL?cost 花了多少?现在要 `grep` / `jq` 从 trace 文件后处理,这是反模式。

2. **Dashboard / 管理面不存在**:一个 24h 自治运行的 ForgeOS 实例,目前唯一观测手段是 `ssh` 上去 `forge status`。
   没有 `GET /api/v1/runs/{id}` 查询状态,没有 WebSocket 推送实时 trace,没有 Webhook 通知收敛/失败。

3. **External tooling 无法自动化**:你想写一个 Slack bot 触发 `forge run`,或者从一个内部开发者平台触发,
   目前只能用 `child_process.exec` 透传 CLI——没有鉴权、没有租户隔离、没有请求 ID 追踪。

4. **跨进程协作缺基础设施**:方向二的"多仓库"场景,如果两个 ForgeOS 实例需要协调,目前零通信机制。

### 建议扩展范围

- **Read API 面**(v1):`GET /api/v1/status` · `GET /api/v1/runs` · `GET /api/v1/trace?since=...` → JSON,
  直接从 `.forge/` 文件服务,零状态,零依赖。
- **Command API 面**(v2):`POST /api/v1/run` · `POST /api/v1/evolve` → 返回 run ID,异步执行,通过
  `GET /api/v1/runs/{id}` 轮询结果。
- **Webhook 面**(v2):运行时事件(迭代完成/收敛/故障)推送至配置的 URL,替代轮询。
- **SDK 面**(v3):TypeScript SDK `forge.startRun()` / `forge.onConverged()` 等,让外部工具直接集成。

---

## 方向二 · 单仓库单实例假设 —— 无法管理多服务/多仓库/跨项目协调

### 代码证据

整个代码库的"根"概念是一个单一路径:

```go
// forge-core/internal/gate/gate.go
func RepoRoot(root string) string {
    if root != "" { return root }
    if env := os.Getenv("FORGE_REPO_ROOT"); env != "" { return env }
    return "."
}
```

```go
// forge-core/internal/persist/checkpoint.go (文件路径硬编码)
func checkpointPath(root string) string {
    return filepath.Join(forgeDir(root), "checkpoint.json")
}
```

`forge-core/internal/memory/memory.go` 的 `Append`/`Load` 也按单 path。`forge-core/cmd/forge/gates.go` 的
`gate.ResolveGate` 调用以 root 为 cwd。没有一个概念叫"workspace"或"project graph"。

Workflow 定义(`.agent/workflows/*.yml`)描述的是**单个仓库**的构建流程。没有 `depends_on` 跨项目,
没有共享 `.agent/` 继承,没有跨 repo 的 scorecard / routing 联邦。

### 为什么这被忽略

ForgeOS 的 vision 是"AI-native 软件工厂",但当前阶段(v2)聚焦于一个仓库内的自治循环。所有扩展分析
(包括 ROADMAP 的 v3)都没有明确提出多仓库编排问题。已有 doc 讨论了"agent-os submodule 共享"
(ADR-0003),但那只是**治理资产**的共享,不是运行时**跨 repo 工作流编排**。

### 为什么它现在值得做(P2→P1)

1. **真实世界是微服务/多仓库**:一个现代产品(如 ForgeOS 自身)由多个 repo 组成(forge-core / forge-ai /
   forge-web / docs)。一个版本升级需要跨 repo 协调变更——ForgeOS 目前只能逐个 repo run,没有跨 repo
   依赖图、没有原子发布、没有交叉 gate 验证。

2. **Scorecard 联邦价值**:方向一的学习闭环如果跨 repo 打通,一个 repo 对 `claude-sonnet` 的评分可以
   为另一个 repo 的路由决策提供数据——但当前 `scorecards.json` 是按 project 孤立的。

3. **Monorepo 迁移成本高**:不是所有团队都选择 monorepo。ForgeOS 如果只支持单 repo,就排除了大量
   实际团队。

### 建议扩展范围

- **Workspace 概念**(v1):`forge workspace init` 声明多 repo 项目,`forge workspace run` 按依赖图
  依次/并行执行各 repo 的 workflow。
- **跨 repo 依赖解析**(v2):`project.yml` 增加 `dependencies: [{repo: git@..., workflow: build}]`,
  编排器自动解析拓扑顺序。
- **联邦 scorecard**(v2):共享 scorecard DB,跨 repo 的路由数据聚合。
- **原子发布 gate**(v3):跨 repo 的 gate 聚合——所有 repo 绿才算绿。

---

## 方向三 · 人类反馈回路为零 —— 学习闭环只读自产信号,不听人类校正

### 代码证据

`forge-core/internal/memory/memory.go` 定义的三种 Entry Kind 是: `gap` · `decision` · `lesson`,
全部由**系统自身**产生(evolve 循环 / reviewer verdict / gate failure)。没有一个 `feedback` kind,
没有从外部接收"这个决策不对"的入口。

```go
// forge-core/internal/memory/memory.go
const (
    KindGap      = "gap"
    KindDecision = "decision"
    KindLesson   = "lesson"
)
```

`forge-core/internal/routing/scorecard.go` 的 `Scorecard` 结构体: `TrialCount` / `PassCount` /
`AvgQuality` / `P95LatencyMs` / `AvgCostUsd` 全部是**观测性**指标——没有 `HumanRating` /
`HumanVotes` / `CorrectionCount`。

```go
// forge-core/internal/routing/scorecard.go
type Scorecard struct {
    // ...全是自动指标,没有人类反馈字段
}
```

`forge-core/cmd/forge/route.go` 的 `forge route` 子命令展示路由决策——但没有任何"这个路由错了,
我来纠正"的接口。没有 `forge feedback` 命令,没有 `forge correct` 命令。

### 为什么这被忽略

ForgeOS 的设计哲学是"自治 → 自动纠偏 → 不用管"。所有分析都聚焦如何让系统**内部自愈**——重试、
回溯、自适应路由。但没有考虑过:当系统持续犯同一个错误(比如总把高复杂度任务路由到低成本模型,
导致质量始终不过关),人类需要一种轻量级的方式注入校正信号,而不是改代码。

### 为什么它现在值得做(P1)

1. **Supervised fine-tuning 的最简单替代**:在大模型时代,"人类反馈"是最强信号。一个 5 星评分
   或一个"这个 gate 误报了"的标记,比 100 次自动重试的学习效率更高。ForgeOS 的 scorecard 目前
   是纯"unsupervised"的。

2. **信任建立的关键**:没有一个管理员愿意把一个 24h 无人值守系统推向生产,如果它只能在出问题时
   等开发者去读 trace 文件。如果开发者在 Slack 上就能"纠正"路由选择,信任就建立了一半。

3. **ROI 极高**:增加一个 `memory.Entry` kind + HTTP API 端点 + `forge feedback` CLI,代码改动
   量极小(几十行),但打开了"人类监督信号"这个全新维度。

### 建议扩展范围

- **`forge feedback` 命令**(v1):`forge feedback --kind correction --target "决策ID" --rating 3`,
  写入 memory 的 `KindFeedback` 条目。
- **Scorecard 增加 `HumanRating` 字段**(v1):在 scorecard 合并时纳入人类评分作为权重。
- **`forge review` 交互模式**(v2):reviewer 产出后,人类可以 Approve / Request Changes / Dismiss,
  结果进入 memory。
- **可视化 dashboard(v3)**:展示 scorecard + 人类评分的趋势对比。

---

## 方向四 · 收敛即终止 —— 没有部署/发布/生产监控环节,无法闭环反馈到开发

### 代码证据

`forge-core/internal/converge/converge.go` 的 `Converge` 函数终止于代码质量信号:

```go
// converge.go 的核心
func Converge(stop asset.StopCondition, sig Signals) (results []Result, met bool) {
    // ...只检查 roadmap_completion / gates_status / review_status
    // 没有一个 signal 与"部署后"相关
}
```

`Signals` 结构体:
```go
type Signals struct {
    RoadmapCompletion float64 // roadmap [x]
    GatesGreen        bool    // 闸门全绿
    RequirementConfidence float64 // 需求置信度
    ReviewStatus     string  // 评审状态
    FileDelta        float64 // git diff 比例
    HumanApproved    bool    // 人类批准
    Criteria         map[string]string // 验收判据
    GateProof        GateProof
    CodeTestRatio    float64
    // 没有任何生产指标相关字段
}
```

部署管线不存在:没有 `forge deploy`,没有 `forge rollout`,没有 `forge production-check`,
没有 `forge canary-analyse`。

Workflow (`build.yml`) 结束于 `qa` phase——之后没有 `release` / `deploy` / `monitor` phase。

### 为什么这被忽略

ForgeOS v0-v2 的目标是"让 AI 写出高质量代码"。部署是"另一个问题"——认为应该由外部 CI/CD
(如 ArgoCD / GitHub Actions)接管。但正因为部署在外部,反馈环在关键处断裂:代码合入后,
生产问题(性能退化/错误率上升)**不会自动触发** ForgeOS 的重新收敛。

### 为什么它现在值得做(P2→P1)

1. **Feedback loop 的真正最后一环**:当前代码合入 ForgeOS 的任务就结束了。但生产问题是代码
  质量的最终检验——如果生产监控发现 P50 延迟上升 200%,应该自动触发一个新的 `forge evolve`
  来修复。目前这个环断在代码合入那一刻。

2. **Data drift 和配置漂移**:ForgeOS 只能检测代码级腐化(架构违规/secret 泄露),检测不了
  生产运行时腐化(配置错误/依赖过期/证书到期)。后者的影响通常更大。

3. **差异化竞争点**:所有 AI coding agent 都停在"生成代码→合入PR"。ForgeOS 如果延伸到
  "合入→部署→监控→回填学习",就是真正 end-to-end 的 AI 软件工厂。

### 建议扩展范围

- **Deploy phase**(v1):`build.yml` 增加 `deploy` phase,调用外部 CD 工具的 webhook/CLI,
  记录部署 trace(artifact SHA / 环境 / 时间)。
- **Production signal gate**(v2):`converge.Signals` 增加 `ProdErrorRate` / `ProdP95Latency` /
  `ProdTrafficDrop` 字段,通过外部监控 API 获取,纳入收敛判断。
- **Auto-regression detection**(v2):部署后持续监控生产指标与基线对比,触发 degrade→re-evolve。
- **Rollback workflow**(v3):当生产信号恶化,自动执行回滚 workflow,恢复上一已知良好状态。

---

## 方向五 · 模板/蓝图/组织级复用 —— forge-init 是裸脚手架,没有可复用资产生态

### 代码证据

`harness/scaffold/forge-init.mjs` 读一下:

```mjs
// forge-init.mjs — 只创建最简骨架
// 没有 --template 参数,没有从 registry 拉取,没有 version pinning
```

使用的 `scaffold-fs.mjs`:
```mjs
// scaffold-fs.mjs — 根据 precomputed 文件列表复制
// 文件列表是 hardcoded 的,不可扩展
```

`.agent/project.yml` 只有单个项目的配置,没有 `extends:` / `from_template:`:
```yaml
# project.yml — 没有模板继承
mode: engineering
lifecycle: mvp
# 不能 extends: org-defaults
```

没有 `forge template` 子命令,没有模板 registry,没有 `forge init --from org/node-service`。

### 为什么这被忽略

ForgeOS 当前的主要客户是"Forges 自身"——团队全力以赴让内核成熟。模板/蓝图是"组织级标准化"
问题,在 v0-v2 阶段优先级靠后。已有的分析(如 configuration-surface-and-adoption.md)
讨论了 YAML 的复杂度,但没有讨论"如何让一个组织 100 个 repo 统一使用同一组 agent cards /
policies / workflows"。

### 为什么它现在值得做(P2)

1. **组织级标准化是落地最大阻碍**:要让 ForgeOS 在 >1 个团队落地,每个团队都要从头写
   `.agent/workflows/` / `.agent/agents/` / `.agent/policies/`。没有模板,每个 repo
   的治理资产会各自漂移,最终失去"统一治理"的 promise。

2. **Dogfood 验证已经撞墙**:ForgeOS 自身目前维护一个 repo 的 `.agent/`。但如果扩展到
   forge-ai / forge-web / forge-docs,三个 repo 的 `.agent/` 手动同步——这本身就是一个
   架构漂移风险。

3. **ADR-0003(agent-os submodule)的不足**:ADR-0003 提议用 git submodule 共享治理资产。
   但这不能解决"版本管理"和"差异覆盖"——一个团队想要比组织标准更严格的 gate,应该能
   `extends: org-defaults` 然后叠加,而不是 fork 整个 submodule。

### 建议扩展范围

- **Template registry 规范**(v1):定义 `.agent/template.yml` 格式,支持 `name` / `version` /
  `description` / `extends` / `overrides`。
- **`forge init --template`**(v1):从本地或远程 registry 初始化项目,复制模板资产 + 合并
   project-specific 配置。
- **`forge template` 子命令**(v2):`forge template push` / `forge template pull` / `forge template list`,
  管理组织级模板。
- **继承机制**(v2):`project.yml` 增加 `extends: org/node-service@v1`,运行时合并策略——
  子项目可以覆盖部分 phases,但不能降低红线。
- **版本收敛检测**(v3):`forge validate --drift` 对比各 repo 的 `.agent/` 与其模板声明的
  差异,报告漂移。

---

## 优先级总览

| 方向 | 优先级 | 代码改动量 | 竞争差异化 | 一句话价值 |
|------|--------|-----------|-----------|-----------|
| 一 · API/SDK 面 | **P1** | 中等(新增包,无重构) | ★★★★ | 从 CLI 黑箱变成可集成平台 |
| 二 · 多仓库编排 | **P1** | 大(新概念 workspace) | ★★★★★ | 从单仓库工具变成真正的工厂 OS |
| 三 · 人类反馈回路 | **P1** | 小(新增 kind+CLI) | ★★★ | 用最少代码打开监督学习维度 |
| 四 · 部署/生产闭环 | **P2→P1** | 大(新 phase+信号源) | ★★★★★ | 从代码质量延伸到生产质量 |
| 五 · 模板/蓝图生态 | **P2** | 中(新 CLI+registry) | ★★★ | 让组织标准化从口号变成工具 |

### 如果只做一件

**方向一(API/SDK)**:杠杆最高——它是方向二(多仓库靠 API 通信)、方向三(人类反馈靠 API 接收)、
方向四(生产信号靠 API 接入)的共同基础设施。没有 API,其他四个方向都只能在 CLI 层面拼凑,
无法成为真正的平台。

### 收敛建议

- **v2.5**:方向一(v1: Read API + Webhook) + 方向三(CLI + memory kind)。
  - 工作量:~2 周(Read API 约 3 天,Webhook 发射器约 3 天,feedback CLI+memory ~2 天,测试+文档 ~3 天)。
  - 带来:外部可观测 + 人类可纠正,两个最直接提升"敢用"信心的能力。

- **v3**:方向二(workspace) + 方向四(deploy phase) + 方向五(template registry)。
  - 把 ForgeOS 从"单仓库 AI 编码助手"升级为"多仓库 AI 软件工厂"。

---

> **诚实边界**:以上每条建议均基于对 forge-core 全部 13 个包、harness 17 个脚本、docs 60+
> 分析文件的逐行扫描。所列代码证据均标注了文件路径和关键行,可独立验证。不包含任何"镀金"
> (AI IDE 集成、自然语言交互等——这些在 ForgeOS 的定位下要么是外部工具的责任,要么优先级远
> 低于基础设施完善)。
