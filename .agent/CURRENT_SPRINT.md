# ForgeOS — Current Sprint

## Sprint 1–4 (✅ 完成)
- **S1 声明式治理层**:9 agent / 7 skill / 4 workflow / mode 矩阵 / 路由 / 评估 schema / 适配器 / BOOTSTRAP / 2 ADR;gate 切 `block`。
- **S2 验证脊柱**:多 agent 跑通 `build.yml`(plan→impl×2→gate→reviewer→fix);消除 SoT 漂移。
- **S3 Evaluation Stop 闸门**:`forge accept` 聚合判 ACCEPTED/REJECTED,诚实标 n/a。
- **S4 Dogfood 真实应用**:`examples/url-shortener` 经完整 pipeline(architect→3 implementer→fresh reviewer→fix)端到端建成;reviewer 揪出"app 未被 accept gate 覆盖"并补全;现 39 app 测试被 `forge accept` 实际执法。

## 🏁 里程碑:ForgeOS 工厂已证可用
v0+v1 完成 + **首个真实产品被端到端造出并自我治理**。harness 三工具(gate/check/accept)+ 28 自测(check 12 / gate 8 / accept 8);app 39 测试;整仓 `block` 模式全绿。

## Sprint 5 (✅ 完成) — 扩展五方向(全局扫描 → 根 ROADMAP → dogfood 实现)
全局扫描代码库提出根 [`ROADMAP.md`](../ROADMAP.md) 五方向,多 agent 并行实现 + fresh-review + 主控逐方向跑闸门:
- **① 韧性运行时**:超时/取消 · 错误分类+重试消费者 · trace · checkpoint/resume(forge-core +`trace`/`persist` 包)
- **② 学习闭环**:scorecard trajectory · history-tiebreak · converge per-criterion
- **③ Context/Memory**:检索器 `retrieve` · `memory` 包 · Context Engine 注入(硬约束始终注入)
- **④ 执法补完**:function-length + circular 机器执法(消除两个 `policies.yml` TODO);reviewer 抓出 brace-matcher 假阴性已修;两处上帝文件(main.go/scan.mjs 499)按「先拆分」拆分
- **⑤ 安全合规**:secret 扫描(`security_findings` N/A→真查,纳入 LOAD_BEARING)· `risk` 分类器(critical→Opus)

## 🏁 里程碑:扩展五方向核心全部交付
forge-core **11 Go 包**纯 stdlib 零依赖;arch-check **8 检查** + secret-scan;整仓 honest 全绿(go test 11 包 / 四闸门 / accept ACCEPTED / secret-scan 0)。每方向经 fresh-context reviewer 独立审(方向五 reviewer 遇 API 故障 → 主控亲自审 + 对抗验证)。**dogfood 成功**:方向四的 function-length 执法在方向五并行开发中真实抓到 113 行测试函数 → 被迫重构。

## Sprint 6 (✅ 完成) — 治理深化(全局化继承 + 最高杠杆闸门)
- **forge-init 全局化**:新项目继承**全套 host-independent 执法器**(`gate`+`arch-check`+`scan`+`secret-scan`+`.arch/rules.yaml`,实跑 out-of-the-box 三执法器 PASS);`check`/`accept` 随 `.agent/` 充实后启用(诚实标注,不假装完整 accept)。
- **Human-Approval 闸门**:实现 `design.yml` 的 `human_gate`(`converge.Converge`,批准才收敛,未批准 `awaiting human approval (non-bypassable)`);`durable_wait`(Temporal)诚实标注 v2/v3。**fresh reviewer 抓出 `forge evolve` 路径绕过审批(安全漏洞:LoopEngine 调 `Evaluate` 而非 `Converge`、丢弃 `HumanApproval`)→ 纵深修复**(`cmdEvolve` fail-closed 拒绝 human_gate + LoopEngine 改走 `Converge`)+ 补 LoopEngine 层对抗测试。实跑坐实:`forge evolve design` 由 exit-0 绕过 → exit-1 拒绝。

## Sprint 7 (✅ 完成) — 中枢旋钮第三块:Workflow 深度 mode-gating
`mode×lifecycle` 现在真正驱动**三处**(Router ✅ + Harness ✅ + **Workflow 深度 ✅**):新 `forge-core/internal/mode`(distill modes.yml)按 mode 过滤 gate-set + skip reviewer phase。**★安全★ production lifecycle 一票否决强制全执法**(explorer+production 实跑 = 全 6 gate + reviewer,override 不可被宽松 mode 绕过);fail-safe 未知输入→全开(绝不漏执法);零值=全开向后兼容。fresh reviewer APPROVE(empirical 坐实 override 与 fail-safe)。仅 gate-set+reviewer 维度;discover/adr/evolve 深度诚实标注后续。

## Sprint 8 (✅ 完成) — 中枢旋钮迁移:explorer→engineering 状态迁移
`forge migrate --to engineering`:vision 的「创业→企业」治理升级。读 modes.yml 的 migration → 收紧 harness(全 6 gate / coverage 80 / block)+ 抬 router floor(haiku→sonnet)+ 启用 workflow + **派生 5 补债任务**(backfill-tests / add-ci / add-monitoring / refactor-oversized / security-pass)。**默认 dry**(打印 plan 不写文件),`--apply` 才改 project.yml mode + 注入 ROADMAP。fresh reviewer SHIP(stress-test project.yml 多格式保留 + Plan 逐行对齐 modes.yml + apply-ordering 安全)。新 `internal/migrate` 纯叶子(零 import)。

## Sprint 9 (✅ 完成) — 方向五补全:risk 特征自动提取
`risk.FromChangedPaths` 从改动文件路径启发式推 risk 特征(payment/auth/secret/migration→irreversible + BlastRadius),接 `forge route --diff-files/--from-git`(fail-tolerant)。**只推高不压低**(auto 与人工 `--risk`/`--touches-*` 取更严,OR/max/AND/manual-only merge——empirical 坐实);honesty:粗启发式(只读路径不读内容/调用图)、ProdTraffic 永不从路径凭空推断、`--from-git` 仅 tracked 改动。fresh reviewer APPROVE。

## Sprint 10 (✅ 完成) — forge-init 完整 Project Harness Template
forge-init 从「复制执法器」升级为「复制**完整治理**」:`.agent` 通用资产(agents/skills/workflows/eval/routing/policies)+ 全套 harness(工具+自测)+ 生成 CLAUDE.md + CI(`.github/workflows/forge.yml` 跑 forge accept)+ seed app `examples/starter`(真实通过的脚手架起点)。新项目 `forge accept` **ACCEPTED**(6 真 PASS + 4 诚实 N/A,完整治理非仅执法器)。fresh reviewer APPROVE(falsification 坐实 seed app 真测试 + PyYAML skip 不掩盖)。兑现全局化 70% 全局 / 30% 项目差异。

## Sprint 11 (✅ 完成) — 补三个「声明但未实现」gap(逻辑/框架可测,缺工具诚实 N/A)
- **scorecard recency 衰减**(`policy.yml` recency_half_life_days=30):`decayWeight` 指数衰减 + `merge` 按 age 衰减旧分权重,fail-open(坏时间戳→1 不污染),向后兼容(decayFactor=1 逐位不变)。
- **mode-gating evolve 维度**(`modes.yml` workflow_depth.evolve):`EvolveDepth`→max-iter(opportunistic 2/standard 5/thorough 10/advisory 1),production override 收紧,显式 `--max-iter` 优先。
- **adapters lint 接入框架**(`adapters/*.yml` 从纯声明→可执行):`probeLint` 探测语言→读 adapter→linter 装且配好则跑、缺/未配则诚实 N/A(eslint 装了但无 config→N/A **不 FAIL**),非 load-bearing,装上配好即自动执法。forge-init 同步复制 adapters。
fresh reviewer APPROVE(三块;honesty footgun 对真实 eslint exit-2 坐实无伪造)。

## Sprint 12 (✅ 完成) — adapters coverage + 系统「声明 vs 实现」审计
- **adapters coverage 接入**:coverage criterion 从恒 N/A → 可执行框架(像 lint:探测语言→读 adapter coverage→工具装且能跑则跑对照阈值、缺/未配则诚实 N/A)。honesty:go 用 `go version`、`parseCoveragePercent` 只信 %-signed、installed-but-unrunnable→N/A 不 FAIL、清理 coverage 产物不污染被判树。非 load-bearing。
- **lifecycle floor 已实现确认**:task 8/12 已接 `require_min_gates` floor(我误判后诚实纠正),补 `coverage_delta`/`enforce_floor` 的 honesty 注释(归属其他子系统)。
- **系统审计**:逐一核对 `.agent` 声明 vs forge-core/harness 实现,产出准确 gap 清单。真正未实现的本环境可验证 gap:orchestrator loop-back 控制流(`on_fail`/`on_unmet`,最大)、adapter `test:` 命令零消费、coverage threshold 未按 mode、`priorities` 零消费、`workflow_depth.discover/design/adr`/`model_tier` 未 modeled。

## Sprint 13 (✅ 完成) — orchestrator loop-back 状态机 + adapter test 消费(审计最大 gap)
- **orchestrator loop-back**(审计指出的最大 gap):asset.Phase 加 `OnFail`、StopCondition 加 `OnUnmet`;`RunFrom` 在 gate FAIL 且声明 `on_fail.loop_back` 时**定向跳回 target phase**(按 name 找 index,非 abort 非整体 replay),`MaxLoopBack` 上限 fail-closed;LoopEngine `on_unmet` 让后续迭代从 target phase(planner)起。向后兼容:无 on_fail 仍 abort,human_gate/mode-gating/resume/retry 逐位保留。dry-run 下机制就绪(fake fail-then-pass 坐实跳转),真修复需真 agent。
- **adapter test 消费**:probeAppTests 改走 adapter `test:` 命令(go-taskd via `go test ./...`),布局不匹配则诚实 fallback(url-shortener:vitest 不匹配 .mjs→node --test)。两条路径都 load-bearing(注入破坏坐实 REJECTED)。
fresh reviewer APPROVE(定向 loop-back 真实 + 向后兼容注入验证 + load-bearing 两路坐实)。

## Sprint 14 (✅ 完成) — coverage 阈值 mode×lifecycle + per-phase model_tier
- **coverage 阈值按 mode×lifecycle**:从 hardcoded 60 → 读 project.yml mode×lifecycle 对照 modes.yml(`coverage_threshold` + `coverage_delta`,封顶 95);fail-safe 缺→60;coverage 工具 N/A 时不改 N/A 结果。fresh review 抓出 **copy-anywhere regression**(测试 hardcode "本仓=80" 但复制给每个 balanced 脚手架=60)→ 修(测试改为读 host config 动态算期望,balanced 脚手架 ACCEPTED 恢复)。
- **per-phase model_tier override**:build.yml 的 `implementer:sonnet`/`reviewer:opus`(之前 parse 后丢弃)现作为 raise-only override 生效(`Higher(base,model_tier)`),**安全下限只升不破**(reviewer/architect 即使 phase 写低档仍 opus,注入坐实)。
fresh reviewer:B6 APPROVE;C1 REQUEST-CHANGES→已修。

## Sprint 15 (✅ 完成) — 中枢旋钮 workflow_depth 全维度齐(discover/design/adr)
mode.Policy 补 `DiscoverDepth`/`DesignDepth`/`ADR`(之前 modes.yml 声明但未消费):discover stage 在 explorer **skip**(0 phases)、engineering full;ADR 在 design stage 按 mode 叙述 required/not;production override 一票否决→full/full/true;fail-safe→full。honesty:dry-run 下是**决策就绪+叙述**(报告 skip/depth/ADR verdict),不假装真跑了 discovery 或真写了 ADR(需真 agent)。向后兼容:build 不受影响,gate-set/reviewer/evolve/loop-back/human_gate/model_tier/resume 全保留。fresh reviewer APPROVE(production override + 向后兼容经 8 个 live forge run 坐实)。
**★中枢旋钮完整★**:一个 mode×lifecycle 设置现驱动 **Router 档位 + Harness gate-set/严格度 + Workflow 深度(discover/design/adr/reviewer/evolve) + migration**。

## Sprint 16 (✅ 完成) — test_acceptance copy-anywhere 加固
`test_acceptance.mjs` 的「real repo ACCEPTED」集成测试 hardcode 了 forgeos examples(go-taskd/url-shortener)+ 环境细节,作为 forge-init COPIED_FILE 复制到脚手架(只有 starter)直接跑会失败,靠 INNER skip 掩盖。改为 **host-agnostic**(验证 ACCEPTED + load-bearing PASS + adapter/fallback 路径生效模式,不绑 app 名/工具状态);核心保证保留、INNER 防递归 guard 不动。**脚手架直接跑 test_acceptance 现 exit 0(实际跑+过,不再靠 skip 掩盖),copy-anywhere 不变量真正成立**。

## Sprint 17 (✅ 完成) — priorities 诚实处理(审计 B1):校验 + 可观测,不发明路由语义
`modes.priorities`(speed/quality/cost ranking)声明但零消费。诚实分析:它是 mode trade-off **意图**,效果已隐含在 router_tier/gates/evolve;硬接独立「priorities→路由加权」会发明 modes.yml 未声明的行为(镀金)。两个诚实处理:① check.py `check_mode_priorities` 校验(治理完整性,消除零消费 + 防声明漂移)——**诚实发现 cto priorities `{speed:3,quality:1,cost:3}` 是故意 tie(不产代码),故 enforce ranking 弱序而非严格排列,不误报 cto**;② forge route surface priorities(可观测,`--mode` flag)。诚实标注:不假装 priorities 独立驱动路由;独立加权语义待设计决策。check.py 8 checks PASS、test_check 24 测试。

## Sprint 18 (✅ 完成) — enforce 按 mode×lifecycle:中枢旋钮 Harness 严格度完整
gate.mjs 的 enforce(warn/block)从读 policies.yml 全局 → `resolveEnforce` 按 project mode×lifecycle 解析(modes.yml enforce + lifecycle enforce_floor,取更严);**production 强制 block 一票否决**(任何 mode×production→block);fail-safe 缺/garbage→block 保守。honesty:warn 模式**仍报告每个违规**(文件+数)但 exit 0、block 报告 + exit 1、违规永不静默。向后兼容:本仓 engineering×mvp→block 不变。(API 超时恢复:impl 正确,test_adapters 超限 603→拆出 test_enforce.mjs + 修 collateral test bug。)fresh reviewer APPROVE(production override + warn honesty + 向后兼容 fixture 坐实)。**★中枢旋钮 Harness 严格度完整(gate-set + enforce + coverage)★**。

## Sprint 19 (✅ 完成) — SCA/CVE + cost/latency telemetry:诚实适配器框架(把「需外部资源」做成真框架)
把两项「曾推迟为需外部资源」做成真实可验证框架,外部数据缺则诚实降级(同 lint/coverage 适配器模式)。**SCA**(`sca.mjs`):OSV-format advisory 解析 + semver 匹配引擎(parseManifest go.mod/package.json/requirements.txt;半开区间 [introduced,fixed);ecosystem 隔离),接 acceptance 非载重 `dependency_vulnerabilities`——有 DB→PASS/FAIL(真漏洞阻断)、无 DB→N/A(不伪造扫全网),供 OSV/NVD DB 即全功能。**telemetry**(`scorecard*.mjs`):percentile 引擎填 schema 的 p95_latency_ms(从 trace.jsonl duration_ms 真实测量)/avg_cost_usd(token×单价估算)/window;无数据→省略不编 0、真 0 仍记录;向后兼容逐位。copy-anywhere:forge-init 纳入 sca.mjs + 两新自测,新项目仍 ACCEPTED。fresh review APPROVE(独立 fixture 坐实 semver 边界/阻断语义/honesty)。

## Sprint 20 (✅ 完成) — recursion-depth guard:真点火安全前置①(防深度 fork-bomb)
真 agent 被 prompt 驱动可自调 `forge run --executor=command`→ 再 spawn agent→ 无限递归 fork-bomb 烧预算(真点火不敢启用的关键障碍)。`CommandExecutor` 经继承的 `FORGE_AGENT_DEPTH` 跨进程计数,每次 spawn 注入 parent+1(`childEnv` REPLACE 而非 append——重复键解析跨 libc 未指定),达上限拒绝(不可重试 `KindRecursionLimit`)。默认 cap 2、`--max-agent-depth` 可配。fail-safe:garbage/缺→0 不阻断合法顶层;honesty:防**意外**递归、非恶意篡改 env。fresh review REQUEST-CHANGES(libc 事实纠正 glibc 返回 LAST + fail-safe 安全边界标注)已修 → APPROVE。

## Sprint 21 (✅ 完成) — agent-call budget guard:真点火安全前置②(成本上界)
recursion guard 的配对:guard 防深度,budget 防单次 run 的**总** agent-phase 执行数(N phase × K loop-back 重跑 = N×(K+1) 真 spawn,MaxLoopBack/MaxIter 不覆盖)。`Engine.MaxAgentCalls`:RunFrom 在每个 runAgentPhase **前** checkAgentBudget 计数,超限 fail-closed(phase 永不 Execute,spawn ledger 坐实);loop-back 重跑计入。默认 0=无限(向后兼容);`--max-agent-calls` 接 run+evolve。**evolve 为 per-iteration**(计数每迭代重置,总 ≤ max-iter × this)——flag/字段/error 全处诚实披露。fresh review(6 独立 fixture)REQUEST-CHANGES(evolve 文档诚实)已修 → APPROVE。**★真点火安全护栏完整成对(深度 + 总量)★**。

## Sprint 22 (✅ 完成) — output-size cap:真点火安全前置③(防 runaway 输出 OOM)
CommandExecutor 原 `CombinedOutput()` **无界**读子进程 stdout/stderr 到内存——runaway 真 agent 会 OOM forge。改 `cappedBuffer`(保留 ≤ cap、drain 其余、Write 永不 short-write 免 wedge 子进程)+ `cmd.Run`,同指针 Stdout+Stderr 让 os/exec 串行化(stdlib same-writer 保证,无锁)。截断诚实标注、不假装完整。`--max-output-bytes` 可配、默认 10MiB(对正常 phase 日志透明)。fresh review APPROVE(自测 10MB 流过 1KiB cap 只留 1KiB;`-race -count=20` 并发 stdout+stderr 零 race;边界 honesty)。**★真点火资源安全护栏四维完整:深度(recursion)+ 数量(budget)+ 时间(timeout)+ 内存(output-cap)★**。

## Sprint 23 (✅ 完成) — acceptance.mjs 单一职责拆分(dogfood,reviewer flag)
acceptance.mjs 涨到 499/500 且把 共享 runner kernel + 7 probe + app-test + 编排 + 裁定 + 渲染 塞一文件——违反 ForgeOS 自己「单一职责」规范(非 500 行硬限,它合规;dogfood 纪律即拆)。拆三:`acceptance-kernel.mjs`(58,纯原语 run/result/splitCmd + PASS/FAIL/NA/ROOT,**只 import node:**,依赖图底)· `acceptance-quality.mjs`(155,lint+coverage adapter probe)· `acceptance.mjs`(345<400,编排 + 其余 probe + app-test + collect/decide/render)。共享原语下沉 kernel 保无环(kernel←quality←acceptance,circular-dependency PASS);re-export 保 test import 不变;forge-init 复制两新模块(copy-anywhere)。**零行为变化逐字节铁证**(默认 + --json 双模式 git-stash diff 空)、211 自测全绿。

## Sprint 24 (✅ 完成) — 真点火真 claude 端到端坐实(+ 暴露并修两个 gap)
用户授权后,throwaway 项目用真 `--agent-cmd=claude` 跑最小 implement→gate→converge workflow,**完整闭环在真 LLM 下坐实**:claude 真写 `multiply.mjs`(纯函数)+ node:test → harness-gates 真跑 test+complexity 绿 → 收敛。环境检查纠正了「需外部凭证」的错判(claude CLI 在 PATH、OAuth 认证可用)。真跑暴露并修两个真 gap:① **任务注入**——buildPrompt 的 Gather 原只注入 ADRs+constraints、无任务源,agent 不知实现什么;加第三 lane 注入 `.agent/ROADMAP.md`(capped 至 taskCap)。② **写权限**——`claude -p` headless 默认只描述不施加编辑;agentExecutor 对 claude-family 加 `--permission-mode acceptEdits`(自动接受文件编辑、不放开 Bash),`--agent-permission` 可配。两 gap 单测覆盖;permission 测试推 main_test 过 500 → 拆 evolve_test(零行为变化)。docs/ignition.md 记录闭环 + 旋钮。**★真点火从「echo 坐实基础设施」跃升为「真 LLM 完整闭环坐实事实」★**。

## Sprint 25 (✅ 完成) — 真点火 multi-agent 跑到 converge MET(增量级 + 版本级,诚实分工)
用户授权烧钱测试后,真 `--agent-cmd=claude` 跑完整 5-phase build(planner→implementer→harness-gates→reviewer→qa)多-agent 自治协作。真跑暴露并修三个新 gap:③ **模型路由**——Build 无 `--model`、routing 算的 tier 被丢弃;导出 `orchestrator.PhaseTier`,Build 对 claude 加 `--model <tier>`(opus 下限 + override 真生效)。④ **工作目录**——`CommandExecutor.Dir=o.root`,agent 在项目根写码而非 forge cwd。⑤ **成本第三维**——claude `--max-budget-usd` 经 `--agent-max-budget-usd` per-call 美元封顶(直接回应「真点火烧钱」)。**converge MET 坐实**:`mab`(stop=gates_status==green、全工具门)真 claude 跑到**增量级 MET**;`vab`(stop=roadmap 100% AND gates green)跑到**版本级 MET**——揭示 honesty 的**机制层**:implementer(acceptEdits 无 Bash)跑不了自查 → **诚实拒绝勾 ROADMAP**,客观验证交 harness、版本竣工留人确认。`evolve` echo 验证 LoopEngine 多迭代 + checkpoint/memory/trace 落盘 + converge 驱动停止(非 round-count)。**★真点火验证矩阵全维度坐实:single/multi-agent · 增量/版本 converge MET · 多迭代演化 · agent 自治 + 人确认的诚实分工★**。

## Sprint 26 (✅ 完成) — 真点火深化:观测闭环 + pipeline 数据流 + 闸门自纠
延续 S24/25,真 claude 跑续暴露并修真 gap(累计**八个**):⑥ **trace latency**(evolve iteration `duration_ms` 恒 0、telemetry 算不到真延迟 → LoopEngine 测 iteration 墙钟 → `OnIteration` → checkpointHook 写 `Event.DurationMs`;真 claude 坐实 2640 → scorecard p95=2640)⑦ **cost telemetry**(`avg_cost_usd` 恒 n/a → claude `--output-format json` 真实计费 `total_cost_usd` 经通用 `Observe` hook → claude-specific `cost.go` 解析 → trace `cost_usd_micros`(per-phase)→ scorecard;坐实 avg_cost_usd=0.1841)⑧ **reviewer 缺前序 gate 信号**(acceptEdits 无 Bash 下盲目试 `node --test` 重验、烧穿 budget → `Engine.OnGateResult` 回调 → `gateLedger`(prompt_context.go)→ buildPrompt 注入 harness-gates 客观裁决;真 claude 前后对比:5 Bash-denial+budget 烧穿 → 0-Bash+省 31%+产真裁决)。**★Learning loop 三维真数据完整:quality+latency+cost★**(telemetry 框架早备,本轮真 claude 补齐 latency/cost 真数据)。
**pipeline 数据流**:gate 裁决注入 reviewer + planner 任务拆分前传 implementer/reviewer(`feeds_forward`/`phaseOutputLedger`;**避污染**:只规划角色前传、reviewer 绝不收 peer 自述、保 fresh-context 独立性,echo+单测+fresh-review 三重坐实)。
**闸门自纠**:arch-check `checkFanin` 误把测试文件算进耦合(与同文件 checkLayering/checkPackage 排除测试不一致)→ 误报纯数据模型包 `asset`(7 生产 importer 被 13 测试文件顶到上限)、逼出扭曲 workaround;修(排除测试)+ 上限对齐 repo 约定(7×2=14)。**教训:闸门告警先查闸门本身是否算错**。
**分层 + 解锁**:vendor-specific(claude-JSON 解析/prompt/ledger)隔离 cmd/forge、通用层(orchestrator/trace/CommandExecutor)经回调(costSink/OnGateResult/Observe)解耦、arch layering 执法;orchestrator.go(拆 `mode_gating.go`)/main.go(prompt 构造移入 `prompt_context.go`)贴 500 闸门已纯提取(byte/hash-identical、fresh-review 过)解锁。
每改动经 fresh-context reviewer 独立审 APPROVE;honesty 贯穿(telemetry 无数据 omit 不伪造 0、reviewer 抓出实现者自评失实记录在案、误撤销 trace fix 后诚实恢复重验)。docs/ignition.md 更新。

## Sprint 27(✅ 完成)— 治理债务清偿(先拆分,再继续)+ REVIEW 段中枢旋钮补线 + 多轮 fresh-review 修 bug
接手时工作树已积累大量未提交在建功能(信号处理/context 传播、`forge detect`、doctor/preflight 诊断命令、Loop Memory/Learning、`internal/yaml2json` 手写解析器重写等),但 `forge accept` 判 **REJECTED**:8 文件超 500 行(`validate.go` 994 行为最)、20 函数超 50 行、`cmd/forge` 包 15 文件超 14 上限——违反 CLAUDE.md「先拆分,再继续」红线。多 agent 并行按包边界拆分(独立包并行、`cmd/forge` 因共享文件数预算改串行):`validate.go`→新 `internal/doctor` 包(诊断逻辑出 CLI 层,同 `internal/migrate`/`internal/mode` 先例)· `internal/memory`/`internal/yaml2json` 按自然缝拆多文件 · `prompt_context/memory/verdict.go` 三文件合并重分布(15→14 文件,回归预算)· `main.go`/`evolve.go`/`preflight.go`/`scorecard_wind.go` 抽 helper 消化超长函数。全绿坐实:`go build/vet/test -race`、`gate.mjs` PASS、`arch-check.mjs` 8/8 PASS、**`forge accept: ACCEPTED`**。

**REVIEW 段中枢旋钮补线**:审出 `.agent/policies/modes.yml`/`design.yml`/`review.yml`(新脊柱段 Discover→Design→★REVIEW★→Build→Evolve,四维深度评审:security/distributed/performance/CTO)已声明 `workflow_depth.review`(skip/standard/full),但 `forge-core/internal/mode` 未建模——「声明但未实现」缺口,同 Sprint 15 discover/design/adr 先例补齐:`Policy.ReviewDepth` 字段 + `ReviewSkip/Standard/Full` 常量 + baseline 表(严格核对 modes.yml 四行取值)+ production lifecycle floor(`reviewFloor`)+ `deeperReview` + `ReviewSkipped()` + `clone()` 补漏;`orchestrator/mode_gating.go` 加 `reviewStageSkipped`(镜像 `discoverStageSkipped`)接入 `RunFrom`(串行)与 `RunParallel`(并行)两条路径;`main.go` run-narration 补 `review=%s`。

**多轮 fresh-context review 揪出真 bug(遵 AGENTS.md「reviewer 必须 fresh-context 独立 agent」纪律,不自审)**:7 agent 独立审拆分后代码,坐实 **2 个 blocking + 8 个 important** 真缺陷(均已修 + 补回归测试):
- **★yaml2json block-scalar 损坏(blocking)★**——`consumeBlockScalar` 把整行 `"key: >"` 连同缩进指示符拼进解码值,导致**每个真实 workflow 文件**的 `description:`/`note:` 字段被注入字面量 `"> "`/`"| "` 前缀直送 agent prompt(`prompt_context.go` 逐字注入);差分安全网测试(`TestToJSON_MatchesPythonShim`)本应能抓到,但只调 `t.Logf` 从不 `t.Errorf`——**测试本身失效,6/7 真文件早已跑偏却全绿**(第二个 blocking)。重写 `normalize.go`:干净拆分 key/indicator/chomp、正确折叠规则(更深缩进行/空行保留字面换行,而非一律折成空格)、行号跟随块消耗行数推进、block scalar 值绕过标量强制转换(纯字符串,不因形似数字/null 被误转);差分测试改真断言,7/7 真文件对 PyYAML 逐位吻合。
- **ReviewDepth 生产覆盖存在旁路**——`skipByMode` 的 `optional_for` 分支只查裸 mode 名(如 `"balanced"`),不查 lifecycle 拉高后的实际深度,导致 `balanced+production` 本应强制全四维评审,却仍因 `review.yml` 的 `performance-reliability-review: optional_for:[balanced]` 被静默跳过——违反「production 一票否决,松散 mode 永不能松动」的既定安全承诺(discover.yml 同款 pre-existing 旁路一并修)。加 `stageDepthAtMax` 按 stage 查对应深度维度,production 覆盖现真正压过 `optional_for`(新回归测试坐实)。
- `internal/memory` `summarizeBlock` 用同一 map 键同时记单 topic 计数与总计,Topic 撞 Kind 字符串时静默漏记/重复计;`Compact` 对负 `keepPerKind` 无夹紧(越界 panic,`Prune` 早有夹紧但未同步)。
- `pi-batch.py`(独立批处理脚本,零测试覆盖)超时机制对 stdout/stderr 两个 reader 线程分别给满额 timeout 预算,实际杀进程延迟可达 ~2× 配置值(命中脚本自身目标场景:详细 stdout+安静 stderr 的流式 CLI);`FileNotFoundError` 一律误报「pi not found in PATH」,不区分二进制缺失与 `cwd` 不存在。
- `internal/doctor`(新拆包)零测试文件,`forge validate`/`--models` 的 agent 引用校验因 JSON 行扫描依赖 pretty-print 格式(实际全链路只产生紧凑 JSON)**完全静默失效**——已改走 `encoding/json.Unmarshal` 结构化解析 + 补 6 个测试文件覆盖 `internal/doctor` 全部导出函数。
- `forge scorecard rebuild`(灾难恢复路径)按 phase 名子串匹配 agent 角色推 task_type,对 `evolve.yml`(phase 名≠agent 名,如 `implement`→`implementer`)全部推空,静默丢弃 evolve 循环的真实 trace 归因——改为优先读真实 workflow 定义建真值映射,子串启发式降级兜底。
- `FreshContext` 车道抑制(AGENTS.md 明文红线:fresh-context reviewer 绝不可见前序输出/gate 裁决/评审意见)此前零回归测试;`usage()` 文本与实际 CLI 行为漂移(`preflight` 位置参数、`route --scorecard`、`approve` 缺失)。

**cmd/forge 包文件数预算二次告警**:并行修 bug 时两个 agent 各自为压 500 行拆出新文件(`validate_agents.go`/`scorecard_rebuild.go`),`cmd/forge` 反弹至 16 文件超 14 上限——再派专项 agent 做架构级消解(非临时合并):`validate_agents.go` 逻辑并入 `internal/doctor`;`scorecard_rebuild.go` 的纯逻辑(`agentTaskType`/`taskTypeForAgent`/`ScorecardPair`/`PhaseTaskTypes`/`ExtractRebuildPairs`)抽成新 `internal/attribution` 包(零 cmd/forge 依赖),CLI 胶水缩回 `scorecard_wind.go`,两文件净删,14 文件达标。

**结果**:`go build/vet/test -race`(18 Go 包全绿)· `gofmt -l` 干净 · `gate.mjs` PASS(360 文件)· `arch-check.mjs` 8/8 PASS(184 源文件)· **`forge accept: ACCEPTED`**(6 PASS · 0 FAIL · 5 诚实 N/A)。honesty:审出的 minor/nit 级发现(如裸 `-` 序列项静默丢弃——纯 YAML 语义缺口、本仓零命中;`review.yml` 一处装饰性但已失效的 `required_when` 注解)诚实记录未处理,不夸大为「全部修复」。

## Sprint 28(✅ 完成)— REVIEW 段收敛信号闭环:`review_status` 从「声明字段」到「真信号」
Sprint 27 把 `ReviewDepth` 接进中枢旋钮(mode-gating 决定哪些评审相位跑/跳),但**收敛信号本身**是断的:`internal/converge` 早已声明 `Signals.ReviewStatus` + `evalReviewStatus`(`review_status == approved` 才 MET),`review.yml` 的 stop_condition 也已声明这条判据——但全仓无一处真正**赋值** `ReviewStatus`,`gatherSignals` 建 `Signals{}` 时压根不提它,导致 `forge run review` 的收敛判定**永远卡在 `review_status= (no review phase data)`,即便真 agent 真批准了也无法 MET**。live 坐实:`forge run review --executor dry --mode engineering` 输出确认此症状。

对照已跑通的先例——`build.yml` reviewer 相位的 `VERDICT: APPROVE`/`VERDICT: REQUEST_CHANGES` 机读契约(`.agent/agents/reviewer.md` 声明 + `cost.go` 的 `parseReviewerVerdict` + `prompt_context.go` 的 `observeFor` 落 `verdictLedger` + `orchestrator.Engine.AgentVerdict` 拉取驱动定向 loop-back,全链路真实可用)——发现 `.agent/agents/cto.md` 从未为 review.yml 新增的 `executive-review` 相位(五择一裁决:Approve / Approve with Simplification / Redesign / Delay / Reject,ADR-0004 已声明「机读裁决」设计意图但未实现)补机读契约,自然也没有解析器和信号赋值。补齐全链路:
- `cto.md` 加 `## Review 阶段 · executive-review 相位` 段(设计段既有职责不动,新增第二职责),定 5 个 UPPER_SNAKE 机读 token(`VERDICT: APPROVE` / `APPROVE_WITH_SIMPLIFICATION` / `REDESIGN` / `DELAY` / `REJECT`)。
- `cost.go` 新增 `parseExecutiveVerdict`(镜像 `parseReviewerVerdict` 的 `unwrapClaudeResult`+末行精确匹配写法)。
- `observeFor` 先试二元 reviewer 契约、失败再退到五择一 executive 契约,两者落同一个 `verdictLedger`(不建平行结构)——不改动既有 build 段 reviewer 行为。
- `gates.go` 新增 `reviewStatus(verdicts)`(APPROVE/APPROVE_WITH_SIMPLIFICATION → `"approved"`,其余原样透出使 `evalReviewStatus` 的 detail 有意义而非空白),`gatherSignals`/`reportConvergence`/`execEngine`/`evolve.go` 的 `buildLoop` 逐层穿针引线(`verdicts` 早已在 `execEngine` 作用域内,只是从未传给 `reportConvergence`——纯缺线,非重新设计)。

**live 端到端双向坐实**(独立于实现 agent 复现,不只信自评):`forge run review --executor command --agent-cmd <fake-agent 末行吐 VERDICT: APPROVE>` → `convergence: MET`、`review_status=approved`;换 `VERDICT: REDESIGN` → `convergence: NOT MET`、`review_status=redesign`(证明信号真随 agent 输出变化,非硬编码;且 REDESIGN 不误触 reviewer 二元契约的 loop-back,两套契约互不干扰)。`go test -race`/`gate.mjs`/`arch-check.mjs`(8/8)/`check.py`(9 检查,cto.md 新增段未破坏 `check_workflow_agent_refs`)/`forge accept: ACCEPTED` 全绿。

honesty:`requirement_confidence`(discover 段的姊妹信号,同样声明但未赋值)诊断中一并发现,本轮不在 REVIEW 收敛链路范围内,诚实记录为后续同类缺口,未顺手改动——**Sprint 29 补齐**。

## Sprint 29(✅ 完成)— 系统性审计 `converge.Signals` 全字段 + 补齐剩余两个断信号 + 架构自纠
Sprint 28 只顺手修了 `ReviewStatus` 一个断信号,遗留「同款缺口是否还有」的疑问。本轮**主动**(非等下次任务顺带撞见)通读 `Signals` struct 全部 8 个字段 + `evalOne` 全部 metric 分支,逐一核对「声明 → 消费者 → 赋值处」三点是否闭环,而非只修被动撞见的那个:

| 字段 | 消费者 | 赋值处(本轮前) | 结论 |
|---|---|---|---|
| RoadmapCompletion / GatesGreen / GateProof / Criteria / HumanApproved / CodeTestRatio | `evalRoadmap`/`gates_status`/`greenDetail`/`evalCriterion`/human_gate/warning | `gatherSignals` 全已赋值 | 闭环,不动 |
| ReviewStatus | `evalReviewStatus` | Sprint 28 已修 | 闭环 |
| **RequirementConfidence** | `evalRequirementConfidence`(discover.yml `requirement_confidence >= 80`) | **从未赋值,永远 0 → 永远 unmet** | **断信号①** |
| **FileDelta** | `orchestrator/loop.go` 的诚实性交叉验证告警(`roadmap>50% 且 FileDelta<30%` → 警告"agent 自报可能夸大") | **从未赋值,恒为 0 → `0<0.3` 恒真 → 该告警在任何 roadmap>50% 的 `forge evolve` 都会误报**,即便文件改动完全对得上 roadmap 声明 | **断信号②,且是活跃假阳性 bug,非仅"未实现"** |

**断信号① RequirementConfidence**:discover.yml 原有一条「诚实边界」注释,称 v1 故意让它恒 unmet、真评估「需真 agent」——但这条注释写在 Sprint 28 的 `VERDICT:` 机读契约模式确立之前;`product-manager.md` 早有「confidence ≥ 80% 才过」的散文描述,只是从未定成机读格式。判断:这不是「设计上刻意留白」,是「Sprint 28 模式确立前的历史遗留」,同款模式理应推广。补齐:`product-manager.md` 加机读契约(末行 `CONFIDENCE: <0-100>`)、`cost.go` 加 `parseConfidenceScore`(第三级 fallback,接在二元 reviewer / 五择一 executive 契约之后,同一个 `verdictLedger`)、`gates.go` 的 `requirementConfidence(verdicts)` 归一化、`gatherSignals` 接线。

**断信号② FileDelta**:与①不同,这条**不需要新设计机读契约**——声明本就是「从 git diff 机械算出」(同 `computeCodeTestRatio` 的既有写法),不是 agent 自报。`computeFileDelta`:读 ROADMAP.md 的**已勾选**(`- [x]`)项(诚实性问题只对"声称做完"的项有意义,未勾选项没有可核对的声明)、`git diff --name-only HEAD` 取改动路径、逐项关键词子串匹配(同 `internal/risk.FromChangedPaths` 的「廉价代理,非证明」诚实定位)、算匹配比例。`gatherSignals` 接线。

**live/测试双证**(独立复现,不只信实现 agent 自评):`forge run discover --executor command --agent-cmd <fake-agent 末行吐 CONFIDENCE: 85>` → `convergence: MET`、`requirement_confidence=85`;换 `CONFIDENCE: 50` → `NOT MET`。FileDelta 走单测路线(真 git fixture 验证 0/全匹配/部分匹配/零匹配四态 + `LoopEngine.reportConvergence` 的告警在 FileDelta 高时不误报、低时才报)。

**架构自纠**:补线过程中 `gates.go` 顶到 500 行,先拆的 agent 把「N/A 豁免矩阵 + 逐 gate 裁决」纯逻辑（`gatesGreen`/`resolveGate`/`exemptNA` 等,只 import `internal/gate`+`internal/converge`,零 CLI 关切)切成 `cmd/forge/gate_resolve.go` 新文件,顶破 `cmd/forge` 包文件数上限后**径直把 `.arch/rules.yaml` 的 `package.max_files` 从 14 抬到 18** 了事——复查判定这是抄近路,不是本仓「先拆分」纪律的正确应用:这类纯逻辑该像本 sprint 之前的 `internal/doctor`/`internal/attribution` 一样**流入既有的 `internal/gate` 包**,而非留在 `cmd/forge` 硬撑预算。改派专项 agent 纠正:逻辑迁入 `internal/gate/resolve.go`(导出 `GatesGreen`/`ResolveGate`/`HarnessRunner` 三个真被外部调用的符号,其余保持不导出)、`gate_resolve.go` 删除、`cmd/forge` 包文件数回落到 15(14 原始基线 + 确实是新增 CLI 面的 `approve.go`,合理)、`package.max_files` 从 18 **回调到 16**(15 实测 + 1 headroom,对齐本文件既有的「实测 + headroom」惯例,而非放任虚高)。

**结果**:`go build/vet/test -race` 全绿 · `gofmt -l` 干净 · `gate.mjs` PASS(366 文件)· `arch-check.mjs` 8/8 PASS(190 源文件,`cmd/forge` 15 文件 ≤ 16)· `check.py` PASS(9 检查)· **`forge accept: ACCEPTED`**。honesty:本轮系统扫过 `Signals` 全字段与其全部消费者,`converge.Signals` 目前无已知断信号;这是**对当前代码库这一具体审计范围的陈述**,不是「全仓功能需求已穷尽」的断言——ForgeOS 没有独立的功能需求清单可供逐条勾核,`stop_condition` 的权威定义仍是 ROADMAP.md 末尾那句「roadmap 完成度 / 闸门全绿」。

## Sprint 30(✅ 完成)— `docs/FUNCTIONAL_REQUIREMENTS_AUDIT.md`:把「无需求清单」的结构性缺口本身补上
Sprint 29 结尾诚实承认:「ForgeOS 没有独立功能需求清单可供逐条勾核」。本轮直接把这句话变成可核实的产出——从项目**自己的**权威源头(根 `ROADMAP.md` + `.agent/{ROADMAP,PROJECT,ARCHITECTURE}.md` + 4 篇 ADR + `.agent/DECISIONS.md` + 全部 5 个 `.agent/workflows/*.yml` 逐字段 + 全部 12 个 agent 卡/9 个 skill 卡的机读契约 + `CURRENT_SPRINT.md` 29 个 sprint 里每一处「诚实标注/仍待/未处理」的微承认)**推导出**一份显式需求清单,而非凭空发明外部规格。5 路独立 agent 并行通读各自源头、逐条核对「声明→消费者→实现」是否闭环,合并去重、交叉验证分歧后产出 `docs/FUNCTIONAL_REQUIREMENTS_AUDIT.md`:四桶分类(DONE / BLOCKED-EXTERNAL 需外部资源 / DEFERRED-BY-DESIGN 项目自己文字承诺推迟 / GAP 无文字借口的真缺口),**DONE 约 90 条、BLOCKED-EXTERNAL 3 条(Firecracker/LiteLLM/SCA-DB,均已是诚实框架+N/A 降级,非空白)、DEFERRED-BY-DESIGN 约 15 条(每条引证项目自己白纸黑字的推迟声明,不接受自造借口)、GAP 14 条**。

**GAP 逐条收口**(同日全部处理,非留待未来):
- **`requires_tools` degrade-and-flag 机制**(discover.yml 声明「无检索工具则降级 advisory 并打标」但零代码)——`asset.Phase` 补 `RequiresTools` 字段,`requiresToolsGuard`(dry-run / 非 claude / 无 allowlist / 工具未确认 → 诚实降级 + 提示 agent 标注未核实内容;确认可用则静默放行)接入真实 `agentExecutor` 构建路径,单测+端到端测试坐实。
- **`readonly` 声明但零执行**(每个 workflow 每个 phase 都标了,reviewer 阶段写权限却和 implementer 完全一样)——解码 + 叙述(每次 readonly phase 起跑打印其只读边界 + 允许写的 emits 清单)已落地;**技术强制**经调查后**主动不做**:`docs/ignition.md` 记录的真机坐实事实证明去掉 `acceptEdits` 会让 headless `claude -p` 只描述不落地,连该 phase 自己该写的 `emits:` 产物都会被一并挡住——正是任务书本身警告的「naive deny-all 打断 emits」陷阱;更精细的按路径授权需要真 claude CLI 验证语法,本轮无预算授权真跑,诚实记录为未决缺口而非悄悄放弃。
- **`secondary_template`(review.yml 性能评审阶段的第二模板)零消费**——补齐,镜像既有 `uses_template` 全部消费点(prompt 注入拆到新 `prompt_artifacts.go` + `doctor.EvaluateWorkflowModels` 校验);live 坐实:`forge validate --models` 现对其产出 PASS 行,与 `uses_template` 对称。
- **`stop_condition.on_rejected` 死代码**——追踪全部真实调用路径证实:`forge evolve` 在进 LoopEngine 前就拒绝 human_gate workflow、`forge run` 从不循环、review.yml 的 conjunction 型 stop 也过不了 `IsHumanGate` 守卫——三条路径都到不了这段代码。判定为「机制本身正确,但当前单趟 CLI 架构没有能触发它的多阶段迭代能力」,加诚实注释说明,零行为改动。
- **yaml2json 裸 `-` 序列项丢失**(Sprint 27 已知但未修的遗留)——`parseSeqItem` 空分支补齐为与其余分支对称的无条件 append,修复 + 测试,对本仓全部 7 个真实 YAML 文件零影响(无一命中此模式)。
- **4 处「声明但被另一套机制取代」的死字段**(`mode_gating:` 顶层块 / `blocking:` / `confidence_metric:` / review.yml 的 `required_when`)——判定重新接线属于给已跑通的机制生造平行实现,收益不值风险;逐处加一行 `NOTE:` 注释诚实标注,不动字段本身(仍留作人读交叉引用)。
- **ADR-0004「balanced 只跑 P1+P2」与代码不符**(实际 P1+P2+P4 都跑,只有 P3 可跳)——ADR 加 `[corrected 2026-07-02: ...]` 勘误,引证 `TestRun_BalancedSkipsOptionalReviewPhase`,历史 Decision 原文不动。
- **ADR-0002 的 `forge-ai`(Python 智能层)缺推迟措辞**(Rust 有明确「v3」标注,Python 没有却同样零代码零目录)——补对称的推迟措辞。
- **G3 多维模型路由(complexity/dependency/context/business-impact)不驱动真实执行,只喂手动 `forge route` CLI**——复核后**改判**:`internal/routing` 包自己的文档已明说 `TierFor` 「非完整多维评分器(那是 v2+ Router service)」,是已有的自我推迟声明,构建真正的自动多维评分是一个独立大特性、非接线小修,本轮不强推,归入 DEFERRED-BY-DESIGN 而非「今日可关」的 GAP。

**架构自纠(再一次)**:`secondary_template` 的实现把 `prompt_context.go` 顶过 500 行,合理拆出 `prompt_artifacts.go`(镜像 `prompt_memory.go` 先例),`cmd/forge` 文件数 15→16,顶满上一轮刚定的 `package.max_files:16` 零余量——本轮直接把注释更新到「实测 16 + headroom 1 = 17」,不留虚假余量继续下一次自然拆分。

**结果**:`go build/vet/test -race` 全绿 · `gate.mjs` PASS(370 文件)· `arch-check.mjs` 8/8 PASS(193 源文件)· `check.py` PASS(9 检查)· **`forge accept: ACCEPTED`**。honesty:`docs/FUNCTIONAL_REQUIREMENTS_AUDIT.md` 是**对本仓自己声明源头的审计**,不是外部强加的规格书;它明确排除了纯计数漂移(agent 卡数/skill 卡数/Go 包数等,清单低估了已经长大的能力,非功能缺失,附注留给下次维护 ROADMAP.md 的人),避免把「文档数字过时」误记成「功能缺失」。

## Sprint 31(✅ 完成)— GAP 二轮复审:把「文档标注」升级成「真实现」,只留有理有据的例外
Sprint 30 结尾把 14 个 GAP 逐条收口,但 5 处收口方式是加一行 `NOTE:` 注释而非真接线。复审判定:这 5 处里有 4 处其实**低风险、有真实增益**,当初判"不值得接线"是判断过严;只有 1 处(`blocking:` 字段)复审后确认**没有任何值得实现的行为差异**(全仓无一处声明 `blocking: false`,接线等于为一个从未被使用的取值发明新行为——真正的镀金)。逐条动手:

- **`readonly` 技术强制此前卡在**"验证路径限定 Write/Edit 语法需要真 claude 付费跑、本 session 无授权"——发现这个前提是假的:用 `claude-code-guide` 专项 agent 查官方文档(`code.claude.com/docs/en/permissions.md`,免费本地操作,`--help`/文档抓取不产生真实 API 调用成本,与"真跑一次完整 agent 任务"完全是两回事)权威确认了 gitignore 式路径限定语法(`Edit(/docs/review/**)`,deny 先于 allow)。按此实现 `claudeArgv`:readonly phase 拿 `--disallowedTools "Edit Write"` + 按 agent 卡自己写明的产出目录(`docs/discovery/` / `docs/design/` / `docs/review/` / 声明 `writes_adr` 时加 `docs/adr/`)重开 `--allowedTools`——目录来自 agent 卡边界段原文,非发明。20+ 单测坐实 argv 逐位正确;诚实标注:**按文档契约构造正确、单测坐实,未过真实 claude 进程验证运行时行为**(仍未获真跑预算),不夸大为"已验证"。
- **`stop_condition.on_rejected` 死代码**——镜像既有 `.forge/<stage>.approved` 签核标记模式,新增 `.forge/<stage>.rejected` 标记:`forge run` 起跑前若探到该标记 + human_gate stop + `on_rejected.action==loop_back`,解出 target_phase 索引、从那里起跑(而非 phase 0)、**消费**(删除)标记,保证一次性触发。独立复现坐实(不只信实现 agent 自评):建二进制、写标记、精确匹配叙述行 `human_gate REJECTED (marker consumed)` 恰好命中 1 次,标记确认消失;第三次跑(标记已耗尽)恰好命中 0 次、退回默认 phase 0——真正一次性、向后兼容零回归。
- **`confidence_metric:` 字段驱动**——`requirementConfidence` 从硬编码查 `"requirement-discovery"` 改为扫 `wf.Phases` 找哪个 phase 声明了匹配的 `ConfidenceMetric`,找不到才退回硬编码名(对 discover.yml 现状逐位不变,新增测试证明改名后的 phase 也能被正确拾取)——`gatherSignals` 早已有 `wf` 在作用域,零新增管线。
- **`mode_gating:` 顶层块**——没有重新接线出一套平行执行机制(会重复已跑通的 `internal/mode`),而是加一道**漂移守卫**:`harness/check.py` 新 `check_workflow_mode_gating`(逻辑量拆到 `harness/mode_gating_check.py` 保体积)逐 workflow 解出 `mode_gating:` 声明值,对照 `authority:` 指向的 `modes.yml` canonical 值,不一致就报——这是这仓库自己的既有模式(`check_modes_router_tiers`/`check_mode_priorities` 同款),独立验证对本仓当前 5 个 workflow **全部一致、零漂移**,不是"扫过就算"。
- **review.yml 的装饰性 `required_when`**——复审判定:与其为一个从未生效的字段造假消费者,不如诚实删掉这处误导性声明本身(它暗示了一套实际不存在的 per-phase 门控)。已删,确认无测试依赖其存在、YAML 仍可解析、`check.py` 仍过。
- **`blocking: true`**——唯一维持"仅文档标注"的一项,明确给出理由而非默认懒惰:grep 全仓确认没有任何 workflow 声明过 `blocking: false`,该字段唯一的"未接线行为"(红灯不阻断)从未被任何真实场景需要过;实现它等于凭空发明新行为,判定为镀金,不做。

**架构自纠(第三次)**:`mode_gating` 漂移守卫新增两个 harness 文件后忘了同步 `forge-init` 的 `COPIED_FILES` 清单,`forge-init` 的 copy-anywhere 完整性自测当场抓到("每个 harness 源文件必须被复制或在白名单")——这正是该自测存在的意义:立即补两行清单项 + 为不顶 500 行把两处补充说明从多行注释压成单行,复检 `forge-accept: ACCEPTED` 恢复。

**结果**:`go build/vet/test -race` 全绿 · `gate.mjs` PASS(374 文件)· `arch-check.mjs` 8/8 PASS(195 源文件)· `check.py` PASS(10 检查,新增 mode_gating 漂移守卫)· **`forge accept: ACCEPTED`**。`docs/FUNCTIONAL_REQUIREMENTS_AUDIT.md` 的 Resolution addendum 同步改写:5 处从"DOCUMENTED"改判"RESOLVED"(各附二轮复审的具体理由),1 处("blocking")明确保留为**经过论证的例外**而非默认懒惰。honesty:两个仍标注为「需真 claude 验证」的机制(readonly 强制的真实运行时行为、on_rejected 在真实多相位评审下的表现)诚实移入下一前沿,不假称已用真 agent 坐实。

**人工决策收尾(2026-07-03)**:两处「需真 claude 验证」的机制(readonly 路径限定强制、on_rejected 拒绝重跑)是否值得为验证而花真实 API 预算,征询用户——用户明确选择「就此打住,单测已足够」(而非授权花钱真跑)。这是本仓「花真钱需用户显式授权」既有纪律(Sprint 24-26 皆如此)下的正常终止路径,不是回避:两个机制本身**已经真实实现**(非声明未接线),只是运行时行为的最后一道经验证据止步于「按官方文档契约构造 + 单测坐实参数正确性」,未再往「真 claude 进程实测」推进——用户对此知情并明确接受为最终状态。

## Sprint 32(✅ 完成)— BLOCKED-EXTERNAL 复查:环境实测证明 SCA/CVE 的外部资源已可得,收口成真实 PASS
`/goal` 重新发起后,先复查根 ROADMAP/`docs/FUNCTIONAL_REQUIREMENTS_AUDIT.md` 的三个 BLOCKED-EXTERNAL 项是否仍然真的 blocked,而非默认沿用旧结论——**实测(非假设)**:`/dev/kvm` 本环境存在且当前用户可读写(但 `firecracker` 二进制未装,搭建 microVM runner 是架构级工作,维持 blocked)、无任何跨厂商 LLM key(维持 blocked)、**`api.osv.dev` 真实网络可达**(`curl` 直接验证 200)。第三项此前唯一的阻塞理由就是「缺 DB」,而 DB 恰恰就是这次实测证明可得的外部资源——收口。

**实现**:新增 `harness/sca_fetch.mjs`(人工/周期性刷新工具,复用 `sca.mjs` 已有的 `discoverManifests`/`parseManifest`/`compareVersions`,不重新发明 semver 比较):对本仓 4 个 manifest 解出的唯一真实依赖(`harness/requirements.txt` 的 `PyYAML>=6.0,<7.0`,forge-core/go-taskd 的 go.mod 及 harness 的 package.json 均零依赖)向 OSV API 查询**完整历史**(不按当前 pin 过滤版本,以便未来版本升级落入已知漏洞区间时仍能命中),转写为 `sca.mjs` 既有的简化 schema 并写盘 `.agent/security/advisories.json`。**诚实边界(设计即声明,非事后找补)**:该刷新工具**从不**被 `forge accept`/gate 路径调用——harness 的 gate 链路必须保持零网络、确定性、可离线跑;这是运维者按需手动/定期跑的工具,同 vendoring lockfile 的姿势,`sca.mjs` 本身继续只读盘上快照。

**去重正确性**(独立复核抓出的真 bug,同日修复):OSV 对同一漏洞常见"GHSA 原生记录 + PYSEC 别名记录"两份,且两份自己的 `introduced` 边界可能有细微出入(如 `5.1b7` vs `5.1`)。初版按 (package,ecosystem,introduced,fixed) 做去重键——两份记录只要边界不完全一致就被误判成"不同漏洞",产出重复且矛盾的 DB 行。改按**规范 id**(优先 GHSA-* 别名)去重,合并时取**最保守窗口**(`min(introduced)` + `max(fixed)`,`fixed` 缺失/开放式永远压过任何具体已修复版本)——复用 `sca.mjs` 已测试过的 `compareVersions`,不重新实现 semver 排序。4 条真实 PyYAML 历史漏洞(CVE-2020-1747/CVE-2020-14343 等)全部已在 5.4 之前修复,本仓 pin 在 6.0,故 `dependency_vulnerabilities` 判定 **PASS**(0 known-vulnerable vs 4 条真实 advisory)。

**copy-anywhere 双重坐实**(两处独立自测各抓到一次真回归,同日修复,不是事后合理化):① `sca_fetch.mjs` 遗漏 `forge-init` 的 `COPIED_FILES` 清单,被 `test_forge-init.mjs` 的清单完整性守卫当场抓到(同 Sprint 31 的先例)——补一行清单项;`forge-init.mjs` 因此顶破 500 行上限,把三份纯数据数组(`GOVERNANCE_DIRS`/`COPIED_FILES`/`HARNESS_NOT_COPIED`)拆到新 `harness/scaffold/copy-manifest.mjs`(`export`+局部 re-export,外部 import 路径不变)。② `test_acceptance.mjs`(它自己就是 `COPIED_FILES` 之一)最初把 `probeSCA()` 的断言硬编码成"本仓=PASS",这正是 Sprint 14 已经踩过一次的"copy-anywhere regression"同款错误——一个**没有** `.agent/security/advisories.json` 的新脚手架项目跑这份被复制的测试会得到 N/A 而非 PASS,断言会假失败。改为运行时探测 `existsSync(ROOT + '.agent/security/advisories.json') || FORGE_SCA_DB`,按探测结果分支断言 PASS 或 N/A——两条路径都真实可达且都被验证过(本仓 = PASS 分支;推演脚手架 = N/A 分支,由 `test_forge-upgrade.mjs` 的端到端脚手架-then-accept 集成测试间接坐实)。

**结果**:`go build/vet/test -race` 全绿(forge-core 18 包)· `gate.mjs` PASS · `arch-check.mjs` 8/8 PASS · `node --test`(harness 246 + arch 34 + scaffold 11)全绿 · python 43 测试全绿 · **`forge accept: ACCEPTED`**(`dependency_vulnerabilities` 从 N/A 转 PASS,`0 known-vulnerable dependencies (4 manifest(s), 1 dep(s) vs OSV advisory DB)`)。`docs/FUNCTIONAL_REQUIREMENTS_AUDIT.md` 同步:该行从 BLOCKED-EXTERNAL 划去、移入 DONE(附证据),Firecracker/LiteLLM 两行标注 2026-07-16 复查未变。honesty:仅覆盖本仓当前实际存在的依赖生态(唯一真实依赖是 PyPI 一条;Go/npm 生态零真实依赖,该框架对它们的正确性靠既有 `test_sca.mjs` 的 fixture 覆盖,未经真实数据验证)——不夸大为"全依赖树已扫描"。

## Sprint 33(✅ 完成)— 参考 Pi 从零建立 Rust Agent Runtime 离线首切片

用户明确要求参考 Pi Coding Agent 从零构建。核对当前官方 `earendil-works/pi` v0.82.1(MIT)后,仅借鉴 provider/loop 分离、事件流、核心与 UI 分离、deterministic provider 测试四个边界,不复制 TypeScript 源码/提示词/UI/品牌。新增 `forge-runtime/` 四层 Cargo workspace:`domain` 定义消息/事件/provider/tool 端口,`application` 独占单一 Agent Loop,`infrastructure` 提供 JSONL sink、严格只读 workspace 工具与假 provider,`interfaces` 只做 CLI 接线。

首个真实离线 demo 完成两轮模型流 + 一次 `read_file` 工具调用,输出 13 条严格递增 JSONL 事件并正常终止；默认零网络、零真实 LLM 费用、零写/Shell 权限。测试覆盖工具往返、能力拒绝、turn 上限、截断工具参数禁止执行、provider 协议失败单终止事件、`..`/symlink workspace 逃逸与 JSONL framing。诚实边界(截至本 Sprint):这只是 runtime proof；真实 provider、SQLite 恢复、steer/follow-up、审批、写/进程工具、TS UI 和 OS 沙箱均未实现。Sprint 34 后已有独立的本地 Conversation/Prompt SQLite ledger，但仍未接入 Agent Run 恢复。设计与分期见 ADR 0006 + `docs/design/forge-runtime-mvp.md`。

## Sprint 34(✅ 完成)— Local-first Conversation Hub:Global / Project / Group + Prompt ledger

用户把 CLI 入口明确为「不加路径进入全局会话空间；加路径进入项目会话；多个 frontend/backend/SSO 项目可联动成组讨论；长期保留 Prompt；未来账号绑定后可看远程会话」。按 ADR 0007 将 Space / Conversation / Run / AuthSession 拆开，先完成不依赖服务器且不伪装远程能力的本地基础。

`forge-runtime` 现支持无路径 Global、裸路径或 `-C` Project、`--group` Group；`session new/list`、`prompt add/list`、`group create/add/list` 均可脚本化并提供版本化 JSON。Rust 四层依赖保持向内，SQLite v1 持久化 Project/Conversation/Prompt/Group/带描述性角色的 Project link；写操作事务提交后才报成功，`group add` 的 Project 注册与 link 同事务回滚。`--idempotency-key` 支持跨进程复用，同 key 不同 payload 会冲突；缺失实体/未知 schema 失败关闭。Global Prompt 查询跨全部本地 Conversation；Group 可拥有讨论 Conversation，frontend/backend/sso 只是组织标签，不是 ACL 或 Agent 能力。

安全与诚实边界：Prompt/路径是明文；Unix 新建或空的专用 state 目录收紧为 `0700`，DB/WAL/SHM 为 `0600`，symlink 拒绝；已有内容且对组/其他用户开放的目录拒绝且不 chmod。`prompt add SESSION_ID -` 可避开 argv/shell-history 正文，add 回执不回显正文，但显式 `prompt list` 仍输出明文。当前 Prompt 必须显式添加，deterministic demo 不暗中写入；自动 Agent 历史回放、Run/事件恢复、远程账号/OIDC/同步、共享 ACL、多 Agent 组执行、真实 provider 与沙箱仍未实现。实现与契约见 `docs/design/conversation-hub-phase1.md`；两位独立最终复审均 APPROVED，Rust 76 tests 与 fmt/clippy/check/build、整仓 gate/arch 均通过。

## Sprint 35(✅ 完成)— P0/P1 契约、链状态、生产交付与验收闭环

本轮把已采纳需求计划中仍可在本环境交付的点逐项收口。链式控制面现用版本化 state 恢复 Discover→Design→Review→Build→Deploy→Evolve，已完成阶段不重放；拒绝回修、审批等待、CTO halt、cycle/max-stage、共享 call/美元预算和 resume 参数一致性均失败关闭。所有 `--chain` entry 初载及 waiting chain 的整条历史路径重建均只用 native loader，显式恢复参数冲突在重建前拒绝，仓库 Python YAML shim 不会在锁前初载或恢复重建时先行执行。普通 Evolve checkpoint 升为严格 v2：每个恢复必填字段必须显式存在且非 null，可选 phase/spend 标量一旦出现也不得为 null；同时绑定完整 normalized workflow digest、mode 与 resolved lifecycle。无/旧 `_format` 状态仍可诊断读取但不可 resume，缺字段、负 iteration/phase/spend、越界 roadmap/phase、MaxInt 溢出均在 trace/Agent 启动前拒绝。planner `TASK_LIST`、phase emits/ADR、review/CTO/release verdict、artifact provenance、trace 查询、私有状态与版本迁移都有机器契约和反例测试；其中普通 emit 契约是仓内 regular/non-empty + provenance（允许幂等既有内容），ADR/release 才额外要求当前 attempt freshness，普通 reviewer 是已声明的 advisory fail-open、CTO 无有效批准则 Review stage 不收敛、release verdict 则严格 fail-closed。空白/重复 phase 名与单 phase 内经 portable path-clean 后重复的 emit target 由 loader、串行、并行、Waves、输出契约和治理检查统一拒绝；跨 phase 对同一 canonical emit 的显式修订仍允许。`required_gates` 现统一为前置条件：只有 `agent:harness` 是纯闸门，QA 等非 harness agent 过绿后仍会真实执行；Evolve 明确拆成 `implement→harness-gates→review→evaluate`，红灯只回 implement。Evolve 的 iteration depth 与 mutation authority 已分轴并回写 canonical `modes.yml`：production/未知 lifecycle 可加严质量 floor，但绝不把 Explorer/CTO 的 `propose-only` 放宽为 `auto-act`。workflow 用唯一 `effect: mutate` 标出与 Agent 名无关的产品修改边界；其前所有 observe/propose phase 必须 readonly，Claude 只获经 containment/role ceiling 校验的精确 emit `Edit`，不继承 Bash，自定义 command 因无法执行该权限契约而失败关闭。proposal-only prefix 禁止 `required_gates`，LoopEngine 注入纯失败 gate 而不持有宿主 runner，循环信号只读文件/ledger，并跳过 acceptance probe 与 scorecard 更新；`release-engineer` 只可存在于 immutable Deploy/Rollback，在 asset/executor 两层拒绝 Evolve 借用。缺边界、重复边界、未知 effect 或任意非 mutate writer 在执行前拒绝，串行、并行及 resume 共用同一边界。`forge detect` 对 greenfield 现建议可执行的 `forge run discover`，`forge evolve auto` 也按 one-shot run 语义转交，显式 evolve-only flags 则给出明确用法错误。

ADR 0005 的 Deploy/Rollback 仍不做远程动作，但本地边界已从“目录约定”提升为可执行信任契约：immutable workflow；`dontAsk` + 每 phase 精确 `Edit(/emit)`；operator 提供仓库外绝对 Claude 路径与内容 SHA-256；Linux 内部 helper 复制到匿名 executable memfd，加入并复核 write/grow/shrink/seal/exec 密封位后才做最终摘要与 ELF 校验，再只读重开同一 inode 并从开放 FD 执行；既有可写别名与原路径替换均不能改变执行字节，shebang 与其他 binfmt payload 均拒绝，不支持该能力的 kernel/host policy 失败关闭；kernel/ELF loader/shared libraries 是明确 host TCB。source inventory 固定使用仓库外 `/usr/bin/git`，从 `/` 到 binary 的组件必须无 symlink、同一 host owner 且调用者不可写；最小 env 强制关闭 repo fsmonitor/hooks/external excludes/pager，PATH shadow 与恶意 fsmonitor 哨兵均有回归。固定最小 role/phase purpose，不 Gather role card/ROADMAP/ADR/memory/glob；整棵 release tree postflight；当前 attempt 的 fresh emit；单一成功 JSON envelope 与严格双 verdict；validation `REQUEST_CHANGES` 固定报告回流；receipt 绑定 agent/prompt、被审 product 工作树摘要及本 stage 固定 artifact 集。批准/驳回冲突、源码/产物变化或旧 receipt 均失败关闭。pin 证明 operator 指定字节，不证明厂商签名；`actor_hint` 不是身份认证；human reject marker 不含反馈正文。本轮未获新的付费模型预算授权，只跑 native fake-agent/E2E/单测/race。

仓内 `.forge` 控制面同步改为 fail-closed 状态文件系统契约：根状态目录必须是真实 `0700` 目录，所有 checkpoint/chain cursor/trace/memory/provenance/approval/receipt 叶均通过 bounded regular-file、identity 与不可预测临时文件原子发布检查，Unix 另强制 no-follow/single-link；symlink、Unix hard-link、固定 `.tmp` 别名与路径替换反例均有哨兵回归。Evolve checkpoint 的 retain history 不再先 rename 移走 current，而是快照后复制发布历史、最后原子提交 current；history/final 故障注入证明失败后旧 current 仍可 Load，进程崩溃也不会因旋转窗口误判为 fresh start（目录未 fsync，因此不外推为断电持久性承诺）。Linux 上 Git index 一旦跟踪 `.forge` 或 `.forge/**`（含 ASCII case、反斜杠与 clean alias），chain 首次 entry 在读取 cursor 前、resume/approval/status/preflight 及 run/evolve 锁后都会经可信 `/usr/bin/git` 拒绝；`--root` 必须与 canonical Git toplevel 为同一目录，父 worktree 子目录与 symlink/special `.git` 控制路径失败关闭，不能再漏检 `sub/.forge/**`、重复拼接 Git 路径或伪造已完成链段/人工签核。非 Unix 保留目录/类型/静态 symlink/identity/权限/尺寸约束，但不声称 link-count 或对恶意 Git index 有同等级 provenance 保证。release/proposal-only 子进程环境另有独立受限档：不传 host `HOME`/XDG/temp/shell/parent PATH/`CLAUDE_CONFIG_DIR`，固定 `/usr/bin:/bin`，只保留直接认证与 locale/TLS 输入。

验收递归发现 Node/Python harness，并对 Go/Node/Python/Rust/Java 项目要求可观察的正测试数；manifestless、零测试、不可读子树和未配置目标不再空过。项目结果类别是结构字段，路径/输出文字不能伪造 `inapplicable`。init 生成真实 starter manifest/test；upgrade 的 source/target/state/backup/prune 路径对 symlink、special file、portable case/Windows 别名和 inode alias 失败关闭，且 init/upgrade 在首写前预检全部计划叶，晚发现坏目标不再留下部分更新。Rust SQLite 对整个 open→PRAGMA/WAL→schema 序列在 BUSY/LOCKED 下用统一 5 秒期限重试，8×16 并发首次打开、2.3 秒独占锁、DB/WAL/SHM `0600` 与 workspace `workspace_unavailable` 单终态均有回归；并发 opener 的旧权限 metadata 会按当前目录 inode/mode 复核，权限收紧通过目录 FD 完成，路径被替换为 symlink 时失败关闭且不修改链接目标。

验证证据：Go 全包 test/race/vet/build，Darwin/Windows amd64 交叉编译；Node 主 harness 306/306 + arch 38/38 + scaffold 33/33；Python 61/61；Rust workspace 78 tests + fmt/clippy/check/build；`gate` PASS（477 files）、`arch-check` 8/8（340 source files）、`check.py` 11 checks、secret scan 447 files/0 findings、SCA 5 manifests/1 dependency/0 known-vulnerable；完整 `forge accept` 为 **ACCEPTED**（9 pass、0 fail、2 个诚实 N/A；递归 Node 19 files/377 tests、Python 5 files/61 tests、forge-core Go 1021、dogfood Go 22/Node 47 个测试）。Rust 扫描覆盖全部 38 个 production Rust 文件；legacy Go 的四层映射仍是启发式局部覆盖，规则文档不把它夸大为整仓形式化证明。

## Sprint 36(✅ 完成)— Rust durable Project Run + bounded history + opt-in Responses

ADR 0006 的 Agent Loop 与 ADR 0007 的本地 Conversation/Prompt ledger 已接成最小可用的 Project Run。SQLite schema 从 v1 原子迁移到 v2，新增 execution-bound `runs`、append-only `run_events`、增量语义 cursor 与 Run-assistant 关联；Run 必须绑定真实 Project Conversation、既有 user Prompt、provider/model、system Prompt、exact read allowlist 与全部 limits。事件按 `(run_id, seq)` 连续追加，同事件重试幂等、异内容冲突；SQLite 先提交、JSONL 后输出，`tool_started` 在工具效果前持久化。每次 append 通过同事务 cursor 做 O(1) 语义推进，完整 inspection 在同一 SQLite snapshot 内读取 Run/cursor/events/bound Prompt，并从 durable prefix 重建 cursor 比对；journal 仍有事件数、单事件与总字节硬上限。

`run start/list/show` 已跨进程工作。新 Run 最多加载所选 user Prompt 之前 16 条完整 lowercase `user`/`assistant` causal 消息，历史正文总量严格限制为 512 KiB；Run answer 永远锚定原 user，即使晚于后续 user 才 crash-repair，多 Run 同源也会保留 source+最新 bounded answers，损坏关联失败关闭。孤立 assistant 前缀会丢弃，当前 Prompt 只追加一次。journal 严格验证 user/turn/tool/result/terminal 全状态机；`run_finished.completed` 必须等于最后 committed、无 tool call 的 assistant。完成写回由 validated Run 授权，在一个事务内创建 assistant Prompt 与唯一关联，不再依赖可伪造的内部 key 约定。相同 `RunOutcome::Completed` 终态重试在 API key preflight 前完成，不调用 provider/tool，只修复缺失 writeback；`RunOutcome::Failed`、`RunOutcome::Cancelled`、`RunOutcome::LimitExceeded`、incomplete 与 pending-tool 均不进入这条修复路径。

默认路径仍是 deterministic/offline。显式 `--live` 才启用 OpenAI Responses streaming adapter，并要求调用者显式提供 CLI idempotency key 与环境中的 `OPENAI_API_KEY`；API key 不进 argv、Hub 或错误。live 默认零工具/零 WorkspaceRead，repeatable `--allow-read` 只授权 exact relative file。adapter 只接受固定 HTTPS endpoint，禁 redirect 与隐式 retry，校验 SSE content type，并限制 total bytes/frame/buffer/pending call/timeout/token；`store:false` 在 tool turns 间原序回放完整 validated reasoning/function/message output items，保留 encrypted content、function identity/status 与 assistant phase，并用 projection equality 防重复/漂移。streamed message/function item identity 必须与 terminal output 精确一致；`commentary` 只保留在 raw context，`final_answer` 与 legacy null/omitted phase 保持实时 delta。只有 `max_output_tokens` incomplete 映射为正常 length limit，content filter、未知 reason、矛盾 status 均失败关闭，任何 incomplete call 都不可执行。后到终态失败不能撤回已发文本，但不会 commit Assistant 或授权工具。loopback 两 POST 测试覆盖 reasoning→commentary→function call→tool output→final answer，另有 refusal/failed/transport/超限/脱敏反例；本轮没有发起真实付费模型请求。没有写/replace/Shell/process/network 工具或 OS sandbox。

隐私边界已明确：SQLite 依赖私有文件权限但不加密；Prompt/history、Run 配置、model delta/provider context、tool 参数/结果与授权读取的文件正文都可能明文 journal，并被 `prompt list`/`run show` 显式输出。

验证证据：Rust workspace 203 tests，fmt/clippy/check/build 全绿；完整 `forge accept` 为 **ACCEPTED**（9 pass、0 fail、2 个诚实 N/A）。v1→v2 数据保留、顺序/冲突、同快照 inspection、journal backpressure、durable-before-effect、历史边界/角色/UTF-8/byte cap/causal repair、多 anchor/多 Run 同源裁剪、provider 两 POST 完整 output replay、phase/incomplete/status/item identity、跨进程 CLI 与 assistant crash-repair 均有正反例。架构决策与完整契约见 ADR 0008 和 `docs/design/run-journal-phase1.md`。远程账号/同步、共享 ACL、Group 多 Agent 执行、自动 execution resume/branching、derived memory、TypeScript UI 与 mutating sandbox tools 继续分期。

## Sprint 37(✅ 完成)— Rust local Group context dossier

ADR 0007 的 frontend/backend/SSO Group 从“只展示关联关系”推进到原子、只读的跨会话 Prompt dossier。新 `group context GROUP_ID` 在单一 SQLite deferred transaction 中解析当前成员、Group discussion 与成员 Project 的非空 Conversations；Global、其他 Group、非成员 Project、canonical path、文件、Run event、tool/provider context 与 idempotency key 全部排除。role 始终只是 provenance，不转成 ACL、Agent 角色或 capability。

上下文采用固定有界 policy：成员最多 16（超出失败关闭）、Group Conversations 最多 4、每个成员 Project Conversations 最多 2、每 Conversation 最多 8 条 causal Prompt；延迟 assistant writeback 仍锚定 source user。正文 newest-first 跨 Conversation round-robin 分配，单条 excerpt 16 KiB、总量默认 256 KiB/最高 512 KiB，并在 UTF-8 边界截断；source 连一个完整 Unicode scalar 都容纳不下时，同 causal anchor 的 answers 不得绕过它消费预算。payload 记录 source、Prompt ID/role/time、原始字节数、content SHA-256、omission/truncation stats 和带版本域分隔符的 canonical slice SHA-256。CLI 默认只显示 manifest，同时隐藏 excerpt 与逐 Prompt 指纹；`--include-content` 才显示可公开重哈希的有界 payload。Human 输出报告 omission/truncation 并转义终端、行分隔和 bidi 控制符。命令始终本地离线且不打开 workspace。

这仍是 on-demand preview，不假装已完成模型分析或 multi-Agent execution。未来 provider 消费前必须先持久化 exact dossier snapshot 并让幂等 Run 重放该快照，不能重新查询“最新”；任何 off-machine 发送还需单独显式同意。本轮没有发起真实付费模型请求。最终验证：Rust workspace 221 tests 全绿，fmt/locked Clippy/check/build、531-file gate、392-source arch-check、501-file secret scan 与 `git diff --check` 全绿；完整 `forge accept` 为 **ACCEPTED**（9 pass、0 fail、2 个诚实 N/A）。专项正反例覆盖真实并发 snapshot、公开 payload 重哈希、窗口外旧非法 Prompt role、成员溢出、跨组/Global/非成员隔离、ASCII/UTF-8 极小预算因果门控、总字节裁剪、延迟 Run answer 锚定、默认隐藏正文/逐 Prompt 指纹、缺失 Group ID 不落库与 Human 控制字符；fresh-context 安全/契约复核最终 APPROVE。

## Sprint 38(✅ 完成)— Durable prepared Group Run snapshot

ADR 0009 把 Sprint 37 的 on-demand dossier 接到明确的本地持久化边界。SQLite schema v3 新增独立 `group_runs`，内嵌完整 canonical `GroupContextSlice` BLOB、raw 32-byte 内/外 SHA-256、固定 prepared 状态、Group/version/幂等键与原始时间；合法 snapshot 上限 8 MiB，list 只读元数据。它没有复用 execution-bound Project `runs`，因此不会伪造 Conversation/Prompt/provider 或把空 journal 冒充执行。

`group run prepare/show/list` 全走 Hub 管理路径。首次 prepare 在单一 `BEGIN IMMEDIATE` 中先查 key、再读取 Group/member/Conversation/Prompt、编码一次并提交；同 key + 同 Group/full policy 忽略重试候选 ID/时间并返回原冻结字节，任何语义变化冲突，损坏数据失败关闭且绝不从“最新”历史修复。默认 Human/JSON 隐藏 excerpt、逐 Prompt hash、raw JSON、路径和 key；只有显式 `--json --include-content` 才包含可独立重算两层 digest 的完整公开结构。输出明确 prepared/frozen 且 model/provider execution 未启动；命令不构造 provider/tool/workspace，不写 Project Run/event/assistant Prompt。v1→v3、v2→v3、迁移回滚、跨进程 replay、并发同 key/分歧 key、历史变化、corruption、scope/privacy 与零执行副作用均有测试。本轮没有发起真实或付费模型请求；Group snapshot 的 provider 消费、计划/讨论、多 Agent、远程账号/同步/ACL 与外发同意仍属后续。

最终验证：Rust workspace 262 tests 全绿，fmt/locked Clippy/check/build 与 `git diff --check` 全绿；545-file gate、405-source arch-check、11 项治理检查、515-file secret scan、5-manifest SCA 均通过。完整 `forge accept` 为 **ACCEPTED**（9 pass、0 fail、2 个诚实 N/A）；两份 fresh-context 独立复核均 **APPROVE**，无发布阻断项，当时记录的 schema-v3 完整结构复验加固项现已由 Sprint 41 闭环。专项反例还覆盖默认正文脱敏、显式内容两层重哈希、重复全局 key、终端控制/双向字符注入、正文损坏时 metadata-only list 可用而 show/replay 失败关闭。

## Sprint 39（✅ 完成）— Local Group execution integrity receipt

ADR 0010 在 immutable prepared Group Run 与未来真实 Group Agent 之间增加了一个诚实的本地执行边界。SQLite schema v4 使用独立 `group_executions` 与 `group_execution_events`；key-first `BEGIN IMMEDIATE` 先验证并 pin exact frozen source、创建 incomplete intent，随后三条确定性事件各自在独立 immediate transaction 中原子推进 cursor、journal bytes 与 status。崩溃可留下合法 incomplete prefix；同 key 只因该模式纯本地、无外部 effect，才允许校验 prefix 后补 missing suffix，`start` 必须到 `snapshot_validated` terminal 才返回成功。

`group execution start/show/list` 均走同步 Hub 管理路径；`start` 强制调用方提供显式 key，保证尚未输出 ID 时中断的多事务 prefix 仍可恢复。默认 JSON/Human 只给 record、status、event count 与 content-free receipt summary，不含 event/context body、Prompt/excerpt、逐 Prompt hash、路径、key 或 raw context；JSON list 还明确标记 metadata-only、未复验 source/journal 并指向 `show`。命令不读取 cwd/workspace/`OPENAI_API_KEY`，不构造 AgentRuntime/model/provider/tool/network，也不创建 Project Run/event/assistant Prompt。成功只记录本地冻结 snapshot integrity 校验已完成；receipt/event SHA-256 不是 MAC、签名或第三方 attestation，分析、讨论、task result、Group 模型消费和 multi-Agent execution 均未实现。

最终验证：Rust workspace 296 tests 全绿，fmt/locked Clippy/check/build 与 `git diff --check` 全绿；558-file gate、417-source arch-check、11 项治理检查、528-file secret scan、5-manifest SCA 均通过。完整 `forge accept` 为 **ACCEPTED**（9 pass、0 fail、2 个诚实 N/A）。领域/SQLite/CLI 的反例覆盖 cursor 状态序列与 receipt/status 绑定、source/event/cursor 损坏、v1/v2/v3→v4 原子迁移、并发幂等、崩溃 suffix-only repair、终态零追加、metadata-only list 不越权宣称复验，以及跨进程无 Project Run/Prompt/provider/workspace/network 副作用；fresh-context 最终复核 **APPROVE**，无 Blocker/Major/Minor/Nit。

## Sprint 40（✅ 完成）— Two-phase Group model analysis

ADR 0011 把一个已验证 prepared Group Run 接到独立、可审计的单模型分析边界，而没有复用 Project `AgentRuntime` 或 v4 的可补后缀本地 receipt。SQLite schema v5 新增 `group_model_analyses`、紧凑事件 journal 与独立 result artifact；`group analysis prepare` 在一个 immediate transaction 中绑定完整 source、固定 versioned system Prompt、provider/endpoint/model/limits、canonical private config、exact Responses request bytes、domain-separated hashes 与 `analysis_prepared` 事件。请求唯一 user message 是 frozen `context_json`，固定 `tools:[]`、`store:false` 与 streaming；prepare 不读凭证、不构造 provider、不访问 workspace/当前历史/网络，也不写 Conversation、Project Run、Prompt、task 或 memory。

`group analysis send` 只在 durable 状态仍为 `awaiting_consent` 时接受当次 `--confirm-off-machine`。接口先验证 `OPENAI_API_KEY` 非空且可安全构造 Authorization header，并核对实际完整 endpoint/model；应用层再次在 claim 前核对 provider metadata、source 和逐字节重编码，inspect/list 也会重建 application-owned 固定配置并拒绝自洽篡改。随后 `BEGIN IMMEDIATE` 只允许一个赢家提交 `provider_dispatch_released`，且非 Clone authority 重哈希实际 body 后才消费释放 exact bytes；输家只得到无正文 inspection。claim 一提交即为 `dispatch_unknown`，超时、取消、EOF、HTTP/SSE/provider/protocol/tool-call、本地 byte/event/token limit 或 result commit 失败都不会自动重发，也绝不伪装成 provider `Length`。只有真实 `Completed`/`Length` terminal、零工具且随后 EOF 才能把 canonical result、独立 byte count/hash、completion event、cursor 和 terminal status 原子落库。

默认 prepare/show/send/list 使用专门安全 view，隐藏 frozen excerpt、request/config/event/result body、逐 Prompt hash、key、provider context 与 credential；list 明示 metadata-only/未复验 source+journal，`--include-result` 只显示已验证 final projection，Human 输出转义终端控制字符。所有 SHA-256 只证明本地 domain-separated 内容一致性，不是 MAC、签名、同用户改库防护、remote attestation 或事实认证。结果明确标为单模型生成，不冒充 multi-Agent discussion/consensus、工具执行或 Conversation memory。本轮未发起真实或付费模型请求。

专项测试覆盖 10 个领域 journal/authority 契约、9 个应用两阶段/collector 契约、1 个真实 application→SQLite→reopen 完成链、8 个 SQLite 集成契约与 1 个同连接 late-write 原子回滚契约、9 个 prepared-request/transport adapter 契约、9 个 v5 迁移/定义契约、5 个 CLI parser 与 4 个跨进程 CLI 契约；畸形 header credential 在 claim 前失败，provider sentinel 不进入公共错误，concurrent claim 只有一个 authority，incomplete function call 与 terminal 后分片 frame 均失败关闭，应用/SQLite canonical result 不再漂移，local limit 不成为 `Length`。v1–v4→v5 与 late-conflict 全链回滚继续有反例；另有 19 类错误 column/key/CHECK/index/FK/trigger/catalog 定义在打开 v5 时失败关闭。为保持架构指标语义准确，arch-check 同时以测试坐实 Rust `crate/self/super` 是 crate 内 cohesion、仅外部 Cargo-crate import 计入 fan-in。

最终验证：Rust workspace 350 tests 全绿，fmt/locked Clippy/check/build 与 `git diff --check` 全绿；598-file gate、456-source arch-check、11 项治理检查、568-file secret scan、5-manifest SCA 均通过。完整 `forge accept` 为 **ACCEPTED**（9 pass、0 fail、2 个诚实 N/A）；fresh-context 加固复核 **APPROVE**，无 Blocker/Major/Minor。

## Sprint 41（✅ 完成）— Strict Hub v0–v5 schema ownership

ADR 0012 在不升级 schema v5、不改变任何业务表布局的前提下，闭环 Sprint 38 留下的旧 Schema 结构复验项。Hub 的 `main` catalog 现在明确为应用独占：14 张 v1–v5 表、8 个具名显式索引、由 PK/UQ 派生的 25 个隐式 autoindex，且不允许额外 table/view/trigger/virtual shadow。每次打开都在任何迁移 DDL 前先复验其声明版本的完整 prefix，v0 必须是空应用 catalog；合法 v0–v4 才在原有 `BEGIN IMMEDIATE` 中迁移并运行完整 v5 复验，只有通过才提交。声明布局不匹配及 SQLite corruption/not-a-database 视为 corruption 且不自动修复；锁耗尽、I/O 等环境错误沿用 availability 分类。迁移末端契约失败会把新对象、`user_version` 与数据变化整链撤销。

期望契约由独立内存库顺序执行同一组 v1 create + v2–v5 migration SQL 生成，不维护第二份 SQLite 约束解析器。磁盘定义既要逐字匹配 `main.sqlite_schema`，也要与 schema-qualified `table_xinfo`、`foreign_key_list`、`index_list`、`index_xinfo` 的列/default/hidden、FK、PK/UQ、origin/unique/partial、key 顺序/CID/DESC/collation 结构一致。显式索引还绑定名称；SQLite 自动索引名称不稳定，因此比较排序后的语义签名及 25 项 owning-table multiset。已发布 DDL 的 length-framed SHA-256 固定为 `cb3b65a96f9d4434995ecc409acd7da256332f800142bc661e25f9ab7296ebf8`，独立结构契约摘要固定为 `790b05cb9b2727755829f42fae47e3d0193170acdba41415a2005444e797bbf9`；未来 SQLite 合法表示变化必须显式维护历史契约或新迁移。

专项反例覆盖 fresh/v1/v2/v3/v4→v5 合法升级重开与各代数据保留，v1 Conversation CHECK/index、v2 Run FK/PK、v3 Group Run CHECK/index、v4 execution event FK/PK，owned-table rogue index/trigger、独立 rogue table/view 与持久化 `pragma_index_list` FTS virtual shadow；还覆盖非空 v0、迁移前 future-table blocker、raw autoindex owner 篡改、PK/UQ/显式索引计数 golden 与 writer-lock exhaustion 分类。畸形 v1/v4 prefix 会在 DDL 前拒绝且保持原库不变；另一个仅测试可用、连接作用域的确定性 fault 在合法 v1 真实完成 v2–v5 全部 DDL 后、最终 v5 validator 前注入 rogue table，证明 validator 返回 corruption 时同一事务会撤销整条迁移，原 schema/data/version 不变且无新对象或 fault 残留。这是应用 Schema 漂移检测，不是数据库加密、MAC/签名、同 OS 用户防篡改或 validation 后 TOCTOU 防护；本轮没有网络、provider、credential、workspace、tool、model、Conversation/Prompt/Run 或真实付费请求副作用。

最终验证：Rust workspace 364 tests 全绿，fmt/locked Clippy/check/build 与 `git diff --check` 全绿；605-file gate、462-source arch-check、11 项治理检查、575-file secret scan、5-manifest SCA 均通过。完整 `forge accept` 为 **ACCEPTED**（9 pass、0 fail、2 个诚实 N/A）。原发布复审提出的两项证据准确性 Minor 均已修复并复核关闭；最终 fresh-context 发布复审 **APPROVE**，无 Blocker/Major/Minor。

## Sprint 42（✅ 完成）— Durable local Group analysis panel

ADR 0013 把同一个 frozen Group Run 上已经完成的单模型分析，冻结成一个有序、可复验、纯本地的 panel artifact。`group panel prepare GROUP_RUN_ID --analysis ID...` 只接受 2–8 个 `Completed` 且非 `Length` 的 analysis；每个成员必须绑定同一 exact Group Run/source snapshot，并重新验证 prepared source、canonical result、byte count 与 domain-separated digest。SQLite schema v6 新增 panel 与 ordered membership 两张表；key-first immediate transaction 保证同 key 同 manifest 重放、语义漂移冲突，写入后在同一事务内回读完整 record/manifest。`show` 默认隐藏结果，只有 `--include-results` 才返回经验证 projection；`list` 始终 metadata-only。

该能力刻意只做 artifact assembly：输出机读声明 `assembly_only=true`、`synthesis_performed=false`，不把多份回答冒充讨论、投票、共识或 moderator synthesis。命令不访问网络、provider、credential、workspace、tool、Conversation/Prompt、Project Run、task 或 memory；SHA-256 仍只是本地一致性证明，不是签名、事实认证或同 OS 用户防篡改。真正的 moderator、跨会话实时讨论、远程账号/同步与持久综合记忆继续需要各自独立的同意、权限和 provenance 契约。

安全复审发现并关闭四个高优先级缺口：完整 catalog 现会拒绝任何额外 `sqlite_*` 对象而非跳过；应用层逐字段核对 prepared source；写入路径不再把 corruption/availability 混报为 conflict；SQLite `CORRUPT`/`NOTADB` 统一分类为 corruption。真实 hidden writable-schema trigger、合成 SQLite `CORRUPT`/`NOTADB` 错误分类、矛盾 source 与候选/已存 manifest 分歧均有回归测试。

最终验证：Rust workspace **391 tests** 全绿，fmt/locked Clippy/check/build 与 `git diff --check` 全绿；636-file gate、491-source arch-check、12 项治理检查通过，完整 `forge accept` 为 **ACCEPTED**（9 pass、0 fail、2 个诚实 N/A）。经用户明确授权，另在隔离临时仓库中由真实 `codex exec --ephemeral --sandbox workspace-write` 做黑盒 QA：离线 CLI 集成 2/2、help contract 1/1、构建后二进制拒绝矩阵 6/6，共 **9/9**，没有修改产品源码，被测 CLI 未发起 provider/API 请求。

## Sprint 43（✅ 完成）— Strict Build QA verdict handshake

ADR 0014 为 Build QA 增加独立、失败关闭的 `qa_v1` 机器裁决，而不改变普通 Reviewer、Evolve 或 Release 的既有宽松/专用契约。Build 的 QA phase 必须显式声明 `verdict_contract: qa_v1`，并在所有 mode 下保留自身的 `test` gate；mode 不能跳过 QA。命令执行器保留未经清洗的原始输出，普通可执行文件只接受末个非空行精确等于 `QA_VERDICT: ACCEPTED` 或 `QA_VERDICT: REJECTED`；被解析为 Claude 的命令还必须先给出唯一、完整、成功且非 error 的 JSON result envelope，纯文本不得冒充成功 envelope。缺失、空白包装、尾随 prose、畸形 envelope、裸 CR 或未知 token 都不会制造批准。

`REJECTED` 只能回到更早、可写且不可被 mode 跳过的实现 phase；资产检查与运行时都会验证 target，默认三次 loop-back 预算耗尽后中止。dry/echo 无真实 Agent 输出，因此会在 QA 失败关闭；带严格 QA 的并行执行在启动任何 Agent 前被拒绝。QA 接受不会生成或改写 Deploy/Build/Rollback validation receipt，Release 的 operator-pinned executable、严格 JSON/verdict 与人工批准边界保持不变。

专项 Go 正反例、命令执行 E2E 与 `-race` 全绿，两份独立 fresh-context 契约复核均 **APPROVE**。经用户明确授权，真实 `codex exec --ephemeral --sandbox workspace-write` 从提交 `8b03cd1` 构建静态 Forge 二进制（SHA-256 `e07b8d987d285689a9b15d9a7c7268adcc36c0f8b68c2245fa32d00c8e115f57`），通过原生编译 Agent 子进程和真实 Node gate 跑完 **16/16** 黑盒场景：6 条成功/兼容路径、10 条失败关闭/前置拒绝路径；release receipt 30-byte sentinel 字节不变。Codex 沙箱宿主拒绝 7 个与本功能无关的 sealed-memfd release 测试，但主环境随后完整 `forge accept` 覆盖并通过全部 Go **1040 tests**、Python **68 tests**、Node **378 tests**、Rust 各 workspace、go-taskd 22 tests 与 url-shortener 47 tests，最终 **ACCEPTED**（9 pass、0 fail、2 个诚实 N/A）。

## Sprint 44（✅ 完成）— Two-phase Group panel synthesis

ADR 0015 把 Sprint 42 的 durable panel 接到一个新的、独立且两阶段的单模型 synthesis artifact。`group synthesis prepare PANEL_ID` 在本地重新验证 panel、ordered analyses、各自 source/result 与全部 domain-separated digest，随后把唯一 user message 固定为 canonical `GroupAnalysisPanelManifest` JSON；请求固定 versioned moderator system Prompt、`tools:[]`、`store:false`、streaming、bounded output/event/byte limits、`local_artifact` 与 `writeback:none`。它不另附 dossier/excerpt 字段，但 copied result text 本身仍可能引用或复现原 source，因此不能把“没有独立字段”误述为“不含 source 内容”。prepare 不读 credential、不访问 workspace/当前历史/网络，也不写 Conversation、Prompt、Project Run、task 或 memory。

`group synthesis send SYNTHESIS_ID --confirm-off-machine` 要求针对这次新披露的 fresh consent；先前 Group analysis consent 不授权 panel synthesis。接口和应用层在 claim 前复验完整 endpoint/model/credential、source、canonical config 与 exact request bytes；SQLite 在同一 `BEGIN IMMEDIATE` 里再次读取 durable state，只有一个赢家能提交 claim 并获得非 Clone 的 exact-byte authority，输家只得到脱敏 inspection。claim 一提交即进入 `dispatch_unknown`；超时、取消、HTTP/SSE/protocol/tool-call、缺 usage、非真实 terminal、terminal 后 frame、非 EOF、本地上限或 completion commit 失败都不会自动重发。只有 metered、零工具的真实 `Completed`/`Length` terminal 且随后 true EOF 才原子写入 result/event/cursor/status，result 时间在 EOF 后采样。prepare、claim、complete 都在提交前回读持久态；真实 seq-1/seq-3 trigger fault 证明 late event failure 会整事务回滚。

默认 `prepare/show/send/list` 隐藏 Prompt、panel results、request/config/event/result body、keys 与 credential；只有 `--include-result` 能显示经完整验证的 final projection，Human 输出会转义终端控制字符。metadata-only list 永不根据未复验 status 宣称 synthesis 已完成；所有输出明确这是 one model turn，不是多 Agent discussion、vote、consensus、factual verification、tool/workspace work 或 writeback。schema v7 现为 19 张 owned table、12 个显式索引、33 个隐式 autoindex；v1–v7 length-framed DDL SHA-256 固定为 `4346d038501209b6dc1f5f087b8d330399e526c7c689eb3c10c09a0340940e57`，v7 structural SHA-256 固定为 `44fed8268f1a301860f2ad540f448134e32424c8b6ee5b17fb56a42fcb5b3470`。

最终验证：Rust workspace **424 tests** 全绿，fmt/locked Clippy/check/build 与 `git diff --check` 全绿；700-file gate、553-source arch-check 8/8、12 项治理检查通过，完整 `forge accept` 为 **ACCEPTED**。两轮只读安全/协议审查均无剩余发现；fresh-context 发布复审 **APPROVE**。经用户明确授权，另在隔离临时仓库使用真实 `codex exec --ephemeral --sandbox workspace-write` 做黑盒 QA：6 个 Cargo 命令共 **29/29** 专项测试、35 次直接二进制探测（非法参数 **26/26** 失败关闭），隔离仓 tracked tree 前后均干净。所有产品进程都移除 `OPENAI_API_KEY`，没有 product provider/API/HTTP 请求；Codex QA 控制平面本身使用模型，不能冒充离线。

## Sprint 45（✅ 完成）— Durable lifecycle promotion migration

ADR 0016 把 lifecycle-driven dynamic migration 落成显式、可审计的持久事件：`forge migrate --to-lifecycle production [--apply]` 默认 dry 且零写入，只有 `--apply` 才持久晋升。Explorer 的真实非生产→production 边会在同一事务中变为 engineering、追加五个既有治理欠债任务并写入 production；balanced/engineering/cto 只改变 lifecycle，ROADMAP 字节不变；已在 production 且无回执的仓库保持精确 NOOP，不臆造历史迁移。旧的 `forge migrate --to engineering` 也复用同一事务内核。无显式参数时 run/evolve 读取 `.agent/project.yml`，显式 mode/lifecycle 仍只是本次调用覆盖且永不改写 selector；等待链对隐式 selector 漂移失败关闭。

两类操作共享 `.forge/run.lock`、一个 canonical pending intent 与各自独立的 terminal receipt。事务按 intent→ROADMAP→project commit point→receipt→移除 intent 的顺序 durable publish，精确绑定 before/after bytes、权限位与 digest；失败后只有匹配操作能确定性 roll-forward。所有预览、状态和 busy probe 都是 bounded、side-effect-free read；symlink/hardlink/FIFO、非 canonical/超限状态、跨操作伪造、tracked `.forge/**` provenance、selector/marker/receipt 漂移均在 mutation 或 workflow/agent/trace/checkpoint 前拒绝。`forge status [--json]` 会报告 pending、完成的 operation ID 与恢复命令；竞争提示明确禁止 unlink 锁路径，避免创建第二锁命名空间。

最终安全审查 **APPROVE**，无 Blocker/Major。专项 Go、`-race`、随机顺序、`go vet`、Windows/Darwin 交叉编译、553-source arch-check 与 12 项治理检查全绿。隔离提交树的完整 `forge accept` 两次均 **ACCEPTED**（9 pass、0 fail、2 个诚实 N/A），覆盖 Forge Go **1111 tests**、Python **68 tests**、Node **378 tests**、全部 Rust workspace、go-taskd 22 tests 与 url-shortener 47 tests。真实 Codex 首轮在提交 `29d2689` 的 13 场景黑盒验收中得到 12/13，并发现 busy 路径缺少 never-unlink 警告；修复提交 `2a72acb` 后，Codex 从干净 checkout 离线构建 VCS-pinned 二进制（SHA-256 `e20bfa8f1636f2e1d5a9ad9a926f071921e3a07a1970b60d3007e17715dd2f98`），完整复跑 **13/13 PASS**。独立 held-flock 复现 4 ms 失败关闭、tracked bytes/modes 不变、无回执，解锁后一次重试产生恰好五个不同 marker；前后 `git status --porcelain` 均为空，未调用真实 provider。

## Sprint 46（✅ 完成）— Durable local Group Agent Graph

ADR 0017 在真正调度前增加一个 immutable、可检查的 Group Agent Graph artifact。`group graph prepare/show/list` 把一个 exact prepared Group Run、manager label/instruction、1–32 个 authored task node、冻结的 `project_id + role`、acceptance 和最多 512 条 dependency edge 绑定为 versioned canonical manifest；同一项目可以拥有多个任务节点。Application 按 `(from,to)` canonicalize edge 并派生 authored-order Kahn waves；node order 保持语义并作为 ready-node tie-break。Waves 只表达先后约束，不携带 predecessor result，也不证明 manager/node Agent 被调度或执行。

SQLite schema v8 新增一个 `ON DELETE RESTRICT` immutable graph table 与两个显式索引。Prepare 采用 key-first `BEGIN IMMEDIATE`，在同事务内重新验证 exact Group Run、全部 member binding、canonical order/waves，插入后完整回读再提交；`show` 在同一 deferred snapshot 中复验 source/member/manifest，`list` 诚实保持 metadata-only。同 key 只重放完全相同的 semantic graph 并保留原 ID/time/bytes；source、node order、task 或 manager 漂移冲突，stored corruption 不降级为 conflict。v8 owned catalog 固定为 20 tables / 14 explicit indexes / 35 implicit autoindexes；v1–v8 length-framed DDL SHA-256 为 `5e2108cca17e10f12566abcabe69d8c1a0c965856344c4463f6992ddd30edcce`，v8 structural SHA-256 为 `1edda54070b62bf9777a62166222f5f62c33d6a48484be5e525cc9f42b3304ed`。

该切片只准备可审计的 interchange graph：`forge-core` 仍是未来唯一 dependency-wave scheduler，Rust Agent Runtime 仍只拥有未来单 node model/tool loop；本轮不执行 manager/node、不选择或调用 model/provider、不授权 tools/network、不隐式扫描 workspace，也不写 Conversation/task result/memory。Caller 明示的 spec file 可以被读取并由输出单独报告；默认 prepare/show 隐藏 manifest、path、key、project/role 与全部 instruction/task/acceptance，`--include-spec` 才显式揭示；Human 与通用 argv 错误统一 terminal-escape。Pi 只作为 session/tool/event/RPC 与真实 OS isolation 边界的 clean-room 参考，没有复制其代码、Prompt、类型或产品文本。

最终验证：Rust workspace **474 tests** 全绿，fmt/locked Clippy/check/build 与 `git diff --check` 全绿；725-file gate、577-source arch-check 8/8、12 项治理检查通过，完整 `forge accept` 为 **ACCEPTED**（9 pass、0 fail、2 个诚实 N/A）。安全复核与 fresh-context 发布复审最终均 **APPROVE**，合计无剩余 Blocker/Major/Minor。经用户明确授权，真实 `codex exec --ephemeral --sandbox workspace-write` 在隔离快照 `5318130`（tree `9d101e7`）中对 locked/offline 构建后二进制（SHA-256 `4ca8634c40af9682b2232567ab1834e2337e4c6ca78fa002c1ac081818fb15d6`）完成独立 CLI 黑盒 QA：**23 PASS / 0 FAIL / 1 N/A**；唯一 N/A 是沙箱无 `strace`，未伪算 PASS。隔离仓 HEAD/tree/status 与项目 fixture hashes 不变；全部被测产品进程移除 OpenAI/Anthropic provider 环境变量且未调用 provider 路径。Codex QA 控制平面本身使用模型，不能冒充产品离线执行。

## Sprint 47（✅ 完成）— Core-owned passive Group Agent Graph Run plan

ADR 0018 把静态 Graph 推进到第一个真实跨语言控制契约，同时继续阻止未授权执行。Go 新 `internal/dependency` 成为 workflow 与 Group Graph 共用的唯一 authored-order Kahn 实现；`forge graph-plan --graph-id --manifest-sha256 --input` 严格读取现有 v1 graph spec，canonicalize edges，复算 waves，并输出版本化 Core Plan。Plan 精确绑定 graph/manifest、authored node order、edges/waves 与 scheduler protocol，固定 `execution_contract_present=false`、`dispatch_authority_released=false`；Go/Rust 共读一个 canonical-byte golden，plan SHA-256 固定为 `e286b16586904bd82bd38c63e453843d36ec6c39cc2fbc139e877f53ba56d0d3`。

Rust 新 `group graph run prepare/show/list` 只做被动 admission。SQLite v9 在 key-first `BEGIN IMMEDIATE` 中完整复验 exact Graph、frozen Group source/member、canonical plan 与唯一 `graph_run_prepared` event，原子插入后完整回读才提交；同 key 同语义保留原 Run/time/plan/event，divergent input 冲突，stored corruption 不降级。`show` 用同一 deferred snapshot 重验整条 source/plan/journal，`list` 明确 metadata-only。v9 owned catalog 固定 22 tables / 16 explicit indexes / 38 implicit autoindexes；v1–v9 length-framed DDL SHA-256 为 `4d56c12494001f4584ce021a02c3729afc6c97dc292dfff2edaa91716aa16eab`，v9 structural SHA-256 为 `c9bd523268ade499fe446673a3baa25543f6268c963d502112fd14a607300607`。

Run 状态唯一是 `awaiting_execution_contract`，没有 persisted node-ready/running/completed projection，也没有 Rust next-wave/claim/advance loop。特别是同一项目的多个 node 即使落在同一 topology wave 也不代表 workspace/resource safe。本切片不运行 manager/node，不选择或调用 model/provider，不授权 network/tool/workspace，不产生 result/Conversation/Prompt/memory/writeback；只有 caller 明示的 spec/plan file 可被读取并由输出分别报告。下一 effectful slice 必须先冻结 Node Execution Contract、同项目串行或隔离身份、预算/审批/result provenance，并由 Go core 通过 passive CAS journal 做 claim-before-effect；未知 dispatch 永不因 lease expiry 自动重试。

最终验证：Go `test ./...`、`test -race ./...`、`vet ./...` 全绿，完整验收观察到 Forge Go **1131 tests**；Rust workspace **512 tests**、Graph Run CLI 专项 **6/6** 全绿，fmt/locked Clippy/check/build 与 `git diff --check` 通过；762-file gate、612-source arch-check 8/8、12 项治理检查通过。编译后的真实 Go→Rust CLI 链路完成 Group/Graph→553-byte 无末尾 LF canonical plan→Graph Run 创建/精确重放/完整 show/metadata-only list，产品进程移除 OpenAI/Anthropic 凭证且未调用 provider。完整 `forge accept` 为 **ACCEPTED**（9 pass、0 fail、2 个诚实 N/A）。fresh-context Go 与全系统复审发现的 direct-Build 非法 UTF-8、stdout LF、Human 完整 plan、逐输出 no-effect 声明和 v9 文档漂移均已闭环；最终复审 **APPROVE**，无剩余 Blocker/Major/Minor。本轮未进行 Codex 模型 QA；一次无 Prompt 的本地 `codex exec` 误调用在 `No prompt provided` 前置校验处退出，未发起模型请求。

## Sprint 48（✅ 完成）— Core-owned first-node execution contract

ADR 0019 把 passive Core Plan 推到“已冻结、但绝不执行”的第一个 node contract。Rust `group graph run control export` 只从完整 v1 `awaiting_execution_contract` Run 重建 exact private control snapshot，绑定 Graph/manifest/Core Plan、seq-1 head 与全部 manager/task plaintext，输出 canonical UTF-8 且无末尾 LF。Go `forge graph-node-contract` 是唯一 node selector，固定选择 `plan.waves[0][0]`，构造 exact system/user Prompt 和 request/lane/contract domain digest，并冻结 caller-pinned provider/model、token/byte/event/time/cost/result budgets、`workspace:none`、零 tools/predecessor dataflow、fresh future consent 与 unknown-dispatch no-retry policy。两端共用 byte golden，也共用保守的 byte-stable HTTPS grammar；fresh review 抓到的 Go `net/url` 与 Rust WHATWG normalization 差异已通过两端相同正反例闭环。

Rust `group graph run contract admit/show/list` 在读取 Hub 前拒绝 malformed、oversized、unknown-field 或非 canonical contract。Application 公开 export 仍只允许 v1，但 admission 可从完全重验的 v1 或 exact v2 journal 私下重建原 base control，使同 key 重放在第一次 admission 后仍能返回原 identity/time/event/contract bytes。默认 admission/show/list 只显示 metadata 与真实 honesty flags；control export 和显式 `--include-contract` 才揭示 Prompt、task、project/member、endpoint/model/budget plaintext，Human 输出统一 terminal-escape。

SQLite v10 重建 Graph Run/event table 的 exact v1/v2 状态并新增唯一 contract table。Key-first `BEGIN IMMEDIATE` 在同一 snapshot 内重验 frozen Group source、members、Graph、Core Plan、journal 与 control snapshot，以 expected seq/head CAS 插入 contract 和 `node_execution_contract_admitted` seq-2 event，再只把 Run 推到 v2 `awaiting_core_dispatch`，完整回读后才提交；second key、stale head、divergent bytes、identity reuse 与 stored corruption 均失败关闭，late reread fault 整事务回滚。Catalog 固定为 23 tables / 18 explicit indexes / 41 implicit autoindexes；v1–v10 DDL SHA-256 为 `16752cf9b054b8e840a98976b06e8f2d015aca6f001191943d4ac54a237e352b`，v10 structural SHA-256 为 `ce5383f44a3a982ab127608acda473d1531ff10fc4b6ca8e7036d84fdec75d8d`。

Run 到此仍没有 dispatch authority、claim、provider request、credential read、Agent/model execution、workspace/network/tool capability、task result、Conversation/Prompt/memory/writeback 或 node/wave advance。真实跨进程测试使用 Rust CLI 准备 Group/Graph/Run、导出 control，调用真实 Go binary 生成 contract，再由 Rust admission 创建/重放/显示/列举并直接核对 SQLite ID/time/event/contract bytes 不变；产品进程显式移除 OpenAI/Anthropic 凭证。本轮没有获得新的 Codex/付费模型调用授权，因此没有运行 Codex QA，也没有 product provider 请求。

最终验证：Go full/race/shuffle/vet/build 全绿，完整验收观察到 Forge Core **1146 tests**；Rust workspace **543 tests**、Domain contract **4/4**、Application contract **7/7**、真实 Go→Rust CLI **2/2** 全绿，fmt/Clippy/check/build 通过；807-file gate、655-source arch-check 8/8、12 项治理检查、68 个 Python 与 378 个 Node harness tests、secret scan 和 `git diff --check` 全部通过。最终独立 `forge accept` 为 **ACCEPTED**（9 pass、0 fail、2 个诚实 N/A）。fresh-context Reviewer 发现 `contract show` 曾只验证 contract↔record/event/run 与 Graph↔Run，却过度声明 control 已完整重验；现已从 v2 journal 与 Graph 重构 base control，重新验证全部 control binding、首节点及 exact manager/task Prompt，并以 locally self-consistent tamper 回归锁定，最终复审 **APPROVE**，无剩余 Blocker/Major/Minor/Nit。

## Sprint 49（✅ 完成）— Evidence-backed Evolve scan contract

ADR 0020 把 `workflow_depth.evolve` 从 iteration budget 延伸为可执行的内容契约。shipped Evolve workflow 的唯一首 phase 显式声明 `scan_contract: evolve_scan_v1`，且必须 readonly/observe、无 gate/dependency/write、`feeds_forward:true`；有效 mode×lifecycle `EvolveDepth` 同时进入提示、dry narration 与裁决，生命周期 floor 可加严扫描但永不放宽独立 mutation authority。最终非空行只接受 `EVOLVE_SCAN_V1: {compact JSON}`。六个固定维度、finding/clear/unavailable 状态、opportunity 派生关系、depth-specific 规则与 JSON shape 全部失败关闭；证据 locator 必须指向当前仓库内已有、非 symlink、非空、≤1 MiB 的 UTF-8 regular file 与有效行号。thorough 必须覆盖全部六维、不得 unavailable，且每个 finding 都生成带共同 locator 的 candidate task；opportunistic 只报告有直接证据的 obvious opportunity，standard/advisory 不伪造 completeness 或 implementation authority。

完整 canonical report 以 64 KiB 原子上限进入后续 prompt，不能退化为普通 800-rune summary；原始输出上限为 1 MiB。checkpoint v3 把 phase cursor、canonical scan report、预算 cap/累计整数微美元、Agent-call cap/consumed 和 loop-back cap/consumed 一并持久化。串行在 spawn 前 write-ahead 预留 call，失败 attempt、verdict、正常前进和 loop-back 都先持久化；scan 完成后 resume 重验 report/evidence 并恢复 feed-forward，不再调用 Agent。恢复参数必须匹配原 cap，负成本拒绝，超大有限成本在 checkpoint/trace 同步饱和到 `MaxInt64` 而不溢出为负。serial mid-iteration 状态不得切换 parallel；native parallel 仅声明 iteration-boundary checkpoint，中断 iteration 可整体重放。未声明该 capability 的 legacy workflow 保持原行为。

实现经独立复审确认无 Blocker/Major。`go test -count=1 ./...`、完整 `-race`、`go vet`、20 项 Python workflow checks、12 项治理检查、8 项 arch-check、文件规模 gate 与 `git diff --check` 全绿。按用户要求，真实 `codex exec --ephemeral --sandbox workspace-write` 从最终源码构建二进制（SHA-256 `64a7ff7134f55df9f93213b37aac51814feb31193f195e3bc8c72b7543cb0ae1`），仅通过公共 CLI 和确定性离线 fake provider 跑完 **13/13 cases、77/77 assertions、26 次 CLI invocation**；额外验证 phase0 已耗 call 的 serial checkpoint 在任何 Agent 前拒绝 `--resume --parallel`，以及 `total_cost_usd:1e308` 在 checkpoint/trace 同为 `MaxInt64`、恢复后仍耗尽且不再调用 provider。该黑盒没有发起真实 Claude 或付费模型请求，因此不把 deterministic provider 的通过冒充 live-provider 质量证明。

## Sprint 50（✅ 完成）— Core-owned exact first-node dispatch request preparation

ADR 0021 把已完整冻结的首节点 execution contract 推进为 exact、durable、但仍不具备发送权限的 provider request。Application 仅调用无实例、无凭证、无网络的静态 OpenAI Responses codec，从 contract 精确重建一个 request body；content-addressed request identity 同时绑定 Graph Run、contract、seq-2 journal head、首节点/lane、provider endpoint/model/destination、pricing snapshot、codec 版本及 exact body digest/length。Graph Run 只从 v2 原子推进到 v3 `awaiting_dispatch_authorization`，追加唯一 seq-3 `node_dispatch_request_prepared` receipt，并持续固定 `dispatch_authority_released=false`。

SQLite schema v11 新增 immutable dispatch-request table。Key-first `BEGIN IMMEDIATE` 先处理 exact replay，再在同一 snapshot 内以 parent-first aggregate deep read 重验 frozen Group/Graph/Core Plan/contract、journal head 与 codec bytes，以 contract/request/head CAS 原子插入 request、seq-3 event 并推进 Run；projection 声明存在但 child row 缺失会先报 stored corruption，late reread、journal-head CAS、并发、identity reuse 或任意 binding 漂移全部回滚或失败关闭。通用 v3 Graph Run/contract inspection 会读取实际 request row，复验 record/body digest/length、按 production codec 逐字节重编码，并把 seq-3 receipt（含 envelope `graph_run_id`）每个字段逐项绑定；metadata-only list 明确不声称完成这些复验。v11 catalog 固定为 24 tables / 20 explicit indexes / 45 implicit autoindexes；v11 structural SHA-256 为 `ba468ed1b393264b7788f2a82332667b3053aa1f0ff9074a0b148c1aa8c83fd7`，v1–v11 DDL SHA-256 为 `7019cd92d67e07733b4fbca71757c3f914323e5af944367cb693343fe6694a19`，既有 v1–v10 hash 保持不变。

公共 CLI 只有 `group graph run dispatch prepare/show/list`；默认隐藏 exact body、endpoint/model、pricing、key 与 private source，只有 `show --include-request` 显式揭示 authored bytes，Human 输出统一 terminal-escape。命令没有 claim/send/retry surface，不读取 credential，不构造 provider、AgentRuntime、workspace 或 tool，不访问网络，也不产生 node result、Conversation/Prompt/memory/writeback 或 node/wave advance。fresh-context 复审发现并闭环了四项 honesty/integrity 风险：v3 source inspection 曾只数 row，list 曾对空或未验证 metadata 过度声明 request/pricing，公共 seq-3 validator 曾未绑定 envelope Run ID，prepare 曾在 projected contract child 缺失时把 durable corruption 降级为普通 NotFound/Conflict；修复后 exact body/receipt/aggregate 任一 corruption 均失败关闭，而空/非空 list 都只报告其真实验证边界。本轮完整验收为 **ACCEPTED**（9 pass、0 fail、2 个诚实 N/A），覆盖 Rust workspace、Forge Go 1215、Node 378、Python 74、go-taskd 22 与 url-shortener 47 个测试；851-file gate、697-source arch-check、Clippy/fmt/check/build、secret scan 与 `git diff --check` 均通过。

经用户明确授权，最终真实 `codex exec --ephemeral --sandbox workspace-write` 在隔离的最终工作树快照中自行审计并增强 CLI 黑盒探针，对 27 个真实 Rust/Go 产品子进程得到 **23 PASS / 0 FAIL / 0 N/A**：完整 public prerequisite pipeline、v2→v3 三表原子变更、exact compact Responses bytes/envelope/物理 byte count、destination/dispatch/seq-2/seq-3 domain identity、默认三处脱敏与显式 reveal、同 key identity/body/time 零写 replay、second-key/malformed/claim/send/retry 失败关闭、移除 member workspaces、LD_PRELOAD `connect`/`getaddrinfo` sentinel 零调用、unrelated tables/Conversation/Prompt/memory 零写均有直接证据。每个产品子进程都移除 OpenAI/Anthropic 凭证；测试前后产品源码与文档哈希一致，Codex QA 控制平面本身使用模型，产品没有调用 live provider。

## Sprint 51（✅ 完成）— Effect-free Node Dispatch release authorization

ADR 0022 把 v3 `awaiting_dispatch_authorization` Run 推进到独立、跨语言、但仍无副作用的 release decision。Rust `dispatch release-control export` 从完整重验的 current Graph/Run/Core Plan/manifest、三事件 journal、execution contract、dispatch request 与 exact provider bytes 生成 private canonical v1 snapshot；Go `forge graph-node-dispatch-authorize` 不信任 Rust 的结论，独立重建 original base control 并复验 scheduler/source/head/request/body/lane/destination/pricing/budget/failure policy，输出 domain-separated content-addressed authorization；Rust `dispatch authorization verify` 再从当前 durable state 重建唯一合法 snapshot 并逐字段验证 exact authorization。共享 Go/Rust golden 固定两份 canonical bytes、字段顺序、digest domain、ID 与无末尾 LF。

本切片不改 schema：Hub 仍为 v11，Run 仍为 v3、seq 3，authorization 不落库，dispatch authority 仍为 false。Rust export/verify 使用专用 existing-current-schema read-only open；missing/legacy/corrupt Hub、非 persistent WAL `2/2` header，或 WAL/SHM/rollback-journal sidecar 均失败关闭，不创建目录/DB、不迁移、不 chmod、不配置 WAL，也不进入写事务。默认 verify 输出只暴露必要 identity 与逐项 honesty flags；private snapshot/authorization 包含 Prompt、Project、endpoint/model/pricing 与 exact body，只有显式 artifact 管道可以读取。整个流程不取得 consent、不读 credential、不构造 provider、不访问 network/workspace/tool、不 claim lane、不生成 result/writeback，也不 advance node/wave。

最终验证覆盖 Go full/race/vet/build 与 Rust workspace all-targets/all-features locked-offline test、Clippy/check/build/fmt；专项包括 infrastructure Hub 14 + effect-free safety 3、真实 Rust→Go→Rust CLI 5、release Domain 4/shared golden 1/Application 5。887-file gate、729-source arch-check 8/8、12 项治理检查、secret scan、SCA 与 diff-check 全绿；完整 `forge accept` 观察到 Forge Go 1227、Python 74、Node 378 等测试并 **ACCEPTED**（9 pass、0 fail、2 个诚实 N/A）。fresh-context Reviewer 先后复现并推动闭环 missing-state 建库和 immutable 忽略 hot rollback journal 两个 Blocker，最终用 DELETE + cache-spill + 2 MiB dirty-main held transaction 独立确认 export/verify 均在 store open 拒绝，文件 SHA-256 不变；最终结论 **APPROVE**，无剩余 Blocker/Major/Minor/Nit。本轮没有新的付费模型预算授权，因此未调用 Codex/LLM 或 live provider，所有产品测试均为 deterministic local fixture。

## Sprint 52（✅ 完成）— Effect-free registered destination and pricing readiness

ADR 0023 在 SQLite v11、Graph Run v3/seq 3 与 `dispatch_authority_released=false` 完全不变的前提下，补齐首个真实 dispatch 之前仍可纯本地判定的 destination/pricing 前提。Go `forge graph-node-pricing-snapshot` 只接受四个显式 operator 输入，固定 `openai_responses` 官方 endpoint、`usd_micros`、每百万 token unit、`ceil_each_token_component_v1`、`operator_asserted` 与 `vendor_attestation_present=false`，输出无末尾 LF 的 canonical artifact；共享 Go/Rust golden 固定 destination/pricing digest 与 `840960` 微美元样例。Rust exact decoder 拒绝非 canonical/未知/重复/错型/越界/摘要漂移，使用 checked wide integer arithmetic 分别向上取整 input/output component，再与 authorization frozen budget 比较。该上界只在 operator 声明的 rates 与 input-token ceiling 条件下成立，不是 vendor price sheet、签名、实时价格或账单保证。

Rust production registry 固定同一官方 Responses destination，先以纯 `resolve` 绑定 authorization/pricing/quote，再只从调用方显式传入的 header-safe credential 构造既有 provider adapter；factory 不读取环境变量，HTTP client 禁用 ambient proxy discovery，构造阶段不请求网络。公共 `group graph run dispatch readiness verify GRAPH_RUN_ID --authorization FILE|- --pricing FILE|-` 则完全不触碰该 credential/provider construction 边界：它先有界读取两个 artifact，以 existing-current read-only Hub 重新验证 current durable aggregate、exact request 与 authorization，再验证 registry/pricing/budget，只返回 redacted metadata 和逐项 effect=false。非法 UTF-8、超限输入与 `--idempotency-key` 都在数据库构造前失败；真实 Go pricing + Go authorization → Rust readiness 流程移除 member workspaces、比较 state 全目录字节与 SQLite sidecar，确认 Run 仍停在 v3 且无 consent/credential/provider/network/lane/execution/result/database/advance。

本切片仍不发布 synthetic Node Result 或 Core terminal receipt：二者必须绑定尚不存在的真实 seq-4 claim/head、dispatch ID 与 lane ownership evidence，否则 content digest 会把 fixture 伪装成执行证据。最终验证中 Go full/race/shuffle/vet/build 与 Rust workspace all-targets/all-features locked-offline test、Clippy/check/build/fmt 全绿；readiness Application 3/3、registered factory 4/4、真实 CLI 3/3 通过。906-file gate、747-source arch-check 8/8、12 项治理检查、875-file secret scan、SCA 与 diff-check 全绿；完整 `forge accept` 观察到 Forge Core 1236、Node 378、Python 74、go-taskd 22、url-shortener 47 等测试并 **ACCEPTED**（9 pass、0 fail、2 个诚实 N/A）。fresh-context Reviewer 独立复跑协议与 workspace 测试后最终 **APPROVE**，无 Blocker/Major/Minor/Nit。没有新的显式付费模型测试预算，因此未启动外部 `codex exec` 或产品 live-provider 测试；全部产品验证均为 deterministic local fixture。

## Sprint 53（✅ 完成）— Single-node Dispatch terminal lifecycle

ADR 0024 把此前纯被动的 v3 readiness 推进为一个不可拆开的单节点 effectful 生命周期。公共 `group graph run dispatch execute` 要求 explicit authorization/pricing、fresh `--confirm-off-machine` 与 operator-pinned Go Core binary；在任何可写 open 前，专用 immutable v11/v12 preflight 会完整重建 release control 并拒绝非“单 node、单 wave、零 edge”拓扑。通过后 SQLite v12 才以 seq-3/head CAS 和全 Hub Project lane 原子 claim，approved service path 只有提交赢家取得 non-`Clone` exact request authority，可信 store adapter 属于进程内 TCB；provider 构造禁用 ambient proxy、redirect 与隐式 retry，发送后 collector 只接受 bounded terminal + true EOF，且永不根据 `retryable` hint 重发。

Result/Uncertainty artifact 绑定 Graph Run、node/attempt、dispatch、真实 seq-4 claim head、authorization/request/body/pricing、lane ownership、bounded output/partial output、usage/cost 与完整 terminal chronology。Linux bridge 每次把已打开 Core source 复制到带 write/grow/shrink/exec/seal 的匿名 memfd，复验最终密封字节后才从 descriptor 执行；非 Linux 或不支持 sealing 的 host 失败关闭。纯 Go `graph-node-terminal-receipt` 从真实 v4 private control 独立复验这些 binding，只为 single-node terminal state 生成 canonical receipt；最终 `BEGIN IMMEDIATE` 同时保存 artifact/receipt、追加 seq 5、进入 `completed`/`failed`/`failed_uncertain` 并精确删除 lane。显式 Application cancellation token、可捕获的 provider/HTTP/timeout/protocol/local-limit 失败被固化为 uncertainty；CLI v1 未把 OS signal 接入 token，SIGINT/SIGTERM/KILL/OOM 与其他 hard crash、Core 失败或最终 commit 不确定性会保留 v4 `dispatch_unknown` 与 active lane，重新调用只返回 quarantine，禁止 lease 自动释放和任何 resend。

默认 CLI 只返回 metadata；`--include-result` 才显式揭示已完整验证的 result，uncertainty 也可能包含 bounded partial output。artifact、receipt 与 output 以本地 SQLite plaintext 持久化，fresh consent 不授权 workspace/tool、Conversation/Prompt/memory/task writeback 或其他 node。本协议只承诺同一 Hub 内的 local single-consumption，不声称 remote exactly-once；多节点 successor/dataflow 和 v4 hard-crash no-send adjudication仍属后续协议。

最终专项证据：共享 Go/Rust canonical goldens覆盖 seq 4、claim/lane、result/uncertainty、terminal control/receipt 与 seq 5；Application 覆盖 completed/length/uncertainty、true EOF、HTTP/transport/protocol 分类、fresh-consent/cancel、同 Run 并发单 provider call 与 no-resend；SQLite 覆盖双 claimant、跨 Run Project lane、全阶段 fault rollback、v11→v12 与 exact v11 多节点只读拒绝；Infrastructure 执行真实 sealed pinned Go Core bridge，并拒绝 malformed/incomplete WAL sidecar。公共 CLI 覆盖参数/Core pin、v11 多节点与 consent/auth/pricing/credential 失败零迁移、credential fence、clean/hot-WAL v4 quarantine 脱敏重入；hot-WAL 路径保持 DB/WAL bytes 与 logical state 不变，只允许 SQLite transient SHM read lock。完整 `forge accept` 最终 **ACCEPTED**（9 pass、0 fail、2 个诚实 N/A），观察到 Forge Core 1251、Node 378、Python 74、go-taskd 22、url-shortener 47 以及全部 Rust workspace 测试；818-source arch-check 8/8、严格 Clippy、Go vet/build 与 secret/SCA/diff 检查全绿。fresh-context Reviewer 最终 **APPROVE**，无 Blocker/Major/Minor/Nit。产品测试均使用 deterministic provider 或纯本地 Core。

经用户明确授权，真实 Codex QA 在隔离 clone 中运行；产品子进程处于无产品 provider 凭证、Cargo/Go offline、空代理并拦截非 loopback `connect` 的环境。Rust 既有专项 **226/226**、一次性 credential-absent v11 multi-node probe **1/1**，Go `graphterminal` **13 个顶层测试及 25 个子测试通过**，另 1 个显式 opt-in 测试按设计 skip；Core bridge 首跑因清空环境后缺少 `GOCACHE` 得到 12 pass/1 environment fail，补入隔离 `GOCACHE` 与 `GOENV=off` 后同目标 **13/13** 通过，不属于产品缺陷。首份 Codex 末行虽为 `FAIL`，唯一异议却来自 QA prompt 超出已接受 ADR 0024：它错误要求 topology 先于 ADR 明定的 bounded artifact/Core protocol preflight，属于验收契约假阴性；两位独立 fresh-context reviewer 均按 ADR 裁定 **APPROVE**、无需代码变更。另一次 corrected-contract 只读裁定因控制面长时间无输出被主动终止，未产生 verdict，也未计入产品结果。隔离 clone 初末 HEAD `afdff3663dd448b9c00557d206e1438a64b7ed14`、tree `b1f5a74b6086a1ff4b985504199946697b58c3e9` 与 tracked SHA-256 aggregate `3aa91f9eabf37573c17ed09b9301c5de714a27cdf868150ecdcf3f66e8ac1d9d` 均不变，status/diff 为空且无外连记录；Codex 控制平面使用了用户授权的模型，但产品未调用 live provider/API。

## Sprint 54（✅ 完成）— Passive multi-node Graph Execution Schedule

ADR 0025 在不松动 Sprint 53 单节点执行 fence 的前提下，为 frontend/backend/SSO 等多节点 Graph 增加一个 Core-owned、content-addressed、纯被动的 schedule sidecar。Go `forge graph-execution-schedule` 只接受 exact private v1 control，按 topology wave 再 authored order 冻结 serial order、`max_in_flight_nodes=1`、attempt 1、Project lane digest、authored-order direct-predecessor receipt slots、完整 initial frontier 与确定性 initial node；固定 `completed_contiguous_prefix`、exactly-once attempt、fail-fast/no-retry、ordering-only/no-dataflow/future verified receipt slots。四个 artifact authority/progress flag 永远为 false，canonical digest domain 为 `forge.group-agent-graph-execution-schedule.v1\0`；共享 diamond fixture digest 固定为 `809d5235e4298ea8a66cb0654b0e662b94a8568e4c184cf1a927bda1c46e8148`。

Rust domain/application 独立重建 exact control 并逐字段复验 schedule；公共 `group graph run schedule admit/show/list` 在 SQLite v13 的独立 immutable table 中保存每 Run 唯一 sidecar、exact bytes/digest/head 与本地 replay key/time。`BEGIN IMMEDIATE` 内先完整验证 current pristine v1/seq-1 source，再处理 create 或 exact replay；stale-head same-key replay 也必须 conflict，stored corruption 优先于 replay/conflict。迁移只增加一表一索引，v12 lifecycle 表、Run/journal、active v4 claim/lane 与 WAL state不改；v13 catalog 为 29 tables / 25 named indexes / 64 implicit indexes，v1–v13 DDL SHA-256 为 `1e10710c621e80e62c927842f73097fe141ff247df0fba851543175ee6012a49`，结构摘要为 `2b12222a5a0f1e7d3336ac4399e80cfa6a097f50bd3de3cc145541e43d6fbbc1`。

默认输出隐藏 schedule body、node/predecessor/lane 与 key；只有 `show --include-schedule` 显式揭示。历史 schedule 可在 later legacy contract v2/v3 后继续完整 show，但 JSON 把冻结假值明确命名为 `artifact_*` 并声明 `current_run_lifecycle_included=false`，不把 artifact 当成当前 Run 状态。fresh-context 审查在发布前发现并推动闭环三项竞态/honesty 缺陷：same-key replay 曾跳过 pristine head、application post-commit reread 曾把合法并发 contract advance 误报 Corrupt、历史 view 曾把 artifact false 写成无 scope 的 lifecycle false；修复后 schedule→contract 真实 CLI E2E、advance 后 replay conflict 与 historical show 均有回归证据，最终 reviewer **APPROVE**、无剩余发现。

最终验证：Rust workspace **790 tests** 全绿，Go full test/vet/build、fmt、locked offline strict Clippy/check/build、1014-file gate、851-source arch-check 8/8、12 项 governance check 与 `git diff --check` 全绿；完整 `forge accept` 为 **ACCEPTED**（9 pass、0 fail、2 个诚实 N/A）。产品测试只运行 deterministic local fixture 与真实 Go/Rust 子进程，没有 credential/provider/network/workspace/tool/result/writeback effect。当前 schedule 仍不是 contract、receipt、progress 或 successor execution；本轮没有新的显式付费模型预算，因此未启动外部 Codex/LLM 或 live provider 测试。

## Sprint 55（✅ 完成）— Passive schedule-bound initial-node contract candidate

ADR 0026 在不消费 Graph Run journal、不松动多节点 dispatch fence 的前提下，只交付 contract-v2 的严格 ordinal-zero 空前驱子集。Go `forge graph-scheduled-node-contract` 从 exact private v1 control 独立重建 Core schedule，只匹配 caller 声明的 schedule digest 而不读取 Hub，唯一选择 execution ordinal 0 / attempt 1，并冻结 pristine seq-1 head、node/Project lane、canonical system/user Prompt、provider/model、全部预算以及空 predecessor-node/terminal-receipt 集合。调用方不能选择 node、ordinal、attempt、schedule body 或 receipt；六个 lifecycle/provider-request/authority/progress/successor flag 固定 false。共享 Go/Rust fixture 锁定 canonical candidate、logical request、digest domain、content ID 和无末尾 LF 字节。

Rust domain/application 从 exact control 和 stored schedule 独立重建每个 source、Prompt、lane 与 identity；公共 `group graph run scheduled-contract admit/show/list` 在任何 Hub open 前拒绝 malformed、oversized、非 canonical、绑定错误、空 key/ID/filter。SQLite v14 只新增 one-per-Run immutable candidate sidecar；key-first `BEGIN IMMEDIATE` 在同一 snapshot 中完整验证 current Run/Graph/schedule、全部候选字节与 v1/v2 family exclusion，插入后完整回读才提交。exact replay 只有在 source 仍 pristine 时保留原 bytes/time；不同 key/input、stale head、identity reuse、stored corruption 与 v1/v2 并发均失败关闭。Catalog 固定为 30 tables / 27 explicit indexes / 71 implicit autoindexes；v1–v14 release SHA-256 为 `6e573a754bdee36aaea820554d45b0c72a5c30fdbc50a9f0deb75ce88047f616`，v14 structural SHA-256 为 `ce999cba9a007d9e91cd303a8c631bb0a5fceb5818bda371dc356b51915abce9`。read-only compatibility 保留 exact v11–v14，hot-WAL no-send reentry 保留 exact v12–v14。

默认 admission/show/list 只返回 metadata，隐藏 Prompt、node/member/profile、Project lane、request、provider、budget、key 与 candidate plaintext；只有 `show --include-contract` 显式揭示完整私有 artifact。每个 JSON/Human view 都明确它只是 passive initial candidate、`current_run_lifecycle_included=false`，且没有 lifecycle contract、provider request、authority、progress、predecessor receipt、successor 或任何 credential/provider/network/workspace/tool/result/writeback effect。真实 Go→Rust CLI E2E 在移除三个 member workspace、注入 CRLF poison credential sentinel 和 nonblocking loopback endpoint 后完成 admission/show/list/replay/conflict；除 candidate sidecar 外全部 Hub table 的排序逻辑快照前后完全相等，listener 保持零连接。Rust consumer 另覆盖 duplicate/unknown/missing/null/reorder/trailing/Unicode/invalid UTF-8/oversize/identity 与 cross-run/schedule/source/manifest/plan/control/node/lane/ordinal/attempt substitution；store 覆盖 stale-head、same-key divergent valid input、corruption-first replay、same-v2 及 cross-v1/v2 race 和 late-reread rollback。

最终验证：Rust workspace **821 tests** 全绿，candidate Domain **7/7**、Application **3/3**、真实 Go→Rust CLI **4/4**、SQLite **8/8**；Go full/race/vet/build、Rust fmt/locked-offline strict Clippy/check/build、1056-file gate、890-source arch-check 8/8、12 项 governance check 与 `git diff --check` 全部通过。完整 `forge accept` 为 **ACCEPTED**（9 pass、0 fail、2 个诚实 N/A），其中观察到 Forge Core **1283 tests**。测试只运行 deterministic local fixture 与真实本地 Go/Rust 产品进程；没有调用 live provider，也没有新的外部 Codex/付费模型预算。

## Sprint 56（✅ 完成）— Passive scheduled-node provider-request sidecar

ADR 0027 为已 admission 的 ordinal-zero scheduled candidate 增加独立 exact-byte request sidecar，而不复用 legacy dispatch lifecycle。Rust 从完整重验的 candidate/Run/Graph/control/schedule/Prompt/lane/provider/pricing/logical request 生成唯一 `ModelRequest`，再通过公共确定性纯 Responses codec 编码并立即复验 compact canonical JSON；固定 `store:false`、`stream:true`、空 tools、原 output-token ceiling 与 reasoning encrypted-content include。body、destination 与新 envelope 各自 domain-separated；candidate 的 `provider_request_present=false` 保持 creation-time 事实，新 inspection 另以 `provider_request_sidecar_present=true` 表达当前 sidecar，避免把 artifact 字段冒充 aggregate lifecycle。

SQLite v15 只新增 35 列 immutable `group_agent_graph_scheduled_node_provider_requests` 与两个显式索引。key-first `BEGIN IMMEDIATE` 对 ID/Run/schedule/contract/logical-request/两个 slot 的所有匹配行先做 corruption-first 完整验证，再重建 source 与 production codec bytes、要求 pristine v1/seq-1 head、guarded insert、全源回读后提交；exact replay 保留原 ID/body/time，divergent key/input、identity reuse、source drift、stored corruption 与 late reread 均失败关闭。Run、main journal、legacy request/lifecycle/lane 表完全不变。Catalog 固定为 31 tables / 29 explicit indexes / 79 implicit autoindexes；v1–v15 release SHA-256 为 `3de756301993c122077feab587c102108fe337a2cef4920b9d756a5171aae393`，v15 structural SHA-256 为 `d9f6c0eb2a2374b24ee460435cc818f34c78d08f9092519d646d8c5518bf078b`；immutable dispatch preflight 保留 exact v11–v15，hot-WAL no-send reentry 保留 exact v12–v15，并在两条 opener 路径中优先保留 corruption 分类。

公共 `scheduled-contract provider-request prepare/show/list` 在 Hub open 前拒绝空 ID/key/filter 与非法 limit；show/list 使用 existing-current read-only opener，默认隐藏 exact body、Prompt、endpoint/model、lane、pricing、digest/key，只有 `show --include-request` 显式揭示。list 明示没有验证 current Run/source/body。真实 Go schedule/candidate→Rust request E2E 在移除 frontend/backend/SSO workspaces、注入 CRLF poison credential 与 nonblocking loopback endpoint 后比对 SQLite BLOB 和显式 reveal 的 exact bytes，并证明除新 sidecar 外所有 Hub table 不变、零连接。legacy prepare/show/list/release/authorization/readiness 都无法发现或消费该 sidecar；跨层审计另发现 legacy `dispatch execute` 曾在 source/consent/readiness 前启动 pinned Core handshake，现已重排，并以真实可执行 sentinel 证明 scheduled-only Run 拒绝时 Core 零启动、Hub/workspace 零写入、endpoint 零连接。

最终验证：Rust workspace **862 tests** 全绿，其中 scheduled request Domain **4/4**、Application **6/6**、SQLite integration **9/9**、production exact-byte golden **1/1**、真实 CLI **6/6** 与 legacy/Core fences **2/2**；Forge Core **1283 tests**，Go full/race/vet/build、Rust fmt/locked-offline strict Clippy/check/build、1082-file gate、915-source arch-check 8/8、12 项 governance check 与 `git diff --check` 全部通过。完整 `forge accept` 为 **ACCEPTED**（9 pass、0 fail、2 个诚实 N/A）。测试只运行 deterministic local fixture 与真实本地 Go/Rust 产品进程；没有调用 live provider，也没有新的外部 Codex/付费模型预算。

## Sprint 57（✅ 完成）— Effect-free scheduled-node dispatch authorization

已完成 `/home/u1/ai-batch-runner` clean-room 功能审计与 ADR 0028 决策：参考树当前 ahead/dirty 且 README 明示 monorepo provenance 与 no-license，故不复制实现。只采纳 fingerprint-resume 与 fail-closed gate 两项协议原则，映射为 Rust 从完整重验的 v15 current state 导出 private release control → Go 独立重建 schedule/candidate/request 并生成 content-addressed authorization → Rust fresh-state verify。artifact 对未来 exact lifecycle admission + execution/dispatch release 作决策，但当前 admitted/released facts 仍为 false。

Rust Application 在 export/verify 两条生产路径都先以配置的 exact provider codec 复验 persisted Responses bytes，再做 Domain 的结构与 content binding；Go 严格 decoder 独立重建 schedule/candidate/logical request/provider request、lane、destination、pricing identity、budgets 与 failure policy。两端共用 canonical no-LF golden 和 domain-separated digest；Rust verify 只返回 redacted metadata，诚实区分可见 content-addressed IDs 与隐藏 standalone digests。schema 保持 v15、Run 保持 pristine v1/seq-1，authorization 不落库；export/verify 不写 DB，不读取 consent/credential，不构造 provider，不访问 network/workspace/tool，不 claim lane，不观察 progress，不生成 terminal receipt/result/writeback，也不授权 successor。

最终验证覆盖 Rust workspace all-targets/all-features locked-offline、strict Clippy/check/build/fmt，Go full/race/vet/build，共享 golden、Domain/Application adversarial tests 与真实 Rust→Go→fresh Rust CLI 4/4；文件规模、架构、治理、secret/SCA 与 diff gate 全绿，完整 `forge accept` 为 **ACCEPTED**。两位 fresh-context Reviewer 分别从架构和安全边界复核后均 **APPROVE**。按用户要求，独立 Codex 在隔离快照中对已编译二进制完成黑盒 QA 并给出 **QA ACCEPTED**：4/4 E2E、Go file/stdin byte-exact、全部负例 fail-closed、零 loopback connection、零 pre-Hub state、测试前后 binary hash 与 repo status 不变；产品没有调用 live provider 或公网。

## Sprint 58（✅ 完成）— Effect-free scheduled-node dispatch readiness

ADR 0029 采用 ADR 0023 已发布、与 Graph source 无关的 Go canonical pricing snapshot，而不另造 scheduled pricing 格式。Rust 将 fresh current-v15 scheduled authorization 与 exact pricing bytes、官方 registered destination、相同 checked integer cost 算法和 frozen budget 组合验证；成功只返回脱敏 readiness metadata。schema、Run、journal、candidate/request 与全部 current effect facts 不变，readiness 不落库，也不读取 consent/credential、构造 provider、访问 network/workspace/tool、claim lane、send、产生 result/receipt/writeback 或推进 successor。

安全审计否决了“先 admission/release/claim、以后再 send”的拆分：durable claim 后的任何 crash 都已形成不能靠时间或 lease 重建 send authority 的不确定状态。Domain/Application/registered-registry/CLI 的正反例与真实 Go→Rust CLI E2E 已交付；架构闸门发现并修复 52 行函数与 application fan-in，fresh review 又补齐 future-only authorization/current-send-false 输出和 endpoint/model/destination/pricing/token/budget drift 矩阵。两位 fresh-context Reviewer 均 **APPROVE**。独立 Codex 首轮受限沙箱完成 30/30 黑盒负例但因 loopback bind 被宿主拒绝而诚实拒收；没有豁免该结果，新的隔离、显式开放本地 loopback 能力的 QA 随后真实运行 native E2E 3/3 并给出 **QA ACCEPTED**。产品未调用 live provider 或公网。

最终验证：Rust workspace all-targets/all-features locked-offline test、strict Clippy/check/build/fmt，Go full/race/vet/build，1136-file gate、965-source arch-check 8/8、12 项 governance check 与 `git diff --check` 全部通过。完整 `forge accept` 为 **ACCEPTED**（9 pass、0 fail、2 个诚实 N/A），其中观察到 Forge Core **1294 tests**。

## Sprint 59（✅ 完成）— Independent scheduled-node effectful dispatch lifecycle

ADR 0030 把 scheduled ordinal-zero request 从 readiness 推进到一条独立于 legacy
single-node family 的 effectful sidecar 协议。Rust 在 fresh authorization/pricing、
registered destination、header-safe credential 与 pinned Core preflight 全部通过后，
才打开 SQLite v16 写状态；`BEGIN IMMEDIATE` 原子完成 pristine v1/seq-1 Graph Run
校验、exact prepared-request claim 与全局 Project-lane claim。claim 同时绑定两种不同
内容身份：`provider_request_sha256` 是 prepared envelope digest，
`request_body_sha256` 是实际发送字节 digest，二者贯穿 authorization、claim、artifact、
control、receipt，不能互换。

一次性 authority 只把精确 body 交给 bounded collector；completed/length/uncertainty
都必须有完整的 terminal chronology、usage/cost 约束与 canonical byte count。scheduled
terminal control 交给 SHA-256 固定的 Go Core 子进程，Core 独立复验并生成一个中间
receipt；随后第二个立即事务完整回读并原子保存 artifact/control/receipt、释放 lane。
Core/commit 失败只保留 artifact-only quarantine，任何重发、retry、resume、lease、
health check 或 successor/wave advance 都被禁止。scheduled Run/journal 继续保持
v1/seq-1，legacy dispatch 也不能发现 scheduled sidecar。

SQLite v16 的完整 owned-catalog/structural contract、corruption-first read/reentry、
跨 source/artifact/control/receipt 绑定、canonical output 与 metadata-only CLI projection
均已加入回归。Rust/Go full test、vet、fmt、locked-offline check/clippy 与真实本地 Go
Core protocol handshake 通过；测试只使用 deterministic/local fixtures 和本地 pinned
Core，不触碰 live provider、付费模型、workspace、tool 或 successor。

## Sprint 60（✅ 完成）— Passive successor contract candidate consuming verified predecessor receipts

ADR 0031 把 scheduled ordinal-zero 的 terminal receipt 接成 successor 前置，但保持
effect-free：`scheduled-contract predecessor-receipt export PROVIDER_REQUEST_ID` 从
v16 lifecycle sidecar（仅 terminalized）导出 exact canonical receipt 与 domain digest；
`forge graph-scheduled-node-contract --predecessor-receipt FILE...` 由 Go Core 重建
schedule、验证 receipts 构成 serial 连续前缀（每份 receipt 绑定 graph_run/lane/node/
attempt 且 retry/successor 双 authority 为 false）、选定下一个 ordinal 并生成
scope=schedule_successor_only 的 successor candidate（predecessor 只作证据，
`predecessor_content_included=false`，全部 effect flags 保持 false）；Rust
`scheduled-contract successor admit/show/list` 在 SQLite v17（新不可变侧车表，33 表
catalog）中原子保存，admission 逐字节复验每个 receipt 与 durable terminalized
lifecycle 一致。Go/Rust 共享 golden 之外，专项测试覆盖 receipt 前缀/漂移/全消费拒绝、
scope-aware 验证（initial 仍拒收任何 predecessor）、CLI 解析与 key 门控，以及
v17 迁移的全量旧版兼容（未来版本测试、降级 fixture、dispatch re-entry 允许 v17）。
Sprint 59 的架构闸门修复（包文件数/扇入/函数长度/严格 clippy）与 v17 迁移测试更新
已一并收口：`forge accept` 为 **ACCEPTED**（9 pass · 0 fail · 2 诚实 N/A），
Rust 911 tests、Go 全量、arch-check 8/8 全绿。仍未 dispatch、不 claim lane、不读
credential、不推进 wave/successor；跨 node disclosure/consent、effectful successor
dispatch 与 legacy v4 hard-crash adjudication 仍属后续协议。

## Sprint 61（✅ 完成）— Effectful successor dispatch:ordinal-N 被动链全通

ADR 0032 解除 scheduled 家族 passive 链上的 ordinal==0 硬墙,让 ADR-0031 的
successor candidate 能走进 provider-request codec 管线:domain 的
`validate_against_sources` 按 scope 分支(initial 路径逐字节不变;successor 路径
校验 serial 选择、direct-predecessor 覆盖与 Project lane),admission record 按
predecessor_receipt_count 区分 ordinal 槽位,provider-request record 与 dispatch
release control 接受 1..=31。claim/terminalize 内部本就 ordinal-agnostic,故
effectful execute(ADR-0030)语义不变:fresh consent、原子 claim+lane、bounded
collector、pinned Core receipt、terminalize+释放 lane、no-resend quarantine;
scheduled Run 保持 v1/seq-1。同 project 的串行节点由
`exclusive_until_terminal` lane 策略天然串行(前一 ordinal terminalize 释放后
后继才能 claim),不同 project 用不同 lane 可独立推进。专项测试:基于 SpyHub
真实 contract 改造成 ordinal-1 successor 的 provider-request prepare 全链路
(codec/绑定/record 校验),证明 successor 请求字节可被同一纯 codec 编码并持久化。
`forge accept` 为 **ACCEPTED**;Rust 912 tests、clippy/arch 全绿。仍未做:
跨 node disclosure/consent(predecessor 内容入 prompt)、wave 并行、legacy v4
hard-crash adjudication;dispatch execute 的拓扑 fence 不变(successor 需先
通过被动链取得授权)。

## Sprint 62（✅ 完成）— SQLite v18:provider-request 表支持 successor ordinal

ADR-0032 的 domain 放松后,SQLite 层仍有两道 ordinal 墙:v15 provider-request
表的 `execution_ordinal = 0` CHECK 与指向 initial candidate 表的 FK/EXISTS
保护。SQLite v18 重建该表:CHECK 放宽为 0..=31,列级约束保留,两个 FK 移除
(引用完整性由 store 层双表校验替代);INSERT 的序列-1 head 保护改为 initial/
successor 双候选表匹配;回读/Graph Run 关联校验在 initial 表 NotFound 时
fallback 到 successor 表(轻量 decode,避免经 Graph Run 重查的递归)。
专项集成测试证明 ordinal-1 successor candidate admit 后,其 provider request
可落库并在回读中完整复验。迁移兼容:v17→v18 数据保留、v12–v16 降级 fixture
重建 v15 表、dispatch re-entry 接受 v18、未来版本测试移至 19。
`forge accept` 为 **ACCEPTED**;Rust 913 tests、clippy/arch/gate 全绿。
effectful dispatch execute 的拓扑 fence 仍不变;跨 node disclosure/consent、
wave 并行与 legacy v4 hard-crash adjudication 仍属后续协议。

## Sprint 63（✅ 完成）— G3 多维路由自动评分器接入真实执行路径

Sprint 30 把"完整多维评分器接入真实执行"改判为独立大特性(包文档自我推迟为
v2+ Router service)。本轮补齐它的自动生产者与真实消费端:`routing.FromChangedPaths`
从改动路径集合推导六维 dims(complexity=文件量/8、context_size=跨域数/3、
dependency_change=核心面命中/5、business_impact=敏感面/生产流量/迁移、
risk=分类器级别 0..1、security 镜像 business_impact),纯确定性;`forge run`/
`evolve` 的 tier resolver 新增 score 提升步骤(raise-only,BandForScore 决定
+0/+1/Opus,位于 budget 降档之前所以评分提升不被预算压掉,风险提升之后叠加);
`forge route --diff-files/--from-git` 对未显式设置的维度自动填充(diff 驱动
自动评分,显式 flag 仍权威)。无 diff 时全链路逐字节向后兼容。专项测试覆盖
敏感多文件 diff 达到 Sonnet/Opus 带、单文件 README 只带复杂度、空输入全零、
band 提升逻辑。`forge accept` 为 **ACCEPTED**;Go 全量测试、arch 8/8、gate 全绿。
至此 G3 的多维路由(复杂度/风险/依赖/上下文/业务影响)从手动 `forge route`
跃升为 run/evolve 的真实执行输入;跨厂商模型池仍属 v3。

## Sprint 64（✅ 完成）— ADR 0033 跨 node predecessor content disclosure + 独立 consent

多节点 Graph 执行闭环的最后一环:successor 的 agent 现在能携带前驱产出的
exact result 文本。request-v2 user Prompt 增加可选 `predecessor_output` 字段
(omitempty —— 所有既有候选/ golden/ digest 逐字节不变);Go Core 以
`--predecessor-content FILE|-` 把有界(≤1 MiB)UTF-8 前驱文本嵌入 prompt 并置
`predecessor_content_included=true`;Rust 严格校验 flag 与字段共存(prompt
含内容 ⇔ flag true),admit 要求 `--predecessor-content` 并逐字节验证 prompt
内嵌文本 == 调用者提供文本 == durable terminalized lifecycle 的 result-class
artifact 输出(uncertainty 不可披露);effectful dispatch execute 对含前驱
内容的候选要求独立 `--confirm-predecessor-content`(与 --confirm-off-machine
互不推断)。专项测试:prompt 往返、flag/字段一致性拒绝、Go 端注入/省略、
consent 门控;Sprint 62 的 ordinal-1 集成测试随输入结构同步更新。
`forge accept` 为 **ACCEPTED**;Rust 915 tests、Go 全量、arch/gate 全绿。
剩余 Graph 协议:wave 并行与 legacy v4 hard-crash adjudication。

## Sprint 65（✅ 完成）— ADR 0034:本地 hard-crash no-send adjudication

补上 ADR-0024/0030 的 stranded-claim 收尾:dispatch execute 在 claim 前写
`.forge/executor-pids/<request_id>.pid`(pid + hostname)、terminalize 后删除;
可捕获的 SIGINT/SIGTERM 折入 cancellation token(干净 uncertainty terminal +
lane 释放,不再 stranded)。`dispatch adjudicate PROVIDER_REQUEST_ID` 是唯一
显式、永不自动/时间驱动的 lane 回收:要求 durable 状态为硬崩溃后的 claimed(无
任何 terminal evidence、lane active),读 pid sidecar 证明同主机 executor 已
停止(sidecar 缺失、跨主机、pid 存活一律拒绝),然后原子置
status='adjudicated' + lane_active=0。SQLite v19 重建 lifecycle 表(status
CHECK 加 'adjudicated';列不变,旧库查询零破坏),dispatch re-entry 接受 v19,
future 版本测试移至 20。专项覆盖 v19 迁移全代兼容与状态/证据形状校验。
`forge accept` 为 **ACCEPTED**;Rust 915 tests、Go 全量、arch/gate 全绿。
诚实边界:pid liveness 是本地 OS 级证据(同用户可信模型,非 MAC/签名/跨主机
adjudication);完整 claim→crash→adjudicate 端到端需真实硬崩溃场景,store 逻辑
经编译与回归验证。wave 并行仍属后续协议。

## Sprint 66（✅ 完成）— Wave-parallel successor planning + 外部资源本机部署验证

两部分:
(1) **外部资源解锁**:`/dev/kvm` ACL 可写 + 网络可达 + sudo 可用,Firecracker
v1.7.0 已安装并真实启动 KVM 后端 microVM(官方 vmlinux + 自建 busybox
rootfs,guest init 输出 FORGEOS-FIRECRACKER-VERIFIED 后 poweroff);LiteLLM
跨厂商路由实测(deepseek @ :4000 + 本地 Ollama @ :11434 双后端,厂商 B 完整
推理成功,厂商 A 路由正确但上游月度限额)。两项从 BLOCKED-EXTERNAL 划为
host-VERIFIED,记录于 `docs/external-resource-verification.md`。
(2) **wave 并行计划层**:SQLite v20 重建 successor-candidates 表
(graph_run_id/schedule_id 的 one-per-Run UNIQUE 移除,保留
UNIQUE(graph_run_id,node_id,attempt) per-node 槽位),store 适配多候选
查询;Go `BuildSuccessor` 支持指定目标节点(带拓扑就绪验证)并新增
`ReadySuccessorNodes` 输出就绪节点清单 —— diamond 图同 wave 并行分支的
批量候选计划视图。专项测试:per-node 冲突、就绪清单序列、乱序 receipts。
`forge accept` 为 **ACCEPTED**;Rust 916 tests、Go 全量、arch/gate 全绿。
多 dispatch 并发编排命令(一次跑多个 execute)仍属后续编排层。

## Sprint 67（✅ 完成）— 真端点冒烟验证（LiteLLM + Ollama live gateway）

Sprint 59 以来 effectful dispatch 只有确定性 provider 测试;本 sprint 用本机
已验证的外部资源做了首次**真实网络 + 真实推理**验证:
(1) **LiteLLM Responses 转译实测**:forge 的 OpenAI Responses codec 请求与
SSE 解析与真实端点互通(:4001 `/v1/responses`,双后端 deepseek + 本地
Ollama)。发现并修复 LiteLLM 转译缺陷:qwen3.5 思考模式下输出全进
reasoning、流式缺 assistant message —— 配置 `reasoning_effort: none`
(映射到 Ollama `think:false`)后得到标准纯 `output_text` 流。
(2) **防漂移校验活体验证**:LiteLLM 转译给流式 item 与 completed 快照分配
不同 message id(`msg_…` vs `chatcmpl-…`),forge adapter 的
terminal-consistency 守卫**正确拦截**(provider_protocol
"terminal output did not match streamed assistant events")—— 真实转译缺陷
下防漂移防御的实证。记录于 `docs/external-resource-verification.md`。
(3) **测试基础设施**:`new_insecure_for_test` 工厂(TestLoopback policy +
loopback destination 校验)+ `FORGE_LIVE_GATEWAY_ENDPOINT` 环境变量
gated 冒烟测试(无端点时诚实 skip,CI 不受影响)。
`forge accept` 为 **ACCEPTED**(343 infra / 全 workspace 全绿);
LiteLLM :4001 实例常驻本机(deepseek-flash + local-qwen)。

## Sprint 68（✅ 完成）— Firecracker sandbox runner 真实接入

forge-core 的 v3 Sandbox 扩展点(原 fail-closed:任何非 none sandbox 一律拒绝)
获得第一个**真实运行实现**:
(1) **FirecrackerRunner**(新子包 `internal/orchestrator/firecracker`):每轮运行
从 busybox rootdir 模板拷贝 + 注入命令 init 脚本 → `mke2fs -d` 构建全新 ext4
镜像(免 sudo)→ firecracker v1.7 启动(KVM)→ guest 挂载/执行/写退出码
marker/自动 poweroff → 宿主 debugfs 回读 marker + 串口捕获输出。错误分类:
缺二进制/KVM = KindConfig、guest 超时 = KindTimeout、非零退出 = KindFailed。
(2) **踩坑实录**(诚实记录):debugfs 直接注入模板镜像会失败 —— ext4 journal
重放覆盖注入(inode 块分配异常、旧事务恢复),多次实验后改为"从零构建镜像"
方案根治(每次 mke2fs 新镜像,无历史状态)。
(3) **验证**:真 KVM 微虚拟机端到端 PASS(`FORGELIVE-VM-OK` 从 guest 内输出,
~1.4s 完成);fake-runner 接线测试(接线的 argv/超时/分类);arch-check 8/8
(package 导出 33→30:runner 拆子包);`forge accept` 为 **ACCEPTED**。
模板准备脚本见 `docs/external-resource-verification.md`。

## Sprint 69（✅ 完成）— Docker sandbox runner(v3 扩展点第二个 runtime)

`SandboxConfig.Type "docker"` 的真实实现(`internal/orchestrator/docker`):
每轮 `docker run --rm --network none` 全新容器(无网络隔离 = sandbox 语义),
`--memory` 限额,stdout 捕获,退出码透传。错误分类与 firecracker 对称
(daemon/镜像缺失 = KindConfig、超时 = KindTimeout、非零 = KindFailed)。
**隔离契约提升为独立 `sandbox.Runner` 接口**(`internal/orchestrator/sandbox`,
firecracker 与 docker 两个 runtime 共同实现,消除 firecracker 名绑定)。
验证:真 alpine 容器端到端 PASS(`FORGELIVE-DOCKER-OK`,0.35s;非零退出码
透传测试 7);firecracker 真 VM 回归 PASS;arch 8/8;`forge accept` ACCEPTED。

## Sprint 70（✅ 完成）— Sandbox 配置即用(自动接线)

`SandboxConfig` 增加 **auto-wire**:声明 `Type` + 配置字段即可运行,无需
手动注入 runner —— `{Type:"docker", Image:"alpine:latest"}` 或
`{Type:"firecracker", Kernel, Image(rootdir)}`。sandbox 逻辑从
command_executor.go(528 行)拆到独立 `sandbox_config.go`(gate 500 行上限)。
验证:auto-wire docker 真容器 PASS(AUTOWIRE-DOCKER-OK,0.35s);未知类型
(podman)/不完整 firecracker 配置 fail-closed(KindConfig);firecracker 真 VM
回归 PASS;`forge accept` ACCEPTED。

## Sprint 71（✅ 完成）— CLI 沙箱接线(`forge run --sandbox`)

`forge run` / `forge evolve` 新增 `--sandbox docker|firecracker` +
`--sandbox-image`(+ firecracker 的 `--sandbox-kernel`):flags → runOpts →
`orchestrator.SandboxConfig` → auto-wire 真实 runner。空 `--sandbox` 保持
宿主执行。验证:sandboxConfig 映射单测(三类型 + 空);orchestrator
auto-wire 真容器测试(既有);executor.go 485 行/gate 500 上限;arch 8/8;
`forge accept` ACCEPTED。CLI 端到端(完整 workflow + 容器内 claude)由组件
测试覆盖,诚实标注。

## Sprint 72（✅ 完成）— Wave 并行计划 CLI 命令

`forge graph-scheduled-ready-nodes --control FILE --schedule-sha256 SHA256
[--predecessor-receipt FILE...]` 输出拓扑就绪的 successor 节点 ID 清单
(JSON 数组)—— wave 并行编排的**计划视图**:消费 receipt 集后,同 wave 的
所有就绪分支一次列出(serial 序),调用方对每个节点用既有
graph-scheduled-node-contract(--target-node 已支持)生成候选并 admit。
effect-free;错误路径:无 receipts/未知节点 receipt 一律拒绝。
测试:diamond 消费 frontend 后就绪=1 节点、无 receipt 拒绝、drift receipt
拒绝;`forge accept` ACCEPTED。

## Sprint 73（✅ 完成）— ai-batch-runner 高维特性移植 + wave-admit 编排命令

(1) **工具移植**(docs/ai-batch/ + docs/reviews/):十阶段评审框架
(run-review.py 脱钩版:review_core/agents/bounds 拆分 ≤500 行)+ 10 阶段模板 +
31 角色 prompt + 高维分析工具薄壳(pi-batch.py:classify/rules/assess/eval,
依赖闭包 8 模块全部 ≤500 行,零外部依赖,PyYAML 可选)。engineering-principles
适配 ForgeOS 版(证据权威表指向 .agent/ 与 harness/)。
(2) **高维分析应用**:用 assess 评估 wave 并行调度场景 —— 8/8 维度完整、
**工作流 L3_platform(9 分)**、产品化 L3(克制:低成本预留 tenant 类字段、
高成本 Billing 禁止提前设计);画像 frontend_ui 为关键词误判(诚实记录)。
(3) **wave-admit 编排命令**(Rust CLI):`group graph run scheduled-contract
wave-admit GRAPH_RUN_ID --schedule-sha256 ... --predecessor-receipt ...`
—— 调 Go core ready-nodes 计划 → 逐节点 --target-node 生成候选 → admit
落库。集成测试 3/3(真 Go core 全链路:计划→物化→证据链防线正确拒绝伪造
receipt;参数拒绝;drifted receipt 零落库)。fanin 30(经
scheduled_contract_command 复用 service 构造)。
`forge accept` 为 **ACCEPTED**;Rust 920 tests;arch 8/8。

## Sprint 74（✅ 完成）— 十阶段评审驱动修复(架构评审发现 4 项)

用移植的评审框架对 graph successor 协议 + sandbox 做真 agent 架构评审
(Stage 01,~1500s),发现并修复:
(1) **High — v20 per-node 不变量未实施**:`reject_existing_candidate_identity`
  仍保留 per-run/per-schedule 一次性检查(串行链遗墙),第二个 successor
  候选必拒。修复:移除 run/schedule 级检查,保留 per-node/per-ordinal/
  per-request 槽位;新增 serial-three fixture(3 节点)+ 同 run 双候选
  admit 测试(**sso 在 backend 之后同一 run 成功 admit**,3/3 PASS)。
(2) **Medium — 文档漂移**:BuildSuccessor/包文档/命令文档仍写
  "contiguous prefix / ordinal order"(Sprint 66 已实现拓扑选择)。
  修复:全部改写为 ADR-0035 拓扑就绪语义。
(3) **Low — 死代码**:`successorPredecessorsCovered`(Go)+
  `find_by_schedule`(Rust rows)删除。
**诚实记录下一阶段**(评审建议,非本 sprint):v21 迁移(provider-request v18
表 UNIQUE(graph_run_id) 与 lifecycle v16 表 UNIQUE(graph_run_id) 改为
per-(run,node,attempt)—— effectful 多节点 dispatch 的前提)与 ADR-0035
receipts 应只含 direct predecessors 的协议对齐(Go 过滤 + Rust 校验放宽 +
v20 CHECK 连锁)。
`forge accept` 为 **ACCEPTED**;Rust 921 tests;评审产物:
docs/reviews/reviews/forgeos-review-context/stage-01.out.md。

## Sprint 75（✅ 完成）— ADR-0035 证据绑定对齐 + SQLite v21

架构评审 Finding 2(Medium)完整修复:
(1) **Go 侧**:`buildSuccessorRequest` 现在只携带**直接前驱**的 receipts
(ADR-0035:同 wave 兄弟是进度证据但不被候选携带);`validate.go` 放宽
(空直接前驱集允许 0 receipts,覆盖校验保留);diamond fixture 的 backend
候选 = 0 receipts。
(2) **Rust 侧**:`predecessors_valid`/`ordinal_slot_valid`/
`predecessor_count_valid` 同步放宽(required 覆盖为核心,receipts 0..=31)。
(3) **SQLite v21**:successor_candidates 表重建,CHECK 从 1..31 放宽到
0..=31;完整 schema 链(版本 21、结构 digest 与 v20 相同 —— CHECK 不参与
结构签名、future-schema 22、dispatch re-entry 12..=21)。
(4) **测试**:diamond fixture(frontend/backend 同 wave 0 前驱)+ 0-receipt
候选全链落库 + **wave-admit 成功路径**(backend Created + successor show
可查 —— 之前证据链拒绝只因 0-receipt 候选无 lifecycle 依赖)。
`forge accept` 为 **ACCEPTED**;Rust 922 tests。
**诚实记录下一阶段**(评审 Finding 1b):provider-request(v18)/lifecycle(v16)
表的 per-run UNIQUE 迁移(per-(run,node,attempt))—— effectful 多节点
dispatch 的前提,影响 dispatch 语义,独立切片。

## Sprint 76（✅ 完成）— SQLite v22:effectful 多节点 dispatch(评审 Finding 1b)

架构评审 Finding 1b 完整修复 —— v18/v16 的 per-run 一次性墙移除:
(1) **迁移**:provider-request 表去掉列级 UNIQUE(graph_run_id/schedule_id),
保留表级 per-node 槽位;lifecycle 表去掉列级 UNIQUE(graph_run_id),加
UNIQUE(graph_run_id, node_id, attempt)。单 batch(provider+lifecycle),
完整 schema 链(v22、digest 0x0167…、future 23、re-entry 12..=22)。
(2) **store 适配**:provider-request identity 检查改 per-node 槽位;run
binding 校验多行遍历,每行用**自身 contract 轻量解码**(避免递归 —— 初版
复用调用方 contract 导致 body 校验对错契约 + 栈溢出,调试定位后修复);
lifecycle binding 同。
(3) **测试**:diamond 双节点(initial + backend zero-receipt successor)在
同一 run 各落 provider-request(v18 会死锁第二个,10/10 通过);
every_matching_identity 改 per-node 槽位语义。
`forge accept` 为 **ACCEPTED**;Rust 923 tests;gate/arch/clippy 全绿。
**里程碑**:wave 并行从"计划+候选"打通到"多节点 provider-request 落库",
effectful 多节点 dispatch 的存储层前提完成。

## Sprint 77（✅ 完成）— ADR-0036 归档 + wave 全链路核对

(1) **ADR-0036**(docs/adr/0036-wave-parallel-storage-and-orchestration.md):
记录 v20–v22 三版迁移(候选 per-node 槽位、零 receipts 候选、effectful
多节点 dispatch)、编排命令(ready-nodes / --target-node / wave-admit)、
失败语义与备选方案(v21 单 batch 原因、binding 轻量自载避免递归)。
(2) **per-run 假设核对**:Go graphdispatch 与 dispatch execute/release/
readiness 链全部按 request_id 操作,无 per-run 单 request 假设;
`has_graph_run_child`(删除保护)语义 = 任一子记录即阻止,多节点下仍正确。
(3) **binding 多行路径验证**:双节点 provider-request 测试中 prepare 的
create → run inspect → binding 遍历已隐式通过(10/10)。
`forge accept` 为 **ACCEPTED**;Rust 923 tests。

## Sprint 78（✅ 完成）— Stage 04 实现评审 9 项修复

第二轮独立评审(Stage 04,wave 存储 + 编排)发现 10 项,修复 9 项:
(1) **F2(Medium)**:wave-admit 硬编码 provider/model/预算/假 pricing digest
(cccc…)→ **结构性不可 dispatch**。修复:9 个执行选项 pass-through flags
(镜像 Go core 命令),缺省 fail-closed(usage 错误)。
(2) **F3(Medium)**:--idempotency-key 被接受但丢弃。修复:确定性 per-node
key `{key}-{node_id}`,重跑 Replayed 而非 rejected。
(3) **F1(Medium)**:validate_graph_run_binding 注释"返回第一个"实际返回
最后一个。修复:改 Result<()>,遍历校验无返回值。
(4) **F4**:rejected 非空 → 非零退出码(JSON 保留)。
(5) **F5/F7/F8/F9/F10**:re-entry 消息 12..=22、ReadySuccessorNodes 空集
返回 [] + exit 0、v22 迁移去重复 PRAGMA、测试注释、加载器共享文档
(递归原因 ADR-0036)。
**F6 留档**(fan-out fixture + 双节点 wave-admit E2E,~0.5 天)。
`forge accept` 为 **ACCEPTED**;Rust 923 tests;gate/arch/clippy 全绿。
评审产物:docs/reviews/reviews/wave-storage-context/stage-04.out.md。

## Sprint 79（✅ 完成）— Stage 02 安全评审 5 项 + pass-through 验证

第三轮独立评审(Stage 02 安全协议,wave 存储 + 编排):
(1) **F1(High)**:v17→v18/v21→v22 迁移在**有 dispatch 历史**的库上 FK 失败
(测试从未用有数据库)—— 修复:迁移事务内 `PRAGMA defer_foreign_keys = ON`
(COMMIT 时统一校验最终一致状态)。
(2) **F2(Medium)**:successor admit 的 predecessor 证据**不绑定 graph_run_id**
(跨 run 证据重用,Go 侧已校验)—— 修复:lifecycle 的 run == candidate 的
run 检查。
(3) **F3(Low)**:INSERT_REQUEST_SQL 的 `A OR B AND C` 优先级 bug → 括号。
(4) **F4(Low)**:派生 idempotency key 溢出 256 字节 → bound。
(5) **F5(Info)**:数据承载迁移测试 —— 完整 FK 父链构造成本过高,诚实记录
N/A(修复已按评审建议实现,空库迁移回归全过)。
(6) **F2 验证测试**(Stage 04 遗留):wave-admit 候选携带执行选项断言
(provider.model/endpoint/pricing == 传入值,非字面量)。
`forge accept` 为 **ACCEPTED**;Rust 924 tests;gate/arch/clippy 全绿。
评审产物:docs/reviews/reviews/wave-storage-context/stage-02.out.md。

## Sprint 80（✅ 完成）— Stage 03 分布式评审 + SQLite v23

第四轮独立评审(Stage 03 分布式/数据库)发现 High:adjudicate 是死的
(ADR-0034 实现自 Sprint 65 起 UPDATE 引用不存在的 adjudicated_at_ms 列,
且 v22 重建 lifecycle 表时丢失 v19 的 status 'adjudicated' —— 任何
adjudicated 行会使 v22 迁移失败、库永久打不开)。
(1) **v23 迁移**:lifecycle 表重建,status CHECK 恢复 4 状态 +
adjudicated_at_ms 列(status='adjudicated' 时必填,否则 NULL);
完整 schema 链(v23、digest、future 24、re-entry 12..=23)。
(2) **adjudicate 激活验证**:UPDATE 的列/状态在 v23 表上被接受
(0 行 UPDATE 验证 SQL 合法性)。
(3) **F4(Medium)**:claim 幂等补 replay-equality 校验(同 key 不同输入 →
Conflict,不再静默 AlreadyClaimed)。
(4) **F5 机制测试**:defer_foreign_keys 使 DROP-parent-with-children 在
单批次内成功(精确复现评审场景)。
(5) **cli_usage** 补 wave-admit 完整用法。
`forge accept` 为 **ACCEPTED**;Rust 925 tests。
评审产物:docs/reviews/reviews/wave-storage-context/stage-03.out.md。

## Sprint 81（✅ 完成）— 多写并发测试 + Stage 03 遗留清理

(1) **多写并发测试**(Stage 03 F2):diamond 双节点(initial + zero-receipt
backend)的 provider-request 从**两个线程并发 prepare** —— 两行同时落库,
ordinal [0,1] 齐全,wave 并行并发安全实证(BEGIN IMMEDIATE + WAL 单写者
串行化正确)。
(2) **版本文案清理**(Stage 03 Low):只读打开错误消息
"current schema version 18"/"11..=21" → 23;CLI 测试断言同步。
(3) **共享 fixture**:diamond_run_with_two_contracts 提取到 support
(并发/adjudicate/双节点测试复用)。
`forge accept` 为 **ACCEPTED**;Rust 927 tests。

## Sprint 82（✅ 完成）— Stage 03 F3:claim 读路径 O(n²) 优化

claim 的 pristine 门原本走全量 `inspect_in_snapshot(run)` —— 该路径遍历
run 的全部 sibling provider-request 行并解码每个 body(上限 16MiB),
每个 lifecycle 操作 O(nodes × body)。新增轻量
`inspect_pristine_in_snapshot`(run record + 事件计数,跳过 binding 校验链),
pristine 门所需字段(record 全等比较、last_event_seq、事件数)完整覆盖;
完整 binding 链保留在全量 inspect 路径(数据完整性防线不变)。
terminalize/adjudicate 本就不查 run,无改动。
`forge accept` 为 **ACCEPTED**;Rust 928 tests。

## Sprint 82b — F3 优化提交 + F4 测试边界诚实记录

(已随 Sprint 82 提交)。F4(claim replay-equality)修复已实现;其端到端
测试需要完整 ClaimGroupAgentScheduledNodeDispatch fixture(release_control/
authorization/pricing 为 Go 生成结构,store 层无构造器,application 层
已有)—— 诚实标记 N/A(与数据承载迁移测试同类)。并发/adjudicate 测试
(2 个)在新文件保持通过。

## Sprint 83（✅ 完成）— Sandbox 专项评审 13 项修复

对未评审的 sandbox 领域做第五轮独立评审(firecracker/docker runner +
CLI 接线),发现 14 项(4 High 全真实),修复 13 项:
(1) **F1(High)**:sandbox 执行**丢弃 claude prompt** —— prepareInput 剥离的
stdin 从未传给 runner(sandbox 对主要用途是死的)。修复:Runner 接口加
stdin 参数,4 处实现 + 接线;docker cmd.Stdin、firecracker /forge-stdin
注入 + guest 重定向;PromptViaStdin 接线测试。
(2) **F2(High)**:guestOutput 剥任意 "] " 破坏输出(实机验证
"LEFT] RIGHT"→"RIGHT")。修复:只剥内核时间戳前缀(正则式数字)。
(3) **F3(High)**:sandbox 绕过 MaxOutputBytes(docker 无界 Buffer、
firecracker 无界 serial.log)。修复:cappedWriter + 64MiB 限读。
(4) **F4(High)**:MemoryMB 声明但从未应用。修复:machine-config PUT
(mem_size_mib)。
(5) **F5-F7(Medium)**:取消→KindFailed(typed errors.Is)、marker 读错
有界重试、docker 超时孤儿容器清理(docker rm -f,实测 --rm 不停止)。
(6) **F8-F11/F13**:死代码/死分支/注释错位/dry executor 警告/ROADMAP。
F12(sandbox 包零测试)与 F14(隔离强度)记录为后续。
`forge accept` 为 **ACCEPTED**;Go 全量 0 FAIL。
评审产物:docs/reviews/reviews/sandbox-context/stage-04.out.md。

## Sprint 84（✅ 完成）— Stage 06 生产就绪评审修复

第六轮独立评审(生产就绪:部署/备份/恢复/迁移/运维)CONDITIONAL GO,
条件项修复:
(1) **High — backup-before-upgrade**:不可逆迁移前自动快照现有 hub 到
`state/backups/hub-v<N>-before-upgrade-<ts>.sqlite3`(新建库 version=0
不备份);测试:降级到 v14 → 打开迁移 → 断言备份存在且版本 14。
(2) **Medium — docker exit-125**:daemon 故障(125)不再作为 guest 判定,
分类为 config fault。
(3) **High — readiness/日志**与 **Medium — 有数据迁移测试/--allow-migrate
门**记录为后续(需要 CLI/部署层设计)。
(4) **F1 真实验证**:docker stdin(-i 标志)与 firecracker 真 VM stdin
(1.45s boot)均回显 prompt —— prompt 传递链路在两种隔离运行时实证。
`forge accept` 为 **ACCEPTED**;Rust 929 tests;Go 全量 0 FAIL。
评审产物:docs/reviews/reviews/production-context/stage-06.out.md。

## Sprint 85（✅ 完成）— 双语言单一事实源(spec md 驱动实现)

forge-core(Go)与 forge-runtime(Rust)的 scheduled successor 协议由
**同一份权威 spec** 驱动:
(1) **docs/contracts/scheduled-successor-protocol.md**:协议版本/域分离
digest 域/边界/不变量/身份前缀的唯一定义(变更流程:先 ADR,双侧测试
同步,三者不一致 = 缺陷)。
(2) **harness/spec_check.py**:md 表格 → 键值解析器(标题/分隔行/空行
处理;bounds 表输出 min/max;已入 scaffold COPIED_FILES 清单)。
(3) **Go 一致性测试**(5 个):版本/域/边界/前缀常量 vs spec;validate.go
边界字面量提取为命名常量(maxSuccessorOrdinal 等)。
(4) **Rust 一致性测试**(3 个):版本/域/字节边界 vs 同一 spec。
任何一侧漂移 → 测试失败 → forge accept 拒绝。
`forge accept` 为 **ACCEPTED**;Rust 932 tests;Go 0 FAIL。

## Sprint 86（实现完成；⚠️ 默认验收受宿主 Rust 工具链与既有 Go lint 基线阻断）— AI 可移植性、successor 证据闭环与 Sandbox 资源边界

(1) **AI 离线工具可移植性**:`docs/ai-batch` 补齐 system-type methodology、
内建 eval fixtures、build-routing fallback 与统一 canonical `path_base`。rules
check 现在校验 effective built-in/overlay registry；四个公开子命令从任意 cwd
运行，完整复制到无 `.agent` 的临时目录后仍可在 `python -S` 下完成
classify/rules/assess/eval。外部绝对 validator 被移除；runner-only validator/
agent 配置明确标为本移植不执行的样例，不能冒充 standalone 能力。

(2) **有界 predecessor dataflow / storage**:前驱正文固定 ≤1 MiB，Prompt 按
最坏 UTF-8/模板开销精确守界，successor candidate 固定 ≤8 MiB；SQLite v24
只提升 successor row，initial candidate 保持 4 MiB。v24 同时把 successor
ordinal、required/receipt count 等式写入当前 DDL CHECK，迁移前后继续按 exact
catalog/DDL contract 失败关闭。

(3) **scheduled successor 生产闭环**:Go 离线只接受 canonical、identity-bound 的
`completed`/result-shaped receipt，调用方 receipt 文件可任意顺序，但 candidate 始终按
schedule 的完整直接前驱顺序 canonicalize；缺失、重复、无关、failed、伪造
artifact identity 全拒绝。显式 `--target-node` 使空直接前驱的 ordinal>0 节点可在
零 receipt 上就绪，且绝不回退为 initial；正文只绑定 canonical 第一直接前驱。
Rust admission/re-entry 再要求 receipt 与 durable terminalized lifecycle exact match，
并复验 manifest 的 Project/member/profile、system/user Prompt 与 ordinal 1..31。真实 CLI/SQLite 链已覆盖
wave admission → successor show → provider-request prepare/show；production
prepare/release/readiness/effectful dispatch 都可解析 initial 或 successor，且多节点
list 允许共享 Run/schedule、只拒绝重复 node/ordinal slot。

(4) **Sandbox 资源与并发边界**:`--sandbox-memory-mb` 默认 512 MiB，范围
64..32768；Docker/Firecracker 都继承 executor output cap（默认 10 MiB）并显式
报告 overflow。Docker readiness 共用总 deadline，named container 由独立 2s
cleanup context 精确回收。Firecracker 从 prerequisite/rootfs build 起算总 deadline，
PATH 工具解析与 `/dev/kvm` read/write 前提失败关闭，serial 只作 bounded in-memory
capture；模板 regular file 分块、可取消复制，FIFO/device/socket 与注入 symlink
拒绝。并行 auto-wire 只写 receiver-local config，不再竞态修改共享 Runner interface。

(5) **fresh-context 收口**:独立 reviewer/protocol 子审计推动修复了 successor
source binding、真实 SQLite provider 链、共享 Run/schedule list、wave 相对路径与
stdin/重复 flag、Unicode idempotency、Docker preflight deadline、sandbox typed error
classification、Firecracker template 与 auto-wire race；最终报告
Blocker/Major/Minor = 0。

验证：AI smoke 9/9、Python harness 74/74、Node arch/scaffold 72/72；Go
`test ./...`、`vet ./...`、`build ./...` 与 graph/sandbox runner race 全绿。Rust
隔离复验 domain 58/58、application 45/45、CLI unit 150/150、wave/provider-request
E2E 5/5，相关 strict clippy 全绿。protocol 子审计通过生产 v1→v24 schema open，
证明新 DDL 可解析并完成整链迁移；但主 checkout 的 v24 adversarial 定向用例因
离线未缓存 `assert-json-diff 2.0.2` 而未启动。默认 `forge accept` 最终为
6 PASS / 4 FAIL / 1 N/A：test/typecheck/build 的 Go 路径通过，Rust 路径被 PATH 上
Cargo 1.83 无法解析 edition 2024 / 项目 `rust-version = 1.93` 阻断；lint 还同时
暴露 harness 从无 `go.mod` 的仓库根调用 golangci-lint（exit 7），而在
`forge-core/` 正确运行会报告 62 个既有 HEAD finding。当前增量在该模块内以
`golangci-lint run --new-from-rev=HEAD ./...` 复验为 0 issue。这些阻断均未伪报 PASS，
也没有联网、降低 manifest 工具链要求或顺手扩张成全仓历史 lint 清理。

## Sprint 87（规划落地，runtime 未实现）— AI Engineering OS 全流程能力与治理知识模型

用户要求把长期维护型 AI 软件工程团队的全部流程节点、具体职能、Skill、工程规则和演化机制固化为可实施规划。
本 Sprint 先做架构与需求采纳，不把“写了文档”冒充代码能力：

(1) **能力中心化组织**:ADR 0037 决定用「00–16 生命周期决策节点 × 可复用 Capability/Skill × 显式
CapabilityGrant」装配临时 Agent，拒绝按职能名称无限增殖永久 Agent。规划、实现、审查、批准、生产操作分权；
低风险流程可裁剪，高风险职责分离。

(2) **完整节点 SOP + Reflection**:`docs/design/ai-engineering-os/` 逐项定义 Orchestrator、Requirement/BA、Product、UX/UI、
Domain、Architecture、Data、API、Planning、Development、Review/Refactoring、Security/Privacy、QA、
Performance/Reliability、Release、Operations/SRE、Reflection/Evolution 的入口、输入、细项职能、Skill、产物、规则、门禁、
禁止项、权限、升级、退出、交接与记忆写回；另以 38 个可组合 Skill 包逐项列出 trigger、output、rule、automation、
forbidden 和统一 production-ready Checklist。Meta Reflection 每次 R0、L2 R1、L3/L4 R2，evidence-first Critic 只提交
Claim/Debt/Eval/Rule/ADR/New WorkIntent proposal 与 RoutingReceipt，不直接自改系统。

(3) **AADM 决策内核与能力收敛**:ADR 0038 把 CognitiveAtom、TransactionProposal/AuthorizedTransactionSpec、append-only
Attempt/receipt、InteractionEvent、Capability/Artifact ABI、typed hypergraph、Rule Field、pre/effective DiscretionEnvelope、
constraint/Pareto、rolling Controller 与 DecisionCapsule 固化为目标 Kernel。140 个 lifecycle fine capabilities 已由
`capability-skill-map.v1.yml` 完整、无重复地映射到 38 个 Skill primary owner；CLI/Web/API 未来只作 adapter。

(4) **治理知识模型**:规划 Evidence/Claim、可重建 System Knowledge Graph、两阶段 ImpactPreScan→final Assessment Join、ADR v2、
Technical Debt、typed Engineering Constitution、Software Health、content-addressed Context、CapabilityGrant、
Approval/RiskAcceptance、KnowledgeUpdate proposal/receipt、Review/Conflict、封闭 Transition 状态机与 RuntimeObservation/
EvolutionCandidate。Fact/Decision/Inference/Assumption/Proposal/Unknown 分层；缺边必须 UNKNOWN，Agent/PDP/Approver/Operator
权威与认识上限闭合。

(5) **工程规范**:God File 用 size/complexity/change coupling/cohesion/responsibility/effect/test pain 联合判定；
重构按变化原因、characterization test、seam 和渐进迁移。OOP、DI、AOP、DDD、Strategy、Event、Repository、CQRS、
数据迁移与前端拆分均有适用/不适用条件；当前 500/50/零循环继续是硬门，其余指标默认 review trigger。

(6) **Device Fabric 预留**:ADR 0039 采用 default-off ExecutionTarget/Attempt/Artifact/Lease/Fencing/Placement/Reconciliation
抽象，先保持 Local adapter，再分期 Inventory/Observe、verified-sandbox SSH、mTLS Runner、Scheduler、safe migration，
Federation 最后。身份、attestation、数据驻留、egress、LOST/INCONCLUSIVE、workspace delta/CAS 与外部 OperatorReceipt/G8
均失败关闭；现有 Docker/Firecracker 不是远程 Fabric。

(7) **分期与诚实边界**:ROADMAP 采纳 Wave 0B–7，先 Governance/Decision Kernel、Context/Registry/Local ABI，再 Graph/
Impact、Engineering Memory、Skill/Review、Reflection/Evolution，最后 default-off Device Fabric/企业扩展；`.agent` 保持可执行
主干，不另建第二 DAG。所有新目录明示 `planning_only/executable:false`；功能需求审计新增 ADR 0037–0039 的
`ADOPTED-PLANNED`，远程生产 effect 边界不变。

验证：规划目录严格解析为 17 个 `00–16` 节点、每节点 14 个统一字段；145 个 capability references/140 个唯一 fine
capabilities 精确映射到 38 个 Skill primary owner（无 missing/extra/duplicate）；14 个设计/ADR Markdown 的本地链接均解析，
全部新增设计产物 ≤500 行，`git diff --check` 通过。`node harness/gate.mjs` PASS（1303 files），
`python3 -B harness/check.py` PASS（12 checks），`go test -count=1 ./...` 全绿，完整 acceptance 中 forge-core 1379 tests、
examples 22+47 tests 通过。

完整 `node harness/acceptance.mjs` 诚实结果仍为 6 PASS / 4 FAIL / 1 N/A：test/typecheck/build 的 Rust 路径被 PATH 上 Cargo
1.83 无法解析 edition2024（项目要求 Rust/Cargo 1.93）阻断；lint 同时有仓库根 golangci-lint exit 7、ruff/eslint 未安装与
同一 Cargo 解析失败；coverage 维持 N/A。与本轮 planning-only 文档无因果关系，未通过降级 manifest、联网或伪报 PASS
规避。fresh-context 终审最终 APPROVED，Blocker/Major/Minor = 0；本轮没有创建空壳 Agent/Skill、没有改 runtime、没有调用付费模型、连接
远程设备或外部生产系统。

## Sprint 88（✅ contract/shadow 切片完成；runtime 路由仍未启用）— Machine-readable Agent Engineering 规范

用户要求把 Prompt/Context/Memory/Tool/Planning/Loop/Reflection/Graph/Harness/Evaluation/Knowledge/Evolution/State/Contract
Engineering 从长 Prompt 收敛为 Agent 可稳定消费、系统可检测、结果可审计、经验可治理的工程规范。ADR 0040 继续复用
`.agent` 主干、既有 Capability/Skill catalog 和 `forge accept` 单一完成权威，不建立平行 `.agent-engineering` 或第二 DAG。

(1) **七类 shadow 合同**:`activation.yml` 冻结 v1 refs/默认值；`disciplines.yml` 精确记录 14 学科状态；`rules.yml` 提供 11 条
分级原子规则；`detectors.yml` 把 automatic Error 绑定到 `forge accept` 真实 load-bearing probe；`context-routes.yml` 使用 typed
predicate、固定 route order/信任/required/deny 合并代数和 budget 失败语义；`workflow-profiles.yml` 固化 W0–W3 的独立保障
下限；TaskEvidencePackage 只保存 source-bound 结构化观察。两个既有 planning-only Capability/Skill catalog 被直接引用并检查
140 个 capability 的唯一 primary ownership，不另造能力命名空间。

(2) **单一完成真值**:证据包顶层禁止 `status/completed/accepted/verdict`，执行观察必须包含 argv、exit-code 语义、output digest
和同源 tree digest，未执行/N/A 必须给原因；它仍不等于可信执行证明，也不产生放行结论。standalone package validator 保持
shadow，TRUTH-001 因尚未接入 `forge accept` 诚实降为 Review；只有 `forge accept` 能输出 ACCEPTED/REJECTED。

(3) **detector/Context/profile 对抗收紧**:仅“checker 路径存在”不再算执法；validator 固定 automatic detector 的 argv、adapter、
criterion、load-bearing/fail-closed 接线和正反测试，并静态确认 probe 在 `acceptance.collect()` 中实际调用。Context 拒绝自由 keyword、
绝对/`..`/shell glob、未知 predicate/lane/overflow 策略与 instruction-lane 越权；W0–W3 除相邻单调外还各有不可整体删除的保障
floor，gate vocabulary 直接复用 `modes.yml`。

(4) **旧项目可升级**:`activation.yml` 规定缺少 project-level `engineering_spec` 的 ADR-0040 前项目默认为 shadow；
`forge-upgrade` 仍不触碰 identity `project.yml`，却会复制新合同、catalog、validator 和 tests。专门回归从移除全部新资产和绑定的
legacy fixture 升级，证明 `project.yml` 字节不变且升级后 `forge check` 通过。fresh scaffold 则生成显式 canonical binding。

(5) **事实纠偏与研究依据**:`docs/ai-batch/mechanism/REFLECTION.md` 曾把不存在的 `pi-batch reflect`、R0–R2 runtime 和 ledger
写成已实现，现已改为候选接口。ADR 0040 记录 OpenAI Harness/Agent Loop、Anthropic Context/Tool/Effective Agents/Eval、
GitHub scoped instructions、MCP typed tools与 LangGraph persistence 的一手资料，并保持 `AGENTS.md` 为短路由入口。

最终验证：Agent Engineering 对抗测试 **52/52**、完整 Python **126/126**、完整 Node（含 scaffold/upgrade）**379/379**、
Forge Core `go test -count=1 ./...`、`forge check`（13 checks）、`forge gate`（1474 files）、8 项 architecture check 与
`git diff --check` 全绿；fresh scaffold 与 legacy upgrade 回归均通过。fresh-context Reviewer 用 13 个恶意 mutation 复核
execution/learning autonomy、stop/human gate/repair、Context base/budget/trust/security trigger、probe argv/forced PASS、Evidence
identity bounds 和 automatic Rule 反转，均被 validator 失败关闭；终审 Blocker/Major = 0。

完整 `node harness/acceptance.mjs --json` 诚实结果仍为 **6 PASS / 4 FAIL / 1 N/A**：`test_pass`/`typecheck`/`build` 的 Rust
路径被宿主 Cargo 1.83 无法解析 edition 2024（项目要求 Rust/Cargo 1.93）阻断；`lint` 同时记录 golangci-lint exit 7、
ruff/eslint 未安装和同一 Cargo 解析失败；coverage 为 N/A。Go、两个 example app、结构、治理、架构、secret 与 SCA 均真
PASS。未通过降级 manifest、忽略项目或伪报 PASS 规避宿主限制。

## Sprint 89（✅ contract/shadow 切片完成；pre-code runtime gate 未启用）— Backend Engineering Decision Standard

用户要求把资深后端、数据、分布式系统和长期架构经验从长篇建议收敛为 Agent 可执行的思考规范，尤其把持久化对象、
数据身份、业务不变量、事务/并发、网络可靠性、10×/100× 容量和演进成本放到编码之前。ADR 0041 延续 ADR 0040 的
单一治理主干与诚实边界，没有创建第二套 DAG、完成权威或一批空壳 Agent。

(1) **后端决策合同**:`backend-decision-gates.yml` 固化 16 类触发器及逐类 L1–L4/W1–W3 下限、14 步因果顺序、14 个决策维度、
低可逆决策控制和十维 Production Readiness vocabulary。每个维度只能 `addressed/not_applicable/blocked`；触发器要求的
维度不能 N/A，主键/所有权/契约/权限等承重未知必须保留 blocked。

(2) **条件化模型边界**:Request DTO、Command、Domain、Persistence、Read、Response 与 External Service 被定义为语义角色，
而非强制目录。只有 owner、变化原因、安全分类或公共/持久化耦合不同时默认分离；简单内部 CRUD 可使用较少角色，但禁止
公共 API 直接暴露 ORM。OOP、FP/柯里化、DI、AOP、DDD、CQRS/事件等均要求适用证据，不能以“最佳实践”机械套用。

(3) **持久化前置关卡**:规范要求先确定业务/内部/外部/幂等身份、金额/单位/时间/NULL、状态/历史/快照、关系/约束、
访问路径/索引、并发、租户/隐私、删除/归档/修复和 expand–migrate–contract，再生成 ORM/DDL；并把 deadline、唯一重试层、
未知结果、背压、容量、可观测性、RPO/RTO、团队认知、TCO 与删除路径纳入同一决策包。

(4) **十张密集 Skill adapter**:新增 backend、domain、data/transaction、migration、API contract、distributed reliability、
performance/capacity、observability、secure coding 与 architecture tradeoff；每张都有触发、输入、SOP、输出、禁止项、
自动化/验收和一手参考。data/backend Context route 按路径/capability 装载它们，未把每个知识名词变成永久 Agent。

(5) **无自批权 package 与对抗 validator**:`backend-decision-package.schema.yml` 记录 source tree/context digest，并把 policy、Schema、
逐项仓库文件证据、proof type/class/subject、事实/推理、假设、readiness 与 residual risks 分开绑定；递归禁止 `completed/accepted/approved/verdict/gate_result`。
独立 checker 校验 policy、Schema、Skill 和 package，覆盖缺/重复维度、触发维度伪 N/A、虚构/错摘要 proof、事实假设混淆、
低可逆/不可逆 kind 的 ADR 与 Reviewer 绑定缺失、readiness 越过 blocked decision、畸形输入 traceback、触发 floor 降级和伪完成等 mutation；
`harness/check.py`、fresh scaffold 和 legacy upgrade 都继承验证。

(6) **诚实边界**:detector 明示 `state:shadow`、`load_bearing:false`，当前只校验规范资产和手工提供的 package；逐项仓库文件
会被解析并重算字节摘要，但完整 source tree/context digest 尚未由 runtime 重算，系统也尚未从 diff 自动编译 package、签发
Evidence/Claim/Grant 或在 Coding 前 fail-closed。proof class/producer/Reviewer 仍是摘要绑定声明而非 runtime attestation；分类只报告结构有效/阻塞/未就绪/跳过待复核，最终完成仍只属于 `forge accept`。

验证：后端 package 专项 50 个测试、Agent Engineering 路由/合同专项 56 个测试、组合治理检查、8 项 architecture check、
fresh init 与 legacy upgrade 回归均通过。完整仓库验收及宿主工具链限制记录在本 Sprint 后续验证结果中。

## Sprint 90（✅ contract/shadow 切片完成；可信视觉与 pre-code runtime authority 未启用）— Frontend Design Decision Standard

用户要求把产品场景、信息架构、视觉风格、页面模式、操作链路、状态机、权限、Design Token、无障碍、响应式、动效、
React/Vue/Flutter/React Native 实现与截图审查从超长 Prompt 收敛为企业级 AFDS。ADR 0042 延续现有 Kernel、Context route、
Capability ownership 与 `forge accept` 单一完成权威，没有为 CMS/ERP/颜色/框架创建平行 Agent 或第二套 DAG。

(1) **可执行前端决策合同**:`frontend-design-gates.yml` 固化 20 类 L1–L4/W1–W3 风险 floor、五层规则权威、15 步设计顺序、
14 个决策维度、假设阻断阈值和十维 readiness。固定 8pt、14px、44px、390/1024/1440 与视觉 90 分均被纠正为 Profile、
平台或 advisory 选择，不冒充跨平台标准；WCAG、APG、DTCG、React、Vue、Flutter、RN 与 Playwright 的权威边界分开记录。

(2) **Profile×Pattern，而非 Skill 爆炸**:`frontend-profiles.yml` 提供 12 个产品 Profile 与 14 个页面 Pattern，CMS/OA/ERP/MES/
CRM/Analytics/Commerce/Marketing/Immersive/Data Wall/AI UI 的任务、密度、风险和动效策略与 list/form/workbench/wizard/editor/
dashboard/agent-chat 等结构正交组合。三张 canonical Skill adapter 分别负责信息与交互、Design System 与无障碍、框架客户端实现；
user-experience Context route 按路径/capability 装载，React Native 分类优先于 React。

(3) **操作链路与状态先于代码**:FrontendDesignPackage 要求业务任务、事实/假设、主/替代/错误/取消/恢复 flow、显式 state/action、
权限/数据/system guard、失败保留输入、异步重复/未知结果与高风险恢复。action 由业务状态×权限×数据条件×系统状态决定；
截图、视觉 diff 和高分不能覆盖主任务失败、越权、焦点陷阱、无障碍失败或数据丢失。

(4) **证据诚实与对抗 validator**:artifact 与 proof claim 分离并 exact-subject 绑定；verification case 与 claim artifact 集合必须
完全一致；逐项限制路径、字节、SHA-256、source revision、claim class/result，PNG 校验 chunk/CRC、critical chunk、PLTE/IDAT、
有界解压、scanline、32MP 和 viewport×DPR，禁止非 source artifact 跨 subject 复用或用同字节不同 ID 重复证明。50 个专项测试
覆盖 policy/schema 漂移、floor 降级、Profile override 风险、维度缺失/重复、假设冒充事实、状态/flow 悬挂、高风险缺恢复、
自审、摘要/路径逃逸、截图伪造/复用、not_executed 冒充正证据、公开 API 畸形输入与深层嵌套无 traceback；输出只允许
结构有效/阻塞/未就绪/跳过待复核，不产生批准或完成。

(5) **集成与旧 UI 资产收敛**:shadow detector、Context route、`forge check`、fresh scaffold 和 legacy upgrade 已接入；AFDS helper
收进 `harness/frontend_design/`，保留根 CLI adapter，避免突破 package 认知预算。旧 `docs/ai-batch` 修复 repo-root 规则路径、
React Native 最长匹配，并补 CRM/Commerce/AI Agent Profile 与 wizard/editor/canvas/chat/master-detail/settings/timeline/map Pattern。

(6) **诚实边界**:当前 checker 能证明合同、声明交叉引用和本地 artifact 当前字节，不证明 screenshot/trace 真由声明工具产生，也不证明
Reviewer 是独立真实主体；Context route 尚非 runtime selector，系统不会自动从 diff 编译 package 或在 coding 前签发权限。可信 Runner、
append-only ledger、签名 receipt、自动影响识别与 load-bearing gate 留给 Governance/Decision Kernel，最终完成仍只属于 `forge accept`。

验证：前端专项 **50/50**、完整 harness Python **247/247**、完整 harness Node（含 scaffold/upgrade）**379/379**、旧 UI
tests **15/15**、scaffold **34/34**、`forge check` 13 项、gate（1540 files）、architecture 8 项（1088 source files）和
`git diff --check` 均通过；Forge Core 普通测试/竞态/vet/build 全绿（acceptance 观察 1422 tests）。完整 acceptance 诚实为
**6 PASS / 4 FAIL / 1 N/A**：
宿主 Cargo 1.83 无法解析项目 Rust 2024（要求 Cargo 1.93），因而 test/typecheck/build 失败；lint 另有 golangci-lint exit 7、
ruff/eslint 缺失及同一 Cargo 失败，coverage 为 N/A。未降低 Rust edition、架构预算或任何门禁以伪造通过。

## Sprint 91（✅ compiler-backed shadow 切片完成；Vue/Dart 与 load-bearing promotion 未启用）— Frontend Code Architecture Governance

用户要求把高内聚低耦合、模块/public API、上帝文件、目录/复杂度、API/缓存/权限/错误/构建/发布等前端工程经验从零散阈值
固化为独立治理流程。ADR 0043 增加 `frontend-code-architecture` procedural Skill，但不创建新的 fine capability、第四个 AFDS owner
或平行 Agent 树；frontend-client、architecture、review 与 god refactoring 的 canonical ownership 保持不变，其余系统问题继续作为条件化 lens。

(1) **显式项目合同**:`.arch/frontend-architecture.v1.json` 要求 target、Compiler adapter、source/project root、完整/部分 ownership、
module/module-set、layer allowlist 与 public/test entrypoint；空 targets 只能得到 `not_applicable`。baseline 与 waiver 独立且 exact；方向、
所有权、循环和配置完整性既不能 baseline 也不能 waiver，通配、自批、过期和无删除触发器的例外失败关闭。

(2) **Compiler-backed detector**:`frontend.code_architecture` 使用项目 TypeScript Compiler API 与 tsconfig 解析 AST、alias、extensionless、
index、re-export/dynamic literal 和 test source，不用 regex 猜 import。图层执行 ownership/direction/deep-import/production-to-test/Tarjan SCC；
未解析内部 import 或不可用 adapter 返回 inconclusive。Vue/Dart adapter 尚未实现，配置目标时不会伪报 PASS。

(3) **复杂度不冒充语义**:LOC、declaration/import/export、state/effect/handler/branch、目录、模块文件数和 public API 数只输出 raw
review signal；God finding 至少命中三个信号族，且最终阻断仍需独立责任图、变化或行为证据。代码架构报告只允许
pass/fail/inconclusive/not_applicable，detector 明示 shadow/non-load-bearing，完成权威仍仅 `forge accept`。

(4) **接线与继承**:policy/Skill/standard 已进入 user-experience Context route、detector/rule registry、`forge check`、fresh init 和
legacy upgrade；项目所有的 JSON 主合同/基线/waiver 在 init 时播种，legacy 缺失时补齐，已有文件在 upgrade 中逐字节保留。路径触发覆盖
feature/entity/shared UI/API、CSS/SCSS/Sass/Less、theme 与 token。ADR、路线图、
功能清单和 AFDS/client Skill 已同步，未创建 API/error/CSS/build/release 等空壳 Skill。

验证：前端架构专项 **17/17**、Agent Engineering **63/63**、完整 Python **252/252**、完整 Node **398/398**、scaffold/legacy
专项 **28/28**、`forge check` 13 项、gate（1553 files）、architecture 8 项（1093 source files）、Go 普通测试/竞态/vet/build 与
`git diff --check` 均通过。fresh-context Reviewer 三轮驳回并推动关闭 project-instance 覆盖、非可豁免规则降级、
TypeScript 漏扫/借用宿主 compiler、partial ownership 以及 check-then-write TOCTOU；最终 **APPROVED**，无 Blocker/Major。
完整 acceptance 仍诚实为 **6 PASS / 4 FAIL / 1 N/A**：宿主 Cargo 1.83 无法解析项目 Rust 2024
（要求 Cargo 1.93），lint 另有 golangci-lint exit 7、ruff/eslint 缺失，coverage 为 N/A；未降低工具链、manifest 或门禁伪造通过。

## Sprint 92（✅ AFDS 声明式扩展完成；真实 Geometry Runner 与可信来源未启用）— Business UI Geometry Contract

用户要求 UI Agent 不再从组件树和 CSS 开始，而要先理解业务场景、使用角色、工作模式、任务路径、业务对象/状态、数据语义与风险，
再把这些关系编译为可追溯的几何构图、交互和代码。ADR 0044 扩展 ADR 0042 的 AFDS 主干；它没有创建第四个 capability owner、
平行 package 或新的完成权威。

(1) **ownership 不漂移**:`ui-geometry` 是条件化 supporting procedural Skill，只编排既有产物。角色/任务/信息架构/flow/state/action 仍归
`information-interaction-design`，Token/shape/optical/visual judgment 仍归 `design-system-accessibility`，框架实现与项目真实 Runner 仍归
`frontend-client-engineering`；产品类型继续使用 Profile×Pattern，不按 CMS/ERP/工作模式复制 Skill 树。

(2) **业务约束先于几何**:FrontendDesignPackage v1 顶层形状保持不变；layout decision 通过 exact `business_ui_composition` proof 绑定
`application/vnd.forgeos.business-ui-composition+json` source artifact。composition 复用既有 flow/state/action，并显式描述 view/work mode、
fact/computed/AI recommendation/derived display 数据语义、page state、region/axis/group、spacing/stroke/shape、responsive disposition、
load-bearing element 与 optical adjustment；裸尺寸/阈值必须追溯到项目或 Profile policy，不能冒充跨平台普适值。

(3) **report 不是执行权威**:项目配置的真实 Runner 可以附加 `application/vnd.forgeos.ui-geometry-report+json`，通过
`geometry_measurement_receipts` 绑定 exact composition、source/build/fixture/environment、runner、原始观察、policy tolerance 与结果。
`fail`、`inconclusive`、`not_executed` 或缺失测量不能被总分、截图或 pass 文案掩盖；Web DOM 模型也不自动泛化为原生平台。

(4) **确定性 validator 的诚实边界**:`harness/frontend_design/{composition,composition_support,geometry}.py` 只做有界 strict-JSON、引用、摘要、上下文和
声明一致性检查，不启动浏览器/原生客户端，不验证视觉重心、阅读动线、光学校正、业务任务成败或 producer/reviewer 身份。
Context route、AFDS schema/policy、shadow checker、专项回归与 scaffold/legacy-upgrade 沿用既有治理主干；当前 detector 仍
`shadow/load_bearing:false`，唯一完成权威仍是 `forge accept`。

本轮接线实跑 composition/geometry/coordinate 专项 **57/57**、其余 AFDS 合同/对抗 **51/51**、Agent Engineering **64/64**
（复审相关合计 **172/172**）、递归 Python **311/311**、完整 Node **398/398**、scaffold/upgrade 定向 **11/11**、
`forge check` **13/13**、gate（1569 files）、architecture **8/8**（1100 source files）与 `git diff --check`，均通过。
fresh-context Reviewer 先后复现并推动关闭负证据、subject coverage、trigger semantic floor、spatial ownership、幽灵角色、
recovery source/逐状态覆盖、axis reciprocity 和 L4 risk trigger 缺口；最终复审无 Blocker/Major/Minor，建议 ACCEPT。
完整 acceptance 仍诚实为 **6 PASS / 4 FAIL / 1 N/A**：宿主 Cargo 1.83 无法解析项目 Rust 2024，lint 另有
golangci-lint exit 7、ruff/eslint 缺失，coverage 为 N/A；未降低工具链、manifest 或门禁伪造通过。这些结果只证明本地
合同/引用/对抗回归，不升级为浏览器执行、可信 producer 或 UI 质量证明。

## Sprint 93（✅ canonical shadow kernel 完成；truth/authority/Hub 与 load-bearing promotion 未启用）— Evidence / Claim Governance Contract

用户要求把证据、声明、来源、派生关系与验证状态从自由文本提升为跨语言、可迁移、可审计的治理合同。本轮以 ADR 0045 冻结
`forgeos.canonical-json/v1`，保持 shadow/non-load-bearing：结构有效不等于事实为真、来源可信、声明获批或任务完成，唯一完成权威仍是
`forge accept`，也未引入新的 Hub、签名身份或持久化真值系统。

(1) **严格身份与 canonical wire**:EvidenceRecord 与 KnowledgeClaim 使用 ASCII snake_case、键排序、compact UTF-8、禁止 Unicode
控制/双向字符、signed int64、无浮点、无隐式 Unicode normalization；记录、集合、深度、字段、数组和字符串均有硬上限。摘要固定为
`SHA-256(domain + NUL + canonical record with empty self digest)`，使用小写十六进制；业务 subject、数据库式 ID、来源 locator 与
claim derivation 分开，跨 subject 派生允许，但自引用和环被拒绝。正向输出精确为
`STRUCTURALLY_VALID (shadow; no truth or authority attestation)`。

(2) **跨语言 codec 与单一合同**:JSON Schema、golden fixture、Python package/CLI、Go package及 Rust domain module 使用同一 v1 语义；
schema/fixture/registry 摘要固定，两个 golden record digest 分别为
`dc6963537f59e0594e6d5d1651e16070b81365ff379acc5ec09956b18e4b17b4` 与
`953b14819b50db73cdb3e1b523303c7c669a7e9bbeeacefcd89c4b25681da8ec`。Skill 经三组全新请求前向测试后，能区分 golden wrapper 与
checker record-set，按仓库前提选择 Python/Go/Rust 命令，并在缺少 `go.mod`/`Cargo.toml` 或受支持 Rust 1.93 时诚实标记未执行；
`--ignore-rust-version` 只允许诊断，不算正式通过。

(3) **有界输入与失效关闭**:普通 CLI、golden/schema/fixture pin 检查、composed governance detector 及 engineering YAML 入口均先
`fstat` 再最多读取上限+1 字节；超长整数 lexeme、深层 JSON、超大文件和 `MemoryError` 只产生受控错误，不 traceback 或无界分配。
repo locator 同时拒绝 POSIX 逃逸、绝对路径、反斜杠和 `C:/...`/`C:...` drive-qualified 路径。producer 在填入 64 字符 digest 前检查
最终 sealed record 上限，Python/Go/Rust 对 `MAX-64+1` 边界一致；Go typed-wire roundtrip 阻止 required integer `null` 被零值吞掉。

(4) **声明图与版本诚实**:claims 只能引用同一 record-set 中存在的证据或声明，引用图必须无环；subject 表示业务主体/图节点，不冒充
claim record ID。未来 provenance envelope 被明确列为新版本候选，不能以 v1 `kind` 写入，避免同版本 wire collision。当前 verifier 只
证明字节、schema、摘要和图结构，不证明 evidence 内容、外部 producer、reviewer 身份、时效或业务结论。

(5) **接线、scaffold 与兼容**:`governance-contracts.yml`、Context route、shadow detector/rule、`forge check`、fresh scaffold 与
legacy exact-allowlist upgrade 已接入；scaffold 同步 standard/ADR/schema/fixture/Skill/Python package/tests，并移除不存在的 ADR 0037
引用。为守住 500 行工程预算，governance wiring 从 `agent_engineering_check.py` 拆入高内聚 helper，而非压缩代码；`check.py` 的旧
YAML anchor 行为保持兼容，严格 engineering-spec loader 仍拒绝 anchor/alias。

(6) **复审推动的缺陷闭合**:独立复审与对抗 fuzz 关闭了 Python malformed/extreme shape 崩溃、文档 v1 冲突与 subject 歧义、Go
required-int null、sealed-size 边界、Windows drive locator、所有正式入口的有界读取、稀疏/内存异常 YAML、旧 checker anchor
回归和 scaffold dangling reference。第三轮全新上下文冻结树复审又复现 JSON parse/canonical/digest `MemoryError` 会穿透两个公开 CLI
模式；codec、record-set、golden 与 CLI 边界及注入回归全部补齐后，复审重跑结论 **ACCEPT**，0 Blocker / 0 Major / 0 Minor。

最终验证：递归 Python **350/350**、完整 Node **398/398**、Go `test`/`test -race`/`vet`/`build` 全绿；Rust governance 在已安装
1.92 + `--ignore-rust-version` 下 **13/13** 仅作诊断，不能冒充项目要求的受支持 Rust 1.93 结果。`forge check` **13/13**、gate
（1618 files）、architecture **8/8**（1132 source files）和 `git diff --check` 均通过。完整 acceptance 诚实为
**6 PASS / 4 FAIL / 1 N/A**：宿主默认 Cargo 1.83 无法解析 Rust 2024/项目要求 1.93，lint 另有 golangci-lint exit 7、
ruff/eslint 缺失及相同 Cargo 阻塞，coverage 为 N/A；没有降低工具链、manifest、schema 或门禁来伪造完成。

## Sprint 94（✅ 完成）— Local GovernanceRecordJournal v1

本轮把 ADR 0045 后最小可逆的 durability seam 单独拆成 ADR 0046，而不是一次引入 truth ledger、知识 lifecycle、authority 或完整
Governance Envelope。ADR 0046 的狭窄本地 structural journal slice 已完成 Rust domain/application/store、SQLite v25、CLI、
migration/compatibility、对抗测试与 scaffold/upgrade 接线，并经独立 fresh-context 复审和 `forge accept` 验收；这不交付或暗示
truth、knowledge lifecycle、conflict/freshness view、authority 或完整 Governance Envelope。

(1) **exact append identity**:`GovernanceRecordAppendRequest` 只携带 caller idempotency key 与一个 exact canonical v1 record-set string；单批
1–256 records、总计 ≤1 MiB、key ≤256 UTF-8 bytes。record-set 与 request 使用独立 digest domain 和无歧义 u64be length framing，batch ID 从
request SHA-256 确定派生；append time 不进入 identity。

(2) **atomic replay/conflict contract**:首写 receipt 只能是 `stored`，同 key/同 exact bytes 只能返回原 append metadata 加
`exact_replay`；同 key/不同 bytes、换 key 重放既有 records、record ID byte divergence、kind/aggregate/sequence 冲突均失败关闭。完整 batch、records 与
projection 必须一次提交或完全不写，所有 v1 引用对 existing+batch union 验证。

(3) **structural head 非 truth**:默认 show/list 只返回 batch/ordinal/identity/digest/byte-count/time metadata，只有显式 `--include-record` reveal exact
record。`GovernanceStructuralHead(interpretation=structural_sequence_only)` 只表示已保存的最高连续 sequence，可从 immutable rows 重建；不得解释为
current fact、active knowledge、valid/fresh evidence、conflict winner、authority、approval 或 hard-gate verdict。

(4) **additive compatibility**:journal tables 在 canonical SQLite v25 中以 additive empty tables 引入，不 backfill ADR/Memory/旧 Hub 记录；合并后的
current schema 为 v26。受支持 v24、canonical journal v25 与 historical endpoint-only v25 只可由 mutation-capable append 路径收敛到 v26，
read-only journal 命令要求 current v26 且不得创建/迁移。旧 binary 对更高版本必须拒绝而非降级。Schema corruption、byte/digest mismatch、
sequence gap 或 projection divergence 均失败关闭，immutable records 不自动修复或删除。

(5) **治理资产已接线**:`governance-contracts.yml` 升到 policy v3，保留 Evidence/Claim v1 wire/golden 与全部 shadow authority restriction，并新增
journal schema pin、ADR/standard/Skill、protected-policy checker 与 init/upgrade copy contract。Scaffold/upgrade 回归已通过，但只继承治理资产，
不安装 Rust runtime。

(6) **引用闭包 admission 已公开冻结**:policy v3 与 journal schema extension 固定最多 1,024 条 distinct stored dependency records、候选批次加已加载
closure 合计 16,777,216 canonical bytes、`derived_from_claim_record_ids` 最多 256 条边。三者只用于防资源耗尽，不代表证据充分、推理正确、truth 或
authority；超限必须在 atomic append 前失败关闭。

(7) **scaffold 不冒充 runtime**:`forge-init`/`forge-upgrade` 只继承 contract、Skill 与 shadow checker，不安装 Rust `forge-runtime` binary 或 SQLite
journal。命令名统一为 `forge-runtime governance journal`；只有检测到项目批准且兼容 v1 的 executable 才可执行，否则结果为 `not_executed`，没有匹配
receipt 不得声称 `stored|exact_replay` 或 durability。

(8) **完成证据**:Rust 全 workspace `cargo test --all-targets --all-features`、strict Clippy 与 changed-file rustfmt 全绿；Governance integration
14/14、scaffold init 8/8、upgrade 3/3、`forge check` 13/13、architecture 8/8、gate 与 `git diff --check` 均通过。独立 fresh-context
复审结论为 **APPROVE**，完整 `forge accept` 为 **ACCEPTED**。这些证据只关闭 ADR 0046 / Wave 0F-B–1。

## Sprint 95（✅ pure shadow projection 切片完成；完整 Kernel ABI/authority/effect/persistence 未启用）— CognitiveAtom v1

本轮用 ADR 0047 把 ADR 0038 的 CognitiveAtom 目标概念缩成第一个可执行、可逆、无副作用的 ABI 切片。它只从经
ADR 0045 重新验证的 exact canonical KnowledgeClaim record set 做确定性重投影，不从 prompt/model 创建新事实，
不读写 ADR 0046 journal/SQLite，也不扩张 `forge accept` 的完成权。

(1) **七类封闭投影**:`fact|constraint|decision|inference|assumption|hypothesis|unknown` 同名投影；
`lesson|proposal` 可进入 source closure，但不生成 Atom。输入必须是 1–256 条、不超过 1 MiB 的 closed exact
Governance record set；无可投影 Claim、dangling/wrong-kind/subject/cycle/supersession/digest 异常均失败关闭。

(2) **确定性 wire、闭包与 identity**:`forgeos.aadm.cognitive-atom/v1` 固定顶层形状、closed enum、signed int64、
UTF-8/canonical JSON 与字节上限；source Claim metadata、proposition、state、reference arrays、validity 及 confidence 按合同
逐字段投影。Atom/source-closure/set 使用分离 digest domain，Atom ID 另绑定 task/context/policy/source tree/revision 与
source Claim digest；任一载重字节漂移都不能通过重投影验证。

(3) **单一合同与跨语言参考实现**:独立 schema/golden、registry v4 和 ADR 以 SHA-256 pin 绑定；Python universal
checker、Go repository codec 与 Rust domain codec 在同一 fixture 上必须得到完全相同的 payload/Atom/closure/set 字节、
ID 和 digest。`forge check`、shadow detector/activation、fresh scaffold 和 legacy exact-allowlist upgrade 已接入 schema、fixture、
Python checker/package/tests；scaffold 不安装 Go/Rust binary 或持久化 runtime。

(4) **唯一正结果与边界**:结果只能为 `PROJECTED_SHADOW`，并明示
`no truth, authority, instruction, hard-guard, transition, completion or effect attestation`。`authority_ref=null`、
`hardness=none`、`instruction_allowed=false`、`projection_mode=shadow` 不能由输入覆盖。该切片不认证 principal/
collector/reviewer，不使声明成为 truth/instruction/hard guard，不授予 Grant/Approval，不推进 transition/completion，
不执行 effect，不写 Knowledge、GovernanceRecordJournal 或任何其他持久化。完整 Atom/DecisionTransaction/
InteractionEvent/Capability/Artifact/receipt ABI、prompt/model compiler、journal adapter、solver、Registry、Controller 与
Reflection runtime 仍属 planned。

(5) **定向验证**:Python CognitiveAtom/governance integration **38/38**、Go package tests、Rust 1.93 定向 **8/8**、
Python golden/instance checker、`forge check` **13/13**、architecture **8/8** 均通过；schema/golden/registry 当前字节的 pinned
SHA-256 一致。这些结果只证明当前 pure shadow projection 合同、字节及引用接线，不升级为任何完整
Kernel、权威、副作用或持久化能力声明。

## Sprint 96（✅ pure shadow source adapter 完成；身份认证、truth/authority、持久化与 effect 未启用）— Artifact Provenance → EvidenceRecord v1

本轮以 ADR 0048 把既有 `forgeos.artifact.v1` provenance observation 接入 ADR 0045 EvidenceRecord v1，但只交付一个纯函数、确定性、
read-only 的 shadow adapter。它不读取 artifact path 当前内容，不认证 manifest/agent/model/collector，不创建 Claim/CognitiveAtom，
不 append journal、不写 SQLite，也不产生 authority、completion 或 effect；SQLite 保持 v25，无 migration/backfill。

(1) **四模型边界与 exact request**:输入固定为 `api_version + artifact + binding + canonicalization`，artifact/binding 分别要求 exact 十一字段；
历史空 `_format`、未知/缺失字段、非 canonical UTF-8、浮点、int64 越界、Unicode 控制/双向字符、非规范 repo-relative path、无界数组/
字符串/请求均失败关闭。时间只接受 exact RFC3339Nano（1–9 位小数、合法 Z/offset），向下取整到非负 Unix 毫秒。

(2) **identity 与 mapping 分离**:source、完整 request、EvidenceRecord 使用三个独立 SHA-256 domain；record/snapshot ID 分别从 request/source
digest 确定派生。输出固定为 artifact/direct/observed/untrusted-data/valid Evidence，tool principal/collector 仅为 shadow 声明；final record
必须重新通过既有 Governance v1 strict validator，并与 exact re-adaptation 逐字节一致。唯一正结果为
`ADAPTED_SHADOW (no truth, authority, claim, atom, persistence, or effect attestation)`。

(3) **单一跨语言合同**:Schema、golden fixture、registry v5 与 ADR 通过 SHA-256 pin 冻结；Python universal checker、Go repository package、
Rust domain module 对 canonical source/request/Evidence bytes、source/request/Evidence digest 与毫秒时间完全一致。Python 公开 digest helper 也先做
strict request validation，sealed output defensive-copy mutable arrays，非法 sensitivity 和“单项合法但总字节超限”输入不再逃逸或 traceback。

(4) **治理和继承**:Evidence/Claim Skill 明确 artifact branch 输出单个 EvidenceRecord object、普通 record-set/journal 才输出排序数组；shadow
detector 的完整 state/rule/argv/invocation/tests/non-load-bearing 边界与 Skill 的 no-auth/no-current-read/no-persistence 文案进入 drift gate。fresh
scaffold 与 legacy exact-allowlist upgrade 同步 ADR/Schema/fixture/Python package/checker/tests，但仍不安装 Rust runtime 或持久化能力。

(5) **独立复审与验证**:跨语言/边界 reviewer 与治理/scaffold reviewer 最终均 **APPROVE/CLEAN**，0 Blocker/Major/Minor/Nit；复审推动关闭
Python sensitivity TypeError、总请求字节上限、输出数组别名、digest helper 宽松、Skill 输出歧义、detector/Skill 漂移和架构预算误计。
递归 Python **419/419**、scaffold/upgrade/security **36/36**、Go 全仓 `test` + `vet`、Rust 1.93 全 workspace test、strict Clippy 与
changed-file rustfmt 均通过；`forge check` **13/13**、gate（1720 files）、architecture **8/8**（1213 source files）与
`git diff --check` 通过。以上只证明 pure shadow adapter 与接线正确，不升级为 provenance 真实性、当前文件一致性、知识采纳、durability 或完成证明。

## Sprint 97（✅ pure shadow source adapter 完成；命令执行、PASS/完成裁决、身份认证、持久化与 effect 未启用）— Command Observation → Gate/Test EvidenceRecord v1

本轮以 ADR 0049 交付 Wave 1 的第二个独立 source adapter：把 caller 提供的 exact command observation + 显式 Governance binding
确定性映射为既有 `gate_result|test_run` EvidenceRecord v1。它不 spawn 命令、不读取 cwd/stdin/output/current tree、不验证 stream
preimage 或 producer/digest profile，也不把 exit=0、PASS 文本或 caller-declared evidence type 当成 criterion verdict；SQLite 仍为 v25，
无 migration/backfill/auto-append。

(1) **exact observation 与 honest terminal boundary**：request、observation、command/producer/source/streams/termination 都是 closed-world shape；
duplicate/unknown/noncanonical/float/int64 overflow、控制/bidi Unicode、非 normalized cwd、非法 argv/stdin/timeout/hash/time/list、stream
count/hash/truncation 矛盾与所有 size/depth/scalar 上限均失败关闭。Observation wire 可表达 `exited|timed_out|cancelled`，但现有 Evidence
command locator 只能无损保存真实非负 signed-int32 exit，所以 adapter 只投影 exited；timeout/cancel、负 sentinel、signaled/spawn-failed
不得伪装为 process exit。

(2) **四层 identity 与 deterministic mapping**：command、完整 observation、完整 request、sealed Evidence 各用独立 domain-separated SHA-256；
record/snapshot ID 分别从 request/source digest 派生。created-by 固定 shadow tool 且 run 为
`command-adaptation-<request_sha256>`，collector 只复制 producer 声明并以 command digest 绑定参数；runtime snapshot 与历史 Evidence v1
`artifact_sha256` 兼容槽均保存 observation source digest，不把 source 改称 Artifact。combined stream 只表示 producer 记录的 drain-event
chunk 顺序，不证明 OS emission 顺序。最终 record 必须重跑既有 Governance v1 strict validator，并与 pure re-adaptation 逐字节相同。

(3) **单一跨语言合同与治理漂移门禁**：Schema、golden、registry v6 与 ADR 通过 SHA-256 pin 冻结；Python universal checker、Go repository
package 与 Rust domain module 对 canonical command/observation/request/Evidence bytes 和四类 digest 完全一致。Evidence/Claim Skill、shadow
detector、activation/canonical refs、Schema extension、pins、golden recomputation 与 non-load-bearing/no-execution/no-pass/no-persistence 边界均有
正反治理测试；原 governance checker 按职责拆出 `governance_engineering/source_adapters.py`，未创建新的上帝文件。

(4) **scaffold/upgrade 与兼容边界**：fresh init 和 legacy exact-allowlist upgrade 同步 ADR/Schema/fixture/Python checker/package/tests 及其治理
helper；Go/Rust 实现仍明确为 Catalyst repository-only，scaffold 不安装 runtime、producer integration 或 persistence。架构预算仅随一个 universal
root checker 从 35 调到 36，实测 35 个非测试 harness 文件并保留一个 headroom；既有 Evidence/Claim、journal、CognitiveAtom 与 artifact adapter
golden 均保持不变。

(5) **独立复审与最终验收**：三份互相独立的跨语言 strictness、Rust/治理接线、fresh scaffold/upgrade 复审均
**APPROVE/CLEAN**，合计 0 Blocker/Major/Minor/Nit。递归 Python **442/442**、Node **400/400**、Go Core **1,485 observed tests**、
Rust workspace（domain **116**、application **50**、infrastructure **217**、interfaces **158** 等）与 strict Clippy、Go vet/build、
scaffold/upgrade/security、`forge check` **13/13**、gate（1764 files）、architecture **8/8**（1245 source files）和
`git diff --check` 均通过；最终 **`forge accept: ACCEPTED`**（9 PASS、0 FAIL、2 个诚实 N/A）。以上只证明 pure shadow mapping、
字节、兼容和分发正确，不升级为命令真的执行、stream 真实、producer 身份、gate PASS、完成权威、durability 或 effect 证明。

## Sprint 98（✅ pure shadow source adapter 完成；文件/报告验证、扫描裁决、producer 身份、持久化与 effect 未启用）— Evolve Repository Locator → EvidenceRecord v1

本轮以 ADR 0050 交付 Wave 1 的第三个独立 source adapter：把 caller-declared exact Evolve repository locator observation 与显式
Governance binding 确定性映射为既有 `repo_locator` EvidenceRecord v1。它不读取 current repo path/report，不解析 symlink，不验证
file/report/tree/parameters digest preimage，不确认 finding/clear/opportunity、scan coverage/completion 或 candidate 价值；SQLite 保持 v25，
无 migration、backfill、auto-append 或 read/write side effect。

(1) **closed-world observation 与身份分离**：request、binding、content、locator、producer、scan context 和 source 均为 exact shape；
duplicate/unknown/noncanonical/float/int64 overflow、控制/bidi Unicode、非规范/逃逸/drive/protected-root path、空白或超过 4,096 Unicode
scalar 的 path、非法 line/detail/hash/list/relation/opportunity ID 均失败关闭。Opportunity ID 保持 `evolve_scan_v1` 的 1–64 bytes ASCII
词汇；line 0 无损映射为 null range。locator、完整 observation、完整 request 和 sealed Evidence 分别使用独立 domain-separated SHA-256，
record/snapshot/run identity 由 request/source digest 确定派生，任一 observation 或 binding 载重漂移都会改变对应身份。

(2) **确定性 shadow mapping**：Evidence 固定为 direct/observed/untrusted-data/valid 的现有 `repo_locator`；created-by 是 request-derived
shadow adapter principal，collector 只复制 producer 声明，不能冒充已认证身份。content digest 同时进入 artifact compatibility slot 与 locator，
source snapshot 绑定完整 observation；最终 record 必须重跑 ADR 0045 strict validator 并与 pure re-adaptation 逐字节相同。唯一正结果为
`ADAPTED_SHADOW (locator mapping only; no file/report verification, scan judgment, completion, truth, authority, claim, atom, persistence, or effect attestation)`。

(3) **单一跨语言合同与继承**：Schema、golden、registry v7 与 ADR 通过 SHA-256 pin 冻结；Python universal checker、Go repository package 与
Rust domain module 对 canonical locator/observation/request/Evidence bytes、三条 source/request digest 与 Evidence self digest 完全一致。
Evidence/Claim Skill、shadow detector、activation/canonical refs、治理 checker、fresh scaffold 和 legacy exact-allowlist upgrade 已接线；scaffold
只复制 ADR/Schema/golden/Python checker/package/tests，不安装 Go/Rust runtime、真实 Evolve producer 或 persistence。

(4) **验收推动的缺陷闭合**：独立 contract、跨语言和 scaffold reviewer 均 **APPROVE/CLEAN**。复审推动三语言统一非空白/4,096-scalar
path、Unicode `Cc`、Evolve opportunity vocabulary 与 Rust 256-list fail-closed；cold checker 不生成 `__pycache__`。最终聚合验收又发现并关闭
Go ST1005 诊断文案和宿主默认 Cargo 1.83 漏选项目 Rust 1.93 两项真实问题；新增 repository-local `rust-toolchain.toml` 与 CI/manifest
保持 1.93.0 一致，不靠临时环境变量伪造通过。

(5) **最终证据**：递归 Python **471/471**、显式 Node **400/400**、Forge Core **1,500 observed tests**、go-taskd **22**、url-shortener
**47**，Rust workspace/各 manifest test 批次、strict Clippy、typecheck、build 与定向 rustfmt 全绿；Go full test/vet/build 与 golangci-lint
全绿。`forge check` **13/13**、gate（1810 files）、architecture **8/8**（1278 source files）和 `git diff --check` 均通过；完整
`forge accept` 为 **ACCEPTED**（9 PASS、0 FAIL、2 个诚实 N/A：未安装 ruff/eslint 的聚合 lint 与未配置 coverage，不冒充已执行）。
以上只证明 pure shadow mapping、字节、边界与分发正确，不升级为文件/报告真实性、Evolve scan 裁决、知识采纳、durability 或完成证明。

## Sprint 99（✅ 本地 gate command observation producer 完成；不签发 PASS、身份、authority 或 effect 证明）— ADR 0051

本轮交付显式 opt-in、Unix-only 的四条固定本地 gate/check/accept/probe command observation producer。它把 canonical Git root、实际
scrubbed child environment、PATH-resolved top-level executable bytes、bounded-interval working-source inventory/entry observation、raw streams、
termination 与 production identity 收敛为 strict package；普通 gate API 继续保持 capture-disabled byte-compatible 行为。共享
`gitworktreesource` 保持 endpoint pre/post equality 的区间观察语义，不冒充原子 snapshot、execution pin、authenticated Git 或 effect containment。

Schema、exact golden、Python checker、Go runtime、scaffold/upgrade、race/低 FD/TOCTOU/Unicode/路径与资源边界测试、两份独立 CLEAN 复审及
真实 `forge accept` 均已完成；交付 commit 为 `91170f7`。唯一正结果仍是 `OBSERVED_LOCAL_PROCESS`，不得把 exit zero、输出文本、source
revision 或 fixture 解释为 PASS、criterion、completion、truth、identity、authority、persistence 或 external-effect receipt。

## Sprint 100（✅ Local Evolve locator observation producer 完成；不确认扫描判断、完成、真值或持久化）— ADR 0052

本轮在不修改 ADR 0050 observation/Evidence wire 的前提下交付显式 opt-in、Unix-only producer：绑定完整 canonical
`EVOLVE_SCAN_V1: ` report preimage、固定 parameters、共享 `git-worktree-source-tree-v1` bounded-interval source observation，以及按
dimension/relation/opportunity/report 顺序产生的 zero-or-more exact locator observations。同一 path 跨 relation/opportunity 的出现不会去重；
每条 observation 绑定完整 bounded regular-file bytes/hash、同一 capture timestamp、report/source/parameters identity。

实现将 ADR 0051 source capture 抽为中立 `gitworktreesource` 包，同时保持旧 command golden/wire 不变。复审推动关闭 report-only
U+2028/U+2029、引用行外非法 UTF-8、恰好 1 MiB 无换行证据、冒号拼接去重碰撞及 Python CLI 在 16 MiB 解码前无界读取等真实边界；
Python universal checker 使用 opened-FD bounded read，Go/Python 对 canonical bytes、顺序、multiplicity 和失败关闭语义一致。

定向 Python **26/26**、全仓 Python **518/518**、全量 Go test/vet/build/lint、focused race、ADR 0051 regression、architecture **8/8**、
scaffold/upgrade、golden 与 `git diff --check` 已通过，Go contract、整体切片和 Python bounded-read 三份独立 fresh-context review 均
**CLEAN**。Registry v9 将 ADR 0051/0052 同列 `shipped_producers`，`staged_producers` 为空；最终 completion authority 仍只来自本提交上
实际执行的 `forge accept`，失败时不得提交或维持 DONE。Producer 固定 read-only Git argv 但不认证 binary，也不提供 sandbox/egress/effect
containment；它不自动调用 ADR 0050、不创建 Claim/Atom、不 append journal、不写 SQLite/Knowledge，SQLite 仍为 v25。

## Sprint 101（✅ DONE；Local Go package dependency graph observation producer）— ADR 0053

本轮交付显式 opt-in、Unix-local 的 selected Go module lexical dependency observation。合同复用
`git-worktree-source-tree-v1` bounded-interval source，以 `selected-module-all-regular-go-files-union-v1` 和 Go 标准库 parser 记录
module/package/file、compile/test import、external/unresolved classification、coverage 与 diagnostics；单文件 parse failure 不得泄露部分事实。

该候选不运行 `go list|build|test|mod`，不读取 module cache 或网络，不解析真实 GOOS/GOARCH/build tags、`go.work`、
`require|replace|vendor`、dependency availability 或 compiler reachability，也不证明 graph completeness、architecture judgment 或 Impact Closure。
它不创建 Evidence/Claim/Atom/Context/Grant/Impact/Cost/Risk、不 append journal、不写 SQLite，且不签发 completion/truth/authority/effect。

Registry v10 将 ADR 0051/0052/0053 同列 `shipped_producers`，`staged_producers` 为空。Schema、golden、Python checker、Go producer、
governance/Skill/scaffold 接线、跨语言/对抗/资源边界测试已完成，两份独立 fresh-context review 均为 CLEAN；完整 `forge accept` 是最终
completion authority，未真实 ACCEPTED 时不得提交。fixture 永远只是 deterministic contract bytes，不是 live parse/build/architecture receipt。

## Sprint 102（✅ DONE；Local Governance Semantic View v1）— ADR 0054

本轮在 ADR 0045/0046 的不可变 GovernanceRecord journal 与 structural head 之上，交付本地、只读、无权威的 semantic view：
`view`、`conflicts`、`validation-jobs` 均要求显式非负 `--as-of-unix-ms`，永远评估当前 structural aggregate tail，
不得按时间选择历史版本，也不签发 truth、winner、adjudication、approval、completion、freshness 或 effect。Claim type 与
authority-free shadow state、sequence-one 全合法初态及后继边、conflict/job identity、validation plan 与 canonical assessment digest
均由共享 domain 规则重验；业务事实、声明区间位置、冲突声明、校验计划与系统建议保持分离。

(1) **SQLite v27 与 live read boundary**：v26→v27 只新增三张 materialized semantic 表及索引，迁移在同一原子事务内重验完整 batch、
aggregate history、reference relation、lifecycle、head/materialization/parity 后 backfill；dangling/wrong-subject/cycle、非法历史 transition、
digest/cardinality drift 均回滚。升级前备份改为 SQLite-consistent 单文件 snapshot，包含 committed hot-WAL pages。semantic read 使用
exact-v27 `mode=ro` + `query_only` 的单一 Deferred snapshot，不迁移、不做 Hub 逻辑写；SQLite 可能创建/删除空 WAL/SHM 或协调 SHM
read-lock bytes，完全只读文件系统可返回 Unavailable。普通 `show/list/head` 继续保持 immutable、拒绝 sidecar 的既有契约。

(2) **完整性与有界资源**：单 view 在共享预算内验证完整 aggregate history、transitive reference closure 与所有 owning batch sibling 的
unique union（最多 1,024 records/16 MiB）；multi-head scan 共享 65,536 unique records/256 MiB/1,000,000 work units，Claim census
最多 10,000，公开列表/冲突组最多 100。immutable tails、structural heads、semantic heads、Claim child 与 expected/materialized jobs
执行全局双向 identity parity；超限为 Unavailable，缺失/额外/漂移为 Corrupt，禁止返回 partial、empty 或虚假 no-conflict。append 与
exact replay 同样先验证当前 aggregate 的完整 history/closure，再允许写入或重放；rebuild/migration 使用完整 durable 全量校验，不把公开
scan 上限误施加到合法的大型历史库。

(3) **契约、治理与分发**：Schema、golden、registry v11、ADR、canonical human standard、runtime/engineering README、backup runbook、
Evidence/Claim Skill、activation、standalone semantic checker 与 fresh/upgrade scaffold 已对齐。Golden source 使用可解析 JSON Pointer，
checker 与 Rust fixture test 都绑定 source record 的 metadata/id/sequence/digest。当前 pins 为 schema
`360fb89d1571920090eb28e54678e8aa96f5d007d5acec693beb67fbb8f963f3`、fixture
`a3b6fb9b397231a0647fca845f0118d060c77d975ead2dccb55819aeea6dd66a`、governance policy
`a086a3f601cfaa43cea8fa45a91748f5a3ef612c93e1d91dd16c0904eb79424b`；scaffold 复制并实际运行 semantic checker 与其对抗测试，
不安装或冒充 repository-only Rust persistence runtime。

(4) **复审推动的缺陷闭合**：两份 fresh-context contract/runtime review 最终均 **CLEAN**，0 P0–P3。复审推动关闭 immutable opener
误用于 live semantic read、tail-only replay/append、balanced missing+extra parity 绕过、owning-batch amplification、unbounded history decode、
v26 relation backfill 漏验、authority-like Claim states、unbound conflict/job identity、无效 fixture fragment、过期 v24 文档与 hot-WAL 裸复制
备份等真实问题。接受门禁首次还暴露 Node `spawnSync` 默认输出缓冲和仓库内 TMPDIR 污染测试隔离：runner 现使用显式 16 MiB 有界
缓冲，越界保留诊断并失败关闭；最终验收使用 repository-external `/tmp`，未放宽 legacy 或 Go 测试语义。

(5) **最终验证**：Rust 1.93 workspace/all-targets、strict all-feature Clippy、fmt、Go full test/vet/build/lint、architecture **8/8**、
`forge check` **13/13**、semantic adversarial/operational checker、fresh init/legacy upgrade、hot-WAL/live snapshot/concurrent writer/atomic
migration/rollback/backup 与 `git diff --check` 均通过；两份独立复审为 CLEAN。完整 `forge accept` 为 **ACCEPTED**（Python
**589**、Node **402**、Forge Core Go **1,628 observed tests**；9 PASS、0 FAIL、2 个诚实 N/A）。以上只证明本地 deterministic
semantic interpretation 与一致性边界，不把声明、AI 建议或 projection 升级成知识真值、冲突裁决、执行授权或完成证明。

## Sprint 103（✅ DONE；Shadow ContextPackage v1 pure contract）— ADR 0055

本轮把 Wave 0F-B–3a 的 Context 前置能力收缩为无权威、无副作用的 strict `ContextPackageBuildRequest v1`/`ContextPackage v1`。
Caller 必须显式绑定 task/change/node/role、source revision/tree、policy/routes、评价时间、budget 与 tokenizer identity；builder 先对所有
available source 应用 caller-declared UTF-8 byte redaction，再按 eligibility、source max 与 required-first 固定顺序选择。Optional source 只可带
唯一 omission reason 退出；`instruction_candidates`、`trusted_context`、`untrusted_data` 使用 typed JSON lane，所有 snippet 固定
`instruction_allowed=false`。repository/web/log/issue/tool output/artifact/other 不能自升 lane 或 trust。

Python、Go、Rust 独立实现共享 exact golden；raw source content 使用 plain SHA-256，request/cache、projected content、snippet、projection 与 context
分别使用六类 domain-separated digest。TokenCounter 每次调用前重验 identity，并只接收 exact canonical projection bytes；required budget 失败关闭，cache hit 先重算
request key 再完整重装配。Strict package decoder 在三语言均拒绝 duplicate/unknown/float/noncanonical/oversize、trust-lane 提升、越界
redaction/truncation receipt 与 accounting drift。Schema、fixture、registry v12 pins、Context Skill/detector/routes、scaffold/init/upgrade 和事实源已同步。

复审已推动关闭 required token 只做批末计数、ineligible redaction receipt 先后矛盾、cache-key 检查顺序、Go package byte ingress、Schema 尾随
LF anchor、standalone decoder lane 提升和跨语言 receipt bounds 等缺陷。当前 Python 24 tests、Go ContextPackage 定向及 Forge Core 全包、Rust
ContextPackage 16 tests 及 workspace all-targets/all-features、strict domain Clippy、scaffold init/upgrade、`forge check` 13/13、architecture 8/8、
file gate 与 `git diff --check` 均通过；独立复核为 CLEAN。完整 `forge accept` 在本轮树上以 **ACCEPTED** 通过（Python **613**、Node
**402**、Forge Core Go **1,652 observed tests**；五组 Rust all-targets/all-features 全绿；9 PASS、0 FAIL、2 个诚实 N/A）。

该正结果仅为 `ASSEMBLED_SHADOW (no truth, authority, instruction, permission, approval, completion, persistence, or effect attestation)`。
Builder 无 repository/network/process/provider/database I/O，不认证 source/freshness/redaction completeness，不调用模型、不写 journal/Hub；真实
Context Router、semantic retrieval、prompt compiler、production tokenizer、Grant/PDP/Approval 和 durable context store 仍是后续独立合同。

## Sprint 104（✅ DONE；CapabilityGrant v1 contract-only）— ADR 0056

本轮交付 Wave 0F-B–3b-1 的无权威、无副作用 strict `CapabilityGrant v1` envelope 与 declared assessment。四个 exact API、21 项
single-effect closed vocabulary、typed allow/deny scope、budget、subject/task/capability/source/context/policy binding、caller-time validity、SoD、
usage policy 及 production declaration 约束已由 Schema/ADR 冻结。Allow clause 是 OR alternative，clause 内 resource 联合约束且禁止跨 clause
拼接；flat deny 先于 allow。`migration.generate` environment 是 presence-matched exact qualifier，`process.exec` command timeout 与
proposed usage timeout 必须一致。

Python、Go、Rust 独立 strict decoder/evaluator 共用 exact golden，统一 canonical JSON、五类 domain-separated digest/preimage、资源顺序、
deny precedence、no-cross-clause、phase/production/profile/cardinality、nullable binding、budget/time 与 reason self-consistency。复审推动关闭 split
timeout budget、IPv4-mapped IPv6、IPv6 zone ID、DNS-tagged dotted IPv4、Unicode moving secret alias、effect/scope mismatch、deep JSON
recursion、programmatic cyclic object、full-document byte ceiling/self-digest preimage 边界及 optional environment qualifier 等失败关闭缺口。

Registry v13 将 `CapabilityGrant` 仅列于 `shipped_contract_only_kinds`，不进 `shipped_kinds`/`planned_kinds`；Policy/Authority Skill、
shadow non-load-bearing detector、routes、scaffold init/upgrade 与事实源已同步。当前 pins 为 schema
`dd26568ec430ae5e444ae851ba2b58087528a17e84794137268be3860d9c3209`、fixture
`0261a682bddca2f27976a9cd663350e8cf222685389fecc7ad8ae536083fef35`、governance policy
`de4edc116498bd5d193df6146442d4b75a0d2e23615fd9d4db29b8cb7fa686a5`。定向验证为 Python **33 tests**、Go package race/vet、
Rust **24 tests** 与 strict Clippy；无 `jsonschema` 的 universal scaffold 仍执行 contract/golden 自测，仅诚实跳过外部 Draft Schema 校验。
两路 fresh-context 独立复核在最终 Base64URL canonical 修复及 pins 更新后均为 **CLEAN**。最终树使用本机字节版本一致的
Rust 1.93.0 执行完整 `forge accept --timeout 60m`，结果为 **ACCEPTED**：Python **655**、Node **402**、Forge Core Go
**1,681 observed tests**，五组 Rust all-targets/all-features 全绿；**9 PASS、0 FAIL、2 N/A**，两个 N/A 均未伪装为通过。

唯一正结果仍是 `ASSESSED_DECLARATIONS_ONLY`；issuer/proof/principal/policy/Approval authentication、PDP decision、revocation/usage、
pre/postflight、audit receipt、persistence、execution、authorization、permission 与 effect attestation 均未实现。声明关系、Schema 合法或
Skill 结果不得冒充有效 Grant 或生产权限。

## Sprint 105（✅ DONE；Authenticated bootstrap repo-read Grant issuance）— ADR 0057

本轮把 ADR 0056 的 declaration-only Grant 向真实 authority 推进一个严格封闭的 profile。独立非 Agent `forge-kernel` 只接受 operator 在
repository 外显式 pin 的 `GovernanceTrustRoot v1`，以三把 principal/public-key/usage 均互异的 Ed25519 key 分别认证 signed Policy、signed
GrantRequest 与 Kernel issuance；唯一允许的 Grant 是 `bootstrap_planning`、`repository-reader/v1`、单一 `repo.read`、1..16 个 exact path、
小预算/TTL 及 `local|development|test`。成功产生 signed ADR 0056 CapabilityGrant、signed `GrantIssuanceReceipt` 和完整 signed durable ledger；
同 idempotency key + byte-exact request 只返回原签名记录，冲突、clock rollback、错误 pin/key/profile/signature/relation 全部失败关闭。

持久化使用真实 nonblocking flock、bounded canonical snapshot、CAS、temp fsync、rename、directory fsync 与 strict readback；rename 后不确定性固定
`PERSISTENCE_UNCERTAIN`。Runtime 仅支持 Unix，非 Unix 在读 authority input/key 前失败。Authority/state directory 必须 exact `0700`；
叶子为 euid-owned、single-link、无特殊权限位的 exact `0600` regular file。Authority/repository resolved endpoint 按 ancestor filesystem
identity 双向不重叠，caller repository absolute source、首次 resolved path 与 opened directory identity 全 session 绑定；root identity traversal
用真实 `NONBLOCK|DIRECTORY|NOFOLLOW` opener，大小写/Unicode alias、source symlink retarget、rename replacement 与 FIFO swap 都有 fail-closed 回归。

复审推动关闭了 public golden fixture key 被生产 runtime 接受、跨 Policy/Request signing、full-document byte ceiling、source revision 16 KiB/160-byte
跨 ADR 漂移、特殊 mode bit、secret buffer error-path remanence、post-rename uncertainty、stdout short-write/replay、APFS casefold/normalization、repository
source TOCTOU 与 blocking FIFO 等真实问题。Golden keys 只能用于 contract test；生产 binary 对 exact fixture root、任一 fixture public key 与 fixture issuer
key 独立拒绝。两路 fresh-context security/integration review 与一份专门 root-identity review 最终均为 **CLEAN**，0 P0–P3。

Registry v14 只新增 `authenticated_bootstrap_repo_read_grant_issuance_v1` narrow runtime profile；CapabilityGrant kind 仍仅在
`shipped_contract_only_kinds`。当前 pins 为 governance policy
`5c3f4413c1f4bbaeb76a57412a50c63d51b5a6f4ddd23e668628d807edb2f5a7`、schema
`4b68e8bc989f457e602108920a570f9876be8b7bd21e6e1151852314951fdde5`、fixture
`60a234a15080f7c08367ea53f7a3cbfee6722c8ac015bbb09132bdcbdb31b011`。Scaffold/upgrade 只复制 contract/checker，不安装 Kernel、root、key 或 state；
无兼容外部 runtime 固定为 `not_executed`。

最终实现/事实源冻结树的完整 `forge accept --timeout 60m` 为 **ACCEPTED**：recursive Python **39 files / 688 tests**、Node
**21 files / 402 tests**、Forge Core Go **1,757 observed tests**，五组 Rust all-targets/all-features test/check/build 与 strict Clippy 全绿；
architecture 8/8、go vet/build、SCA、secret scan、scaffold 和治理闸门均通过，合计 **9 PASS、0 FAIL、2 N/A**，N/A 未冒充 satisfied。

本 profile 只认证并持久化 Grant **签发**，不执行 repository read，也不提供 plan finalization、Approval、revocation、usage/reservation、
pre/postflight、PEP/effect、ContextPackage/provider、Transition/Knowledge、key provisioning/rotation、staging/production、remote/HA/multitenant authority。
本地 `0600`/euid 不是 OS principal/HSM 隔离；ledger high-water 只相对当前 snapshot，不能抵抗管理员回放旧 signed state；`forge accept` 是完成权威，
绝不是 Grant issuer。

## Sprint 106（✅ DONE；Authenticated bootstrap repo-read execution）— ADR 0058

本轮在 ADR 0057 issuance 后增加唯一的 authenticated repo-read execution profile。独立、repo 外 externally pinned execution root 绑定 exact
issuance root/epoch，但三把 `execution_policy_sign|execution_receipt_sign|execution_request_auth` key 与所有 issuance key 分离；signed execution policy
与 signed invocation 必须 byte-exact 匹配。`allow/activate_once` 才可预留，signed `deny/do_not_activate` 不触碰 repository 或 usage state。

Linux amd64/arm64 runtime 只经 `openat2` + `BENEATH|NO_XDEV|NO_SYMLINKS|NO_MAGICLINKS` 读取 1..16 个排序 exact manifest leaf，逐项校验 regular
file、raw byte length 与 SHA-256。Durable single-use group 固定为 `reserved_no_repo_io -> effect_intent -> completed|failed_consumed|quarantined`；每步
persist + strict reopen，active orphan 永不 resume/reread，只可 signed quarantine。Reservation 必须处于 fresh Invocation window；已开始的 intent/terminal
可越过 expiry，但成功 `elapsed_ms` 仍不得超过 timeout。Reservation 早于任何 repository metadata I/O；每个 signed transition 单独取 wall-clock sample，
clock failure 不伪造旧时间而保留 active tail。Reader 对具体 syscall 前间后检查 timeout，grantstate identity revalidation 只做 composite 前后检查；
blocked op 可越预算，返回后 timeout 优先。Cooperative timeout 不冒充 OS hard deadline。

Usage ledger 永不持久化 `content_base64url`。首次 completed delivery 仅在 terminal strict reopen 后返回 receipt、content-free metadata 与 raw result；后续
canonical pair 或双 64hex terminal replay 在 manifest/repository/clock/receipt-seed access 前返回同 receipt/metadata 与 null raw，digest miss/mixed
失败关闭，failure/quarantine 只返回 receipt。
Crash/short write 后 raw 不可恢复。Mutable bytes 只 best-effort clear；Go strings、GC、kernel/downstream copies 不提供 secure erasure，也无 process
isolation/HSM attestation。Signed high-water 不能抵抗管理员整体替换为旧 snapshot。Pinned root、receipt key 与 usage namespace 不可分割；v1 不支持
rotation、epoch migration 或 state clear/rebase，fresh root/state 不继承 spent history。连续性轮换需要新 profile/ADR 与外部见证的完整 history migration。

Registry v15 将 ADR 0057 issuance 与 ADR 0058 execution 两个窄 profile 同列在 `shipped_runtime_profiles`，并保持
`candidate_runtime_profiles` 为空；`CapabilityGrant` 仍仅是
`shipped_contract_only_kinds`。Schema/fixture pins 分别为 `6eb96621f8160bf8b7e8658d3d51dbe1b66f915df4da0eb70d0d412d250e889b` 与
`309b3da66c64669239ce40bd086cdcbb518d59dc7fd5e1bad60d6acf9107480d`。Fixture root/任一 fixture public key 在 production decoder 中必须拒绝。
晋级后 Registry v15 的 protected policy SHA-256 为 `9b8d3e088419a962a9fa9a7050154b5c7f0590752527947cbf32e2b29e3867ce`。

## Sprint 107（✅ DONE；ApprovalRecord v1 contract-only）— ADR 0059

ADR 0059 已按 strict contract-only 边界交付：`ApprovalRecord`、declared target/request/assessment、detached-proof content identity 与 ADR 0056
`ApprovalRef` 三元组投影已在 Python/Go/Rust 接线。Registry 已升级为 v16，将 `[ApprovalRecord, CapabilityGrant]` 同列
`shipped_contract_only_kinds` 并从 `planned_kinds` 移除 ApprovalRecord；这只表示 wire/纯 evaluator 的交付分类，不代表 runtime authority。

Shadow detector 保持 non-load-bearing，governance route 以 trusted schema + instruction Skill 接线；registry 实际超过 64 KiB 后，只把该 required
source 的 per-file ceiling 提升到 128 KiB，总 Context budget 不变。Universal init/upgrade 复制 ADR/schema/fixture/Python checker/tests/governance
wiring，但不复制 Go/Rust implementation、authority registry、key、revocation/condition/risk state、approval store 或任何 runtime。

`.forge/<stage>.approved`、`--approved`、`actor_hint`、workflow/session/environment/ambient clock 均禁止导入。Approver/authority/proof/SoD
authentication、condition/RiskAcceptance/revocation validation、effective approval、PDP/authorization、permission、persistence、transition 与 effect
仍全部 unavailable；CapabilityGrant 的 `approval_state` 继续固定为 `not_evaluated`。Schema/fixture pins 分别为
`bc11d2b066bac35252bff6739798c3e30a508ed31fca0306b9cf1cdc0ef9ab64` 与
`501320b9f65775091e67ba22c6e7faa5b5ecaa1f1b472a1a196da93c7ab81978`；Registry v16 protected policy SHA-256 为
`d08435217e563a0bbf9bef14a88dfad4652fabd009838dfc2e7f848991c3df03`。Scaffold/upgrade 只复制
ADR/schema/fixture/Python structural checker/governance tests，不安装 Go binary、root、key 或 state；无兼容 runtime 为 `not_executed`。
Independent adjudication 已闭合 declared target 内部 SoD 一致性，且未改变 wire/schema/fixture。正式 candidate-tree `forge accept` 为
**ACCEPTED**：**9 PASS、0 FAIL、2 N/A**；N/A 未计作 satisfied。Recursive Python 为
**45 files / 744 tests**，Node 为 **21 files / 402 tests**，五组 Rust observed tests 为 **248 / 54 / 202 / 248 / 164**，
Go 与 Node examples 分别为 **22 / 47**，Forge Core Go 为 **1,818 observed tests**。Python/TS/Go lint 因工具缺失或未配置诚实为 N/A，
coverage 因无可解析报告诚实为 N/A；五组 Rust strict Clippy `-D warnings` 均通过。该完成裁决只验收合同切片，不认证 Approval、激活 Grant
或产生 authorization/permission/effect authority。首次 sandbox 内尝试因 `spawnSync EPERM` 终止，明确不是验收证据；上述正式事实仅来自
sandbox 外、scrubbed environment 的有效完整 run。

## Sprint 108（✅ DONE；TransitionReceipt v1 contract-only）— ADR 0060

ADR 0060 已按 strict contract-only 边界交付 `TransitionStateVocabulary`、`TransitionReceipt`、declared target/request/assessment、显式 predecessor、
applicability/rework/resume relations 与 ADR 0056/0059 reference compatibility。Registry v17 将 `TransitionReceipt` 列入
`shipped_contract_only_kinds`，`planned_kinds` 仅余 `KnowledgeUpdateProposal`；该分类只表示 wire/纯 evaluator 已交付，不是 shipped runtime 或权威状态机。
Schema/fixture pins 分别为 `94962069c93f55129506b9d4b45f1f9db6d9425ecbdbaef9c06fcbe155e43cbf` 与
`dac0b6d8921aaecaf138c5b62924c8a3b9ac8f9c531a67f2be358d47c1c30da9`；晋级后的 Registry v17 protected policy SHA-256 为
`2c70e6e5a2045a744bf4f1f572dfd63de4328a6da27f3669972387510400637a`。Shadow detector 保持 non-load-bearing，trusted schema route 与
Policy/Authority Skill 已接线；universal scaffold 只复制 ADR/schema/fixture/Python checker/tests/governance wiring，不安装 Go/Rust runtime、controller、
ledger、key/state 或 transition executor。Listed edge、PASS/NA、reference equality、caller-time continuity 均不认证 current state、precondition、
waiver、Grant 或 Approval，不产生 authorization/permission/persistence/transition/completion/effect。

正式 candidate-tree `forge accept` 为 **ACCEPTED**：**9 PASS、0 FAIL、2 N/A**；N/A 未计作 satisfied。Recursive Python 为
**48 files / 778 tests**，Node 为 **21 files / 402 tests**，Forge Core Go 为 **1,837 observed tests**，Go 与 Node examples 分别为
**22 / 47**，五组 Rust observed tests 为 **248 / 54 / 227 / 248 / 164**。该证据来自 sandbox 外、清除
`OPENAI_API_KEY`/`OPENAI_BASE_URL`/`ANTHROPIC_API_KEY` 的宿主完整 run；sandbox `spawnSync EPERM` 与中断 run 均未计入验收证据。
Accepted/DONE 只验收 strict wire、跨语言重现、纯 evaluator 与治理/scaffold 接线，不认证 controller/current state/precondition/waiver/Grant/Approval，
不 append ledger、持久化、推进 transition、完成任务或产生 effect。

## Sprint 109（✅ DONE；KnowledgeUpdateProposal v1 contract-only）— ADR 0061

ADR 0061 已按 strict contract-only 边界交付 `KnowledgeUpdateProposal`、七字段 declared target、request/assessment identity、exact ADR 0045
EvidenceRecord/KnowledgeClaim reachable closure，以及按 aggregate 排序的 `create|supersede` mutation。Schema/fixture pins 分别为
`5825658017a9debf197cd82a0df4d553bf101ed20b1a35f6ff3e9d07064e4c4b` 与
`2808e44b27df5f7b183ae7da3847d5780a3f66887d6b49e5fb4544a069a7ad5f`；golden 的 record-set/proposal/target/request/assessment
digests 分别为 `c14c11c126c1b76ac1affb3421f2ffea20f5c8567fc43f9caef7bed3683c5c7f`、
`a4c08d011e3bfb6c08e9d9f5806f39830406478c16f93bad6c8ecde5d3b519b1`、
`34e367580f5f2ddbf780911d8fb6d73e89949f0231f220444537e30b49eeff85`、
`d0c325f29617e3a164fec4f897c31bbee2bec316c008ba52740477290c05b413`、
`e30a494f0e911cf1b312babd1b296786da00760f797857f7b4f0697fa506b037`。

Registry v18 把 `KnowledgeUpdateProposal` 加入 `shipped_contract_only_kinds` 并清空 `planned_kinds`；这只表示 v1 wire/纯 evaluator
实现已冻结，绝不表示 runtime、adoption 或 apply 已交付。晋级后的 Registry v18 protected policy SHA-256 为
`5170cc701cfaa648395764740ee06b552bd99caa738116fda19eda85885c0d7e`。Shadow detector non-load-bearing，trusted schema route、Evidence/Claim Skill 与
universal init/upgrade 只复制 ADR/schema/fixture/Python checker/tests/governance wiring，不复制 Catalyst-only Go/Rust、journal/database/current-head state、
keys、Kernel、Knowledge apply 或 receipt。Declared Grant/Context/artifact compatibility 不认证 proposer/Grant/Context/Evidence，不评价 truth/current
head/conflict/freshness/policy/authority，且所有 truth/adoption/authorization/permission/persistence/apply/receipt/execution/effect 都不可用。

正式 candidate-tree `forge accept` 为 **ACCEPTED**：**9 PASS、0 FAIL、2 N/A**；N/A 未计作 satisfied。Recursive Python 为
**51 files / 809 tests**，Node 为 **22 files / 406 tests**，Forge Core Go 为 **1,857 observed tests**，Go 与 Node examples 分别为
**22 / 47**，五组 Rust observed tests 为 **258 / 54 / 258 / 248 / 164**。证据来自 sandbox 外、清除
`OPENAI_API_KEY`/`OPENAI_BASE_URL`/`ANTHROPIC_API_KEY` 的完整宿主 run；保留日志
`/tmp/forgeos-adr0061-candidate-acceptance.log` SHA-256 为
`7d0dfa3941608ab8595fe8bf1d0468b8a21e791db17509291a8d953ca1a48492`。Accepted/DONE 只验收 exact wire、跨语言重现、纯 evaluator
与治理/scaffold 接线，不认证或应用任何知识更新，也不产生 Knowledge/runtime authority。

## Sprint 110（✅ DONE；L3/L4 Build Reviewer strict verdict）— ADR 0063

本切片把 Build Reviewer 的严格性限定在 caller-declared `--materiality L3|L4`：canonical workflow 以唯一
`verdict_contract: reviewer_v1` 选择合同，runtime 在 Agent 启动前验证 readonly/fresh-context/不可写、位于 QA 前且定向回到更早
implementer 的安全 shape，并覆盖 mode skip。strict phase 只接受成功 executor payload 的 exact final non-empty
`VERDICT: APPROVE|REQUEST_CHANGES`；缺失/畸形、dry-run、executor error、从 Reviewer 之后起跑、parallel 或恢复降级均不得放行。
L0–L2 与 `materiality_not_bound` 继续走既有 advisory/fail-open 兼容；省略 materiality 不是低风险判定，runtime 不从 diff/mode/
lifecycle 自动推断。

checkpoint/chain 绑定只服务 crash/recovery consistency；same-UID/admin 仍可删除、替换或回滚 state，不能称为认证或防篡改。
本切片也不认证 Reviewer/implementer/model/provider 身份，不证明 review quality 或 cryptographic SoD，不把 verdict 绑定到
source/context/policy/artifact digest。旧 runtime 对 unknown `reviewer_v1` 应失败关闭；universal init/upgrade 只传播 ADR、workflow、
role card 与 ledger，不安装或替换 host Go/Rust/Kernel/runtime。完整 ReviewCase、normalized finding、independence proof、human
adjudication 和 digest-bound approval 仍属后续 Wave。

正式 scrubbed-environment `forge accept` 为 **ACCEPTED**：**9 PASS、0 FAIL、2 N/A**；N/A 未计作 satisfied。Recursive
Python 为 **55 files / 845 tests**，Node 为 **22 files / 423 tests**，Forge Core Go 为 **1,941 observed tests**，Go 与 Node
examples 分别为 **22 / 47**，五组 Rust observed tests 为 **258 / 54 / 258 / 248 / 164**。Python machine coverage 为
**90.126189%**；聚合 coverage 因 Rust 未配置 coverage tool、Vitest 缺失及顶层 Go 无 module/config 而诚实为 N/A。Go full/race/vet、
Rust all-targets/all-features test/check/build 与 strict Clippy、arch 8/8、2615-file gate、13 项治理检查和 `git diff --check` 均通过。
独立复审发现的 earliest-QA、原生 YAML/JSON duplicate/full-consumption、尾随位置参数、chain stale workflow、Claude case-alias
envelope 与 missing-QA 绕过均已以 fail-closed 回归关闭；最终合同复核在澄清旧持久化格式对所有等级均 diagnostic-only 后无剩余
correctness mismatch。该完成只验收 caller-declared L3/L4 的本地严格转移；不认证 materiality、Reviewer/provider/身份/质量/SoD，
也不把 verdict 绑定到 source/context/policy/artifact digest。

## Sprint 111（✅ DONE；local digest-bound Agent output/review/approval）— ADR 0064

本轮已采纳并整体交付 ADR 0064。完成范围被显式拆为 A–D：A 覆盖 Discover/Design/Review/Build/
Deploy/Rollback/Evolve 七个 canonical workflow 中所有 accepted command-mode Agent output；B 把 caller-declared L3/L4 Build
从 `reviewer_v1` 迁到 challenge-bearing `reviewer_v2`，并在 QA 前后复验 freshness；C 让 Design/Deploy/Rollback positive
approval 只接受 current receipt-bound ApprovalContext；D 把 journal head/receipt/context 引用绑定到 checkpoint/chain v5 并传播
scaffold/upgrade。四块 runtime、迁移、独立复审均已关闭，implementation-roadmap 对应项已勾选。

冻结 wire 使用 `output_binding_contract: local_digest_v1`、hardened `forgeos.product-source-state/v1`、exact prebinding
prompt-context SHA、完整 effective local runtime-policy projection、input/output declared-artifact manifests、每 attempt 32-byte CSPRNG
challenge，以及 `.forge/agent-output-receipts.jsonl` 中 chain-linked `forgeos.agent-output-receipt/v1`。Command success 必须依次完成
exact raw validation、semantic/artifact validation、source/policy/artifact postflight、receipt commit，最后才可发布 accepted Observe；
因此旧实现中 Observe 早于 validator、Raw validator 收到 trimmed Rendered 的两个顺序缺口属于本 Sprint 必须关闭的 runtime 工作，
不是文档存在即完成。

本 ADR 不把 local receipt/context/marker 冒充 ADR-0059 ApprovalRecord、ReviewCase、身份、cryptographic SoD、signed PDP/Grant、
semantic truth、atomic repository snapshot、tamper-proof resume 或 effect authority。旧 host 忽略新 selector 所得结果不构成本 ADR
证据；旧 checkpoint/chain/marker/release receipt 对 opt-in positive path 只可诊断，不能猜测升级。独立最终复审已确认 A/B/C/D
实现面无剩余 P0/P1/P2；完整 acceptance 证据由本轮最终 candidate run 记录，不扩大上述本地 observation/control 边界。

## Sprint 112（✅ DONE；authority-free GraphSnapshot v1 foundation）— ADR 0065

本轮冻结通用 `GraphSnapshot v1` stable project-scoped semantic-name identity、31 node/20 relation taxonomy 的方向/endpoint/axes、
source/extractor provenance、closed node/edge/unresolved/crosswalk/coverage/freshness shape，并交付当前唯一
`adr-0053-selected-go-module-lexical-partial-graph-snapshot-v1` profile。它只消费 caller-supplied exact ADR-0053 graph bytes；Go/Python
pure projector、显式输入 CLI、strict full-reconstruction checker、exact golden、registry v20、Skill 与 universal fresh/legacy scaffold 已接线。

Rich golden 精确产生 9 nodes、12 resolved edges、3 unresolved nodes、11 unresolved edges 与 8 个 ADR-0062 crosswalk；所有未观察
surface、system knowledge 和 freshness 保持 PARTIAL/UNKNOWN。独立复审发现并关闭 generic array 误用 edge 81,920 特例、字段字符串限界、
future-profile error classification、aggregate locator precheck 与 crosswalk identity collision；Go/Python 对 exact envelope 逐字节相同。
该交付不是 live producer、selected build、authenticated provenance、完整 System Knowledge Graph、Impact/Cost/Risk、G3、Assessment Join、
persistence 或 authority；Rust与多 surface extractor仍未交付。后续 lexical test-source 扩展必须使用独立 profile，不能改写 ADR-0065
golden，也不能把 `_test.go` 存在冒充 test discovery/execution/PASS。

## Sprint 113（✅ DONE；Local Go lexical test-source GraphSnapshot profile）— ADR 0066

本轮以独立 request/envelope API 和
`adr-0053-selected-go-module-lexical-package-test-source-partial-graph-snapshot-v1` 显式 profile 扩展 ADR 0065 foundation。
每个且仅每个 `test_files` 非空的 ADR-0053 package 生成一个 package-scoped lexical test source-set node 与一条 module→test
structural `contains` edge；`p`/`p_test` 保持独立，diagnostic 不猜 package，且不生成 package→test、`verified_by` 或 `observed_by`。

Go/Python pure projector、显式 CLI dispatch、strict full-reconstruction checker、第二 exact golden、当时的 registry v21（现为 v22）、Skill 与 universal fresh/legacy
scaffold 已接线。Rich golden 精确产生 11 nodes、14 resolved edges、3 unresolved nodes、11 unresolved edges 与 8 个 ADR-0062 crosswalk；
Go surface 为 9 nodes/10 edges，test surface 为 2 nodes/4 edges，两者构成 resolved records 的互斥 PARTIAL partition。ADR 0065 的
API/Schema/golden bytes保持不变；scaffold 不安装 Catalyst-only Go host runtime，legacy upgrade ledger 纳入 ADR 0066/Schema/fixture 与共享 Python。

该 profile 只证明 exact lexical source-set projection，不解析 test declaration/case，不 compile/run tests，不产生 PASS/FAIL、coverage、flakiness、
verified subject、truth、authority、completion、persistence、execution、Impact/Cost/Risk、G3、Assessment Join 或 effect。system knowledge 与
freshness 恒 UNKNOWN；Rust 与 Wave 2 多 surface extractor 总项仍未交付。

最终 scrubbed-environment `forge accept` 为 **ACCEPTED**：**9 PASS、0 FAIL、2 N/A**；N/A 未计作 satisfied。Recursive
Python 为 **59 files / 890 tests**，Node 为 **22 files / 423 tests**，Forge Core Go 为 **2,114 observed tests**，Go 与 Node
examples 分别为 **22 / 47**，五组 Rust observed tests 为 **258 / 54 / 258 / 248 / 164**。Python machine coverage 为
**90.30259623992838%**；Rust coverage tool、Vitest、顶层 Go module/config 缺失的 coverage 项诚实为 N/A。Go full/race/vet、
Rust all-targets/all-features test/check/build 与 strict Clippy、arch 8/8、2,749-file gate、13 项治理检查、fresh 8/8、legacy
upgrade 3/3 与 `git diff --check` 均通过；独立最终复审为 CLEAN。验收命令显式清除了
`OPENAI_API_KEY`/`OPENAI_BASE_URL`/`ANTHROPIC_API_KEY`，没有调用付费 provider。该完成仅关闭 ADR 0066 的窄 lexical
test-source profile，不改变前述 UNKNOWN/非 authority 边界，也不勾选 Wave 2 多 surface extractor 总项。

## 下一前沿(需外部资源 / 后续阶段 / 投机增强 / 明确非目标,非本环境可完整验证)
- **Graph 下一协议切片**:SQLite v17–v24 已交付 successor candidate、per-node request/lifecycle、receipt/content dataflow、wave-ready/admit 与 8 MiB successor candidate 上限；ADR-0096/0097 又交付 read-only whole-schedule reconcile 和 successor-capable zero-effect ready release。ADR-0098 的公开 max-one effectful step、跨 family Project lane 与 exact-owner adjudication 已通过独立复审及正式 clean-clone acceptance，ROADMAP 对应实现项已闭合；ADR 本身仍为 Proposed。下一独立协议才是 durable whole-Graph controller；并发 wave 的失败传播/自动恢复和任意 event-prefix branching 仍更晚。不得把 `ready` observation、逐节点 operator 调用或 Hub-local single-consumption 冒充顶层循环、自动第二节点或远程 exactly-once。
- **真点火** `--agent-cmd=claude`:**multi-agent running to completion 已坐实**(Sprint 25:真 claude 多-agent 跑到 converge MET,增量级 + 版本级)。完整旋钮:四维资源护栏 + 成本三维(phase/时间/美元)+ 任务注入 + 写权限 + 模型路由 + 工作目录 + retry + loop-back;诚实分工:agent 自治增量绿、人确认版本竣工。docs/ignition.md 有完整配方 + 实测
- **外部资源状态**:~~SCA/CVE 漏洞库~~、~~Firecracker 主机前提~~与~~LiteLLM 双后端主机验证~~均已解决；Go Docker/Firecracker runner 也已接入执行器。剩余项是完整 coding-workspace 交换/隔离硬化与生产 provider registry/policy，它们属于后续产品契约，不再是本机外部资源阻塞。〔真 cost/latency telemetry **已达成**——S26 真 claude 补齐真 token/cost/latency 数据,scorecard 三维真值落盘〕
- **投机增强(做即违反反 gold-plating 纪律)**:embedding 语义检索(TF-IDF 已工作,增量仅真点火时体现)
- **后续阶段**:Web UI/`forge-web` 仍属于 v3 目标架构，但不在当前 CLI/声明式核心交付阶段
- **生产跨厂商路由**:`internal/routing` 六维评分已接入 run/evolve；本机 LiteLLM 双后端只证明网关前提。生产 provider registry、健康/容量/成本策略和 operator 治理仍属 v3，不能从一次主机验证推断已经交付。
- **明确后续契约**:`on_approved` 当前只路由；若未来要在批准时物化 `.agent/*`，必须先采纳 producer/source mapping、新鲜度与原子提交契约，不能恢复已被 `forge check` 禁止的无主 `on_approved.emit`。
- **`readonly`/`on_rejected` 的真 claude 进程验证——用户已明确决策终止于此(2026-07-03)**:两机制均已真实实现(非声明未接线);readonly 路径限定按官方文档契约构造 + 单测坐实 argv、on_rejected 用 fake-agent 脚本端到端坐实目标阶段、失败保留与成功消费语义,但都未过真实付费 `claude` 进程验证运行时行为。征询用户是否授权花真实 API 预算推进最后一道经验验证,用户选择「单测已足够,就此打住」——非遗留缺口,是知情决策后的终态;若未来有人想补这道验证,预算授权需重新征询。
- **需求清单本身**:`docs/FUNCTIONAL_REQUIREMENTS_AUDIT.md`(Sprint 30 起 + Sprint 31 修订)是本仓当前唯一的显式功能需求清单,derived from 项目自己的声明源头;后续 sprint 如声明新机制,应同步补一行,不要让清单本身漂移回「不存在」。

历史勘误：Sprint 32 记录的 `/dev/kvm` 存在且可读写是 2026-07-16 当时的探测；
2026-07-27 该设备仍存在，但当前用户已无读写权限。旧 Sprint 31 的 deny-before-allow/Edit+Write
描述也已由本 Sprint 的 `dontAsk` + exact `Edit` 契约取代。
Sprint 31 的“loop-back 开始即消费 rejection”也是历史行为；当前契约为失败保留、
成功完成 rework 后消费。

**stop_condition:** roadmap 完成度 / 闸门全绿(非「继续 N 轮」)。

## Sprint 114（✅ DONE；Proposed-only ADR v2）— ADR 0067

交付 `forgeos.architecture-decision-record/v2` 的新建 Proposed 文档边界：exact compact canonical JSON frontmatter、固定且非空 Markdown body、filename/ADR ID/H1 title 绑定、sorted declarations、validation-owner closure、normalized implementation locators 与 body/self domain-separated digests。Universal Python checker 只读显式文件或 byte-pinned physical golden；Catalyst Go 保留 `writes_adr` 既有 baseline integrity snapshot，但只对 current attempt 唯一新增候选做 v2 验证，旧 ADR 不做 v2 parse、retro-validation、migration 或 rewrite。

Registry v22、activation/context routes、shadow/non-load-bearing detector、ADR Governance Skill、governance integration 与 fresh/legacy scaffold 已接线。owner/approver、Claim/Evidence、affected Graph node 都是 caller/author declarations，不认证 identity/SoD/ApprovalRecord，不解析 truth 或 graph coverage。正结果只表示 proposed document structure/bytes valid；Accepted immutability/supersession/compliance、persistence、lifecycle transition、execution/effect 与 legacy query/migration 仍明确未交付。

最终独立 fresh review 对 valid→valid 字节漂移、retry baseline/target retarget、mode-gated disabled 路径、lexical H2 和 legacy byte-scan 边界均给出 CLEAN。去除外部模型密钥后的完整 acceptance 通过：61 个 Python suite/915 tests、22 个 Node suite/423 tests、forge-core 2,136 tests、examples 69 tests、多组 Rust all-target/all-feature tests，以及 gate/arch/security/SCA/typecheck/build；缺失或未配置的 lint/coverage 工具保持诚实 N/A。冻结 pins：policy `700b88aaf1543f190764004396bb13e76475d2e67373bbc743310df04b58e35f`、Schema `ff3f00b1060b2d777b142947ef1ec9c0920782613d941aa672aecd242cf0341b`、golden `b37dba8cc6d2750bb0ed73c7ee5b3ae61ad25551ec258584ed14618f1cb5c194`。

## Sprint 115（✅ DONE；authority-neutral Capability Registry v1 evaluator）— ADR 0068

冻结一个显式、只读、content-addressed 的 Capability Registry 与 pure declared-resolution 边界。首个 physical entry 只绑定已交付的 `local-go-package-impact-prescan/1` Go/Python实现、Schema、golden 与测试；它不注册 broader `change-impact-analysis`，也不把历史 `repository-reader/1` 的 opaque `888…` contract reference 视为真实内容摘要。Registry/owner/test/implementation 均不认证，resolution 固定无 authorization/permission/invocation/effect/transition/runtime-routing/persistence attestation。

交付范围是 Go/Python strict canonical validator/resolver、单一 `forge capability-registry` 显式输入 CLI、physical checker、three-case cross-language golden、registry v23、Skill/activation/routes/shadow detector/governance/scaffold 接线。ADR v2 frontmatter 继续是 `proposed`，Registry wire 继续是 `staged`；治理 delivery 元数据不伪造 acceptance 或 lifecycle promotion。Registry semantic pin 为 `23b9acd4133598cd1404c78c71f694b4a99c398652e95c21896a507be5ecacf4`，policy pin 为 `d999e1f7054868d99ede5f4d6f491ed819c2b5dd800a5542343b688c05c31cce`，Schema pin 为 `f5c5c5abc68e9c5f5d80dce66bb5b97e4e4dedc8cc69189bcc28612991f1ea81`，golden pin 为 `0ce4929ad82ce70ef0520be80b7bd3eaf47f5ff1205d0a53e12fbe1115ed11b5`。

明确仍开放：140-item planning catalog projection/coverage、catalog→package adapter generation、CapabilityInvocation、Grant/PDP、implementation selection/execution、plugin lifecycle 与 runtime routing。只关闭 implementation roadmap 的“最小 Capability Registry”和 Wave 4 registry schema 两项。

**stop_condition:** Schema/ADR、Python/Go exact resolver、physical golden/checker、fresh/legacy scaffold、governance pins、独立 fresh review 与 scrubbed acceptance 全绿；不得以本切片关闭 catalog adapter generation、Grant/PDP、CapabilityInvocation、plugin lifecycle 或 runtime routing。

## Sprint 116（✅ DONE；Planning Capability Ownership Projection v1）— ADR 0069

从 caller-supplied exact planning catalog 与 mapping bytes 交付 bounded pure projection。Python/Go 独立 strict YAML parser/projector 对同一 physical golden 逐字节重建 request、140 bindings 与 projection，覆盖 17 nodes、145 occurrences、140 unique fine capabilities、38 declared packages，且每个 capability 恰有一个 primary owner；重复生命周期使用保留全部 node IDs 与 occurrence count。产品 CLI 精确为 `forge capability-ownership project --catalog FILE|- --mapping FILE|-`，option 可交换且恰一 stdin；usage=2，input/semantic=1，前三类失败在首个 stdout write 前保持 stdout 零字节，成功为 canonical projection+LF，底层 partial write 失败则产物无效。

Registry v24、Schema/source/golden pins、non-owner governance Skill、activation/routes/shadow detector、治理回归和 source-only fresh/legacy scaffold 已接线。Scaffold 复制 exact sources 与 universal Python checker，但不复制 Catalyst Go runtime、不从 38 owner names 生成物理 Skill/adapter；已有同名 Markdown 也保持 `physical_resolution:not_performed`/`skill_availability:not_evaluated`。ADR 仍为 Proposed，ADR 0068 singleton Registry 不变；只关闭 implementation roadmap 的 complete unique primary-owner coverage + logical adapter refs 一项。Package implementations、physical adapters/portable Skills、capability↔role↔workflow↔artifact↔gate↔permission cross-reference、Grant/PDP、CapabilityInvocation、plugin/runtime routing、persistence/transition/effect仍开放。

冻结 identity：catalog `33000/bc6efe535539c5f129af51486d8e81b9844b5ee6448fae2bce649fc159658d74`，mapping `5924/bfb2277fe66cd9f0c609b5be10ad77ad0969603edd19e5a6ccbe38b8e3409462`，golden `172733/3d0a877bef0939cff5752fc5d602e0d3a90e19639308801008f9d2d9ff139f36`，request `3639c4d3ad21db93db254b7da2643d492ca39c4dda5438de426379cd70718cfa`，projection `53754ded32379d6520f3bd2b9d2956238731ad40c11124be457b724b4c150fa2`，Schema `a2ed6eb754c07478eeaaf2ae73a889ba985553c4220a7b6771be9e6a36078083`，governance policy v24 `b583e7097baa8a7aadfacb873318a40acfa3aaf70a6d3f074f6e4107a7c315df`。ADR v2 body/self/physical 为 `c1dbafc35a9cab89e827de7e89ad8f253b8a145eba0aece661b5b3198d45755d` / `95982bd03ce7bc5d12fe56a6eb7c18b533fef1798c66eea490bb62ef9b530386` / `070768f67e57ec2f5cdfda12b9448c6f74427d34b8c177d8abd59189aeb3b546`，状态仍为 `proposed`。

**stop_condition:** Python/Go golden、Schema/ADR v2、registry v24/governance、source-only fresh/legacy scaffold、focused gate/arch、fresh review 与 full acceptance 全绿；不得把 logical locator、同名文件或完成裁决冒充 physical Skill、Registry mutation 或 runtime authority。

## Sprint 117（✅ DONE；`project-snapshot` narrow package slice）— ADR 0070

ADR 0070 保持 Proposed-only，交付 Linux-only `forge project-snapshot capture` live producer、
strict Go decoder 与独立 Python checker/golden，以及 closed source-distributed
`skills/project-snapshot/` portable package和 `.agent` adapter。两次完整 Git worktree endpoint
observation 只绑定 allowed single-link regular bytes、tracked-absent facts、hashed pre-read
sensitive/control/symlink exclusions、ignored count 与 exact 12-surface coverage；结果固定
non-atomic，currentness/freshness/system completeness UNKNOWN，Git/HEAD 未认证，path policy 不是
content DLP，authority/permission/truth/persistence/effect 全 false。

Governance Registry v25 同时将 strict checker 列为 shipped evaluator、Linux capture 列为 shipped
local producer；shadow detector、activation/routes/disciplines、audit/decision/index 与 roadmap
nested item 已接线。Fresh/legacy scaffold 复制 portable package、adapter、ADR、Schema、golden、
Python checker/tests，但不复制 Catalyst Go runtime、不安装 host Skill、不授予 filesystem/process
permission；unsupported host 或 runtime 不存在固定 exit 3/`not_executed`，已存在但不兼容/执行失败
固定 exit 1，且禁止 fallback。Implementation roadmap 的
38-package parent 与其余 37 packages、Graph/config/deployment semantics、formal roles/cross-reference
runtime、plugin lifecycle、Grant/PDP/CapabilityInvocation/routing/persistence/effect 继续开放。

**stop_condition:** Schema/golden/ADR/portable-manifest/governance pins、Go/Python/package/Skill checks、
fresh and legacy scaffold、fresh dangerous/normal review、focused gate/arch 与 scrubbed full acceptance
全绿；只勾 `project-snapshot` nested item，父 38-package 项保持未勾。

## Sprint 118（✅ DONE；`context-engineering` narrow package slice）— ADR 0071

ADR 0071 保持 Proposed-only，把 ADR 0055 已冻结的 authority-free ContextPackage v1 Python
implementation 包装为 closed 16-file `skills/context-engineering/` source package、零参数 exact canonical
stdin assembler 和 strict physical manifest checker。Registry v26、activation、shadow/non-load-bearing detector、
routes/disciplines、docs/audit 与 roadmap nested item 已接线；Schema/golden/wire/bounds/digest domains 及
Python/Go/Rust semantics 不变。

Fresh/legacy scaffold 复制 ADR、closed package 与既有 universal ContextPackage assets，但不复制 Catalyst
Go/Rust runtimes、不安装 host Skill。Package 不发现 repository/ambient source，不调用 provider/model，
不编译 live prompt，不认证 publisher，不提供 atomic check-to-use、Grant/PDP/Approval、truth/instruction、
completion、persistence、runtime routing 或 effect authority。`-I` 排除 script/current directory、
`PYTHONPATH` 与 user site，但不隔离 system site、stdlib、interpreter startup 或 host。只勾
`context-engineering` nested item；38-package parent 与其余 36 package items 保持未勾。

冻结 identity：Schema `2e2a934393026c96ebe7e2098462303192fd345aae10eebcf79544a69d7621e3`，
golden `1a1c9866f7472055736866be9007040cc8e3d938bb04244bd04fd3bec2aa4b55`，portable manifest
`7590df136eb828ba3ffe4892efffa2ab4a77fb87dff8a1bffccdde2d015852c5`，ADR body/self/physical
`92f2a415e51fac94f3ce61203b7eb3152efb4e18a0233f91e2fc00558cf4b84d` /
`ed72467dddb730de425278d49c8c6bdb9e6f8a82904c8fa5a8eda6ce339fd101` /
`455f097be6c6e8e658d7a92a60d9e50b08ef89300aa13accccac4bbf67098c84`。

**stop_condition:** package checker/tests、official Skill validation、ADR strict、registry v26 pins、focused
governance/agent/scaffold/gate/arch 与独立 fresh normal/dangerous + fresh/legacy acceptance 全绿；不得把
source package delivery 冒充 live prompt/provider/model/PDP/authority/runtime/persistence。

## Sprint 119（✅ DONE；`evidence-claim-management` narrow package slice）— ADR 0072

ADR 0072 保持 Proposed-only，把 ADR 0045 已冻结的 authority-free EvidenceRecord/
KnowledgeClaim v1 Python validator 包装为 closed 18-file `skills/evidence-claim-management/`
source package、零参数 explicit-EOF exact canonical stdin adapter 和 strict physical manifest checker。
Registry v27、activation、shadow/non-load-bearing detector、disciplines、docs/audit 与 roadmap nested
item 已接线；portable prose 不进入 authenticated context routes，ADR-0045 Schema/golden/wire/
bounds/digest domains 及 Python/Go/Rust semantics 不变。

Fresh/legacy scaffold 只复制 ADR、closed package 与 governance checker/test，不安装 host
Skill。Package 只验证 already-authored record-set bytes，不观察或 author、修复、排序、
补 digest、返回或持久化 records，不访问 ambient source/journal/semantic view/proposal，
不提供 atomic check-to-use、truth/instruction/Grant/PDP/Approval/completion/routing/transition/
execution/effect authority。`-I` 排除 script/current directory、`PYTHONPATH` 与 user site，但不
隔离 system site、stdlib、interpreter startup 或 host。只勾 `evidence-claim-management`
nested item；38-package parent 与其余 35 package items 保持未勾。

冻结 identity：Schema `b2f8824c95012d94e71b4643756890a7a23f67dc1b9e0e8ecacf979b016864e8`，
golden `db111600f93e63b3533b1f06b14d7520eb4cbec0e4c6d0e3a6e0fd7e2740824a`，ADR 0045
physical `a04479075dc60828176cd7e68857dcc4f3fc92bb4ae4b567f2caddd93f478b81`，portable
manifest `b5d0d15497f47d4310729e7eadf2df506b0c90a1ae982b30b5b453536e98c771`。ADR 0072
body/self/physical 为 `9aa8871ca9024c163ac83677a7c6f289c0579e1b4a92c8535e950b1d34b4c895` /
`4aa14c22cb0c49a701764b611af045baaeabdb4af6a3144a75423fecd076e741` /
`5ed33ea8d0a7e44e0ff401fad438c0fce0a875914da1187a64cb6cc3452b4929`，registry v27 policy pin 为
`eeba777fff4439e02b19623b66ea336ba1a08e865cd798a487e5a70a1b443991`。

**stop_condition:** package checker/tests、ADR strict、registry v27 pins、focused governance/agent/scaffold/
gate/arch 与独立 fresh normal/dangerous + fresh/legacy acceptance 全绿；不得把 structural
validation 冒充 record authorship、truth、provider/model/PDP/authority/runtime/persistence。

## Sprint 120（✅ DONE；`policy-authority` narrow package source-governance slice）— ADR 0073

ADR 0073 保持 Proposed-only，把 ADR 0056 CapabilityGrant 与 ADR 0059 ApprovalRecord 已冻结的
authority-neutral pure declared evaluators 包装为 closed 30-file `skills/policy-authority/` source
package、两个独立零参数 explicit-EOF exact canonical stdin adapter 和 strict physical checker。
Registry v28、activation、shadow/non-load-bearing detector、disciplines、docs/audit 与 roadmap nested
item 已接线；portable prose 不进入 authenticated context routes，两个 Schema/golden/wire/bounds/
digest domains 及 Python/Go/Rust semantics 不变，scope 未扩大。

Package 不新增 combined envelope，不签发/批准/激活/撤销/预留/消费/持久化/执行，不读 ambient
repository/environment/clock/identity/policy/approval/revocation/usage/runtime，不调用 ADR-0057/0058、
Kernel/PDP/PEP 或 executor，不提供 atomic check-to-use、effective Approval、authorization、permission、
completion、routing、transition 或 effect authority。`-I/-B` 只约束 Python import/bytecode 边界，
不认证 system site、stdlib、interpreter、host 或 publisher。Source-only fresh/legacy scaffold 只复制 source、
不安装 host Skill/runtime；只勾
`policy-authority` nested item，38-package parent 与其余 34 package items 保持未勾。

冻结 identity：Grant Schema/golden `dd26568ec430ae5e444ae851ba2b58087528a17e84794137268be3860d9c3209` /
`0261a682bddca2f27976a9cd663350e8cf222685389fecc7ad8ae536083fef35`，Approval Schema/golden
`bc11d2b066bac35252bff6739798c3e30a508ed31fca0306b9cf1cdc0ef9ab64` /
`501320b9f65775091e67ba22c6e7faa5b5ecaa1f1b472a1a196da93c7ab81978`，portable manifest
`feb21737424b0133e8b57f553ff342b51583917f83e1d47b4b83cd6c3a667132`。ADR 0073 body/self/physical
为 `729fd91714d43244f3ac23f182007289ee4cd21a4abd0bf7fe51253eefadbf86` /
`a92f4ef3d22ceab5264316863e396182eadc84a9530803a43af3ed723144cecd` /
`cb1a9adff937e39f3d42b052e19e7e0e1516968da967948508b45dd735bed619`；registry v28
policy pin 为 `458403f3aa8c6c1250d8602cbd44723c1112bbb06611d60859eb0d2263eb78ed`。

**stop_condition:** package checker/tests、unchanged Python/Go/Rust pure contract suites、ADR strict、
registry v28 pins、focused governance/agent/check/gate/arch 与 source-only fresh/legacy scaffold 全绿；
不得把 source copy 或 declared relation 冒充 host installation、policy/effective Approval/authority/runtime/effect。

## Sprint 121（✅ DONE；`adr-governance` narrow package source-governance slice）— ADR 0074

ADR 0074 保持 Proposed-only，把 ADR 0067 已冻结的 Proposed-document pure validator 包装为
closed 25-file `skills/adr-governance/` source package、exactly-one-basename-argument explicit-EOF
exact document stdin adapter 和 strict physical checker。Registry v29、activation、shadow/non-load-bearing
detector、disciplines、docs/audit 与 roadmap nested item 已接线；portable prose 不进入 authenticated
context routes，ADR-0067 Schema/golden/wire/bounds/digest domains 及 Python/Go semantics 不变，scope 未扩大。

Caller-supplied basename 仅是独立 lexical label，不证明 physical file、repository path 或 identity。
Package 不新增 request envelope，不扫描 repository，不 author、repair、normalize、reseal、accept、
supersede 或 persist ADR，不复制 Catalyst Go `writes_adr` runtime，不提供 atomic check-to-use、identity、
ownership、approval、truth、Graph、compliance、immutability、lifecycle、completion、execution 或 effect authority。
`-I/-B` 只约束 Python import/bytecode 边界，不认证 system site、stdlib、interpreter、host 或 publisher。
Source-only fresh/legacy scaffold 只复制 source、不安装 host Skill/runtime；只勾 `adr-governance` nested
item，38-package parent 与其余 33 package items 保持未勾。

冻结 identity：Schema/golden `ff3f00b1060b2d777b142947ef1ec9c0920782613d941aa672aecd242cf0341b` /
`b37dba8cc6d2750bb0ed73c7ee5b3ae61ad25551ec258584ed14618f1cb5c194`，ADR 0067 physical
`78c7d484cfb0e448c4c896440d4ea272a8e32a60f947539a3ad739baaeead71e`，portable manifest
`88fb16e51af69cb3a2bc38fe2dcae7893a24cee744b85a06eafff70ae841dd3c`。ADR 0074 body/self/physical
为 `a18646f93391a1413d690853a35e5a2ca6a17eb498dcf970696e3606074fb875` /
`15c996fc2286a011a1b99f1d859b506cd6658b0f0e40afbaf97af767dcfb7d65` /
`21d452845cf0f2889fcc5fa22f450cc4a40d5fb694f5b1f202d4b3cfd79f2eb2`；registry v29 policy pin
为 `60a94a2aba34a8d04fb95e9eea51deeffcbc22f871824678a79e8347d282e2df`。

**stop_condition:** package checker/tests、unchanged Python/Go ADR validators、ADR strict、registry v29
pins、focused governance/agent/check/gate/arch 与 source-only fresh/legacy scaffold 全绿；不得把 lexical
basename、structural marker 或 source copy 冒充 physical identity、acceptance/compliance、host installation、
lifecycle/runtime/persistence/effect authority。

### Sprint 122 — Portable `knowledge-graph-curation` partial projectors（DONE）

ADR-0075/Registry v30 以 `c9b8397658c3bcecb474966a3efd155f0af550be4fe7319dcdbf23a63cec2008` manifest pin 分发 closed 46-file source package。两个 zero-argument explicit-EOF adapters 分别复用 ADR-0065/0066 的 exact eight-field request 与既有 envelope；不新增 wrapper/union/dispatcher/profile ABI、authenticated route、live producer/runtime、graph store、impact 或 authority。Coverage 保持 PARTIAL，system/freshness 保持 UNKNOWN，test source-set 不表示 test execution/outcome/coverage/verification。

### Sprint 123 — Portable `change-impact-cost-risk` lexical prescan（DONE）

ADR-0076/Registry v31 以 `d46202beacc000c6fbdc14afb1c5996476af90d9c0e8927da6f1bf56bf354ad5` manifest pin 分发 closed 32-file source package。唯一 zero-argument explicit-EOF adapter 只消费 ADR-0062 已有 exact seven-field canonical request，并输出已有 envelope；不接受 raw/parsed graph、fixture/envelope wrapper、union、dispatcher 或 mode。Schema/golden pins 为 `a4592c63a938c090ccc4d6c8187bba8f37909ef6c2d2253fd06f656623c2bb25` / `bc364e387705651d307a3ff18137b857a3fad2c518685a358bba169a835a68d9`；ADR-0076 body/self/physical 为 `c1097bc6db2f88058f7b4d2af1aeacee0400b035545e01bed0499199525880a5` / `63aa497ce38b8d1182d128cd4227eb45690f9c01cd7c7dbae7c328028418398e` / `d7df301a4236be84e866a05c54089e79507db13ffba08ab85f955d27c3dc8b01`。

Lexical closure 只在 caller-supplied ADR-0053 observation 内按 ADR-0062 完整；system impact 恒 UNKNOWN，zero dependents 不等于 no-impact/safe/low Cost/low Risk。Package 不 capture live repo/graph/build/test/runtime/cross-surface，不提供完整 Impact/Cost/Risk/materiality/safety、route、host installation、persistence 或 authority；仅勾 nested item，其余 31 个 package items 与 Wave 2 Impact Closure 保持开放。

### Sprint 124 — WorkIntent v1 Proposed candidate governance（CHECKED）

ADR-0077 以 Python/Go/Rust exact golden parity 冻结 authority-neutral WorkIntent v1 Proposed candidate；Schema/golden/record pins 为 `3b02fab59eae8767c86caaa73d0830adcbd92825045b7f27db0c3eca5ee10e01` / `8e80553677ebf9f6548a15be4c3cb4ccc8aa6825010a20f2e890e91d1cd7ed7b` / `2fe0424d30405a8b1d716afc99bbd38d602375f3316fd1c54c472890d520a225`。ADR-0078 另行提出 Registry v32 candidate-only metadata、checker-only shadow 与 source-only Python distribution；Go/Rust 保持 Catalyst-only，scope arrays 不变，context route 中没有 WorkIntent。

WorkIntent v1 Proposed candidate 不被接受为 semantic authority，不认证 origin/requester/owner，不 resolve refs，不评估 freshness/materiality/scope，不关闭 G0，不创建 route/runtime/evaluator/producer/consumer、Run、RunJournal、lifecycle、Approval、Grant、persistence 或 effect。该证据项不勾选 `change-intake-orchestration` package，parent 与其余 31 个 package items 保持开放。

### Sprint 125 — Authenticated ADR approval v1 Proposed prerequisite（CHECKED）

ADR-0079 冻结 caller-supplied structure/digests/relations 和 dependency-free Python structural core；ADR-0080 另行提出 Registry v33 candidate-only metadata、checker-only shadow 与 Python source-only distribution。Schema/golden/proposal physical pins 为 `9882e45816f3c3a6e2d84ba09d942848dcc1eae90d3d5193b9cf18b6ebe27198` / `936b989856ff733e2de848ba9907c10f9f626aa188648fc60372775e44dbc7b5` / `6beabf33656998b942036b63c90db99c6a5f9b138cf2e5bd4a5372ec8e1ad1f2`，scope mapping canonical SHA-256 保持 `8ba82b638e8031f0d1be2b9ea6d522a4b9cf064a4ed532e1f0d3281f2dfe874c`。

该 Proposed prerequisite 不验证 Ed25519、不认证或授权、不签发 receipt、不消费或证明 external root pin、trusted time/revocation currentness，不提供 CAS/durability/Accepted lifecycle、G0 closure、Skill、route、scope/evaluator/producer/runtime、persistence 或 effect；不复制 future Go service、production keys/state。Full authenticated approval、ADR lifecycle 与 package rollout 保持开放。

### Sprint 126 — Authenticated ADR lifecycle v1 Proposed candidate（CHECKED shared governance）

ADR-0081 Go approval authority 已经独立 StoredAuthorization seam review，但 Registry v34 只记录其 Catalyst-repository-only evidence；ADR-0082/0083 冻结 lifecycle Python structural candidate、Schema/golden/three proposals、exact20 core pins 与一个 checker-only shadow。Scope canonical SHA-256 保持 `8ba82b638e8031f0d1be2b9ea6d522a4b9cf064a4ed532e1f0d3281f2dfe874c`，无 Skill、route、kind/evaluator/producer/runtime。

生成项目只允许 Python source closure，不复制 Go authority、production root/key/state。Full authority-bearing lifecycle、repository mutation、Accepted source、atomic durable publication、architecture compliance、G0 与 per-package rollout 仍开放。

### Sprint 127 — Registry v35 lifecycle authority evidence（Proposed shared governance）

ADR-0084 exact44 Go lifecycle authority 已独立冻结，ADR-0085 只把它登记为 Catalyst-repository-only evidence。Registry v35 保持完整 scope hash、既有 checker-only shadow 与无 route/Skill/runtime 边界；generated 只接收 exact4 governance source，缺 Go 时仅跳过 Catalyst implementation 审计。

### Sprint 128 — Registry v36 legacy governance read import（Proposed shared governance）

ADR-0086 exact15 pure core 已独立冻结；ADR-0087 登记 exact supplied Memory/ADR bytes 到 `unverified_legacy` read-only view 的 source-only Python candidate。Registry v36 保持完整 scope digest，checker-only shadow 仅声明真实零参数 argv；operator 显式 pipe request 并关闭 EOF。Catalyst exact10 Go parity、ambient path reader、database、state、route、Skill、service、runtime 与 authority 均不分发；exact18 fresh 与真实 Registry v35 upgrade scaffold 已闭合，implementation roadmap checkbox 已完成。

### Sprint 129 — Registry v37 Kernel operational reference（Proposed shared governance）

ADR-0088 exact15 dependency-free Python core、Catalyst exact11 Go/exact13 Rust module parity 与 ADR/Schema/golden 已冻结；共享 Rust `lib.rs` 只要求 operational registration 恰好一次，不整文件 pin。ADR-0089 只登记五类 operational reference records 加 nonsemantic acyclic closure。Registry v37 保持 scope digest，detector 精确运行 pinned-golden argv；source distribution 为 Python exact18，绝不复制 Go/Rust 或 runtime registration。Fresh generated core 33/skip2、governance 12/skip2、真实 v36 added18/changed34/second0 与旧 v35/v34/v33 inverse 均闭合。十四项 authority/effect attestations 为 false，完整 Kernel ABI parent 保持开放。

### Sprint 130 — Registry v38 Kernel decision reference（✅ DONE；Proposed ADRs）

ADR-0090 exact16 dependency-free Python core、Catalyst exact13 Go、flat exact9 Rust parity 与 ADR/Schema/golden 已冻结；共享 Rust `lib.rs` 只要求 decision registration 恰好一次，不整文件 pin。ADR-0091 仅登记 CognitiveAtom v2、DecisionTransaction v1 及对 operational records 的单向 structural reference closure。Registry v38 保持完整 scope digest，detector 精确运行 pinned-golden argv；source distribution 为 Python exact19，绝不复制 Go/Rust 或 runtime registration。22 项 attestations 均为 false，declared authority/hardness 不生效，instruction disabled；无 Skill、route、runtime、PDP/controller。两份 ADR 继续 Proposed；repository-slice 治理已通过正式 `forge accept`，窄 roadmap 项完成。ADR-0038 仍 ADOPTED-PARTIAL，DecisionCapsule、AuthorizedTransactionSpec、authenticated PDP 与 rolling controller 保持开放。

正式 Candidate 验收为 **ACCEPTED**（9 pass、0 fail、2 个诚实 N/A）：Python 92 files / 1323 tests，Node 32 files / 460 tests，Go 2466 tests，Rust 五组 observed tests 334 / 54 / 334 / 248 / 164，examples 22 / 47，真实覆盖率 83.45830729709088%。隔离凭证日志 `/tmp/forgeos-adr0091-candidate-acceptance-rerun.log` SHA-256 为 `9892543e5f82bcf82d10e1ea1bed4ce98c07709e4e729ceb8423dae12d6897b3`；pre/post provenance 四项全等。晋级后 Registry v38 policy physical SHA-256 为 `63b44231ae33a9788177db0d348b94d76ef368a8bcec2c9d67f4dabc7dace271`，decision governance module 为 `a8d4ff8c2085b990bfb6c827968fc0402f5fde886f04611d3bac6aad0b07306b`，exact19 aggregate 为 `ad7220c2c02012cab4eb4a36adc0419142b9bbc7612496165197ea994e217b46`；ADR-0090/0091 bytes 与 exact16 均未改变。

### Sprint 131 — Registry v39 Decision Capsule structural replay（✅ DONE；Proposed-only）

ADR-0092 exact16 dependency-free Python core、Catalyst exact15 Go、exact14 Rust parity 与 ADR/Schema/golden 已冻结；共享 Rust `lib.rs` 只要求 registration 恰好一次，不整文件 pin。ADR-0093 仅登记 `StructuralReplayManifest → DecisionCapsule → EvaluationBranch → StructuralReplayClosure` 的 caller-supplied validate/reseal/compare DAG；专用的后挂载 ReflectionReport refs unresolved 且 outer-only，上游 ArtifactRefs 保持 opaque/uninterpreted。Registry v39 保持完整 scope digest，detector 精确运行 pinned-golden argv；source distribution 为 Python exact19，绝不复制 Go/Rust 或 runtime registration。

32 项 attestations、effect replay/history rewrite 与 replay controls 保持 false；窄 repository-slice completion claim 在正式验收后为 true，其余六项 broader completion claims 保持 false。无 Skill、route、runtime、model/rule/world-state/history/Reflection consumer、persistence、PDP/controller。ADR-0092/0093 始终 Proposed/null；exact19 source-only distribution、independent review、fresh/legacy scaffold 与正式 `forge accept` 已完成，roadmap checkbox 已勾选。正式验收为 **ACCEPTED**（9 PASS、0 FAIL、2 honest N/A）。ADR-0038 仍 ADOPTED-PARTIAL，完整 DecisionCapsule、AuthorizedTransactionSpec、authenticated PDP 与 rolling controller 保持开放。

### Sprint 132 — Explicit durable Project Run resume（bounded runtime slice）

交付 `forge-runtime` 的显式 `run resume RUN_ID`：从经过 `RunInspection` 校验的 durable journal 推导安全 continuation point，并恢复已持久化的 Conversation history、消息与运行计数。已提交的 `tool_started` effect 永不自动重放；`tool_finished` 后仅补写缺失的 Tool message，未开始的 tool call 才允许继续执行，已提交 assistant answer 可补写 terminal。未决外部 effect、无 durable prefix 或 Project 绑定漂移均 fail closed，resume event 序号从 journal 尾部继续，成功后复用既有 assistant writeback。仅 `RunOutcome::Completed` 的已完成终态 Run 在 Project 绑定后进入 writeback-only recovery：它在 credential/provider/tool/history setup 前幂等 reconcile 已持久化 answer，不读取 workspace 内容；`RunOutcome::Failed`、`RunOutcome::Cancelled` 与 `RunOutcome::LimitExceeded` 终态仍拒绝 resume。

恢复点保留 rejected batch 的 disposition：中断后的剩余调用继续以同一 code/message 拒绝，
不会转成工具执行；最后一个 `tool_call_limit`/`cancelled` rejection 补齐受字节上限约束的
Tool message 后直接写入对应 terminal outcome，不会错误进入新 turn。

产品边界保持明确：这是 caller-triggered bounded recovery，不是 automatic retry、whole-Graph execution、remote sync、mutating tool 或 provider usage 历史伪造，也不隐式创建分支。domain resume-point 单测、CLI 跨进程回归（不重复工具、pending effect refusal、Project binding）与 `cargo test` 聚焦套件通过。

### Sprint 133 — Bounded Project Run explain query

交付只读 `run explain RUN_ID`：从同一份经过 `RunInspection` 校验的 durable journal 生成
content-free evidence summary，包含事件支持的事实、已提交消息的 role/bytes/SHA-256、
工具 started/finished/rejected 生命周期、显式 workspace read allowlist、可继续性和
open assumptions。completed-terminal Run 的 continuation 显示为 writeback-only recovery；failed、
cancelled 与 limit-exceeded terminal Run 均显示为不可 resume；无 durable prompt 的 incomplete
prefix、pending tool effect 分别报告不可安全继续和 operator review。查询不读取 workspace、
不调用 provider/tool，并通过 existing-current immutable reader 打开 Hub，不创建、迁移、
配置或写入 SQLite；不回显 Prompt/answer/tool output；preceding Conversation
history 未被 Run v1 snapshot 绑定，Grant/Approval/PDP 仍明确是未交付的 authority boundary。
Parser、terminal/incomplete/finished-tool/pending-tool CLI 回归、`cargo clippy` 已通过。
Provider-controlled tool-call ID 不进入 explanation 明文，只保留长度/SHA-256；纯
`tool_rejected` 调用纳入生命周期摘要，已完成但尚未 commit Tool message 的 output 以指纹呈现。
人类输出显示 continuation 安全理由与去除 `assistant_delta` 分片噪声后的证据时间线。
Human 输出同时显示消息/工具输出 hash、实际 allowlist 与 terminal outcome；provider-controlled
工具名只显示受信标签或 `unrecognized` 及长度/SHA-256，配置路径均做 terminal-safe 转义。

### Sprint 134 — Prepared Project Run restart

交付 `forge-runtime --idempotency-key KEY -C PATH run restart SOURCE_RUN_ID`。应用层只接受
经过同快照完整校验的 terminal source，并将其已持久化的 Project、Conversation、user Prompt
与 exact execution configuration 物化为新的独立 Run。source 指纹与显式 key 通过域分隔
SHA-256 稳定映射到专用 Run ID namespace；换 source 复用 key 会冲突，普通 begin 不能占用
该 namespace。创建与 crash repair 共用既有 begin/event 精确重放契约，新 Run 只包含一个
`run_started` seed，随后由 caller 显式执行 `run resume NEW_RUN_ID`。晚到精确重试根据目标
journal 返回真实 `incomplete`、`pending_tool_effect` 或 `terminal` 状态，不伪报可恢复。

准备阶段只解析并校验 Project，不读取 credential/workspace 内容，不构造
provider/tool/transport，不访问 network，也不复制 source journal suffix、result 或 answer。
Project binding 在写入前校验；非终态 source、source/key 漂移与跨操作 key ownership 均 fail
closed，JSON 输出保持 content-free；新 seed 声明 `ready_to_resume`、`resume_required=true`、
`external_effects=false`。restart 保持 independent rerun preparation 语义，不创建 lineage；
root-input branch 与直接父系由 Sprint 135 的独立命令承载。

### Sprint 135 — Queryable Project Run root-input branch

交付 `forge-runtime --idempotency-key KEY -C PATH run branch PARENT_RUN_ID`：应用层只接受
经过完整同快照验证的 terminal parent，并在 SQLite v28 的单个 `BEGIN IMMEDIATE`
事务中原子创建 child Run、immutable direct-parent lineage 与恰好一个 fresh
`run_started` seed。child 复用 parent 持久化的 Project、Conversation、user Prompt 与
exact execution configuration，不复制 parent journal suffix、result、answer 或 tool events。

branch identity 使用专用 digest domain 和 `run-branch-` namespace；同 parent/key 精确重放已
提交 child/lineage/seed，换 parent 或跨 start/restart 操作复用 key 会冲突。lineage v1 固定
`root_input` 与 source event seq 1，以 domain-separated SHA-256 绑定 exact parent Run/root
event 及完整直接父系字段。读取时重验 parent terminal/root event、source digest 与 child 继承配置。

`run lineage RUN_ID` 使用 existing-current immutable reader 返回 content-free direct-parent view，
不迁移/写入 Hub，不展开 Prompt/event 正文或祖先链。branch 准备不读取 credential/
workspace 内容，不构造 provider/tool/transport，不访问 network；child 只在 caller 随后
显式执行 `run resume CHILD_RUN_ID` 时开始运行。context/workspace snapshot 均未绑定，
任意 event-prefix branching 保持为后续独立协议。

### Sprint 136 — Scheduled Graph Progress Snapshot + Core Reconcile v1（read-only runtime slice）— ADR-0096 Proposed

交付 `forge-runtime group graph run reconcile GRAPH_RUN_ID --core-bin ABSOLUTE_PATH
--core-bin-sha256 SHA256`。Infrastructure 在 exact-current SQLite v28 的一个 deferred read
transaction 内重验 Graph Run、Graph、唯一 schedule 及每个 ordinal 的 candidate、prepared
provider request、lifecycle 与 Core terminal receipt，再投影不含 Prompt/request body/result/
artifact/credential/workspace 内容的 canonical `ScheduledGraphProgressSnapshot`。它复用同一
snapshot 已加载的 source objects，并以跨表 count reconciliation 拒绝 schedule 外、orphan/
presence-chain 缺口、重复或 source-binding 漂移的已存在记录及 row-count disagreement；合法未
materialize 的 ordinal 仍投影为空 evidence 并由 Core 判断为 `ready`。

显式 digest-pinned Go Core 通过 `graph-scheduled-reconcile --protocol-version` v1 handshake
接收 exact snapshot，并在 schedule-v1 serial/one-in-flight/completed-contiguous-prefix/
exactly-one/fail-fast policy 下返回七类 source-bound disposition：`ready`、
`claimed_unknown`、`manual_recovery_required`、`failed`、`failed_uncertain`、`completed` 或
`incompatible_progress`。只有 `ready` 带 exact next ordinal/node；Rust 严格重验 decision
canonical bytes、digest、snapshot/schedule binding 与 field shape，不复制 Core 的调度选择。

Core 是 operator-trusted same-user TCB。SHA-256 pin 只证明 Rust copy/execute 的 exact bytes，
v1 handshake 只证明协议兼容；两者不认证 publisher 或 binary function。empty environment、sealed
executable bytes、bounded I/O/deadline 不是 sandbox，Runtime 没有对 Core 提供 filesystem/network/
syscall/namespace/mount/egress confinement，也没有 effect-containment attestation。

Forge Runtime 自身只观察现有 durable state：不 migration、不 logical write Hub、不读取 credential
或 workspace、不构造 provider、不联网、不 claim/release Project lane，不 materialize candidate、
prepare/send request、consume consent、adjudicate/recover/retry/resend、执行节点或授予 successor
authority。Official Core command 按 pure transform 实现并测试，但 combined no-effect claim 以 operator
信任该 exact official Core 为前提。SQLite live reader 仍可能使用 SHM read coordination；`ready`
不是 dispatch authorization。CLI JSON 以 `effect_facts_scope="forge_runtime"` 约束全部为 false 的
`runtime_effect_facts`；独立 `core_trust_boundary` 把 same-user/operator-trust、binary identity、protocol
handshake 与 empty environment 报为 true，把 filesystem/network isolation、effect containment 与
effect attestation 报为 false，Human 输出也重复 trusted same-user TCB 警示。这些字段是条件化合同与
缺口披露，不是 arbitrary pinned child 的 syscall observation 或 attestation。

当前聚焦证据包括 Go 全七 disposition、canonical/digest mutation 与 strict decode tests；Rust
domain/application strict codec、presence-chain/source binding/error mapping tests；schedule-only、
candidate+prepared-request、non-contiguous evidence、missing-schedule、out-of-schedule candidate
五组 SQLite projection tests；compiled Go Core 的 Rust→Go exact golden bridge；以及 2/2 process-level
CLI tests。后者覆盖 repository-built official Core ready、wrong-pin fail-closed、logical Hub table snapshot 不变、
workspace sentinel 不变、credential/endpoint poison 不泄露且 loopback sentinel 未收到连接。冻结
golden snapshot/ready-decision SHA-256
分别为 `a847c1b486323dc5b31922b579a5586636d7fd83eac1cca03d2722642be46d20` 和
`0c5682601d192a19abb1d23d8bb1597c0eacde8fa098a49b4db548fd5bc56af0`。

**stop_condition:** 本 sprint 到只读 reconcile 为止。Concurrent terminalization、完整 32-node 与
stored-corruption matrix 属 effectful step 前置；one-node step 必须每次 fresh exact-request
consent 且至多执行一个 Core-selected node。Durable whole-Graph controller、第二节点自动循环与
concurrent wave execution 在各自 journal/budget/re-entry/failure/recovery contract 独立验收前保持关闭。
上述定向 observation 不证明 arbitrary operator-pinned Core 的 filesystem/network effect confinement。

### Sprint 137 — ADR-0096 pre-effect storage validation closure（read-only/testing slice）

本 sprint 不新增 CLI/schema/wire/digest/disposition，也不把 `ready` 转成 authority。它只闭合
ADR-0096 自己列出的三个 effectful 前置：concurrent terminalization、32-node count/order bound 与
stored-corruption/source-binding matrix。

SQLite reader 新增仅在 unit-test build 存在的确定性交错 seam：deferred transaction 完成 Run、
Graph 与 schedule source read 并固定 snapshot 后，第二连接通过真实 store path 对合法 scheduled
lifecycle 完成 claim→terminalize。被固定的 reader 精确返回完整 `claimed` 前态；fresh reader 精确
返回完整 `terminalized/completed` 后态及 receipt，candidate/request identity 保持一致，未观察到
跨版本混合。该正向 fixture 同时暴露了既有 scheduled lifecycle claim 的 production defect：
`INSERT_SQL` 曾把 `{TABLE}` 当成字面 SQLite token。最小修复只把受控 `TABLE` 常量代入原 SQL，
不改变 schema、claim contract、lane authority 或 terminal semantics，并由真实 claim/terminalize
回归覆盖。

32-node evidence 在真实 SQLite v28 中构造完整 serial schedule，验证全部 ordinal 顺序、attempt、
content-free 空 progress、exact canonical round-trip，并确认该 representative encoding 不超过
64 KiB。Go Core 另以两个 32-node decision-boundary shape 验证 31 个 completed receipt 后 ordinal
31 为唯一 `ready`，32 个 completed receipt 返回 `completed`；两份 signed canonical snapshot
均不超过 64 KiB。它们证明 node-count boundary 与这些 exemplar 的 size check，不宣称使用最长
identifier 或为 `ready` node 填满全部可选 evidence 后得到 byte-maximal snapshot。

Stored corruption suite 使用 fresh fixture 注入 **42 个独立状态**：initial candidate 7、successor
candidate 7、provider request 9、claimed lifecycle 9、orphan/extra/count 5、terminal evidence 5。
覆盖 cross-run/schedule、ordinal/node/attempt、candidate/request body 与 digest、presence-chain、
不可投影 extra row、claim/release JSON、status/evidence shape、artifact/control/receipt canonical JSON、
stale receipt digest，以及“自身合法重签但与 terminal control/source 漂移”的 receipt；全部返回
`HubStoreError::Corrupt`，另有 claimed/terminalized 两个 honest baseline。聚焦 scheduled progress
为 **21/21 PASS**，Go test/race/vet、infrastructure strict Clippy 与 rustfmt 均通过。

审计同时确认下一步不能直接进入 effectful step：现有 Go `graphscheduledrelease` v1 的 source rebuild、
contract/provider/authorization header 都固定 initial candidate/ordinal 0，而 Rust 已能表示 successor。
因此下一独立协议切片是 zero-effect `Scheduled Ready-Node Release Authorization v2`：Core 必须重跑并
绑定 exact snapshot+reconcile decision，重建 exact initial/successor source 与直接前驱 closure，输出
max-one future release policy。它仍不是 consent、current execution authority、lane claim 或 provider
send。只有该 parity、fresh exact consent、snapshot-to-claim CAS、竞争、pid-sidecar owner、hard-crash/
uncertain-commit 与安全复审闭合后，才可实现 effectful one-node step；durable controller 继续更晚。

### Sprint 138 — Scheduled Ready-Node Release Authorization v2（zero-effect runtime slice）— ADR-0097 Proposed

本 sprint 保持既有 scheduled release v1 的 initial/ordinal-0 wire、command 与 digest 不变，新增独立
`forge graph-scheduled-ready-node-dispatch-authorize --control FILE|-` 和 exact `--protocol-version=2`，
以及 Runtime `group graph run ready-release GRAPH_RUN_ID --core-bin ABS --core-bin-sha256 SHA`。v2 control
和 authorization 分别使用 domain-separated
`forge.group-agent-scheduled-ready-node-dispatch-release-control.v2\0` 与
`forge.group-agent-scheduled-ready-node-dispatch-authorization.v2\0` identity；完整 control 上限 64 MiB，metadata-only
authorization 上限 1 MiB。

Application 先通过既有 atomic progress reader 取得 S0，释放其 transaction 后调用 pinned reconcile Core，
验证 exact source-bound decision 并要求 `ready`。Rust SQLite adapter 随后用 S0 digest 与 selected ordinal/node
在单个 deferred transaction 中原子重建 source bundle A：exact Run/Graph/schedule、同一 progress snapshot、
selected initial 或 successor candidate、prepared request/exact body、有序直接前驱 terminal receipt closure，
以及只在显式 content flag 下允许的首直接前驱 durable result artifact。A 关闭后 Application 才把 exact
reconcile decision 绑定进 control 并调用 handshake-2 authorization Core；Core 严格解码 control、重跑
ADR-0096 reconcile，并只为同一 `ready` ordinal/node 重建 source。authorization Core 返回后 Runtime 通过同一路径
读取第二个 atomic bundle B，要求 A/B source 与用同一 decision 构造的 control bytes exact 相等，再验证只含 metadata 的
`maximum_future_node_releases=1` authorization。真实 Application service + SQLite barrier 在 A 后暂停
authorization Core port，合法 claim 提交后再恢复 B，精确返回 `SourceChanged`；较低层 snapshot interleaving 另覆盖
terminalization 后旧 snapshot 拒绝。没有 transaction 跨越 child process。

initial 必须是空 receipt closure/content-free；successor 精确使用 schedule canonical direct-predecessor
order，允许空 direct set，receipt-bearing 路径分别覆盖 content-free 与绑定首 receipt 的可选 result
artifact。content presence 不推导 disclosure consent，`ready`、receipt 或 authorization 也不推导
off-machine consent。compiled Go/Rust 边界覆盖 initial、empty-direct successor、receipt-bearing content-free、
content-bearing successor，以及完整 32-node、31-receipt、ordinal-31 五种 shape；mutation matrix 覆盖
non-ready、stale/decision/source drift、closure/order/content/digest、
strict canonical framing、wrong pin/handshake 与 process I/O bounds。
Accepted ADR-0033 的 predecessor output 保持 exact nonempty valid UTF-8，而 task/acceptance 仍用既有 prose
grammar；双语言 user-Prompt ceiling 扩为精确 6,553,926 bytes，并用 1 MiB NUL 验证最坏 6x inner JSON
escaping 仍落在 8 MiB candidate 内。Go canonical codec 将 encoding/json 的 U+2028/U+2029 JSONP escape
归一为既有 Rust raw-scalar wire；包含这两个 scalar 的 candidate + durable artifact 已通过整份 Rust control
→真实 Go Core→Rust authorization compiled 往返，literal `\\u2028` 文本保持不变。

CLI 在 Hub/private input 前完成双 handshake，公共 JSON/Human 只输出 authorization metadata、Runtime-only
effect facts 与同用户 trusted-Core 警示。official tested path 不做 migration 或 logical Hub write，不读
credential/workspace、不构造 provider/transport、不联网、不收集/消费 consent、不 admit/release lifecycle、
不 claim/release lane、不 send/terminalize/persist receipt/retry/resend/recover/advance。pin 只证明 binary bytes，
handshake 只证明 wire compatibility；empty environment、sealed executable 与 bounded I/O 都不是 filesystem/
network/syscall confinement、publisher authentication 或 function/effect attestation。

聚焦验证为 Application unit **7/7**、Application + real SQLite A/B race **1/1**、ready-release Domain **6/6**、
SQLite store **3/3**、Core bridge **2/2**、CLI compiled cross-language **6/6**；Go full/race/vet/build
与 Rust workspace full test、fmt、strict Clippy、check/build
均通过。v2 artifact 不持久化 consumption，也不消除 B 后 staleness；两个不变状态的调用可以得到相同
deterministic policy，因为本切片没有 effect。

**stop_condition:** 本 sprint 到 zero-effect max-one future-release policy 为止。后续 effectful one-node
step 必须在 `BEGIN IMMEDIATE` 中把 fresh progress/decision/source 与 claim 做 CAS，独立取得 exact-request
off-machine consent，并在 predecessor content 存在时另取 content consent；还须闭合单赢家竞争、lane/
pid-sidecar ownership、one-shot send、post-claim no-resend、hard-crash 与 uncertain-commit。Durable controller、
自动第二节点与 concurrent schedule-v2 wave 继续保持关闭。

### Sprint 139 — Effectful Scheduled One-Node Step（runtime slice）— ADR-0098 Proposed

新增公开 `group graph run step GRAPH_RUN_ID`，要求 caller 同时锚定
`--expected-provider-request-id`、`--expected-ready-authorization-sha256`、exact `--pricing`、
operator-pinned `--core-bin/--core-bin-sha256` 与 fresh `--confirm-off-machine`；selected candidate
包含 predecessor output 时另要求 fresh `--confirm-predecessor-content`。Application 每次 prospective
effect 都重新运行 ADR-0097 A/Core/B 并比较两个 expected identity；缺 consent 或 identity/source 漂移
在 credential、owner、provider 与 send 前失败。CLI 默认 metadata-only，只有 `--include-result` 显式披露
结果正文；SIGINT/SIGTERM 映射为 bounded cancellation/uncertainty，命令不执行第二个 node。

ready claim 使用 lifecycle v2 并保持既有物理 SQLite 表与 schema 不变。reader 只接受 stored
release/authorization `(1,1)=legacy` 或 `(2,2)=ready`，mixed/unknown pair 一律 corruption。provider 可在
claim 前构造但保持 unopened；exact owner durable 后，`BEGIN IMMEDIATE` 同时重建 current ready
progress/selected initial-or-successor source，CAS exact release/authorization/pricing/request/body，并检查
legacy 与 ready 两个 lifecycle family 的 Hub-global Project lane。只有 commit winner 获得 non-`Clone`
authority 并至多 poll 一次 bounded provider stream；这个本地观测不证明远端已观测 request。exact replay 忽略 contender 新生成的 owner/time，但仍要求全部 immutable
source evidence 相等。terminal receipt 或 artifact-only quarantine 才释放 lane，所有 claim 后路径和
claimed/terminalized/quarantined/adjudicated re-entry 均禁止 automatic retry/resend。

旧 `<request>.pid` overwrite 方案已被统一 Linux exact-owner sidecar 取代。owner 路径绑定 request + random
lane ownership ID，directory/file 分别要求 `0700`/`0600`、non-symlink、create-new、single-link 与 ≤4 KiB
canonical JSON；document 绑定 machine/boot/PID namespace/time namespace/PID/process-start，创建与同 boot 裁决
均验证 `/proc/self` numeric target 精确等于 `getpid()` 后才读取 `/proc/<pid>/stat`；cleanup 还校验
device/inode，replacement 保留。
directory advisory lock 串行化容量检查与 durable create，任意条目总数达到 1024 即失败关闭；unknown entry
同样占容量且不自动 scavenging，故 crash orphan 有硬上限但不会被误判为可安全释放的 owner。
sidecar 在 claim commit-uncertain 前切为 preserve-on-drop；hard crash、claim commit uncertainty 或 terminal
commit uncertainty 无法证明 lane 已安全释放时保留证据。公开
`group graph run scheduled-contract provider-request dispatch adjudicate PROVIDER_REQUEST_ID` 先读 durable
any-family exact owner；machine 不同失败关闭，旧 boot 足以证明 executor 已死，同 boot 只接受 exact PID/time
namespace 与 verified procfs view 的 `dead|pid_reused`，
再以 guarded immediate transaction 重验 claimed + lane-active + no-terminal + exact owner 并要求恰好更新一行，
commit 后才 cleanup。该机制不构成分布式 executor identity、
fencing token 或 remote exactly-once；同 UID hostile replacement 的窄 final-check→unlink race 保持披露。

fresh review 揭示的 honesty/portability gap 已修：应用结果携带独立 invocation effect receipt，入口 replay 与
已执行 Core/credential/provider/owner preclaim 的 CAS loser 不再合并；durable Core-failure quarantine 返回结构化
metadata，terminal commit-after-success 会 fresh inspect 后恢复确切 terminal 结果，其余 claim/post-claim uncertainty
错误固定披露 poll/remote-attestation/no-resend。CLI 分开报告 terminal protocol handshake 与 stored receipt。
scheduled adjudication 输出也分开 `dispatch_performed=false` 与 `database_written=true`。v1/v2 reader 严格绑定
created=claim release、terminalized=artifact created、status↔adjudication timestamp，并把负值/NULL/drift 返回
`Corrupt` 而非 panic。非 Linux 编译使用 API-compatible unsupported sidecar 与 cfg signal watcher，effect/adjudication
在 private/Core/provider 前明确拒绝，不再拖垮其余 CLI。
最终安全复审又定位到旧 scheduled `Execute` 路由仍在 platform guard 前读取 authorization/pricing；guard 已提升到
公开命令路由的 inspect/input-read 之前，并保留执行/裁决内部的纵深检查。新增 non-Linux 公开进程回归以不存在的
private source/Core 路径证明先返回 Linux-only 且不创建 Hub。
后续终审又关闭三类事实缺口：Go scheduled terminal digest 改为与 Rust 一致的递归 object-key canonicalization，
同时以 `UseNumber` 保持 exact `u64`，真实 pinned Core 现接受 control 并持久化 receipt；CAS loser、durable
terminal/quarantine 与 adjudication 的 commit 后 cleanup failure 不再覆盖已知 effect receipt，而独立报告
cleanup failed/presence unknown；terminal 写入前失败则保留 owner 并返回固定 no-resend uncertainty。
legacy scheduled `Execute` 也把 claim commit uncertainty 固定为 poll=false、其余 post-claim uncertainty 固定为
poll=true；terminal/quarantine commit-response-loss 仅在 exact claim、lane released、expected status 重读成功后
恢复结构化结果，Core refusal 的 durable quarantine 与 cleanup failure 不再退化为 generic error。其 JSON 固定
`remote_provider_request_observation=not_attested`，不把本地 poll/dispatch 冒充远端已观察 request。
安全终审实证了 time namespace 对 `/proc/<pid>/stat` starttime 的 boottime-offset 重写可把 live owner 误判为
PID reuse；sidecar 现额外绑定 canonical `/proc/self/ns/time` identity，同 boot namespace mismatch 在读取目标
PID stat 前失败关闭，并保留跨 boot 可证明旧 executor 已死的恢复语义。

截至本节更新，定向证据包括 Application + real SQLite ready-step **9/9**、ready cleanup/uncertain failure **4/4**
与 legacy claim/terminal/quarantine response-loss **3/3**，
ready CAS/replay/cross-family 竞争 **7/7**，sidecar permission/create-new/symlink/hardlink/replacement/
PID-namespace/time-namespace/procfs-view/liveness/preserve/capacity **21/21**，
lifecycle/adjudication/version/timestamp corruption **22/22**，公开 ready-step process **5/5**、legacy Execute
quarantine/cleanup/re-entry process **1/1** 与公开 adjudication process **1/1**。两个 effect process 都用 production
binary/registered adapter 和测试进程内 `getaddrinfo`/`connect` 拒绝垫片。ready process 真实完成 claim→一次本地
provider poll→pinned Core receipt→terminalized→`--include-result` re-entry；legacy process 则完成一次本地 poll 后由
pinned Core 拒绝并 durable quarantine，在故意制造 cleanup failure 时保留结构化事实，随后 re-entry 零重发。各自
marker 证明首调网络路径被本地拦截且二次调用没有再次触发；ready 三节点 plan 仍只有一个 lifecycle。所有测试只用 deterministic provider、
本地 pinned Core 或本地拒绝网络，没有 live provider/付费模型请求。终审修复后的 Rustfmt、strict workspace
Clippy、arch **8/8**、gate、governance 与 `cargo test --workspace --all-targets` 均通过，最后一项 exit 0、耗时
350.16 秒；Go `test ./...` 与 `vet ./...` 也分别 exit 0。初轮及终轮 fresh security/final review 曾准确给出
NOT APPROVED 并推动上述修复；最终 security 与 contract review 均已 **APPROVE/CLEAN**。实现提交
`713b4b3` 与 inert credential-fixture 扫描标注修复 `3b3e64a` 落盘后，正式候选以 `umask 0022` 从
`3b3e64a` 创建 clean clone，并在 journal 前预建精确、已排除的 `forge-runtime/target/`，避免 Cargo 1.93
冷初始化的原子 `targetXXXXXX` staging 被误记为源码漂移；候选 Git 状态及 staged/unstaged diff 均为空，冻结文件为
`0644`/single-link。`node harness/acceptance.mjs` 最终 **ACCEPTED（9 pass / 0 fail / 2 honest N/A）**：Python
97 files / 1397 tests、Node 41 files / 609 tests / 0 skipped、forge-core Go 2620 tests、Rust 五组 observed tests
389 / 66 / 389 / 303 / 211，examples 22 / 47；complexity、governance、architecture、secret scan、SCA、
typecheck 与 build 全部 PASS，缺失/未配置的 lint/coverage 工具保持诚实 N/A。ROADMAP 实现项据此勾选；
ADR-0098 仍为 Proposed，未发生 lifecycle 晋级。本切片始终不包含 durable whole-Graph controller、automatic second-node、
schedule-v2 concurrent wave、lease expiry、provider-side idempotency 或自动 crash recovery。
