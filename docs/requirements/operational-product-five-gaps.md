# ForgeOS: 运营与产品化五维缺口

> **角色**: 资深架构师 / 产品经理  
> **方法**: 全局代码库扫描（forge-core 18 Go 包 · harness 39 模块 · `.agent/` 完整骨架 ·  
>   `.agent/CURRENT_SPRINT.md` 31 个 Sprint 演进记录 · 40 篇 `docs/analysis/` · 67 篇 `docs/requirements/`  
>   逐一交叉验证，确保每个方向在已有文档中**无独立设计覆盖**）。  
> **约束**: 不写代码；每个方向附代码级证据 + 交叉引用已有分析，避免重复。  
> **日期**: 2026-07-10

---

## 为什么是这五个方向

已有 107 篇分析文档覆盖了：引擎就绪（Sprint 5–15）、边缘可靠性（Sprint 16–31）、
多项目生态（第三地平线）、边界情况/性能瓶颈、配置表面积、MQTT/WASM 集成、自我测试质量、
增长瓶颈、数据飞轮、安全/合规架构……

但所有这些分析都假设**系统已经安装好、用户已经知道它、组织已经在用它**。
以下五个方向聚焦于从「技术项目」到「产品/平台」的跃迁中**还没有被充分论证**的运营化缺口。

---

## 方向一：Binary Lifecycle Platform —— 安装/更新/回滚/验证

### 代码证据

```
forge-core/go.mod:      module forgeos/forge-core (go 1.26, 零外部依赖)
forge-core/cmd/forge/main.go:14: var forgeVersion = "dev"     # 默认值「dev」
forge-core/cmd/forge/main.go:17: var forgeCommit = ""         # 默认为空
```

根 `README.md` 和 `BOOTSTRAP.md` 均未提及如何安装 forge 二进制。
`harness/scaffold/forge-upgrade.mjs` 只升级 **harness 文件**（复制 `.arch/rules.yaml`、
`harness/gate.mjs` 等），不升级 forge 二进制本身。
没有 `go install` 指南、没有安装脚本、没有 Homebrew tap / APT repo / GitHub Release 工作流、
没有 checksum 校验、没有签名、没有 rollback 机制。

### 为什么需要

| 维度 | 理由 |
|------|------|
| **采纳门槛** | 一个需要用户「`go build` 然后祈祷」的系统，不可能获得企业采用。ForgeOS 要治理别人的工程，自己却连安装程序都没有。 |
| **版本管理** | `forgeVersion = "dev"` 的默认值意味着开发环境与生产环境的二进制无法区分。没有 `forge version --check-update`，没有 `forge upgrade --dry-run`，没有「当前版本 N，可用版本 N+1，变更日志。」 |
| **供应链安全** | 一个对整个组织的工程产出施加治理的系统——其自身的二进制如果被篡改，整个信任模型崩塌。无签名、无校验、无 SLSA  provenance，这是安全架构中的**皇帝的新衣**。 |
| **回滚能力** | `forge evolve` 已经能对项目做 checkpoint/resume，但对 forge 自身的版本升级没有回滚能力。一次有 bug 的 forge 升级可能阻塞整个 CI 流水线。 |
| **升级策略** | 没有 canary 升级、没有 staged rollout、没有 upgrade compatibility check。用户被强制「全量更新或永不更新」。 |

### 邻近覆盖对照

| 已有文档 | 覆盖了什么 | 本方向新增 |
|----------|-----------|-----------|
| `genuine-uncovered-five-binary-state-output-session-datalifecycle.md` | 二进制「状态/输出/会话」的**运行时语义** | 二进制**生命周期管理**（构建/分发/签名/安装/升级/回滚） |
| `forgotten-frontiers-five.md` | in-toto attestation（对 forge**产出**的签名） | forge**自身二进制**的签名与分发 |
| `expansion-horizon-three.md` 方向四 | 治理资产（YAML/规则）的升级管理 | 编译型 Go 二进制的升级管理——完全不同的机制（二进制替换、SIGUSR2 热升级、回滚标记） |
| `five-systemic-oversights-v45.md` | `forge test install` 概念 | 从「测试安装是否正确」到「完整的安装/更新/分发平台」 |

### 关键设计要素

```
┌──────────────┐     ┌──────────────┐     ┌──────────────┐
│  Build &     │ ──→ │  Release     │ ──→ │  Distribution│
│  Sign (CI)   │     │  Manifest    │     │  (tap/repo)  │
└──────────────┘     └──────────────┘     └──────────────┘
                                                    │
┌──────────────┐     ┌──────────────┐              ▼
│  Rollback    │ ←── │  forge       │ ←── ┌──────────────┐
│  (atomic)    │     │  upgrade     │     │  Verify      │
└──────────────┘     └──────────────┘     │  Checksum    │
                                          └──────────────┘
```

- **Build & Sign**: GitHub Actions release workflow → `go build` 多平台(Linux/macOS/Windows × amd64/arm64)→ `cosign` 签名 + SPDX SBOM + `sha256sum.txt`
- **Release Manifest**: 机读 manifest（SemVer + checksums + sig + changelog + upgrade_compat: [from_version]），forge 本地 cache 副本供离线验证
- **`forge upgrade`** 子命令：`check`(对照 manifest)、`fetch`(下载+校验)、`apply`(原子替换+保留旧二进制)、`rollback`(恢复旧版本)
- **安全底线**: `forge upgrade --verify-only` 作为 CI step 前置检查；默认连接 `https://` 下载，`--insecure` 显式 opt-in；升级前运行 `forge doctor` 确保当前环境健康

---

## 方向二：Watch Mode / Long-Running Daemon

### 代码证据

```
# 当前所有入口均是一次性 CLI 调用：
forge-core/cmd/forge/main.go:69-76: subcommands = map[string]func([]string)int{
    "run": cmdRun, "evolve": cmdEvolve, "gate": ..., "route": ...
    # 没有 "daemon" / "serve" / "watch" 子命令
}

# 架构文档明确只规划了 CLI 入口：
.agent/ARCHITECTURE.md:46: "CLI forge run/evolve/gate/check/accept/migrate/route"
.agent/architecture/north-star.md:97: "Web UI(Next)" ≠ daemon 模式

# ContextCache 生命周期绑定到单次 run：
internal/prompt/cache.go: 创建于 cmdRun/cmdEvolve 始，销毁于终
# 每次 run 重建缓存，从不跨会话共享
```

没有任何文件 watcher（`fsnotify`）、HTTP/Unix socket listener、定时器（cron）、信号驱动的事件循环。
系统完全被动的——用户必须手动调用 `forge run` 或 `forge evolve`。

### 为什么需要

| 维度 | 理由 |
|------|------|
| **开发体验** | 当前工作流：改文件 → `forge gate`（手动）→ 发现问题 → 改文件 → `forge gate`。应该：改文件 → 自动触发 gate → 终端通知「复杂度超标」 |
| **24h 自治的前提** | `forge evolve` 是一个长时间运行的进程。如果它在 terminal 被关闭、SSH 断连、CI job 超时后被 kill，循环终止。daemon 模式 + `nohup`/`systemd`/`launchd` 集成 = 真正的 24h 自治。 |
| **预热加速** | `forge run` 每次从零加载 `.agent/` 全部文件、解析 YAML、构建 prompt cache。一个常驻 daemon 持有预热状态，将重复 run 的延迟从秒级降到毫秒级。 |
| **事件驱动触发** | 没有 daemon，就没有 git push webhook → auto-evolve、没有 `cron(触发每周一凌晨的 evolve)`、没有 file-change → re-gate。自治系统被人工触发束缚。 |
| **健康可见性** | `forge status --watch` 目前是 polling trace.jsonl。daemon 模式可以提供 WebSocket/Unix socket 实时流。 |

### 邻近覆盖对照

| 已有文档 | 覆盖了什么 | 本方向新增 |
|----------|-----------|-----------|
| `extension-frontier-five.md` | 提及 daemon 作为「跨会话缓存」的载体，~5 句，未展开 | daemon 进程模型的完整设计：启动/停止/健康检查/SIGUSR2 重载/systemd 集成/优雅关闭 |
| `four-truly-unexplored-architectural-gaps.md` | 指出「零信号监听器」的 gap | 设计这个 listener 本身——不是指出问题，而是给出架构 |
| `expansion-horizon-three.md` 方向三 | 外部事件驱动的**触发语义**（webhook → 启动 workflow） | daemon 作为这些事件触发的**常驻宿主进程**——无 daemon 则无事件驱动 |

### 关键设计要素

```
forge daemon start          # 启动后台进程
forge daemon status         # 健康/uptime/缓存统计
forge daemon stop           # 优雅关闭（完成当前 gate 后终止）
forge daemon reload         # SIGHUP → 重加载 .agent/ 配置（不中断进行中的 run）
```

- **进程模型**: 双进程模式（父进程监听信号 + 子进程执行工作负载），避免 `exec.CommandContext` 的进程组 kill 波及 daemon 自身
- **缓存预热**: 启动时加载 `.agent/` 全部 YAML、workflow、agent cards、history scorecard；`inotify`/`fsnotify` 监听变更自动增量更新
- **Unix socket**: `/var/run/forge.sock` 提供状态查询、日志 tail、暂停/恢复 evolve 循环的控制接口
- **安全隔离**: socket 文件权限 0700，禁止未授权进程操控 forge daemon
- **与 systemd/launchd 集成**: 安装时可选 `forge daemon install` 注册为系统服务，含自动重启策略

---

## 方向三：Governance Provenance & Compliance Audit Trail

### 代码证据

```
# trace 是事后审计，但无防篡改结构：
forge-core/internal/trace/trace.go: Event 结构是普通 JSONL
  # 无签名、无 hash chain、无完整性校验
  # 文件权限 0644，任何进程可追加或篡改

# converge 信号纯内存，不写入带认证的记录：
forge-core/internal/converge/converge.go: Signals
  # 只在内存中 Evaluate，不持久化到可审计的 ledger

# check.py 检查治理完整性但自身产出不签名：
harness/check.py: run_checks() → print()
  # stdout 文本，无机器可验证的 attestation

# 当前「audit trail」= trace.jsonl + checkpoint.json
# 均无来源认证、无时间戳签名、无链式完整性
```

ForgeOS 对**其他项目**施加严格的治理执法，但自身执法行为的可审计性为零。
没有一个 regulator/auditor 可以独立验证"forge gate 在时间 T 真实运行并产生了结果 R"。

### 为什么需要

| 维度 | 理由 |
|------|------|
| **治理系统的可信度** | ForgeOS 的独特定位是「治理控制平面」。如果一个治理平台不能证明自己的执法行为是可审计、不可篡改的，它的治理主张就缺乏根基。这是 Eat Your Own Dogfood 的最深层次。 |
| **合规场景** | 金融/医疗/政务软件需要证明「每个 production 部署都经过了 security gate 且结果未被篡改」。目前 trace.jsonl 可以被随意编辑。 |
| **事后归因** | 当 evolve 循环产生了有问题的代码，需要追溯「是哪个 gate 漏了什么?是 gate 没跑还是 gate 跑了但结果被改动?」——链式哈希让篡改可发现。 |
| **治理变更审计** | `modes.yml` / `.arch/rules.yaml` / `AGENTS.md` 这些治理关键文件的变更没有结构化记录。谁在何时改了什么阈值——目前依赖 git log，但 git 可以被 amend/force-push。 |

### 邻近覆盖对照

| 已有文档 | 覆盖了什么 | 本方向新增 |
|----------|-----------|-----------|
| `forgotten-frontiers-five.md` | in-toto attestation 对**构建产物**的签名 | 对**治理执法行为本身**的签名——gate 结果、convergence 裁决、approval 决策的不可篡改记录 |
| `genuinely-uncovered-five-frontiers.md` | 指出「hash chain」的概念性 gap（647 行） | 完整的审计链设计：签名轮次、信任锚、验证流程、合规导出格式 |
| `production-product-gaps-v43.md` | 生产就绪的 observability | 从「观测」到「证明」的跃迁——不仅是可见，还要可验证、可审计 |
| `strategic-production-gaps.md` | 紧急 bypass 的 audit trail 概念 | 将 audit trail 从「紧急 bypass 的附属品」提升为「一等公民基础设施」 |

### 关键设计要素

```json
// 每条 trace event 扩展为带签名的证明单元：
{
  "_format": "forgeos.audit.v1",
  "seq": 1247,
  "kind": "gate",
  "name": "security",
  "verdict": "PASS",
  "timestamp_unix": 1720543200,
  "checksum_prev": "sha256:e3b0c44298fc1c14...",
  "signing_key_id": "key-2026-07",
  "signature": "MEQCIGP2HlGq...",
  "enforcement_policy_snapshot": "sha256:policy-version-43"
}
```

- **Hash chain**: 每条事件包含前一条的 SHA-256 —— 篡改中间任意一条会破坏链条完整性
- **Ed25519 签名**: 事件由 forge 进程持有的临时密钥签名；密钥由 forge daemon 启动时生成，公钥输出到 `forge auditor --pubkey`  
- **Policy snapshot**: 每条 gate 事件记录触发它的 `modes.yml` / `.arch/rules.yaml` 的哈希值——审计时可以复现「当时的规则」
- **`forge auditor`** 子命令: `verify`(验证 audit trail 完整性)、`export`(导出为 regulator-friendly 格式)、`compliance-report`(按时间范围/项目/rule 聚合审计摘要)
- **信任锚**: forge 二进制本身内置根公钥（与二进制签名同一把），形成从「已安装的 forge → 执行的 gate → 记录的 event」的信任链闭环

---

## 方向四：Multi-Run Quality Trend Analysis & Regression Alerting

### 代码证据

```
# scorecard 只写不读（持续写入但从不分析趋势）：
forge-core/internal/routing/scorecard.go: HistoryTiebreak
  # 只用于「给定候选模型，择优」，不做趋势分析
  # readScorecards 读取后仅排序，不计算变化率

forge-core/cmd/forge/scorecard_wind.go: 写 scorecards.json
  # 不检查 quality_score 是否下降
  # 不发出任何跨次运行的告警

harness/scorecard.mjs: decayWeight
  # 衰减旧分权重(time-decay)，但无趋势检测
  # 一条持续 N 周的 quality_score 下降——无声

# trace 是 append-only 流，无聚合查询层：
forge-core/internal/trace/trace.go: Emit
  # 写入 JSONL，没有读路径、没有查询接口、没有聚合函数
```

系统积累了丰富的正交观测数据（trace: 所有事件 · scorecard: 模型/agent 质量 · telemetry: 延迟/成本），
但**没有任何一层在问**：quality_score 是上升还是下降？gate PASS 率在恶化吗？cost 在失控吗？

### 为什么需要

| 维度 | 理由 |
|------|------|
| **自我认知** | 一个宣称自治的系统，如果不能感知自己表现的变化趋势，就无法自治地调整策略。quality_score 持续下降 → 应自动升级 reviewer model tier。gate PASS 率下降 → 应自动放宽 threshold 或调查 false positive。 |
| **预警前置** | 当前的 fault model 是「gate FAIL → 立即报错」。但更常见的故障模式是「quality_score 连续下降 3 周 → 第 4 周用户抱怨」——在变成 gate FAIL 之前就应该发出 warning。 |
| **退化根因定位** | 没有趋势数据，就无法回答「质量下降是代码库变差、还是评估者(LLM)变差、还是 gate 标准变严、还是模型降档了?」。`forge rca` 的概念在 `novel-extensions-v12.md` 中提到过但从未实现。 |
| **驾驶舱信号** | `forge status` 当前只显示当前状态，不显示状态变化。一个持续下降的趋势比一个静态的「PASS」更有信息量。 |

### 邻近覆盖对照

| 已有文档 | 覆盖了什么 | 本方向新增 |
|----------|-----------|-----------|
| `expansion-next-frontier.md` | 指出「scorecard 不做 trend analysis」的 gap（~5 句） | 完整的趋势检测系统设计：聚合函数、回归分类器、告警阈值、根因分析框架 |
| `agent-orchestration-five-novel-perspectives.md` | `quality_trend == degrading` 的收敛告警概念 | 从「converge 报告里的一个字段」到「独立的趋势检测引擎 + 分级告警 + 自动 root cause 关联」 |
| `novel-extensions-v12-architect-perspective.md` | `forge rca` 的概念性提议 | 将 rca（根因分析）建立在趋势检测的数据基础设施之上——让 rca 可量化、可测试 |

### 关键设计要素

```
# 趋势检测引擎（纯函数，无 IO，可测试）
internal/trend/Trend.go
  - SlidingWindow(window=30d) → 计算 quality_score / pass_rate / cost_per_phase 的移动平均
  - detect_regression(window, baseline): 
      斜率连续 N 期为负 → TrendDegrading
      斜率连续 N 期持平且低于阈值 → TrendStagnant
      斜率正且绝对值超阈值 → TrendImproving
  - detect_anomaly(value, {mean, stddev}): 
      value 超过 mean ± 3σ → Anomaly

# 告警分级
TrendDegrading → forge status 黄灯 "quality_score 连续 3 次 run 下降"
Anomaly        → forge status 红灯 "cost_per_phase 激增 500% [可能原因: model 降档失败]"
TrendImproving → forge status 绿灯 (可选通知)

# 持久化与查询
.forge/trends/<metric>.jsonl    # 每 run 写入一个聚合数据点
forge trend --metric quality_score --window 30d   # CLI 查询
forge trend --alert --slope-threshold -0.05       # 设置告警阈值

# 与 Route 的闭环
trend.QualityScoreDegraded("implementer/sonnet")
  → routing.ScorecardAdjust("implementer", "sonnet", -0.1)  # 自动扣分，下一轮选更优模型
```

---

## 方向五：Delegated Human-in-the-Loop with SLA & Escalation

### 代码证据

```
# HumanApproval 是单用户、二进制、无超时：
forge-core/internal/converge/converge.go: HumanApproved bool
  # true/false 二元信号
  # 来源: --approved flag 或 .forge/<stage>.approved 文件存在
  # 无「谁批准的」记录、无「多久没批准」的告警、无「审批权转交」

# approve 子命令只写一个标记文件：
forge-core/cmd/forge/approve.go
  # os.WriteFile(markerPath, []byte("approved"), 0644)
  # 不记录审批人身份，不验证授权，不通知审批人

# 架构文档诚实承认 durable_wait 未实现：
north-star.md: "long-running, retryable, human-reviewed durable wait (Temporal)"
# v1 的 HumanApproval=「你手动跑 `forge approve`，或自己放一个文件」
```

对于声称「24h 无人值守」的系统，Human-in-the-loop 是最薄弱的一环：
当前实现要求一个特定的人注意到审批请求、停下来手头的活、手动执行 `forge approve`。
如果这个人度假了、请假了、或者只是没看消息——整个 pipeline 卡住。

### 为什么需要

| 维度 | 理由 |
|------|------|
| **24h 自治的矛盾** | 系统宣称自治运行 24h，但 HumanApproval 是纯同步阻塞——没有人注意到审批请求，流程就永远卡住。self-driving car 不能碰到红灯就永远停着。 |
| **团队协作** | 一个工程团队有 N 个工程师。HumanApproval 应当路由到「当前 on-call 的架构师」，而不是硬编码的一个人。没有角色/所有权模型，审批就是单点故障。 |
| **时间敏感** | Design→Build 门闩的等待时间直接影响 feature delivery velocity。没有 SLA/过期机制，一个被遗忘的审批请求可能 pending 数天。 |
| **审计需求** | 谁在什么时间批准了什么——当前 `forge approve` 不记录 `who`（`$USER` 或 `OIDC subject`），零审计。 |
| **紧急 bypass** | 生产事故修复需要 bypass 正常审批。当前没有「emergency override with post-hoc attestation」的能力——要么全松、要么全紧。 |

### 邻近覆盖对照

| 已有文档 | 覆盖了什么 | 本方向新增 |
|----------|-----------|-----------|
| `genuine-architectural-horizons-five.md` | durable_wait 的 v1 诚实边界 | 超越「二进制审批标记」——设计审批路由、SLA、升级链、紧急 bypass 的完整原型 |
| `five-high-value-extensions-v44.md` | 提及设计审批跟踪 | 从「跟踪」到「工作流」——SLA 计时、转交、逐级升级、审批委派 |
| `strategic-extensions-v15-deep-boundary.md` | 紧急 bypass 的 audit trail 概念 | 具体的 bypass-with-evidence 模式——bypass 记录 `who/bypass_reason/authorizing_incident_ticket` |

### 关键设计要素

```
# 扩展 project.yml 支持审批角色定义：
project:
  approval:
    roles:
      architect:       [alice@co, bob@co]       # 默认架构审批人
      security-lead:   [carol@co]                # 安全相关 bypass 须通知
    sla:
      default: 4h                               # 默认 4 小时内响应
      escalate_after: 8h                         # 8 小时后升级
      expire_after: 24h                          # 24 小时后过期（自动拒绝）
    emergency:
      bypass_role: [tech-lead, vp-eng]           # 可紧急 bypass 的角色
      require_post_hoc: true                     # bypass 后 24h 内补审批记录
```

- **审批信号扩展**: `HumanApproved` 从 `bool` → `struct{Approved bool, ApprovedBy string, ApprovedAt int64, ValidUntil int64, BypassTicket string}`
- **审批路由**: `design.yml` 的 `human_approval` 段增加 `approval_roles: [architect]` —— 系统知道该通知谁、等谁
- **SLA 计时器**: daemon 模式下（方向二的前置依赖），forge 持有 pending 审批的计时器。超时 → log(`[APPROVAL] design approval for projectX pending 8h, escalating to [tech-lead]`)  
- **升级链**: `escalate_after` 触发 → 自动 `@mention` 上级角色 / 发 Slack webhook / 写 ticket comment
- **紧急 bypass with evidence**: `forge approve --bypass --reason "hotfix SEC-123" --ticket "INCIDENT-456"` —— 强制记录 bypass 理由，写入 audit trail（方向三的输出端消费者）
- **审批人验证**: v1 用 `$USER` 或 `~/.forge/identity` 文件（自声明，non-authoritative）；v2+ 接 OIDC

---

## 五方向关联与执行建议

```
方向二  Watch Mode/Daemon
  │
  ├── 承载 ──→ 方向一  Binary Upgrade (daemon 进程进行灰度和回滚)
  ├── 承载 ──→ 方向三  Audit Trail (daemon 持久持有 signing key)
  ├── 承载 ──→ 方向四  Trend Analysis (daemon 跨 run 缓存历史)
  └── 承载 ──→ 方向五  Approval SLA (daemon 持有 pending 定时器)
```

**依赖关系**:
- 方向一（Binary Lifecycle）无外部依赖，可独立首发
- 方向二（Daemon）是方向三、四、五的**基础设施依赖**，但方向三/四/五的纯逻辑部分（审计签名、趋势算法、审批模型）可与 daemon 并行设计
- 方向五（Delegated Approval）的 SLA 计时器依赖方向二的 daemon 进程模型

**推荐执行顺序**:
1. **方向一**（独立，低风险，高价值 — 让用户能拿到并信任 forge）
2. **方向二**（架构级投资 — 打开后面三个方向的闸门）
3. **方向三 + 方向四**（可并行 — 纯 go 包，无外部依赖）
4. **方向五**（依赖 daemon，且涉及外部沟通渠道如 Slack webhook，复杂度最高）

---

## 已被排除（或有意识推迟）的方向

- **Web UI / Dashboard** → north-star 已规划为 forge-web (Next.js)，偏离 CLI/声明式核心，v3 范围
- **Multi-vendor model pool (LiteLLM)** → ROADMAP v3，等待跨厂商密钥就绪
- **Firecracker sandbox** → ROADMAP v3，等待 KVM/特权
- **Multi-tenant isolation & billing** → 在 daemon 模式落地之前，多租户的隔离模型无法验证
- **IDE 集成 (VS Code extension, JetBrains plugin)** → 本质上是方向二 daemon 的客户端消费者，应在 daemon API 稳定后启动
