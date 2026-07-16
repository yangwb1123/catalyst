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

## 下一前沿(需外部资源 / 投机增强 / 架构外,非本环境可完整验证)
- **真点火** `--agent-cmd=claude`:**multi-agent running to completion 已坐实**(Sprint 25:真 claude 多-agent 跑到 converge MET,增量级 + 版本级)。完整旋钮:四维资源护栏 + 成本三维(phase/时间/美元)+ 任务注入 + 写权限 + 模型路由 + 工作目录 + retry + loop-back;诚实分工:agent 自治增量绿、人确认版本竣工。docs/ignition.md 有完整配方 + 实测
- **需外部资源(框架已就绪)**:SCA/CVE 漏洞库 OSV/NVD(差 DB)· 跨厂商池 LiteLLM(差多厂商 keys)· Firecracker 沙箱(差 KVM/特权)。〔真 cost/latency telemetry **已达成**——S26 真 claude 补齐真 token/cost/latency 数据,scorecard 三维真值落盘〕
- **投机增强(做即违反反 gold-plating 纪律)**:embedding 语义检索(TF-IDF 已工作,增量仅真点火时体现)
- **架构外**:Web UI(偏离 CLI/声明式核心)
- **独立大特性,非接线小修(Sprint 30 复核后从 GAP 改判)**:`internal/routing` 的完整多维评分器(complexity/dependency/context/business-impact)接入真实执行路径(目前只喂手动 `forge route` CLI)——包自身文档已自我标注为「v2+ Router service」。
- **`readonly`/`on_rejected` 的真 claude 进程验证——用户已明确决策终止于此(2026-07-03)**:两机制均已真实实现(非声明未接线);readonly 路径限定按官方文档契约构造 + 单测坐实 argv、on_rejected 用 fake-agent 脚本端到端坐实一次性触发语义,但都未过真实付费 `claude` 进程验证运行时行为。征询用户是否授权花真实 API 预算推进最后一道经验验证,用户选择「单测已足够,就此打住」——非遗留缺口,是知情决策后的终态;若未来有人想补这道验证,预算授权需重新征询。
- **需求清单本身**:`docs/FUNCTIONAL_REQUIREMENTS_AUDIT.md`(Sprint 30 起 + Sprint 31 修订)是本仓当前唯一的显式功能需求清单,derived from 项目自己的声明源头;后续 sprint 如声明新机制,应同步补一行,不要让清单本身漂移回「不存在」。

**stop_condition:** roadmap 完成度 / 闸门全绿(非「继续 N 轮」)。
