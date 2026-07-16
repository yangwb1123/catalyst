# ForgeOS — Five Production-Grade Extension Directions

> **视角**:资深架构师 / 产品经理。**方法**:全局扫描当前代码库(all source, all docs, all sprints, all requirements audits),识别已有 75+ 份 expansion doc 均未覆盖的、真正高价值的扩展方向。**约束**:不写代码,只出分析。**目标**:每个方向都能在 1–2 个 sprint 内产出可验证的前进步,而非永不可及的北极星。

---

## 方向 ①:预测性成本估算与预算治理 (Predictive Cost Estimation & Budget Governance)

### 为什么需要

当前 forge-core 的成本控制已经实现了三层护栏:per-call 美元上限(`--agent-max-budget-usd`)、run-level 累计上限(`--run-budget-usd`)、phase-count 上限(`--max-agent-calls`)。但这些全是**被动止损**——只有"花到了就停",没有"花之前就知道要花多少"。

对于一个生产环境的团队,在启动一个 `forge evolve` 之前,需要回答:
- 这次演化迭代大概要花 $5 还是 $500?
- 上周跑的 3 次 evolve 一共花了多少钱?哪个 phase/agent 最贵?
- 本月预算还剩多少?还能跑几次 full review 周期?
- 如果我把 reviewer 从 Opus 降到 Sonnet,能省多少?会不会降低收敛质量?

没有预测能力,成本治理就永远是事后诸葛亮。Sprint 26 已经实现了真 cost telemetry(real `total_cost_usd` from claude JSON),但这只是**记录**,不是**预测**。

### 当前基线(可直接利用的资产)

- `cost.go` — 真实美元成本解析,`Observe` hook,`runBudget` 累计器
- `trace.jsonl` — 每个 phase 的 `duration_ms`、`cost_usd_micros`、`model` 均已落盘
- `scorecard_wind.go` — 历史记分卡已按 `task_type` 聚合
- `internal/routing/routing.go:HistoryTiebreak` — 历史择优机制已存在
- Sprint 26 已验证 end-to-end cost telemetry 完整链路

### 扩展内容

1. **Pre-flight cost estimator**:起跑前读取历史记分卡中同类 task_type 的 `avg_cost_usd` + phase 数量 + 预期 loop-back 次数 → 输出预估范围(如 "estimated $8–$15 for this evolve run, 80% confidence")。`forge run` / `forge evolve` 加 `--dry-cost` flag 只出估算不执行。

2. **Cost anomaly detection**:每次 phase 执行后,将实际成本与历史同 task_type + model 的 P50/P95 对比,偏差 > 2σ 时输出 `⚠ cost anomaly: reviewer phase cost $0.42 vs typical $0.12 (3.5x, routed to opus but spent unusually many tokens)`。可配置为 warning 或 blocking。

3. **Per-team/per-project budget allocation**:`project.yml` 新增 `budget:` 段(monthly_hard_cap, monthly_alert_at, owner),`forge accept` 增加 budget 合规检查——不是美元硬墙(那需要计费系统),而是**声明式预算纪律**:超限则 gate WARN 或 BLOCK,看 enforce 级别。

4. **Cost attribution dashboard(仅 CLI)**:`forge cost --since 7d --by phase --by agent` 输出文本表格,聚合 `.forge/trace.jsonl` 中最近 N 天的成本。作为 Web UI 的前置替代,零额外基础设施。

### 边界情况与风险

- **冷启动**:新项目无历史数据时退回到 model-tier 表定价 × 典型 token 消耗(可配置 fallback),并在输出中诚实标注 "cold start — estimate based on tier default pricing, ±50% uncertainty"
- **非 claude 执行器**:echo/dry-run executor 成本恒为 0,应主动识别并标注 "cost estimate N/A (dry-run executor)"
- **多项目聚合**:`project.yml` 的 budget 字段只管单仓;跨项目汇总需方向 ② 的 fleet mechanism 支持
- **美元精度**:claude JSON 输出的 `total_cost_usd` 只到 4 位小数,预估算法的置信区间不应假装更高精度

---

## 方向 ②:语义收敛验证 (Semantic Convergence Verification)

### 为什么需要

当前 converge 判断"工作完成"的信号有三个:
1. **RoadmapCompletion** — agent 自报的 checkbox 勾选率(诚实但信任性,本质是 agent 自评)
2. **GatesGreen** — harness 机械闸门全绿(语法/测试/复杂度等,但**不验证功能正确性**)
3. **FileDelta** — git diff 与 ROADMAP 关键词的路径匹配(粗启发式,agent 可以轻易伪造)

没有一条信号能回答:**"实现代码真的符合需求吗?"**

Sprint 29 的 FileDelta 已经是一种**对抗性诚实机制**——它假设 agent 可能夸大完成度,用 git diff 做交叉验证。但这只是基于**文件名**的匹配。真正的生产级验证需要基于**行为语义**的匹配:
- ROADMAP 条目说 "Add user authentication" → 代码应该包含 login 路由、session 管理、密码哈希
- ROADMAP 说 "Add rate limiting" → 代码应该包含 middleware、限流配置、测试用例
- acceptance 标准说 "API returns 400 on invalid input" → 测试应该覆盖该场景

### 当前基线

- `internal/converge` — 完整的 signal evaluation 框架,`evalOne` 的分发机制可以轻易扩展新 metric
- `gates.go:computeFileDelta` — 已有跨验证思路的技术债务,可扩展为更通用的语义验证器
- `prompt_context.go` — `buildPrompt` 已经把 ROADMAP 条目注入 agent prompt,链路已通
- `asset.go` — `Criterion` 结构体有 `Raw`/`Metric`/`Operator`/`Threshold`/`Value` 字段,可以扩展一个 `AcceptanceScript` 字段("如何验证这条验收标准")

### 扩展内容

1. **Machine-readable acceptance criteria**:ROADMAP.md 的 `- [x]` 条目后允许声明 `[accept: "node --test test/auth.test.mjs && grep '401' <test-output>"]` 形式的可执行验收脚本。`forge accept` 将其作为新的 gate 执行(非载重,同 lint/coverage N/A 模式)。

2. **Behavior-driven convergence metric**:新增 `converge.Signals.AcceptancePass` 字段,对应 `stop_condition.all_of[]` 的新 metric `acceptance_pass`。使 workflow 能声明 "不仅有 roadmap 完成度 ≥ 80%,还要 acceptance test 全绿才收敛"。

3. **Agent-generated acceptance self-check**:在 implementer agent 的 prompt 中注入 "for each ROADMAP item you implement, emit a one-line self-check command at the end of your output(如 `SELF_CHECK: echo 'login route exists' && grep -q 'login' src/routes/`)",由 `cost.go` 的第三级 fallback 解析器(已存在模式,用于 `CONFIDENCE` 和 `VERDICT`)采集,作为额外的轻量信号注入 converge。

4. **Convergence signal dashboard**:`forge converge --verbose` 输出每个 metric 的原始值 + 历史趋势(如 `roadmap_completion=80% (last 3 runs: 45% → 67% → 80%) ; acceptance_pass=3/5 awaiting self-check`),让 operator 能判断收敛是真实的还是表面游戏。

### 边界情况与风险

- **验收脚本的安全性**:可执行验收脚本必须沙箱化——禁止网络、禁止写文件、超时强制切断。复用已有的 `CommandExecutor`(有 timeout + output cap + process group),但加 `--sandbox` 严格模式(readonly filesystem,no-network)。
- **自检的诚实性**:agent 生成的自检命令可能刻意简单(如 `grep -q 'login'` 而非完整集成测试)。应标记为 `self_check` 而非 `acceptance`,在聚合时权重低于真实的 `gates_status`。
- **语义理解的边界**:这不是 AGI——不要求"理解代码是否正确"。只要求"声明过的验收标准可被机械验证"。验收标准写得好,验证就强;写得差(如 "系统应该很快")就退化为 N/A。这是**honesty 的延伸,不是智能的替代**。

---

## 方向 ③:多仓库舰队治理 (Multi-Repository Fleet Governance)

### 为什么需要

ForgeOS 的 `forge-init` 可以完美地初始化一个项目——全套 `.agent/`、harness 工具、CI、seed app, `forge accept ACCEPTED`。这是单人单项目的极致体验。

但真实组织的形态是**舰队**(fleet):10 个微服务、3 个前端、2 个数据管道,各自有独立的仓库。没有舰队治理层,就会:
- 每个仓单独升级 policy → 版本漂移、配置蔓延
- 安全策略更新需要逐个仓 PR → 慢、漏
- 无法跨仓聚合 cost/quality 指标 → 管理盲区
- 每个团队各自微调 policy → 治理碎片化

ADR-0003 (global shared governance via submodule) 从 Sprint 1 就设计就绪,但被推迟到"被治理项目 ≥ 2-3 个"。现在 go-taskd 和 url-shortener 已存在,forge-init 也已经可以生成完整治理。**条件已经触发。**

### 当前基线

- `harness/scaffold/forge-init.mjs` — 完整的项目初始化管道,已能复制全套治理资产
- `.arch/rules.yaml` — 包结构/阈值已参数化为 YAML,可被覆盖
- `harness/policies.yml` — 全局执法策略
- ADR-0003 — 机制已设计(git submodule + 双层覆盖 + 路径解析改造),只待拍板和执行
- `harness/check.py` — 治理完整性检查,可被扩展为跨仓一致性检查

### 扩展内容

1. **Central policy repository scaffold**:`forge fleet init --central <url>` 初始化一个 fleet 治理仓(如 `forgeos-policies`),包含:
   - 全局 `policies.yml` / `modes.yml` / `.arch/rules.yaml` (各下游仓的"上级覆盖")
   - 全局 agent 卡模板(允许局部 override)
   - ADR 跨仓索引
   - 用 ADR-0003 的 submodule 设计,但先做**轻量版**:`forge fleet sync` 从中央仓拉取策略,本地可局部覆盖(覆盖声明写入 `project.yml`),覆盖行为可审计(`forge fleet diff` 展示本地 vs 中央的差异)

2. **Fleet-wide scorecard aggregation**:各仓的 `.forge/scorecards.json` 被可选地推送到一个共享位置(如 S3/Git),`forge fleet scorecard` 聚合全舰队视图:按团队/语言/阶段聚合的 cost、quality、loop-back rate、convergence rate。

3. **Centralized security & compliance dashboard**:secret-scan / SCA / policy 违规跨仓汇总。`forge fleet audit` 输出每个仓的治理状态表(类似 `forge accept` 但跨仓)。

4. **Gradual policy rollout**:中央仓支持 `policy_canary: team-alpha@2weeks` 声明——新策略先在 canary 团队生效、2 周后全舰队推开。每个仓的 `forge accept` 在检测到 pending 策略时输出 "⚠ pending fleet policy P3 will enforce coverage>=80 from 2026-08-01"。

### 边界情况与风险

- **不要求中央基础设施**:初始版本可以基于 git push/pull 工作流(无 server),从轻起步。SaaS 控制面板留到舰队数量 > 5 后再考虑。
- **覆盖权限**:本地覆盖必须被中央记录和审计。不是阻止覆盖(有些团队确有特殊需求),而是让覆盖**可见**、**可追溯**、**有到期时间**(`overrides:` 段可声明 `expires: 2026-09-01`)。
- **submodule 的已知陷阱**:ADR-0003 已分析 symlink/npm/subtree/vendoring 四种方案并否决前三者。需验证 submodule 的 `git -C` 远程操作不会与 forge-core 的 `command_executor.go` 的 `Dir` 和工作目录设定产生冲突(当前 `CommandExecutor.Dir=o.root` 保持不变,submodule 的工作流应只涉及 `forge fleet` 命令内部的 git 操作)。
- **不能等最终设计**:舰队治理不需要一开始就完美。MVP 只需要"一个共享策略仓 + `forge fleet sync` + 跨仓 diff 审计"。走 ADR-0003 的 Stage A(纯拉取,不强制)开始,无需等到完整分布式设计做完。

---

## 方向 ④:异步协作人审界面 (Human-in-the-Loop Collaboration Surface)

### 为什么需要

当前的人审闸门是一个**二进制信号**:要么有 `.forge/<stage>.approved` 标记或 `--approved` flag,要么没有。`reportHumanGate` 诚实地输出 "awaiting human approval (non-bypassable)" 或 "approved → unlocks next_stage"。

但这与现实世界的工程协作严重脱节。真实的代码评审不是"允许/拒绝"的二元问题:
- "API 设计 OK,但需要补测试" → **有条件批准**
- "架构合理,但我对性能评审中的缓存策略有疑问,请更改后重新提交" → **带反馈的重做**
- "这部分改动太大,拆成两个 PR" → **拆分建议**
- "approve —— 但仅限于 P1(P2 下次迭代)" → **部分批准**
- "我周五前没空看,但我不 blocking" → **超时自动批准**

没有这些人审语义,ForgeOS 的自治循环只能要么卡住(等待永远不会来的批准),要么盲目放行(在没有真正人类判断的情况下冲过闸门)。**这是从「单兵利器」到「团队平台」的最大跃迁障碍。**

### 当前基线

- `converge.go:humanGate()` — 已有 `human_gate` stop condition 的评估框架
- `.forge/<stage>.approved` — 标记文件机制已存在,可扩展为 `.forge/<stage>/` 目录存放多状态
- `on_rejected` — Sprint 31 的 loop-back 机制已实现,可根据 rejection 类型定向跳转
- `approve.go` — `forge approve list` 命令已存在,只差扩展 approve 子命令的语义
- `design.yml` — human_gate 声明在 `design->build` 闸门处,review.yml 的 cto 裁决也有 5 种 verdict

### 扩展内容

1. **Rich approval states**:`.forge/<stage>/` 目录下存储结构化审批元数据(JSON):
   ```json
   {
     "status": "approved_with_conditions",
     "approved_by": "user@example.com",
     "approved_at": "2026-07-10T14:30:00Z",
     "conditions": "Approved for P1 API routes; P2 caching layer needs redesign — see review-thread-5",
     "target_phase": "implementer",
     "expires_at": "2026-07-14T14:30:00Z"
   }
   ```
   当前 `--approved` 标志位兼容:无 .forge/<stage>/ 目录时退回到二进制逻辑。

2. **`forge approve` 子命令扩展**:`forge approve design --with-conditions "..." --target-phase architect --expires 72h`。`forge reject design --reason "architecture needs simplification" --loop-back-to proposal-generator`。`forge approve list` 扩展为 `forge status` 显示所有待审闸门 + 各闸门的等待时长。

3. **Async review workflow 支持**:`forge run design` 在 human_gate 处不终止进程,而是持久化等待标记后 exit 0(clean pause)。另一次会话中 `forge review design` 读取等待标记、展示 diff/emits/裁决请求、让人类操作员交互式审查。(这是 `durable_wait` 的轻量替代——不依赖 Temporal,只靠文件系统和两次 CLI 调用。)

4. **Diff-aware approval context**:`forge approve design --review` 展示提议的变更摘要(产出物 diff、修改的文件列表、关键决策),而非仅仅要求批准一个隐式请求。人类操作员在批准前看到"这条 PRD 新增了 3 个 API endpoint、修改了数据模型、增加了一个新的外部依赖"这样的上下文。

### 边界情况与风险

- **不是 Slack bot / Web UI**:CLI-first,保持 ForgeOS 的核心哲学。Web UI 仍然是 v3。但 `forge review` 交互模式可以是一个 curses/TUI 风格(参考 `pi` 的思路),或者简单的 prompt 驱动。
- **条件批准的执行**:关键是"执行了条件的验证"。如果人类说"approved with conditions: must add test coverage for auth endpoints",系统需要在下一阶段验证这个条件是否被实现,而非简单放行。可以用方向 ② 的语义验证来检查条件是否满足。
- **安全**:`.forge/` 是 git-ignored,但审批元数据需要防篡改。MVP 阶段不做加密签名(本地开发者信任),但 `forge fleet` 场景下需要 `approval.sig` 用 GPG/minisign 签名。
- **时区**:分布式团队的等待超时需要感知时区。条件中的 `expires_at` 应使用 ISO 8601 + UTC,`forge approve` 在输出时间时使用本地时区渲染。

---

## 方向 ⑤:自治运行可观测性与事后调试 (Autonomous Run Observability & Post-Mortem Debugging)

### 为什么需要

当一个人运行 `forge evolve` 并且它未能收敛(或者更糟——收敛到了错误的结果),目前唯一的事后诊断工具是直接读取 `.forge/trace.jsonl`。这是原始的 JSON 事件流,没有结构化查询、没有可视化、没有对比分析。

在 Sprint 26 的真人测试中,团队能够观察到 "agent 说它完成了但 gate 没过"——但那是实时观察的。如果同样的问题发生在凌晨 3 点的无人值守运行中,第二天早上 operator 面对的是一个空荡荡的 exit code 和一串 JSONL。

生产级自治系统需要**事后可调试性**(debuggability post-mortem):
- 这次 run 和上次成功的 run 有什么不同?phase 时间线对比
- 为什么 budget 烧完了?哪个 phase 花的钱超出预期?
- 为什么 agent 反复 loop-back?reviewer 每次 rejection 的具体原因是什么?
- 能不能"重播"某次 run 的决策过程,但不实际调用 LLM?

**没有可观测性,自治就是盲飞。** 这是从"能跑"到"可信"的关键一跃。

### 当前基线

- `internal/trace/trace.go` — 完整的事件溯源框架,支持 `Event` 类型的 phase/gate/checkpoint/convergence/iteration
- `trace.jsonl` — JSONL 格式的持久化事件流,零额外基础设施
- `scorecard_wind.go` — 运行完成后的 wind-down 回调已存在
- Sprint 26 已经验证了 trace latency(2640ms) 和真实 cost(0.1841) 的落盘
- `persist/checkpoint.go` — phase-granular checkpoint 已实现,支持 resume

### 扩展内容

1. **Run comparison (`forge diff --runs`)**:接受两个 `.forge/trace.jsonl`(或 checkpoint ID),输出结构化对比:
   - phase 数量、总成本、总时长、loop-back 次数差异
   - 每个对应 phase 的 model tier / cost / duration / verdict 对比表
   - "Run A 比 Run B 多花了 $2.30,主要是因为 reviewer 阶段 loop-back 了 2 次(对比 0 次)"
   - 基于事件的甘特图(纯文本,用时间线格式)

2. **Failure root-cause summary**:当一次 run exit non-zero 时,`forge run --explain` 重读 trace.jsonl,分析失败链:
   - "Gate `test` FAILED in phase implementer → investigo 3 test failures found → all relate to auth middleware → suggest checking auth routes"
   - "Run aborted by budget exhaustion: spent $12.50 of $10.00 cap → most expensive phase was security-review at $4.20 → consider --agent-max-budget-usd for reviewer phases"
   - 基于已有 trace 事件的模式匹配(非 LLM 分析,纯规则引擎)

3. **Phase replay (`forge replay --phase <name> --from-run <trace>`)**:从历史 trace.jsonl 中提取指定 phase 的 prompt 和产出,用当前代码状态和**相同**的 model tier 重跑该 phase,输出"这次的结果 vs 历史记录对比"。(关键:不改变已有文件,只输出对比报告。目的是诊断"如果重跑会不会不一样?")

4. **Structured run log view (`forge log --timeline --phase <name>`)**:把原始的 JSONL 渲染为人类可读的多层级时间线:
   ```
   forge evolve (2026-07-10 02:15:03 UTC) — 6 iterations, 23 phases, $8.42
   ├── Iteration 1 (02:15:03→02:18:44, 3m41s, $1.20)
   │   ├── scan          dry-run          0.02s   $0.00   ⏺
   │   ├── gap-analysis  opus             1m12s   $0.45   ⏺
   │   ├── implementer   sonnet           1m55s   $0.62   ⏺
   │   └── gate(test)    —               12s     $0.00   ✅ PASS
   ├── Iteration 2 (02:18:45→02:25:10, 6m25s, $3.80)
   │   ├── scan          dry-run          0.02s   $0.00   ⏺
   │   ├── gap-analysis  opus            1m08s   $0.42   ⏺
   │   ├── implementer   sonnet          2m30s   $0.85   ⏺
   │   ├── gate(test)    —               14s     $0.00   ❌ FAIL → loop-back to implementer
   │   ├── implementer   sonnet (retry)  2m10s   $0.72   🔄
   │   └── gate(test)    —               11s     $0.00   ✅ PASS
   ├── ...                 ...            ...     ...     ...
   └── convergence: MET (roadmap 100%, gates green) — 6 iterations
   ```

### 边界情况与风险

- **trace 膨胀**:长时间运行(100+ 迭代)的 evolve 会产生大量 trace。`trace.jsonl` 应支持自动轮转(每 5000 事件新文件),`forge log --last` 默认只读最近的 trace 文件。`forge log --all` 显式合并全部。
- **敏感信息**:trace 中的 agent prompt 可能包含 ROADMAP 内容或代码片段。`forge log --redact` 选项,输出时过滤潜在敏感关键词(API key pattern,`-----BEGIN` 等)。secret-scan 可复用。
- **replay 的精确性**:`forge replay` 不能保证完全相同的 LLM 输出(模型非确定性)。replay 的目的不是"复现 bug",而是**诊断 prompt 质量和 model tier 的影响**。输出应诚实标注 "model output may differ from original due to LLM non-determinism — focus on structural comparison(phase duration,model tier,cost)"。
- **非 claude 执行器**:dry-run/echo executor 的 trace 中 prompt 和 cost 字段为空,渲染时应诚实显示 `—` 而非假装有数据。

---

## 优先级建议

| 方向 | 用户价值 | 实现成本(估算) | 依赖 | 推荐时序 |
|---|---|---|---|---|
| ① 预测成本 | 高(省钱) | 低(1 sprint) | 已有 cost telemetry | **Sprint N+1** |
| ⑤ 可观测性 | 高(信任) | 中(1-2 sprints) | 已有 trace 框架 | **Sprint N+1** |
| ② 语义收敛 | 高(质量) | 中(1-2 sprints) | 已有 converge 框架 | **Sprint N+2** |
| ④ 异步人审 | 高(协作) | 中(1-2 sprints) | 已有 human_gate 机制 | **Sprint N+2** |
| ③ 舰队治理 | 中(规模) | 高(2-3 sprints) | ADR-0003 待拍板 | **Sprint N+3** |

**推荐首发(同 sprint 可并行)**:
- **Sprint A**:方向 ①(cost estimator + `forge cost`) + 方向 ⑤(`forge log --timeline` + failure RCA)
- **Sprint B**:方向 ②(acceptance gate + semantic converge metric) + 方向 ④(`forge approve` 扩展 + async review)
- **Sprint C**:方向 ③(fleet sync + fleet audit + cross-repo scorecard)

每 sprint 后 `forge accept` 应仍 ACCEPTED,每个方向产出可独立验证的增量(非"框架搭好但零消费")。这与当前 31 个 sprint 的纪律一致:每 sprint 提供可验证的停止闸门通过的产出。

---

## 不做的(anti-product)

- ❌ **Web UI**:v3 范畴,不在此 roadmap 内。CLI first 是 ForgeOS 的基因。
- ❌ **自有模型微调**:ForgeOS 是治理层,不是 ML 平台。方向 ② 的语义验证靠脚本/规则,不是 LLM-as-judge。
- ❌ **实时协作(多人在线编辑 workflow)**:异步协作 > 实时。单用户 CLI 到团队异步 CLI,跳过 WebSocket 多人编辑。
- ❌ **"自治"全覆盖**:人类审批不会消失。方向 ④ 的目标是让人类审批更高效、更上下文感知,而不是消灭审批。
