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

验收递归发现 Node/Python harness，并对 Go/Node/Python/Rust/Java 项目要求可观察的正测试数；manifestless、零测试、不可读子树和未配置目标不再空过。项目结果类别是结构字段，路径/输出文字不能伪造 `inapplicable`。init 生成真实 starter manifest/test；upgrade 的 source/target/state/backup/prune 路径对 symlink、special file、portable case/Windows 别名和 inode alias 失败关闭，且 init/upgrade 在首写前预检全部计划叶，晚发现坏目标不再留下部分更新。Rust SQLite 对整个 open→PRAGMA/WAL→schema 序列在 BUSY/LOCKED 下以 15 秒统一期限和有界退避重试，8×16 并发首次打开、2.3 秒独占锁、超时失败关闭、DB/WAL/SHM `0600` 与 workspace `workspace_unavailable` 单终态均有回归；并发 opener 的旧权限 metadata 会按当前目录 inode/mode 复核，权限收紧通过目录 FD 完成，路径被替换为 symlink 时失败关闭且不修改链接目标。

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

产品边界保持明确：这是 caller-triggered bounded recovery，不是 automatic retry、whole-Graph execution、remote sync 或 provider usage 历史伪造，也不隐式创建分支。该 recovery slice 不会自行扩大某个 Run 已持久化的工具面；ADR-0108 的第一方 Agent candidate 可按 persisted mode/toolset 恢复其只读或 `--dev` edit/process 工具。domain resume-point 单测、CLI 跨进程回归（不重复工具、pending effect refusal、Project binding）与 `cargo test` 聚焦套件通过。

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

### Sprint 140 — Durable Scheduled Whole-Graph Controller v1（runtime slice）— ADR-0099 Proposed

在已独立验收的一次一节点 step 之上，新增公开
`group graph run controller start/show/advance/step`。SQLite v29 为每个 Graph Run 保存一个 immutable
controller header 与 bounded append-only event journal；header 绑定 exact schedule/Core/profile/预算，event
链以 digest CAS 记录 materialize、prepare、fresh-consent、dispatch reservation、completion 与 terminal stop。
公开 `show` 只读重建 journal；`start/advance` 使用无 executor 的 passive service；`step` 复用既有
snapshot-to-lifecycle claim 与跨 family Project lane 作为唯一 send fence，并且一个调用至多执行一次
`executor.execute`/provider poll。

controller 只接受 schedule-v1 serial、contiguous prefix、exactly-one-attempt、fail-fast policy。Core 的
reconcile、ready-release、materialization 与 terminal 四个 protocol handshake 均在任何 Hub mutation 前完成；
Rust 又重验每个 source-bound decision。completed node 可在同一显式调用内被动 materialize/prepare 一个
content-free successor，但必停在新的 `AwaitingFreshConsent`；自动路径不传播 predecessor result 正文，也不
递归执行第二个 node。每个 effectful step 在可能的外部 effect 前持久化 `DispatchPlanned`，不可退款地预留
一个 step 与完整 per-node maximum cost，并要求 caller 锚定 exact awaiting event/request/authorization/
snapshot/decision 与 fresh off-machine consent；存在 predecessor content 时仍另需独立 content consent。

crash/re-entry 先检查 any-family lifecycle，再触碰 pricing、credential、provider 或 schedule drive。completed
receipt 可精确修复为 `NodeCompleted`；claimed/quarantined/adjudicated/failed/failed-uncertain 进入对应 terminal
stop。裸 `DispatchPlanned` 的 lifecycle `NotFound` 或 `Unavailable` 都不能证明旧进程无害，只要 controller
journal 可写便持久化 `Stopped(claimed_unknown)`；只有 credential、provider construction 或 owner evidence
这三类明确发生在 claim/poll 前的失败先写入 durable `RetryablePreclaimFailure`，才允许后续重新计算 current
authorization、产生新 awaiting digest 并重新取得 consent。SIGINT 卡在 pricing read 的进程测试也验证裸
dispatch 永久 stop，而不是把 cancellation 冒充 safe release。

controller CAS 仅排序 journal，不授予 provider authority。并发 loser 必须 reload 同时严格后继 original 与
previous journal 的完整有效链，重新检查 exact completion、允许的 uncertainty terminal 与 lifecycle；无关预算/
兼容性/reauthorization terminal 不能清除 unresolved effect uncertainty。rebase 受 512-event protocol cap 约束，
每次 retry 都要求严格 chain growth。SQLite v29 的 `BEGIN IMMEDIATE` start/append、exact replay、head CAS、
canonical blob/column cross-validation、v28→v29 migration/final rollback、两连接竞争与 persisted header/event
corruption matrix 均通过。

所有 public controller identifier/digest/profile preflight 都在 Hub path/Core 构造之前执行，并拒绝 control 与
bidi format characters；compiled CLI 以不存在的 Hub/Core/pricing 证明 malformed `show/start/advance/step`
不会访问它们。默认输出只有 bounded metadata，journal 不保存 prompt/request body/predecessor output/model
result；Core 明确标为 operator-trusted same-user code，pin/handshake/empty environment 不冒充 publisher/
function attestation 或 filesystem/network/syscall/effect containment。Linux-only Core/effect path 在公开 usage 中
披露；测试全部使用 deterministic provider、本地 pinned Core 或本地拒绝网络垫片，没有 live provider 或
付费模型请求。

fresh-context governance 与 security 最终复核均为 **Critical 0 / Major 0 / Minor 0**。聚焦证据包括 Domain
**13/13**、Application controller **42/42**、SQLite controller **10/10**、CLI unit **13/13**，compiled
handshake/re-entry/process suites 与 Go focused 全绿；prepare 五个 crash cuts、legacy lifecycle、late completion、
budget/clock rollback、StoreUnavailable-but-controller-writable stop、uncertainty/passive-writer race、bidi
preflight、wrong pin/四 handshake、SIGINT/FIFO 与 provider-count/privacy sentinel 均有回归。最终未提交快照的
Rust workspace all-targets、fmt、strict Clippy、check/build，Go full/race/vet/build，architecture **8/8**、
gate、governance、ADR v2 validator 与 diff-check 全部通过。

实现提交 `dd89f26` 与 ADR/design 提交 `0ca5068` 落盘后，以 `umask 0022` 从 exact `0ca5068` 创建 clean
clone `/tmp/tmp.J1j97bTIqf/catalyst`，并预建已忽略的 `forge-runtime/target/`。候选 Git status、staged 与
unstaged diff 均为空；从 `b34248f..0ca5068` 的 **112** 个 changed files 全为 `0644`/single-link。
只把确认 cwd 位于该 clone 的运行计作正式证据：`node harness/acceptance.mjs` 最终
**ACCEPTED（9 PASS / 0 FAIL / 2 honest N/A）**。Python 为 **97 files / 1397 tests**，Node 为
**41 files / 609 tests / 0 skipped**，forge-core Go 为 **2622 observed tests**，Rust 五组为
**402 / 109 / 402 / 316 / 224 observed tests**，examples 为 **22 / 47**；complexity、governance、
architecture、secret scan、SCA、typecheck 与 build 全部 PASS。缺失/未配置的 ruff/golangci-lint、ESLint
project config 与 coverage 工具保持诚实 N/A；Rust Clippy 五组实际 PASS。

**stop_condition:** durable loop budget、terminal lifecycle dispositions、per-request consent、CAS/replay、
schedule-version compatibility、privacy、fresh review 与正式 acceptance 已闭合，ROADMAP 实现项据此勾选。
ADR-0099 仍为 Proposed/null，未发生 lifecycle promotion。v1 是 caller-driven explicit re-entry，不是 daemon；
不提供 schedule-v2 concurrent wave、parallel/in-flight multi-node、multiple attempts、automatic retry/resend、
budget refund、stopped-state recovery、lease expiry、quarantine repair、claim adjudication、predecessor-content
auto propagation、provider-side idempotency、remote exactly-once 或 Core/SQLite same-user tamper resistance。

### Sprint 141 — Local App Server Bootstrap v1（R0 product slice）— ADR-0100 Proposed（✅ DONE）

把 v3 Web UI 目标收窄为第一个可独立验收的产品进程边界：新增 Go `forge-server` 与
`internal/appserver`，要求 caller 显式提供 absolute non-root dedicated state directory，只监听 literal loopback IP。
首次启动只创建 missing leaf；后续必须验证 exact `0700`、effective-user ownership、v1 identity、closed layout 与
single-link `0600` lock identity。state path 必须 canonical；existing direct parent 是 euid-owned、non-group/world-
writable trust anchor，完整 parent path 在绑定该 directory identity 前后均拒绝 symlink component。实现拒绝 replaceable
state parent、特殊权限位、hard link、未知 entry 与 arbitrary private directory，且不 chmod/truncate 既有对象。
`GOOS=linux`（排除 Android）使用 nonblocking advisory lock；Android、AIX、Darwin、Illumos、Solaris、Windows 与其他目标可编译，
但因 v1 没有 descriptor-bound 独立 ACL 模型校验而在任何 state access 前失败关闭。
direct parent 的 pre/open/post identity 均重验 owner/mode 并覆盖 same-inode ABA；更高 namespace component 只做
pre/post symlink observation，并拒绝 mapped-untrusted owner 与 non-sticky group/world-writable ancestor；Linux non-initial
user namespace 只接受 configured overflow UID 且必须不落入任一 inside map；所有折叠到该 UID 的 unmapped host
principal 均不可区分并归入 supervisor TCB，不能冒充已认证 host owner，也不声称抵抗能在不同 startup 间重映射
accepted euid-owned parent entry 的 supervisor/OS/root authority。

公开 HTTP surface 固定为 `forgeos.app-server/v1` 的 metadata-only startup receipt 与
`GET|HEAD /api/v1/health`；unknown route 和 mutation method 返回稳定 JSON error，响应固定 no-store、
no-sniff、same-origin resource policy、no-referrer 与 deny-all CSP，且不发 CORS grant。startup receipt 采用 bounded
可取消 publication，成功后才开始 HTTP Accept；request Host 必须等于 exact listener authority，general `OPTIONS *`
关闭，live connection 与 in-flight request 有固定上限。health 只含 service、
API version、bounded build version/commit 与 process `ok`，不含 absolute path、host、Prompt、Project、
credential、Runtime、Harness 或 product state，也不声称 dependency readiness。listener 采用 bounded HTTP
header/read/write/idle timeout；caller cancellation 在正常 serving 或 blocked startup output 阶段均 bounded 退出。

本 slice 不创建 `control.db`，不读取 repository、Go checkpoint、Rust Hub 或 Harness state，不启动 Agent/
provider、不发现或读取 ambient workspace（只检查 caller-supplied state path 及祖先安全）、不发出站网络、
不推进 Objective/Change/WorkItem/Attempt/Approval/Outcome，
也不包含 Web/TUI asset。后续 Command API、Query projection、Runtime event inbox、browser origin/authentication
与 UI 必须另立版本化 sprint/ADR。

验证要求覆盖 config preflight、exact HTTP bytes/headers/Host/raw OPTIONS、connection/request saturation、private
filesystem identity/alias/special-mode/hardlink、same-state real-process contention/reuse、真实 ephemeral loopback、
pre-cancel/announce-error/blocked-output cancellation，以及 built command process 的 startup→health→SIGINT→zero-exit
和 Linux/Android/AIX/Darwin/Illumos/Solaris/Windows build/lock-selection。所有测试仅访问本机 loopback，不使用 live provider、模型、凭证
或外部网络。

实现证据已覆盖 focused normal/race、最终全仓 non-cached race、full vet/build、7-target build/lock-selection、
真实 built-command process、ADR v2 双 validator、architecture 8/8、gate/governance 与 fresh-context architecture/
security 双 CLEAN。正式 repository acceptance 是最后的 completion gate；任何失败均要求撤回本 DONE/ROADMAP 标记。

**stop_condition:** Proposed ADR v2 exact validator、focused/full Go tests、gofmt、architecture/gate/governance、
fresh-context architecture/security review 与正式 `node harness/acceptance.mjs` 全部通过后，才可勾选 R0-A。
通过只关闭 process/bootstrap/health 边界，不代表 F1–F4、R0 Developer Preview 或完整 App 已交付。

### Sprint 142 — Platform Core Identity + Envelope v1（R0-B1）— ADR-0101 Proposed（✅ DONE）

本切片只交付 Platform Core 的第一段纯合同：typed opaque ID、contiguous `ScopeRef`、`ActorRef`、
`RecordRef`、`ArtifactRef`、`CommandEnvelope` 与 `EventEnvelope`，并在 Go control、Rust domain 和独立
Python Harness 中实现 exact canonical JSON、domain-separated conformance digest、共同 golden 及
malformed/boundary/adversarial tests。`envelope_version` 只版本化信封，正 signed-int64
`(schema_name, schema_version)` 独立标识 payload schema。

共同 golden 仅示意 supplied WorkItem target 与 Attempt-scoped runtime event；本切片不解释
`StartAttempt` payload，也不保证 Attempt 创建或 first-event 语义，这些属于后续 payload/state contract。
非空 causation 不得自指。Artifact-backed envelope 必须把 Artifact 的 ProjectSnapshot 与 producing Attempt
绑定到同一 scope，且 Artifact 不得晚于 command issue/event occurrence。结构通过不认证 actor/record，
不授权、reserve idempotency、append event、证明 content、持久化、迁移状态、验证或完成。

多轮 fresh-context architecture/security review 推动修复 Python bool/int 与 signed-i64 漂移、descriptor-relative
稳定读取及 special-file open、Attempt/Artifact owner 冲突、envelope/payload version 混淆、自因果、
cross-scope Artifact substitution、Schema grammar/type/true-end 漂移、三语言 whole-envelope depth 计数、
Go typed-nil writer 和 Python high-fanout 输出放大。最新 completion-tree review 又发现缺失 App Server startup
announcer 可绕过回执门槛、请求饱和先于 exact route preflight、Python mutable writer 的 validate/serialize
alias race、显式深路径 FD fan-out、metadata failure FD leak，以及 exported route constructor 可绕过完整
Run lifecycle；后续复审又补出 Go state-dir 无前置 byte/component budget 及 Root/File `Stat` failure 延迟关闭，
以及 JSON Schema 未表达 payload/ArtifactRef exact XOR，均已修复并有专项回归，新的 architecture/security
双 CLEAN 终审均已通过。

验收证据覆盖 Go focused normal/race、full non-cached race 与 vet，Rust workspace all-targets test、fmt 与
strict clippy，Python 23 项 golden/malformed/boundary/file-race/alias tests、独立 checker，ADR-0101 v2 exact digest，
architecture 8/8、gate 3733 files、governance 13 checks、`git diff --check`，以及 exact recursive Node
41 files / 609 tests。正式 acceptance 首轮仅因新增 repository-only Platform Core Python harness 未进入
scaffold copy/whitelist ownership 而拒绝 `test_pass_node`；现已显式列入 `HARNESS_NOT_COPIED`，并以 disjoint/
existence guard 防止复制或陈旧清单漂移。

**completion_boundary:** 本 DONE/ROADMAP 标记仅在正式 acceptance 对这棵精确 completion tree 通过时保留，
任何失败均立即撤回。完成也只关闭
PC-01、PC-02、PC-03 的 `ArtifactRef` 部分及 PC-06–08 对三类 wire 的首段 strict conformance；不代表
完整 Platform Core v1、F1、R0 Developer Preview 或完整 App。

### Sprint 143 — Platform Core Receipt + State v1（R0-B2）— ADR-0102 Proposed（✅ DONE）

本切片在 R0-B1 的 identity/envelope 之上冻结三个新增纯 wire：`ExecutionReceipt` 绑定 session-level
Scope、Attempt/Session/ProjectSnapshot、executor adapter、nullable unresolved Grant/Approval refs、observed usage、
terminal Attempt state、Artifact 输入/输出、可选 event range、时间与原因；`VerificationRequest/Receipt` 绑定一个
immutable output Artifact、排序检查集、exact request digest、Harness producer、applicability、reason/evidence refs 与
严格派生的 overall status。`declared_required` 暂不参与完成策略，all-N/A 固定派生 `not_executed`。

Go control、Rust domain 与独立 Python Harness 已实现 exact canonical JSON、三份 domain-separated digest、
WorkItem/Attempt/Action 纯状态边、七类 broad rejection code、共同 golden、共享 JSON-pointer mutation corpus 及
boundary/semantic/adversarial tests。Schema 只作 non-load-bearing structural shadow；既有 Envelope v1 bytes 和 digest
不变，diagnostic 文本仍不稳定。

本切片不生成、认证或持久化任何 Receipt，不执行 Harness check，不读取 Artifact bytes，不解析或认证
Grant/Approval/Evidence 等被引用记录，不做 reference resolution，不读取 current state 或应用 edge，不 append journal，
不做 replay/idempotency/current-version/compatibility/legacy mapping，也不
把 completed/pass 冒充 WorkItem/Change 完成。

验收前证据覆盖 Go focused/full normal、race 与 vet，Rust workspace all-targets、strict Clippy 与 fmt，Python
49 项 exact golden/malformed/boundary/subclass-hook tests、双 independent checker，三份 Receipt digest，29 个状态、
61 条合法边及所有其余 pair 拒绝、62 个 wire cases、9 个 targeted transition cases、4 个 evidence ordering cases、
3 个 executor role cases与七类 rejection code。ADR-0102 仍为 Proposed，body/self digest 分别为
`5d97907c849ed28d783728e34ed05cfbe1fb830b3e5aa9299cbaa79d67d45aba` 与
`8e9b0802afba76a4536092cb6c826e83bf017d8d0293642f0a4edb7be26e15ac`。

Repository 证据包括 architecture 8/8、gate 3766 files、governance 13 checks、scaffold manifest drift 1/1、
完整 forge-init 8/8 且临时生成项目 `ACCEPTED`，以及最终 fresh-context architecture/robustness 双
`APPROVE/CLEAN`。正式 repository acceptance 是最后 completion gate；任何失败均要求立即撤回本
DONE/ROADMAP 标记。

**completion_boundary:** 本 DONE/ROADMAP 标记仅在正式 acceptance 对这棵精确 completion tree 通过时保留。
完成只关闭纯 supplied-bytes Receipt/Verification/state-vocabulary conformance，不代表 journal/current-version authority、
Runtime/Harness transport、真实 Receipt producer/replay、Objective→Outcome consumer、完整 Platform Core v1、F1、
R0 Developer Preview 或完整 App 已交付。

### Sprint 144 — Control Store Journal Foundation v1（R0-C1）— ADR-0103 Proposed（✅ DONE）

本切片只交付 Go Product Control Plane 的私有持久化原语。`internal/controlstore` 固定
`modernc.org/sqlite v1.57.0`，在既有 App Server descriptor-bound instance lock 后持有同一 state root 的
directory FD，并只通过 `/proc/self/fd/<fd>/control.db` 打开 SQLite，避免 configured pathname rename/
replacement 把 lock 与数据库拆成两个物理 namespace。初始 database 以 exact `0600`、euid-owned、single-link
regular file 创建；state closed layout 只扩展 `control.db` 与可选 `-journal|-wal|-shm`，已有文件均在 readiness
前重验。Android/non-Linux 继续在 state access 前失败关闭。

历史 ADR-0001 已 Superseded 且只决定启动时序，ADR-0002 的 Go-core polyglot 决策也未永久禁止外部 module。
原 zero-require 测试已收敛为更窄的机器策略：exact `go.mod`、完整 `go.sum` digest、唯一
`internal/controlstore/open_linux.go` blank driver import，以及 `CGO_ENABLED=0` forge CLI build；任何额外 module、
checksum 或 source import 漂移都会失败。

Schema v1 使用 exact application/user/schema identity、STRICT tables、explicit indexes、append-only trigger、
WAL/FULL/foreign-key/defensive/trusted-schema-off profile、5 秒 busy bound、quick/foreign-key/catalog/relational
validation。非空库先经 SQLite header 和 read-only main+WAL 预检，通过后才打开可改变
journal mode 的读写连接。首次启动在 WAL profile 后、schema commit 前中断所留的 exact
zero-identity/empty-catalog/single-page/zero-freelist SQLite shell 会在下次启动安全重试。空 R0-A state
transactionally 初始化；foreign、
partial、future、downgrade 或 catalog drift 不修复，
在 listener bind/announcement 前拒绝。

`Commit` 只接受 exact canonical Platform Core Command/Event：command 必须有 expected-version；1–32 个 Go-Control
event 必须 target 一致、只更新 Go-owned aggregate、aggregate version 与 component sequence 连续，global
replay sequence 由 store 独立分配。idempotency lookup、message/causation/correlation 解析、aggregate head compare、
event append、head advance、0–32 outbox append 与原始 result
receipt 在一个 immediate transaction 中完成；same key+same canonical command 返回第一次 bytes，same key+different
command 拒绝；receipt 同时保留 exact command bytes/digest，replay 会重验 canonical metadata。Inbox 只接受
non-Control canonical Event（含拒绝 `legacy_importer`），首 event 把 `source_component` 固定到 explicit source
stream，并区分 exact replay、mixed-source conflict、payload conflict、gap 与 reorder；不同 stream 独立，outbox ack 另存 immutable row。所有 page 1–100，读取重算 digest 并重验
canonical metadata；`ControlSourceHead`/`InboxCursor` 提供下一层构造 event 与恢复 transport 所需的只读 durable cursor，
事务仍会重新比较，预读值不是锁或 authority。

当前 focused evidence 已覆盖 exact private schema/reopen、非空不兼容库零 journal-mode 变更、descriptor
rename/replacement、symlink/hardlink、foreign/version/catalog drift、exact pragma/defensive/immediate 语义、
successful commit/replay（含 non-null empty result）、same-key payload conflict、expected-version 双并发单赢家、
component/global sequence 分离、causation/correlation、Rust-owned aggregate 拒绝、injected outbox/inbox insertion
failure 全事务回滚、outbox ack、inbox independent stream/cursor reopen、stored request drift rejection、
grouped non-correlated startup plan、concurrent Close 与 abrupt-exit WAL commit/rollback 恢复，以及 App Server
store-before-announcement/schema-before-readiness。独立存储复审追加发现的 interrupted-first-init、
interleaved command range 和 reopen causal-validation 三个 Major 已以跨进程恢复、window/range invariant、
startup causal join 及 exact corruption/reopen fixtures 修复；App Server 也不再丢弃 Store.Close 错误。

最终代码树已通过 focused/full Go normal+race+vet、CGO-disabled build、Darwin/Windows fail-closed
cross-build、gofmt、module verify、architecture **8/8**、gate **3791 files** 与 governance **13 checks**。
fresh-context architecture/protocol 与 storage/security 复审均为 `APPROVE/CLEAN`，Blocker/Major/Minor 均为 0。
ADR-0103 保持 Proposed，body/self SHA-256 分别为
`e2da59ac298aef16e2d1fbcd8b5dd033628c6aa5beb37424a2b623451f9a55da` 与
`3cf27714d0fa6cfed781ef2ff4fd02318a413e49c38b273dc06fe1cd1d84fb99`，strict Proposed-v2 validator 通过。

本切片没有产品 Command/Query route、local actor/browser auth、Space/Project/Objective/Change/WorkItem repository、
Runtime command client、live outbox/inbox worker、Harness check/Receipt producer、projection/Timeline、Reconciler、
completion authority、backup/repair、schema v2 或 UI。health wire 不变，只表示 local process 在 exact schema 验证后
serving，不表示 Runtime/Harness/project/workflow ready。

**completion_boundary:** 本 DONE/Roadmap 标记是正式 acceptance 的候选完成树；只在同一树上
`node harness/acceptance.mjs` 通过时保留，任一失败必须立即撤回两个标记。通过只关闭 FC-03
的基础子集，不代表 F3/F4、R0 Developer Preview、Objective→Outcome 或完整 App 已交付。

### Sprint 145 — Workspace Catalog Application Service v1（R0-C2）— ADR-0104 Proposed（✅ DONE）

本切片只实现 FC-04 的 Go 内部 Workspace catalog。`internal/workspace/domain` 用 pure fold 从 exact
`forge.workspace.space_created`、`project_registered`、`project_snapshot_recorded` event 重建 immutable v1
Space、Project 和 ProjectSnapshot reference；每个 aggregate 必须只有一个 version-1 creation event，未知 schema、
额外 history、payload/scope/actor/source/reference drift 全部失败关闭。`application` 提供 create/get/list、parent
existence、cross-Space binding、expected-version zero、canonical Command/Event 派生、cryptographic EventID、bounded
source-sequence retry 和 stable internal error；`store` 只适配既有 `controlstore.Store`，不打开第二个 DB、不接触 SQL。

Project RootPath 固定为 2–4096 byte canonical absolute POSIX lexical declaration，状态只能是
`declared_unverified`。服务不 stat/open 路径，不调用 Git、不做 language/build/secret discovery。Snapshot 操作名为
`RecordProjectSnapshot`，只保存通过 Platform Core structural validation 的 caller-supplied `RecordRef`，状态固定
`declared_unresolved`；不读取 referenced bytes、不执行 capture、不认证 digest/actor。Alias 是 display token，不是
uniqueness key；ProjectID 才是 identity。List 复用已有 global event cursor，每次最多检查 1,000 个真实 journal
events；目标 aggregate 在输出前重新完整 fold 并重验 parent，任一错误返回原 cursor 的零 item page。parent 不存在
与空 list 严格区分；当前没有 projection table 或 schema v2 migration。

为保持 `controlstore` package export hard cap 30，原 command/event/outbox 三个 storage-location-specific ID conflict
sentinel 收敛为统一 `ErrIdentifierConflict`，错误文本仍区分具体 identifier，现有 exact collision tests 继续覆盖。
这是尚无 production consumer 的 Go `internal` source breaking revision，不冒充 C1→C2 source compatibility；durable
schema、wire 与 health 不变，source rollback 必须恢复旧 sentinel 和 adapter tests。
Control Store 只新增 integrity-checked aggregate-version page；List 复用已有 global-order page，schema/catalog bytes 不变。

当前 focused normal/race evidence 已覆盖纯 fold mutation、invalid-before-commit、parent/cross-Space、canonical
causation/correlation/actor、sequence contention stable-event retry、real SQLite create/get/list、nonexistent path remains absent、
reopen/exact replay、same-key conflict、same aggregate concurrent single winner、filtered cursor 和 canonical-but-semantic
history drift。首轮 fresh-context architecture/storage review 发现 list stale-history、durable parent、partial error page、
boundary evidence、无索引 aggregate-type scan 和 bidi path 风险；实现已改为 global bounded scan + per-aggregate fold，
并补齐 parent/zero-page/Unicode/bounds/retry-exhaustion/real-SQLite unknown-v2 回归。修复后复审、完整 gates 与 formal
acceptance 仍待 completion tree 冻结后执行。

第二轮 fresh review 又发现 parent list 早期错误未保留 nonzero cursor、1,000 行主扫描后 continuation probe
实际读取第 1,001 行，以及继承自 R0-C1 的 pending-outbox anti-join 会无界跨过已 ack 前缀。当前修复把
Workspace budget 明确拆为 999 个过滤/fold rows + 1 个 lookahead，所有 parent 错误保留原 cursor，并把 pending
outbox 改为最多 `limit` 个 raw sequence rows 的窗口、显式检查 cursor 与同 statement exact `More`；schema v1
仍不变。Envelope/scope/payload-type/result receipt 与 acknowledged-prefix/query-plan 回归同步补齐。

第三轮 review 又发现 aggregate replay 未绑定 originating Command actor、schema v1 缺少独立冻结基线，以及批量
global read 会在 item limit 后预验证多于一个 lookahead。当前 aggregate/global reads 同时完整验证 originating
canonical Command 并向 Workspace 暴露 actor；List 固定使用 one-row global page；C1 ordered catalog pin 与压缩
physical `control.db` fixture 独立于 `schemaObjects` 固化。actor mismatch 跨 reopen、fixture reopen 和 single-lookahead
回归已补齐；终审仍须在该修复树上重跑。

第四轮 fresh review 指出 nonexistent-path sentinel 只能证明未创建路径，不能独立证明零读取，并发现
`ControlSourceHead` 遗漏 corruption→`ErrInvalidHistory` 映射。当前 exact production-import allowlist 已锁定
Workspace domain/application/store 只能依赖已审查的 pure/Core/Control 边界，拒绝新增 filesystem/process/network
或 alternate infrastructure import；source-head 现也统一经过稳定错误映射并由全 sentinel 单测覆盖。该修复随后进入
新的 fresh 双终审与候选完成树验证。

最终候选树已通过 focused/full Go normal+race+vet、CGO-disabled build、Darwin/Windows fail-closed
cross-build、gofmt、module verify、architecture **8/8（3181 source files）**、gate **3831 files** 与
governance **13 checks**。修复后的 fresh-context architecture/domain 与 storage/security 双终审均为
`CLEAN`，Blocker/Major/Minor/Nit 全部为 0；Linux 有效 `controlstore` API 为 29 exports，互斥 build-tag
文本口径为 30/30。ADR-0104 保持 Proposed，body/self SHA-256 分别为
`1c24b72737ced3d4142f4b80ca5219b4ed66ee14a97408d1bbe7af8595b6340f` 与
`679188209a3bcc81dca98f53357a2a84a2f807978b071cde498617da43fbff71`；C1 ordered catalog/physical fixture
SHA-256 分别为 `d306abca185dbdf0601b2cda5ab0cb214c1eb5d001ac759aab543aab207552cc` 与
`156f9c54419d770639cf54322eeb0fc3023e6c7e177d03c3cd5e1026742f66b9`，strict ADR validator 与 fixture reopen 通过。

本切片没有 authenticated local actor、authorization、HTTP/CLI/TUI product route、filesystem observer、真正 Snapshot
capture、Objective/Change/WorkGraph/WorkItem、Reconciler、Runtime/Harness transport、outbox/inbox worker、projection、
completion authority、backup/repair 或 UI。health wire 不变，内部 service 尚无用户入口。

**completion_boundary:** 本 DONE/Roadmap 标记是正式 acceptance 的候选完成树；只在同一树上
`node harness/acceptance.mjs` 通过时保留，任一失败必须立即撤回两个标记。通过只关闭 FC-04，
不代表 FC-05、F3/F4、R0 Developer Preview、Objective→Outcome 或完整 App 已交付。

### Sprint 146 — Delivery Domain v1（R0-C3）— ADR-0105 Proposed（✅ DONE）

本切片只实现 FC-05 的 Go 内部 pure Delivery Domain。`internal/delivery/domain` 已定义 Objective、
Change desired/observed state、AcceptanceCriterion、snapshot-bound WorkGraph/WorkItem、budget、DAG 和
caller-supplied snapshot comparison；依赖仅允许 pure 标准库与 Platform Core reference/state vocabulary。
为使 no-production-consumer 的离线 Go metadata 证明不解析降级输出，本切片也对共享内部 `execbound` 做 scoped
hardening：bounded byte stdin、普通 combined capture 的单一 raw pipe、saturating count overflow、显式
`DrainIncomplete`、context failure precedence，以及 gate/Git machine parser 的 fail-closed 处理；所有现有 caller
同树迁移。它是 repository proof 支撑，不是 Delivery production consumer，也不提供 descendant containment。

目标是为后续 pure Reconciler 和单 WorkItem 垂直闭环提供确定输入，而不是先冻结新的跨语言 wire。
本切片不生成 ID/time，不创建 Command/Event，不写 `control.db`，不读取 filesystem/Git，不解析 current Snapshot，
不认证 actor/Approval/Grant，不选择 ready node，不 dispatch Attempt，不消费 Runtime/Harness Receipt，也不暴露
HTTP/CLI/TUI/App。状态 validator 只检查声明边，不推进 current state；snapshot comparison 只比较 supplied IDs。

验收要求覆盖 exact bounds、typed IDs、Unicode/control/bidi、Objective/Change/Graph 状态边、snapshot/criterion
coverage、budget aggregation、Artifact snapshot relation、DAG permutation/cycle/missing/self/duplicate/edge bounds、
deterministic randomized property tests 和 production import allowlist。完成前必须通过 fresh-context architecture/domain
与 security/reliability 双审、共享 executor lifecycle/parser 回归、full Go normal/race/vet/build、architecture/governance
及正式 acceptance。

首轮 fresh security/reliability review 发现 Firecracker 的 `/forge-exit` 与串口结束 sentinel 可被 root guest
workload 伪造、sandbox runner 在返回 nil/zero 的同时已取消时仍可能进入 output commit，以及 Docker/Firecracker
host output count 在 32-bit `int` 上可溢出。候选修复已改为随机 root-only result path、`0700` PID-1 init、
no-new-privs/空 capabilities 的 uid/gid 65534 workload、VMM 真正退出后的 bounded debugfs status/output 读取；
串口只作 bounded diagnostics。sandbox 以 Runner 返回后的立即 context sample 为 completion/cancellation
线性化点，已可见的 cancel/deadline 禁止 validation/commit/Observe；两种 runner 均使用 signed-64 saturating
count。受信 rootfs 现在必须提供 exact `setpriv` profile，旧主机 boot 证据不冒充新版通道 live re-verification。
该轮不是 CLEAN，修复树仍需两位全新 reviewer 重新独立审查。

后续 fresh review 又发现并修复三组边界问题。Firecracker 的 VMM lifecycle 现在由单一 waiter 持有 reap，Unix
用 `Wait4(WNOHANG)` 与 group terminate 共用锁，避免 leader reap 后向复用 PGID 发信号；diagnostic copy 与 reader
close 错误均保留，回归测试直接记录 stale-group signal 调用。共享 `execbound` 不再把负 PGID signal 交给可能晚于
`Cmd.Wait` 的标准库 cancel callback：它自己持有 cancellation watcher，支持 `waitid(WNOWAIT)` 的 Linux 先观察退出、
再在 lifecycle lock 内完成 `Cmd.Wait`，因此 numeric PID/PGID 在 signal/reap 竞争期间不会复用；其余平台安全降级为
`os.Process.Kill` direct-child teardown，同时保留全平台 parent-reader drain bound。gate/orchestrator 的 group-reap
断言已收窄到 Linux，Darwin/BSD/Windows/Solaris/AIX 等目标只验证可移植的 direct-child/drain 合同。

Control Store 的 durable `msg_` identity domain 现在跨 `message_index` 与 `outbox_messages` 统一：command、control event、
inbox event 与 outbox 在同一 commit 和既有 durable state 上都拒绝碰撞，启动关系校验也拒绝跨表腐化；新增测试覆盖
command/event/outbox 的同批与跨批冲突、outbox→inbox 冲突及人工腐化后的 reopen fail-closed。Session worktree 的 Git
输出限制也改为 stdout+stderr aggregate saturation，并从任意 repository 子目录解析 canonical root；split capture 和
尾空格路径回归覆盖对应边界。以上修复树仍以最终 fresh review 与正式 acceptance 为完成条件。

最终 repair verification 已关闭剩余边界：非 benign cancellation failure 现在按 `os/exec` precedence 在成功 wait
后仍返回错误，并由 observed execution 强制分类为 `wait_failed`；local command observation producer 另校验 terminal kind
与 `CtxErr` 一致，矛盾事实不得封存。Sandbox 在 `Runner.Run` 返回后的第一条语句冻结 context error，随后才读取可注入
clock 或复制 output；同步测试证明 sample 后发生的 cancel 不会追溯污染已完成 operation。顶层 `CLAUDE.md` 也已从过时的
forge-core 全模块零依赖声明修正为 sole `modernc.org/sqlite` direct import、exact module closure 与 no-CGo machine policy。
execbound/caller、Control Store/Firecracker 和 whole-tree 三条独立复核最终均为 CLEAN；full Go normal/race/vet/build、
八个非 Linux 目标交叉测试编译、architecture/governance 与 diff/format checks 均已通过。

**completion_boundary:** 本 DONE/ROADMAP 标记只在同一棵冻结树通过 fresh-context 复审与
`node harness/acceptance.mjs` 时保留，任一失败必须立即撤回。通过只关闭 FC-05 pure domain，不代表 FC-06、F3/F4、
R0 Developer Preview、Objective→Outcome 或完整 App 已交付。

### Sprint 147 — Pure Pre-Effect Reconciler v1（R0-C4）— ADR-0106 Proposed（✅ DONE）

本切片只实现 FC-06 的 pure pre-effect selection 子集。`internal/reconcile/application` 作为
`internal/delivery/domain` 的 exact sole production consumer，一次接收 caller-owned、完整且调用期间 race-free stable 的
Objective/Change/WorkGraph、supplied current snapshot identifiers 与 WorkItem assessments。入口重新执行 Domain/DAG、
version/declaration/evidence 和 progressed-predecessor 校验；合法输入按 uncertainty→snapshot drift→aggregate lifecycle→
in-flight→terminal blocker→lexicographic-topological frontier→Policy/Approval/Budget 的固定顺序，恰好返回一个 passive
`NoOp|AwaitApproval|ReadyWorkItem|BlockWorkItem|ReplanChange|EscalateUncertain`。

Assessment 的 `unknown|satisfied_declared|unsatisfied_declared|uncertain` 只是 caller declaration；1–16 个 `RecordRef` 只做
结构与同 identity digest 一致性校验，不解析或认证。`ReadyWorkItem` 仅选择一个零效果候选，不请求 Platform Core edge，
不生成 identity/time，不写 `control.db`，不创建 Command/Event/journal/outbox，不 claim/dispatch Attempt，不调用 Runtime/
Harness，不解析 Artifact/Receipt，不完成 WorkItem/Change，也不提供 loop/worker/retry/manual override/API/CLI/TUI/UI。

最终标记树的 focused normal/race、96.8% package coverage、50 组 fixed-seed randomized DAG permutation、128
WorkItem/assessment exact bound、assessment ID/effect pre-lookup bound、Delivery sole-consumer、full Go normal/race/vet/
CGO-disabled build、module verify、Darwin/FreeBSD/NetBSD/OpenBSD/Windows 八个 OS/arch 目标的全量 production build +
focused test compile、strict ADR v2、architecture 8/8、zero Reconciler production consumer、gate 3924 files 与
governance 13 checks 已通过。fresh-context architecture/domain 与 reliability/security 双审均以
Blocker/Major/Minor/Nit 0 返回 CLEAN；ADR body/self pins 为 `7041df374cc2c5ac5dc70ada740289bc36eacb4ee5cb6ede1341017b4b0011b9`/
`3459996cc7bed1f83fc99a9aa3000e7f0633e69ed7b6536b0719b747345774fc`。

**completion_boundary:** 本 DONE/[x] 标记只关闭 FC-06 pure pre-effect selection 子集，并以紧随封树运行的
`node harness/acceptance.mjs` 作为保留门禁；若正式验收失败必须撤销。F6/F7、FC-07/08、F3/F4、R0 Developer Preview、
Objective→Outcome 与完整 App 继续开放。

### Sprint 148 — Runtime Attempt Request Domain v1（R0-C5 / FR-03a）— ADR-0107 Proposed（✅ DONE）

本切片只建立 Rust Runtime-owned Attempt 的 pure request construction boundary。`AttemptRequestInput` 由 caller
提供完整 Platform Core Scope、explicit Attempt/WorkItem/Project/ProjectSnapshot refs、四项 Control aggregate
versions、executor descriptor、可选 context Artifact/workspace capability/Grant、normalized Approval/effect sets、Attempt budget、timeout 与
idempotency key。构造成功时防御性复制为 private-field `AttemptRequest`，只暴露只读 getter，并把初始 state 固定为
Platform Core `requested`；调用者后续修改原输入不得改变已构造值。

验证必须失败关闭 scope 缺层/越层、explicit ref type/ID substitution、snapshot mismatch、错误 record type、重复或非法
effect、非空 effects 缺 Grant、非法 idempotency、非正/超限 Control version/budget 与 timeout，以及 timeout 超过
duration ceiling。Executor、Artifact、workspace capability、Grant、Approval、effect 与 budget 均只按 caller
declaration 做 structure/relation 检查，不解析 record/artifact
bytes，不认证 producer/issuer/approver/principal/currentness/revocation/permission，不查询 current Control versions，也不
reserve budget。

本切片未加入 Attempt reducer/transition authority、Session/Turn/Action、serde/canonical wire/digest、SQLite schema/
journal/outbox、CAS/Artifact resolution、protocol server/handshake/ack、Runtime adapter、Reconciler consumer、WorkItem edge、
claim/dispatch/provider/tool/filesystem/process/network effect、Verification/completion 或产品入口。

最终标记树的 invariant tests 20/20、boundary tests 6/6、domain lib 436 tests、Rust workspace all-target tests/build、
fmt、strict Clippy、Platform Core/Application 回归、Go workspace regressions、strict ADR v2、gate 3942 files、
architecture 8/8（3271 source files）、governance 13/13 与 diff check 均通过。fresh-context architecture/domain 和
reliability 终审均以 Blocker/Major/Minor/Nit 0 返回 CLEAN；authority-boundary 复核独立确认 caller declaration、
no-consumer/no-effect 与 fail-closed 关系。ADR body/self pins 为
`6b9748819c50bae58ebd349f13751b8b0548df1d23a16a5b94dee5e80246504a`/
`3696901c7e9fb4a896f219391235b0d9cc1575aa755e98db595e49fb4c5c5d79`。

**completion_boundary:** 本 DONE/[x] 标记只在同一棵冻结树通过上述测试、独立复审与紧随封树运行的
`node harness/acceptance.mjs` 时保留，任一失败必须立即撤回。通过也只关闭 FR-03a pure request value；依赖链仍为
`FR-03 后续 lifecycle → FR-04 execution journal/outbox → FR-06 local protocol → FC-07 RuntimePort client`，F2/F3/F4/F6、
R0 Developer Preview、Objective→Outcome 与完整 App 继续开放。

### Sprint 149 — First-party Dev Agent v1（ADR-0108 Proposed；implementation candidate，正式验收待定）

本切片为已有 Rust `AgentRuntime` 增加直接产品入口
`forge-runtime -C PATH agent [--dev] PROMPT|-`，而不是引入第二套 loop、scheduler 或 persistence。
默认版本化 toolset 只含 `list_files`、`search_text` 与 `read_file`；Unix 上只有显式 `--dev` 才增加
`edit_file` 与 `exec_command`。外部 Claude Code/Codex 仍可互操作，但不再是这条 candidate 路径的运行前提。
该入口只处理单个有限任务，不执行 Sprint、Attempt、Work Graph 或 Group Graph 编排。

credential preflight 位于 stdin/workspace/Hub 之前；sole `-` 从 stdin 读取最多 256 KiB 的非空 valid UTF-8，
避免把敏感 Prompt 放入 argv/shell history。Human/JSON start disclosure 明示 provider egress、local plaintext
journal、same-user execution 以及不存在 filesystem/network sandbox。外层 human provider failure 只返回稳定错误码和
durable Run 检查指引，不复制 provider-controlled secret/control/bidi 文本；machine event stream 仍保留既有协议事实。

Project selection 只打开一次 descriptor-anchored `CapStdAgentWorkspace`：selected path 与 canonical path 经
filesystem identity 双向重验，随后 Project registration、persisted workspace identity、Runtime factory 和全部工具复用
同一 bundle。路径在 descriptor open 后被替换时失败关闭；恢复旧 Run 仍按 persisted mode/toolset version 与 workspace
identity 重建，未知版本在 provider/tool/writeback 前拒绝。
Agent explicit resume 默认复用不显示 tool arguments/results 的 HumanEventSink，只有 global `--json` 才输出完整
machine event stream；每个不受信 assistant 物理行固定带 `[assistant]` 前缀并转义 terminal control，后续 human
Prompt list 也转义已持久化 provider text，不能伪造 trusted run/tool status 行。runtime/provider failure 的外层
stderr 同样只显示 stable code。

Agent start 现在以一个 SQLite immediate transaction 原子提交 Prompt、Run、seq-1 `run_started` 与 seq-2 matching
user `message_committed`。只有 `BeginRunDisposition::Created` 且 inspection 仍为 exact pristine two-event seed 才能构造并
调用 provider；同 idempotency key 的精确重放不会自动再次发送。terminal replay 只做幂等 assistant reconcile，
incomplete replay 固定要求 operator 先检查后显式 `run resume`，pending tool effect 继续拒绝自动重放；并发同 key
process regression 要求最多一个 provider request。terminal replay 返回既有 outcome，仅 completed outcome 触发
assistant writeback reconcile；failed/cancelled/limit-exceeded 不产生 assistant writeback。

所有可能持有执行权的 original start/Agent atomic seed/explicit resume 都在 provider/tool loop 全程持有 private
Hub-side empty coordination file 上的 nonblocking OS lock；zero-byte 文件会持久存在但不保存 owner token，进程退出
释放的是 OS lock。Unix 上已打开文件无论 caller umask 都强制固化并复验为 current-user-owned `0600`、single-link
empty regular file。同一 Hub 即使是不同 Run 也保守串行，竞争者在 provider/tool 前失败。该锁只协调 participating
Forge process，不抵御 non-cooperating same-user program，也不提供 crash 后 remote exactly-once。

文件发现/search/read 继续走同一 workspace descriptor。`edit_file` 的 staged plaintext 在 namespace commit 前保持
private `0600`；replacement 仅在 staged inode 与 target owner/group 相同时继续，并在 rename 后恢复且复验旧 POSIX
mode。namespace-changing rename/link 一旦调用，其 Err（包括 network filesystem 的歧义失败）以及后续
确认/sync/permission 错误都返回 `tool_effect_uncertain`。replacement CAS 重验 content digest 与 length/device/inode/
mode/nlink/uid/gid/ctime identity。Linux 还会拒绝 parent default POSIX ACL、calling process 可枚举的任意 target
或 staged xattr/access ACL、不可读或发生漂移的可枚举 extended metadata，避免 v1 静默丢失
已观察到且无法精确保留的安全元数据；kernel 对调用进程隐藏的 attribute 不在保证内。这不构成 non-Linux 通用
ACL/xattr preservation，也不是对非协作 writer 的全局 filesystem transaction。
任何 precommit 失败若不能确认 private named stage 已删除或原本不存在，也升级为 `tool_effect_uncertain`，避免把
可能残留的部分 plaintext temp 误报为普通安全失败。

`exec_command` 的 cwd 从已打开 workspace descriptor 进入，清空环境后只复制显式 non-secret operational 变量。
任何 spawn 后 abnormal result——timeout、cancellation、wait error 或 direct child 已退出但 capture pipe 仍被持有——
一律返回 `tool_effect_uncertain`，不写 false `ToolFinished`/terminal。原 process group 的 kill/reap 只是 bounded best effort，
不能证明 descendant 未创建新 session；direct child 已 reap 后也不再向可复用 numeric PID/PGID blind signal。
只有 pre-spawn cancellation 保持无进程 effect 的普通 cancelled。

Agent public profile 固定最多 64 turns、256 configured tool calls、32,768 output tokens、256 KiB cumulative model output
与 2,048 个可从 journal 重建的 text/tool-call/provider-context model events。每 turn 另只允许一个 aggregate Usage
与一个 Finished；重复 Usage 是 protocol failure，因此不会形成 resume 后遗忘的无界计数。每次 tool output 为
`min(128 KiB, 32 MiB / (12 × max_tool_calls))`；12 来自一个结果在 `ToolFinished` 与 Tool message 中两份持久化、
且 JSON string 每输入 byte 最坏 6 倍展开。默认 64 calls 得 43,690 bytes，public maximum 256 calls 得 10,922 bytes。
保守事件证明保持在 8,192 上限内；真实 Application + SQLite 回归已让单 turn 2,048 个 minimal tool calls 到达 terminal
seq 4,101，并让 256 个完整 NUL worst-expansion outputs 后仍能追加 terminal，cursor byte count 与实际 stored JSON
一致且不超过 64 MiB。

上述只约束 per-Run journal 与最终 retained history projection，不约束 Hub 生命周期总 Prompt/Run/plaintext/disk 增长；
causal history integrity query、Project binding、keyed replay 与 session/global snapshot 在返回 bounded projection 前仍可
按完整 Conversation/Hub 增长 SQL 或内存工作量。v1 不提供 retention、prune、总磁盘 quota 或固定 SQL/memory-work
budget，长期 local state 仍由 operator 管理，production availability 需另立合同。

当前仅记录 implementation candidate 事实。ADR-0108 继续是 Proposed/null，正文与 body/self seal 未因本节改变；
不得把 focused tests、候选代码或本节文字冒充 independent final review、完整 gate 或正式 repository acceptance。
最终晋级前仍须在同一冻结树完成 Rust fmt/strict Clippy/workspace all-target tests、repository architecture/governance/diff
checks、fresh-context architecture/security/final review，并紧随其后运行正式 `node harness/acceptance.mjs`。

**completion_boundary:** 在上述终审与正式 acceptance 成功前，ROADMAP 项保持未勾选、Functional audit 保持
`PROPOSED-STAGED`。即使最终验收通过，也只关闭 trusted same-user local single-task developer preview；OS/network
sandbox、production approval、remote deploy、multi-Agent、automatic Sprint/Attempt/Graph、provider-side idempotency 与
任意 shell 安全保证仍不在本切片内。

### Sprint 150 — Runtime Attempt Lifecycle Domain v1（R0-C6 / FR-03b）— ADR-0109 Proposed（✅ DONE；以本节 completion_boundary 为条件）

本切片已由 Roadmap 选择，用户于 2026-09-08 明确要求继续实现既定的 R0-C6 窄边界。实现只在 Rust domain 增加
sibling `execution::attempt_lifecycle`：private-state `AttemptLifecycle` 从 explicit `requested` seed 开始，closed
`AttemptTransitionRequest` 只含 `Accept`、`BeginStarting`、`ObserveRunning`、`ObserveInterrupted`、`ObserveCompleted`、
`ObserveFailed` 与 `ObserveEffectOutcomeUncertain` 七类请求；每次 reducer 都复用 Platform Core `validate_attempt_transition`，返回新 value，
不复制状态图或修改 current value。

影响文件是 ADR-0109、`execution/mod.rs`、独立 `attempt_lifecycle.rs`、focused lifecycle tests，以及 R0-C5
boundary support 中新增但不放宽原规则的 lifecycle-only source/API/no-consumer policy。`execution::attempt` 四文件及
`AttemptRequest` public API 必须保持 frozen；Platform Core state vocabulary/edge table、Cargo dependency manifests、
SQLite schema、wire 和现有 consumer 不变。正式验收暴露既有 CLI concurrency fixture 的时序竞争后，另增加
test-only response gate，并替换三处依赖固定 sleep 的并发测试同步；不修改 CLI production behavior。
第二轮验收后还加固两个既有 test-only stdin fixture：模拟 materializer 在返回前 drain input；CLI helper
在完整 wait/reap 后只允许 unsuccessful exit 的 `BrokenPipe`，success 仍要求完整 write。生产 I/O 拒绝行为不变。

机器验收已证明：八个 known current states × 七类 request 的 56 组合恰好 13 个 canonical edge 成功、43 个
失败；四个 terminal state 全关闭；same-state/backward/reset/retry/reopen/fast-forward 不存在或失败；只有 explicit
effect-outcome-uncertain request 可产生 `uncertain`；重复与并发调用确定、failure atomic。Source/API inventory
拒绝 raw/Unknown target、unchecked restore、`Default`/`From`、public field/`&mut self`、serde/wire/digest、identity/
version、budget/usage、journal/outbox/receipt、ambient I/O、provider/tool/Harness/Go 依赖和未审 production consumer。

ADR-0109 strict v2 validation、代码前 fresh-context design review 与实现后的 independent architecture/domain review
均通过。Reliability/security 首轮发现 `#[r#path]` 可绕过既有 path scanner；现已规范化 raw attribute identifier，
并覆盖正向 local path、lifecycle source/test/module、旧 Attempt source、absolute escape 与 inline module 的回归。
修复后的 fresh-context reliability/security review 为 CLEAN，状态文档的独立复核也已消除 candidate/status 和
caller-declaration 语义歧义。ADR-0109 继续保持 Proposed/null；以上 review 不产生 authenticated approval authority。

Focused lifecycle tests 4/4、boundary tests 14/14、domain lib 436 tests、Rust workspace all-target/all-feature tests、
fmt、strict Clippy、all-target build、architecture 八项、governance 十三项与 diff checks 均通过。首次 sandbox
workspace run 在需要 bind loopback 的 CLI fixture 遇到 `Operation not permitted`；已获准在 sandbox 外移除真实
模型凭证并以 locked/offline 重跑全量，全部通过。Shared `domain/src/lib.rs` 的既有 0664 权限已恢复为 0644，
未修改该文件 source bytes 或 registration。正式 `node harness/acceptance.mjs` 首轮返回 8 PASS、1 FAIL、2 N/A；
失败是既有 `cli_agent_idempotency` 在 project test 中退出 101。首轮条件化完成标记已撤回；focused 重现确认
`different_key_lock_loser_announces_its_durable_run_for_explicit_resume` 的固定 sleep 不能保证 contender 重叠。
修复使用显式 request-arrival/response-release gate：首个 request 到达后才启动 contender，取得 contender 结果
后才释放首个 response；等待有界，channel disconnect 可退出，不削弱原有 lock/idempotency assertions。
修复后受影响 CLI tests 21/21，通过三轮并行重复的 idempotency/atomicity tests 27/27；最终 helper 借用签名
调整后再次通过 idempotency/atomicity 9/9、fmt、strict Clippy、all-target/all-feature build、architecture 八项、
governance 十三项与 diff checks。Lifecycle 4/4 与 boundary 14/14 也通过，新增 helper 未引入 consumer 例外。
Test-only 修复的 fresh-context independent review 为 CLEAN。上述独立全 workspace pass 属于修复前结果；
修复后最终冻结树的正式 `node harness/acceptance.mjs` 必须重新完整运行，不能沿用首轮失败或先前测试结果。
第二轮正式验收仍返回 8 PASS、1 FAIL、2 N/A：workspace 全量通过，但逐 crate 检查中的
`scheduled_node_materialization_bridge` 与 `cli_group_agent_node_dispatch_authorization` 失败；完成标记再次撤回，
该轮不能作为完成依据。Rust strict Clippy/typecheck/build 均通过；Python coverage 为 84.133%，
整体 lint/coverage 因其他语言工具缺失或未配置而诚实 N/A。

第二轮原始 assertion 被 project adapter 摘要截断，两个目标 isolated 重跑通过；materializer 六轮完整重跑
与另 32 次 invalid-candidate 重跑（含 12 次并发）均通过。因此无法声称已复现该轮具体失败原因。
代码检查发现的 fixture stdin/exit race 已作上述 test-only 加固；CLI 新增两个确定性 regression cases，先确认
子进程关闭 stdin 再写入，分别保留拒绝 status/diagnostic 与拒绝 successful incomplete write，不使用 sleep。
修复后 materializer 4/4、四组受影响 CLI targets 合计 20/20、lifecycle 4/4、boundary 14/14、fmt、workspace
strict Clippy/all-target/all-feature build、architecture 八项、governance 十三项与 diff checks 全通过；两项独立
fresh-context fixture reviews 均为 CLEAN，但不证明先前 clipped failure 的具体原因。
最终封树后须重跑完整正式验收；设置 `CARGO_TERM_QUIET=true` 仅减少 Cargo progress 输出，已验证仍保留
真实测试计数，避免 progress 淹没失败摘要，不改变测试目标、features 或断言。仍移除真实模型凭证并离线运行。
第三轮正式验收返回 8 PASS、1 FAIL、2 N/A，`test_pass_project` worker 被 SIGKILL 终止，未形成完整项目
测试结果；其余正式检查通过，整体 lint/coverage 仍为诚实 N/A。完成标记再次撤回；SIGKILL 原因尚未证实，
不能把 worker 终止当作测试通过或已定位的代码缺陷，最终正式验收仍待完成。
有限只读排查将该信号定位到 worker 的外层 `unshare` launcher，未找到 OOM 或跨任务全局 kill 的因果证据；
现有 private process group、PID/start-time 与 exact worker token 清理边界未发现可据此修复的缺陷。
未改变验收清理逻辑，也未停止或修改外部 campaign。终止来源未明，当前只交接 implementation candidate。

2026-09-09 继续实现时，fresh-context 复审确认 Serde `remote` literal-path 可绕过 lifecycle no-consumer gate：
normal/raw/Unicode-escaped mirror enum 均可编译并调用 generated deserialize，旧三项 lexical/path gates 却接受。
新增四项 boundary regressions，首项在旧实现上稳定失败；修复只扩充 lifecycle 的 Serde metadata policy，
不修改 R0-C5 frozen lexer/inventory、AttemptRequest、Platform Core 或 lifecycle production bytes。
新 policy 在 source/test exemptions 前扫描真实 attribute，跳过无关 comments/literals，并规范化 raw identifier；
只允许 closed inert metadata grammar。所有未审 path-bearing/unknown options 失败关闭，不依赖解码后搜索
lifecycle 名称；仅保留 exact `Option::is_none` 两种已有 attribute 形状，以及由 source path + whole-source
SHA-256 双重固定的 `run_store` legacy named default。Normal/raw/Unicode/hex literal、raw option names、
nested/unknown option、旧 exemption 路径、inert wire names 与例外替换均有回归。
修复后 lifecycle 4/4、boundary 18/18、fmt、workspace strict Clippy 与 all-target/all-feature build、
architecture 八项、governance 十三项、file gate 与 ADR strict v2 validation 均通过。
另一个全新上下文的 independent reviewer 对 Serde 修复及 integration 返回 CLEAN，并独立运行 boundary
18/18（含 live workspace scan）；冻结 lexer/codegen/inventories、原 Attempt、Platform Core 与依赖未改变。
本轮带 signal-only trace 的独立 project probe 因转入已确认边界缺口修复而由本任务主动 SIGINT 取消（exit 130），
不属于正式 acceptance，不是先前 SIGKILL 的复现，也不提供项目测试通过证据。

**completion_boundary:** 本 DONE/[x] 为封树前预置的条件化标记，仅在同一棵冻结树通过上述测试、独立复审与紧随封树运行的
`node harness/acceptance.mjs` 后保留；标记本身不是验收结果，任一失败必须立即撤回。通过只关闭 FR-03b authority-free Attempt lifecycle
value/reducer；Session/Turn/Action、FR-04 durable aggregate/journal/outbox、FR-06 local protocol、FC-07 RuntimePort、
authenticated authority、budget reservation/usage、effect 与产品入口继续开放。

### Sprint 151 — Runtime Attempt Admission Journal v1（R0-C7 / FR-04a）— ADR-0110 Proposed（✅ DONE；以本节 completion_boundary 为条件）

用户在 R0-C6 完成后于 2026-09-09 要求继续实现。前一切片第四轮正式验收已返回 ACCEPTED：9 PASS、
0 FAIL、2 个诚实 N/A，验收前后 tracked diff 与所有 untracked source digests 一致；不沿用其结果验收本切片。

本片选择实际 SQLite requested admission，而不提前建立需要独立 observation evidence 合同的 lifecycle
推进。设计见 `docs/design/forge-workspace/runtime-attempt-admission-v1.md`；fresh-context design review
已通过，并修复 caller 无法构造 private request digest 绑定 payload 的缺口：新增 pure `request_sha256`
准备接口。ADR-0110 strict v2 validation 通过，仍为 Proposed/null；design review 不等于实现或正式验收。

新 adapter 仅在 caller 显式打开并移交的 on-disk Connection 上工作；拒绝 memory/attached/active-transaction/
foreign schema，验证 exact SQLite v1 profile，WAL/FULL/foreign keys 和完整有界数据关系。路径、权限、descriptor、
SQLite/VFS 与 host 信任仍归 caller，不宣称安全文件 opener、sandbox 或生产 readiness。旧 Hub v29 不迁移。

私有 request record 通过 frozen getters 编码，恢复时重建十五字段 input 并再次调用原 validator；canonical
bytes/digest、固定 Platform Core creation Event、数据库全局 Attempt/key/event/message uniqueness、连续 local
cursor、request/event/outbox 一对一关系在同一 transaction 重验。Exact duplicate 返回原结果，不新增 cursor/
outbox；任何 payload/event 差异冲突。所有状态只为 `requested`，无 runnable/authorization 含义。

边界闸门仅增加 exact path + whole-source-SHA reviewed consumers，Serde/codegen/path 检查仍先执行；原 Attempt
四文件、lifecycle source/API、Platform Core 与 Cargo dependencies 不改，且不允许 lifecycle consumer。
本片不增加 transition、evidence authentication、budget reservation、effect、sender/ack/delete、application
service、local protocol、Go consumer、CLI/UI 或 Receipt producer。SQLite error 不合成 lifecycle `uncertain`。

实现后的 sqlite_execution unit tests 15/15 与 admission integration tests 22/22 通过；其中独立编写的
4 种 crash/fault 场景加 1 个显式子进程 fixture 覆盖三表已写入但尚未 commit 的真实 kill、commit 后回复丢失的 kill、
precommit error 与 deferred-foreign-key commit error 的全事务回滚。子进程以明确 pipe 信号同步并有界
wait/kill/reap，不用 sleep 猜测边界；这些不证明断电、恶意 VFS 或物理硬件 durability。
私有 codec 与 corruption tests 覆盖 exact bytes、missing/unknown/duplicate/nested fields、digest、schema、
完整关系和先检查长度再读取记录；22 integration cases 覆盖 creation binding、重开、幂等、并发、count
capacity 和有界分页。新 21-file path+SHA consumer closure、4 项新增拒绝/复用回归已接入 live workspace
scan，lifecycle 4/4、boundary 22/22 通过。原 Attempt/lifecycle/Platform Core/Cargo bytes 未改变。
workspace fmt、strict Clippy、all-target/all-feature build、architecture 8/8、file gate、governance 13/13、
strict ADR v2 与 diff check 通过；不沿用前一切片正式验收替代本树验收。
另一个全新上下文的 implementation/security reviewer 独立核验 21 个唯一 path 与对应源码 SHA，确认
consumer closure 无 missing/extra、Serde/codegen 先于 exact exception、path/r#path reuse 失败关闭；其独立
执行的 codec 4/4、corruption 6/6 与 admission boundary regressions 4/4 均通过。该审查不替代正式验收。

**completion_boundary:** 本 DONE/[x] 为封树前预置的条件化标记，不是验收结果。只有最终冻结树完成 request/event/transaction/
reopen/concurrency/corruption/capacity/page/crash 回归、fresh-context independent implementation/security reviews、
Rust fmt/strict Clippy/tests/build、architecture/governance 与完整 `node harness/acceptance.mjs` 后，才可保留
R0-C7 DONE/[x]；任一失败立即撤回。本片最多关闭 FR-04a requested admission，不关闭完整 FR-04、FR-06、FC-07
或产品执行能力，也不改变 ADR 的 Proposed/null 状态。

### Sprint 152 — Hub Conversation Change Journal v1（DONE；ADR-0111 仍为 Proposed）

个人多端路线从 Rust Hub 的跨进程同步基础开始。ADR-0111 严格通过 Proposed-only v2 文档校验并继续保持 Proposed/null；它只决策将新 Conversation 与 Prompt 写入绑定到 Hub v30 的本地 ID-only change journal。

候选实现保留 ADR-0007 的 Rust canonical owner：创建 Conversation 与追加 Prompt 时，在原 immediate transaction 内各追加一条变更记录并推进持久化全局 cursor head 与 per-Conversation version head；精确幂等重放不新增游标。v29 已有 Conversation/Prompt 保留且不伪造历史事件；迁移在独立 baseline 表登记其 Conversation ID，并将版本头和全局游标头初始化为零；不生成历史 event/cursor，其首条 v30 后 Prompt 从 journal version 1 开始并可继续写入。读写会验证完整版本头集合、baseline 与日志计数/最大值，因此中间事件、尾事件或 baseline 标记缺失均拒绝继续追加或读取；若非 baseline 的新会话缺少 `conversation_created` event，Prompt 写入也失败关闭。读接口只将 Global snapshot 与 store-global cursor head 配对；Project/Group snapshot 是点查询，不声称拥有 scoped feed。当前 cursor 只表示本 Hub 的日志位置，不是身份、租户、权限或审计证明。

本 Sprint 不增加 Go/Public API、OIDC/Snaplink 验证、跨进程 Runtime transport、客户端、多端同步、Runner、设备库存、placement 或远程执行。它只实现持久游标基础；R0-C7/FR-04a 的状态与本记录互不替代。

初始 focused application contract `snapshot_cursor_and_change_pages_support_idempotent_session_sync` 通过（1/1）。独立复审修复了既有 v29 Conversation 首次追加 Prompt 被拒绝的问题，收窄 snapshot bootstrap 到 Global，并增加 baseline 元数据以区分合法旧历史与新会话缺失创建事件；随后增加持久化全局/Conversation heads，以检测尾部事件删除和游标重排。Prompt 变更追加已集中在公共 `insert_prompt` 事务 helper，覆盖普通 Prompt、Run 用户 Prompt 与完成 Run 的 assistant 写回；插入日志失败、推进游标失败及后续 Run event 失败均有回滚和相同幂等请求重试证据。v30 migration 8/8、Hub 集成 20/20、Run seed 原子性 6/6、assistant writeback 1/1、open-lock adversarial 6/6（包含 15.84 秒超时边界）、完整 Rust workspace/全目标测试通过。外部持锁重试现在以 15 秒 deadline 配合 10–100ms bounded exponential backoff，单次在途 SQLite busy timeout 最多延续 250ms，deadline 后不再开始新尝试。

最终冻结实现树的正式 `node harness/acceptance.mjs` 通过：9 pass、0 fail、2 N/A；N/A 仅表示 Rust/TypeScript coverage 与部分未安装 lint 工具没有配置，不计作通过项。递归测试包含 Python 100 files/1446 tests、Node 41 files/609 tests、Go forge-core 2964 tests，以及全部观察到的 Rust workspace/all-target suites；Clippy、类型检查、构建、架构、文件长度、治理、安全扫描和依赖扫描通过。全新上下文 implementation/security review 与最终 fresh-tree review 均未发现 blocker。格式、严格 Clippy、diff check、ADR-0111 Proposed-only v2 校验及其他适用门禁通过；ADR-0111 保持 Proposed/null，未被本 Sprint 接纳。

本 Sprint 将 Hub SQLite BUSY/LOCKED 的 open 重试窗口从 Sprint 35 的 5 秒提高到 15 秒，并改用 10–100ms 有界退避。v30 首次 schema initialization 会竞争运行 8×16 并发 opener；在整套 infrastructure tests 并行运行时，旧 5 秒窗口可耗尽并使合法 opener 失败。完整 `sqlite_hub` target 连续运行三次通过；外部持锁 fixture 断言 14–18 秒边界，并实测 15.84 秒失败关闭。重试 sleep 被裁剪到 deadline 剩余时间，deadline 后不会开始新 open；expected schema cache 在 migration writer lock 前预热，避免把历史结构回放算入锁持有时间。

**completion_boundary:** 只在同一冻结树完成 populated v29 migration/reopen、schema 精确校验与 rollback、实体/变更原子性、幂等与冲突、不连续/错误游标拒绝、snapshot+head 原子读取、Rust focused tests/fmt/strict Clippy、架构/governance checks、fresh-context review，并按仓库正式流程完成 applicable acceptance 后，才可把本 Sprint 的 Hub journal foundation 标为完成。即使通过，也只关闭本地变更日志与复制起点子项，不关闭 authenticated coordinator API、P1/P2 客户端、FR-06 transport 或 P3/P4 Device Fabric。

### Sprint 153 — Runtime Hub Read Bridge v1（ADR-0112 Proposed；Sprint 154 同树复验通过）

为 Go Coordinator 的后续认证 Query API 建立真实的 Go→Rust 本机进程 seam。本切片不开放 HTTP route、不 provision Snaplink client、不写 Conversation/Prompt/Run，也不读取 Go controlstore 里的 Hub 状态；Rust Hub 继续是唯一 Conversation/Prompt canonical owner。

Rust `forge-runtime` CLI 增加仅供本机进程调用的 `--runtime-rpc --database <path>` 模式，使用 `forgeos.runtime-bridge/v1` 单请求单进程、stdin/stdout JSONL 合同，仅允许 `snapshot_at_cursor` 与最大 128 行的 `conversation_changes_after`。Request/response 各有严格字节上限、精确单 LF framing、unknown-field 与 request ID 检查；输出 serializer 通过 capped writer 限制 JSON wire buffer。Hub 以已有 current-schema live read-only 打开，支持读取活动 WAL，不 migration、不 logical write；错误不回显 path/SQLite detail，snapshot Project DTO 不包含 `Project.path`，change rows 仍是 ID-only。

新增 Go `internal/runtimebridge` subprocess client：绝对 Runtime executable/state dir、固定 argv、无 shell、清空 child env、context deadline、`WaitDelay` 保证继承输出管道不会拖住请求；配置约定必须启动直接的 Runtime 二进制且不派生子进程。响应进行 exact framing、duplicate/unknown/missing-field 验证（包括 snapshot 数组成员、Global/Project/Group tagged scope 的字段形状与错误对象），验证连续 page 与 cursor 边界。AppServer StateDir 与 RuntimeStateDir 必须已存在、经 symlink resolution 后物理路径不相同且互不嵌套。此 client 尚未接入 `appserver.Run` 或网络 handler。

资源边界诚实性：RPC JSON wire 有 2 MiB 上限，序列化不再构造超限完整字符串；但目前 `HubService::snapshot_at_cursor` 在输出 DTO 前会完整加载 Global snapshot，因此此桥接无数据库读取/RSS 硬上限。保持本机、非远程、低频开发边界；任何远程 bootstrap API 前必须增加数据库侧分页或 pre-load size gate。

focused evidence：Rust `runtime_rpc` 基础单测；Go runtimebridge race tests（含继承输出管道时的 bounded timeout、重复 JSON key/嵌套缺字段与 symlink alias 拒绝），Go vet 通过。真实二进制 E2E 使用环境变量启用：`forge-runtime` 创建 Hub，再由同一个 `forge-runtime --runtime-rpc` 入口与 Go Client 读取 snapshot cursor 和 empty tail page。Sprint 154 扩展后的冻结同树通过正式 acceptance：9 pass、0 fail、2 N/A；strict Clippy、架构和治理检查通过。

**completion_boundary:** ADR-0112 保持 Proposed/null。只有最终冻结树通过 reviewer、架构/治理/文件预算、Go Rust focused tests 与适用正式 `forge accept` 后，本地只读进程 bridge 才能标为完成；通过最多完成 Go→Rust 的两个只读 query，不表示 Coordinator network API、安全身份、Prompt/Run 写入、Timeline、CLI/TUI/Web/App/Mobile、设备库存或计算调度已完成。

### Sprint 154 — Local Conversation Prompt History Page（ADR-0112 Proposed 扩展；同树验收通过）

沿用已审的 same-user、本机、只读 Go→Rust 边界，为会话正文增加真正分页读取。Rust Hub 新增 `ConversationPromptPage`，按 `(created_at_ms DESC, id DESC)` exclusive keyset 读取单个 Conversation；先 SQL 扫描最多 129 条轻量 metadata，再逐条加载 Prompt body，硬限 128 条/页与 256 KiB aggregate UTF-8 正文预算。达到字节预算时返回 `has_more` 与末条 cursor，不静默截断；持久库中发现超过 Hub Prompt/id/role 约束的异常行则 fail closed。专用 DTO 不查询、不序列化 `idempotency_key`。

`forge-runtime --runtime-rpc` 与 Go `Client.ConversationPrompts` 增加 `conversation_prompt_page` 操作；请求 limit、Conversation ID、cursor 有严格边界，响应校验 exact nested field set、scope、排序、cursor、记录归属和聚合正文大小。此接口仍不接 App Server startup/HTTP，不构成身份/租户授权；Prompt 内容只跨同 OS user 的可信本机进程。Global snapshot 预加载全量行的 RSS 缺口仍在，不能远程 bootstrap 或高频 polling。

focused evidence：Rust SQLite 页测试 3/3（Conversation 隔离、同时间戳稳定排序、无重漏续页、aggregate byte budget、数据库超长正文拒绝、只读 cursor 不变）；Runtime RPC 8/8（含 256 KiB NUL 正文 JSON 6× escape 仍低于 2 MiB wire cap、unknown operation/字段与缺失会话拒绝、原始响应不含幂等键）；Go runtimebridge race tests/vet 通过，并增加原始非法 UTF-8 拒绝测试；真实 Runtime 二进制创建 Hub/会话/Prompt 后 Go Client snapshot、change cursor、Prompt page 往返通过。冻结同树正式 acceptance：9 pass、0 fail、2 N/A、ACCEPTED。递归套件：Python 100 files/1446 tests、Node 41 files/609 tests、Rust 5 组（436/109/436/422/263）、forge-core Go 2974 tests；架构、治理、安全、SCA、类型检查与构建通过。

**completion_boundary:** ADR-0112 保持 Proposed/null。此页最多完成本机单会话 Prompt-history read query，不完成 Prompt submit→Run、Run timeline、Snaplink认证与 owner授权、HTTP/API、客户端 UI、设备登记或调度。

### Sprint 155 — Bounded Local Conversation Bootstrap Page（ADR-0112 Proposed 扩展；实现中）

为后续共享会话 Query API 提供有界初始化路径，继续保持同 OS user 的本机只读桥接、不接 HTTP。首个 SQLite read transaction 原子读取 journal head 与第一分页边界；固定 head 被每个 continuation 携带。Bootstrap 分两个有界阶段：先按 binary ID 从 v29 baseline 表分页，再按 change cursor 扫描不超过 `limit+1` 个 journal rows 并只 hydration `conversation_created` 记录；Prompt-only 页可返回空会话集合；`scanned_through_cursor` 明示已扫描位置，使最终空页也可验证。每条输出附 `creation_cursor` 与读取时 `aggregate_version`，允许客户端对 head 后重放采用单调版本合并。每个响应至多 128 条 Conversation metadata，禁止 Prompt body/path。

本切片仅界定 Conversation bootstrap；Global `snapshot_at_cursor` 仍会完整物化 projects/groups/members/conversations，不得远程使用。journal head 当前通过 count/max 验证，会扫描全日志、没有硬 CPU 上限。Snaplink SDK 可复用验证 access token 和 route scope，但 Forge conversations 仍缺少 issuer/subject/tenant owner 与 instance-grant 持久化合同，所以此页不开放给网络主体。

**acceptance_boundary:** 覆盖 v29 baseline、journal phase、跨分页创建/Prompt 并发、并列 ID/游标、损坏日志、超前 head、只读状态不变、真实 Go↔Rust E2E、ADR/architecture/strict Clippy 与正式同树验收后再标记完成。通过最多完成本地有界会话初始化，不完成 auth、API、多端 UI、Prompt 写入、Run timeline、设备资源或调度。

正式 acceptance 进程曾被启动，但进程结束时没有留下 verdict 或可核验 artifact；Sprint 155 继续保持“实现中/验收待完成”，不沿用任何旧树验收声称其已完成。


### Sprint 156 — P0 对齐与 owner-enforced authenticated session API foundation（实现中；ADR-0113 Proposed/null）

用户已批准个人单账号、单一逻辑 Coordinator 下的跨端会话和后续多设备执行路线。该批准已同步到 `.agent/ROADMAP.md`、cross-device plan、产品蓝图/实施计划与功能交互设计；它是产品实施授权，不是 ADR v2 lifecycle acceptance。ADR-0113 以 strict Proposed-only v2 candidate 记录 Snaplink principal、私有 HTTPS 部署边界、Rust Hub Conversation owner 与 owner-filtered query/write contract，字段 `status=proposed`、`accepted_at_unix_ms=null`、`acceptance_id=null` 必须保持不变。

已实现切片：Rust Hub v32 新增 exact `(issuer, subject, tenant_id)` owner 表；v30 既有会话保持无 owner，网络 API 不返回、不自动 claim。Go App Server 经 Snaplink resource-server 验证 JWT 与 `forge:conversations:read/write` scopes，从已验证 claims 构造 owner；session API 配置必须 pin 唯一 Coordinator tenant/subject，防止同租户其他账号通过已知 Project/Group ID 污染本地 scoped views。支持有界 Global/Project/Group 会话列表与创建、历史分页、Prompt 幂等追加和 `GET /api/v1/conversation-changes` owner-filtered replay feed，Rust Runtime RPC v2 在 Hub 事务中重复验证 owner，Prompt append 使用 aggregate-version CAS。Conversation scope 只是组织信息：每次读写仍限制在精确 owner，Group 成员不会获得他人会话，Project scope 不授予执行权限。Replay feed 使用 v32 每个精确 owner 独立的 dense cursor，Hub global journal cursor 只在服务端内部映射校验；foreign-owner activity 不改变其他 owner 的 head/游标。迁移从既有 owned changes 回填 owner-local 序列，legacy ownerless 会话不进入 feed。Page 只含 ID 元数据；CLI 有 `remote changes list`，TUI 有 `sync`，Console 每 15 秒轮询最多四个 128-row page；CLI/TUI 使用本地 owner-bound 检查点，Flutter Console 在 Web/native 平台持久化 owner-bound cursor。Flutter history DTO 校验 owner、排序、cursor 与 256 KiB 内容预算，并接受合法的 byte-budget partial page。客户端均无 live stream。Go/Flutter 校验 dense continuity、scanned cursor、empty page 不推进及 change-feed `has_more` 页长约束。指定的 `~/workspace/demo/snaplink` 已提供通用 RFC 8628 设备授权端点、需认证的审批、client-bound polling、`slow_down` 和可配置 DeviceCodeStore；后续续段已添加 Forge CLI/Console public-client 源配置和 CLI device login。Sprint 156 原始切片中的 CLI 仅存 access token；2026-09-14 continuation 增加 Linux/macOS refresh rotation 与系统 keyring（见 §45）；Console token 独立存于 tab-scoped session storage。源配置 race 测试验证 device/client 同账号 owner tuple 与 Forge audience 相同，但不证明 live deployment 已加载配置。远程监听要求显式私网地址、TLS 和会话配置；Web CORS 只接受显式 exact Origin，并允许有限方法/headers。Prompt 目前只固化 Rust-owned user Prompt，不创建 Run intent、不声称任务已执行；Run intent 还要服务端 Project consent/grant 与执行 profile。完整跨端 journey 未交付，P1/P2 仍开放。

Rust CLI 已有 `remote login`、`remote sessions list/create`、`remote prompts list/add` 和交互式 `remote tui`，覆盖 Snaplink RFC 8628 登录、会话分页浏览、历史查看、创建和 Prompt 提交/重试。CLI/TUI 通过同一 coordinator API 工作；`FORGE_ACCESS_TOKEN` 可作显式 override，access token 仍保存在 Unix 权限保护文件；Linux/macOS 通过系统 Keychain/Secret Service 续期，其他 CLI 平台仍不支持安全持久 refresh。Prompt append 仍要求 expected-version 与幂等键，且只写入 Prompt，不创建 Run。远程 CLI/TUI 新建会话目前默认 Global；authenticated API 已支持 Global/Project/Group owner 私有 scope，但 CLI/TUI 仍缺少 scope 选择和远端 scope 浏览。这些仍不代表完整 CLI/TUI 产品旅程已交付。

Flutter Console 在 `/forge/` 增加部分会话界面：列表/创建、分页 Prompt history、Prompt append 与同请求幂等重试；它复用现有 Snaplink 登录并请求 `forge:conversations:read/write` scopes。最新 Console 定向测试 12/12、`flutter analyze` 通过。Rust fmt/strict-Clippy 通过；CLI all-targets 的 288 个单测曾全部通过，最终复跑通过 286 个、跳过先前已通过的两项长压力测试且所有 integration targets 通过；Infrastructure 430 个单测、全部 integration/doc targets 通过。该 Console 切片没有 Run timeline、任务执行、设备库存或调度，也不能代表原生 App/Mobile 或统一多端接受旅程。

聚焦验证当前通过：schema migration suite 124/124；Rust CLI all-targets 最后复跑 286 单测（跳过两项此前同树已通过的长压力测试）及全部集成目标；Rust infrastructure 430 单测及全部 integration/doc targets；fmt/strict Clippy；Go authn/runtimebridge/appserver race tests/vet 与新构建 Runtime 上真实 HTTP→Runtime process→Hub owner A/B E2E；Console 定向测试 12/12 与 analyze。指定 Snaplink 仓库未在本 Sprint 修改或运行测试；代码检查确认其已有通用 RFC 8628 flow 与 `slow_down`，Forge 专用 client provisioning 和 CLI 登录仍未交付。owner-local dense cursor 的独立安全复审未发现必须修复问题。formal same-tree acceptance 尚未运行。最终退出门继续要求适用的 Go 回归、迁移/回滚与 legacy owner-null、身份/越权负例、幂等/CAS、限界页、CORS/TLS、真实进程往返、CLI/TUI API 集成、strict Clippy、architecture/governance gates 与冻结同树 `node harness/acceptance.mjs`。现有 UI 是局部切片；paged feed 仍缺 Run intent/timeline、live delivery、持久化断线 cursor 与完整 Web/App/Mobile 生命周期，设备权威 inventory、placement 或远程执行也未交付。


#### 2026-09-13 continuation — Forge client login and explicit stored-Prompt result

指定 Snaplink 源配置现有 clients.grant_types allowlist，并在 Kubernetes sample 中为 forge-cli 限定 device-code grant，为 forge-console 配置 authorization-code/refresh-token grants；两者使用 forge-api、相同 Forge conversation scopes、subject type 与 tenant。race 集成测试通过两客户端签发并核对 exact issuer/subject/tenant 和 audience；这只验证源代码与 handler，不表示 live deployment 已 reload。

Rust CLI 新增 remote login（RFC 8628）、精确保留 Snaplink issuer 字符串的 claim/account binding、受限 device response、无 redirect 的 TLS HTTP client 与 Unix 0700/0600 凭据文件；只保留 access token，过期需重新登录，未接 OS keychain。跨域验证页仅接受 HTTPS；loopback HTTP 仅当 issuer 本身 loopback HTTP 且同源时接受。Flutter Console 登录使用独立 forge-console client-scoped token slot；TUI 和 Console 成功反馈明确说明 Prompt 已存储但未启动 Run。

本续段验证：Rust remote_command 34/34、remote_args 4/4、全目标严格 Clippy、格式检查；Snaplink owner-parity race test 通过；Console 定向测试 12/12 与 flutter analyze 通过；三个仓库的 git diff --check 通过。一次未过滤的 CLI 单测运行进入现有 agent_run_limits 长压力测试并超过 60 秒，随后停止；不把该运行记为全套通过。Prompt 仍只入库，不创建 Run intent/timeline，也不调度或执行设备任务。没有完成 live deployment、远程 Runner 或设备注册。ADR-0113 仍 Proposed/null，Sprint 156 继续受上方 completion_boundary 约束。

#### 2026-09-13 continuation — separate session scope from execution authorization

只读授权审计确认 Hub `projects` 没有账号 owner/grant，Group project role 只是描述标签；已验证 JWT 本身也不能证明其 subject 是本 Coordinator 的个人账号。因此 session API 配置新增必填 exact tenant 与 subject pin，先在 Go 资源服务器拒绝其他租户/账号，再将允许的 `(issuer, subject, tenant_id)` 交给 Rust owner transaction。继续开放已批准的 Global/Project/Group 会话组织能力，同时把它与执行授权明确分离：Rust 只允许引用 Hub 已存在的 scope；Conversation 列表、历史、Prompt 写入和 feed 仍只返回 exact owner 的记录；scope 不会返回项目路径或授予 Run 权限，Group 成员也不会获得对方会话。没有新增 Run route，也未使用接受调用方 Project/provider/tool/path 的本地 RunStore 作为远程接口。

下一段 Run-intent 的前置条件是 Hub 持久化且显式 consent 的 owner→Project 授权，以及由服务端策略选择的执行 profile；随后才可设计 owner/CAS/idempotency/Prompt/pending-intent/timeline 的原子事务。该片仍不允许 Runner enrollment/dispatch，ADR-0113 保持 Proposed/null。

本续段验证：Rust Hub contract 14/14、Runtime RPC 16/16（含真实 SQLite Hub 中 Project/Group owner 会话创建、私有列表/feed、foreign-owner 404 以及无 path/run 字段）；Go authn/appserver/runtimebridge race tests、forge-server 平台构建测试与 vet 通过。真实 HTTP→Go→Rust→Hub 进程 E2E 用本机 CLI 先创建现有 Project/Group scope，再由配置 pin 的 exact principal 建立 Global/Project/Group 会话、查询、追加 Prompt、读历史/feed，并验证同租户其他 subject 在 API 边界 403 且 feed 不变。workspace all-target strict Clippy、fmt 和根目录 diff check 通过。正式 acceptance 仍需全仓 architecture/governance 与冻结同树 `node harness/acceptance.mjs`；本续段不代表 Sprint 156 或正式验收完成。

**completion_boundary:** 在所有上述验证于同一冻结树通过前，本 Sprint 保持“实现中”；用户对产品路线的批准不能替代技术验收或 ADR 生命周期转换。

#### 2026-09-13 continuation — remote session scopes and device-enrollment boundary

Rust CLI `remote sessions create` 新增 `--scope global|project:ID|group:ID`；`remote sessions list --scope ...` 在已校验的当前服务端页上按精确 scope 过滤，并保留服务端分页 cursor/has_more，使调用者可继续获取后续页。交互 TUI 同样支持 `create --scope ... TITLE`，会在列表中显示 Global/Project/Group scope，且 scope 会随 pending-create 重试与恢复信息一并保留。scope 仍只是会话组织元数据，不授予 Project、Group 或设备执行权限。

设备审计确认 Console Agent Hub 有独立设备/GPU 与任务 UI/API，但其 task 没有 Forge Conversation/Run 绑定；Snaplink workload identity、Aero-ID membership query 和 Audit Governance ingest 可提供后续身份/审计参考，均不构成 Forge device registry 或调度器。当前 ADR-0039 明文禁止 live registration/heartbeat。新增 ADR-0114 Proposed-only 候选，描述默认关闭、设备 key proof、owner approval、sequence/freshness、Go-owned persistence 和只读 inventory 的拟议边界；它不修改 ADR-0039、不授予运行时授权。设备端点、凭证签发、库存持久化与资源调度仍未实现。

本续段聚焦验证：CLI 远程相关测试在后续增量前为 39/39 通过（含 Project scope 创建、Group scope 查询参数解析、当前页过滤、分页 cursor 保留、TUI ambiguous retry 同请求 Project scope）；本机 cursor 的新增验证见下方续段。ADR-0114 通过 Proposed-only v2 structural checker，状态 `proposed`、`accepted_at_unix_ms=null`、`acceptance_id=null`。完整 P0/P1/P2、设备 inventory、Runner 和远程执行仍未完成；本续段不代表 Sprint 156 或正式验收完成。

**completion_boundary:** 设备注册和 heartbeat 仍受 ADR-0039 禁令约束；ADR-0114 的结构有效不等于接受或授权。Sprint 156 保持“实现中”，直到适用测试、独立复核、架构/治理检查与冻结同树 `forge accept` 完成。

#### 2026-09-13 continuation — durable replay checkpoints across CLI/TUI/Flutter

使用 `remote login` 保存的凭据时，CLI 的 `remote changes list` 默认从上次成功读取的位置续传，并在有效页通过校验后更新本机 checkpoint；显式 `--after-cursor` 只影响本次读取，不覆盖 checkpoint。TUI 启动时恢复同一游标，执行 `sync` 后只有在变更页、会话列表和当前选中历史全部读取且校验成功后才提交新位置。检查点按 Coordinator origin、issuer、client、tenant 和 subject 分区，使用独立 `.cursor` 文件、严格 schema、大小限制、Unix 0700/0600、无符号链接读取与原子替换；不会保存访问令牌。显式 `FORGE_ACCESS_TOKEN` 模式不持久化 checkpoint。Flutter Console `/forge/` 现在在 Web localStorage 和原生平台偏好中保存游标；键绑定 API origin、client/resource 与 issuer/tenant/subject 哈希，不保存 token，且必须等相关会话/历史刷新成功才推进。Flutter 历史页还校验 owner、精确 DTO 字段、排序、游标及 256 KiB 内容预算，并允许服务端按字节预算提前结束的非满页。默认客户端仍为轮询/手动 sync；CLI/TUI、Console 另提供显式 opt-in change stream。

验证：Rust `cargo test -p forge-runtime-cli remote_` 51/51，新增覆盖内容预算导致的合法非满 Prompt page；CLI checkpoint、显式游标隔离、TUI history 503 保留游标、FIFO 拒绝均覆盖。Console `flutter test test/forge_change_cursor_store_test.dart test/forge_sessions_widget_test.dart test/forge_conversations_models_test.dart test/forge_conversations_api_test.dart` 19/19，覆盖 native 偏好续传、Coordinator/owner/client/resource 分区、token 不落盘、同 isolate 并发写入串行化、屏幕重建恢复、history 503 不推进、严格 cursor 校验与合法字节预算部分页。Rust strict Clippy、`cargo fmt --all -- --check`、根目录 `git diff --check`、Console 定向 `dart analyze`、Dart format、Console `git diff --check` 与 `flutter build web --no-pub` 均通过；独立复核确认 Flutter 合法部分页和断点提交顺序无阻断问题。本实现不改变服务端 owner 授权或 feed 语义，也不完成完整 P1/P2。

#### 2026-09-13 continuation — explicit legacy local Conversation import

Rust CLI 新增 `remote sessions import LOCAL_CONVERSATION_ID` 两步导入：先从正在运行也可读的本地 Hub 以 live read-only snapshot 读取 ownerless 会话，只展示 user/assistant transcript、来源 scope、Global 目标 scope、Coordinator 与目标 account-claim 预览及 SHA-256 确认摘要；预览阶段不联网上传。确认时重新读取并绑定源元数据、完整 transcript、Coordinator、目标 principal 和 Global scope，内容变化会重显当前预览并拒绝过期确认。空白 legacy Prompt、超过 128 条消息或超过 256 KiB 聚合 UTF-8 内容的来源在预览阶段拒绝。请求只传 title 与 user/assistant Prompt，local ID、路径、Run、tool/provider context 不离开本地；成功后原本地 Conversation 保持不变。

Coordinator Go API 从已验证的 Snaplink principal 派生 owner 并要求 write scope；Runtime RPC 校验严格字段/限额。Rust Hub 用单一事务写入 Global owner-bound Conversation、Prompt 与 owner-local change entries；owner-scoped 幂等键保证重试复用结果，且后续追加 Prompt 不破坏原导入重放。该切片不自动 claim/同步历史、不创建 Run、不调度设备，也不解决更广泛的导入选择/脱敏策略；Sprint 156、P1 与 ADR-0113 lifecycle 仍开放，ADR-0039 的设备注册/执行边界不变。

本续段验证：Rust application contract 14/14、SQLite import 5/5、CLI/RPC `import` 定向 12/12；Go HTTP→Rust Runtime→Hub 真进程 import 集成通过，Go appserver/runtimebridge 定向测试与 vet 通过；workspace strict Clippy、fmt 与根目录 diff check 通过。未完成 CLI 全量回归（长 stress case 已停止）、`go test ./...` 全量回归或正式同树 `forge accept`，不将本续段记为 Sprint 156 / P1 完成。设计边界记录于 [cross-device implementation plan](../docs/design/ai-engineering-os/cross-device-session-and-fabric-plan.md) §19。

#### 2026-09-13 continuation — owner-scoped read-only Run observation

Rust Hub 新增 owner-filtered Run summary 与 timeline metadata page；Go 增加需要 `forge:conversations:read` 的两个只读 API；CLI 增加 `remote runs list` / `remote runs timeline`，Flutter Forge Console 增加同一只读 Runs 面板，支持摘要和事件页续读。summary 只包含 Run/Prompt ID、创建时间、最新序号和封闭状态；timeline 只返回序号、时间戳及 `run_started`、`turn_started`、`activity`、`run_finished` 四类之一。assistant、tool、error 事件统一折叠为 `activity`，不返回消息正文、工具信息、运行配置、路径、幂等键或错误详情。foreign/missing Conversation 或 Run 对请求方统一表现为 not found。

SQLite owner check 和查询位于同一个 deferred transaction。Run 页上限 25，timeline 上限 128；两者分别限制 2 MiB 事件源字节预算，使用 BLOB 字节长度扫描骨架、逐条读入事件，并允许返回非满的 `has_more` 页供游标续读。timeline 强制密集序号；无 `run_started` 的损坏 Run 失败关闭。该 observer 只查看 Hub 中已经存在的 Run；既不创建 Run，也不把 Prompt 变成执行请求。它没有推送、Run 内容、Project 执行授权、设备注册、设备调度或 Runner 执行。

验证：SQLite owner Run 查询 5/5、Application 校验 1/1、Runtime RPC 2/2；CLI 相关定向测试 13/13；Runtime domain/application/infrastructure/CLI all-target strict Clippy `-D warnings`、workspace fmt 与 diff check 通过。Go `internal/runtimebridge` / `internal/appserver` race tests 与 vet 通过；配置 Runtime binary/state dir 的 HTTP→Go→Rust→SQLite E2E 通过。Console Runs 模型/API/widget 定向组 24 项通过，Forge widget regression 5 项通过；本次复跑 Runs API/widget 测试 4 项通过，`flutter analyze` 与 `flutter build web --no-pub` 通过，Console diff check 通过。没有运行全仓 `go test ./...`、全 workspace Rust test suite 或正式同树 `forge accept`，本续段不代表 Sprint 156、P1 或 ADR-0113 完成。

**completion_boundary:** Prompt 仍只写入 Hub；此 Run observer 不代表任务正在运行。ADR-0113 保持 Proposed/null；ADR-0039 对实时设备注册、心跳、扫描和远程 Runner 执行的限制不变。本续段只完成脱敏的 owner-scoped Run 观察切片，完整 Run-intent/timeline、设备治理与调度仍未完成。设计边界记录于 [cross-device implementation plan](../docs/design/ai-engineering-os/cross-device-session-and-fabric-plan.md) §20。

#### 2026-09-13 continuation — inert Project execution consent foundation

Rust Hub schema v33 adds a time-bounded execution-consent record keyed by the exact verified owner tuple and Project, bound to an opaque server-selected profile ID plus SHA-256 digest. It stores grant and revocation as append-only events, allows one active unexpired grant per exact owner/Project, and makes grant/revoke writes transactional and idempotent. The grant result is a receipt for the original operation; replay does not assert current consent. Runtime RPC v2 exposes only the trusted local Hub contract. No HTTP consent route, Run intent, provider, worker, device, reservation, or dispatch was added.

The next pending-intent transaction must derive Project from the stored Project-scoped Conversation, require exact owner/profile consent that is still unexpired and unrevoked in the same immediate transaction, and atomically store Prompt + pending intent + a `submitted` timeline marker. It must not create ordinary `runs`/`run_events` or emit `run_started`. Public Coordinator APIs must select profile ID/digest from server-owned policy and authorize Project independently; neither value is caller-selected.

Focused evidence on the resulting v33 tree: infrastructure consent atomicity 2/2; v32→v33 migration and final-validation rollback 2/2; application owner/replay/revoke/expiry tests 2/2; Runtime RPC consent tests 2/2 and all Runtime RPC tests 23/23; pure device_registry placement tests 11/11. `cargo fmt --all -- --check` and targeted all-target strict Clippy (`forge-runtime-domain`, `forge-runtime-application`, `forge-runtime-infrastructure`, `forge-runtime-cli`, `-D warnings`) pass. An independent read-only review found no blocker and confirmed the current owner checks, replay scoping, active-grant concurrency gate, composite foreign keys, and migration contract; it recorded the existing trust assumption that only authenticated Go constructs the RPC owner.

The offline Go placement dry-run and Rust placement-policy parity remain declaration-only; Flutter Android APK build is verified, while iOS and real-device behavior remain unverified. Aero integrations remain future downstream seams: Snaplink owns identity, Hub owns session/intent truth, IM may carry status links, Vault may store authorized artifact bytes, and Audit Governance may receive minimized facts from a Hub-owned outbox. No external system becomes canonical Run state.

**completion_boundary:** ADR-0113 remains Proposed/null and ADR-0039 still blocks live device enrollment/heartbeat/discovery/dispatch. This consent foundation alone does not complete P1/P3 or the multi-client Prompt→intent journey. Keep Sprint 156 in progress until an HTTP path uses the profile catalog and an authorized client consent flow exists, client observation is implemented, and same-tree acceptance plus applicable architecture/governance checks pass. Design details are in [cross-device implementation plan](../docs/design/ai-engineering-os/cross-device-session-and-fabric-plan.md) §§21–22.

#### 2026-09-13 continuation — inert atomic pending Run intent

Rust Hub schema v34 now stores an owner-bound pending Run intent separately from ordinary Runs. A new submission derives Project only from the stored owner-bound Project Conversation and, within one immediate transaction, checks exact aggregate CAS and an unexpired, unrevoked consent grant matching the exact owner, Project, server-selected profile ID, and digest. It atomically appends the user Prompt through the existing change-journal path, the inert intent, and one payload-free `submitted` timeline event. It creates no `runs` or `run_events`. Global, Group, ownerless, and foreign Conversations are rejected or hidden as specified.

Idempotent replay returns the original Prompt/intent/event receipt before current CAS, grant, or profile checks; it creates no second write and never reports present-day consent. Reusing the key with changed Conversation or content conflicts. Owner-scoped intent pages and timelines remain separate from Run observation and omit Prompt body/profile digest. Go `internal/runtimebridge` has strict v2 consent grant/revoke, submit/page/timeline contracts, but AppServer routes and `/prompts` behavior are unchanged; no public intent/consent route exists; the private owner-filtered Project lookup/profile catalog are loaded from startup policy but are not used by an HTTP route.

Independent validation on the resulting tree: infrastructure pending-intent atomicity 6/6; v33→v34 migration/rollback 2/2; fresh/legacy schema validation and reopen 1/1; Application replay/consent test 1/1; Runtime RPC tests 2/2; four-crate all-target strict Clippy; workspace fmt and diff checks; Go runtimebridge race tests/vet; and a real Go→Rust subprocess test with a temporary Hub covering consent grant/revocation, original receipt replay after profile/CAS changes, owner isolation, and no Run creation. No full workspace tests or formal `forge accept` were run.

**completion_boundary:** this is a durable intent, not an execution request or active Run. ADR-0113 remains Proposed/null; ADR-0039 continues to prohibit live device enrollment, heartbeat, discovery, and remote Runner dispatch. Sprint 156 remains in progress pending the server-owned profile/consent UX, full client observation, applicable governance reviews, and frozen same-tree acceptance. See [cross-device implementation plan](../docs/design/ai-engineering-os/cross-device-session-and-fabric-plan.md) §23.

#### 2026-09-13 continuation — storage-only Prompt HTTP and server-owned profile policy

Go `internal/runtimebridge` now wraps Hub v33 Project consent grant/revoke in strict Runtime v2 request/response contracts. The real Go→Rust pending-intent subprocess test uses those methods for grant and revocation; callers still must provide trusted owner/profile inputs, and no HTTP handler can call them. Runtime v2 also returns only `conversation_id`/`project_id` for an exact-owner Project-scoped Conversation. Missing/foreign/ownerless records are uniform `not_found`; owner-owned Global/Group records return `conflict` without a Project ID.

The Go profile catalog accepts a bounded, duplicate-free `forge-server --execution-profile-binding` startup policy and fails closed for unknown Project IDs. It obtains the Project only from the Hub identity operation, verifies the conversation ID echo and strict response shape, and does not grant consent or execute anything. Current HTTP routes do not call the catalog.

The existing authenticated Prompt endpoint has an isolated temporary-Hub HTTP→Go→Rust regression for new append, exact replay after later writes, changed-body conflict, stale expected-version conflict, exact change-feed/history write count, and an empty Run page. Intent/consent HTTP paths remain unregistered and return `404`. Two Rust RPC tests, real Go→Rust profile-resolution integration, all-target strict Runtime Clippy, and `go test -race`/`go vet` across executionprofile, appserver, runtimebridge, and forge-server pass. This does not constitute full Go or repository acceptance.

**completion_boundary:** `/prompts` remains storage-only. Project consent and pending-intent operations are private subprocess capabilities; the profile catalog still lacks HTTP integration and a public consent flow; intent submission/observation UI, live device registry, scheduler, and Runner dispatch also remain absent. ADR-0113/0114 remain Proposed/null, ADR-0039 remains in force, and Sprint 156 stays in progress.

#### 2026-09-13 continuation — private HTTP candidate for consent and inert intent

App Server now contains an unregistered HTTP handler candidate for reading the server-selected Project profile preview, explicitly granting/revoking owner-to-Project consent, and submitting/listing/timeline-reading inert pending Run intents. The preview derives Project from an exact owner-filtered Hub identity operation and selects profile ID/digest only from the immutable startup catalog. Grant requires `confirm_execution_profile=true`, expiry, and idempotency; submit accepts only Prompt text, expected Conversation version, and idempotency. Runtime Hub rechecks owner, Project, active consent, profile digest, and CAS atomically. Profile IDs/digests supplied by a client are rejected.

The ordinary App Server constructor and `Run` remain unchanged, so the real production surface still returns `404` for these candidate paths. A direct package-level handler test with a fresh temporary Hub verified profile selection, Global-scope rejection, read/write scope enforcement, unknown-field rejection, exact grant/submit replay, bounded sanitized intent/timeline reads, revocation blocking new submission, original receipt replay after revocation, foreign-owner denial, and an empty ordinary Run list. Go appserver/executionprofile/runtimebridge race tests, vet, and the real Go→Rust integration pass.

**completion_boundary:** this is an unexposed server candidate, not a public consent flow or completed client journey. Flutter Console and Rust CLI/TUI still need consent preview/confirmation and pending-intent views. ADR-0113 remains Proposed/null; its applicable lifecycle and same-tree acceptance gates must pass before route registration. ADR-0039 continues to prohibit live enrollment, heartbeat, discovery, scheduling, and remote Runner execution. Sprint 156 remains in progress; design details are in [cross-device implementation plan](../docs/design/ai-engineering-os/cross-device-session-and-fabric-plan.md) §24.

#### 2026-09-13 continuation — CLI/TUI older Prompt history pages

CLI `remote prompts list CONVERSATION_ID` now accepts the paired `--before-created-at-ms` and `--before-prompt-id` cursor fields. TUI `older` loads the next earlier page for the selected session after `open`; it retains the cursor after a failed fetch and resets it when selection changes or history is refreshed. The response validator rejects rows at or newer than the requested keyset cursor, preventing duplicate boundary rows or server pages that move forward.

Validation: Rust remote command tests 62/62, Prompt argument tests 3/3, and the TUI older-page HTTP fixture 1/1 pass. `cargo clippy -p forge-runtime-cli --all-targets -- -D warnings`, workspace formatting, and `git diff --check` pass. The unfiltered CLI package suite was interrupted at an existing long-running Agent stress test after the relevant client tests had passed; this does not count as a full package pass. P2 remains partial, and this read-only history increment does not expose Run intents or change the ADR-0039 device boundary.

#### 2026-09-13 continuation — contract-only Forge Prompt audit projection

Forge Core now projects one authenticated owner-scoped Hub `prompt_appended` change into a closed `forge.prompt.accepted.v1` event. The projector accepts only the owner tuple supplied by the authenticated caller and the ID-only committed change DTO; it checks their shape but cannot independently verify the token or Hub authorization. The audit actor ID is a stable SHA-256 pseudonym over issuer, subject, and tenant, so the raw subject is not exported and principals from different issuers remain distinct. The strict JSON Schema and golden fixture pin that identity rule and the sole `content_included=false` payload field.

The real HTTP→Rust Hub integration now projects the authenticated owner change feed and verifies deterministic replay, event-to-Conversation/Prompt/version binding, and absence of Prompt text, request idempotency keys, token fields, or content fields. Audit Governance model review found the event fields, time range, idempotency behavior, and payload shape compatible. The projector also rejects tenant, aggregate, or operation identifiers beyond Audit Governance's 85-byte archive bound or containing whitespace, path separators, or stream delimiters. A future source registration must bind `forge-runtime` to the approved producer client and register this exact schema with only `content_included` allowed. No source registration, outbox, delivery cursor, acknowledgement, retry worker, or external call was added.

Validation: `go test ./internal/auditprojection`, Go vet, and `git diff --check` pass. The configured `FORGE_RUNTIME_BIN=... go test -race ./internal/appserver` suite passes against the real Rust Hub subprocess, including the new authenticated change-feed projection check. Rust workspace formatting also passes. ADR-0039 and the current lifecycle boundary remain unchanged.

#### 2026-09-13 continuation — Flutter Console automatic Run observation

The existing 15-second Forge change-sync now performs bounded read-only polling of the selected Conversation's Run summary page and the selected Run's timeline after its last validated sequence. A first Run is selected if none exists; an existing selection survives refreshes, and event sequence is committed only after exact-page validation. A bad page keeps the prior sequence so the next poll retries it. Run polling continues even when the Conversation change page is empty or a separate Prompt/session refresh fails; the conversation-feed checkpoint retains its existing success rules. All polling requests use authenticated GET endpoints and presentation remains metadata-only.

Validation: `forge_runs_widget_test.dart`, `forge_sessions_widget_test.dart`, and `forge_change_cursor_store_test.dart` pass (10/10); the new widget case verifies updated status, incremental timeline cursor, malformed-page retry, and bearer-authenticated GET-only traffic. Targeted Dart analysis, `flutter build web --no-pub`, and Console `git diff --check` pass. This is status observation only; no Run-start, consent, device registration, dispatch, or outbound delivery was added. P2 remains partial and ADR-0039 remains in force.

#### 2026-09-13 continuation — implementation phase gate alignment

The cross-device delivery table now reflects implemented semantics: `/prompts` stores conversation input, while Run observation reads only existing Runs; neither implies task start. Device work is split into P3a caller-supplied offline declarations and P3b live owner-scoped inventory. P3b remains unimplementable/unexposable until the applicable architecture/security decision is formally Accepted; P4 remote execution also requires a separate accepted execution decision. This is documentation alignment only and does not alter any ADR status or authorize new device effects.

#### 2026-09-13 continuation — real Rust CLI through Coordinator E2E

The real multi-client integration now runs built `forge-runtime` CLI subprocesses against the same temporary Go HTTP Coordinator and Rust Hub already used by the Go A/B fixture. CLI principal A creates a Conversation; independent principal B lists it and appends a Prompt; A reads its Prompt history and owner-local change feed. A third signed principal C uses the same issuer/audience/tenant and valid read/write scopes but a different subject; it passes App Server authentication, receives an empty owner list and uniform 404 for the foreign Conversation, proving the Rust Hub exact-owner boundary is exercised. The test verifies the resulting Hub Run page and pending Run-intent page remain empty.

The HTTP request recorder allows only the eight expected session/Prompt/change/Run-read calls and pins their query strings; no Run-intent, device, placement, or dispatch path is allowed. CLI subprocesses use explicit `FORGE_API_URL` / `FORGE_ACCESS_TOKEN`, an isolated temporary HOME/XDG location, a minimal system environment, 30-second timeout, and 2 MiB output caps. This is real CLI→Go→Rust integration evidence, not a Flutter live-server journey or full P2 acceptance.

Validation: focused E2E and its race-enabled run pass; `FORGE_RUNTIME_BIN=... go test ./internal/appserver -count=1` passes on the current tree; `go vet ./internal/appserver`, gofmt, and `git diff --check` pass. No production files changed.

#### 2026-09-13 continuation — shared Conversation page contract and Snaplink device-grant fix

Added one canonical owner-scoped Conversation page fixture consumed by Go Runtime bridge, Rust CLI, and Flutter Forge Console tests. `scripts/test-forge-contracts.sh` resolves the fixture from its own repository root and supplies its absolute path to all three test suites. Flutter list parsing now receives the actual requested page limit and `after_id`, rejects unknown response fields, enforces valid scope/ID shapes and strict ID order, and requires `has_more` and `next_after_id` to agree. Rust applies the same closed response shape to page/entry fields and validates each nested Conversation projection; Go's fixture decoder rejects unknown fields.

Snaplink Device Grant now honors an explicit client `grant_types` allowlist before issuing refresh tokens. A device-only profile receives no refresh token and refresh exchange remains rejected; explicit `refresh_token` permission and the legacy empty-allowlist behavior retain issuance and rotation. The Forge CLI profile is still device-code-only, so this change does not add CLI auto-refresh or change live Snaplink configuration.

Validation: `scripts/test-forge-contracts.sh`, Flutter Console model/API tests and Dart analysis, Rust CLI strict Clippy/fmt, and the race-enabled Rust CLI→Go→Hub ownership E2E pass. Snaplink `TestDevice_*`, Go build/vet, and diff check pass. This closes contract parity for the Conversation list DTO and an OAuth grant mismatch; P2 is still partial and ADR-0039 / ADR-0114 live device gates remain unchanged. Details are in [cross-device implementation plan](../docs/design/ai-engineering-os/cross-device-session-and-fabric-plan.md) §29.

#### 2026-09-13 continuation — Forge Console refresh-token rotation

Forge login now retains the Snaplink refresh token in the `forge-console` client slot. After Forge returns 401, Console uses the Snaplink public refresh grant, merges concurrent rotation attempts, stores the rotated pair, and retries the API request once with the same body and idempotency key. Failed rotation clears only the Forge slot; known-rejected tokens are not sent again. This keeps browser credentials in tab-scoped session storage and native credentials in process memory.

Validation: refresh service, API replay, session storage, explicit Forge login, Forge widget and route tests all pass (25/25); targeted Dart analysis and Console `git diff --check` pass. This does not add a Run-start path or change device enrollment/execution gates.

#### 2026-09-13 continuation — shared offline placement policy fixture

Added `forge-device-placement-policy-parity-v1.json` and test-only Go/Rust adapters for the shared OS/architecture/CPU/memory/storage/runtime/residency/trust/sandbox/concurrency comparison. Identity tuple, unknown-state, heartbeat/lease, and GPU differences remain language-specific; no production API or dispatch contract was added. The Go scanner now also rejects JSON null values instead of silently interpreting null primitives as zero values.

Validation: `scripts/test-forge-contracts.sh` passes all Go, Rust, and Flutter fixture suites; `go test ./internal/deviceplacement`, its `go vet`, Rust placement fixture test, domain strict Clippy/fmt, and repository diff checks pass. ADR-0039 is unchanged and ADR-0114 remains Proposed/null. See [parity fixture contract](../docs/contracts/forge-device-placement-policy-parity-v1.md) and plan §§30–31.

#### 2026-09-13 continuation — persistent TUI Prompt transcript

The Forge TUI now keeps the selected Conversation's validated Prompt pages in local view state. `open`, `older`, and `sync` update one deduplicated, chronological transcript; successful Prompt writes refresh it immediately. A newest-page refresh preserves the cursor for the oldest loaded row. A failed read after an accepted write keeps the write confirmed and the previous transcript visible. Failed session/history refresh during `sync` leaves the persisted change cursor unchanged and returns to the interactive loop. The change feed remains manual.

Validation: `cargo test -p forge-runtime-cli remote_command::tui` passes 14/14; strict all-target CLI Clippy and Rust workspace formatting pass. The plan records this as §32. This remains session/Prompt observation only; Prompt submission still does not create a Run, and ADR-0039 / Proposed ADR-0114 device gates are unchanged.

#### 2026-09-13 continuation — Flutter Console API through Coordinator E2E

The real multi-client test now launches Flutter tests that use Console's production `ForgeConversationsApi`, Forge-scoped token slot, `ForgeSessionsGate`, and `ForgeSessionsScreen` against the same temporary Go Coordinator and Rust Hub as the Go and CLI clients. The service lists the shared session, reads another client's Prompt, appends its own Prompt, and reads the owner change feed; the gate loads the screen from the Forge client slot and it renders that session/history and submits another Prompt through its UI. Go reads both Console Prompts back from the Hub. A request allowlist pins the API and widget calls, permitting only a metadata-only Run GET and rejecting Run writes/device paths. The test token stays in a mode-0600 temporary input file and is not passed in process arguments; the loopback Coordinator origin is passed as a compile-time define.

Validation: `scripts/test-forge-shared-session-e2e.sh` builds the Rust CLI and passes the real Flutter gate/widget/API-service → Go → Rust integration; the same integration passes under `go test -race`, and focused Dart analysis is clean. This validates the Forge gate and screen widget, not browser deployment or native/Web/Mobile OAuth journeys. No Run or pending intent is started; no device registration, inventory, placement, or dispatch occurs. ADR-0039 and Proposed ADR-0114 remain unchanged; details are in [cross-device implementation plan](../docs/design/ai-engineering-os/cross-device-session-and-fabric-plan.md) §33.

#### 2026-09-13 continuation — default-off device route regression and Roadmap parity

Updated the P2 Roadmap evidence to include the Flutter API-service, Forge client token slot, `ForgeSessionsGate`, and `ForgeSessionsScreen` live E2E without claiming browser deployment or Web/App/Mobile OAuth completion. Added a route-level test for representative device inventory, enrollment, and heartbeat paths; the configured session Coordinator returns the fixed `404 not_found` response for each while no device handler is registered. This records the default-off boundary and gives future P3b wiring a regression point.

Validation: the focused App Server device-route gate test, App Server vet, repository diff checks, and the race-enabled multi-client Flutter/CLI/Go/Rust integration pass. This does not authorize or implement device credentials, persistence, heartbeat, inventory, or placement; ADR-0114 remains Proposed/null, ADR-0039 remains planning-only, and P4 still needs a separate accepted execution/security decision. See [cross-device implementation plan](../docs/design/ai-engineering-os/cross-device-session-and-fabric-plan.md) §34.

#### 2026-09-13 continuation — real Forge TUI through Coordinator PTY E2E

The shared multi-client integration now also launches the actual `forge-runtime remote tui` process after the Flutter and non-interactive CLI clients. A util-linux PTY feeds `open <shared-id>`, `prompt ...`, and `quit`; the test checks that the terminal renders the existing shared session and Prompt, reports the new Prompt stored without starting a Run, and records exactly four Coordinator calls: session list, Prompt history, Prompt append, and post-write history refresh. Go then reads all four shared Prompts back from the Hub. This exercises the scripted TUI process path under a PTY, not manual keyboard UX or browser/native/mobile deployment.

Validation: `scripts/test-forge-shared-session-e2e.sh` passes with the real TUI PTY flow; the matching multi-client integration passes under Go's race detector, the default-off device route regression passes, App Server vet is clean, and focused Dart analysis is clean. The PTY harness requires util-linux `script`. No Run or pending intent is started, and no device API, inventory, placement, or dispatch is exercised; ADR-0039 and Proposed ADR-0114 remain unchanged. See [cross-device implementation plan](../docs/design/ai-engineering-os/cross-device-session-and-fabric-plan.md) §35.

#### 2026-09-13 continuation — Forge Console real-browser Web E2E

Added an opt-in browser pass to the shared multi-client integration. The harness builds the actual Flutter Web bundle with `/forge/` as its base path and serves it from the same temporary loopback origin as the authenticated Coordinator. Python Playwright opens that route in Chromium, installs a signed test token only in the Forge-scoped tab `sessionStorage`, selects the session created by another client, reads its Prompt history and submits a new Prompt. The browser verifies a successful Prompt `201`; Go then reads the exact appended row from the Rust Hub. The recorder permits session list/history reads, Prompt append, owner change-feed reads, and a metadata-only Run page; device, scheduling, and Run-write paths fail the allowlist.

Validation: the browser-enabled `scripts/test-forge-shared-session-e2e.sh` and the same integration under `go test -race` pass; the focused default-off device-route test and App Server vet pass; Dart analysis, Go formatting, shell syntax, and Python compilation checks are clean. Set `FORGE_BROWSER_E2E=1` to build and run this optional harness; it requires Python Playwright and Chromium. The bundle is served by a test-only same-origin wrapper, so this verifies the browser route/storage/API journey without asserting production proxy deployment, cross-origin CORS, or live OAuth. No Run or pending intent is created; ADR-0039 and Proposed ADR-0114 remain unchanged. See [cross-device implementation plan](../docs/design/ai-engineering-os/cross-device-session-and-fabric-plan.md) §36.

#### 2026-09-13 continuation — native foreground refresh for Forge sessions

The shared Flutter Forge screen now pauses its 15-second change-feed poll whenever the app leaves the resumed lifecycle state. Returning to the foreground immediately synchronizes owner changes and refreshes the selected session list/history, so work submitted from another client is visible without waiting for the timer. The owner-local cursor still advances only after required reads succeed.

Validation: the widget test simulates Flutter's valid inactive/hidden/paused/resumed sequence, advances 30 seconds while paused to prove polling stops, and verifies the owner feed plus changed session title after resume. Forge session/native-route widget tests pass, focused Dart analysis is clean, and `flutter build apk --debug` passes. This is a widget lifecycle regression, not physical-device or native OAuth evidence. No Run write, device inventory, scheduling, or execution is added; ADR-0039 and Proposed ADR-0114 remain unchanged. See [cross-device implementation plan](../docs/design/ai-engineering-os/cross-device-session-and-fabric-plan.md) §37.

#### 2026-09-13 continuation — Forge device-scoped sign-out

Added a “Sign out of Forge on this device” action that removes only the Forge Console client credential slot and returns through the Forge-specific Snaplink login request. The Admin Console slot remains intact, and the local owner change cursor remains available for a later same-owner sign-in. The action makes no server revocation request and does not claim to invalidate an issued token.

Validation: the native-route widget test checks that access, refresh, and session values for Forge are cleared, the Admin token remains, and the login route retains the Forge client. Focused Dart analysis and formatting pass. Service-side revocation, live delivery, device inventory, and task execution remain open; see [cross-device implementation plan](../docs/design/ai-engineering-os/cross-device-session-and-fabric-plan.md) §38.

#### 2026-09-13 continuation — Snaplink-backed Forge sign-out and token status checks

Forge sign-out now serializes with refresh rotation, sends separate form revocations for the freshest access and refresh tokens, then clears only the Forge Console credential slot. Snaplink accepts public `none` authentication only on `/token/revoke`, binds access and refresh tokens to the exact registered client, enforces each endpoint's registered client-authentication method, and keeps `/token/introspect` confidential-client-only. Unknown, expired, foreign-client, and already-consumed tokens preserve uniform no-op responses. A recognized active refresh token revokes its rotation family; Redis records an atomic family tombstone before cleanup so surviving siblings cannot be consumed and later descendants cannot be issued. Leftover storage keys retain their finite original TTL if physical cleanup fails. Revocation checks token authenticity while allowing suspended users to revoke; operational issuer/store failures return `503` with `Retry-After`. Introspection projects `tenant_id` only from verified token claims.

Forge App Server also has a default-off online introspection mode that checks Snaplink on every authenticated API request, requires same-origin HTTPS and a dedicated confidential backend client secret file, and fails closed without JWKS fallback or response caching. The Console still completes local sign-out when either remote revocation fails, so there is no durable retry queue. Snaplink's full `./test` race package and affected Redis/OAuth/SSO race suites pass after aligning stale test clients with their registered Basic, post, or `private_key_jwt` request methods; `go build ./...`, `go vet ./...`, the focused empty/duplicate-secret regressions, Discovery contract test, and five existing E2E tests pass. Console token-refresh/sign-out and native-route tests pass (9/9), `flutter analyze` is clean, Forge authn/App Server race tests and vet pass, and the shared Flutter/CLI/TUI → Go → Rust E2E plus repository diff checks pass.

The full Snaplink repository acceptance gates remain incomplete: all-package `go test -race ./...` stalled in `TestOpenAPIRoutesRegistered` while docscheck traversed ignored `.pi-batch/worktrees`; `make ci` stops at formatting errors inside that ignored tree; and the architecture check reports the existing `docs` fan-out limit (16 subdirectories, maximum 15). These checks did not modify the ignored worktrees. No production credentials, physical Mobile device, live device inventory, scheduling, or Runner execution were tested or added. ADR-0039 remains in force and ADR-0114 remains Proposed/null; P1/P2 remain partial. See [cross-device implementation plan](../docs/design/ai-engineering-os/cross-device-session-and-fabric-plan.md) §39.

#### 2026-09-13 continuation — local-only Runtime process Execution Fabric ABI v1

The existing Runtime `exec_command` call now enters a local-only `ExecutionTarget` adapter through a versioned in-memory `ExecutionAttempt`. The attempt is correlated to the already-emitted `ToolStarted` sequence, requires the `current_runtime_only` `local` alias, declares `Process` as potentially side-effecting and mobility as pinned, and carries explicit `environment_digest: not_captured` plus empty ArtifactRef arrays. This records the earlier persistence slice; the bounded environment capture is now delivered in the 2026-09-16 continuation below. The AttemptRef represents one local tool invocation only; it is not a cross-instance identity, retry generation, or fencing ID. ArtifactRef is declared content-addressed metadata, not proof that a CAS object exists or matches; the local adapter rejects nonempty ArtifactRef arrays. A local process Evidence value records the captured exit code/output size and is explicitly only an in-process observation; the AgentTool output/event schema cannot carry it, so the adapter currently drops it and does not persist it. The existing ToolStarted/ToolFinished journal remains the only persisted lifecycle; the AgentTool output/event schema, direct argv dispatch, workspace anchoring, environment filter, output bounds, cancellation and uncertain-effect handling are unchanged. No remote target variant, registry, transport, scheduler, or connection code is loaded.

Validation: the ABI v1 fixture covers target, attempt, digest state, effect/mobility, Evidence and ArtifactRef shape; domain contract tests pass (4), exec-command behavior/boundary tests pass (20), and the application coding-agent integration passes (1), including the unchanged ToolStarted/ToolFinished events. This closes only the process-call adapter slice. Other Runtime operations do not yet route through Fabric; the generated Evidence is not persisted, environment is not fingerprinted, and workspace changes are not captured as CAS artifacts. Therefore ADR-0039 §14 step 0 remains open, as do device inventory, resource scheduling and remote execution under their separate ADR/security gates.

#### 2026-09-13 continuation — bounded CLI session pagination

`remote sessions list --all` now follows validated owner-scoped Conversation cursors for at most 64 pages (8,192 sessions), applies an optional exact scope filter across all scanned pages, and returns the last cursor plus `has_more` so the caller can continue. Default listing remains one page; paged reads do not claim a frozen snapshot.

Validation: parser coverage verifies `--all` and duplicate rejection; HTTP client coverage verifies a second-page scope match, the continuation request, terminal cursor, and the 64-page cap. `cargo test -p forge-runtime-cli session_list` passes (5 tests); strict all-target CLI Clippy and workspace formatting pass. This changes no API, token grant, Run, device, scheduling, or execution boundary.

Broader test attempt: `cargo test -p forge-runtime-cli` passed all 366 unit tests, then stopped at the unrelated `cli_governance_record_journal::reads_refuse_v24_without_migration_but_append_migrates_it_to_current` integration test (`Hub v24 main catalog has invalid object inventory`). That migration fixture and command path were not changed in this session; the session-list-focused tests, formatting, and strict CLI Clippy pass.


#### 2026-09-13 continuation — persist local process observation evidence

`ExecCommandTool` now returns its typed local `ExecutionEvidence` through a new additive AgentTool result method; tools without evidence retain the existing output-only method and default to `None`. The existing `ToolFinished` event carries the optional value, omitting the field when absent. Older event JSON still decodes unchanged, and SQLite persists the evidence in the same Run event row with no new sequence or schema migration. The journal accepts it only when ABI/source, session/run, adjacent `ToolStarted` sequence, `exec_command`, and the local target match. Recovery still commits the Tool message and does not rerun the completed effect.

Validation: full domain suite passes; the real coding-agent `exec_command` integration verifies the captured sequence/identity/local target/exit code; SQLite append-readback verifies the evidence survives and recovery remains `CommitToolMessage`; all workspace tests compile with `cargo test --workspace --no-run`; strict library/test-target Clippy, formatting, and the 22-test Attempt-boundary suite pass. Full all-target Clippy still reports five `needless_borrow` lints in the existing `sqlite_hub/owned_run_read.rs`.

The boundary scanner now explicitly permits named imports from the already-published pure `execution::fabric` leaf, while rejecting root aliases/globs, Fabric aliases/globs, lifecycle imports, and extra module declarations. This only corrects the whitelist for the existing Fabric module; it does not admit Attempt lifecycle or remote execution. Evidence remains a local process observation and is not included in the owned remote Run timeline projection. Environment capture, CAS artifact refs, inventory, scheduling, and dispatch remain open; ADR-0039 is unchanged and ADR-0114 remains Proposed/null.

#### 2026-09-13 continuation — offline placement state and GPU parity

Expanded the shared Go/Rust caller-declaration fixtures with pending/revoked approval, cordoned/offline state, future and stale snapshot timestamps, expired leases, the exact freshness boundary, and a separate required-GPU/memory comparison. Test adapters map implementation-specific offline/heartbeat/GPU reason labels to the shared vector; this remains test-only and defines no live inventory contract. Go still asserts that all candidate attributes are unverified and execution/reservation/dispatch are false.

Validation: `go test ./internal/deviceplacement`, its `go vet`, Rust `placement_parity` tests, strict changed-target checks, and `scripts/test-forge-contracts.sh` pass. The fixtures do not authenticate or discover devices, select targets, or reserve/dispatch work. ADR-0039 remains unchanged and ADR-0114 remains Proposed/null; details are in [cross-device implementation plan](../docs/design/ai-engineering-os/cross-device-session-and-fabric-plan.md) §42.

#### 2026-09-13 continuation — TUI session scope filter

`forge-runtime remote tui` now supports `filter global|project:ID|group:ID` and `filter clear`. This is client-side list rendering only; it preserves the raw server cursor so `next` still reaches a match beyond an empty filtered page. Opening an already loaded but filtered-out session remains available, and the UI labels the filter as organization-only rather than authorization or device identity.

Validation: the TUI tests cover exact scope/ID matches, an empty 128-row first page followed by a second-page match using the unchanged `after_id`, clear-and-reveal behavior, and opening a selected Conversation outside the filter. `cargo test -p forge-runtime-cli remote_tui` passes (4 tests); strict CLI Clippy, workspace formatting, and diff checks pass. P2 remains partial; details are in [cross-device implementation plan](../docs/design/ai-engineering-os/cross-device-session-and-fabric-plan.md) §43.

#### 2026-09-14 continuation — CLI/TUI refresh-token persistence and rotation

Saved Forge credentials now refresh through Snaplink's public `refresh_token` grant in CLI commands and long-lived TUI requests. Linux Secret Service/macOS Keychain hold the refresh token; the access credential remains in the existing private Unix file. Refresh happens within 60 seconds of expiry, keyring work runs on Tokio's blocking pool, and no API write is replayed after `401`. A per-account, validated Unix `flock` serializes login replacement and token rotation across separate CLI/TUI processes; each waiter rereads the current credential before deciding to exchange. The new refresh token is verified before the access-token file is replaced, and `invalid_grant` clears the old keyring value.

The Snaplink distributed profile now allows `refresh_token` for `forge-cli` and configures a 5-second, Redis-backed rotation grace for immediate ambiguous-response retries across replicas. This profile change is source-only, not deployed; the grace does not recover a lost response after five seconds. A failed credential-file write during re-login restores the prior refresh token. Linux requires a usable D-Bus Secret Service. Persistent CLI/TUI login is currently supported only on Linux/macOS; unsupported targets fail closed and can use an explicit `FORGE_ACCESS_TOKEN` override without refresh.

Validation: CLI client-auth tests (3), credential tests (8), and device-login tests (8) pass; `cargo check -p forge-runtime-cli --all-targets` passes; Snaplink's distributed-profile loader test passes. These tests use local fixtures, not live Snaplink or an OS keychain. No physical-device check was performed. Web/App/Mobile lifecycle and delivery remain incomplete, and the existing authorization gates still prohibit live device enrollment, inventory, scheduling, and dispatch. See plan §45.

#### 2026-09-14 continuation — Flutter desktop Forge credential persistence

Extended the Console's existing versioned secure Forge credential record and cold-start restore path from Android/iOS to Linux/macOS/Windows. macOS now has the Keychain entitlements required by the plugin and stores credentials as device-bound Keychain items. Web continues to use tab-scoped `sessionStorage`; unsupported targets and secure-storage failures fail closed. Updated Forge session docs cover libsecret/Secret Service runtime requirements on Linux and ATL for Windows builds.

Validation: platform policy, credential-store, token-refresh, and OAuth client-selection suites pass (20 tests); `flutter analyze --no-pub` is clean; the `/forge/` Web build and Android debug APK build succeed. Linux desktop build stops because `libsecret-1>=0.18.4` is missing. macOS/Windows build and real OS keychain integration remain unverified. This improves Console cold-start continuity only; live instance inventory, resource discovery, scheduling, and dispatch remain gated by ADR-0039, and ADR-0114 remains Proposed/null. See plan §46.

#### 2026-09-14 continuation — periodic Forge snapshot recovery

The Flutter Forge screen now retries a previously failed Conversation snapshot after a successful periodic owner change-feed read. A temporary initial network outage can therefore recover the session list without requiring manual refresh; existing state and cursor safety rules remain unchanged. The regression widget test covers the failed initial snapshot, later feed success, and automatic recovery. No Run write, device inventory, scheduling, or dispatch was added. See plan §47.

#### 2026-09-14 continuation — remote CLI stdin Prompt submission

`forge-runtime remote prompts add CONVERSATION_ID --expected-version N -` now reads bounded UTF-8 Prompt content from stdin. The sole marker preserves multiline text and avoids argv/shell-history disclosure; empty, invalid UTF-8, oversized, or mixed marker input fails before the API call. Existing idempotency, CAS, owner authorization, and storage-only Prompt semantics are unchanged.

Validation: all 101 remote CLI/TUI tests pass, including stdin bounds and multiline request checks; `cargo check -p forge-runtime-cli --all-targets`, strict CLI Clippy, and workspace formatting pass. No Run, device inventory, scheduling, or dispatch path was added. See plan §48.

#### 2026-09-14 continuation — offline Forge Conversation metadata recovery

Flutter Console now keeps a bounded owner-bound snapshot of the last successful Conversation list for temporary network failures. Only IDs, scope, title, timestamps, and aggregate version are cached; Prompt bodies, Run data, tokens, and owner claims are excluded. A stale/offline banner marks fallback data, successful network reads replace it, opaque tokens disable the cache, and Forge sign-out clears the current owner's snapshot. Cache validation rejects unknown fields, invalid metadata, duplicate/misordered IDs, oversized records, and binding mismatches. Focused cache/widget tests and `flutter analyze --no-pub` pass. This is read continuity only; device inventory, scheduling, dispatch, and execution remain gated by ADR-0039/ADR-0114. See plan §49.

#### 2026-09-14 continuation — Flutter desktop refresh-token process lock

Flutter Linux/macOS/Windows refresh rotation now takes a per-client application-support `FileLock`, reloads secure credentials under that lock, and lets a waiting process reuse the winner's successor token instead of consuming a single-use refresh token twice. Web and Mobile keep their process/tab-scoped no-op lock. Lock and secure-store failures fail closed; injected lock/backend seams keep tests deterministic.

Validation: the full Flutter test suite passes with 3 skipped, `flutter analyze --no-pub` is clean, the `/forge/` Web build and Android debug APK build succeed, and the focused refresh/credential tests cover serialized actions and stale-record reload. Real macOS/Windows multi-process/keychain runs remain unverified; Linux still needs libsecret and Secret Service. This remains session continuity only; device inventory, scheduling, dispatch, and execution stay gated by ADR-0039/ADR-0114. See plan §50.

#### 2026-09-15 continuation — bounded Forge GET read retries

The Flutter Forge API now retries only idempotent GET reads after transport failures or HTTP 408/425/429/5xx responses, at most three attempts with 50 ms and 100 ms backoff. Conversation and Prompt POST writes keep one-request transient behavior; their explicit retry state reuses the same idempotency key, while an existing one-time 401 refresh retry remains available. Tests verify transient read recovery and exactly one request for a failed write.

Validation: `flutter analyze --no-pub`, the full Flutter suite with 3 skipped, the `/forge/` Web build, Android debug APK build, Dart formatting, and diff checks pass. This changes read convergence only; owner authorization, Prompt/Run semantics, device inventory, scheduling, dispatch, and execution remain gated by ADR-0039/ADR-0114. See plan §51.

#### 2026-09-15 continuation — bounded Rust CLI/TUI GET read retries

Rust remote CLI/TUI GET calls now retry only replay-safe session reads after transport/read failures or HTTP 408, 425, 429, and 5xx responses. The bounded policy allows three total attempts with 50 ms and 100 ms backoff. Conversation and Prompt POST writes remain single-attempt for transient responses; the TUI preserves the exact body, CAS version, and idempotency key for explicit recovery, and 401 still never causes API replay.

Request tests cover authenticated recovery after a transient read response and assert that a failed Prompt write is sent once. Existing TUI failure fixtures cover bounded retry exhaustion while retaining selected history and change-cursor behavior. `cargo test -p forge-runtime-cli --bin forge-runtime remote_` passes (103 tests), strict CLI Clippy and workspace formatting pass. This changes read continuity only; device inventory, scheduling, dispatch, and execution remain gated by ADR-0039/ADR-0114. See plan §52.

#### 2026-09-15 continuation — Flutter pagination, credential lock, and response status closure

The Flutter Forge session list preserves its existing `after_id` when a
load-more read fails; only a first-page cache fallback clears pagination state,
so a later retry requests the same page. Desktop Linux/macOS/Windows
credential-store restore, login writes, clears, and refresh rotation now share
one per-client `FileLock` through a non-reentrant lock-owned scope. Login and
sign-out wait for an in-flight rotation, while Web/Mobile keep no-op locks.
The Flutter transport preserves the received HTTP status for malformed,
oversized, and unreadable response bodies; transient 5xx responses still use
bounded GET retries and malformed 401 responses still enter one-time refresh.
A failed secure-store reload clears the in-memory Forge slot before another
request can use it.

Widget, credential, refresh, and API resilience tests cover these paths.
`flutter analyze --no-pub`, the full Flutter suite (1,443 passed, 3 skipped),
Web build, Android debug APK build, formatting, focused tests, and
`git diff --check` pass. Real macOS/Windows multi-process/keychain runs remain
unverified; Linux still needs libsecret and Secret Service. This remains
session/read continuity only; device inventory, scheduling, dispatch, and
execution stay gated by ADR-0039/ADR-0114. See plan §53.

#### 2026-09-16 continuation — Agent session request history

Agent operations now has an instance-pinned, bounded request-history dialog
for Hub-accepted session creation and close operations. It uses one
authenticated GET with strict DTO/status/error validation, opaque pagination,
instance and duplicate checks, and a 256 KiB response bound. Read-only detail
refresh and explicit open validate the request/session/instance binding;
history never retries or replaces locally unconfirmed POST state. Authorization
or selection changes clear late responses, while 404/405/501 older Hub routes
are reported as unsupported.

The current shared Flutter worktree passes `flutter test --no-pub` (1509
passed, 3 skipped) and `flutter analyze --no-pub`. The isolated verification
record remains historical and documents MockClient/Chrome evidence only. This
is Agent Hub history UI, not Forge Conversation authority, device inventory,
scheduling, or dispatch; ADR-0039 remains in force and ADR-0114 remains
Proposed/null. See plan §54.

#### 2026-09-16 continuation — Forge stale-state closure and Local ABI environment digest

The Forge session screen now clears the visible Conversation, selection, and
Prompt/Run details after a deterministic first-page API or response error;
only transport failures and HTTP 5xx retain the owner-bound offline metadata
fallback. A failed load-more page preserves the confirmed first page and its
cursor so the same page can be retried. A feed 401/403 ends any in-flight
initial snapshot loading, invalidates late responses, clears credentials, and
leaves a sign-in error instead of a permanent spinner. Widget regressions
cover deterministic first-page errors, feed authorization races, pagination
cursor preservation, and newer feed versions winning over late older pages.

ADR-0039 §14 Local ABI Step 0 now captures a bounded, deterministic
domain-separated SHA-256 digest of a fixed safe environment allowlist. The
allowlist excludes credential/token/key/secret/password/auth names, uses one
canonical sorted manifest, rejects duplicate or invalid UTF-8 names, missing
`PATH`, oversized values/manifests, and serialization failures as
`NotCaptured`, and gives the command the exact snapshot used for the digest.
The work directory remains outside the digest and is governed by existing
workspace/cwd checks. The slice remains local-only with empty ArtifactRefs;
there is no CAS, device registration/heartbeat, inventory, scheduler,
reservation, dispatch, network, or remote execution.

Validation: Flutter `flutter test --no-pub` **1509 passed, 3 skipped** and
`flutter analyze --no-pub` clean; Forge/Agent focused tests 22/22. Rust
infrastructure 456/456, domain fabric 5/5, environment/fabric targeted
10/10, format and focused Clippy pass. The broader infrastructure Clippy
command still has five pre-existing `needless_borrow` warnings in
`sqlite_hub/owned_run_read.rs`. See cross-device plan §§54–55.
The repository-wide `node harness/gate.mjs` remains BLOCKED by 12 existing or
expanded files over the 500-line limit; this continuation did not include that
file-size refactor.

#### 2026-09-16 continuation — Local ABI validation convergence

The Local ABI validation now has one Rust domain owner. `EnvironmentDigest`
validates the bounded SHA-256/entry-count/reason representation, and the
infrastructure adapter reuses that validator. `ExecutionEvidence::validate_local`
binds ABI/source, the current local target, exact Run identity, adjacent
`ToolStarted` sequence, truncation state, and rendered output byte count; the
Run journal invokes it before accepting `ToolFinished` evidence. Invalid UTF-8
environment names fail closed as `NotCaptured`.

Validation: domain Fabric 6/6, Run-journal transcript 15/15, infrastructure
environment/fabric targeted tests, infrastructure **456/456**, strict domain
Clippy, and workspace formatting pass. This remains local ABI/evidence
validation only; it does not add device enrollment, heartbeat, inventory,
placement, reservation, scheduling, CAS artifacts, or remote execution.
ADR-0039 remains in force and ADR-0114 remains Proposed/null. See plan §57.


#### 2026-09-16 continuation — Shared-session response contract fixture

Added `docs/contracts/fixtures/forge-shared-session-v1.json` to freeze the
owner-scoped Conversation list, newest-first Prompt history, dense owner-local
change feed, and storage-only Prompt append receipt. Go Runtime bridge
validators, Rust CLI/TUI decoders, and Flutter Console models all validate the
same fixture through `scripts/test-forge-contracts.sh`, including exact fields,
Conversation identity, cursor order, aggregate versions, and user Prompt
role. The fixture is a contract guard and does not create Runs, pending
intents, device records, inventory, reservations, scheduling, or dispatch.

Validation: the cross-repository contract script passes its Go, Rust, and
Flutter checks. The Rust invocation is scoped to the CLI binary and domain
library contract targets so unrelated broken integration test targets do not
mask this contract result. The new Go test is 58 lines and no production Go
file changed; `go build ./...`, `go vet ./...`, and
`go test ./... -run 'TestMaintainability_|TestArchitecture_'` pass. The
Console repository exposes `python3 cli.py check` rather than a `quality`
command; that check still reports its existing oversized Flutter files (and
ignored `.pi-batch` worktrees) while `flutter analyze --no-pub` remains clean.
ADR-0039 remains in force and ADR-0114 remains Proposed/null. See plan §56.


#### 2026-09-13 continuation — Android/iOS Forge credential persistence (historical; desktop behavior superseded by §46)

Forge OAuth credentials now use one versioned secure-storage record on Android/iOS. Login waits for write-and-readback, the Forge route gate restores the record asynchronously after cold start, and refresh rotation persists before replacing the in-memory credential. Forge sign-out revokes the freshest access/refresh tokens best-effort and clears only the Forge slot; app-wide session cleanup also removes that record. Android backup is disabled, while iOS uses this-device-only Keychain accessibility and Runner entitlements. Web keeps its Forge tab-scoped `sessionStorage`; Linux/macOS/Windows still use process memory.

Validation: the full Flutter test suite passes (2,235 tests), `flutter analyze --no-pub` is clean, and an Android debug APK builds successfully. The secure-store tests cover write-before-cache, cold restore, wrong-client rejection and cleanup, refresh rotation, and global cleanup while preserving the separate Admin slot. iOS compilation and physical-device lifecycle behavior were not tested. This adds no API or server behavior and does not enable live inventory or dispatch; ADR-0039 remains in force and ADR-0114 remains Proposed/null. Details are in [cross-device implementation plan](../docs/design/ai-engineering-os/cross-device-session-and-fabric-plan.md) §44.

#### 2026-09-16 continuation — Shared-session and Local ABI gate convergence

The shared owner-scoped session fixture now drives exact-field, identity,
cursor, aggregate-version, and append-role checks in Go Runtime bridge, Rust
CLI/TUI, and Flutter Console. The live Go App Server → Rust Hub journey was
rerun with independent clients: Flutter's API service and `ForgeSessionsGate`
screen, the Rust CLI, and the Rust TUI read and append the same Conversation.
The Flutter live harness now injects an in-memory credential store so it does
not require the host platform secure-store/file-lock plugin; production
credential behavior remains unchanged. The request recorder rejects Run-intent,
device, placement, scheduling, and dispatch calls.

The Rust file-size split is complete and `node harness/gate.mjs` is green:
4,245 files, zero violations. Go build, vet, full tests, and the live shared
session E2E pass; Rust workspace no-run compilation, CLI 386 tests,
infrastructure 457 tests, SQLite Run-store 12 tests, Run-journal transcript 15
tests, strict CLI/domain/infrastructure Clippy, formatting, and the shared
contract script pass. Flutter `flutter test --no-pub` reports **1,578 passed,
4 skipped**, and `flutter analyze --no-pub` is clean. The Console's
`python3 cli.py check` still reports its existing oversized Flutter files and
ignored `.pi-batch` worktrees even though analysis and tests pass.

This remains session/Prompt/read-only Run and local ABI evidence work. No live
device enrollment, heartbeat, inventory, reservation, scheduler, placement
authority, or remote execution was added; browser deployment, native App/Mobile
OAuth, and physical-device evidence remain open. ADR-0039 is still
planning-only and ADR-0114 remains Proposed/null. See cross-device plan §58.

#### 2026-09-16 continuation — Offline device-resource observation contract

Added `forge-device-inventory-observation-v1`, a strict caller-supplied,
read-only resource observation fixture. It carries the owner and instance
declarations, approval/cordon/liveness, fixed timestamps, CPU/memory/storage,
runtime/GPU, residency/trust/sandbox, and concurrency fields while requiring
unverified markers and false execution/reservation/dispatch authority bits.
Go validates and projects the rows through the existing offline placement
comparison; the Rust device-registry reference maps the same rows to typed
candidates; Flutter adds `ForgeDeviceInventoryPage` for exact read-only
decoding. No API route or persistence is involved.

Validation: `scripts/test-forge-contracts.sh` passes the Go, Rust, and Flutter
inventory consumers; the targeted Flutter model test and analyze pass, Rust
format and the targeted domain test pass, and repository diff checks are clean.
This remains P3a offline contract work. It does not prove device identity,
freshness, liveness, or schedulability and does not add enrollment, heartbeat,
inventory persistence, reservation, scheduling, dispatch, or execution. ADR-
0039 remains planning-only and ADR-0114 remains Proposed/null. See plan §59.

#### 2026-09-16 continuation — Owner-bound Conversation detail

The authenticated shared-session surface now has a read-only
`GET /api/v1/conversations/{conversation_id}` projection. Go passes only the
verified issuer/subject/tenant tuple to Rust, SQLite joins the owner record,
and missing and foreign IDs share the sanitized `not_found` result. The
Runtime bridge rejects malformed or expanded detail responses, and the shared
session fixture now covers list, detail, Prompt history, dense change feed, and
Prompt append receipt.

CLI adds `remote sessions show CONVERSATION_ID`; TUI re-reads the selected
entry before refreshing history; Flutter exposes `getConversation` and checks
the returned ID. Web coverage verifies the Forge tab-scoped credential slot
restores without overwriting the Admin slot. This remains a metadata/read
slice: it does not create Runs, device records, inventory, reservations,
scheduling, dispatch, or remote execution authority.

Validation: Go route, owner-isolation, and Rust RPC integration tests; Rust
remote request/parser/response tests; Flutter API and shared-contract tests;
the cross-repository contract script; Go build/vet/tests; Rust workspace
compilation; `cargo fmt --all -- --check`; strict client Clippy; and
`node harness/gate.mjs` all pass. Browser deployment, native App/Mobile OAuth,
and physical-device evidence remain open. ADR-0039 remains planning-only and
ADR-0114 remains Proposed/null. See plan §60.

#### 2026-09-16 continuation — Final shared-slice regression closure

The post-detail/post-inventory regression is green: Go build, vet, and the
full `go test ./... -race -count=1`; Rust workspace no-run compilation,
formatting, strict Clippy for CLI/domain/infrastructure, CLI 388 tests, and
infrastructure 457 tests; Flutter `1,578 passed, 4 skipped` with clean
analysis; the browser Forge credential gate; the shared Go/Rust/Flutter
contract script; and both repository diff checks. The root size gate reports
4,251 files with zero violations.

These remain local/test-server checks. Live Runner identity, registration,
heartbeat, authoritative inventory, reservation, scheduling, dispatch,
remote execution, native App/Mobile OAuth, and physical-device behavior stay
outside the approved boundary. ADR-0039 remains planning-only and ADR-0114
remains Proposed/null.

#### 2026-09-16 continuation — Pure lease and fencing contract

Added `forge_runtime_domain::execution::lease`, a bounded in-memory contract
for the future Runner boundary. It binds attempt/target identity, lease
epoch, fencing token, and server-observed expiry; renewal rotates epoch and
token; stale, foreign, expired, reused-token, and backwards-time proofs fail
closed. Terminal receipts replay only the original proof/disposition, reject
changed outcomes, and keep uncertain effects terminal without automatic retry.
No clock, storage, HTTP, Runner, reservation, scheduler, artifact transfer,
or process dispatch was added; the current local-only target is unchanged.

Validation: 7 focused domain tests, strict domain Clippy, workspace Rust
formatting, and the repository diff check pass. ADR-0039 remains
planning-only; ADR-0114 and the separate P4 execution decision remain
unaccepted.

#### 2026-09-16 continuation — Read-only Run observer resume contract

Added `forge-run-observer-resume-v1`, a shared metadata-only timeline fixture
consumed by Go Runtime bridge validation, Rust CLI validation, and Flutter
Console validation. It fixes three bounded pages, advances
`after_sequence → scanned_through_sequence`, and verifies event sequences
`1..5` without gaps while preserving Conversation/Run identity and sanitized
event metadata.

Validation: `scripts/test-forge-contracts.sh` passes the Go, Rust, and Flutter
consumers. No Run write, execution resume, live stream, device registration,
inventory, scheduling, Runner dispatch, or remote execution was added.
ADR-0039 remains planning-only; ADR-0114 and the separate P4 execution
decision remain unaccepted.

#### 2026-09-16 continuation — Pure heartbeat sequencing contract

Added the shared `forge-device-heartbeat-contract-v1` fixture consumed by the
Go effect-free reference model and the Rust device-registry tests. It covers
first generation/sequence, monotonic updates, generation restart, replay,
skipped and old generations, instance changes, server-clock regression,
revoked devices, lease bounds, and timestamp overflow. The fixture's identity,
persistence, inventory, execution, reservation, and dispatch authority bits
are all false. The contract script executes both validators; no route,
listener, credential, database, clock source, enrollment, or remote execution
was added. ADR-0039 remains planning-only and ADR-0114 remains Proposed with
null acceptance fields.

#### 2026-09-16 continuation — Pure device identity proof and approval binding

Added `forge-device-identity-proof-contract-v1`, consumed independently by a
pure Go reference package and the Rust device-registry model. The fixture
covers exact `(issuer, subject, tenant_id)` owner binding, immutable device/key
binding, one-time challenge identity and freshness, pending versus approved
owner state, key-rotation mismatch, replay/expiry, and revoked/expired
credential states. A proof digest is only a bounded test-vector label; no
cryptographic verifier, key material, issuer, challenge consumption, or
credential issuance exists in this slice. All authority bits remain false.

Validation: Go and Rust fixture consumers pass with strict unknown-field
decoding, Rust formatting and the repository diff check pass, and the shared
contract script now runs both identity validators. No HTTP route, persistence,
device listener, inventory authority, reservation, scheduling, dispatch, or
remote execution was added. ADR-0039 remains planning-only and ADR-0114
remains Proposed/null.

#### 2026-09-16 continuation — Pure heartbeat persistence CAS plan

Added `forge-device-heartbeat-persistence-contract-v1`, a pure value-level
compare-and-swap plan consumed by Go and Rust. It binds expected revision to a
complete heartbeat replacement and covers initial insert, monotonic sequence,
replay, version conflict, foreign/revoked device, server-clock rollback,
missing snapshot, invalid persisted revision, and revision overflow. The
Rust contract maps revision zero through the validated persisted-state
constructor; it does not bypass the invariant. No database, clock, network,
listener, retry, inventory publication, reservation, scheduling, dispatch, or
execution was added. All authority bits remain false.

#### 2026-09-16 continuation — Pure inventory status projection

Added `forge-device-inventory-status-contract-v1` and matching Go/Rust pure
projections. Fixed-time cases classify declarations as revoked, cordoned,
offline, stale, pending, reserved, or online, with `declared_eligible` only
for the online display state. Future snapshots, invalid lease windows, and
unknown states fail closed. This is a display/comparison contract only and
does not expose identity, persistence, heartbeat, inventory, reservation,
scheduling, dispatch, or execution authority. ADR-0039 remains planning-only;
ADR-0114 remains Proposed/null.

#### 2026-09-16 continuation — Flutter inventory status projection consumer

Flutter Console now consumes the shared
`forge-device-inventory-status-contract-v1` fixture through a strict
`ForgeDeviceInventoryStatusObservation` decoder and fixed-time pure
projection. The test asserts the exact envelope, all-false authority bits, and
all eleven status/error cases, keeping status precedence and
`declared_eligible` parity with the Go and Rust references. The contract
script runs this consumer alongside the existing inventory observation test.

This remains a local display contract: no clock, endpoint, persistence,
registration, reservation, scheduling, dispatch, or execution authority was
added. ADR-0039 remains planning-only and ADR-0114 remains Proposed/null;
see implementation plan §68.

#### 2026-09-16 continuation — Pure owner-scoped inventory snapshot canonicalization

Added `forge-device-inventory-snapshot-canonical-v1`, consumed by pure Go,
Rust, and Flutter models. A fixed caller-declared owner tuple and supplied
observation time are validated; rows are copied without mutating input and
ordered by `(device_id, instance_id)`. Foreign-owner rows, duplicate composite
keys, invalid identifiers, and zero observation time fail closed. The shared
length-prefixed, domain-separated SHA-256 is only an integrity label for an
unverified declaration.

Validation: Go and Rust fixture consumers, Rust strict Clippy/formatting,
Flutter fixture test/analyze, and the cross-repository contract script pass.
No route, database, clock source, registration, heartbeat listener, discovery,
reservation, scheduling, dispatch, or Runner execution was added. ADR-0039
remains planning-only; ADR-0114 remains Proposed/null; see implementation
plan §69.

#### 2026-09-16 continuation — Pure Runner command and terminal receipt ABI

Added the Rust Runtime-only `execution::runner_command` contract and strict
fixture `forge-runner-command-terminal-receipt-v1`. `RunnerCommand` accepts
only bounded direct `argv`, an opaque staged-workspace reference, an
idempotency key, timeout/output limits, and an exact lease proof. A
domain-separated command digest binds `RunnerTerminalReceipt` to those bytes;
receipt validation rechecks the supplied current lease and preserves epoch,
fencing, idempotent replay, and terminal `uncertain` semantics. Five focused
domain tests and the contract-script consumer pass.

This is a pure value ABI. It does not execute a process, read a clock, persist
state, reserve capacity, connect a Runner, stage artifacts, publish audit, or
grant execution authority. Go has no assignment consumer yet because P4 still
requires a separately Accepted execution/security decision; ADR-0039 remains
planning-only and ADR-0114 remains Proposed/null. See implementation plan §70.

#### 2026-09-16 continuation — Flutter offline placement dry-run consumer

Flutter Console now consumes the existing CPU-only and GPU placement parity
fixtures through a fixed-time `ForgeDevicePlacementRequest` evaluator. It
compares resource, runtime, GPU, residency, trust, sandbox, concurrency,
approval, cordon, liveness, freshness, and lease declarations, returning
stable device and exclusion-reason ordering. Invalid requests and duplicate
device IDs fail closed.

The result keeps all declaration and authority flags honest and always reports
`execution_authorized=false`, `reservation_created=false`, and
`dispatch_performed=false`; no target is selected. The cross-repository
contract script now runs this consumer with the Go and Rust parity checks.
This remains local display/comparison logic with no endpoint, reservation,
scheduler, dispatch, or Runner effect. ADR-0039 remains planning-only and
ADR-0114 remains Proposed/null. See implementation plan §71.

### Cross-device plan §72 — Rust CLI offline placement dry-run

- `forge-runtime device placement dry-run --input FILE|-` consumes the strict
  placement parity document through the Rust domain evaluator.
- The command is bounded and read-only: it opens no Hub or network endpoint,
  writes no state, and emits explicit false authority bits for identity,
  heartbeat, authoritative inventory, reservation, execution, and dispatch.
- CPU/GPU fixture, stdin, malformed/unknown-field, oversized, parser, and
  contract-script coverage is required before treating this slice as complete.

### Cross-device plan §73 — Rust CLI offline inventory show

- `forge-runtime device inventory show --input FILE|-` consumes the strict
  `forge-device-inventory-observation-v1` fixture and validates owner,
  device/instance, duplicate-row, unverified, and authority-false declarations.
- Output is sorted by `(device_id, instance_id)` and exposes the declared CPU,
  memory, storage, GPU, runtime, liveness, placement, and concurrency values
  for local inspection. File/stdin, stable-order, unknown-field, oversized,
  authority mutation, parser, and contract-script coverage is included.
- The command is offline and read-only: no Hub, network, clock, persistence,
  registration, target selection, reservation, scheduling, dispatch, or Runner
execution is performed. ADR-0039 remains planning-only and ADR-0114 remains
Proposed/null.

### Cross-device plan §727 — Challenge expiry, reissue, and CAS race boundaries

Forge Core now covers exact-expiry reissue, consumed-challenge replacement
after persistence/restart, minimum/maximum TTL and `uint64` overflow rejection,
and concurrent file-CAS issuance where exactly one active challenge succeeds.
The challenge remains candidate-only, owner-scoped, preview-only, and
all-false; production enrollment/heartbeat and ordinary/accepted writes remain
closed.

### Cross-device plan §728 — Console instance-hidden Conversation creates remain private-read free

The shared Snaplink Console Web/App/Mobile Sessions screen refreshes the
selected owner-bound client-instance session/resource projection before an
owner-wide Conversation create. If the returned Conversation is not declared
by that instance, it remains outside selection, URL navigation, Prompt/Run
hydration, and private state while the owner-side creation result is retained.
A focused Flutter regression proves one create POST and zero Prompt/Run reads;
no membership writer or instance authority was added.

### Cross-device plan §74 — Pure session-to-device placement observation

- `forge-session-placement-observation-v1` binds a caller-declared owner tuple,
  Conversation ID, and Run ID to the fixed-time placement parity result while
  retaining each `(device_id, instance_id)` pair and stable exclusion reasons.
- Go `deviceplacement`, Rust Runtime domain, and Flutter Console
  `forge_session_placement` consumers fail closed for duplicate instances,
  invalid owner/session bindings, or candidate mismatches. Selected device and
  instance remain empty, and identity/heartbeat/inventory/reservation/
  execution/dispatch authority bits remain false.
- The contract script covers all three consumers and the malformed duplicate
  instance case. The observer is value-only: no endpoint, clock, storage,
  registration, scheduler, reservation, Runner, or process execution is
  added. ADR-0039 remains planning-only and ADR-0114 remains Proposed/null.

### Cross-device plan §75 — TUI offline inventory inspection

- `forge-runtime remote tui` accepts `inventory show --input FILE` and renders
  the bounded inventory observation used by the standalone CLI. It requires a
  filesystem path so the interactive stdin stream remains available; stdin
  input is supported by the standalone CLI command.
- The TUI output retains declared resource rows plus explicit unverified and
  authority-false markers. Its focused regression confirms that rendering the
  local observation emits no `/api/v1/devices` request.
- No device route, Hub mutation, clock, persistence, registration, target
  selection, reservation, scheduling, dispatch, or Runner execution is added.
  ADR-0039 remains planning-only and ADR-0114 remains Proposed/null.

### Cross-device plan §76 — Pure Run-intent observation across session and placement

- Added the shared `forge-run-intent-observation-v1` fixture and strict
  value-only consumers in Go `deviceplacement`, Rust Runtime domain, and
  Flutter Console. The contract binds a payload-free accepted Prompt receipt,
  an existing Run summary, and the existing owner/Conversation/Run placement
  observation, preserving exact IDs, status/sequence, replay state, and
  placement decision/eligible-instance counts.
- The result is always a preview: `preview_only=true`, no device or Runner
  instance is selected, Prompt content is absent, declarations remain
  unverified, and identity/heartbeat/inventory/reservation/execution/dispatch
  authority bits are false. Confused owner/session/Prompt/Run/placement
  bindings and claimed authority fail closed.
- Go/Rust/Flutter fixture tests and the contract script pass. This adds no
  Prompt or Run creation, clock read, Hub/Runner request, persistence,
  registration, scheduler, reservation, dispatch, or process execution.
  ADR-0039 remains planning-only, ADR-0114 remains Proposed/null, and P4
  still needs a separately Accepted execution/security decision.

### Cross-device plan §77 — CLI/TUI offline Run-intent preview

- Rust CLI exposes `device placement run-intent-preview --input RUN_FILE|-
  --placement-input SESSION_FILE|-`; at most one bounded input may consume
  stdin. The command strictly decodes both shared fixtures, recomputes session
  placement through the domain evaluator, and invokes the pure Run-intent
  observer.
- `remote tui` exposes `run-intent-preview --input RUN_FILE --placement-input
  SESSION_FILE` with file-only inputs so interactive stdin remains available.
  JSON/human output carries no Prompt content, selects no target, and keeps all
  identity/heartbeat/inventory/reservation/execution/dispatch authority false.
- CLI integration and TUI focused tests cover stable output, unknown and
  oversized input, owner mismatch, parser bounds, and no device request. This
  adds no Prompt/Run creation, clock, Hub/Runner call, persistence, selection,
  reservation, dispatch, or process execution. ADR-0039 remains planning-only,
  ADR-0114 remains Proposed/null, and P4 still needs a separate Accepted
  execution/security decision.

### Cross-device plan §78 — Pure multi-instance resource summary

- `forge-device-resource-summary-v1` combines a caller-supplied inventory
  declaration with an already observed session-placement declaration. Go,
  Rust, and Flutter aggregate device/Runner-instance counts, declared
  available CPU/memory/storage/GPU totals, and eligible device/instance
  counts, binding exact owner, Conversation/Run, and device/instance pairs.
- Resource totals include declarations from ineligible instances and therefore
  are observations of supplied values rather than schedulable capacity. Every
  value remains unverified; selected IDs are null and identity, heartbeat,
  inventory, reservation, execution, and dispatch authority bits remain false.
  Duplicate, foreign, missing, malformed, and overflow bindings fail closed.
- Strict fixture consumers and confused-binding/GPU aggregation tests are
  connected to the contract script. The slice adds no device route, network
  discovery, clock, persistence, registration, heartbeat, scheduler,
  selection, reservation, dispatch, Runner, or process execution. ADR-0039
  remains planning-only and ADR-0114 remains Proposed/null; P4 still needs a
  separate Accepted execution/security decision.

### Cross-device plan §79 — CLI/TUI offline multi-instance resource summary

- `device inventory resource-summary --input FILE|-` now provides a bounded
  standalone CLI view over the shared resource-summary fixture. It reuses the
  pure Rust domain aggregator and emits stable JSON or human-readable metrics;
  unknown fields, oversized input, confused owner bindings, and claimed
  authority fail closed.
- `remote tui` accepts `inventory resource-summary --input FILE`, with a
  filesystem path required so interactive stdin remains available. The
  focused regression verifies aggregate metrics, false authority, and no
  `/api/v1/devices` request. Both surfaces remain read-only observations with
  no route, clock, storage, registration, heartbeat, selection, reservation,
  scheduling, dispatch, Runner, or process execution; ADR-0039 remains
  planning-only and ADR-0114 remains Proposed/null.

### Cross-device plan §80 — CLI/TUI offline inventory status projection

- `device inventory status --input FILE|-` now consumes the strict
  `forge-device-inventory-status-contract-v1` envelope, recomputes every case
  with the pure Rust fixed-time projector, and emits stable JSON or human
  status/fresh/declared-eligible output. Unknown fields, duplicate case names,
  oversized input, claimed authority, and expected-result mismatches fail
  closed; bounded stdin is supported by the standalone CLI.
- `remote tui` accepts `inventory status --input FILE`, requiring a file path
  so interactive stdin remains available. Focused CLI/TUI tests cover stable
  output, file/stdin, authority mutation, malformed boundaries, and absence of
  a `/api/v1/devices` request. This remains a local display projection with no
  route, clock, storage, registration, heartbeat, selection, reservation,
  scheduling, dispatch, Runner, or process execution; ADR-0039 remains
  planning-only and ADR-0114 remains Proposed/null.

### Cross-device plan §81 — CLI/TUI offline inventory snapshot canonicalization

- `device inventory snapshot-canonical --input FILE|-` now consumes the
  strict `forge-device-inventory-snapshot-canonical-v1` envelope, reuses the
  pure Rust canonicalizer and digest, and emits stable ordered keys and
  digest/error results. Unknown fields, duplicate case names, oversized input,
  authority mutation, and expected-result mismatch fail closed; bounded stdin
  is supported by the standalone CLI.
- `remote tui` accepts `inventory snapshot-canonical --input FILE`, requiring
  a file path so interactive stdin remains available. Focused CLI/TUI tests
  cover stable ordering, file/stdin, malformed boundaries, digest mismatch,
  false authority, and absence of a `/api/v1/devices` request. This remains a
  local display projection with no route, clock, storage, registration,
  heartbeat, selection, reservation, scheduling, dispatch, Runner, or process
  execution; ADR-0039 remains planning-only and ADR-0114 remains Proposed/null.

### Cross-device plan §82 — Flutter identity and heartbeat persistence contract consumers

- Flutter Console now consumes the shared `forge-device-identity-proof-contract-v1`
  and `forge-device-heartbeat-persistence-contract-v1` fixtures through strict,
  effect-free models. Identity validation preserves exact owner/device/key/
  challenge binding, credential and approval state, and caller-supplied windows;
  heartbeat persistence uses bounded `BigInt` uint64 values and recomputes the
  complete CAS replacement with revision, generation, sequence, lease,
  rollback, and overflow behavior.
- Contract tests cover all shared cases plus unknown-field and authority
  mutation rejection. The new consumers are included in
  `scripts/test-forge-contracts.sh`, extending parity to Flutter Web/App/Mobile
  code without adding cryptography, key material, challenge consumption,
  persistence, clock, network, enrollment, inventory authority, reservation,
  scheduling, dispatch, Runner, or process execution. ADR-0039 remains
  planning-only and ADR-0114 remains Proposed/null; P4 still needs a separate
  Accepted execution/security decision.

### Cross-device plan §83 — Flutter heartbeat sequencing contract consumer

- Flutter Console now consumes `forge-device-heartbeat-contract-v1` through a
  strict pure model shared with the Go/Rust fixture semantics. It preserves
  full uint64 values with `BigInt` and checks device binding, revoked approval,
  generation/sequence monotonicity, Runner instance changes, explicit server
  time, bounded TTL, and lease-expiry overflow.
- The 12 shared cases plus unknown-field, authority-mutation, and full-uint64
  decoding tests are wired into `scripts/test-forge-contracts.sh`. Capability
  fields remain unverified declarations; this adds no clock, network,
  persistence, live heartbeat publication, registration, inventory authority,
  reservation, scheduling, dispatch, or Runner execution. ADR-0039 remains
  planning-only and ADR-0114 remains Proposed/null; P4 still needs a separate
  Accepted execution/security decision.

### Cross-device plan §84 — Flutter live shared-session cold-start restore evidence

- The authenticated Go HTTP → Rust Hub → Flutter E2E now writes the Forge
  token through an injected credential backend, clears the in-memory Forge
  slot, and lets `ForgeSessionsGate.restore()` recover it before reading the
  shared Conversation and appending a Prompt. The test continues to keep the
  Admin client slot separate.
- This is test-backend evidence rather than physical Android/iOS secure-store,
  browser OAuth, or production deployment evidence. Prompt append remains
  storage-only and creates no Run; the allowlist remains Conversation,
  Prompt, change-feed, and metadata-only Run reads, with no device route,
  inventory authority, reservation, scheduling, dispatch, or Runner execution.

### Cross-device plan §85 — Cross-client pending-write recovery metadata contract

- Go, Rust, and Flutter now consume the strict
  `forge-pending-write-recovery-v1` fixture. The pure projection carries only
  operation, optional Conversation ID, expected aggregate version,
  idempotency key, pending/unconfirmed state, and caller-supplied observation
  times; an unconfirmed write requires reconciliation and every retry must
  reuse the same key.
- Prompt content, title, scope, credentials, and Run payloads are absent.
  The projection reads no clock, contacts no service, writes no persistence,
  creates no Run, and grants no retry authority. It does not provide
  cross-process Prompt-body persistence or automatic replay. Strict Go/Rust/
  Flutter fixture tests are wired into `scripts/test-forge-contracts.sh`;
  device routes, inventory authority, reservation, scheduling, dispatch, and
  Runner execution remain unavailable under ADR-0039/ADR-0114 and the P4
  execution/security gate.

### Cross-device plan §86 — Flutter live Prompt idempotency replay evidence

- The real Go HTTP → Rust Hub → Flutter API journey now submits one Prompt
  twice with the same expected Conversation version and idempotency key. The
  first response creates the Prompt; the second returns the same Prompt ID and
  aggregate version with `replayed=true`. Owner change-feed and Go readback
  prove that history contains one copy, while the widget path appends one
  separate Prompt.
- This validates cross-client retry deduplication through the live storage
  path. The operation remains storage-only and creates no Run or task; the
  test uses a signed test token and injected credential backend, not
  production OAuth or physical Android/iOS secure storage. Device routes,
  inventory authority, reservation, scheduling, dispatch, and Runner
  execution remain gated by ADR-0039/ADR-0114/P4.

### Cross-device plan §87 — Cross-client Snaplink Forge profile contract

- Added `forge-snaplink-profile-v1`, freezing the `forge-api` resource/audience,
  two conversation scopes, public `forge-cli` RFC 8628 device-code plus
  refresh-token grants, and public `forge-console` authorization-code plus
  refresh-token grants.
- Go validates the fixture with the resource-server configuration; Rust checks
  the CLI device-login constants; Flutter checks `ForgeAuthProfile` and the
  Console OAuth request. All three consumers reject unknown fixture fields and
  keep authority bits false.
- The issuer is a deployment placeholder and this is configuration parity
  evidence only. It does not prove live client registration, JWKS reachability,
  token issuance, consent, Conversation access, device inventory, scheduling,
  dispatch, or Runner execution. ADR-0039 remains planning-only, ADR-0114
  remains Proposed/null, and P4 still needs a separately Accepted
  execution/security decision.

### Cross-device plan §88 — Pure Prompt/Run to Runner-command binding

- Added the shared `forge-runner-execution-intent-v1` fixture. Go
  `deviceplacement`, Rust Runtime, and Flutter Console bind the exact owner,
  Conversation, accepted Prompt receipt, existing Run, Attempt, command,
  opaque target, direct-argv digest, lease proof, and idempotency identities.
- Flutter reproduces the Rust domain-separated command digest; strict tests in
  all three consumers reject foreign Run/target bindings and unknown or
  malformed values. `selected_target_id` remains null and `preview_only` is
  always true.
- Every authority marker remains false. The slice does not issue or persist a
  lease, select or reserve a device, contact a Runner, dispatch or execute a
  command, publish audit, or add a route. ADR-0039 remains planning-only,
  ADR-0114 remains Proposed/null, and P4 still needs a separate Accepted
  execution/security decision.

### Cross-device plan §89 — Cross-language Runner terminal receipt observation

- Go `deviceplacement` and Flutter Console now consume the existing
  `forge-runner-command-terminal-receipt-v1` fixture alongside Rust Runtime.
  Strict consumers recompute the domain-separated direct-argv command digest
  and bind command ID, attempt/target/epoch/fencing proof, bounded grant
  window, and caller-supplied observation time.
- Completed receipts require the declared receipt digest; failed and
  uncertain receipts retain bounded reasons. `uncertain` is always surfaced
  as reconciliation-required with `automatic_retry=false`.
- This remains pure ABI parity and fail-closed observation. It does not issue,
  persist, renew, or revoke a lease; read a clock; contact a Runner; stage,
  reserve, dispatch, or execute work; or publish audit. Authority bits remain
  false. ADR-0039 remains planning-only, ADR-0114 remains Proposed/null, and
  P4 still requires a separate Accepted execution/security decision.

### Cross-device plan §90 — Local Snaplink JWKS authenticated session boundary

- Added a test-only Forge app-server integration that starts the real Snaplink
  SSO handler on an ephemeral TLS listener, obtains independent tokens for the
  public `forge-console` and `forge-cli` clients, and sends those tokens through
  Forge's real JWKS-backed `authn` middleware.
- The test creates a Conversation with one client, lists it with the other, and
  appends a Prompt. Every backend call receives the same verified
  `(issuer, subject, tenant_id)` owner tuple, proving that client identity is
  derived from Snaplink claims rather than request bodies.
- The harness uses only memory stores and temporary test keys. It adds no
  production OAuth registration, device route, inventory/heartbeat authority,
  reservation, scheduling, dispatch, Runner execution, or audit outbox.
  ADR-0113 remains Proposed, ADR-0039 remains planning-only, ADR-0114 remains
  Proposed/null, and P4 still needs a separately Accepted execution/security
  decision.

### Cross-device plan §91B — Snaplink-authenticated inert execution intent over Rust Hub

- Added a test-only Go app-server E2E that starts the real Snaplink SSO handler
  on an ephemeral TLS listener, obtains independent `forge-console` and
  `forge-cli` JWTs, validates them through the real Snaplink JWKS, and sends
  both clients through the private inert execution surface to a real Rust Hub.
- Client A creates a Project Conversation, previews and grants the Hub-bound
  execution profile consent; client B submits a consent-checked pending Run
  intent and reads its bounded intent/timeline views. The test revokes consent,
  proves a fresh intent conflicts while the original idempotency key replays
  its immutable receipt, and verifies normal Runs remain empty.
- The production `newConversationRoutes` surface is checked on the same
  consent and pending-intent paths and remains 404. The test is skipped unless
  `FORGE_RUNTIME_BIN` is supplied and adds no device registration, inventory or
  heartbeat authority, lease issuance, reservation, selection, scheduling,
  dispatch, Runner execution, or audit outbox. ADR-0039 remains planning-only,
  ADR-0114 remains Proposed/null, and P4 still needs a separately Accepted
  execution/security decision.

### Cross-device plan §91A — Snaplink introspection-backed authenticated session boundary

- Added a test-only Forge app-server E2E that starts the real Snaplink SSO
  handler on ephemeral TLS, logs in with the public `forge-console` client,
  and validates every protected request through a confidential
  `forge-introspector` client at `/token/introspect`.
- The introspection secret is read from a temporary owner-private 0600 file;
  a counting transport proves there is no JWKS fallback and that revocation is
  observed on the next request. Conversation create/list/Prompt preserve the
  same verified `(issuer, subject, tenant_id)` owner tuple.
- Wrong audience, wrong tenant, missing write scope, and wrong introspection
  secret fail closed. `forge-core/go.mod` uses the requested local Snaplink
  ecosystem replace so the tested introspection response includes the signed
  `tenant_id` provenance field. No device/inventory/heartbeat authority,
  execution route, reservation, scheduling, dispatch, Runner, or audit outbox
  is added; ADR-0039/0114/0113 and P4 gates remain open.

### Cross-device plan §92 — Snaplink JWT consumed by the real CLI/TUI and Flutter shared-session clients

- Extended the §91B test-only cross-process E2E to invoke the actual
  `forge-runtime` binary with the two real Snaplink JWTs. Client A lists the
  owner Conversation, client B reads the Prompt history, and the CLI Run page
  remains empty; client B then opens the same Conversation in the TUI and
  appends a Prompt. With `FORGE_CONSOLE_E2E=1`, the Flutter Console API and
  widget tests use the same Snaplink JWT to cold-start, read the Conversation,
  and append idempotent Prompts. `FORGE_BROWSER_E2E=1` additionally serves the
  Flutter Web build and drives the browser path with that JWT.
- The test is conditional on `FORGE_RUNTIME_BIN` and uses temporary homes plus
  the ephemeral Snaplink/Rust Hub fixtures. It does not exercise device-code
  credential persistence, which depends on the host OS credential store.
  No device/inventory/heartbeat authority, lease, reservation, scheduling,
  dispatch, Runner, execution, or audit outbox is added; ADR-0039/0114/0113
  and P4 gates remain open.

### Cross-device plan §93 — Snaplink change-feed cursor consumed by the real CLI and TUI

- Extended the §92 authenticated process E2E so the real Rust CLI reads the
  owner-scoped change feed and verifies the Conversation-created and
  Prompt-appended events at dense cursors. The real Rust TUI then runs `sync`
  with the same Snaplink token, advances through both events, refreshes the
  selected session and history, and appends a Prompt.
- Exact request allowlists cover only Conversation, Prompt, and change-feed
  reads plus the tested Prompt write. No Run is created, and no device route,
  inventory/heartbeat authority, lease, reservation, scheduling, dispatch,
  Runner execution, or audit outbox is added. ADR-0039 remains planning-only;
  ADR-0114/0113 remain Proposed/null and P4 still needs separate Accepted
  execution/security governance.

### Cross-device plan §94 — Real Snaplink RFC 8628 login and saved CLI credential consumption

- Added an opt-in Go app-server E2E that drives the actual Rust CLI through a
  temporary Snaplink device-code issuer. It reads the verification code,
  approves it with a Forge Console bearer, waits for token polling, checks the
  owner-bound credential file and protected refresh-token store, then starts a
  second CLI process that loads the saved credential and sends its Bearer token
  to the Forge session-list endpoint.
- The test rejects refresh-token material in the JSON credential file and
  verifies issuer/client/subject/tenant binding. It is enabled only with
  `FORGE_DEVICE_LOGIN_E2E=1` because it needs an unlocked host Secret Service;
  the shared-session script preflights `secret-tool` and D-Bus for this mode.
  No Forge device registration, heartbeat/inventory authority, Run, lease,
  reservation, scheduling, dispatch, Runner execution, or audit outbox is
  added. ADR-0039 remains planning-only; ADR-0114/0113 remain Proposed/null
and P4 still needs separate Accepted execution/security governance.

### Cross-device plan §95 — Saved CLI change-feed cursor resumed across processes

- Extended the opt-in RFC 8628 device-login E2E so the first real Rust CLI
  process runs `remote changes list` without `--after-cursor`, persists the
  owner-bound checkpoint, and a second process with the same saved credential
  resumes from it. The temporary Forge endpoint asserts the exact dense
  request sequence `after_cursor=0` then `after_cursor=1`; the second page is
  empty, proving no duplicate replay after a process restart.
- This remains credential and read-replay continuity evidence. It requires an
  unlocked host Secret Service when enabled and adds no Forge device
  registration, heartbeat/inventory authority, Run, lease, reservation,
  scheduling, dispatch, Runner execution, or audit outbox. ADR-0039 remains
  planning-only; ADR-0113/0114 remain Proposed/null and P4 still needs a
  separate Accepted execution/security decision.

### Cross-device plan §96 — Authenticated offline placement preview boundary

The configured Forge session handler now exposes a bounded
`POST /api/v1/device-placement/preview` route for the existing
`forge.device-placement-dry-run/v1` caller declaration. It requires the
verified Snaplink owner tuple to match the declaration exactly and uses the
existing conversation read scope. The handler is stateless and deterministic:
all owner/device attributes remain unverified and `execution_authorized`,
`reservation_created`, and `dispatch_performed` are always false. Query
parameters, wrong methods, malformed declarations, missing read scope, and
foreign owners fail closed; `/api/v1/devices` and heartbeat paths remain 404.

This closes only an authenticated P3a observation boundary so Web/App/Mobile
clients can submit a bounded offline declaration for comparison. It does not
read a registry, Hub, clock, network, inventory or heartbeat store and does not
select, reserve, schedule, dispatch, execute, or publish audit. ADR-0039 stays
planning-only; ADR-0113/0114 remain Proposed/null and P4 still requires a
separate Accepted execution/security decision.

### Cross-device plan §97 — Flutter authenticated placement preview consumer

Flutter Console's Forge API client now serializes the strict placement
declaration and sends one authenticated `POST /api/v1/device-placement/preview`.
The response decoder requires the fixed schema and notice, stable device and
reason ordering, exact owner parity, and false execution/reservation/dispatch
authority. It rejects a foreign owner or any response that claims authority;
the client does not retry an uncertain POST, persist the declaration, or infer
live capacity. The result remains a display-only P3a comparison for
Web/App/Mobile; P3b inventory and P4 scheduling/Runner execution remain
gated by ADR-0039/0113/0114 and a separate Accepted execution decision.

### Cross-device plan §98 — Authenticated placement preview consumed by CLI and TUI

Rust remote CLI now exposes `remote placement preview --input FILE|-` and
sends one authenticated POST to the existing P3a placement preview route. Its
strict decoder binds the response owner and exact device ID set to the
submitted declaration, checks stable result/reason ordering and the fixed
all-false authority envelope, and rejects malformed or forged responses. The
remote TUI adds `placement-preview --input FILE`; it uses the same decoder and
leaves interactive stdin available for commands. Focused request, parser,
bounds, authority-mutation, exact-device-set, and TUI rendering tests pass.

The Snaplink-authenticated Go→Rust Hub process E2E now invokes the actual CLI
and PTY TUI against the route, verifies the returned unverified result, and
asserts one exact placement POST in each client request allowlist.

This is client consumption of an offline declaration only. No registry read,
live enrollment/heartbeat, target selection, reservation, scheduling,
dispatch, Runner/process execution, or audit outbox was added. ADR-0039 remains
planning-only; ADR-0113/0114 remain Proposed/null and P4 still needs a
separate Accepted execution/security decision.

### Cross-device plan §99 — Pure Aero-ID profile and source-membership projection

Added the bounded `forge.aero-id-profile-projection/v1` fixture and isolated
Go, Rust, and Flutter consumers. The projection retains the exact
caller-supplied Snaplink owner tuple, Aero-ID-owned display fields, and
source-scoped membership rows with deterministic ordering; unknown fields,
foreign owners, malformed values, and any authority bit fail closed. The
authority envelope is fixed false, so source memberships are never treated as
Forge tenant or authorization grants.

This is P5 groundwork only. It does not call Aero-ID, create a separate
OAuth client or audience, forward a token, persist a profile snapshot, add a
route, or affect Conversation, device, inventory, reservation, scheduling,
dispatch, Runner, or execution authority. ADR-0039 remains planning-only;
ADR-0113/0114 remain Proposed/null and P4 still needs separate Accepted
execution/security governance.

### Cross-device plan §100 — Flutter Console live placement-preview consumption

The opt-in Snaplink-authenticated Flutter Console API E2E now receives the
same strict P3a placement declaration as the Go route and Rust CLI/TUI. It
submits exactly one `POST /api/v1/device-placement/preview`, checks the
returned owner tuple and evaluation time against the request, verifies the
declared device result, and rejects execution/reservation/dispatch authority.
The Go recorder allowlist includes this single observation request and still
rejects unrelated device or effect paths. This is live transport evidence for
the Web/App/Mobile API client, not live inventory or a claim that the current
Console screen has server-owned device data. The declaration remains
caller-supplied and unverified, is not persisted, and is not retried after
uncertain delivery. No registry, heartbeat, enrollment, target selection,
reservation, scheduling, dispatch, Runner/process execution, or audit outbox
is added. ADR-0039 remains planning-only; ADR-0113/0114 remain Proposed/null
and P4 still requires separate Accepted execution/security governance.

### Cross-device plan §101 — Snaplink-authenticated read-only Run observation

An opt-in app-server E2E seeds one deterministic completed Run through a local
Rust Hub fixture, then uses a real Snaplink access token with the actual Rust
CLI to read the owner-scoped Run page and metadata-only timeline. A PTY TUI
performs the same Conversation/Run observation. Recorder assertions require
the CLI and TUI to issue only the expected Run reads, and timeline output is
checked to exclude prompt/output/tool payload fields. The local fixture command
produces the test Run; remote CLI/TUI surfaces remain read-only and do not
create, resume, cancel, dispatch, or execute work. No public Run write route,
device enrollment/inventory/heartbeat authority, scheduler, reservation,
Runner, artifact transfer, or audit outbox is added. ADR-0039 remains
planning-only; ADR-0113/0114 remain Proposed/null and P4 still requires
separate Accepted execution/security governance.

### Cross-device plan §102 — Flutter API observation of a populated Run

The opt-in Snaplink-authenticated Run-observation E2E now also drives the real
Flutter Console API client after a local deterministic fixture creates a
completed Run. Flutter reads the same owner-scoped Run page and metadata-only
timeline as CLI/TUI, verifies owner/conversation/run binding, completion,
sequence markers, and payload-free event types, and issues only the two
bounded GET requests allowed by the recorder. This closes the populated-Run
transport check for the shared Web/App/Mobile client path. No Run write,
resume/cancel, dispatch, device enrollment/inventory/heartbeat, scheduler,
reservation, Runner/process execution, or audit outbox is added; ADR-0039
remains planning-only, ADR-0113/0114 remain Proposed/null, and P4 still needs
separate Accepted execution/security governance.

### Cross-device plan §103 — Flutter Web browser observation of a populated Run

With the opt-in browser E2E enabled, the authenticated Snaplink Run-observation
fixture serves the real Flutter Web build and drives Chromium through `/forge/`.
The browser selects the seeded completed Run, verifies the metadata-only
timeline markers and completed status, rejects fixture/event payload text, and
issues only bounded session, Prompt, Run, timeline, and validated change-feed
reads. This proves the Web route renders the same owner-scoped read path as the
shared Flutter API client. No browser write or device enrollment/inventory/
heartbeat, scheduler, reservation, Runner/process execution, or audit outbox
is added; ADR-0039 remains planning-only, ADR-0113/0114 remain Proposed/null,
and P4 still needs separate Accepted execution/security governance.

### Cross-device plan §104 — Flutter native App/Mobile observation of a populated Run

The authenticated populated-Run E2E now cold-starts the shared Flutter Forge
gate with an injected credential backend and mounts the real
`ForgeSessionsGate`/`ForgeSessionsScreen` widget path. The native test reads the
owner Conversation, Run summary, and metadata-only timeline over the real Go
HTTP endpoint, verifies completion and terminal markers, rejects fixture/event
payload text, and issues only bounded Conversation, Prompt, Run, and timeline
reads. This is shared Flutter widget evidence for desktop App and Mobile
clients; it does not claim platform secure storage or production OAuth. No Run
write, device enrollment/inventory/heartbeat, scheduler, reservation,
Runner/process execution, or audit outbox is added; ADR-0039 remains
planning-only, ADR-0113/0114 remain Proposed/null, and P4 still needs separate
Accepted execution/security governance.

### Cross-device plan §105 — Authenticated placement and Run-intent observation binding

The Snaplink-authenticated Run-observation E2E now submits one caller-supplied
P3a placement declaration through the normal authenticated session route before
reading the same Run timeline. Go verifies exact owner, stable device result,
and all-false authority; the pure observer binds that placement to the same
Conversation/Run and validates the payload-free Prompt/Run reference with no
selected target. The recorder requires exactly one placement POST between the
Run-page and timeline reads and still rejects device, heartbeat, reservation,
dispatch, and execution paths. The placement is unverified caller data and the
Prompt receipt is a bounded test fixture, not a write route or execution intent;
ADR-0039 remains planning-only, ADR-0113/0114 remain Proposed/null, and P4
still needs separate Accepted execution/security governance.

### Cross-device plan §106 — Flutter Forge Sessions offline device observation panel

The shared Flutter Forge Sessions screen now accepts an explicit caller-supplied
P3a offline device observation. Its pure builder reuses strict inventory,
placement dry-run, session-placement binding, resource-summary, and status
projection contracts, then renders the read-only inventory panel only when the
observation's Conversation and Run IDs match the selected owner-scoped Run.
The panel exposes declared resources, Runner instances, status, exclusions, and
all-false authority without offering an action; a foreign Run is omitted.

This is injection/display evidence only. No live inventory or heartbeat route,
registry, enrollment, persistence, clock, network scan, scheduler, reservation,
dispatch, Runner/process execution, or Audit Governance outbox was added.
Normal Sessions behavior remains unchanged with no observation supplied.
ADR-0039 remains planning-only; ADR-0113/0114 remain Proposed/null and P4 still
needs separate Accepted execution/security governance.

### Cross-device plan §107 — Canonical cross-client session device observation envelope

Added the strict `forge.session-device-observation/v1` envelope for the P3a
offline path. Flutter now decodes/re-encodes the exact owner,
Conversation/Run, caller inventory, placement observation, resource summary,
null selection, and all-false authority shape, then recomputes the summary
before rendering Sessions. Go now has a bounded strict decoder/validator with
exact nested keys, duplicate/null rejection, deterministic ordering, safe
integer bounds, owner/Run binding, and aggregate recomputation. A shared
fixture and contract-script entries cover both consumers.

Rust CLI/TUI consumption and an authenticated session-bound preview route are
now complete as bounded local consumers and a stateless authenticated preview
route. No live inventory/heartbeat route,
registry, persistence, scheduler, reservation, target selection, dispatch,
Runner/process execution, or Audit Governance outbox was added. ADR-0039
remains planning-only, ADR-0113/0114 remain Proposed/null, and P4 still needs
separate Accepted execution/security governance.

### Cross-device plan §108 — Session-bound device observation preview across clients

Rust CLI/TUI now consume the canonical `forge.session-device-observation/v1`
fixture from bounded local `FILE|-` input and render only the recomputed,
unverified resource summary. Go exposes the authenticated, stateless
`POST /api/v1/conversations/{conversation}/runs/{run}/device-observation/preview`
preview with read scope and exact owner/path binding; it accepts caller
declarations only and keeps selection and authority false. The contract script
covers Go, Rust CLI/TUI, and Flutter consumers. No live inventory/heartbeat
route, registry persistence, scheduler, reservation, selection, dispatch,
Runner/process execution, or Audit Governance outbox was added. ADR-0039
remains planning-only; ADR-0113/0114 remain Proposed/null; P3b and P4 retain
their separate governance gates.

### Cross-device plan §109 — Flutter authenticated session device observation consumer

`ForgeConversationsApi.previewSessionDeviceObservation` now posts the strict
caller-supplied session placement declaration to the authenticated Go preview
route and consumes `forge.session-device-observation/v1`. Flutter validates the
request locally, then requires exact owner, Conversation/Run, evaluation time,
and device/Runner declaration parity in the response. It rejects candidate
drift, unknown/authority-bearing wire fields, and any other binding mismatch;
the single POST is never retried after uncertain delivery. Web/App/Mobile share
this API path. Sessions can now receive an explicit placement request, fetch
the preview once for the matching selected Run, and render the strictly bound
result; local/offline observation injection remains available.
No live inventory/heartbeat route, registry persistence, scheduler, reservation,
selection, dispatch, Runner/process execution, or Audit outbox was added.
ADR-0039 remains planning-only; ADR-0113/0114 remain Proposed/null; P3b and
P4 retain separate governance gates.

### Cross-device plan §110 — Snaplink-authenticated Flutter session observation E2E

The opt-in populated-Run E2E now drives the real Flutter API client through the
Snaplink-authenticated Go route with the same owner, Conversation, and Run. It
submits one caller-supplied placement declaration, consumes the strict
`forge.session-device-observation/v1` envelope, verifies the unverified
inventory/Runner binding and all-false authority, and records exactly one
session observation POST. The real `forge-runtime` binary E2E passed under
`FORGE_RUNTIME_BIN` and `FORGE_CONSOLE_E2E`; no live inventory/heartbeat,
registry, target selection, reservation, scheduling, dispatch, Runner/process
execution, or Audit outbox was added. ADR-0039 remains planning-only;
ADR-0113/0114 remain Proposed/null and P4 retains its separate Accepted
execution/security gate.

### Cross-device plan §111 — Authenticated Rust CLI/TUI session observation preview

Rust remote CLI now provides `remote session-observation preview --input FILE|-`;
the authenticated TUI provides `session-observation-preview --input FILE`.
Both post a bounded caller-supplied owner, Conversation/Run, placement, and
Runner-instance candidate declaration once to the session-bound Go preview
route. The response is required to be the canonical
`forge.session-device-observation/v1` envelope; Rust recomputes the resource
summary, binds owner/time/Conversation/Run and the exact declaration set, and
rejects unknown fields, candidate drift, selected targets, summary drift, or
any authority bit. TUI input is file-only to preserve its interactive stdin.
Focused parser, authenticated request, authority-mutation, and TUI rendering
tests pass. The opt-in Snaplink Run-observation E2E also drives the real Rust
CLI and PTY TUI and requires exactly one session observation POST per surface.
This remains stateless P3a observation and adds no live inventory,
heartbeat, enrollment, registry persistence, scheduling, reservation,
selection, dispatch, Runner/process execution, or Audit outbox. ADR-0039
remains planning-only; ADR-0113/0114 remain Proposed/null; P3b and P4 retain
their separate governance gates.

### Cross-device plan §112 — Flutter Sessions Run-intent observation card

The shared Flutter Forge Sessions screen now accepts an optional pure
`forge.run-intent-observation/v1` value and renders the existing read-only
Prompt-to-Run card only for an exact selected Conversation/Run binding. The
screen rechecks the offline schema/mode, prompt/Run/placement binding flags,
null target selection, unverified declarations, and all-false authority before
display; foreign Run values and caller-constructed authority mutations are
omitted. `ForgeSessionsGate` forwards the same guarded value for Web,
App, and Mobile. Focused widget tests cover matching, foreign, and authority
mutation cases. This is display-only and adds no write, consent/profile,
device, inventory, heartbeat, scheduler, reservation, dispatch, Runner,
process-execution, or audit-outbox behavior. ADR-0039 remains planning-only;
ADR-0113/0114 remain Proposed/null; P3b and P4 retain separate governance
gates.

### Cross-device plan §113 — Flutter strict Run-intent observation envelope consumer

`ForgeRunIntentObservation` now strictly parses and re-encodes
`forge.run-intent-observation/v1`. The consumer enforces exact keys, owner and
identifier shape, safe integer/status/count bounds, null target selection,
and an all-false authority envelope before the Sessions card receives a
value. It accepts only the fixed `v=1`/`type=device_run_intent_preview` local
Rust framing and strips it when re-encoding. Round-trip and
unknown/target/authority/overflow mutation tests pass.
This adds no public Run-intent or consent route, Prompt/Run write, device or
inventory authority, scheduler, reservation, dispatch, Runner/process
execution, or Audit outbox; ADR-0039 remains planning-only,
ADR-0113/0114 remain Proposed/null, and P3b/P4 retain their governance gates.

### Cross-device plan §114 — Snaplink-authenticated Flutter Run-intent observation E2E

The opt-in populated-Run E2E now serializes the same Go pure Run-intent
observation and injects it into the real Flutter native `ForgeSessionsGate`.
Flutter strictly consumes the envelope, rechecks the Conversation/Run binding,
and renders the read-only Run-intent card beside the metadata-only timeline.
The HTTP recorder allowlist is unchanged and no new route or write is issued.
This proves transport/presentation only for caller-supplied observation; no
public intent/consent API, live inventory, selection, reservation, scheduler,
dispatch, Runner/process execution, or audit outbox is added. ADR-0039 remains
planning-only; ADR-0113/0114 remain Proposed/null; P3b/P4 retain their
separate governance gates.

### Cross-device plan §115 — Canonical Rust offline inventory envelopes

Rust JSON output for `device inventory show --input FILE|-` now matches the
canonical `forge.device-inventory-observation/v1` envelope consumed by Go and
Flutter: CLI-only `v`/`type`/nested-authority wrappers are removed, while
human/TUI rendering stays unchanged. The local resource-summary JSON output
also removes its CLI-only wrapper fields and matches the canonical nested
`forge.device-resource-summary/v1` shape. Focused Rust CLI tests cover both
paths and keep authority false. This remains bounded caller-supplied P3a
offline data with no live registration, heartbeat, persistence, discovery,
selection, reservation, scheduling, dispatch, Runner, or audit behavior;
ADR-0039 remains planning-only, ADR-0113/0114 remain Proposed/null, and P3b
and P4 retain their separate governance gates.

### Cross-device plan §116 — Flutter import of canonical offline observations

The shared Forge Sessions Run panel now exposes a bounded process-local import
for canonical `forge.session-device-observation/v1` JSON. Web, App, and Mobile
share the same strict Flutter wire consumer: it recomputes the declared
resource summary, requires all authority bits false, and binds the imported
value to the selected Conversation/Run before rendering the read-only device
and resource panels. Malformed and foreign-Run values stay in the dialog with
generic errors; the value is not persisted, posted, retried, or used for
execution. Focused widget tests cover matching import, foreign-Run rejection,
and the no-POST guarantee. ADR-0039 remains planning-only; ADR-0113/0114
remain Proposed/null; P3b and P4 retain their separate governance gates.

### Cross-device plan §117 — Nine-instance cross-client observation E2E

The authenticated session-device preview now reuses the canonical nine-
candidate placement fixture. Go verifies stable device/Runner pairing,
byte-identical repeated output, nine inventory and placement rows, aggregate
CPU 66, memory 135168 B, storage 67584 B, and eligible counts 2/2 with no
selection or authority. The opt-in Snaplink E2E sends the same request through
Rust CLI, PTY TUI, Flutter API, and native Sessions; the Web browser imports the
canonical CLI envelope locally and renders the aggregate and candidate rows
without a preview POST. No live inventory, heartbeat, persistence, scheduler,
reservation, dispatch, Runner execution, or audit outbox was added. ADR-0039
remains planning-only; ADR-0113/0114 remain Proposed/null; P3b/P4 retain their
separate governance gates.

### Cross-device plan §118 — Rust CLI/TUI terminal receipt observation preview

The pure `forge.runner-command-terminal-receipt/v1` contract is now consumed
by `device runner-receipt-preview --input FILE|-` and the file-only TUI
`runner-receipt-preview --input FILE`. Rust reuses the bounded Runner command,
lease, fencing, digest, and terminal disposition domain checks and emits the
same metadata-only receipt projection. Completed values remain preview-only;
uncertain values require manual reconciliation and never enable automatic
retry. Unknown fields, authority mutation, digest/proof drift, and expiry fail
closed. Focused parser, canonical output, domain observation, and TUI tests
pass; no device request is issued. No Runner, inventory/heartbeat, selection,
reservation, scheduling, dispatch, process execution, or audit outbox was
added. ADR-0039 remains planning-only; ADR-0113/0114 remain Proposed/null; P3b
and P4 retain separate governance gates.

### Cross-device plan §119 — Rust CLI/TUI Runner execution-intent observation preview

The pure `forge.runner-execution-intent/v1` contract is now consumed by
bounded local `device runner-execution-intent-preview --input FILE|-` and
file-only TUI `runner-execution-intent-preview --input FILE`. Rust strictly
decodes the owner, payload-free Prompt receipt, existing Run reference,
repeated Conversation/Prompt/Run/attempt/command/target identities, and
direct-argv declaration; the domain recomputes the command digest and requires
null target selection plus all-false authority. Focused parser, canonical
output, mutation, domain, and TUI tests pass. This adds no Prompt/Run write,
Hub/clock read, lease issuance/persistence, device selection/reservation,
Runner/dispatch/process execution, or audit outbox. ADR-0039 remains
planning-only; ADR-0113/0114 remain Proposed/null; P3b and P4 retain separate
governance gates.

### Cross-device plan §120 — Flutter Runner execution-intent observation card and Web/native E2E

The shared Flutter Forge Sessions gate/screen now consumes strict canonical
`forge.runner-execution-intent/v1` observations and renders a read-only card
only for the selected Conversation/Run. The parser requires exact keys,
`pure_runner_binding_only`, valid repeated Prompt/Run/attempt/command identity,
null target selection, the idempotency tuple, and all-false Runner authority;
the card exposes metadata/digest identity without argv, output, lease, target,
or execution controls. The authenticated populated-Run native E2E injects the
same Go pure observation derived from the Rust-compatible command digest; Web
uses the same bounded process-local import and browser E2E injection without a
preview POST. Contract, widget, analyzer, Go appserver, and full
contract-script checks pass.
No public execution route, Prompt/Run write, live inventory/heartbeat,
lease persistence, selection, reservation, scheduler, dispatch,
Runner/process execution, artifact transfer, or audit outbox was added.
ADR-0039 remains planning-only; ADR-0113/0114 remain Proposed/null; P3b and
P4 retain separate governance gates.

### Cross-device plan §121 — Session-bound Runner terminal receipt observation card and E2E

The canonical `forge.session-runner-receipt-observation/v1` envelope now has
strict Go/Rust value consumers that bind the existing Runner execution-intent
observation to the existing terminal receipt projection across owner,
Conversation/Prompt/Run, attempt, command, target, and digest identities.
Flutter consumes the same envelope in `ForgeSessionsGate` and renders a
read-only receipt card only for the selected Run. Web/App/Mobile can import a
bounded JSON value into process-local state; native and browser populated-Run
E2E paths inject the Go-derived observation without a receipt POST. Completed
and uncertain receipts remain preview-only; uncertain means manual
reconciliation and `automatic_retry=false`. Unknown/foreign/drifted values,
selected targets, and every authority mutation fail closed. No public route,
receipt/lease persistence, device registration/heartbeat, selection,
reservation, scheduler, dispatch, Runner/process execution, artifact transfer,
or Audit outbox was added. ADR-0039 remains planning-only; ADR-0113/0114
remain Proposed/null; P3b and P4 retain separate governance gates.

### Cross-device plan §122 — Rust CLI/TUI session-bound Runner receipt preview

The canonical `forge.session-runner-receipt-observation/v1` envelope is now
consumed by bounded local `device session-runner-receipt-preview --input
FILE|-` and file-only TUI `session-runner-receipt-preview --input FILE`.
Rust reuses the domain validator and emits the same metadata-only receipt
projection, with no argv/output, target selection, or authority. The TUI
reserves `-` for interactive input. Focused CLI argument, canonical output,
mutation, and TUI no-device-request tests pass; the contract script includes
both surfaces. No route, Hub/clock read, receipt/lease persistence, device
registration/heartbeat, selection, reservation, scheduler, dispatch, Runner,
process execution, artifact transfer, or Audit outbox was added. ADR-0039
remains planning-only; ADR-0113/0114 remain Proposed/null; P3b and P4 retain
separate governance gates.

### Cross-device plan §123 — Authenticated session Runner receipt observation preview

The canonical `forge.session-runner-receipt-observation/v1` envelope now has a
read-only authenticated preview route at
`POST /api/v1/conversations/{conversation_id}/runs/{run_id}/runner-receipt-observation/preview`.
It requires `forge:conversations:read`, strict JSON, exact path/session
identity, and owner equality with the verified Snaplink principal. Go
revalidates nested receipt identity/state, null target selection, and every
all-false authority bit before echoing canonical JSON. Flutter's
`ForgeConversationsApi.previewSessionRunnerReceiptObservation` performs the
same local display-only/path checks and rejects owner or response drift.
Focused Go route tests cover deterministic echo, foreign owner/path, scope,
method/query, malformed authority/selection, and the unchanged `/devices`
404 boundary; Flutter API tests cover request shape and response owner drift.
This is only a caller-supplied display bridge: it does not read Hub state or a
clock, persist receipts or leases, register/heartbeat devices, select or
reserve capacity, schedule, dispatch, contact a Runner, execute a process, or
publish an Audit outbox. ADR-0039 remains planning-only; ADR-0113/0114 remain
Proposed/null; P3b and P4 retain separate governance gates.

### Cross-device plan §124 — Authenticated Rust CLI/TUI session Runner receipt observation preview

Rust now consumes `forge.session-runner-receipt-observation/v1` through
`remote session-runner-receipt preview --input FILE|-` and the authenticated
TUI command `session-runner-receipt-preview --input FILE`. The client posts the
bounded canonical envelope once to the existing session Runner receipt preview
route and strictly validates the canonical echo, owner/path/session binding,
nested receipt state, null target, and all-false authority.
`session-runner-receipt-offline-preview --input FILE` preserves the prior
file-only TUI inspection path, with `-` reserved for the interactive input.
Parser, HTTP, response-mutation, TUI route, and no-device-request tests are
in the contract script. No Hub/clock read, Run creation, persistence, device
selection, reservation, scheduling, dispatch, Runner/process execution, or
public inventory/execution route was added; ADR-0039 remains planning-only,
ADR-0113/0114 remain Proposed/null, and P3b/P4 retain separate governance
gates.

### Cross-device plan §125 — Snaplink authenticated Rust receipt preview E2E

The opt-in populated-Run Snaplink E2E now drives the real Rust CLI and PTY TUI
through the authenticated session Runner receipt preview route. Each surface
posts the same bounded `forge.session-runner-receipt-observation/v1` value once;
the recorder and response checks require owner/Conversation/Prompt/Run,
command/digest, null target, metadata-only output, and all-false authority.
The TUI rejects command payload text and device paths. This is transport
evidence only: no Run/receipt/lease persistence, device registration or
heartbeat, selection, reservation, scheduler, dispatch, Runner execution,
artifact transfer, or Audit outbox was added. ADR-0039 remains planning-only,
ADR-0113/0114 remain Proposed/null, and P3b/P4 retain separate gates.

### Cross-device plan §126 — Snaplink authenticated Flutter receipt preview E2E

The opt-in populated-Run Flutter API and native Sessions E2E now consume the
same canonical `forge.session-runner-receipt-observation/v1` value as Rust.
The API client posts once to the authenticated receipt preview route and
verifies the exact Conversation/Prompt/Run and command digest bindings; the
native Sessions path renders the metadata-only card for the selected Run. The
recorder allowlist contains that one receipt POST and still excludes device
and execution routes. This remains display transport evidence: no Run,
receipt/lease persistence, device registration/heartbeat, selection,
reservation, scheduler, dispatch, Runner execution, artifact transfer, or
Audit outbox was added. ADR-0039 remains planning-only, ADR-0113/0114 remain
Proposed/null, and P3b/P4 retain separate gates.

### Cross-device plan §127 — Snaplink authenticated Flutter Web receipt preview E2E

The opt-in populated-Run browser E2E now drives the real Flutter Web build in
Chromium, imports the canonical session Runner receipt card for the selected
Run, and sends the same bounded envelope through the browser's authenticated
receipt preview request. The browser validates the `200` canonical echo and
metadata-only card, while the Go recorder requires exactly one receipt preview
POST and still rejects device, inventory, and execution routes. Release Web
JSON may materialize an integral number as `double`, so the receipt parser
accepts only a finite exact integer within the platform-safe bound; fractional
or rounded values remain rejected. No Run,
receipt/lease persistence, device registration/heartbeat, selection,
reservation, scheduler, dispatch, Runner execution, artifact transfer, or
Audit outbox was added; ADR-0039 remains planning-only, ADR-0113/0114 remain
Proposed/null, and P3b and P4 retain separate gates.

### Cross-device plan §128 — Snaplink authenticated Flutter Web session device observation preview E2E

The opt-in populated-Run browser E2E now posts the caller-supplied
`forge.session-device-observation/v1` request once to the authenticated
`device-observation/preview` route, validates the canonical response's exact
Conversation/Run binding, null selected device/instance, and all-false
authority, then imports that response into the Web Sessions panel. The Go
recorder requires exactly one device-observation POST alongside the receipt
preview POST and continues to reject live device, inventory, and execution
routes; browser waits scroll through the long nine-instance declaration panel
so later metadata cards remain observable. The browser also mutates a local
receipt selection, verifies the Flutter import error, then reimports the
canonical value without a second preview request. This is P3a transport evidence
only: no registry/heartbeat persistence, enrollment, selection, reservation,
scheduler, dispatch, Runner/process execution, artifact transfer, or Audit
outbox was added; ADR-0039 remains planning-only, ADR-0113/0114 remain
Proposed/null, and P3b/P4 retain separate gates.

### Cross-device plan §129 — Flutter Web session device observation fail-closed import E2E

The populated-Run browser path now mutates the canonical session device observation locally by setting `selected_device_id`, verifies the Flutter import error, closes the failed dialog, and successfully reimports the canonical response. The negative import is process-local and does not issue another device-observation preview request; the recorder still observes exactly one authenticated POST. This closes the Web consumer's selected-target fail-closed check for the P3a display envelope. No live inventory/heartbeat persistence, enrollment, selection, reservation, scheduler, dispatch, Runner/process execution, artifact transfer, or Audit outbox was added; ADR-0039 remains planning-only, ADR-0113/0114 remain Proposed/null, and P3b/P4 retain separate gates.

### Cross-device plan §130 — Flutter Prompt byte preservation across retry

Flutter now uses trimming only to reject an all-whitespace Prompt; non-empty leading spaces and trailing newlines remain byte-preserved through the authenticated append request and the pending-write retry with one idempotency key. The Sessions widget regression covers the whitespace-bearing Prompt and both attempts. No Run creation, device inventory/heartbeat, target selection, reservation, scheduler, dispatch, Runner execution, artifact transfer, or Audit outbox was added; ADR-0039 remains planning-only, ADR-0113/0114 remain Proposed/null, and P3b/P4 retain separate gates.

### Cross-device plan §131 — Snaplink authenticated Flutter Web Prompt byte preservation E2E

The independent-client E2E now sends a browser Prompt with leading spaces and a trailing newline through the real Flutter Web route; a Go client reads the Rust Hub record and compares the exact content bytes. The browser recorder still permits only bounded session/history reads and one Prompt POST, with no Run/device/inventory/scheduling route. This verifies Web semantics against Flutter retry behavior and the CLI/TUI/API contract; no execution authority or device effects were added, and ADR-0039/0113/0114 plus P3b/P4 gates remain unchanged.

### Cross-device plan §132 — Authenticated Flutter-to-Rust Prompt byte parity E2E

The shared-session E2E now drives the real Flutter API/native test path with Prompt values containing leading spaces and trailing newlines. Rust CLI history reads both values exactly, and the PTY TUI re-renders the same JSON-escaped values from the owner-scoped Conversation. This extends §130's local retry evidence across Flutter, Hub, CLI, and TUI without creating a Run or authorizing device work; ADR-0039/0113/0114 and P3b/P4 gates remain unchanged.

### Cross-device plan §133 — Flutter Forge foreground refresh coalescing across platforms

The shared Flutter Sessions screen now coalesces overlapping foreground refreshes. Android/iOS lifecycle delivery and desktop window integrations can report another `resumed` notification while the prior owner feed/session/Run reads are pending; one in-flight resume refresh is retained and a later foreground cycle starts only after it finishes. The widget regression uses Flutter's valid inactive/hidden/paused/resumed sequence, starts a second foreground cycle while the first feed read is blocked, and proves one change-feed request plus one follow-up session refresh. Focused Sessions tests, targeted analyzer, Web release build, and Android debug APK build pass. This is read-only lifecycle coordination and adds no Run write, device inventory/heartbeat, selection, reservation, scheduling, dispatch, Runner/process execution, artifact transfer, or Audit outbox; ADR-0039 remains planning-only, ADR-0113/0114 remain Proposed/null, and P3b/P4 retain separate gates.

### Cross-device plan §134 — Flutter session device observation rejects selected instances

The shared Web/App/Mobile offline observation import now rejects a caller mutation of `selected_instance_id` with `Invalid offline device observation.` and leaves no observation panel rendered. The regression complements the selected-device mutation check and proves the `(device_id, instance_id)` pair remains display-only without a request. This remains a local P3a validation boundary: no live inventory/heartbeat persistence, selection, reservation, scheduler, dispatch, Runner/process execution, artifact transfer, or Audit outbox was added; ADR-0039 remains planning-only, ADR-0113/0114 remain Proposed/null, and P3b/P4 retain separate governance gates.

### Cross-device plan §135 — Flutter Forge session deep-link selection

The Forge route now passes `/forge/conversations/{conversation_id}` into the authenticated Sessions screen. If the requested owner-scoped Conversation is outside the first keyset page, Flutter performs one bounded detail read, merges the exact result, and loads its Prompt and Run metadata; refreshes preserve the selected owner session across page-boundary changes. The deep-link widget regression verifies the request sequence and owner-scoped selection. This is read-only P2 navigation and observation: no Run/Prompt write, inventory/heartbeat persistence, target selection/reservation, scheduling, dispatch, Runner execution, artifact transfer, or Audit outbox was added; ADR-0039 remains planning-only, ADR-0113/0114 remain Proposed/null, and P3b/P4 retain separate gates.

### Cross-device plan §136 — Flutter Forge session selection URL synchronization

Selecting another owner-scoped Conversation now updates the same-document
`/forge/conversations/{conversation_id}` route with
`BrowserNavigation.replaceState`. The authenticated Sessions screen remains
mounted and no navigation reload or extra route read is triggered, while a
refresh or bookmark can restore the selected session through the existing
owner-scoped deep-link path. A targeted widget regression checks the canonical
path and the absence of a non-GET request. The URL remains a selection hint;
the authenticated list/detail read remains authoritative. This is read-only
P2 navigation continuity and adds no Run/Prompt write, device
inventory/heartbeat, target selection/reservation, scheduling, dispatch,
Runner execution, artifact transfer, or Audit outbox; ADR-0039/0113/0114 and
P3b/P4 gates remain unchanged.

### Cross-device plan §137 — Flutter Forge same-document history selection recovery

The authenticated Forge Sessions screen now listens to
`BrowserNavigation.listenToLocationChange` and restores an owner-scoped
Conversation after same-document push/replace or back/forward changes. Local
first-page selections reuse the existing owner-filtered object; older deep
links resolve through the authenticated detail fallback. Returning to `/forge`
clears local selection metadata, duplicate URL events are coalesced, stale
location work cannot overwrite a newer selection, and the listener is removed
on dispose. Widget coverage includes local recovery, back navigation, detail
fallback, duplicate reads, and disposal. This remains read-only P2 navigation:
no gate rebuild, Prompt/Run write, inventory/heartbeat persistence,
target selection/reservation, scheduling, dispatch, Runner execution,
artifact transfer, or Audit outbox; ADR-0039/0113/0114 and P3b/P4 gates remain
unchanged.

### Cross-device plan §138 — Flutter Forge gate restores the current route

`ForgeSessionsGate` now treats the entry route as a bootstrap hint while the
Forge credential record is restored. It re-reads `BrowserNavigation.currentUri`
after the asynchronous restore, so a deep link entered during restore wins and
a return to `/forge` clears the stale entry selection before constructing the
Sessions screen. Delayed secure-store widget regressions cover both transitions
through a route-aware test builder. This is read-only P2 route continuity; the
authenticated owner-scoped read remains authoritative and no Prompt/Run write,
inventory or heartbeat persistence, target selection/reservation, scheduling,
dispatch, Runner execution, artifact transfer, or Audit outbox was added.
ADR-0039/0113/0114 and the P3b/P4 governance gates remain unchanged.

### Cross-device plan §139 — Create a bookmarkable Forge session selection

After the authenticated create response succeeds, Forge now updates the
same-document `/forge/conversations/{conversation_id}` URL through the same
selection helper used by list taps. A transport failure leaves the existing
`/forge` or prior session URL intact while the pending create is retried; only
the confirmed server Conversation changes the route. The replacement is
handled in place and causes no extra list/detail, Prompt, or Run read. Widget
coverage verifies success and failure/retry behavior. This remains read-only
route continuity around the existing Conversation write; no inventory or
heartbeat persistence, target selection/reservation, scheduling, dispatch,
Runner execution, artifact transfer, or Audit outbox was added.
ADR-0039/0113/0114 and P3b/P4 governance gates remain unchanged.

### Cross-device plan §140 — Rust TUI owner-scoped detail fallback

Rust TUI `open` and `detail/show` now resolve a Conversation outside the
currently loaded page through the existing authenticated owner-scoped detail
GET. `open` continues to load Prompt history and `detail` renders the validated
metadata-only projection; loaded-page `open` keeps the zero-extra-GET fast
path. A successful fallback commits `selected_entry` only after ID/owner
validation, while a missing or foreign lookup preserves the previous selection
and history. Focused PTY-style tests cover both commands and failed lookup
preservation. This is read-only CLI/TUI session continuity and adds no device
registration/heartbeat, live inventory authority, target selection/reservation,
scheduling, dispatch, Runner execution, artifact transfer, or Audit outbox.
ADR-0039/0113/0114 and P3b/P4 governance gates remain unchanged.

### Cross-device plan §141 — Ecosystem revalidation and outbound-audit boundary

Rechecked the user-named Snaplink, Console, Aero-ID, Aero-IM, Aero-Vault, and
Audit Governance repositories. Console Agent Hub's device/task APIs are a
separate scheduler: their instance/project/task data has no Forge owner,
Conversation/Prompt/Run/Attempt, key-proof, sequence/freshness, lease/fencing,
or Runner-receipt binding, so it cannot become Forge inventory. Snaplink's
authenticated Forge client evidence remains human identity only; Aero-ID
projection, Aero-IM delivery, and Aero-Vault objects stay downstream and do
not provide Runner authority. Audit Governance's real event endpoint requires
registered tenant source bindings, credentials, bounded receipts, and a relay;
Forge lacks the approved registration/outbox/redaction lifecycle.

The existing content-free `forge.prompt.accepted.v1` remains contract-only.
`auditprojection` now has an AST-based negative boundary test rejecting
network/database/process/runtime imports and publisher/dispatch/enrollment
entry points, ensuring an ungoverned Audit relay cannot appear beside the pure
projection; the Forge contract script executes it. No external call,
persistence, device/heartbeat API, inventory,
reservation, scheduler, dispatch, Runner execution, or Audit publication was
added. ADR-0039 is still planning-only; ADR-0113/0114 remain Proposed/null;
P3b/P4 remain gated.

### Cross-device plan §142 — Flutter failed deep-link selection preservation

The shared Flutter Web/App/Mobile Sessions screen now treats a failed
owner-scoped detail fallback as a selection failure rather than a failed
Conversation-list read. A missing, foreign, malformed, or unavailable
same-document deep link keeps the last successful list, current selection,
and rendered Prompt/Run state, reports the detail error, and performs no
Prompt/Run read for the inaccessible ID. The URL remains an untrusted hint and
the failed ID is never selected. Authorization failures retain the existing
fail-closed credential and visible-owner-data clearing path. Focused widget
coverage verifies state preservation and the exact list/detail request bound;
the existing authorization regression remains green. This is P2 read and
navigation resilience only and adds no inventory/heartbeat persistence,
target selection/reservation, scheduler, dispatch, Runner execution, artifact
transfer, or Audit outbox. ADR-0039/0113/0114 and P3b/P4 governance gates
remain unchanged.

### Cross-device plan §143 — Rust TUI renders per-instance resource declarations

Authenticated TUI session-device observation output now renders each placement
decision as `device_id/instance_id`, with the bounded caller-declared CPU,
memory, storage, GPU presence, and GPU memory values. Focused PTY-style
coverage verifies eligible and excluded instance rows and the absence of
device-route requests. This remains a display projection of unverified P3a
input: no live inventory/heartbeat persistence, registration, target
selection/reservation, scheduler, dispatch, Runner execution, artifact
transfer, or Audit outbox was added. ADR-0039 remains planning-only,
ADR-0113/0114 remain Proposed/null, and P3b/P4 remain gated.

### Cross-device plan §198 — Go/Rust persisted inventory placement-input parity

Forge Core and Rust Runtime now convert one persisted, owner-bound inventory
value into a strict pure placement input without a route, database, clock, or
policy evaluation. The conversion preserves only persisted owner/device/Runner
state and canonical capabilities, rejects foreign owners and Runner/device
binding drift, and keeps the owner tuple unverified. Residency, trust,
sandbox, and concurrency are absent from persistence and are carried as empty
or unknown values, so an enabled policy fails closed. The shared strict
`forge-device-inventory-placement-input-v1` fixture covers online, stale,
expired, pending, cordoned, revoked, offline, owner mismatch, binding
mismatch, and missing-policy-attribute cases; all authority fields remain
false. No selection, reservation, dispatch, Runner execution, or production
authority changed.

### Cross-device plan §144 — Flutter clears imported Runner observations across session selection

Process-local Runner execution-intent and session Runner receipt observations
are now scoped to the selected Conversation/Run. Selecting another
Conversation, clearing the same-document `/forge` selection, creating a new
Conversation, or observing a different Run clears those imports before later
reads can complete. A widget regression imports both display-only cards,
switches to a second owner-scoped session, and returns to the first without
resurrecting stale cards. This is local display-state hygiene only; no Prompt
or Run write, inventory/heartbeat persistence, target selection/reservation,
scheduler, dispatch, Runner execution, artifact transfer, or Audit outbox was
added. ADR-0039/0113/0114 and P3b/P4 governance gates remain unchanged.

### Cross-device plan §145 — Rust TUI clears owner data after authorization rejection

The authenticated Rust TUI now clears its owner-scoped Conversation list,
selection, Prompt history, pagination cursor, and pending write state after an
HTTP 401/403 from a session read or write. A focused PTY-style regression
loads a session and Prompt history, receives 403 from the Run read, and proves
the final render contains neither the prior Conversation nor Prompt. Missing,
conflict, and transient responses retain the trusted view. This aligns the
CLI/TUI fail-closed read behavior with Flutter's existing authorization path;
it adds no credential revocation, device/inventory persistence, target
selection, scheduling, dispatch, Runner execution, or Audit outbox. ADR-0039
remains planning-only, ADR-0113/0114 remain Proposed/null, and P3b/P4 remain
gated.

### Cross-device plan §146 — Strict nested session device-observation request boundary

The authenticated Go session device-observation preview now routes its
caller-supplied body through a bounded strict decoder that requires the full
nested placement, requirements, GPU, device, and candidate shape. This aligns
the authenticated observation request with the standalone placement decoder;
missing nested fields can no longer become valid Go zero values. Package and
route regressions reject an omitted placement `gpu` object before evaluation.
The slice remains stateless P3a observation and adds no live inventory,
registration/heartbeat persistence, target selection/reservation, scheduler,
dispatch, Runner execution, artifact transfer, or Audit outbox. ADR-0039
remains planning-only, ADR-0113/0114 remain Proposed/null, and P3b/P4
governance gates remain unchanged.

### Cross-device plan §147 — Flutter Forge sign-out hides owner data before cleanup

Flutter Forge Sessions now invalidates and hides the owner-scoped Conversation,
Prompt, Run, and imported observation state immediately when device-scoped
sign-out begins. The change-feed timer stops while Snaplink revocation and
secure-store cleanup finish; a failed secure-store delete leaves the owner
view hidden and exposes an explicit retry state. The native-route regression
seeds a Conversation and Prompt, blocks revocation, and proves neither remains
visible during pending sign-out. This is client lifecycle hygiene only: no
device registration/heartbeat, inventory, target selection, reservation,
scheduling, dispatch, Runner execution, artifact transfer, or Audit outbox was
added. ADR-0039 remains planning-only, ADR-0113/0114 remain Proposed/null,
and P3b/P4 remain gated.


### Cross-device plan §148 — Flutter refreshes a replaced session device observation

The shared Web/App/Mobile Sessions State now handles a parent replacing the
caller-supplied session device-observation declaration without recreating the
screen. It clears the previous fetched/imported value, invalidates the old
generation, and performs one bounded preview for the new selected
Conversation/Run; equivalent declarations do not trigger another request. A
widget regression updates the mounted screen from no request to a matching
request and verifies the authenticated preview plus per-instance panel. This
remains P3a display state only: no live inventory/heartbeat persistence,
registration, selection, reservation, scheduling, dispatch, Runner execution,
artifact transfer, or Audit outbox was added. ADR-0039 remains planning-only,
ADR-0113/0114 remain Proposed/null, and P3b/P4 remain gated.

### Cross-device plan §149 — Rust TUI clears owner data after placement preview authorization failure

The authenticated Rust TUI placement preview now clears its owner-scoped
Conversation list, selected entry, Prompt history, pagination cursor, and
pending write state after HTTP 401/403, matching the existing session, Run,
and session-observation read behavior. A focused PTY-style regression loads a
private session, returns 403 from the stateless placement preview route, and
proves the next render contains no stale owner data. This is P2 client-state
hygiene around a P3a display-only preview; no live inventory/heartbeat,
registration, target selection/reservation, scheduling, dispatch, Runner
execution, artifact transfer, or Audit outbox was added. ADR-0039 remains
planning-only, ADR-0113/0114 remain Proposed/null, and P3b/P4 remain gated.

### Cross-device plan §150 — Flutter rejects duplicate Runner instance declarations

The Flutter offline device-inventory decoder now rejects duplicate
`instance_id` values, matching the existing Go and Rust strict contracts. A
fixture regression mutates two device rows to share one Runner instance and
proves the page is rejected before resource aggregation or rendering. This is
P3a contract parity for caller-supplied display data; it adds no live
inventory/heartbeat persistence, registration, target selection/reservation,
scheduling, dispatch, Runner execution, artifact transfer, or Audit outbox.
ADR-0039 remains planning-only, ADR-0113/0114 remain Proposed/null, and P3b/P4
remain gated.

### Cross-device plan §151 — Flutter stops owner polling after authorization invalidation

The shared Flutter Web/App/Mobile Forge Sessions screen stops its
foreground owner change-feed timer immediately after an authenticated read or
write receives HTTP 401/403. Existing fail-closed cleanup still clears the
Conversation, Prompt, Run, and process-local observation state and the Forge
credential slot; a widget regression advances beyond one polling interval and
verifies that no further session read is sent. This is P2 client lifecycle
hygiene only: no live inventory/heartbeat persistence, registration, target
selection/reservation, scheduling, dispatch, Runner execution, artifact
transfer, or Audit outbox was added. ADR-0039/0113/0114 and the P3b/P4 gates
remain unchanged.

### Cross-device plan §152 — Rust CLI rejects duplicate device and Runner instance declarations

The Rust `device inventory show` decoder now tracks device and Runner instance
identities independently. Repeated `device_id` or repeated `instance_id`
declarations fail before sorting and output, matching the Flutter, Go, and
authenticated TUI boundaries; CLI coverage exercises both mutations. This is
P3a offline contract parity only. No live inventory/heartbeat persistence,
registration, target selection/reservation, scheduling, dispatch, Runner
execution, artifact transfer, or Audit outbox was added. ADR-0039 remains
planning-only, ADR-0113/0114 remain Proposed/null, and P3b/P4 remain gated.

### Cross-device plan §153 — Flutter keeps authorization-invalidated polling disabled across resume

The shared Flutter Web/App/Mobile Forge Sessions screen keeps owner polling
disabled after an authenticated 401/403, including a later `resumed` lifecycle
event. The timer start path, foreground refresh, and change-feed poll all honor
the authorization-invalidated state, and the widget regression proves that
neither the Conversation snapshot nor change feed is requested after resume.
This remains P2 client lifecycle hygiene only; no live inventory/heartbeat,
registration, target selection/reservation, scheduling, dispatch, Runner
execution, artifact transfer, or Audit outbox was added. ADR-0039/0113/0114
and the P3b/P4 gates remain unchanged.

### Cross-device plan §154 — Rust rejects duplicate Aero-ID projection keys

The Rust domain consumer for the pure `forge.aero-id-profile-projection/v1`
fixture now performs a recursive duplicate-object-key scan before
`serde_json` materializes the projection. This matches the Go strict decoder
and prevents a caller from using a later duplicate value to alter an owner,
profile, membership, or authority field. Root and nested duplicate-key
regressions pass; bounded shape, unknown fields, membership order, owner
binding, and all-false authority checks remain active. This is pure P5
projection parity only: no Aero-ID call, token forwarding, profile
persistence, Forge authorization, device registration/heartbeat, inventory,
selection, reservation, scheduling, dispatch, Runner execution, artifact
transfer, or Audit outbox was added. ADR-0039 remains planning-only,
ADR-0113/0114 remain Proposed/null, and P3b/P4 remain gated.

### Cross-device plan §155 — Rust CLI/TUI rejects duplicate JSON keys in offline inventory snapshots

The Rust `device inventory snapshot-canonical` input boundary now recursively
rejects duplicate object keys before `serde` materializes the bounded
caller-supplied fixture. Root and nested duplicate-key regressions prove
fail-closed behavior while preserving bounded input, unknown-field,
case-identity, canonical ordering, digest, and all-false authority checks. The
authenticated TUI reuses this same local command boundary; no live
inventory/heartbeat persistence, registration, selection, reservation,
scheduling, dispatch, Runner execution, artifact transfer, or Audit outbox was
added. ADR-0039 remains planning-only, ADR-0113/0114 remain Proposed/null, and
P3b/P4 remain gated.

### Cross-device plan §156 — Rust TUI explicit local Conversation import

The interactive Rust TUI now accepts `import LOCAL_CONVERSATION_ID
[--confirm SHA256]`, reusing the CLI's bounded ownerless local Hub loader,
target binding, digest preview, idempotent import request, and result validation.
No confirmation only renders the visible user/assistant Prompt preview; an
exact current lowercase digest is required before upload, and a mismatch makes
no request. The optional `--state-dir` reaches the TUI, and 401/403 import
failures clear the owner session view. This closes the explicit P1/P2 TUI
session-continuity parity gap without adding Run execution, device
registration/heartbeat, live inventory, selection, reservation, scheduling,
dispatch, Runner execution, artifact transfer, or Audit outbox. ADR-0039
remains planning-only, ADR-0113/0114 remain Proposed/null, and P3b/P4 remain
gated.

### Cross-device plan §157 — Rust CLI/TUI heartbeat persistence CAS preview

The Rust CLI `device heartbeat-persistence-preview --input FILE|-` and
authenticated TUI `heartbeat-persistence-preview --input FILE` consume the
shared heartbeat persistence fixture through the pure domain CAS evaluator.
They restore caller-supplied snapshots, compare replacement fields or bounded
error names, reject duplicate JSON keys/unknown fields/authority mutations,
and preserve all-false authority. The slice performs no storage, clock,
network, enrollment, inventory publication, target selection, reservation,
scheduling, dispatch, Runner, or Audit action; ADR-0039 remains
planning-only, ADR-0113/0114 remain Proposed/null, and P3b/P4 remain gated.

### Cross-device plan §158 — Rust CLI/TUI identity proof binding preview

The Rust `device identity-proof-preview --input FILE|-` CLI and file-only
TUI command now consume the shared identity-proof fixture through the pure
binding evaluator. Owner, device, key, challenge, approval, credential, and
validity cases are checked with bounded input, strict unknown/duplicate-key
rejection, expected-result matching, and all-false authority output. No
cryptography, key material, challenge consumption, persistence, enrollment,
network, live inventory, target selection/reservation, scheduling, dispatch,
Runner execution, or Audit outbox was added; ADR-0039 remains planning-only,
ADR-0113/0114 remain Proposed/null, and P3b/P4 remain gated.

### Cross-device plan §159 — Rust identity proof envelope parity

The Rust CLI/TUI identity-proof consumer now applies the same fixed envelope
checks as Go and Flutter: exact notice/schema mode, matching owner declaration,
canonical base device/key and approved-active state, unconsumed base challenge,
twelve unique cases, and all-false authority. Envelope mutations fail before
case evaluation. This remains strict P3a fixture parity only; no cryptography,
key material, challenge consumption, persistence, enrollment, network, live
inventory, target selection/reservation, scheduling, dispatch, Runner
execution, or Audit outbox was added. ADR-0039 remains planning-only,
ADR-0113/0114 remain Proposed/null, and P3b/P4 remain gated.

### Cross-device plan §160 — Rust CLI/TUI inventory row bound parity

Rust `device inventory show` now rejects more than 128 caller-supplied
device/Runner-instance declarations, matching Go, Flutter, resource-summary,
and session-observation consumers. CLI coverage builds a valid 129-row
observation and proves fail-closed behavior; the authenticated TUI reuses the
same local boundary and emits no device request. This remains P3a offline
hygiene with no live discovery, registration, persistence, inventory
publication, target selection/reservation, scheduling, dispatch, Runner
execution, or Audit outbox; ADR-0039 remains planning-only, ADR-0113/0114
remain Proposed/null, and P3b/P4 remain gated.

### Cross-device plan §161 — Cross-client Prompt idempotency replay

Owned SQLite Prompt retries now resolve the original `prompt_appended`
aggregate version instead of returning the current Conversation head. This
keeps Prompt identity/content/version stable when Flutter's idempotency key is
replayed later by Rust CLI; infrastructure coverage exercises a later write,
and the shared Flutter Console → Rust CLI E2E proves one history entry and
stable change-feed versions. This is P1/P2 retry parity only: no device
registration/heartbeat, live inventory, target selection/reservation,
scheduling, dispatch, Runner execution, artifact transfer, or Audit outbox was
added. ADR-0039 remains planning-only, ADR-0113/0114 remain Proposed/null,
and P3b/P4 remain gated.

### Cross-device plan §162 — Rust CLI/TUI inventory safe-integer parity

Rust `device inventory show` now rejects evaluation, snapshot, lease, memory,
storage, and GPU-memory declarations above the shared
`9_007_199_254_740_991` JavaScript/Dart safe integer limit. The authenticated
TUI reuses the same decoder, and focused CLI coverage mutates every numeric
field independently. This remains P3a offline declaration hygiene with no
live discovery, registration, persistence, inventory authority, target
selection/reservation, scheduling, dispatch, Runner execution, artifact
transfer, or Audit outbox; ADR-0039 remains planning-only, ADR-0113/0114
remain Proposed/null, and P3b/P4 remain gated.

### Cross-device plan §163 — Owner-bound Run timeline checkpoint/resume

Rust CLI/TUI now expose explicit metadata-only `remote runs timeline ...
--resume` / `timeline RUN_ID --resume` paths backed by a private checkpoint
bound to coordinator, issuer, client, owner, Conversation, and Run; manual
`--after-sequence` remains one-off. Flutter persists the same sequence-only
owner-bound checkpoint and resumes after cold start or Run re-entry. Only a
validated dense page advances it; malformed, failed, stale, or unauthorized
reads do not. No token, Prompt body, event payload, device declaration, or
execution data is persisted. This is P2 reconnect hygiene only: no live event
delivery, Run content, device registration/heartbeat, inventory authority,
target selection/reservation, scheduling, dispatch, Runner execution,
artifact transfer, or Audit outbox was added. ADR-0039 remains planning-only,
ADR-0113/0114 remain Proposed/null, and P3b/P4 remain gated.

### Cross-device plan §164 — Flutter Native populated-Run remount resume evidence

The real Flutter Web/App/Mobile widget E2E mounts a populated completed Run,
reads its metadata timeline from sequence zero, unmounts the route, and mounts
it again with the same persistent credential store. The remount receives no
device declaration and resumes from the owner-bound positive checkpoint; the
Go recorder requires one zero cursor, one positive cursor, and one device
observation preview POST. This is P2 reconnect evidence only: no live device,
inventory, scheduling, dispatch, Runner, artifact, or Audit action was added;
ADR-0039 remains planning-only, ADR-0113/0114 remain Proposed/null, and P3b/P4
remain gated.

### Cross-device plan §165 — Go Runner execution-intent command digest parity

Go `ObserveRunnerExecutionIntent` now recomputes the Rust-compatible
`forge.runtime.runner-command.v1` digest from the supplied direct-argv command
and rejects a detached or mismatched `execution_intent.command_sha256`; a
matching digest remains accepted. This is pure P4 preparation only: no device
verification, lease, selection, reservation, scheduling, dispatch, Runner,
artifact, or Audit action was added. ADR-0039 remains planning-only,
ADR-0113/0114 remain Proposed/null, and P3b/P4 remain gated.

### Cross-device plan §166 — Cross-client Run JSON-safe integer parity

Run page/timeline cursors, summaries, timeline pages, metadata-only resume
checkpoints, Rust CLI/TUI parsers, runtime RPC, and Go appserver/runtime bridge
now reject values above the shared JSON-safe integer ceiling
`9_007_199_254_740_991` while accepting the ceiling itself. Focused tests cover
request/response, parser, runtime-RPC, checkpoint, and Go route boundaries so
Flutter/Web/Rust/Go cannot silently diverge on large Run sequence or timestamp
values. This remains P2/P3a transport and reconnect hygiene only; no live
device registration/heartbeat, authoritative inventory, selection,
reservation, scheduling, dispatch, Runner, artifact, or Audit action was added;
ADR-0039 remains planning-only, ADR-0113/0114 remain Proposed/null, and P3b/P4
remain gated.

### Cross-device plan §167 — Run summary invariant parity

Rust remote Run-page validation now rejects zero `latest_sequence` and
repeated `run_id` values within one response page, matching Go runtime bridge
validation while retaining the shared JSON-safe number ceiling. Focused Rust
and Go regressions cover zero-sequence, duplicate-summary, cursor, and
metadata timeline cases. This is read-only P2/P3a response hygiene only; no
Run/device mutation, live registration/heartbeat, authoritative inventory,
selection, reservation, scheduling, dispatch, Runner, artifact, or Audit
operation was added. ADR-0039 remains planning-only, ADR-0113/0114 remain
Proposed/null, and P3b/P4 remain gated.

### Cross-device plan §168 — Flutter Web Run checkpoint reload evidence

The real Chromium Web E2E opens a populated Run, reads its metadata timeline
from sequence zero, reloads `/forge/` with the same tab credential, and
re-enters the Run using the persisted owner-bound positive cursor. The remount
shows no timeline markers and no duplicate `run_started`/`run_finished` values;
the recorder accepts the initial zero cursor followed only by positive cursors
from bounded refreshes. The same path continues to validate caller-supplied
device observation and session Runner receipt previews. This is P2/P3a evidence
only: no live enrollment/heartbeat, authoritative inventory, selection,
reservation, scheduling, dispatch, Runner, artifact, or Audit action was added;
ADR-0039 remains planning-only, ADR-0113/0114 remain Proposed/null, and P3b/P4
remain gated.

### Cross-device plan §169 — Rust TUI owner-bound Run timeline resume process evidence

A real two-process PTY integration now runs authenticated Rust TUI with a
private saved credential: process one reads the populated Run timeline from
sequence zero and persists the validated positive cursor; process two resumes
with the same owner/coordinator/Conversation/Run binding and reads an empty
page without replaying timeline markers. The recorder asserts only
owner-scoped Conversation, Prompt, Run, and timeline GETs; no device,
inventory, Runner, or execution request is made. This remains P2 reconnect
evidence only; ADR-0039 remains planning-only, ADR-0113/0114 remain
Proposed/null, and P3b/P4 remain gated.

### Cross-device plan §170 — Cross-client Prompt JSON-safe integer parity

Prompt page timestamps and (created_at_ms, prompt_id) cursors now use the
shared JSON-safe ceiling 9_007_199_254_740_991 across Go HTTP/runtime
bridge, Rust runtime RPC/CLI, and Flutter API/model boundaries. The ceiling is
accepted while +1 fails closed at request and response edges; Hub SQLite
storage keeps its existing signed-integer range. This remains P1/P2
pagination hygiene only, with no live inventory, scheduling, dispatch,
Runner, artifact, or Audit operation; ADR-0039 remains planning-only,
ADR-0113/0114 remain Proposed/null, and P3b/P4 remain gated.

### Cross-device plan §171 — Cross-client Conversation JSON-safe timestamp parity

Conversation `created_at_ms` and `updated_at_ms` now use the shared
JSON-safe ceiling `9_007_199_254_740_991` across Go runtimebridge/HTTP,
Rust remote/RPC, and Flutter model boundaries. The ceiling is accepted and
`+1` fails closed; Rust RPC covers snapshot, bootstrap, and owned
create/import/list/detail responses, while Hub SQLite storage remains
unchanged. This is P1/P2 transport hygiene only, with no live inventory,
scheduling, dispatch, Runner, artifact, or Audit operation; ADR-0039 remains
planning-only, ADR-0113/0114 remain Proposed/null, and P3b/P4 remain gated.

### Cross-device plan §172 — Cross-client Conversation changes JSON-safe integer parity

Global and owner-scoped Conversation change feeds now reject values above the
shared JSON-safe ceiling `9_007_199_254_740_991` across Go Runtime bridge and
HTTP, Rust Runtime RPC/remote CLI-TUI, and Flutter models. The ceiling remains
valid while `+1` fails closed for after/scanned/next/head cursors, row cursors,
schema/aggregate versions, and creation timestamps; unsafe Runtime storage
returns bounded query/storage errors without changing SQLite ranges. Focused
Go, Rust, and Flutter regressions cover request, response, RPC, and model
boundaries. This remains P1/P2 feed transport hygiene only; no live inventory,
scheduling, dispatch, Runner, artifact, or Audit action was added; ADR-0039
remains planning-only, ADR-0113/0114 remain Proposed/null, and P3b/P4 remain
gated.

### Cross-device plan §173 — Cross-client Conversation timestamp chronology parity

Flutter, Go, Rust remote, and Rust Runtime RPC now reject Conversation rows
where `updated_at_ms < created_at_ms`, while retaining the §171 JSON-safe
timestamp ceiling. This is P1/P2 response hygiene only; no live inventory,
scheduling, dispatch, Runner, artifact, or Audit action was added. ADR-0039
remains planning-only, ADR-0113/0114 remain Proposed/null, and P3b/P4 remain
gated.

### Cross-device plan §174 — Cross-client pending Run-intent JSON-safe numeric parity

The private Go HTTP candidate route and Runtime bridge pending-intent
DTO/request/response validation, plus Rust Runtime RPC/HubService pending-intent
cursors, timeline sequences, submit
versions, summaries, Prompt receipts, and event markers now share
`9_007_199_254_740_991`; the ceiling is accepted and `+1` fails closed, while
unsafe stored projections return `storage_corrupt`. The intent remains an
inert consent-checked receipt with no public execution path; SQLite storage is
unchanged, Flutter has no pending-intent RPC consumer, and its separate
display-only observation contract already enforces the same bound. No live
inventory, scheduling, dispatch, Runner, artifact, or Audit action was added;
ADR-0039 remains planning-only, ADR-0113/0114 remain Proposed/null, and P3b/P4
remain gated.

### Cross-device plan §175 — Cross-client Conversation aggregate-version JSON-safe parity

Go runtimebridge/HTTP, Rust remote/Runtime RPC, and Flutter now share
`9_007_199_254_740_991` for owner Conversation aggregate versions across
list/detail/create/import projections, Prompt append receipts, and
expected-version inputs. The ceiling is accepted and `+1` fails closed at
request, response, RPC, and model boundaries; Hub SQLite schema and ADR
invariants remain unchanged. This is P1/P2 transport/CAS hygiene only; no
live inventory, scheduling, dispatch, Runner, artifact, or Audit action was
added. ADR-0039 remains planning-only, ADR-0113/0114 remain Proposed/null,
and P3b/P4 remain gated.

### Cross-device plan §176 — Pending Run-intent HTTP candidate response guard

The private pending Run-intent HTTP candidate now validates backend response
numbers before serialization: submit Prompt receipts, intent summaries,
initial event markers, page cursors, timeline sequences, and event timestamps
must stay within `9_007_199_254_740_991`. Unsafe replacement-backend values
fail with a bounded service error; production route wiring remains disabled.
This is P2 inert-intent transport hygiene only, with no Run, live inventory,
scheduling, dispatch, Runner, artifact, or Audit action. ADR-0039 remains
planning-only, ADR-0113/0114 remain Proposed/null, and P3b/P4 remain gated.

### Cross-device plan §177 — Flutter placement preview response re-evaluation parity

The authenticated Flutter placement-preview client now recomputes the same
deterministic P3a evaluator over its validated caller declaration and rejects
any response whose owner, evaluation time, device set, eligibility, or
exclusion reasons differ. Authority bits, schema, ordering, and JSON-safe
numeric limits remain fail-closed. Flutter owner text now rejects Go's full
C0/C1 control range, and `min_cpu_cores` is bounded to the Go `uint32` wire
range. Regressions cover response tampering, C1 input, and the uint32 edge.
This remains an authenticated, stateless, caller-supplied preview: no live
inventory, heartbeat, reservation, scheduling, dispatch, Runner, artifact, or
Audit operation was added; ADR-0039 remains planning-only, ADR-0113/0114
remain Proposed/null, and P3b/P4 remain gated.

### Cross-device plan §178 — Go owner-scoped inventory read candidate seam

Forge Core now has a private unregistered candidate handler that derives the
owner from verified claims, requires the separate `forge:devices:read` scope,
validates an injected unverified inventory observation and response budget, and
fails closed on foreign, unsafe, unavailable, or oversized source data. The
production Coordinator still returns 404 for `/api/v1/devices`, enrollment,
and heartbeat; no discovery, persistence, live authority, selection,
scheduling, dispatch, Runner, or Audit operation was added. ADR-0039 remains
planning-only, ADR-0114 remains Proposed/null, and P3b/P4 remain gated.

### Cross-device plan §179 — Rust/Go/Flutter placement strict parity

Rust CLI/TUI and Go placement boundaries now share the complete nested wire
shape, `available_storage_bytes`, duplicate/unknown/null rejection,
deterministic response re-evaluation, and JSON-safe numeric ceiling. Authority
bounds now also align Flutter `concurrency_slots` with Go/Rust `uint16`; all
authority bits remain false. This remains P3a caller-supplied offline comparison
with no live inventory, heartbeat, target selection, reservation, scheduling,
dispatch, Runner, artifact, or Audit action; ADR-0039 remains planning-only,
ADR-0113/0114 remain Proposed/null, and P3b/P4 remain gated.

### Cross-device plan §180 — Rust offline resource-summary declaration parity

Rust CLI/TUI resource-summary and session-device-observation consumers now
aggregate caller declarations directly rather than applying live registry
`RunnerInstance`/`CapabilitySnapshot` constraints. Go, Rust, and Flutter now
agree on zero capacity, unknown declaration states, and zero/non-registry
snapshot or lease timestamps while retaining owner and `(device_id,
instance_id)` binding, bounded rows, safe-integer totals, duplicate-key
rejection, deterministic placement consistency, and all-false authority. The
raw aggregator is shared by both Rust commands and nested duplicate-key
regressions cover each local envelope. This remains P3a offline observation;
no live inventory, heartbeat, enrollment, selection, reservation, scheduling,
dispatch, Runner, artifact, or Audit action was added. ADR-0039 remains
planning-only, ADR-0113/0114 remain Proposed/null, and P3b/P4 remain gated.

### Cross-device plan §181 — Cross-client Prompt replay after bridge reconstruction

The Go HTTP-to-Rust integration now rebuilds the Runtime bridge after the
initial write sequence, replays the first Prompt idempotency key after a later
Prompt advances the Conversation head, and requires the original Prompt
receipt and aggregate version. It then re-reads owner history and the owner
Conversation page through the rebuilt route. This is P1/P2 persistence and
idempotency evidence only: Prompt submission remains storage-only, with no Run,
live inventory, scheduling, dispatch, Runner, artifact, or Audit action. ADR-
0039 remains planning-only, ADR-0113/0114 remain Proposed/null, and P3b/P4
remain gated.

### Cross-device plan §182 — Rust TUI change-feed cursor process recovery

Rust TUI sync now has a two-state regression over one owner/coordinator-bound
checkpoint: the first state consumes cursor zero and commits cursor one only
after session/history refresh; a fresh state requests after cursor one and
renders no duplicate change. A Go PTY integration runs two real saved-
credential TUI processes against a bounded Forge recorder and checks the
persisted cursor binding plus the absence of device/Run requests. This remains
P2 reconnect evidence only; no live event stream, device inventory,
heartbeat, scheduling, dispatch, Runner, artifact, or Audit action was added.
ADR-0039 remains planning-only, ADR-0113/0114 remain Proposed/null, and P3b/P4
remain gated.

### Cross-device plan §183 — TUI cursor recovery test portability and bounded fixture serving

The three Rust sync regressions that use the Unix saved-credential checkpoint
helper are now explicitly Unix-gated, while all fixture listener accepts use a
five-second bounded loop and restore accepted streams to blocking mode before
request reads, so missing requests fail promptly instead of hanging a test
thread on any supported socket platform. The standard shared-session E2E script
now selects the real saved-credential two-PTY TUI test whenever it builds the
Runtime binary. This
is test portability and failure containment only; no production route,
inventory, scheduling, dispatch, Runner, artifact, or Audit action was added.
ADR-0039 remains planning-only, ADR-0113/0114 remain Proposed/null, and P3b/P4
remain gated.

### Cross-device plan §184 — Flutter Web Conversation change-feed cursor reload recovery

The real Chromium Forge Console browser runner now enables an explicit
`reload_session` branch for the shared-session Prompt journey: it captures a
change-feed read from cursor zero, waits for the opaque owner-bound Web
checkpoint, reloads `/forge/` in the same tab, and captures a second automatic
feed read from a positive cursor before appending the Prompt. Go input plumbing
and recorder assertions require that zero-to-positive sequence while retaining
bounded Conversation, Prompt-history, Run, and change-feed reads. Run
observation callers pass `reload_session=false`, so the existing `run_id`
timeline and observation sequence remain compatible. This is P2 Web reconnect
evidence only; production routes and all inventory, scheduling, dispatch,
Runner, artifact, and Audit gates remain unchanged. ADR-0039 remains
planning-only, ADR-0113/0114 remain Proposed/null, and P3b/P4 remain gated.

### Cross-device plan §185 — Session device observation Go→Rust TUI→Flutter envelope roundtrip

The populated-Run integration now passes the canonical
`forge.session-device-observation/v1` envelope returned through the Rust
remote path into the Flutter API E2E. Flutter strictly decodes it and compares
the same owner, Conversation/Run, `evaluated_at_ms`, nine device/instance
pairs, nine decisions, resource totals, eligibility counts, null selection,
and all-false authority with its authenticated preview response. The real
Rust TUI output asserts the same timestamp, summary, candidate pairs, and
offline authority line. This remains P3a caller-supplied observation evidence;
live inventory, enrollment, heartbeat, scheduling, dispatch, Runner, and Audit
surfaces remain gated. ADR-0039 remains planning-only, ADR-0113/0114 remain
Proposed/null, and P3b/P4 remain gated.

### Cross-device plan §186 — Default-disabled gate for the Go inventory read candidate

The private Go owner-scoped inventory candidate now requires an explicit
injected configuration with `Enabled=true` and a non-nil source. Missing,
zero-value, disabled, or source-less configuration returns the bounded 404
surface, while the enabled fixture path retains verified-owner derivation,
the separate `forge:devices:read` scope, and strict source validation. Scope,
foreign-owner, unsafe-number, unavailable-source, and response-budget
regressions fail closed. The candidate remains unregistered; production
`/api/v1/devices`, enrollment, and heartbeat stay 404, and ADR-0039 remains
planning-only while ADR-0113/0114 remain Proposed/null. P3b/P4 remain gated.

### Cross-device plan §187 — Go heartbeat persistence compare-and-swap boundary

Forge Core's heartbeat package now exposes a pure value-level
`PersistedInstance` and `Commit` evaluator for a future storage transaction.
Malformed restored snapshots, stale expected revisions, and revision overflow
fail closed; accepted heartbeats return a complete replacement without
mutating the supplied snapshot. Focused tests cover invalid lease state,
successful and stale CAS attempts, and the shared ten-case persistence fixture
continues to match the Rust domain consumer. This is a transaction ABI and P3b
preparation only: no database, clock, auth, enrollment, listener, inventory,
reservation, selection, scheduling, dispatch, Runner, artifact, or Audit
operation was added. Production device routes remain 404 and ADR-0039,
ADR-0113/0114, and the P3b/P4 gates are unchanged.

### Cross-device plan §188 — Go heartbeat capability snapshot parity

Forge Core's pure heartbeat model now carries a bounded `CapabilitySnapshot` on
`Heartbeat` and `Instance`. CPU, memory, storage, GPU, runtime, label,
capacity, count, and duplicate validation mirrors the Rust Runner registry;
runtime names and GPU rows are canonicalized before a transition returns.
`Apply` and `Commit` retain the snapshot and reject malformed inbound or
restored declarations. The existing v1 heartbeat and persistence fixture JSON
is unchanged; fixture consumers inject the top-level capability declaration
into heartbeat/current values before evaluation. This remains P3b preparation:
production device routes remain 404 and ADR-0039, ADR-0113/0114, and the
P3b/P4 gates are unchanged.

### Cross-device plan §189 — Flutter heartbeat capability declaration parity

The Flutter pure heartbeat fixture consumer now validates the top-level
capability declaration with the Rust/Go bounds for normalized OS/architecture
and runtime tags, CPU/memory/storage capacities, available-capacity relations,
runtime and GPU counts, GPU memory, labels, and duplicate values. Optional GPU
rows are decoded and sorted by identifier; runtime names are lower-cased and
sorted. Regressions cover canonicalization, GPU rows, invalid capacity, and
duplicate declarations. This remains offline contract parsing only: no
heartbeat request, registration, authoritative inventory, selection,
reservation, scheduling, dispatch, Runner, or Audit operation was added;
production routes remain 404 and ADR-0039/0113/0114 plus P3b/P4 are unchanged.

### Cross-device plan §190 — Flutter heartbeat capability propagation parity

Flutter heartbeat signals and observed instances now carry the validated
capability snapshot through the pure transition. The persistence heartbeat and
CAS state use the same capability value, inject the existing persistence
fixture's bounded default declaration without changing its JSON, canonicalize
before returning, and reject missing or malformed heartbeat/current snapshots.
Regression coverage checks accepted heartbeat and CAS outputs retain runtime
and GPU declarations and that a missing snapshot fails closed. This remains
pure offline transition propagation: no heartbeat request, registration,
clock/storage access, authoritative inventory, selection, reservation,
scheduling, dispatch, Runner, or Audit operation was added; production routes
remain 404 and ADR-0039/0113/0114 plus P3b/P4 are unchanged.

### Cross-device plan §191 — Go persisted capability canonicality boundary

The Go heartbeat persistence CAS evaluator now rejects restored capability
snapshots whose values validate but whose OS/architecture spelling, runtime
order, or GPU order is not canonical under the Rust constructor semantics.
The pure semantic comparison treats nil and empty Go collections as the same
empty Rust `Vec`; canonicalization copies slices and focused tests prove
canonical replacement plus unchanged input on every rejection. This remains
P3b preparation only: no storage, clock, auth, enrollment, live inventory,
selection, reservation, scheduling, dispatch, Runner, artifact, or Audit
operation was added; production routes remain 404 and ADR-0039/0113/0114 plus
P3b/P4 are unchanged.

### Cross-device plan §192 — Go/Rust heartbeat boundary error precedence parity

Forge Core now checks heartbeat device binding, revocation, and lease bounds
before canonicalizing the declared capability snapshot, matching the Rust
heartbeat transition's stable rejection order for foreign, revoked, and
invalid-lease heartbeats even when capabilities are malformed. Focused tests
pin the precedence while eligible heartbeats still fail closed on invalid
capabilities. This remains pure transition preparation: no device request,
credential verification, storage, clock, enrollment, listener, inventory,
selection, reservation, scheduling, dispatch, Runner, artifact, or Audit
operation was added; production routes remain 404 and ADR-0039/0113/0114 plus
P3b/P4 are unchanged.

### Cross-device plan §193 — Go persisted inventory value and owner-isolation boundary

Forge Core now exposes a pure `PersistedInventoryState` contract combining an
owner-tuple-bound (still unverified) device record with a bounded Runner
instance. Restore validation
checks nonzero revision, exact device/Runner binding, canonical capability
collections, approval/cordon/reservation declarations, and lease bounds.
`CommitPersistedInventory` is an exact-revision complete-replacement evaluator;
`ProjectPersistedInventory` applies the fixed-time status projection and
rejects foreign owners. Focused tests cover restart-style restore, stale and
expired states, owner isolation, binding changes, revision conflicts and
noncanonical capabilities. No database, clock, listener, credential,
enrollment, live inventory, reservation, selection, scheduling, dispatch,
Runner, artifact, or Audit operation was added; production routes remain 404
and ADR-0039/0113/0114 plus P3b/P4 remain gated.

### Cross-device plan §194 — Go/Rust Runner lease and fencing contract parity

Forge Core's pure `executionlease` model now matches Rust Runtime
`execution::lease` for bounded grant validation, epoch/token renewal,
stale-proof rejection, terminal replay/conflict, digest/reason validation,
and uncertain-terminal no-retry semantics. The shared strict sixteen-case
`forge-runner-lease-fencing-v1` fixture runs in Go and Rust with all authority
bits false. This is a P4 precondition only: no clock, storage, enrollment,
inventory route, reservation, scheduler, transport, Runner, process, artifact,
or Audit operation was added; ADR-0039 remains planning-only, ADR-0113/0114
remain Proposed/null, and the accepted P4 execution/security decision is
still required.

### Cross-device plan §195 — Go/Rust persisted inventory CAS and projection parity

Rust's device-registry reference now exposes the same complete persisted
inventory value as Go: revision, owner tuple, device approval/cordon/
reservation declarations, and one bounded Runner instance with canonical
capabilities. The shared strict twelve-case
`forge-device-inventory-persistence-v1` fixture covers exact-revision
replacement, overflow, device/Runner binding, owner rejection, and
online/stale/pending/cordoned/offline/revoked projections. This remains a P3b
pure restart/CAS preparation slice: no database, clock, auth, enrollment,
heartbeat listener, inventory route, reservation, selection, scheduler,
transport, Runner, artifact, or Audit operation was added; ADR-0039 remains
planning-only, ADR-0113/0114 remain Proposed/null, and P3b/P4 remain gated.

### Cross-device plan §196 — Go/Rust Attempt lifecycle contract parity

Forge Core now exposes an authority-neutral `executionattempt.Lifecycle` value
that validates only the frozen Platform Core Attempt state graph. Rust Runtime
and Go consume the strict `forge-attempt-lifecycle-v1` fixture covering all
thirteen legal edges plus same-state, undeclared, terminal, and unknown-state
rejections. Reduction returns a new value and leaves the source unchanged;
the value does not persist an Attempt or authorize execution. This is a pure
P4 precondition: no database, clock, authentication, enrollment, inventory,
reservation, selection, scheduler, transport, Runner, artifact, or Audit
operation was added; ADR-0039 remains planning-only, ADR-0113/0114 remain
Proposed/null, and P3b/P4 remain gated.

### Cross-device plan §197 — Go/Rust Attempt request contract parity

Forge Core now exposes an authority-neutral `executionattempt.AttemptRequest`
value aligned with Rust Runtime's frozen `AttemptRequest`. The strict
`forge-attempt-request-v1` fixture covers full Attempt scope and exact entity
bindings, control versions, executor and optional record roles, deterministic
approval/effect normalization, budgets, timeout, idempotency, and invalid
value/reference classifications. Construction defensively copies caller
values and exposes read-only accessors; it performs no resolution or durable
write. This is a pure P4 precondition: no clock, authentication, enrollment,
inventory publication, reservation, selection, scheduler, transport, Runner,
artifact, or Audit operation was added; ADR-0039 remains planning-only,
ADR-0113/0114 remain Proposed/null, and P3b/P4 remain gated.

### Cross-device plan §199 — Go/Rust content-free Run observer evidence parity

Forge Core and Rust Runtime now project an existing owner-scoped Run summary
into the pure `forge.run.observed.v1` value. It exposes only opaque owner
linkage and bounded Conversation/Run/Prompt metadata; raw owner claims and
Prompt/result/tool/provider/path/token/lease/credential content are excluded,
and all eight authority fields remain false. Strict Go/Rust fixture consumers
cover unknown fields, bounds, status, owner shape, content-free output, and
authority invariants. This is evidence compatibility with the audited
Aero-ID/Audit Governance observer boundary only; Catalyst does not publish or
enqueue it. No outbox, route, device registration, heartbeat, inventory
authority, placement, reservation, scheduler, dispatch, Runner, artifact, or
Audit action was added; ADR-0039 remains planning-only, ADR-0113/0114 remain
Proposed/null, and P3b/P4 remain gated.

### Cross-device plan §200 — Ecosystem observer-fixture parity

The canonical `forge.run.observed.v1` fixture is now consumed by the
read-only Aero-ID Audit Governance publisher contract, the Audit Governance
receiver contract, and Snaplink Console's strict Dart evidence parser. The
standalone Go repositories embed byte-identical fixture copies; the contract
runner compares both copies with Catalyst's canonical fixture before running
focused tests. Consumers retain only opaque owner linkage and bounded Run
metadata, reject unknown/content-bearing fields, and require all authority
bits to remain false. This is evidence compatibility only: no publication,
device registration, heartbeat, inventory authority, placement, reservation,
scheduler, dispatch, Runner, artifact, or execution behavior was added.
ADR-0039 remains planning-only, ADR-0113/0114 remain Proposed/null, and P3b/P4
retain their separate governance gates.

### Cross-device plan §201 — Go/Rust persisted placement timestamp safety parity

The persisted-inventory-to-placement conversion now rejects Runner observation
and capability-lease timestamps above the JSON-safe integer ceiling
(`9007199254740991`) in both Go and Rust. Two shared fixture cases pin unsafe
observation and unsafe lease-expiry rejection to the stable
`invalid_persisted_inventory_placement_input` error, closing a freshness-input
parity gap before pure placement comparison. No route, storage, clock,
heartbeat listener, enrollment, inventory publication, selection, reservation,
scheduling, dispatch, Runner, artifact, or Audit operation was added;
production device routes remain 404 and ADR-0039/0113/0114 plus P3b/P4 remain
gated.

### Cross-device plan §202 — Go/Rust Run execution-evidence binding parity

Forge Core and Rust Runtime now bind an existing owner-scoped Run observer to
an existing session Runner terminal-receipt observer in the pure
`forge.run.execution-evidence.v1` value. Exact opaque-owner and
Conversation/Run/Prompt bindings are required; only bounded Attempt,
target, command, digest, disposition, and uncertain/manual-reconciliation
metadata are retained. Unknown/content fields and enabled authority fail
closed, and all evidence authority bits remain false. This adds no receipt
persistence, lease, target selection, reservation, dispatch, Runner,
artifact, or Audit behavior. ADR-0039 remains planning-only,
ADR-0113/0114 remain Proposed/null, and P3b/P4 retain separate gates.

### Cross-device plan §203 — Console Attempt-lifecycle fixture consumer

Snaplink Console now strictly consumes `forge-attempt-lifecycle-v1`, checking
the closed state graph, legal transitions, explicit rejection classes, and
all-false authority. The parser is offline evidence only and adds no network,
persistence, device, lease, placement, or execution behavior. ADR-0039 remains
planning-only, ADR-0113/0114 remain Proposed/null, and the separate accepted
P4 execution/security decision remains required.

### Cross-device plan §204 — Console Run execution-evidence fixture consumer

Snaplink Console now strictly parses `forge.run.execution-evidence.v1`,
checking opaque owner/reference fields, bounded metadata, command digest,
disposition/uncertain pairing, content-free flags, closed fields, and
all-false authority. It retains manual-reconciliation evidence without
retry/execution claims and adds no network, persistence, device, lease,
placement, reservation, dispatch, Runner, artifact, or Audit behavior.
ADR-0039 remains planning-only, ADR-0113/0114 remain Proposed/null, and P3b/P4
remain separately gated.
### Cross-device plan §205 — Go/Rust persisted inventory offline placement evaluation adapter

Forge Core and Rust Runtime now adapt one already owner-bound persisted
inventory placement input into their existing fixed-time pure placement
comparators. The shared `forge-device-inventory-placement-evaluation-v1`
fixture checks revision and device/Runner identity, deterministic exclusion
reasons when persisted inventory lacks residency/trust/sandbox/concurrency
attributes, and all-false authority. Both runtimes use the same 90-second
persisted-heartbeat freshness boundary and reject any non-empty GPU
capability with `unsupported_persisted_placement_capability` until a lossless
multi-GPU adapter exists. No selection, reservation, dispatch, Runner,
persistence, route, heartbeat, enrollment, or clock operation was added;
ADR-0039 remains planning-only, ADR-0113/0114 remain Proposed/null, and
P3b/P4 remain gated.

### Cross-device plan §206 — Console persisted inventory placement-input fixture consumer

Snaplink Console now strictly parses the canonical
`forge-device-inventory-placement-input-v1` fixture. Its offline value model
checks exact envelope, owner and Runner/device bindings, bounded persisted
capabilities, all case shapes, closed unknown policy defaults, unsafe timestamp
rejection cases, and all-false authority; unknown root, case, and expected
result fields fail closed. This is display and contract validation only: the
parser sends no request and adds no device registration, inventory
publication, target selection, reservation, scheduling, dispatch, Runner
execution, or receipt persistence. ADR-0039 remains planning-only,
ADR-0113/0114 remain Proposed/null, and P3b/P4 remain gated.

### Cross-device plan §207 — Flutter persisted inventory placement-evaluation fixture consumer

Snaplink Console's shared Web/App/Mobile API layer now strictly parses the
`forge-device-inventory-placement-evaluation-v1` fixture. The value model
checks the exact evaluation envelope and source binding, policy requirements,
sorted exclusion reasons, bounded identifiers, and all-false authority. Unknown
fields, enabled authority, invalid reason tokens, and inconsistent accepted
decisions fail closed. This remains offline display and contract validation
only: no request, device registration, heartbeat, target selection, reservation,
scheduling, dispatch, Runner execution, or receipt persistence was added.
ADR-0039 remains planning-only, ADR-0113/0114 remain Proposed/null, and P3b/P4
remain gated.

### Cross-device plan §208 — Go/Rust persisted inventory batch placement evaluation

Forge Core and Rust Runtime now evaluate a bounded set of owner-bound persisted
inventory inputs through the same fixed-time pure comparator. Full owner
tuples, duplicate device/Runner identities, empty input, sorted decisions,
revision/instance preservation, null selection, stable GPU/timestamp errors,
and all-false authority are covered by
`forge-device-inventory-placement-batch-evaluation-v1`. No registry read,
clock, route, registration, heartbeat, target selection, reservation,
scheduling, dispatch, Runner, artifact, persistence, or Audit action was
added; ADR-0039 remains planning-only, ADR-0113/0114 remain Proposed/null,
and P3b/P4 remain gated.

### Cross-device plan §209 — Rust CLI/TUI persisted inventory batch-evaluation consumer

Rust CLI now accepts `device inventory placement-batch-evaluation --input
FILE|-`, and the authenticated TUI reuses the same file-only offline preview.
Both consumers strictly validate the batch and sibling source fixtures,
duplicate JSON keys, unknown fields, authority mutations, duplicate
identities, unsafe timestamps, and source drift, rendering only deterministic
metadata decisions with null selection. No device request, registration,
heartbeat, inventory publication, selection, reservation, scheduling,
dispatch, Runner execution, receipt persistence, or Audit outbox operation was
added. ADR-0039 remains planning-only, ADR-0113/0114 remain Proposed/null, and
P3b/P4 remain gated.

### Cross-device plan §210 — Flutter Web/App/Mobile persisted inventory batch-evaluation consumer

Snaplink Console's shared Web/App/Mobile API layer now strictly parses the
canonical `forge-device-inventory-placement-batch-evaluation-v1` fixture. The
value model preserves the owner tuple, fixed evaluation time, requirements,
bounded case decisions, stable error cases, null selection, and all-false
authority; unknown fields, duplicate identities, invalid reasons, enabled
authority, and inconsistent decision identities fail closed. This remains
offline display and contract validation only: no inventory request, device
registration, heartbeat, target selection, reservation, scheduling, dispatch,
Runner execution, or receipt persistence was added. ADR-0039 remains
planning-only, ADR-0113/0114 remain Proposed/null, and P3b/P4 remain gated.

### Cross-device plan §211 — Flutter Web/App/Mobile persisted inventory restore/CAS projection consumer

Snaplink Console's shared Web/App/Mobile API layer now strictly parses and
purely evaluates `forge-device-inventory-persistence-v1`. The consumer keeps
revision, owner, device/Runner bindings, bounded capabilities, and full uint64
values as `BigInt`; restore, exact-revision replacement, and fixed-time status
projection are checked against all twelve canonical cases. Unknown fields,
duplicate case names, unknown errors, invalid bindings/owners, and enabled
authority fail closed.

This is value-only restart/CAS preparation. Flutter performs no storage write,
heartbeat, clock read, device request, registration, inventory publication,
target selection, reservation, scheduling, dispatch, Runner execution, receipt
persistence, or Audit publication. ADR-0039 remains planning-only,
ADR-0113/0114 remain Proposed/null, and P3b/P4 retain their separate gates.

### Cross-device plan §212 — Rust CLI/TUI persisted inventory restore/CAS projection preview

Rust CLI now accepts `device inventory persistence-preview --input FILE|-`,
and the authenticated TUI reuses the same file-only offline preview. The
consumer strictly decodes `forge-device-inventory-persistence-v1`, rejects
duplicate JSON keys, unknown fields, enabled authority, invalid state values,
and expectation mismatches, then evaluates restore, exact-revision replacement,
and fixed-time projection through the Rust domain model. It renders bounded
case metadata only and grants no target or execution authority.

Focused CLI stdin/path/negative tests, argument parsing, and TUI rendering
tests pass. This is pure CAS/projection consumption: no storage write, clock
read, network request, device registration, heartbeat, inventory publication,
selection, reservation, scheduling, dispatch, Runner execution, receipt
persistence, or Audit operation was added. ADR-0039 remains planning-only,
ADR-0113/0114 remain Proposed/null, and P3b/P4 retain their separate gates.

### Cross-device plan §213 — Real forge-server process shared-session smoke and negative boundary

The actual forge-server binary now has a process-level test with the actual
Rust Runtime bridge and a temporary trusted loopback Snaplink JWKS issuer. Two
independent client tokens create, list, append, replay, and read one
owner-scoped Conversation/Prompt; wrong Host, pinned-owner mismatch, missing
write scope, stale version, and caller-supplied placement preview are covered,
with all placement authority bits false. The default server remains health-only
and device registration, enrollment, heartbeat, execution-consent, and
run-intent paths remain 404. No device registry, credential issuer,
authoritative inventory, selection, reservation, scheduling, dispatch, Runner
transport, remote execution, artifact, or Audit operation was added. ADR-0039
remains planning-only, ADR-0113/0114 remain Proposed/null, and P3b/P4 remain
gated.

### Cross-device plan §214 — Flutter Forge response and change-feed hardening

Snaplink Console now rejects duplicate object keys in raw Forge JSON before
Dart decoding, applies closed-field validation to Prompt append results, and
resnapshots owner metadata before persisting a feed cursor for a conversation
outside the loaded page. API/model and periodic widget regressions cover
duplicate, unknown, missing, and unloaded-conversation cases. This is
client-side parsing/read synchronization only: no device registration,
enrollment, heartbeat, inventory authority, selection, reservation, scheduling,
dispatch, Runner, artifact, receipt, or Audit operation was added. ADR-0039
remains planning-only, ADR-0113/0114 remain Proposed/null, and P3b/P4 retain
their separate acceptance gates.

### Cross-device plan §215 — Go persisted inventory to owner-bound observation source

Forge Core now converts already restored `PersistedInventoryState` values into
the existing owner-bound `forge.device-inventory-observation/v1` envelope at an
explicit evaluation time. The pure adapter reuses the fixed 90-second
projection, canonicalizes resources, sorts rows, rejects foreign owners,
duplicate identities, unsafe timestamps, unsupported reservation declarations,
and unsupported GPU mappings, and preserves unverified resources with all
authority bits false. A test-only private read-source bridge derives the owner
from verified claims and checks cancellation; it is not wired to production
routes. Focused Go tests cover canonical ordering, pending/stale/offline/
expired states, owner isolation, unsupported values, unsafe time, and response
validation. No device table, control-store change, clock read, heartbeat
listener, credential issuer, enrollment, production inventory route, target
selection, reservation, scheduling, dispatch, Runner, artifact, or Audit
operation was added; `/api/v1/devices`, enrollment, and heartbeat remain 404,
ADR-0039 remains planning-only, ADR-0113/0114 remain Proposed/null, and P3b/P4
retain their separate acceptance gates.

### Cross-device plan §216 — Rust persisted inventory to owner-bound observation source

Rust Runtime now exposes a pure domain adapter from already restored
`PersistedInventoryState` values to the existing
`forge.device-inventory-observation/v1` envelope. The adapter takes an exact
`SnapshotOwner` and explicit evaluation time, reuses restore validation and the
fixed 90-second projection, sorts rows, preserves unverified capability/state
declarations, and rejects owner drift, duplicate identities, unsafe/future
observations, reservations, and GPU values that the current envelope cannot
represent losslessly. JSON serialization keeps every authority bit false.

Focused domain tests cover ordering, status declarations, owner/time binding,
unsupported values, duplicate device/Runner identities, and envelope shape.
This is a P3b preparation seam only; no storage, clock, listener, credential,
route, registration, enrollment, heartbeat, inventory publication, selection,
reservation, scheduling, dispatch, Runner, artifact, or Audit operation was
added. ADR-0039 remains planning-only, ADR-0113/0114 remain Proposed/null,
and P3b/P4 retain their separate gates.

### Cross-device plan §217 — Flutter Prompt request preflight parity

Snaplink Console now rejects unsafe Prompt request values before transport:
listPrompts requires an owner-session-safe Conversation ID and validates the
Prompt cursor ID, while appendPrompt validates the Conversation ID, non-empty
bounded UTF-8 content, and the single-use-safe idempotency-key shape. Prompt
cursor parsing also rejects control characters. Focused API/model tests prove
all invalid values make zero HTTP requests.

This closes client preflight parity with the Rust remote client and Go
Conversation handlers. It does not create a Run, submit a pending intent,
resolve an execution profile, read inventory, select a target, reserve,
schedule, dispatch, execute a Runner, persist a receipt, or publish Audit
events. ADR-0039 remains planning-only, ADR-0113/0114 remain Proposed/null,
and P3b/P4 retain their separate gates.

### Cross-device plan §218 — Rust persisted inventory observation preview

Rust CLI now accepts `device inventory persisted-observation --input FILE|-`,
and the authenticated TUI reuses the same file-only offline preview. The new
`forge-device-inventory-persisted-observation-v1` fixture supplies restored
value declarations, exact owner/time, all-false source authority, and the
expected shared `forge.device-inventory-observation/v1` envelope. Strict
parsing and tests cover duplicate JSON keys, unknown fields, authority
mutations, invalid state/capability values, owner drift, future timestamps,
duplicate identities, reservation/GPU lossiness, and deterministic sorted
output.

The adapter and consumers remain pure value/display paths. They perform no
storage, clock, network, route, registration, enrollment, heartbeat, inventory
publication, target selection, reservation, scheduling, dispatch, Runner,
artifact, receipt, or Audit operation. ADR-0039 remains planning-only,
ADR-0113/0114 remain Proposed/null, and P3b/P4 retain their separate gates.

### Cross-device plan §219 — Flutter persisted inventory observation envelope consumer

Snaplink Console's shared Web/App/Mobile inventory parser now consumes the
expected `forge.device-inventory-observation/v1` envelope from the Rust
persisted-observation fixture. The contract checks source metadata, owner/time
binding, sorted device/Runner rows, CPU/memory/storage declarations,
liveness/approval state, and all-false authority; an authority mutation fails
closed through the same strict parser used by the display path. This is a
display contract only: no storage, device route, registration, heartbeat,
inventory authority, target selection, reservation, scheduling, dispatch,
Runner, receipt, or Audit operation was added. ADR-0039 remains planning-only,
ADR-0113/0114 remain Proposed/null, and P3b/P4 retain their separate gates.

### Cross-device plan §220 — Aero-ID and Audit Governance execution-evidence consumer parity

Aero-ID and Snaplink Audit Governance now consume byte-identical copies of the
canonical `forge.run.execution-evidence.v1` fixture. Strict receiver tests
validate the closed metadata shape, lowercase owner/command digests,
content-free payload, compatible Audit event, and all-false authority;
unknown fields and raw prompt/result/output content fail closed. This is
read-only ecosystem contract validation: no event publication, outbox,
receipt persistence, device registration, inventory publication, selection,
reservation, scheduling, dispatch, Runner, artifact, or Audit authority was
added. ADR-0039 remains planning-only, ADR-0113/0114 remain Proposed/null,
and P3b/P4 retain their separate gates.

### Cross-device plan §221 — Persisted observation JSON-safe resource bounds

The Go and Rust persisted-inventory observation adapters now reject available
memory and storage declarations above the JSON-safe integer ceiling before
building `forge.device-inventory-observation/v1`. Regression cases cover both
resource dimensions in each runtime and preserve decoding parity for the
Flutter Web/App/Mobile consumer. This is a pure value boundary: no storage,
clock, route, registration, heartbeat, authoritative inventory, target
selection, reservation, scheduling, dispatch, Runner, receipt, or Audit
operation was added. ADR-0039 remains planning-only, ADR-0113/0114 remain
Proposed/null, and P3b/P4 retain their separate gates.

### Cross-device plan §222 — Aero IM and Aero Vault integration gate audit

The ecosystem audit confirms that Aero IM's machine notification endpoint is a
side-effecting publish path requiring installation authorization, target
mapping, idempotency, and durable delivery. Aero Vault owns generic
tenant-scoped object metadata and storage but has no Forge ArtifactRef ABI or
accepted digest/size/sensitivity/retention/version mapping. No Forge adapter is
added until those contracts and their outbox/receipt or artifact authorization
decisions are accepted. IM delivery and Vault object writes therefore remain
outside the session/inventory preview; no network, message publication,
object write, artifact staging, device registration, inventory authority,
selection, reservation, scheduling, dispatch, Runner, or Audit operation was
added. ADR-0039 remains planning-only, ADR-0113/0114 remain Proposed/null,
and P3b/P4 retain their separate gates.

### Cross-device plan §223 — Rust CLI/TUI Attempt request preview

Rust CLI now accepts `device attempt-request-preview --input FILE|-`, and the
authenticated TUI exposes the same file-only preview. Both strictly decode
`forge-attempt-request-v1`, reject duplicate keys, unknown fields, authority
mutations, malformed case envelopes, and expectation drift, then run each
caller-supplied request through the existing pure Attempt request model. They
render only bounded case names, stable rejection classes, normalized effect or
approval metadata, and all-false authority. Focused CLI argument, contract,
duplicate-key, metadata-only output, and TUI no-network tests pass.

This closes the Rust CLI/TUI consumer gap for the P4 Attempt request
precondition. No reference resolution, storage, clock, network, inventory,
reservation, target selection, scheduling, dispatch, Runner execution,
artifact transfer, receipt persistence, or Audit operation was added. Flutter
still has lifecycle fixture consumption but no Attempt request card; that is a
separate display-only follow-up. ADR-0039 remains planning-only,
ADR-0113/0114 remain Proposed/null, and P3b/P4 retain their separate gates.

### Cross-device plan §224 — Rust TUI selected Run incremental sync

The authenticated Rust TUI now retains the validated Run timeline sequence
selected by `timeline RUN_ID`; a later `sync` requests the next bounded,
owner-bound metadata-only timeline page and renders contiguous new markers.
Changing the selected Conversation or clearing owner state clears this
process-local Run selection. A PTY-style regression covers sequence-zero then
sequence-one reads, while transport, authorization, malformed-page, and
cursor-regression failures leave the Conversation change cursor unadvanced.
Durable cross-process resume remains explicit `timeline RUN_ID --resume`.
No Run write, execution, device/inventory request, target selection,
reservation, scheduling, dispatch, Runner, artifact, receipt, or Audit
operation was added; ADR-0039 remains planning-only, ADR-0113/0114 remain
Proposed/null, and P3b/P4 retain their separate gates.

### Cross-device plan §225 — Flutter Attempt request preview consumer

Snaplink Console's shared Web/App/Mobile API layer now consumes the
`forge-attempt-request-v1` fixture as a bounded, read-only preview. The strict
decoder checks the closed envelope, all-false authority, unique case names,
request shape, normalized sorted effects and approval IDs, stable rejection
classes, and duplicate JSON keys from raw fixture text. A display card renders
case outcomes and marks the value offline and unverified. Focused fixture and
widget tests pass with `flutter analyze --no-pub`.

This slice does not resolve references, read or write storage, call a route,
read a clock, select or reserve a device, schedule or dispatch a Run, execute
a Runner, transfer an artifact, persist a receipt, or publish Audit evidence.
ADR-0039 remains planning-only, ADR-0113/0114 remain Proposed/null, and
P3b/P4 retain their separate acceptance gates.

### Cross-device plan §226 — Flutter pending Run-intent preview consumer

Snaplink Console now consumes `forge-pending-run-intent-v1` across the shared
Web/App/Mobile surface. The strict bounded decoder verifies the owner envelope,
all-false authority, Prompt/intent/event bindings, page and payload-free
timeline continuity, JSON-safe numbers, expectation parity, and duplicate raw
JSON keys. The optional Sessions card renders receipt metadata only and omits
Prompt content. Focused fixture and widget tests pass with `flutter analyze`.

This remains read-only pending-intent observation: no Run is created, no
Project profile is resolved, no storage or private candidate route is called,
and no device selection, reservation, scheduling, dispatch, Runner, artifact,
receipt, or Audit operation is performed. ADR-0039 remains planning-only,
ADR-0113/0114 remain Proposed/null, and P3b/P4 retain their separate gates.

### Cross-device plan §227 — Rust CLI/TUI and Flutter authenticated pending Run-intent observation

Rust's authenticated remote client now exposes owner-scoped pending Run-intent
list and payload-free timeline GETs. `remote run-intents list` and `remote
run-intents timeline` enforce bounded pages, JSON-safe cursors, complete keyset
pairs, owner binding, newest-first ordering, pending status, sequence
continuity, and a closed metadata-only response shape. The TUI exposes the same
selected-session reads and renders only intent/profile/status metadata and
immutable event envelopes. Mock HTTP and TUI tests cover authenticated GET
requests, unknown fields, and prompt-content exclusion.

The real Snaplink-issued-token → Go app-server → Rust Hub E2E now reads the
same test-only inert receipt through both CLI commands and the TUI. Its request
recorder asserts only the two added GET paths and keeps production execution
routes at 404.

Flutter's common Web/App/Mobile API layer has matching owner-bound list and
timeline response models and request preflight. The live methods remain a
read-only API seam; the Sessions card continues to consume the offline fixture
and no submit method is wired into product flow.

This slice does not submit a pending intent, create a Run, resolve a caller
profile, access device storage, register or heartbeat a device, publish
authoritative inventory, select or reserve a target, schedule or dispatch work,
execute a Runner, transfer an artifact, persist a receipt, or publish Audit
evidence. Production execution routes remain 404/default-off. ADR-0039 remains
planning-only, ADR-0113/0114 remain Proposed/null, and P3b/P4 retain their
separate acceptance gates.

### Cross-device plan §228 — Flutter owner-bound device inventory read candidate seam

Snaplink Console's shared Web/App/Mobile Forge API now has a strict,
owner-bound `GET /api/v1/devices` candidate method. It validates the expected
owner tuple before transport, decodes the existing
`forge.device-inventory-observation/v1` envelope, and rejects owner drift.
Focused tests prove the authenticated GET has no query, body, or idempotency
key and invalid owner input performs no request.

This is an API seam for the explicitly injected Go read candidate; production
`/api/v1/devices` remains unregistered and 404. No enrollment, heartbeat,
live-registry read, target selection, reservation, scheduling, dispatch,
Runner execution, receipt persistence, or Audit publication was added.
Inventory values and authority markers remain unverified. ADR-0039 remains
planning-only, ADR-0114 remains Proposed/null, and P3b/P4 remain separately
gated.

### Cross-device plan §229 — Flutter live pending Run-intent API E2E parity

The test-only inert execution surface now runs Snaplink Console's shared
Web/App/Mobile `ForgeConversationsApi` against the same owner-scoped pending
Run-intent receipt used by the Rust CLI/TUI. The Flutter API E2E performs
authenticated bounded list and payload-free timeline GETs, verifies the
pending metadata and initial `submitted` event, and then continues the
existing Prompt retry and placement-preview checks. Go's recorder requires
the two exact pending paths when enabled and rejects extra execution/device
requests.

This supplies live API evidence for the common Flutter seam; the Sessions
card remains fixture-only. Production execution-consent and pending-intent
routes remain 404. No pending intent submission, Run creation, device
registration/heartbeat/inventory authority, selection, reservation,
scheduling, dispatch, Runner, artifact, receipt, or Audit operation was added.
ADR-0039 remains planning-only, ADR-0113/0114 remain Proposed/null, and P3b/P4
remain separately gated.

### Cross-device plan §230 — Flutter authenticated inventory candidate E2E

The test-only inert Go server now mounts the explicitly enabled owner-bound
inventory read candidate beside the shared-session routes. A real
Snaplink-issued token with the separate `forge:devices:read` scope drives the
common Web/App/Mobile Console API through exactly one authenticated
`GET /api/v1/devices`. Flutter verifies the owner tuple, bounded device and
Runner identity, unverified markers, and all-false authority; Go verifies the
request count and owner derived from verified claims.

This exercises the §228 API seam only. Production `/api/v1/devices`,
enrollment, and heartbeat remain 404 because the candidate is mounted only in
the test server. No live registry, credential issuer, discovery, selection,
reservation, scheduling, dispatch, Runner, receipt, or Audit operation was
added. ADR-0039 remains planning-only, ADR-0114 remains Proposed/null, and
P3b/P4 retain their separate gates.

### Cross-device plan §231 — Rust CLI/TUI authenticated inventory candidate parity

Rust Runtime now exposes `remote inventory show` and TUI `inventory read` for
the same test-only owner-bound `/api/v1/devices` candidate. Both clients send
authenticated GET-only requests and strictly validate
`forge.device-inventory-observation/v1`, preserving the owner tuple,
unverified markers, and all-false authority. The real Snaplink JWT → Go inert
server → Rust CLI/TUI E2E checks the exact request and metadata.

Production `/api/v1/devices`, enrollment, and heartbeat remain 404. No
registry, discovery, selection, reservation, scheduling, dispatch, Runner,
receipt, or Audit operation was added. ADR-0039 remains planning-only,
ADR-0114 remains Proposed/null, and P3b/P4 remain separately gated.

### Cross-device plan §232 — Flutter Runner lease/fencing contract parity

Snaplink Console's shared Web/App/Mobile layer now consumes the existing
`forge-runner-lease-fencing/v1` fixture through a pure in-memory model. It
covers activity, renewal with a new fencing token, proof binding, terminal
replay/conflict behavior, and uncertain-terminal reconciliation. Explicit
null optionals, malformed Unicode, and UTF-8 byte-bound violations fail
closed. Local JSON round trips document decimal-string encoding above the Web
safe-integer range; they are not a transport encoder.

This is contract parity only. It reads no clock, creates no lease store or
issuer, reserves no capacity, contacts no Runner, and performs no execution,
dispatch, or Audit publication. Production lease, reservation, and Runner
routes remain absent/default-off. ADR-0039 remains planning-only, ADR-0114
remains Proposed/null, and P3b/P4 retain their separate gates.

### Cross-device plan §233 — Flutter Runner terminal receipt Unicode/UTF-8 parity

Snaplink Console's terminal receipt decoder now checks that every bounded
Runner text value is well-formed Unicode before calling `utf8.encode`. An
isolated high or low UTF-16 surrogate is rejected for lease identities and
fencing tokens, command/argv text, and failed or uncertain reasons. This
prevents Dart's replacement behavior from changing invalid input into a value
that appears to satisfy the shared byte limits. Focused contract tests cover
command, grant, and disposition paths for both surrogate directions, and the
existing valid receipt and uncertain-reconciliation cases remain green.

The change is a pure client value-boundary correction. It adds no clock, lease
store/issuer, device registration, heartbeat, inventory authority,
reservation, scheduling, dispatch, Runner transport, execution, receipt
persistence, or Audit publication. Production device and execution routes
remain default-off; ADR-0039 remains planning-only, ADR-0114 remains
Proposed/null, and P3b/P4 retain their separate acceptance gates.

### Cross-device plan §234 — Flutter Runner lease fixture duplicate-key parity

The raw `forge-runner-lease-fencing/v1` fixture loader now bounds the source,
scans every object depth for duplicate keys, and only then calls Dart's JSON
decoder. Contract coverage mutates both the root `schema_version` and nested
`authority.device_identity_verified` keys, proving that a last-value-wins
decode cannot hide a cross-runtime contract mutation. Valid Go/Rust fixture
consumption and the existing lease/fencing lifecycle cases remain green.

This is fixture decoder hardening only. It adds no network request, clock,
lease store/issuer, device registration, heartbeat, inventory authority,
reservation, scheduling, dispatch, Runner transport, execution, receipt
persistence, or Audit publication. Production device and execution routes
remain default-off; ADR-0039 remains planning-only, ADR-0114 remains
Proposed/null, and P3b/P4 retain their separate acceptance gates.

### Cross-device plan §235 — Authenticated uncertain Runner receipt preview parity

The test-only authenticated Snaplink session Run E2E now exercises both a
completed and an `uncertain` Runner terminal receipt through the same Go
preview route. Rust CLI and PTY TUI assert the disposition, uncertainty,
manual-reconciliation, and all-false authority markers. The recorder requires
exactly one receipt-preview POST per surface and rejects device, execution, or
dispatch requests. Under `FORGE_CONSOLE_E2E=1`, Flutter's shared Web/App/Mobile
API consumes the same uncertain envelope and follows the exact Run/timeline/
session-observation/receipt-preview request contract.

This remains test-only observation evidence: no lease renewal, retry, target
selection, receipt persistence, Runner contact, execution, dispatch, or Audit
publication was added. Production device and execution routes remain
default-off; ADR-0039 remains planning-only, ADR-0114 remains Proposed/null,
and P3b/P4 retain their separate acceptance gates.

### Cross-device plan §236 — Flutter Forge owner tuple Unicode/UTF-8 parity

Snaplink Console's shared `ForgeDeviceOwner` decoder now rejects isolated
UTF-16 high and low surrogates before measuring UTF-8 bytes. Issuer, subject,
and `tenant_id` therefore follow the well-formed Unicode boundary used by the
Go/Rust owner contract; valid multibyte values at exactly 512 bytes remain
accepted and values above that bound fail closed. Focused inventory contract
tests exercise both surrogate directions and both UTF-8 boundary outcomes for
all three owner tuple fields.

This is a pure owner-value decoder correction. It adds no network request,
device registration, heartbeat, inventory authority, lease store or issuer,
reservation, scheduling, dispatch, Runner transport, execution, receipt
persistence, or Audit publication. Production device and execution routes
remain default-off; ADR-0039 remains planning-only, ADR-0114 remains
Proposed/null, and P3b/P4 retain their separate acceptance gates.

### Cross-device plan §237 — Flutter native uncertain Runner receipt preview parity

The populated-Run Flutter native widget E2E now exercises both completed and
`uncertain` session Runner receipt envelopes. Each invocation first sends the
canonical receipt through the authenticated Flutter API and asserts the
disposition, reconciliation/manual-review flags, `automatic_retry=false`, and
`follow_up`; the rendered card exposes the disposition and follow-up as well.
The Go recorder requires exactly one receipt-preview POST and one device-
observation POST per invocation, allows only owner-scoped Run/timeline reads,
and rejects device, execution, or dispatch paths.

This is test-only native observation evidence; browser parity is a separate
follow-up. No lease renewal, retry, target selection, receipt persistence,
Runner contact, execution, dispatch, or Audit publication was added.
Production device and execution routes remain default-off; ADR-0039 remains
planning-only, ADR-0114 remains Proposed/null, and P3b/P4 retain their
separate acceptance gates.

### Cross-device plan §238 — Flutter Web uncertain Runner receipt preview parity

The browser observation E2E now runs completed and `uncertain` session Runner
receipt envelopes in separate authenticated Chromium sessions. Python validates
the echoed uncertainty, reconciliation/manual-review, no-automatic-retry, and
follow-up invariants; the page assertion also requires the disposition and
follow-up strings in the rendered receipt card. Go's recorder applies the exact
owner-scoped Run/timeline/session-observation contract and requires exactly one
receipt-preview POST per browser run, rejecting device, execution, or dispatch
paths.

The Web test code and static checks are complete. With the prebuilt Flutter
Web bundle and the host Google Chrome executable, the authenticated Go →
Flutter Web E2E passes in `43.270s`; each browser session issues one receipt
preview POST and no device, execution, or dispatch request. No lease renewal,
retry, target selection, receipt persistence, Runner contact, execution,
dispatch, or Audit publication was added. Production device and execution
routes remain default-off; ADR-0039 remains planning-only, ADR-0114 remains
Proposed/null, and P3b/P4 retain their separate gates.

### Cross-device plan §239 — Rust session Runner receipt duplicate-key parity

Rust's offline `device session-runner-receipt-preview` input and authenticated
`remote session-runner-receipt preview` input now reject duplicate JSON object
keys in the bounded raw UTF-8 document before `serde_json` materializes maps.
Root and nested authority mutations are covered by CLI/TUI regressions. The
existing 2 MiB bound, closed envelope, owner/Run binding, and all-false
authority remain unchanged. The full `scripts/test-forge-contracts.sh` suite
exits 0.

This is decoder hardening only. No network route, receipt or lease persistence,
device registration, heartbeat, inventory authority, target selection,
reservation, scheduling, dispatch, Runner contact, execution, or Audit
publication was added. Production device and execution routes remain
default-off; ADR-0039 remains planning-only, ADR-0114 remains Proposed/null,
and P3b/P4 retain their separate gates.

### Cross-device plan §240 — Authenticated inventory Runner resource parity

The test-only owner-bound inventory candidate E2E now verifies each distinct
`(device_id, instance_id)` row's architecture, CPU, memory, storage, runtime,
and concurrency fields in Rust CLI, Rust TUI, and Flutter's shared Web/App/
Mobile API. Go continues to require one owner-scoped inventory GET and the
owner derived from verified claims; all declaration and authority markers stay
unverified/false.

This remains a bounded observation parity slice. The candidate is mounted only
by the inert test server, so production `/api/v1/devices`, enrollment, and
heartbeat remain 404. No target selection, reservation, scheduling, dispatch,
Runner contact, execution, receipt persistence, or Audit publication was
added. ADR-0039 remains planning-only, ADR-0114 remains Proposed/null, and
P3b/P4 retain their separate gates.

### Cross-device plan §241 — Cross-client pending Run-intent submit parity

Rust CLI/TUI and Flutter's shared Web/App/Mobile API now submit the same
owner-scoped, consent-checked pending Run-intent candidate with CAS versioning,
required idempotency, strict receipt binding, and retry/recovery semantics.
The real Snaplink-issued inert E2E drives CLI, TUI, and Flutter against one
receipt and verifies the shared metadata and payload-free timeline.

This receipt stores Prompt and pending metadata only. It never starts an
ordinary Run, selects a device, creates a lease, authorizes execution,
schedules or dispatches work, contacts a Runner, or publishes Audit evidence.
Production `/run-intents` and execution routes remain 404; device registration,
heartbeat, authoritative inventory, and scheduler/Runner services remain
disabled. ADR-0039 remains planning-only, ADR-0113/0114 remain Proposed/null,
and P3b/P4 retain their separate acceptance gates.

### Cross-device plan §242 — Fresh pending Run-intent cross-client write/recovery parity

The authenticated inert E2E now creates independent fresh pending receipts from
Rust CLI, Flutter's shared Web/App/Mobile API, and Rust TUI with distinct CAS
versions and idempotency keys, then reads shared metadata/timeline. TUI 409/CAS
conflict handling now clears stale pending state, refreshes the session, and
requires a new submission.

This remains test-only inert receipt evidence. No ordinary Run, target
selection, lease, execution authorization, scheduling, dispatch, Runner
contact, receipt persistence, or Audit evidence was added. Production
run-intent/device/execution routes remain 404; ADR-0039 remains
planning-only, ADR-0113/0114 remain Proposed/null, and P3b/P4 retain their
separate acceptance gates.

### Cross-device plan §243 — Fresh pending Run-intent cross-client readback convergence

The fresh-write E2E now reads back every owner-scoped pending receipt after the
CLI → Flutter → TUI sequence. CLI validates its fresh list item and timeline,
Flutter validates its fresh item and timeline while retaining the original
receipt read, TUI refreshes the list after submit, and a final owner-scoped Go
readback binds all receipts to Prompt history, expected aggregate versions,
payload-free `submitted` timelines, and an empty ordinary Run page.

Optional client branches remain recorder-checked and each client that runs must
contribute its fresh Prompt and receipt. This remains inert evidence: no
ordinary Run, target selection, lease, execution authorization, scheduling,
dispatch, Runner contact, receipt persistence, or Audit evidence was added.
Production run-intent/device/execution routes remain 404; ADR-0039 remains
planning-only, ADR-0113/0114 remain Proposed/null, and P3b/P4 retain their
separate acceptance gates.

### Cross-device plan §244 — Flutter Prompt history refresh merge convergence

The shared Flutter Sessions screen now merges an owner change-feed Prompt
refresh by Prompt ID instead of replacing the locally loaded history. After a
user loads older Prompts, a Prompt appended by another authenticated client is
added while the older rows and the deepest pagination cursor remain visible.
Initial reads, older-page reads, and generation/Conversation binding checks are
unchanged.

The widget regression drives the older-page read, an authenticated
`prompt_appended` change, and the newest-page refresh, then requires the old,
newest, and cross-client Prompt rows plus the older-page affordance. This is
read synchronization only: no device registration, inventory, heartbeat,
target selection, reservation, scheduling, dispatch, Runner, execution,
receipt, or Audit operation was added. Production device and execution routes
remain default-off; ADR-0039 remains planning-only, ADR-0113/0114 remain
Proposed/null, and P3b/P4 retain their separate acceptance gates.

### Cross-device plan §245 — Flutter Sessions owner-bound inventory candidate display

The shared Flutter Sessions Gate/Screen now accepts an explicitly injected
verified owner tuple and inventory-candidate reader. When supplied, Web/App/
Mobile performs one bounded owner-scoped candidate read and renders both
Runner rows, resource values, and the unverified/all-false authority markers
through the existing inventory panel. Refresh performs one new read and a
foreign-owner response fails closed; the default Gate remains request-free for
`/devices`.

The widget regression covers two Runner rows, refresh call count, and owner
mismatch. The authenticated candidate E2E mounts the real Sessions screen and
asserts the panel while Go still requires exactly one owner-bound inventory GET
among normal owner-scoped session reads. No enrollment, heartbeat,
authoritative inventory, target selection, reservation, scheduling, dispatch,
Runner, execution, receipt, or Audit operation was added. Production device
routes remain 404; ADR-0039 remains planning-only, ADR-0114 remains
Proposed/null, and P3b/P4 retain their separate acceptance gates.

### Cross-device plan §246 — Flutter Sessions owner-bound pending Run-intent metadata observation

The shared Flutter Sessions Gate/Screen now accepts an explicitly injected
pending Run-intent metadata reader. The opt-in path reads one bounded
owner-scoped list for the selected Conversation, strictly re-decodes its
Conversation binding/order/pending status/initial sequence, and renders only
intent metadata without Prompt content or action buttons. Refresh, feed
synchronization, selection changes, authorization failure, and sign-out clear
or re-read the projection; the default Gate makes no `/run-intents` request.

Widget coverage proves metadata-only rendering, refresh count, foreign-
Conversation rejection, and the default no-request path. The authenticated
inert E2E mounts the real Sessions screen and requires one bounded pending
metadata GET while rejecting widget POSTs. No pending submission, ordinary Run,
device selection, lease, reservation, scheduling, dispatch, Runner, execution,
receipt, or Audit operation was added. Production run-intent/device/execution
routes remain 404; ADR-0039 remains planning-only, ADR-0113/0114 remain
Proposed/null, and P3b/P4 retain their separate acceptance gates.

### Cross-device plan §247 — Flutter Sessions pending Run-intent timeline metadata expansion

The shared Flutter Sessions Gate/Screen now accepts an explicitly injected
pending Run-intent timeline reader. The receipt card remains collapsed and
does not request a timeline until an owner expands it. The opt-in read is one
bounded initial page, strictly re-decoded for Conversation/intent binding,
`after_sequence=0`, the submitted event, scanned-through sequence, and the
payload-free event shape; the card renders only event ID, sequence, emitted
time, type, and scanned-through metadata. Re-expansion uses the cached page;
selection, refresh, authorization, sign-out, and stale generation clear it.

Widget coverage proves zero-before-expand/one-after-expand behavior, cached
re-expansion, foreign binding rejection, invalid event type rejection, and
the default no-request path. The authenticated inert E2E expands the real
Sessions receipt and requires one owner-scoped timeline GET while rejecting
widget Run-intent writes. No pending submission, ordinary Run, device
selection, lease, reservation, scheduling, dispatch, Runner, execution,
receipt, or Audit operation was added. Production run-intent/device/execution
routes remain 404; ADR-0039 remains planning-only, ADR-0113/0114 remain
Proposed/null, and P3b/P4 retain their separate acceptance gates.

### Cross-device plan §248 — Flutter Sessions pending timeline refresh lifecycle

The shared Sessions refresh path now records whether change-feed
synchronization already refreshed pending Run-intent metadata. A manual
refresh therefore performs at most one bounded pending metadata GET even when
it reloads the owner Conversation snapshot. When the refreshed page remains
the same Conversation with the same closed intent metadata, validated
payload-free timeline pages stay attached and an expanded receipt keeps its
event markers. Changed or failed pages, Conversation switch, authorization
failure, sign-out, and stale generations clear timelines and invalidate
in-flight responses.

Widget coverage proves change-feed refresh call count and expanded timeline
retention. No Prompt body, pending submission, ordinary Run, device selection,
lease, reservation, scheduling, dispatch, Runner, execution, receipt, or Audit
operation was added. Production run-intent/device/execution routes remain 404;
ADR-0039 remains planning-only, ADR-0113/0114 remain Proposed/null, and P3b/P4
retain their separate acceptance gates.

### Cross-device plan §249 — Flutter Sessions owner-scoped inventory refresh lifecycle

The shared Sessions screen now keeps the last owner-validated inventory
snapshot visible while an explicitly injected candidate refresh is pending. A
temporary network, service, or response-validation failure marks the retained
rows stale instead of presenting an empty device pool; a successful response
replaces the snapshot and clears the marker. Authorization failure, sign-out,
owner/reader replacement, and stale generation responses still clear the
projection and cannot restore old data. Widget coverage proves the panel and
stale marker remain during a blocked refresh and after a bounded 503, while
foreign-owner and 401 cases remain fail-closed. No enrollment, heartbeat,
authoritative inventory, reservation, scheduling, dispatch, Runner, execution,
receipt, or Audit operation was added. Production `/api/v1/devices` remains
404; ADR-0039 remains planning-only, ADR-0114 remains Proposed/null, and P3b
and P4 retain their separate acceptance gates.

### Cross-device plan §250 — Flutter Web shared-session deep-link and cold-reload parity

The authenticated browser E2E now opens the real owner-bound route at
`/forge/conversations/{conversation_id}`, verifies that the Conversation is
selected without a title click, reloads the same route, and proves that the
persisted owner change-feed cursor resumes after the initial cursor-0
bootstrap. Existing `/forge/` and Run-observation flows keep their previous
navigation paths. The lazy inventory assertion first resolves the leading row
before scrolling toward the tail, so viewport virtualization does not turn a
present observation into a false failure.

The real Chromium run passes through the authenticated inert server and keeps
the recorder contract limited to owner-scoped session reads, change-feed
reads, and the expected Prompt write. No device registration, enrollment,
heartbeat, authoritative inventory, target selection, reservation,
scheduling, dispatch, Runner, execution, receipt, or Audit operation was
added. Production `/api/v1/devices`, run-intent, and execution routes remain
404; ADR-0039 remains planning-only, ADR-0113/0114 remain Proposed/null, and
P3b/P4 retain their separate acceptance gates.

### Cross-device plan §251 — Lossless persisted inventory observation v2 contract parity

Go and Rust now build the explicit offline-only
`forge.device-inventory-observation/v2` envelope from restored inventory
values. The v2 shape retains reservation state, all declared GPUs, persisted
revision, Runner generation, and heartbeat sequence, with owner binding,
stable device/instance/GPU ordering, JSON-safe bounds, and all-false authority
bits. Snaplink Console adds a strict v2 decoder/encoder and mutation tests;
Rust CLI/TUI add a bounded local `persisted-observation-v2` preview, and the
same fixture is consumed across Go, Rust, CLI/TUI, and Flutter while the v1
envelope remains unchanged.

This remains a value and contract slice. It adds no storage write,
enrollment, heartbeat listener, authoritative inventory route, reservation,
target selection, scheduling, dispatch, Runner contact, execution, receipt,
or Audit operation. Production `/api/v1/devices` and execution routes remain
404; ADR-0039 remains planning-only, ADR-0113/0114 remain Proposed/null, and
P3b/P4 retain their separate acceptance gates.

### Cross-device plan §252 — Flutter Sessions lossless v2 inventory preview panel

Snaplink Console's shared Sessions surface now accepts an explicit v2
persisted-inventory observation and renders its revision, Runner generation,
heartbeat sequence, reservation state, resources, and all declared GPUs in a
read-only panel. The panel has no reader or action callback; the default Gate
does not inject it and therefore keeps the existing route request-free for
v2. A fixture-backed widget test proves both Runner rows, multi-GPU values,
all-false authority markers, and the absence of controls.

This remains an injected display seam. It adds no v2 route, storage write,
enrollment, heartbeat listener, authoritative inventory publication,
reservation, target selection, scheduling, dispatch, Runner contact,
execution, receipt, or Audit operation. Production `/api/v1/devices` and
execution routes remain 404; ADR-0039 remains planning-only, ADR-0113/0114
remain Proposed/null, and P3b/P4 retain their separate acceptance gates.

### Cross-device plan §253 — Lossless v2 offline placement comparison

Go Core and Rust Runtime now compare the complete v2 persisted-inventory
observation against fixed caller requirements. Each sorted decision retains
revision, Runner generation, heartbeat sequence, reservation state, GPU count,
and aggregate available GPU memory. A reserved declaration is an explicit
`device_reserved` exclusion; accelerator runtime requirements fail closed
because the v2 observation has no accelerator runtime claim. The shared fixture
tests multi-GPU retention, stale/expired declarations, owner drift, duplicate
devices, deterministic exclusion reasons, and all-false authority.

This remains a pure offline comparison. It selects no target and performs no
reservation, lease, scheduling, dispatch, Runner contact, execution, receipt,
or Audit operation. Production device and execution routes remain 404;
ADR-0039 remains planning-only, ADR-0113/0114 remain Proposed/null, and P3b/P4
retain their separate acceptance gates. The cross-client CLI/TUI and Flutter
consumers are covered by the §254 slice below.

### Cross-device plan §254 — Cross-client v2 placement comparison consumers

Rust CLI and the authenticated TUI now provide a file-only local
`device inventory placement-evaluation-v2 --input FILE|-` preview. Snaplink
Console strictly decodes the same lossless v2 placement fixture and binds each
decision to its observed revision, Runner generation/heartbeat, reservation,
and multi-GPU totals before exposing metadata. Selected target IDs remain null
and every authority bit remains false.

This is a local contract consumer only. It performs no HTTP request,
enrollment, heartbeat, authoritative inventory publication, reservation,
scheduling, dispatch, Runner contact, execution, receipt, or Audit operation.
Production device and execution routes remain 404; ADR-0039 remains
planning-only, ADR-0113/0114 remain Proposed/null, and P3b/P4 retain their
separate acceptance gates.

### Cross-device plan §255 — Flutter Sessions v2 placement comparison panel

Snaplink Console's Sessions Gate and Screen now accept an optional decoded v2
placement evaluation. The read-only panel renders caller requirements and each
observation-bound decision's reservation, GPU totals, persisted revision,
Runner generation/heartbeat, sorted exclusion reasons, null selection, and
all-false authority. The default Gate leaves the value unset and makes no
placement request.

This is an injected display seam only. It performs no HTTP request, target
selection, reservation, scheduling, dispatch, Runner contact, execution,
receipt, or Audit operation. Production device and execution routes remain 404;
ADR-0039 remains planning-only, ADR-0113/0114 remain Proposed/null, and P3b/P4
retain their separate acceptance gates.

### Cross-device plan §256 — Rust v2 placement declaration canonicality parity

The Rust v2 placement adapter now shares Go Core and Snaplink Console's
fail-closed declaration rules: device and GPU rows are ordered and unique,
resource tags are already lowercase and unique, and v2 state fields stay within
the bounded vocabulary and limits. An exhausted CPU or memory value of zero is
kept as a valid observation and reported through the normal insufficiency
reason. This closes cross-runtime normalization drift without adding storage,
enrollment, heartbeat, authority, target selection, reservation, scheduling,
dispatch, Runner, execution, receipt, Audit, or production device behavior;
ADR-0039 remains planning-only, ADR-0113/0114 remain Proposed/null, and P3b/P4
retain their separate acceptance gates.

## §257 — v2 placement preview integrity

Completed a strict Go local `persisted-observation-v2` consumer, JSON-safe
aggregate GPU bounds, Flutter v2 decision recomputation, and Rust/observation
runtime-count parity. The shared fixture still produces no selected target and
all authority bits remain false. The Flutter raw-text entrypoint also rejects
duplicate JSON object keys before map decoding. No production device,
enrollment, heartbeat, scheduling, dispatch, Runner, execution, receipt, or
Audit route was enabled.

## §258 — v2 capability-lease boundary parity

Go's lossless v2 observation validator and Snaplink Console's v2 observation
decoder now enforce the same 1-second minimum and 10-minute maximum capability
lease TTL already enforced by Rust's restored Runner. Regression tests cover
both rejected edges and the inclusive bounds while preserving the shared
fixture's valid lease. This is offline value validation only; no device route,
enrollment, heartbeat, inventory authority, target selection, reservation,
scheduling, dispatch, Runner, execution, receipt, or Audit operation was added.

## §259 — v2 CLI and runtime declaration boundary parity

Rust CLI v2 preflight now accepts zero available CPU as an exhausted
observation, uses the shared 32-runtime and JSON-safe capability bounds, and
covers that path locally. Go v2 observation validation and Flutter v2 decoding
share the lowercase ASCII runtime/OS/architecture grammar and 64-byte limit,
rejecting URL punctuation and overlong declarations. This remains offline
value validation only; no device route, enrollment, heartbeat, inventory
authority, target selection, reservation, scheduling, dispatch, Runner,
execution, receipt, or Audit operation was added.

## §260 — Flutter raw v2 inventory envelope duplicate-key parity

Snaplink Console's lossless `forge.device-inventory-observation/v2` model now
has a bounded raw-text entrypoint. It scans nested JSON objects for duplicate
names before `dart:convert` materializes maps, rejecting root and nested
duplicates while preserving the valid fixture round trip and existing typed
map entrypoint.

This is transport parsing hygiene only. It adds no device route, enrollment,
heartbeat listener, authoritative inventory, target selection, reservation,
scheduling, dispatch, Runner contact, execution, receipt, or Audit operation.
Production device and execution routes remain 404; ADR-0039 remains
planning-only, ADR-0113/0114 remain Proposed/null, and P3b/P4 retain their
separate acceptance gates.

### Cross-device plan §261 — Rust persisted-observation CLI canonical-tag parity

The Rust CLI/TUI `persisted-observation-v2` reader now uses the same lowercase
ASCII `._-+` canonical grammar and 64-byte limit as Go, Flutter, and the Rust
placement evaluator for v2 OS, architecture, and runtime declarations.
Uppercase, URL-style punctuation, and Unicode values fail closed before the
offline observation is rendered; GPU vendor labels keep their separate display
label grammar. Regression coverage exercises all three malformed declaration
classes while the shared fixture still round-trips exactly.

This is local validation hygiene only. It adds no device route, enrollment,
heartbeat listener, authoritative inventory, target selection, reservation,
scheduling, dispatch, Runner, execution, receipt, or Audit operation. ADR-0039
remains planning-only, ADR-0113/0114 remain Proposed/null, and P3b/P4 retain
their separate acceptance gates.

### Cross-device plan §262 — Flutter Sessions Gate precondition preview wiring

The shared Web/App/Mobile `ForgeSessionsGate` now forwards explicitly injected
Attempt-request and pending Run-intent fixture values through the real Gate to
the Sessions screen. A widget regression mounts that path, verifies both
read-only cards, and records exactly one owner-scoped Conversation GET; all
fixture authority remains false and no device or execution request is made.

This is execution-precondition display wiring only. It adds no Attempt or
Run-intent production write, Run creation, device route, enrollment, heartbeat,
authoritative inventory, target selection, reservation, scheduling, dispatch,
Runner contact, execution, receipt, or Audit operation. Production device and
execution routes remain 404; ADR-0039 remains planning-only, ADR-0113/0114
remain Proposed/null, and P3b/P4 retain their separate acceptance gates.

### Cross-device plan §263 — Rust v2 persisted-observation capability-lease parity

Rust's lossless v2 placement domain and CLI/TUI persisted-observation reader
now enforce the shared 1-second minimum and 10-minute maximum capability lease
TTL used by Go, Flutter, and the restored Runner contract. Lower and upper
edges are accepted, while short and overlong leases fail closed before local
rendering or offline comparison.

This is offline declaration validation only. It adds no device route,
enrollment, heartbeat listener, authoritative inventory, target selection,
reservation, scheduling, dispatch, Runner contact, execution, receipt, or Audit
operation. Production device and execution routes remain 404; ADR-0039 remains
planning-only, ADR-0113/0114 remain Proposed/null, and P3b/P4 retain their
separate acceptance gates.


### Cross-device plan §264 — Flutter Sessions Gate v2 placement precondition parity

The shared Web/App/Mobile `ForgeSessionsGate` now has a real-path widget
regression for forwarding an explicitly injected lossless v2 placement
evaluation to the Sessions screen. The Gate performs the normal single
owner-scoped Conversation read, renders observation-bound Runner decision
metadata, and makes no `/devices` request. Rust CLI/TUI already consume the
same v2 fixture through bounded file-only previews, so the five client
surfaces share tested display wiring for this placement precondition.

This remains injected, offline, read-only wiring. It does not select a target,
create a reservation or lease, schedule or dispatch work, contact a Runner,
execute a command, persist a receipt, or publish Audit evidence. Production
device and execution routes remain 404; ADR-0039 remains planning-only,
ADR-0113/0114 remain Proposed/null, and P3b/P4 retain their separate
acceptance gates.

### Cross-device plan §265 — Aero-ID and Audit Governance observer receiver hardening

Aero-ID and Snaplink Audit Governance now recursively reject duplicate JSON
object names before decoding the shared `forge.run.observed.v1` evidence
fixture, including the root and nested `authority` objects. Aero-ID's observer
publisher copies only non-empty, trimmed, non-control-character
`execution_run_id` and `workflow_instance_id` correlation values into the
governance envelope; non-string or unsafe values are omitted.

This is deterministic, content-free evidence handling only. It adds no event
publication, outbox write, device registration, heartbeat, inventory authority,
reservation, scheduling, dispatch, Runner execution, receipt persistence, or
Audit authority. ADR-0039 remains planning-only, ADR-0113/0114 remain
Proposed/null, and P3b/P4 retain their separate acceptance gates.

### Cross-device plan §266 — Cross-platform credential-storage preflight contract

Forge Runtime now provides a local, read-only `remote credentials status`
command with the versioned `forge.remote-credential-capabilities/v1` contract.
It reports platform, `FORGE_ACCESS_TOKEN` fallback, OS refresh-token keyring,
credential metadata persistence, refresh locking, and aggregate saved-login
availability independently. The command never opens a keyring, creates files,
contacts Snaplink, or touches device/execution routes. Windows is included in
the keyring backend compilation boundary, while its absent secure metadata and
refresh-lock pieces remain explicit and continue to require
`FORGE_ACCESS_TOKEN`; Android and other unsupported targets fail closed too.
This is capability discovery only: no credential persistence, enrollment,
heartbeat, inventory authority, target selection, reservation, scheduling,
dispatch, Runner, execution, receipt, or Audit behavior was enabled. ADR-0039
remains planning-only; ADR-0113/0114 remain Proposed/null; P3b/P4 remain gated.

### Cross-device plan §267 — Flutter native OAuth client-slot isolation

Snaplink Console's native `ForgeCredentialStore` now derives a bounded secure
storage key from each non-default OAuth client ID. The legacy first-party Forge
key remains stable for cold-start migration compatibility, while additional
client slots no longer overwrite or clear one another during store, restore,
refresh, or sign-out. A shared-backend regression writes and restores two
access/refresh pairs, clears one slot, and verifies the other remains usable.
The web `sessionStorage` path and all device/execution routes remain unchanged;
ADR-0039 remains planning-only, ADR-0113/0114 remain Proposed/null, and P3b/P4
remain gated.

### Cross-device plan §268 — Go owner-bound lossless v2 inventory candidate

Forge Core now provides a separate private `/api/v1/devices/observations/v2`
candidate constructor for the lossless inventory envelope. It requires an
explicitly enabled injected source, derives the owner from verified claims,
requires `forge:devices:read`, validates all v2 declaration bounds and
all-false authority, and applies the response budget. A restored-state adapter
builds this observation without storage, heartbeat, or selection side effects;
route tests cover default-disabled, owner binding, foreign-owner, method/query/
scope, source failure, cancellation, and the lossless reservation/multi-GPU
values. Production route constructors do not mount the candidate, so device
routes remain 404; no enrollment, heartbeat listener, inventory authority,
selection, reservation, scheduling, dispatch, Runner, execution, receipt, or
Audit behavior was enabled. ADR-0039 remains planning-only, ADR-0113/0114
remain Proposed/null, and P3b/P4 remain gated.

### Cross-device plan §269 — Bounded CLI change-feed watch and backoff

Forge Runtime now exposes a bounded `remote changes watch` helper for clients
without push transport. It polls the authenticated owner-bound dense feed from
the saved Coordinator/account cursor, returns observed rows and the final
cursor, exponentially backs off across empty pages, and resets the delay for
changes or continuation pages. Saved cursors advance after each valid page;
explicit `--after-cursor` watches never replace the saved checkpoint. Poll and
delay bounds are finite and validated, and a regression observes a change
after two empty pages. This extends Conversation read delivery only: no push
route, Prompt/Run mutation, device/execution route, enrollment, heartbeat,
inventory authority, selection, reservation, scheduling, dispatch, Runner,
receipt, or Audit behavior was enabled. ADR-0039 remains planning-only,
ADR-0113/0114 remain Proposed/null, and P3b/P4 remain gated.

### Cross-device plan §270 — Flutter Web OAuth lifecycle contract

Snaplink Console now has a browser-only OAuth lifecycle regression running
against Chrome's actual `sessionStorage` adapter. It cold-mounts a second
`ForgeCredentialStore`, rotates an expired access token after a Conversation
401 through the public refresh grant, retries the read once with the new
token, and verifies the rotated access/refresh pair. Forge-only cleanup then
clears the Forge slot while preserving the Admin slot. The test uses an
injected HTTP client and reaches no Snaplink, device, inventory, scheduling,
Runner, or execution route; ADR-0039 remains planning-only,
ADR-0113/0114 remain Proposed/null, and P3b/P4 remain gated.

### Cross-device plan §271 — Flutter adaptive change-feed polling and recovery

Snaplink Console's shared Web/App/Mobile `ForgeSessionsScreen` now schedules
one-shot owner change-feed reads with bounded adaptive backoff. The normal
15-second interval expands to 30, 60, and at most 120 seconds after
consecutive feed or required snapshot failures; a valid empty page is a
successful recovery and resets the next interval to 15 seconds. Pause,
sign-out, authorization invalidation, and disposal cancel the pending timer,
and resumed routes do not overlap feed reads. The owner-local cursor remains
committed only after the required authenticated GET refreshes succeed. A
widget regression proves the 15-to-30-second backoff and recovery to 15
seconds after a valid empty page. No push route, Prompt/Run mutation,
device registration, enrollment, heartbeat, inventory authority, target
selection, reservation, scheduling, dispatch, Runner, execution, receipt,
or Audit behavior was added; ADR-0039 remains planning-only,
ADR-0113/0114 remain Proposed/null, and P3b/P4 remain gated.

### Cross-device plan §273 — Snaplink JWT to Flutter lossless v2 inventory candidate evidence

Forge Core now proves the private lossless v2 inventory candidate across
processes: a real in-memory Snaplink SSO issuer signs the bearer token, an
explicit test mux mounts the owner-bound Go source, and a separate Flutter
test process reads `/api/v1/devices/observations/v2` through the authenticated
API client and strictly decodes the exact envelope. The regression retains
owner, revision/generation/heartbeat, reservation, two GPU declarations, and
the all-false authority flags. Go route coverage continues to exercise scope,
request-shape, foreign-owner, source-failure, cancellation, and response
budget failures. The production handler remains 404 because the candidate is
never mounted by production constructors; no enrollment, heartbeat,
authoritative inventory, selection, reservation, scheduling, dispatch,
Runner, execution, receipt, or Audit behavior was enabled. ADR-0039 remains
planning-only, ADR-0113/0114 remain Proposed/null, and P3b/P4 remain gated.

### Cross-device plan §272 — Rust TUI bounded change-feed watch consumer

The authenticated Rust remote TUI now consumes the same finite owner-feed
watch as `remote changes watch`. `changes watch` parses bounded polls and
backoff, supports an explicit one-off cursor, renders validated metadata-only
change rows, advances the in-process cursor, and updates aggregate versions
for loaded sessions. Saved cursor checkpointing remains in the shared client
after each valid advancing page; explicit cursors never replace the saved
checkpoint. Mock TUI coverage serves empty and changed pages, requires
authenticated GET requests with empty bodies, verifies state/render updates,
and rejects invalid bounds before making a request. No push transport,
Prompt/Run mutation, device registration, enrollment, heartbeat, authoritative
inventory, target selection, reservation, scheduling, dispatch, Runner,
execution, receipt, or Audit behavior was added; production device and
execution routes remain 404, ADR-0039 remains planning-only,
ADR-0113/0114 remain Proposed/null, and P3b/P4 remain gated.

### Cross-device plan §274 — Rust TUI pending Run-intent timeline resume boundary

The authenticated Rust remote TUI now accepts `run-intents timeline
INTENT_ID --resume` after a successful metadata-only timeline read in the same
process. It binds the in-process `scanned_through_sequence` to the exact
selected Conversation and pending intent, requests the next page with that
cursor, rejects session/intent drift and cursor regression, and clears the
owner-scoped observation after authorization failure. The checkpoint is
intentionally process-local; no durable pending-intent resume file is inferred
until its separate contract is accepted.

Mock TUI coverage proves the initial sequence-zero read, the bound sequence-one
resume, empty continuation and single-event rendering, plus rejection before
any request when no exact prior binding exists. This extends authenticated
pending-intent observation only. No Prompt/Run mutation, public consent route,
device registration, enrollment, heartbeat, authoritative inventory, target
selection, reservation, scheduling, dispatch, Runner, execution, receipt, or
Audit behavior was added; production device and execution routes remain 404,
ADR-0039 remains planning-only, ADR-0113/0114 remain Proposed/null, and P3b/P4
remain gated.

### Cross-device plan §276 — Aero-IM/Aero-Vault Forge observer evidence conformance

Aero-IM's audit connector and Aero-Vault's governance relay now consume
byte-identical copies of Catalyst's content-free `forge.run.observed.v1`
fixture in standalone receiver contract tests. Both reject recursive duplicate
JSON keys, unknown/content-bearing fields, unsafe metadata, and enabled
authority; Aero-IM also proves the bounded payload can cross its typed audit
envelope without Prompt/result/tool/token/artifact content. The conformance
runner compares both external copies with Catalyst's canonical fixture and runs
the focused tests. This is evidence compatibility only: no publisher, outbox,
relay activation, device registration, enrollment, heartbeat, authoritative
inventory, selection, reservation, scheduling, dispatch, Runner, execution,
receipt, or Audit authority was added. ADR-0039 remains planning-only,
ADR-0113/0114 remain Proposed/null, and P3b/P4 remain gated.

### Cross-device plan §275 — Flutter pending Run-intent metadata cursor pagination

Snaplink Console's shared Web/App/Mobile Forge surface now accepts an explicit
paged pending Run-intent reader. The first read uses a null cursor; `Load more
pending Run-intents` passes the strict `next_cursor` to the next bounded read,
revalidates Conversation/order/duplicate identity boundaries, and merges older
metadata while preserving already loaded payload-free timeline markers. The
legacy single-page callback remains compatible and the default Gate leaves both
candidate readers unset. Widget coverage drives two pages, asserts the cursor
binding, and verifies the continuation row/control lifecycle. No Prompt body,
consent write, Run creation, device registration, inventory authority,
selection, reservation, scheduling, dispatch, Runner, execution, receipt, or
Audit operation was added; production `/run-intents` and device/execution
routes remain 404, ADR-0039 remains planning-only, ADR-0113/0114 remain
Proposed/null, and P3b/P4 remain gated.
- **DONE — Rust CLI/TUI authenticated lossless v2 inventory candidate (cross-device plan §277)**: Forge Runtime adds `remote inventory show-v2` and TUI `inventory read-v2` for the private `/api/v1/devices/observations/v2` candidate. Each performs one logical authenticated GET with bounded transient retry, validates the owner-bound lossless v2 envelope including revision, Runner generation/heartbeat, reservation, multi-GPU rows, and all-false authority, and renders only metadata. Mock CLI/TUI coverage rejects authority mutation and proves the exact request path/empty body. Production constructors never mount this candidate, so the route remains 404; no registration, enrollment, heartbeat, authoritative inventory, selection, reservation, scheduling, dispatch, Runner, execution, receipt, or Audit behavior was added. ADR-0039 remains planning-only, ADR-0113/0114 remain Proposed/null, and P3b/P4 remain gated.
- The §277 Go E2E can optionally launch the real Rust CLI when `FORGE_RUNTIME_BIN` is set, comparing the CLI envelope with the same authenticated Go and Flutter reads; the default contract runner leaves this cross-process check opt-in.

### Cross-device plan §278 — Flutter authenticated lossless v2 inventory reader lifecycle

Snaplink Console's shared Web/App/Mobile `ForgeSessionsGate` and
`ForgeSessionsScreen` now accept an optional owner-bound
`ForgeDeviceInventoryV2Reader`. When supplied, the screen reads the existing
`ForgeConversationsApi.readDeviceInventoryCandidateV2` seam after the owner
snapshot, strictly re-decodes and owner-checks the v2 page, and renders the
actual response through the read-only lossless v2 panel. Refresh repeats the
bounded GET; transient failures retain the last validated page and mark it
stale, while a 401 clears owner sessions and the v2 projection with the v1
lifecycle behavior. The default Gate leaves the reader unset and sends no v2
device request. API and widget regressions cover the exact path, owner drift,
Runner/revision/GPU/reservation rendering, refresh, stale retention, 401
cleanup, and the request-free default. No enrollment, heartbeat, authoritative
inventory, selection, reservation, scheduling, dispatch, Runner, execution,
receipt, or Audit operation was added; production v2 device routes remain 404,
ADR-0039 remains planning-only, ADR-0113/0114 remain Proposed/null, and P3b/P4
remain gated.

### Cross-device plan §279 — Local Run execution-evidence preview and selected-Run metadata card

Forge Runtime now accepts the canonical content-free
`forge.run.execution-evidence.v1` fixture through local
`device run-execution-evidence-preview --input FILE|-`; the TUI exposes the
same preview for a path-only file and reuses bounded input, recursive duplicate
key rejection, strict unknown/authority/content validation, and metadata-only
output. It never performs HTTP, device access, persistence, target selection,
reservation, dispatch, process execution, or Audit publication.

The shared Snaplink Console Web/App/Mobile Gate and Sessions screen accept an
explicit `ForgeRunExecutionEvidence`, strictly re-decode it, require the exact
selected Conversation/Run binding, and render a metadata-only card with
uncertainty/reconciliation and all-false authority. The owner reference stays a
digest and is not turned into an owner identity. Focused Rust/Flutter tests
cover fixture parity, duplicate/unknown/authority/content rejection, selected
Run matching, strict re-decode, reconciliation display, and the request-free
boundary. ADR-0039 remains planning-only, ADR-0113/0114 remain Proposed/null,
and P3b/P4 remain separately gated; no live inventory, scheduling, dispatch,
Runner, execution, receipt persistence, or Audit behavior was added.

### Cross-device plan §280 — Run observer preview and selected-Run metadata parity

Forge Runtime adds fixture-only local CLI/TUI previews for
`forge.run.observed.v1`, with bounded strict decoding,
duplicate/unknown/content/authority rejection, and metadata-only rendering.
Snaplink Console's shared Gate/Screen accepts an explicit `ForgeRunObserved`,
strictly re-decodes it, requires the selected Conversation/Run binding, and
renders the opaque owner digest plus bounded Run metadata in a read-only card.
No default observer request or HTTP/device/execution route was added;
ADR-0039 remains planning-only, ADR-0113/0114 remain Proposed/null, and P3b/P4
remain gated.

### Cross-device plan §281 — Host-side native cold-start shared-session lifecycle

Snaplink Console adds `forge_mobile_shared_session_e2e_test.dart`, a host-side
native lifecycle harness that uses the Android/iOS `ForgeCredentialStore`
abstraction with an injected secure-store backend. It clears the process
credential slot, restores the same owner Conversation across two cold starts,
appends and replays one idempotent Prompt, checks aggregate version and the
owner change-feed cursor, and proves exactly one matching history row. The
initial `android-host` marker made the host-only result explicit; §348 adds the
matching `ios-host` variant. The opt-in Go E2E launches it through the real
Snaplink JWT → Go → Rust path, records the exact
prompt/session/change-feed calls, rejects devices, Run-intents, placement and
dispatch, and rechecks detail/history from Go. No physical Android run is
claimed; ADR-0039 remains planning-only, ADR-0113/0114 remain Proposed/null,
and no device or execution authority was added.

### Cross-device plan §282 — Populated-Run ForgeRunObserved Flutter live E2E

The opt-in populated-Run Snaplink JWT → Go → Rust E2E now uses pure
`ProjectRunObserved` projection output in a private 0600 Flutter input file.
The native Flutter test strictly decodes it, checks `isFor`/`isDisplayOnly`,
injects the value through `ForgeSessionsGate`, and requires the selected Run
metadata card. Local fake-API probes verify foreign, content-bearing, and
authoritative values remain hidden without changing the live recorder request
sequence. No observer route, device or execution authority, or Audit behavior
was added; ADR-0039 remains planning-only and ADR-0113/0114 remain
Proposed/null.

### Cross-device plan §283 — Flutter selected-Run observer reader lifecycle

Snaplink Console's shared Web/App/Mobile Sessions surface accepts an explicit
`ForgeRunObservedReader` callback for one selected Conversation/Run. The
screen strictly re-decodes the returned `forge.run.observed.v1` value, checks
the exact selected binding and all-false display boundary, refreshes it through
the existing owner/session refresh lifecycle, and retains the last validated
value during a transient refresh failure. The default Gate remains reader-free
and performs no observer request. Focused widget coverage proves loading,
binding, strict display-only validation, and refresh replacement; no observer
route, device or execution authority, or Audit behavior was added. ADR-0039
remains planning-only and ADR-0113/0114 remain Proposed/null.

- **DONE — Rust CLI/TUI Runner lease-fencing preview parity (cross-device plan §284)**: Forge Runtime now exposes `device runner-lease-fencing-preview --input FILE|-` and path-only TUI `runner-lease-fencing-preview --input FILE`. Both consume the canonical fixture with bounded duplicate/unknown/schema/authority/input checks, exercise lease renewal/proof/terminal/replay/conflict/uncertain cases, and emit only lease metadata and redacted case outcomes. Focused CLI/TUI tests prove the request-free boundary and all-false authority; no lease is issued or persisted, no device is selected/reserved, no work is scheduled/dispatched, no Runner is contacted, and no process, receipt, or Audit evidence is produced. ADR-0039 remains planning-only, ADR-0113/0114 remain Proposed/null, and P3b/P4 remain gated.
- **DONE — Android secure-storage instrumentation boundary (cross-device plan §285)**: Snaplink Console adds `ForgeCredentialStorageInstrumentedTest` against the actual native `flutter_secure_storage` Android backend and a runner that requires an explicit disposable `emulator-N`. The test restores a Forge-shaped credential through a second storage instance and verifies deletion/cleanup; no serial yields an explicit `SKIP`, and an unavailable serial fails. This is storage-boundary evidence only and does not start Flutter/MainActivity, claim process recreation, or prove physical Android shared-session Prompt behavior. The host-side JWT → Go → Rust lifecycle test remains the authenticated session evidence; no device/execution authority was added, ADR-0039 remains planning-only, ADR-0113/0114 remain Proposed/null, and P3b/P4 remain gated.

### Cross-device plan §288 — Flutter placement-batch observation parity

Snaplink Console's shared Web/App/Mobile Forge Sessions surface now accepts an
explicit `ForgeDeviceInventoryPlacementBatchEvaluationFixture` and renders the
canonical persisted-inventory placement batch value. The panel shows fixed
evaluation metadata, requirements, candidate revisions, and exclusion reasons
while retaining null selection and all-false authority. The default Gate leaves
the value unset. Widget tests use the canonical fixture through the real
`ForgeSessionsScreen` and assert no `/devices` or placement request. This is
P3a observation parity only; no inventory authority, selection, reservation,
scheduling, dispatch, Runner, execution, receipt, or Audit behavior was added.
ADR-0039 remains planning-only and ADR-0113/0114 remain Proposed/null.
- **DONE — Aero-IM/Aero-Vault execution-evidence receiver parity (cross-device plan §287)**: Aero-IM and Aero-Vault now consume byte-identical copies of Catalyst's `forge.run.execution-evidence.v1` fixture. Their strict receiver tests reject duplicate/unknown/content-bearing fields, unsafe metadata, digest drift, and enabled authority; Aero-IM also proves the value survives its typed audit payload envelope without Prompt/result/tool/token/artifact content. The contract runner compares both copies and runs both focused suites. This is compatibility evidence only: no event publication, outbox/relay activation, receipt persistence, Runner contact, device registration, inventory, reservation, scheduling, dispatch, or execution authority was added; ADR-0039 remains planning-only, ADR-0113/0114 remain Proposed/null, production routes remain closed, and P3b/P4 remain gated.
- **DONE — Flutter response duplicate-key scanner hardening (cross-device plan §286)**: `ForgeConversationsApi` now validates JSON structure before Dart map materialization, deduplicating only actual object member names and normalizing escaped Unicode key aliases. Strings containing braces/colons, arrays, literals, nested objects, and invalid JSON are covered by API/resilience/model/origin regressions. This is response-integrity hardening only; no device or execution authority was added, ADR-0039 remains planning-only, ADR-0113/0114 remain Proposed/null, and P3b/P4 remain gated.
- **DONE — Owner-scoped persisted inventory file read boundary (cross-device plan §289)**: Forge Core's injected read-only `forge.device-inventory-file/v1` adapter restores a complete 0600 regular-file envelope through `statefs`, rejects symlink/permission/size/duplicate/unknown-field drift, validates the exact owner tuple, projects at a fixed caller time, and survives a separate-process revision read. Private appserver v1/v2 bridges and default-off 404 tests use it without constructing production routes; no enrollment, heartbeat, inventory authority, selection, reservation, scheduling, dispatch, Runner, execution, receipt, or Audit operation was added. ADR-0039 remains planning-only, ADR-0113/0114 remain Proposed/null, and P3b/P4 remain gated.
- **DONE — Android Activity recreation shared-session lifecycle (cross-device plan §290)**: Debug Android instrumentation starts the real Forge Gate/Screen, restores an OAuth-shaped credential from native secure storage, exercises authenticated Conversation/Prompt/Run reads, recreates `MainActivity`, and verifies a second authenticated Conversation read via a debug-only recorder channel. The runner requires an explicit disposable `emulator-N`, reports a clear skip without one, and fails an unavailable requested serial. It is native lifecycle evidence only; no physical remote execution or device authority was enabled, and ADR-0039/0113/0114 remain gated.
- **DONE — Injected local Runner execution preview (cross-device plan §291)**: Forge Core's local/test adapter accepts validated Runner intent plus caller-supplied lease proof and observation time, invokes only an injected direct-argv executor, bounds output, maps failure/uncertainty, and returns metadata-only terminal/session receipt observations with all authority bits false. Fake, direct-process, lease, output, uncertainty, and mutation regressions pass; no production route constructs it and no device selection, remote dispatch, lease persistence, receipt persistence, or Audit publication was added.
- **DONE — Flutter Runner lease/fencing preview import (cross-device plan §293)**: Snaplink Console's shared Web/App/Mobile Forge Sessions surface accepts bounded workspace JSON for the strict `forge.runner-lease-fencing/v1` fixture and renders only schema, lease timing, and redacted case outcomes. Fencing tokens, receipt digests, terminal reasons, target, reservation, dispatch, and action fields stay hidden; authority remains false. A compact, semantic AppBar action keeps the request-free import reachable without changing the session list scroll geometry. No device request, enrollment, heartbeat, inventory authority, selection, reservation, scheduling, dispatch, Runner, execution, receipt, or Audit behavior was added; ADR-0039 remains planning-only, ADR-0113/0114 remain Proposed/null, and P3b/P4 remain gated.
- **DONE — Runner dispatch-plan preview value contract (cross-device plan §294)**: Forge Core's pure `ObserveRunnerDispatchPlanPreview` joins offline placement candidates, Attempt lifecycle state, Runner intent target, and a lease grant into deterministic declarative metadata. It reports requirement, target, lease, and Attempt admissibility while keeping selected target null and all authority false; no fencing token, argv, workspace, output, reservation, or dispatch command is represented. Focused Go tests cover accepted, terminal, expired, owner/target/lease mismatch, authority mutation, ordering, and input immutability. No registry, clock, selection, reservation, scheduling, dispatch, Runner, process, receipt, or Audit operation was added; ADR-0039 remains planning-only, ADR-0113/0114 remain Proposed/null, and P3b/P4 remain gated.
- **DONE — Atomic multi-instance persisted inventory read (cross-device plan §295)**: Forge Core's injected `forge.device-inventory-file-set/v1` adapter reads one owner-bound, atomically replaced 0600 regular-file image containing up to 128 complete device/Runner states. It rejects aliases, broad permissions, duplicate/unknown/trailing JSON, malformed members, foreign owners, and duplicate device/instance IDs, then returns deterministic ordering; atomic replacement tests prove revision/heartbeat updates without mixed state. The private candidate projects v1/v2 observations, and an opt-in Snaplink JWT E2E sends two instances through Rust CLI/TUI and optional Flutter API. Production device routes remain 404; no enrollment, heartbeat ingestion, inventory authority, selection, reservation, scheduling, dispatch, Runner, execution, receipt, or Audit behavior was added; ADR-0039 remains planning-only, ADR-0113/0114 remain Proposed/null, and P3b/P4 remain gated.
- **DONE — Client-instance/session view contract (cross-device plan §296)**: Forge Core defines the pure `forge.client-instance-session-view/v1` read-only vocabulary for owner-scoped CLI/TUI/Web/App/Mobile instance metadata and opaque session references. Strict Go decoding and the canonical fixture reject duplicate, unknown, null, trailing, owner-drift, unsupported-kind/status, and oversized input; all identity, session-write, device, reservation, execution, dispatch, and Audit authority bits remain false. Rust `device client-session-view-preview --input FILE|-` and the path-only TUI `client-session-view-preview --input FILE` consume the same fixture and render metadata only. No production instance route, registration, session binding, Prompt-to-Run behavior, Agent Hub authority, device enrollment, scheduling, Runner, or execution behavior was added; ADR-0039 remains planning-only, ADR-0113/0114 remain Proposed/null, and P3b/P4 remain gated.
- **DONE — Flutter client-instance/session view consumption (cross-device plan §297)**: Snaplink Console strictly decodes the shared `forge.client-instance-session-view/v1` fixture and forwards an optional, local value through `ForgeSessionsGate` to the shared `ForgeSessionsScreen`, which re-decodes it before rendering a metadata-only panel for owner, instance kind/status, observation time, and opaque session IDs. The Dart consumer checks duplicate keys before map decoding, exact shapes, owner metadata, sorted unique IDs, supported kinds/statuses, bounds, timestamps, and all-false authority. Canonical fixture tests reject malformed and authority mutations; shared Web/App/Mobile widget coverage drives both the panel and real Sessions screen and asserts no `/devices` request. Default reader/value remains unset, with no network, Agent Hub authority, Prompt/Run write, selection, reservation, dispatch, production route, device enrollment, heartbeat, scheduling, Runner, execution, receipt, or Audit behavior; ADR-0039/0113/0114 and P3b/P4 remain gated.
- **DONE — Client-instance/resource view composition (cross-device plan §298)**: Forge Core defines the pure `forge.client-instance-resource-view/v1` join of owner-declared CLI/TUI/Web/App/Mobile instance/session metadata and sorted device/Runner resource summaries. It keeps client `instance_id` distinct from `runner_instance_id`, bounds revision/generation/heartbeat/capacity/GPU metadata, rejects owner drift, duplicates, lifecycle/capacity inversions, ordering and strict JSON mutations, and fixes all authority false. Rust CLI `device client-instance-resource-view-preview --input FILE|-` and path-only TUI consume the canonical fixture and render metadata only. No production route, enrollment, heartbeat, authoritative inventory, session binding, Prompt-to-Run, selection, reservation, scheduling, Runner, execution, receipt, or Audit behavior was added; ADR-0039 remains planning-only, ADR-0113/0114 remain Proposed/null, and P3b/P4 remain gated.
- **DONE — Flutter client-instance/resource view consumption (cross-device plan §299)**: Snaplink Console strictly consumes the shared `forge.client-instance-resource-view/v1` fixture through an optional local `ForgeSessionsGate`/`ForgeSessionsScreen` value and renders owner, client-instance, Runner, lifecycle, reservation, capacity, and GPU metadata. It re-decodes exact fields, owner bindings, ordering, bounds, and all-false authority before display; widget coverage drives both the panel and real Sessions screen and asserts no `/devices` request. The panel has no Prompt, Run, target-selection, reservation, or execution control, and the default Gate remains null; no production instance/device route, enrollment, heartbeat, authoritative inventory, session binding, Prompt mutation, Run creation, selection, scheduling, dispatch, Runner, execution, receipt, or Audit behavior was added; ADR-0039 remains planning-only, ADR-0113/0114 remain Proposed/null, and P3b/P4 remain gated.
- **DONE — Persisted inventory to cross-client resource view (cross-device plan §300)**: Forge Core maps an explicitly injected owner-bound `forge.device-inventory-file-set/v1` image into the strict `forge.client-instance-resource-view/v1` composition, preserving client `instance_id` versus Runner `runner_instance_id`, revision/generation/heartbeat, liveness/reservation, capacity, and aggregate GPU memory with deterministic sorting and fail-closed owner/duplicate/bounds checks. A private `/api/v1/client-instances/resource-view` candidate is exercised through a real Snaplink JWT and the exact response is consumed by separate Rust CLI/TUI and Flutter API E2E processes; production construction remains 404, the default Sessions Gate remains unset, and no client registration, enrollment, heartbeat, inventory authority, selection, reservation, scheduling, dispatch, Runner, execution, receipt, or Audit behavior was added. ADR-0039 remains planning-only, ADR-0113/0114 remain Proposed/null, and P3b/P4 remain gated.
- **DONE — Explicit Console reader for client-instance/resource view (cross-device plan §301)**: `ForgeSessionsGate` and `ForgeSessionsScreen` now accept an explicit owner plus `ForgeClientInstanceResourceViewReader`, strictly re-decode the returned candidate, require owner/display-only parity, retain the last validated value on refresh failure, and clear it when the reader changes. The default Gate/Screen remain reader-free and request-free; Web/App/Mobile widget coverage exercises the real screen reader seam, while no production instance/device route, registration, enrollment, heartbeat, inventory authority, session binding, Prompt/Run mutation, selection, reservation, scheduling, dispatch, Runner, execution, receipt, or Audit behavior was added. ADR-0039 remains planning-only, ADR-0113/0114 remain Proposed/null, and P3b/P4 remain gated.
- **DONE — Authenticated candidate for client-instance/session view (cross-device plan §302)**: Forge Core adds a private, explicitly enabled `/api/v1/client-instances/session-view` seam that derives owner from verified JWT claims, reuses `forge:conversations:read`, strictly validates the read-only envelope, and remains unmounted by production routes. Snaplink Console accepts an explicit owner plus reader, re-decodes owner/display-only values, preserves stale data during refresh failure, and clears on seam changes. An opt-in Snaplink JWT E2E feeds the same response to Rust CLI/TUI and Flutter API; no client registration, session binding, Prompt/Run mutation, device authority, scheduling, dispatch, Runner, execution, receipt, or Audit behavior was added. ADR-0039 remains planning-only, ADR-0113/0114 remain Proposed/null, and P3b/P4 remain gated.
- **DONE — Authenticated Rust CLI/TUI readers for client-instance candidates (cross-device plan §303)**: Forge Runtime adds bearer-authenticated, no-body `remote client-instances session-view` and `remote client-instances resource-view` reads with strict envelope and all-false authority validation. The TUI renders the same owner-bound metadata, clears its local observation after 401/403, and never selects or dispatches a device. Rust mock-server tests and the opt-in Snaplink JWT E2E now exercise the direct remote CLI/TUI paths against the explicitly enabled candidate mux; production constructors remain 404 and offline fixture readers remain available. No client registration, session binding, enrollment, heartbeat, inventory authority, selection, reservation, scheduling, dispatch, Runner, execution, receipt, or Audit behavior was added. ADR-0039 remains planning-only, ADR-0113/0114 remain Proposed/null, and P3b/P4 remain gated.
- **DONE — Owner-bound local Runner execution-readiness preview candidate (cross-device plan §304)**: Forge Core adds a private, explicitly enabled POST candidate that binds verified JWT owner, conversation, and intent path identities to a strict value-only local Runner preview request. It invokes only an injected deterministic executor, maps completed/failure/uncertain outcomes into metadata-only receipt observations, rejects lease/authority/transport mutations, redacts argv/workspace/fencing/output/error data, and keeps selected target and every authority bit false. The production constructor remains 404; no normal Run/Attempt/device state, inventory authority, reservation, scheduling, remote dispatch, Runner transport, durable receipt, or Audit event is created. ADR-0039 remains planning-only, ADR-0113/0114 remain Proposed/null, and P3b/P4 remain gated.
- **DONE — Authenticated Rust CLI/TUI local Runner preview consumer (cross-device plan §305)**: Forge Runtime adds `remote runner execution-readiness-preview --input FILE|-` and TUI `runner-execution-readiness-preview --input FILE` for the explicitly enabled test candidate. Bounded request validation reuses the Runner intent and lease domains; the client sends one exact owner/path-bound POST without retry, rejects nested identity/authority drift, and renders only metadata (argv, workspace, fencing token, output, and executor diagnostics stay absent). The TUI requires the selected owner-scoped Conversation and clears its local view after 401/403. Production routes remain 404; no Run/Attempt, device registration, inventory authority, reservation, scheduling, remote transport, durable receipt, or Audit event is created. ADR-0039 remains planning-only, ADR-0113/0114 remain Proposed/null, and P3b/P4 remain gated.
- **DONE — Authenticated Flutter local Runner preview consumer (cross-device plan §306)**: Snaplink Console adds an explicit typed consumer for the same test-only candidate. It validates the owner/path-bound Runner intent and lease grant before one POST, disables bearer refresh/replay, strictly rechecks nested intent/session receipt/command identity, observation time, and all-false authority, and covers exact path/body/header plus no-replay behavior. The default Forge Gate/Sessions screen remains unconfigured; production routes stay 404 and no Run/Attempt, device, inventory, reservation, scheduling, dispatch, receipt, or Audit authority is added. ADR-0039 remains planning-only, ADR-0113/0114 remain Proposed/null, and P3b/P4 remain gated.
- **DONE — Production observation-candidate route closure (cross-device plan §307)**: Forge Core's production authenticated-session constructor now mounts only the owner-scoped Conversation API. Placement, session device observation, Runner receipt observation, and local Runner execution-readiness preview are composed only by explicitly named test/opt-in candidate constructors. A configured outer-route regression proves all four candidate paths remain exact 404 while focused candidate tests stay available. No candidate contract or device, inventory, selection, reservation, scheduling, dispatch, Runner, Run/Attempt, receipt, or Audit authority was added; ADR-0039 remains planning-only, ADR-0113/0114 remain Proposed/null, and P3b/P4 remain gated.
- **DONE — Cross-client local Runner preview over one authenticated candidate (cross-device plan §308)**: The opt-in Snaplink JWT test now sends one owner-bound local Runner preview through direct HTTP, Rust CLI, Rust TUI after owner-scoped session refresh, and Flutter typed API. Exact injected executor call counts, identity binding, metadata-only output, and all-false authority are checked across the same candidate mux and token; private output/fencing data remain absent. This is transport parity only; production routes remain 404 and no Run/Attempt, device, inventory, lease, reservation, dispatch, receipt, or Audit authority was added. ADR-0039 remains planning-only, ADR-0113/0114 remain Proposed/null, and P3b/P4 remain gated.
- **DONE — Cross-client execution-consent preview preflight (§309)**: The inert/test-only `/api/v1/conversations/{conversation_id}/execution-consents` candidate now returns strict project/profile/digest/TTL metadata bound to the verified owner and Conversation. Rust `remote execution-consent preview CONVERSATION_ID`, TUI `execution-consent-preview`, and Flutter `getExecutionConsentPreview` consume it with exact GET/no-body/no-write semantics and fail closed on response drift. Snaplink JWT cross-client coverage and production 404 regression are included; no consent, Run, device, lease, reservation, dispatch, Runner, receipt, or Audit behavior is enabled.
- **DONE — Joined identity-heartbeat-inventory lifecycle contract (§310)**: Forge Core composes the pure identity proof, heartbeat generation/sequence CAS, persisted inventory CAS, and fixed-time projection as the canonical `forge.device-enrollment-heartbeat-lifecycle/v1` value transition. Go, Forge Runtime domain tests, and Snaplink Console consume one fixture covering approval, replay, generation, server-clock, revision-conflict, stale, revoked, owner-drift, and restart cases; all authority fields remain false. The real `Run` server keeps enrollment, heartbeat, inventory, and client/device candidates exact 404. No cryptographic verification, challenge consumption, credential issuance, storage write, registration, reservation, scheduling, dispatch, Runner, execution, receipt, or Audit behavior was added; ADR-0039 remains planning-only and ADR-0113/0114 remain Proposed/null.
- **DONE — Composite lifecycle replacement image (§311)**: Forge Core now provides an authority-neutral `PersistedEnrollmentHeartbeatLifecycleState` with one outer revision joining immutable owner/device identity, heartbeat state, and inventory state. Restore rejects split revisions, owner/device drift, capability drift, and malformed values; commit derives nested revisions from the current image and rejects stale outer writers, while copying capability collections across the restart boundary. This is a persistence/restart seam only: no I/O, clock, cryptographic verification, challenge consumption, credential issuance, route registration, registration, reservation, scheduling, dispatch, Runner, execution, receipt, or Audit authority was added. ADR-0039 remains planning-only and ADR-0113/0114 remain Proposed/null.
- **DONE — Cross-language lifecycle persistence image contract (§312)**: The canonical `forge.device-enrollment-heartbeat-lifecycle-persistence/v1` fixture is consumed by Forge Runtime's domain contract alongside Forge Core's composite image seam. It enforces one outer revision matching both nested revisions and rejects split revisions, Runner/device or generation drift, owner drift, and zero revisions while preserving all-false authority and copied capability values. No storage, route, registration, inventory authority, reservation, scheduling, dispatch, Runner, execution, receipt, or Audit behavior was added; ADR-0039 remains planning-only and ADR-0113/0114 remain Proposed/null.
- **DONE — Joined Run–Attempt–lease–dispatch preflight (§313)**: Forge Core's pure `forge.run-attempt-lease-dispatch-preflight/v1` adapter binds caller-supplied Run status to the existing Attempt, Runner intent, fixed-time lease, and offline placement declarations with exact owner/Conversation/Run parity. Focused Go tests, Forge Runtime's Rust fixture consumer, and Snaplink Console's strict Flutter fixture model consume the canonical envelope; the shared Sessions Gate accepts an explicit local value and renders metadata on Web/App/Mobile while its default remains request-free. Accepted, terminal, expired-lease, identity-mismatch, immutability, and observation-validation cases are covered. `selected_target_id` remains null, all authority bits remain false, and fencing/argv/workspace/output/error fields are absent; no store, clock, lease issuance, reservation, scheduling, dispatch, Runner, receipt, Audit, or production route was added, and ADR-0039/0113/0114 remain gated.
- **DONE — Rust CLI/TUI Run–Attempt–lease preflight consumer (§314)**: Forge Runtime now strictly consumes `forge.run-attempt-lease-dispatch-preflight/v1` via `device run-attempt-lease-dispatch-preflight-preview --input FILE|-` and the path-only TUI command. The value model rejects unknown/duplicate fields, unsafe identifiers/numbers, inconsistent admissibility/readiness, unsorted rejection reasons, target selection, and any authority mutation; human output remains metadata-only with `selected_target_id=null`. Focused CLI/TUI tests consume the canonical fixture and assert no `/api/v1/devices` request. It is still offline caller-supplied evidence with no store/clock, lease, selection, reservation, scheduling, dispatch, Runner, receipt, Audit, or production route authority; ADR-0039/0113/0114 remain gated.
- **DONE — Authenticated cross-client Run–Attempt–lease preflight consumer (§315)**: The explicitly enabled Go JWT candidate binds owner, Conversation, Run, and nested intent on one strict POST while production remains exact 404. Rust remote CLI/TUI send one path-bound request without refresh/retry and validate metadata-only output; Snaplink Console exposes the typed one-shot API through an explicit request-free Gate/Screen seam. The canonical request fixture and opt-in JWT E2E cover direct HTTP, Rust CLI, Rust TUI, and Flutter parity. No store, lease issuance, target selection, reservation, scheduling, dispatch, Runner, execution, receipt, Audit, or production route authority was added; ADR-0039/0113/0114 and P3b/P4 remain gated.
- **DONE — Canonical preflight request/response closure (§316)**: Forge Core now strictly decodes the shared Run–Attempt–lease–dispatch request fixture, evaluates it through the real pure placement/Runner-intent/lease preflight adapter, validates the result, and requires byte-for-byte equality with the canonical response fixture. The request's fixed timestamps were aligned with the already shared response, closing a previously untested drift where both files passed independently while describing different observation times. Catalyst and Snaplink Console keep byte-identical request copies. This adds deterministic contract evidence only; production routes remain 404, selection and every authority bit remain false, and no inventory authority, lease issuance, reservation, scheduling, dispatch, Runner, execution, receipt, or Audit behavior was added. ADR-0039 remains planning-only and ADR-0113/0114 remain Proposed/null.
- **DONE — Flutter authenticated Run–Attempt–lease preflight E2E parity (§317)**: The opt-in Snaplink JWT preflight harness now launches the real Snaplink Console Flutter typed API against the same inert candidate mux and bearer token used by direct HTTP, Rust CLI, and Rust TUI. A private 0600 input document reconstructs the strict request; the process sends one POST and rechecks owner, Conversation/Run, Attempt, command, target, lease epoch, observation time, candidate count, null selection, preview mode, and all-false authority. The normal Console Gate remains request-free, production candidate routes remain 404, and no Run/Attempt store, lease issuance, target selection, reservation, scheduling, dispatch, Runner, execution, receipt, or Audit authority was added; ADR-0039/0113/0114 and P3b/P4 remain gated.
- **DONE — Audit Governance receiver compatibility for accepted Prompt evidence (§318)**: Audit Governance now strictly consumes the minimized `forge.prompt.accepted.v1` fixture, rejects unknown/duplicate fields and Prompt/content/token/credential/artifact leakage, and validates the value through its real `Event.ValidateBasic()` boundary. The contract runner exports the fixture, runs Forge source projection tests, compares the downstream mirror, and runs the focused receiver test. This remains compatibility evidence only: no Forge outbox/publisher/relay/source registration/delivery receipt or tenant credential flow was added, and no device enrollment, inventory authority, selection, reservation, scheduling, dispatch, Runner, execution, or Audit publication was enabled; ADR-0039/0113/0114 and P3b/P4 remain gated.
- **DONE — Cross-ecosystem accepted Prompt contract consumers (§319)**: The canonical minimized `forge.prompt.accepted.v1` envelope is now consumed by strict receiver tests in Aero-ID, Aero-IM, Aero-Vault, and Audit Governance. Each mirror rejects unknown/duplicate fields and Prompt or credential-bearing content; Aero-IM carries the value through its typed `AuditClaimPayload` while keeping publication disabled. The contract runner compares every mirror and runs focused suites. No Forge outbox, source registration, tenant credential binding, relay, delivery receipt, event publication, device enrollment, inventory authority, selection, reservation, scheduling, dispatch, Runner, or execution behavior was added; ADR-0039/0113/0114 and P3b/P4 remain gated.
- **DONE — Committed Prompt to audit projection causal binding (§320)**: Forge Core's pure `auditprojection.ProjectCommittedPrompt` requires the owner-scoped Prompt receipt and the Hub `prompt_appended` change to agree on Prompt ID, Conversation ID, user role, and committed timestamp before producing minimized `forge.prompt.accepted.v1` evidence. The authenticated HTTP-to-Rust integration matches every durable Prompt change to exactly one stored Prompt, asserts exact idempotency retries create no second change or event identity, and repeats the check after reopening the Hub through a new bridge. No audit table, outbox, publisher, network call, receipt, credential, device enrollment, inventory authority, selection, reservation, scheduling, dispatch, Runner, execution, or Audit publication was added; ADR-0039 remains planning-only and ADR-0113/0114 remain Proposed/null.
- **DONE — Authenticated Run observed projection candidate (cross-device plan §321)**: The explicitly enabled test-only `GET /api/v1/conversations/{conversation_id}/runs/{run_id}/observation` candidate derives owner from the verified JWT, reads existing owner-filtered Run summaries, and returns only minimized `forge.run.observed.v1` metadata with opaque owner linkage and all-false authority. Unknown/foreign Runs, query/body misuse, path binding, and production exact-404 closure are covered; Snaplink Console's explicit reader compares the transported value with the owner-scoped Run page. No outbox, publisher, device enrollment, inventory authority, selection, reservation, scheduling, dispatch, Runner, execution, receipt, or Audit authority was added; ADR-0039 remains planning-only and ADR-0113/0114 remain Proposed/null.
- **DONE — Atomic owner-bound Run observation read (cross-device plan §322)**: The authenticated Run observation candidate now calls one Rust Hub `owned_run_observation` operation rather than scanning Run pages. Strict RPC validation binds verified owner, Conversation, and Run IDs; SQLite performs owner and membership checks in one deferred read snapshot, uses a uniform not-found boundary, and projects only bounded `forge.run.observed.v1` metadata. The Go bridge validates the projection and the direct HTTP/Rust CLI/TUI/Flutter E2E remains cross-client identical. No production route, outbox, publisher, receipt, credential, device enrollment, inventory authority, selection, reservation, scheduling, dispatch, Runner, execution, or Audit authority was added; ADR-0039 remains planning-only and ADR-0113/0114 remain Proposed/null.
- **DONE — Rust CLI/TUI authenticated Run observation reader (§323)**: Forge Runtime's `remote runs observed` and TUI `run-observed` commands consume the owner-bound `forge.run.observed.v1` candidate through one authenticated GET, enforce exact Conversation/Run binding, metadata-only content, and all-false authority, and clear TUI state on auth failure. The opt-in JWT E2E compares direct HTTP, Rust CLI, Rust TUI, and Flutter. Production routes remain 404 and ADR-0039/0113/0114 plus P3b/P4 remain gated.
- **DONE — TUI selected-Run observation refresh (§324)**: TUI `sync` now refreshes the selected Run's owner/path-bound `forge.run.observed.v1` metadata after its incremental timeline read, renders only content-free metadata, and keeps it in process-local selected-session state. Switching sessions or 401/403 clears it with the owner-scoped view. No Run write, device inventory authority, target selection, reservation, scheduling, dispatch, Runner, execution, receipt, or Audit publication was added; production routes remain 404 and ADR-0039/0113/0114 plus P3b/P4 remain gated.

- **DONE — Opt-in TUI inventory refresh during sync (cross-device plan §325)**: After the user explicitly runs `inventory read-v2`, Forge Runtime TUI retains the validated owner-scoped v2 observation in process-local state and refreshes it during later `sync` calls through one authenticated GET. It renders the lossless revision/generation/heartbeat/reservation/GPU metadata, replaces state only after strict validation, leaves ordinary sync request-free for device routes, and clears the observation on 401/403 with the owner view. A failed refresh leaves the prior value and does not advance the conversation cursor. No enrollment, heartbeat ingestion, inventory authority, selection, reservation, scheduling, dispatch, Runner, execution, receipt, or Audit publication was added; production routes remain 404 and ADR-0039/0113/0114 remain gated.

### Cross-device plan §326 — Browser Run observation candidate transport parity

The opt-in Chromium Forge Web journey now performs one same-origin bearer GET
for the canonical `forge.run.observed.v1` candidate after receiving its
private projection. It compares the response with the Go projection, enforces
closed content-free metadata and exact Conversation/Run binding, and records
the candidate read beside the owner-scoped Run list and timeline requests.
The ordinary Forge Gate remains reader-free, production candidate routes stay
404, and no device enrollment, heartbeat, inventory authority, selection,
reservation, scheduling, dispatch, Runner, execution, receipt, or Audit
publication was added; ADR-0039 remains planning-only and ADR-0113/0114 remain
Proposed/null.

### Cross-device plan §327 — Explicit Flutter Gate Run observation reader

Snaplink Console's shared Web/App/Mobile `ForgeSessionsGate` now has a
default-off, explicitly enabled adapter for the authenticated Run observation
candidate. It builds the typed reader from the restored Forge credential and
an explicit candidate origin, then forwards it through the existing strict
`ForgeSessionsScreen` reader seam. Focused widget coverage proves one exact
owner/path-bound bearer GET and proves default construction makes no candidate
request; caller-supplied readers still take precedence. Production candidate
routes remain 404 and no device enrollment, inventory authority, selection,
reservation, scheduling, dispatch, Runner, execution, receipt, or Audit
publication was added; ADR-0039 remains planning-only and ADR-0113/0114
remain Proposed/null.

### Cross-device plan §328 — Flutter scheduled Run observation refresh

Snaplink Console's shared Web/App/Mobile Sessions screen now forces an
explicit `ForgeRunObservedReader` during each owner change-feed refresh of the
selected Run, including the scheduled poll path that bypasses the
manual/resume wrapper. Widget coverage advances the poll clock and proves a
second metadata read; transient failures retain the last validated value,
authorization failures clear owner state, and the default Gate remains
request-free. Production observation/device routes remain closed, with no Run
write, enrollment, inventory authority, selection, reservation, scheduling,
dispatch, Runner, execution, receipt, or Audit publication; ADR-0039 remains
planning-only and ADR-0113/0114 remain Proposed with null acceptance metadata.

### Cross-device plan §329 — Flutter scheduled v2 inventory refresh

The shared Web/App/Mobile Sessions screen now refreshes an explicitly
injected owner-bound v2 inventory reader during scheduled change-feed polling,
covering the path that bypasses the manual/resume wrapper. Widget coverage
advances the fake poll clock and proves a second metadata-only inventory read;
transient failures retain the last validated snapshot, authorization failures
clear owner state, and the default Gate remains request-free. No enrollment,
heartbeat ingestion, inventory authority, target selection, reservation,
scheduling, dispatch, Runner, execution, receipt, or Audit publication was
added; production device routes remain closed under ADR-0039 and Proposed
ADR-0113/0114.

### Cross-device plan §330 — Flutter scheduled client-instance view refresh

The shared Web/App/Mobile Sessions screen now refreshes explicitly injected
owner-bound client-instance session and resource readers during scheduled
change-feed polling. The timer path keeps instance/session metadata and
unverified resource summaries aligned with the owner feed without requiring a
manual or foreground refresh. Focused widget coverage advances the fake poll
clock and proves a second resource-view read; strict owner/display-only
validation, stale-value retention, authorization cleanup, and the request-free
default remain intact. No client registration, session binding mutation,
device enrollment, inventory authority, target selection, reservation,
scheduling, dispatch, Runner, execution, receipt, or Audit publication was
added; production candidate routes remain closed under ADR-0039 and Proposed
ADR-0113/0114.

### Cross-device plan §331 — Flutter scheduled pending Run-intent refresh

The shared Web/App/Mobile Sessions screen now refreshes an explicitly injected
owner-scoped pending Run-intent metadata reader during scheduled change-feed
polling when the feed has no new changes. Change-bearing syncs already refresh
the metadata and therefore avoid a duplicate read; the default Gate remains
request-free. Focused widget coverage advances the fake poll clock and proves
a second metadata read. No Prompt body, Run/Attempt write, device enrollment,
inventory authority, target selection, reservation, scheduling, dispatch,
Runner, execution, receipt, or Audit publication was added; production
candidate routes remain closed under ADR-0039 and Proposed ADR-0113/0114.

### Cross-device plan §332 — Flutter Runner dispatch-plan preview consumer

Snaplink Console now strictly consumes the canonical
`forge.runner-dispatch-plan-preview/v1` observation through a typed,
display-only model and card. It validates owner/identity/digest/timestamp
bounds, Attempt admissibility, sorted unique candidates, readiness equations,
`selected_target_id=null`, and all-false authority; duplicate-key scanning is
object-scope aware so repeated candidate fields across array elements remain
valid. Contract/widget tests cover the canonical fixture and authority,
selection, candidate, unknown-field, and duplicate-key mutations. The value
is not wired into the default Sessions Gate or a production route; no
inventory authority, target selection, reservation, scheduling, lease
issuance, dispatch, Runner, execution, receipt, or Audit publication was
added, and ADR-0039/0113/0114 remain gated.

### Cross-device plan §333 — TUI scheduled pending Run-intent refresh

Forge Runtime TUI now records the owner/Conversation and page-cursor binding
after the user explicitly opens `run-intents`; later `sync` refreshes only that
metadata page, while ordinary sync remains request-free when it was not
opened. Refresh failure preserves the prior page and change cursor; 401/403
and Conversation changes clear the bound state. Focused TUI tests cover
refresh, cursor binding, failure preservation, and authorization cleanup.
This remains metadata-only and process-local with no Prompt body, Run/Attempt
write, device enrollment, inventory authority, target selection, reservation,
scheduling, dispatch, Runner, execution, receipt, or Audit publication;
production candidate routes remain closed under ADR-0039 and Proposed
ADR-0113/0114.

### Cross-device plan §334 — Explicit Flutter Gate v2 inventory candidate adapter

The shared Web/App/Mobile `ForgeSessionsGate` now has a default-off,
explicitly enabled adapter for the authenticated v2 inventory observation
candidate. It uses the restored Forge credential, an explicit origin, and a
caller-supplied owner before forwarding the strict typed reader;
caller-supplied readers retain precedence. Widget coverage proves one exact
bearer GET and default construction remains request-free. This is unverified
display-only inventory; no enrollment, heartbeat ingestion, inventory
authority, target selection, reservation, scheduling, dispatch, Runner,
execution, receipt, or Audit publication was added, and production device
routes remain closed under ADR-0039 and Proposed ADR-0113/0114.

### Cross-device plan §335 — TUI scheduled client-instance view refresh

Forge Runtime TUI now retains owner-bound client-instance session/resource
observations only after the user explicitly opens the candidate; later `sync`
refreshes those exact authenticated GETs, preserves prior observations/cursor
on failure, and clears on 401/403 or owner/session changes. Ordinary sync
remains request-free when neither view is open. Focused tests cover both views,
binding, failure preservation, and cleanup; no registration, session mutation,
device authority, selection, reservation, scheduling, dispatch, Runner,
execution, receipt, or Audit publication was added.

### Cross-device plan §336 — Chromium Web v2 inventory candidate parity

The opt-in Chromium journey now performs one owner-bound bearer GET for
`forge.device-inventory-observation/v2`, compares it with the canonical
projection, rejects open fields/authority bits, and verifies token retention.
The real Flutter Web build, Google Chrome, Snaplink JWT issuer, and Forge
candidate mux pass together; the ordinary Web Gate remains request-free and
production device routes remain 404.

### Cross-device plan §337 — Chromium client-instance session/resource candidate parity

The opt-in Chromium journey now performs one authenticated bearer GET for each
owner-bound `forge.client-instance-session-view/v1` and
`forge.client-instance-resource-view/v1` candidate, compares both with their
canonical projections, requires display-only/all-false authority, retains the
browser credential, and rejects device/effect paths. The shared entry enables
it only when `FORGE_BROWSER_E2E=1`; `FORGE_CLIENT_INSTANCE_BROWSER_E2E=0` can
disable this candidate while leaving other browser checks enabled. The real
Flutter Web build, Google Chrome, Snaplink JWT issuer, and Forge mux pass
together; the ordinary Web Gate remains request-free and production candidate
routes remain 404. No client registration, session mutation, inventory
authority, target selection, reservation, scheduling, dispatch, Runner,
execution, receipt, or Audit publication was added; ADR-0039/0113/0114 remain
gated.

### Cross-device plan §338 — Flutter Gate client-instance session/resource candidate adapters

Snaplink Console's shared Web/App/Mobile `ForgeSessionsGate` now provides two
separately default-off adapters for the authenticated owner-bound
`forge.client-instance-session-view/v1` and
`forge.client-instance-resource-view/v1` candidates. Each requires an explicit
owner, uses the restored Forge credential and optional candidate origin,
enforces the configured owner before issuing the typed bearer GET, and
lets a caller-supplied reader take precedence. Focused Gate tests prove one
bearer GET per candidate, both panels, and no candidate request when explicit
readers are supplied. The normal Gate remains request-free; no client
registration, session mutation, inventory authority, target selection,
reservation, scheduling, dispatch, Runner, execution, receipt, or Audit
publication was added, and production candidate routes remain closed under
ADR-0039/0113/0114.

### Cross-device plan §339 — Flutter typed client-instance session/resource E2E parity

The shared authenticated E2E entry now enables the existing Flutter typed API
readers for both owner-bound client-instance session and resource candidates.
Each test crosses the real Snaplink JWT issuer, candidate mux, and
`ForgeConversationsApi`, validates the canonical display-only projection, and
preserves the production constructor's exact 404 closure; both toggles remain
independently disableable. Shared CLI/TUI/Flutter integration passes with the
cases enabled. No client registration, session mutation, device enrollment,
heartbeat ingestion, inventory authority, target selection, reservation,
scheduling, dispatch, Runner, execution, receipt, or Audit publication was
added; ADR-0039 remains planning-only and ADR-0113/0114 remain Proposed/null.

### Cross-device plan §340 — Flutter Gate scheduled client-instance candidate refresh

The default-off `ForgeSessionsGate` candidate adapters now have a focused
scheduled change-feed journey. With both explicit owner-bound readers enabled,
the real shared Sessions screen performs a second authenticated session-view
and resource-view GET after the initial load, retains the restored bearer
credential, and keeps both metadata panels visible. This proves the
adapter-to-scheduled-screen boundary only; no client registration, session
mutation, device enrollment, heartbeat ingestion, inventory authority, target
selection, reservation, scheduling, dispatch, Runner, execution, receipt, or
Audit publication was added, and production candidate routes remain closed
under ADR-0039/0113/0114.

### Cross-device plan §341 — Cross-ecosystem client-instance observation contract parity

The canonical `forge.client-instance-session-view/v1` and
`forge.client-instance-resource-view/v1` fixtures are now mirrored and
strictly consumed by Aero-ID, Aero-IM's audit connector, Aero-Vault's
governance relay, and Snaplink Audit Governance. Receiver tests reject
duplicate/unknown fields, foreign resource-owner declarations, and authority
mutations while retaining the CLI/TUI/Web/App/Mobile rows and all-false display
boundary. `scripts/test-forge-contracts.sh` compares all four copies and runs
the focused suites. This is read-only interoperability evidence; no instance
authentication, event publication, device enrollment, inventory authority,
selection, reservation, scheduling, dispatch, Runner, execution, receipt, or
Audit authority is enabled. ADR-0039 remains planning-only and ADR-0113/0114
remain Proposed/null.

### Cross-device plan §342 — Flutter Gate authenticated Run–Attempt–lease preflight candidate adapter

The default-off Gate can now connect a caller-supplied typed Run–Attempt–lease
preflight request to the existing strict authenticated candidate POST after
restoring the Forge bearer token. It fixes the owner/Conversation/Run binding
to the Gate declaration, leaves an injected reader authoritative, and drives
the selected Run screen through one metadata-only preview request; enabling the
flag without a typed request remains request-free. No Run/Attempt store, lease
issuance, target selection, reservation, scheduling, dispatch, Runner,
execution, receipt, or Audit publication was added; production candidate
routes remain closed under ADR-0039/0113/0114.

### Cross-device plan §343 — Chromium Web Run–Attempt–lease preflight candidate parity

The opt-in Chromium journey now loads the real Flutter Web Forge route with a
Snaplink JWT and posts one caller-supplied Run–Attempt–lease preflight request
to the inert candidate mux. It compares the canonical metadata-only response,
checks Conversation/Run binding, and rejects target selection and authority
bits. The ordinary Web Gate remains request-free and production candidate
routes remain closed under ADR-0039/0113/0114. No Run/Attempt store, lease
issuance, device enrollment, inventory authority, reservation, scheduling,
dispatch, Runner, execution, receipt, or Audit publication was added.

### Cross-device plan §344 — Cross-ecosystem Run–Attempt–lease preflight observation parity

The canonical `forge.run-attempt-lease-dispatch-preflight/v1` observation is now
mirrored and strictly consumed by Aero-ID, Aero-IM's audit connector, Aero-Vault's
governance relay, and Snaplink Audit Governance. Receiver tests reject
unknown/duplicate fields, selected targets, and every non-zero authority bit;
the contract runner compares all four copies byte-for-byte and runs focused
suites. This remains caller-supplied metadata-only interoperability evidence:
no Run/Attempt persistence, lease issuance, target selection/reservation,
scheduling, dispatch, Runner, execution, receipt, or Audit publication was
added; production routes remain closed under ADR-0039 and Proposed ADR-0113/0114.

### Cross-device plan §346 — Cross-ecosystem P3b lifecycle persistence image parity

The canonical `forge.device-enrollment-heartbeat-lifecycle-persistence/v1`
fixture is now mirrored and strictly consumed by Aero-ID, Aero-IM's audit
connector, Aero-Vault's governance relay, and Snaplink Audit Governance. Each
receiver preserves the complete owner/device binding, capability declaration,
heartbeat image, inventory image, and one outer revision with matching nested
revisions; unknown/duplicate fields, forged authority, split revisions, and
binding drift fail closed.

This is P3b preparation and interoperability evidence only. No device
authentication, challenge consumption, credential issuance, registration,
heartbeat/inventory persistence, reservation, scheduling, dispatch, Runner,
execution, or Audit authority was added. Production enrollment, heartbeat, and
inventory routes remain closed under ADR-0039 and Proposed ADR-0114; ADR-0113
and ADR-0114 retain their separate acceptance gates.
### Cross-device plan §347 — Cross-ecosystem Runner capability lease/fencing evidence parity

The canonical `forge-runner-lease-fencing/v1` fixture is now mirrored and
strictly consumed by Aero-ID, Aero-IM's audit connector, Aero-Vault's
governance relay, and Snaplink Audit Governance. Receiver tests cover all
sixteen active, renewal, proof, terminal, replay, conflict, and uncertain
cases and reject unknown/duplicate fields, authority mutation, and raw output
content. The contract runner compares all four copies byte-for-byte and runs
focused receiver suites.

This is capability-lease interoperability evidence only: it does not verify a
Runner identity, issue or persist a lease, reserve capacity, select a target,
dispatch a command, execute a process, persist a receipt, or publish Audit
evidence. Production routes remain closed under ADR-0039; ADR-0113/0114 remain
Proposed/null and P4 retains its separate execution/security gate.



### Cross-device plan §348 — Host-side Android/iOS shared-session lifecycle parity

The authenticated native cold-start harness now accepts explicit `android-host`
and `ios-host` markers. Forge Core runs both variants through the real
Snaplink JWT → Go → Rust path; each restores the credential through the
injected native secure-store abstraction across two cold starts, reads the
owner Conversation and Prompt history, appends and replays one idempotent
Prompt, and observes the owner change cursor.

The recorder proves both variants issue only Conversation, Prompt, and
change-feed calls. Device, Run-intent, placement, dispatch, and execution
paths remain untouched. This is host-side lifecycle evidence, not a physical
Android/iOS device claim; production device and execution routes remain closed
under ADR-0039 and Proposed ADR-0113/0114.

### Cross-device plan §349 — Host-side mobile owner cursor persistence across cold starts

The native host harness now seeds and restores the owner/coordinator/client/resource-bound
`ForgeChangeCursorStore` checkpoint alongside the native credential. Cold start one reads
and validates the restored cursor, persists the bounded empty-feed checkpoint before
appending an idempotent Prompt, and cold start two restores that cursor, reads exactly
the new owner-visible Prompt change, and advances the checkpoint after validation.
Android-host and ios-host variants cross the real Snaplink JWT → Forge Core Go → Forge
Runtime Rust boundary; recorder assertions reject device, Run-intent, placement,
dispatch, and execution requests. This is host-side cursor lifecycle evidence only,
not physical-device execution or global journal authority; production device,
scheduling, and execution routes remain closed under ADR-0039 and Proposed
ADR-0113/0114.

### Cross-device plan §350 — Cross-ecosystem device resource-summary receiver parity

The canonical `forge.device-resource-summary/v1` observation is now mirrored
and strictly consumed by Aero-ID, Aero-IM's audit connector, Aero-Vault's
governance relay, and Snaplink Audit Governance. Receivers enforce the bounded
owner binding, nested inventory/placement rows, deterministic ordering,
aggregate resource totals, null selected targets, and all-false authority;
unknown/duplicate fields, authority mutation, foreign device owners, and
selected targets fail closed. `scripts/test-forge-contracts.sh` compares the
four copies byte-for-byte and runs focused Go/Rust suites.

This remains offline, caller-declared resource perception evidence only. It
adds no device authentication/enrollment, heartbeat or inventory persistence,
production inventory route, target selection, reservation, scheduling,
dispatch, Runner, execution, receipt, or Audit publication. ADR-0039 remains
planning-only and ADR-0113/0114 remain Proposed with null acceptance metadata.

### Cross-device plan §351 — Cross-ecosystem persisted inventory observation receiver parity

The canonical `forge.device-inventory-persisted-observation/v1` image is now
mirrored and strictly consumed by Aero-ID, Aero-IM's audit connector,
Aero-Vault's governance relay, and Snaplink Audit Governance. Receivers retain
the owner-bound persisted device/Runner state and deterministic read-only
projection while rejecting unknown/duplicate fields, foreign owners, and
authority mutations. `scripts/test-forge-contracts.sh` compares all four
copies byte-for-byte and runs focused Go/Rust suites.

This is restored-value interoperability evidence only. It adds no device
registration, Runner authentication, heartbeat ingestion, authoritative
inventory, production inventory route, target selection, reservation,
scheduling, dispatch, execution, receipt, or Audit publication. ADR-0039
remains planning-only and ADR-0113/0114 remain Proposed/null.

### Cross-device plan §352 — Cross-ecosystem heartbeat persistence receiver parity

The canonical `forge-device-heartbeat-persistence-contract-v1` fixture is now
mirrored and strictly consumed by Aero-ID, Aero-IM's audit connector,
Aero-Vault's governance relay, and Snaplink Audit Governance. The receivers
validate the bounded owner/device binding, approval state, ten-case pure
compare-and-swap plan, revision chain, heartbeat generation/sequence,
server-clock monotonicity, lease expiry calculation, and stable rejection
errors. Unknown/duplicate fields, forged authority, foreign devices, and
invalid persisted revision state fail closed; `scripts/test-forge-contracts.sh`
compares all four copies byte-for-byte and runs the focused Go/Rust suites.

This is binding-only interoperability evidence. It adds no heartbeat
persistence, device authentication/enrollment, inventory authority, lease
issuance, target selection, reservation, scheduling, dispatch, Runner,
execution, receipt, or Audit publication. Production device and execution
routes remain closed under ADR-0039 and Proposed ADR-0113/0114.

### Cross-device plan §353 — Cross-ecosystem inventory persistence receiver parity

The canonical `forge.device-inventory-persistence/v1` image is now mirrored
and strictly consumed by Aero-ID, Aero-IM's audit connector, Aero-Vault's
governance relay, and Snaplink Audit Governance. Receivers validate the
owner/device/Runner binding, persisted revision, CAS replacement, projection
freshness and status, pending/cordoned/offline/revoked cases, mismatch errors,
and revision overflow. Unknown/duplicate fields, forged authority, foreign
owners or Runner devices, and invalid persisted revisions fail closed;
`scripts/test-forge-contracts.sh` compares all four copies byte-for-byte and
runs the focused Go/Rust suites.

This is persisted-value interoperability evidence only. It adds no inventory
write, heartbeat ingestion, device authentication/enrollment, authoritative
inventory, lease issuance, target selection, reservation, scheduling,
dispatch, Runner, execution, receipt, or Audit publication. ADR-0039 remains
planning-only and ADR-0113/0114 remain Proposed/null.

### Cross-device plan §354 — Cross-ecosystem inventory status projection receiver parity

The canonical `forge-device-inventory-status-contract/v1` image is mirrored
and strictly consumed by Aero-ID, Aero-IM's audit connector, Aero-Vault's
governance relay, and Snaplink Audit Governance. Receivers recompute fixed-time
status precedence, freshness and lease boundaries, and the display-only
`declared_eligible` value; future snapshots, invalid lease windows, unknown
liveness, unknown/duplicate fields, and authority mutations fail closed.
`scripts/test-forge-contracts.sh` compares all four copies byte-for-byte and
runs focused Go/Rust suites.

This remains a pure projection contract. `declared_eligible` is not target
selection, reservation, scheduling, lease issuance, dispatch, execution, or
permission. No inventory route, storage write, heartbeat listener, device
authentication/enrollment, Runner, receipt, or Audit publication was added;
ADR-0039 remains planning-only and ADR-0113/0114 remain Proposed/null.

### Cross-device plan §355 — Cross-ecosystem inventory snapshot canonical receiver parity

The canonical `forge-device-inventory-snapshot-canonical/v1` image is mirrored
and strictly consumed by Aero-ID, Aero-IM's audit connector, Aero-Vault's
governance relay, and Snaplink Audit Governance. Receivers validate the
caller-declared owner tuple, copy and sort rows by `(device_id, instance_id)`
without mutating input, reject foreign owners and duplicate composite rows,
and recompute the domain-separated digest. Empty/ordered snapshots, invalid
IDs, zero observation time, unknown/duplicate fields, six authority mutations,
and digest drift fail closed; `scripts/test-forge-contracts.sh` compares all
four copies byte-for-byte and runs focused Go/Rust suites.

Owner and inventory declarations remain unverified. The digest is an integrity
label only, not identity proof, authentication, freshness, reservation, target
selection, scheduling, dispatch, execution, or Audit permission. No route,
clock, storage write, registration, heartbeat listener, discovery, Runner,
receipt, or Audit publication was added; ADR-0039 remains planning-only and
ADR-0113/0114 remain Proposed/null.

### Cross-device plan §356 — Cross-ecosystem inventory observation v1 receiver parity

The canonical `forge-device-inventory-observation/v1` image is mirrored and
strictly consumed by Aero-ID, Aero-IM's audit connector, Aero-Vault's
governance relay, and Snaplink Audit Governance. Receivers validate the fixed
owner/evaluation time, complete resource and liveness/lease declarations,
deterministic device ordering, and reject foreign or duplicate device/instance
rows, invalid bounds, unknown/duplicate fields, unverified flag changes, and
authority mutations. `scripts/test-forge-contracts.sh` compares all four copies
byte-for-byte and runs focused Go/Rust suites.

This remains `offline_static_only` caller-supplied observation. Owner/resource
values remain unverified; no selected target or schedulable claim is produced.
No route, storage write, registration, heartbeat listener, discovery,
reservation, scheduler, dispatch, Runner, receipt, or Audit publication was
added; ADR-0039 remains planning-only and ADR-0113/0114 remain Proposed/null.

### Cross-device plan §357 — Lossless inventory observation v2 contract documentation

The fixture-only `forge.device-inventory-observation/v2` value now has an
independent contract description at
`docs/contracts/forge-device-inventory-observation-v2.md`. It fixes the
offline caller-supplied boundary for revision, Runner generation and heartbeat
sequence, declared reservation state, multi-GPU rows, resource and lease
bounds, owner equality, deterministic ordering, strict fields, and all-false
authority. A `reserved` declaration remains unverified metadata.

This documentation slice adds no v2 receiver, inventory write, heartbeat
ingestion, enrollment, authentication, production route, target selection,
reservation, scheduler, dispatch, Runner, execution, receipt, or Audit
publication. A later receiver parity slice must remain offline value
consumption; ADR-0039 remains planning-only and ADR-0113/0114 remain
Proposed/null.

### Cross-device plan §358 — Cross-ecosystem inventory observation v2 receiver parity

The canonical lossless `forge.device-inventory-observation/v2` image is
mirrored and strictly consumed by Aero-ID, Aero-IM's audit connector,
Aero-Vault's governance relay, and Snaplink Audit Governance. Receivers retain
unverified revision, Runner generation/heartbeat, reservation, resource/lease,
canonical runtime, and sorted multi-GPU declarations while rejecting owner
drift, duplicate/unsorted rows, unsafe counters, invalid leases/tags/GPU
bounds, unknown/duplicate fields, selected/schedulable claims, flag changes,
and authority mutations. The contract runner compares all four copies
byte-for-byte and runs focused Go/Rust suites.

This remains strict offline value consumption; `reserved` and heartbeat
metadata do not mean live reservation, freshness, authentication, or
execution. No route, storage write, heartbeat listener, enrollment,
authentication, target selection, reservation, scheduler, dispatch, Runner,
execution, receipt, or Audit publication was added. ADR-0039 remains
planning-only and ADR-0113/0114 remain Proposed/null.

### Cross-device plan §359 — iOS host shared-session credential rotation boundary

The explicit `ios-host` (and Android parity) Flutter host lifecycle now
receives a second valid JWT, replaces the persisted credential tuple between
cold starts, clears the in-memory slot, and requires the second client to
restore the rotated token before replaying the same idempotent Prompt and
observing the owner change feed. Forge Core's real Snaplink JWT → Go → Rust
path accepts both tokens, while request assertions continue to reject device,
inventory, placement, reservation, dispatch, and execution calls.

This is injected-backend host evidence only; it does not claim a physical
iPhone, iOS Keychain, simulator, or native device execution. ADR-0039 remains
planning-only and ADR-0113/0114 remain Proposed/null.

### Cross-device plan §360 — Explicit Android emulator Coordinator journey

Snaplink Console now has a separately selected Android instrumentation class
for a real Coordinator journey. An owner-only (`0600` or stricter) private
input file is copied into the debug
app sandbox, so only its filename is passed to `am instrument`; the bearer is
restored through the real Android secure-storage plugin. The Flutter entrypoint
uses the authenticated `ForgeConversationsApi` against the caller-supplied
Forge Go→Rust service, records only Conversation/Prompt/change-feed paths,
persists the owner-local cursor, and reports bounded metadata. Recreating
`MainActivity` repeats the idempotent Prompt probe and requires the replayed
owner-bound result.

The ordinary instrumentation command remains a successful no-device skip; the
Coordinator mode requires an explicit disposable `emulator-N`, a reachable
Coordinator, and a valid private input file. An explicit failure fails closed.
This slice adds no production route, client registration, device
authentication, heartbeat/inventory authority, target selection, reservation,
scheduler, lease issuance, dispatch, Runner, execution, receipt, or Audit
publication. It is real Android-emulator evidence only when the opt-in command
completes; iOS evidence remains host-side and injected. ADR-0039 remains
planning-only and ADR-0113/0114 remain Proposed/null.

### Cross-device plan §361 — Android Coordinator input credential boundary

The opt-in Android Coordinator runner now rejects group- or world-readable
input files before copying them into the debug app sandbox. The documented
flow uses `umask 077` and `chmod 600`; only the filename is passed to
instrumentation, and the short-lived JWT is restored through Android secure
storage. No-device skip and explicit-emulator failure semantics are
unchanged. This is runner credential hygiene only; no production device,
inventory, selection, reservation, scheduling, dispatch, Runner, execution,
receipt, or Audit authority was added. ADR-0039 remains planning-only and
ADR-0113/0114 remain Proposed/null.

### Cross-device plan §362 — Explicit iOS XCTest shared-session acceptance boundary

Snaplink Console now has an opt-in `RunnerTests` boundary for a future macOS
iOS Simulator/device run. A Linux-safe validator requires an owner-only JSON
with exact platform/origin, original and rotated credentials, owner
Conversation/cursor, Prompt, and idempotency fields. Its no-follow descriptor
read and the XCTest direct-file reader reject symlinks, and Conversation and
idempotency identifiers are path-safe ASCII; the runner passes only the path to
`xcodebuild` and rejects public permissions, unsafe origins, duplicate tokens,
and unknown fields. The XCTest allowlist admits
only Conversation, Prompt, and change-feed paths and rejects device,
inventory, placement, reservation, dispatch, Run-intent, execution, receipt,
and heartbeat paths. Linux validates then skips clearly because Xcode is
unavailable; no physical iPhone, Keychain, simulator, enrollment,
scheduling, or execution evidence is claimed. ADR-0039 remains planning-only
and ADR-0113/0114 remain Proposed/null.

### Cross-device plan §363 — Mobile acceptance input strict decoding

The Android and iOS opt-in acceptance validators now reject duplicate JSON
keys and enforce the Forge JSON-safe integer ceiling. The contract script runs
their syntax checks and default no-device/opt-in skips. `expected_version` is
positive, the owner cursor is non-negative, and both are bounded to
`9_007_199_254_740_991`, matching Flutter request preflight. This is disposable
mobile input validation only; no production route, client registration, device
or inventory authority, selection, reservation, scheduling, dispatch, Runner,
execution, receipt, or Audit behavior changed. ADR-0039 remains planning-only
and ADR-0113/0114 remain Proposed/null.

### Cross-device plan §364 — Device Fabric activation gate

Forge Core now has a pure `forge.device-fabric-activation-gate/v1` policy
adapter. The default/zero-value request keeps the fabric `OFF`; explicit
`INVENTORY`/`OBSERVE` requires accepted non-planning ADR-0039 plus accepted
ADR-0113/0114 and evidence for owner isolation, device proof, approval and
revocation, heartbeat CAS/freshness, owner-scoped reads, route closure, and
security review. `EXECUTE` adds a separate P4 decision and Runner
isolation, fencing, uncertain-effect, Vault, and Audit evidence; migration and
federation stay blocked by separate decision codes. `Config.Validate` rejects
an explicit incomplete activation while normal configuration and production
404 route closure remain unchanged. ADR-0039 remains planning-only and
ADR-0113/0114 remain Proposed/null.

### Cross-device plan §365 — Device Fabric activation review packet

Forge Core now validates the review-only `forge.device-fabric-activation-request/v1`
packet with canonical JSON, duplicate/unknown-field rejection, bounded
evidence references, explicit ADR lifecycle metadata, and all-false authority
markers. A proposed repository fixture reflects the current planning-only and
Proposed ADR state; a separate synthetic accepted fixture exercises the pure
positive evaluator without production authorization. `forge-server` accepts an
owner-private activation manifest only as a startup gate; no manifest mounts a
device route, and an absent manifest keeps the Fabric OFF. The threat model and
JSON Schema document the evidence needed before future INVENTORY/OBSERVE
activation. No enrollment, heartbeat listener, inventory authority, selection,
reservation, scheduling, dispatch, Runner, execution, migration, federation,
or Audit behavior is enabled.

### Cross-device plan §366 — Flutter local client-instance session scope filter

The shared Web/App/Mobile Forge Sessions screen now offers a process-local,
display-only scope picker when a caller supplies the client-instance
session/resource observation. It filters the authenticated owner Conversation
page by the selected instance's opaque `session_ids`, and clears/reselects the
local Prompt panel when the current session is outside that projection. The
screen labels the value unverified and does not change API requests, Prompt
writes, idempotency, ownership, or any device/execution authority. Focused
session/resource/candidate widget tests and the broader Sessions widget suite
pass. ADR-0039 remains planning-only and ADR-0113/0114 remain Proposed/null.

### Cross-device plan §367 — Heartbeat transition boundary hardening

Forge Core's pure `deviceheartbeat.Apply` now validates bounded non-empty
device/Runner identifiers, known `pending`/`approved`/`revoked` approval
states, and nonzero generation/sequence values before capability or incarnation
evaluation. It returns stable invalid-device, invalid-instance, and
unknown-approval errors, and `Commit` inherits the same guard through `Apply`.
Focused Go boundary tests cover malformed values with invalid capabilities and
prior-observation paths. This is pure value validation only: no heartbeat
listener, device authentication, enrollment, persistence, inventory authority,
selection, reservation, scheduling, dispatch, Runner, receipt, Audit, or
production route was enabled. ADR-0039 remains planning-only and
ADR-0113/0114 remain Proposed/null; device surfaces stay default-off/404.

### Cross-device plan §368 — Pure owner approval, terminal revocation, and key rotation state machine

Forge Core now provides `forge.device-approval-rotation/v1`, a pure
owner/device enrollment lifecycle value state machine. It validates exact
owner/device binding and the existing `pending`/`approved`/`revoked` vocabulary;
pending can be approved or revoked, approved can be revoked, and revoked is
terminal. Rotation is allowed only before revocation, preserves device ID,
owner, and approval state, replaces key material, and increments key generation
inside the transition rather than accepting a caller generation.

Unknown states/actions, owner/device drift, malformed or unchanged key
material, repeated approval, terminal operations, and generation overflow fail
closed. The result is explicitly preview-only with owner authentication,
credential issuance, persistence, authoritative inventory, and execution
authorization fixed false. The canonical fixture/schema/documentation and
focused contract-script test cover the value boundary. No listener, credential
store, route, inventory write, selection, reservation, scheduling, dispatch,
Runner, execution, receipt, or Audit behavior was added; ADR-0039 remains
planning-only and ADR-0114 remains Proposed/null.

### Cross-device plan §369 — CLI/TUI local client-instance session scope

Forge Runtime CLI `remote sessions list --instance INSTANCE_ID` and the TUI
`instance INSTANCE_ID` command now apply a process-local projection over a
strict caller-declared client-instance session/resource view. The projection
filters owner conversations by opaque `session_ids`, preserves the original
authenticated request shape, and clears/reselects local Prompt/Run state when
the selected session becomes invisible. Path-like/duplicate IDs fail closed;
focused Rust CLI/TUI tests pass. No registration, enrollment, heartbeat,
inventory, placement, reservation, scheduling, dispatch, Runner, execution,
receipt, or Audit authority was added; ADR-0039 remains planning-only and
ADR-0113/0114 remain Proposed/null.

### Cross-device plan §370 — Cross-ecosystem device approval and key-rotation receiver parity

The canonical `forge.device-approval-rotation/v1` pure lifecycle fixture is
mirrored and strictly consumed by Aero-ID, Aero-IM's audit connector,
Aero-Vault's governance relay, and Snaplink Audit Governance. Receiver tests
require the pending initial value, preserve exact owner/device identity for
accepted replacement cases, keep all authority bits false, and reject unknown
or duplicate JSON fields and authority mutations. `scripts/test-forge-contracts.sh`
compares all four mirrors and runs the focused Go/Rust suites. This is offline
preview interoperability only: it adds no authentication, credential
issuance, persistence, route, inventory, selection, reservation, scheduling,
dispatch, Runner, execution, receipt, or Audit authority; ADR-0039 remains
planning-only and ADR-0113/0114 remain Proposed/null.

### Cross-device plan §371 — Authenticated Coordinator read-page transport hardening

Forge Core's authenticated Coordinator now revalidates owner-bound Prompt
pages, Run summaries, Run timelines, and content-free Run observations at the
final HTTP boundary before serialization. It binds pages/observations to the
path Conversation/Run and verified owner, enforces bounded content and
JSON-safe integers, requires dense/newest-first cursor order, and rejects
malformed IDs, non-metadata Run events, or authority-claiming observations.
Focused injected-backend HTTP tests cover foreign Conversation IDs, unsafe
numbers, cursor drift, an unapproved event type, and an unsafe observation.
This adds no execution, device, inventory, scheduling, dispatch, receipt, or
Audit authority; production device routes remain default-off/404 and
ADR-0039/0113/0114 remain gated.

### Cross-device plan §372 — Owner-scoped lifecycle registry restart image

Forge Core now defines the read-only
`forge.device-enrollment-heartbeat-lifecycle-file-set/v1` restart boundary for
a future Go-owned registry. A private `0600` file restores up to 128 complete
identity, heartbeat, and inventory images under one exact owner tuple. The
adapter verifies private regular-file boundaries, strict JSON, nested lifecycle
state, owner equality, duplicate device/Runner rejection, and deterministic
ordering. Focused Go tests cover restart restoration, owner drift, duplicate
members, malformed images, aliases, permissions, and missing state.

This is migration-ready registry aggregation only. It adds no write method,
proof verification, challenge consumption, credential issuance, enrollment or
heartbeat listener, inventory authority, HTTP route, placement, reservation,
scheduling, dispatch, Runner, execution, receipt, or Audit behavior. ADR-0039
remains planning-only and ADR-0114 remains Proposed/null; device surfaces stay
default-off/404.

### Cross-device plan §373 — Restart-boundary execution reconciliation observation

Forge Core and Forge Runtime now share the pure
`forge.execution-reconciliation-observation/v1` value contract. It validates
caller-supplied Run/Attempt/lease/terminal bindings at an explicit observation
time and classifies `await_terminal`, expired/no-terminal, terminal
completion/failure/uncertainty, and state-conflict cases. Foreign/expired
proofs, future receipts, unsafe identifiers/timestamps, and binding drift fail
closed; uncertain, stale, and conflicting evidence requires manual review and
automatic retry is fixed false. Canonical fixture, schema, Go/Rust tests, and
contract-script wiring are included. This remains a read-only observation:
all authority bits are false and no store, clock, registry, route, selection,
reservation, scheduler, dispatch, Runner, receipt persistence, or Audit path is
opened. ADR-0039 remains planning-only and ADR-0113/0114 remain Proposed/null.

### Cross-device plan §374 — Cross-ecosystem execution-reconciliation receiver parity

The canonical `forge.execution-reconciliation-observation/v1` fixture is now
mirrored byte-for-byte in Aero-ID, Aero-IM's audit connector, Aero-Vault's
governance relay, and Snaplink Audit Governance. Each receiver remains
read-only: it validates owner/Run/Attempt/lease bindings and terminal proof
alignment, preserves expired/uncertain/conflict cases as manual-reconciliation
signals, keeps all authority markers false, and rejects unknown or duplicate
JSON, authority mutations, foreign proofs, and automatic-retry fields.
`scripts/test-forge-contracts.sh` compares all four copies and runs the focused
Go/Rust suites. No authentication, persistence, lease issuance, selection,
reservation, scheduling, dispatch, Runner, execution, receipt, Audit, or
production route was added; ADR-0039 remains planning-only and ADR-0113/0114
remain Proposed/null.

### Cross-device plan §375 — Authenticated execution-reconciliation preview boundary

Forge Core adds an opt-in-only authenticated
`execution-reconciliation/preview` candidate under an owner-bound
Conversation/Run path. It validates the caller-supplied Run/Attempt/lease/
terminal restart image against the exact path and authenticated owner, returns
metadata-only classification, and rejects missing/duplicate fields, foreign
owners, path drift, unsafe epochs, expired proofs, and future receipts;
`automatic_retry` and all authority bits remain false. The candidate is mounted
only by focused test constructors; production Coordinator remains 404/default-
off and no Run/device store, lease issuance/renewal, target selection,
reservation, scheduling, dispatch, Runner, receipt persistence, Audit, or
production route is enabled. ADR-0039 remains planning-only and ADR-0113/0114
remain Proposed/null.

### Cross-device plan §376 — CLI/TUI execution-reconciliation preview consumers

Forge Runtime now provides explicit CLI and TUI consumers for the authenticated
execution-reconciliation preview candidate. Both consume the bounded canonical
fixture shape, reject unknown/duplicate/missing fields, bind the Conversation
and Run path to the caller-supplied owner image, issue exactly one authenticated
POST, and require the response to equal the pure reconciliation projection.
The TUI additionally requires the selected owner Conversation and clears its
local session on `401`/`403`; human output is metadata-only and reports no
automatic retry or authority. No retry, lease renewal, target selection,
reservation, scheduling, dispatch, Runner, receipt persistence, Audit, or
production device route was added; production remains 404/default-off and
ADR-0039/0113/0114 remain gated.

### Cross-device plan §377 — Flutter Web/App/Mobile execution-reconciliation preview consumer

Snaplink Console adds an explicitly injected Web/App/Mobile consumer for the
authenticated `execution-reconciliation/preview` candidate. The strict
`ForgeExecutionReconciliationObservation` model re-decodes
`forge.execution-reconciliation-observation/v1`, rejects unknown/duplicate/
missing/unsafe or foreign/path-drifting values, and requires exact
Conversation/Run/Attempt/lease/terminal binding. The shared API sends one
owner-bound POST and accepts only the metadata-only projection with
`automatic_retry=false`, required manual-reconciliation state, and all-false
authority; the optional Gate/card remains display-only.

The default Sessions Gate remains request-free. The candidate adapter surfaces
401/403 without refreshing or replaying a bearer, and does not renew a lease,
select/reserve a target, schedule/dispatch work, contact a Runner, persist a
receipt, or publish Audit evidence. Production Coordinator construction stays
404/default-off; ADR-0039 remains planning-only and ADR-0113/0114 remain
Proposed/null. Focused Flutter contract/API/widget tests and
`bash scripts/test-forge-contracts.sh` are the verification boundary.

### Cross-device plan §378 — Owner-scoped per-device lifecycle registry CAS aggregation

Forge Core now computes a complete owner-scoped
enrollment/heartbeat/inventory registry replacement through a pure per-device
CAS function. Revision zero creates a device; a nonzero expected revision must
match that device's outer lifecycle revision, with no global registry revision
that can let one device update clobber another. The function clones and
validates the current image, applies the joined lifecycle transition while
preserving server-owned cordon/reservation state, validates the complete
replacement, and returns deterministic device/Runner ordering without
mutating caller state.

Owner mismatch, stale revision, invalid member, duplicate device/Runner,
capacity over 128, and revision overflow errors fail closed. The returned
registry/result are value images only: no I/O, lock, clock, proof or
credential authentication, challenge consumption, heartbeat listener,
inventory write, or authority transition occurs. No selection, reservation,
scheduling, dispatch, Runner, receipt, or Audit path was added; production
device routes remain 404/default-off. Focused
`go test ./internal/deviceinventory -run '^TestCommitPersistedEnrollmentHeartbeatLifecycleRegistry' -count=1`
passes; ADR-0039 remains planning-only and ADR-0114 remains Proposed/null.

### Cross-device plan §379 — Flutter explicit execution-reconciliation reader binding

Snaplink Console adds an explicit `ForgeExecutionReconciliationReader` and
typed `ForgeExecutionReconciliationInput` seam from the Sessions Gate to the
selected Conversation/Run. The opt-in candidate adapter revalidates owner,
Conversation, Run, Attempt, lease, and terminal bindings before one
authenticated preview POST, strictly re-decodes the response, and compares it
with the local pure projection before displaying the read-only card. The
default Gate remains request-free; enabling the candidate without a typed input
cannot issue a request. This is an observation seam only: no lifecycle
registry I/O, lease renewal, target selection/reservation, scheduling,
dispatch, Runner, receipt persistence, Audit publication, or production route
was added. Focused Flutter Gate/API/contract/widget suites pass; ADR-0039
remains planning-only and ADR-0113/0114 remain Proposed/null.

### Cross-device plan §380 — Candidate owner-scoped lifecycle registry file CAS boundary

Forge Core adds an explicitly injected private-file adapter for the complete
owner-scoped enrollment/heartbeat/inventory registry image. It strictly reads
the `forge.device-enrollment-heartbeat-lifecycle-file-set/v1` envelope, keeps
the exact bytes and `0600` mode as an opaque token, validates every replacement
member before encoding, sorts device/Runner members deterministically, and
publishes through `AtomicWriteTrackedIfUnchanged`. A missing leaf is a valid
revision-zero create expectation only under an existing private parent;
stale create/update tokens return a stable CAS conflict and preserve the
current image.

This remains a persistence candidate only: no proof or credential validation,
challenge consumption, clock, registration, approval/revocation transition,
heartbeat listener, inventory authority, HTTP route, scheduler, selection,
reservation, dispatch, Runner, receipt, or Audit behavior was added. Production
remains 404/default-off; ADR-0039 remains planning-only and ADR-0114 remains
Proposed/null. Focused deviceinventory Go tests and the contract script cover
create/restore, stale-token preservation, owner/duplicate rejection,
permissions, and filesystem drift.

### Cross-device plan §381 — Lifecycle registry to multi-instance inventory observation bridge

Forge Core adds an explicitly injected source that reads the complete
owner-scoped lifecycle registry file-set and projects its joined inventory
members through the existing authenticated device observation candidates. Both
v1 and lossless v2 readers are supported; v2 retains registry revision,
generation, and heartbeat sequence while every authority, reservation, and
dispatch marker remains false. The source checks the verified owner and
context before every read and reloads the image for each observation, so a
later candidate CAS replacement is visible without exposing file-owned slices.

This remains a candidate read bridge only. No enrollment, heartbeat listener,
inventory authority, production route, target selection, reservation,
scheduling, dispatch, Runner, receipt, or Audit behavior was added. Production
remains 404/default-off; ADR-0039 remains planning-only and ADR-0114 remains
Proposed/null. Focused Go tests cover multi-instance v1/v2 projection,
owner-binding, cancellation, and all-false authority.

### Cross-device plan §382 — Authenticated candidate lifecycle registry read/replace CAS boundary

Forge Core adds an explicitly injected candidate HTTP boundary at
`/api/v1/device-enrollment-heartbeat/lifecycle-registry`. `GET` and `PUT` use
separate lifecycle scopes; the owner is always taken from the verified bearer
principal, `GET` returns a canonical complete image, and `PUT` accepts only
`states` before re-reading and exact-image-CAS replacing the private file.
Stale images, owner drift, malformed members, duplicate identities, broad
permissions, and filesystem changes fail closed. The registry envelope is
canonicalized before it is returned, so an injected store cannot leak an
unsorted or aliased state slice.

The candidate is not mounted by production. It adds no proof verification,
credential issuance, heartbeat listener, authoritative inventory, selection,
reservation, scheduling, dispatch, Runner, receipt, or Audit behavior.
Production remains 404/default-off; ADR-0039 remains planning-only and
ADR-0114 remains Proposed/null. Focused Go tests cover default closure,
separate scopes, owner/query/method/content validation, CAS conflict mapping,
canonical response, and production-route closure.

### Cross-device plan §383 — Rust CLI/TUI lifecycle registry candidate reader

Forge Runtime adds the explicit read-only commands remote lifecycle-registry
show/read and TUI lifecycle-registry show. They send one authenticated GET
only when explicitly requested; TUI startup and sync never refresh this
registry. A closed DTO decoder rejects unknown/duplicate fields, wrong schema,
invalid owner tuples, unsorted members, revision drift, and nested
device/Runner/capability binding drift before display. Focused CLI/TUI tests
cover exact path, bearer header, empty-body GET, strict response rejection,
and explicit-trigger behavior. No PUT, enrollment, heartbeat acceptance,
inventory authority, selection, reservation, scheduling, dispatch, Runner,
receipt, Audit, or production route was added; ADR-0039 remains
planning-only and ADR-0114 remains Proposed/null.

### Cross-device plan §384 — Lifecycle registry source composition for inventory candidates

Forge Core adds a focused migration constructor that feeds one private
owner-scoped lifecycle-registry file source into both authenticated device
observation candidates (`/api/v1/devices` v1 and
`/api/v1/devices/observations/v2` lossless v2). The composition reuses one
validated registry image, preserves deterministic device/Runner ordering and
v2 revision/generation/heartbeat metadata, and keeps execution, reservation,
and dispatch markers false. Focused tests prove both projections and confirm
the production constructor remains 404. No enrollment, heartbeat acceptance,
inventory authority, selection, reservation, scheduling, dispatch, Runner,
receipt, or Audit behavior was added; ADR-0039 remains planning-only and
ADR-0114 remains Proposed/null.

### Cross-device plan §385 — Flutter lifecycle registry candidate contract and Gate boundary

Snaplink Console adds strict contract, authenticated API, display-only panel,
and Sessions Gate coverage for the candidate lifecycle-registry GET. Tests
reject unknown/duplicate fields, foreign owners, unsorted or duplicate
device/Runner identities, identity/revision/heartbeat/capability drift, and
authority mutations; API coverage proves one empty-body bearer GET at the exact
path and rejects origin drift before transport. The default Gate and a
candidate without explicit origin remain request-free. No PUT, enrollment,
heartbeat, inventory authority, selection, reservation, scheduling, dispatch,
Runner, receipt, Audit, or production route was added. Production remains
404/default-off; ADR-0039 remains planning-only and ADR-0114 remains
Proposed/null.

### Cross-device plan §386 — Candidate device heartbeat transaction and lifecycle-file CAS hardening

Forge Core adds an explicitly injected `/api/v1/device-enrollment-heartbeat/heartbeat`
candidate with a separate lifecycle-heartbeat scope, verified bearer owner,
injected server clock, strict device proof/challenge/Runner heartbeat input,
approval and generation/sequence validation, owner-scoped registry CAS, and
private-file publication. The file adapter serializes writers with a platform
lock and rejects lifecycle revision/generation/heartbeat/server-time rollback
and device deletion while preserving stale/concurrent CAS safety. Proof remains
a non-cryptographic value label, challenge consumption and credential issuance
remain absent, authority flags remain false, and production remains
404/default-off; ADR-0039 remains planning-only and ADR-0114 remains
Proposed/null.

### Cross-device plan §387 — Owner-scoped registry placement preview boundary

Forge Core adds the explicitly injected `POST /api/v1/device-placement/registry-preview`
candidate with `forge:devices:placement:preview` scope. The body contains only
strict placement requirements; verified owner, injected server clock, and the
owner-scoped lifecycle-registry v2 source provide the remaining inputs. The
response reuses the deterministic v2 placement evaluation, leaves selected
targets null, and keeps every authority marker false. Strict JSON, owner/source
validation, clock failure, scope, production 404 closure, and lifecycle-file
composition are covered by focused tests and the contract document. This adds
no selection, reservation, scheduling, dispatch, Runner, execution, receipt,
or Audit behavior; ADR-0039 remains planning-only and ADR-0114 remains
Proposed/null.

- **DONE — Registry placement preview cross-client transport closure (cross-device plan §388)**: Forge Runtime adds explicit CLI `remote placement registry-preview --input FILE|-` and TUI `placement-registry-preview --input FILE` consumers for the authenticated registry-backed candidate. Both issue one requirements-only POST, reject duplicate/unknown fields and compact v2 drift, and keep startup/sync request-free. Snaplink Console adds the same strict compact-response decoder, authenticated API, opt-in Sessions Gate reader, display-only panel, and explicit/manual/scheduled refresh semantics shared by Web/App/Mobile. Selected targets remain null and all authority flags remain false; production Coordinator construction stays 404/default-off, ADR-0039 remains planning-only, and ADR-0114 remains Proposed/null.
- **DONE — Flutter client-instance candidate authorization boundary (cross-device plan §389)**: The shared Web/App/Mobile client-instance session-view and resource-view candidate readers now pass `retryUnauthorized=false` to the authenticated transport. A caller-supplied token-refresh callback cannot replay an opt-in candidate GET after a 401. Focused transport coverage proves one bearer request and zero refresh calls for both paths, while existing Gate/session/resource suites preserve default request-free behavior and normal owner-session refresh behavior. No client registration, Prompt/Run mutation, device/inventory authority, target selection, reservation, scheduling, dispatch, Runner, receipt, Audit, or production route was added; ADR-0039 remains planning-only and ADR-0114 remains Proposed/null.

- **DONE — Registry placement preview cross-ecosystem receiver parity (cross-device plan §390)**: The canonical compact registry placement response fixture is mirrored byte-for-byte and strictly consumed by Aero-ID, Aero-IM's audit connector, Aero-Vault's governance relay, and Snaplink Audit Governance. Focused receivers enforce exact fields, owner/time/counter bounds, unique sorted candidates/reasons, null selected targets, matching eligible counts, and all-false authority while rejecting unknown, duplicate, trailing, selected, or authority-bearing mutations. The contract script compares all four mirrors and runs the focused Go/Rust suites. This remains read-only compatibility evidence; no device authentication, Audit publication, inventory authority, selection, reservation, scheduling, dispatch, Runner, execution, or production route was added; ADR-0039 remains planning-only and ADR-0114 remains Proposed/null.

- **DONE — Cross-ecosystem device identity proof receiver parity (cross-device plan §391)**: The canonical `forge.device-identity-proof-contract/v1` pure binding vector is mirrored byte-for-byte and strictly consumed by Aero-ID, Aero-IM's audit connector, Aero-Vault's governance relay, and Snaplink Audit Governance. Receivers enforce the owner/device/key/challenge envelope, twelve accepted/rejected binding cases, digest/time bounds, unique names, and all-false authority while rejecting unknown, duplicate, trailing, malformed, or authority-bearing mutations. This is offline identity interoperability evidence only; no cryptography, challenge consumption, credential issuance, enrollment persistence, approval write, heartbeat acceptance, inventory authority, selection, reservation, scheduling, dispatch, Runner execution, Audit publication, or production route was enabled. ADR-0039 remains planning-only and ADR-0114 remains Proposed/null.
- **DONE — Owner-scoped device approval and key lifecycle candidate (cross-device plan §392)**: Forge Core adds the explicitly injected `POST /api/v1/device-enrollment-heartbeat/approval-candidate` under `forge:devices:lifecycle:approval`. The verified bearer supplies the owner; the strict request supplies only a device ID, pure approve/revoke/rotate action, optional next-key values, and expected lifecycle revision. The handler applies `forge.device-approval-rotation/v1` and exact-image-CAS publishes only an optional `approval_candidate`; live DeviceBinding, heartbeat, inventory, credential, Runner, and registry revision remain unchanged. Responses are preview-only with `candidate_published=true` and all authority false; terminal revocation, generation/owner/device/revision/JSON/scope/CAS checks and production 404 closure are tested. No cryptographic proof, credential issuance, enrollment, heartbeat acceptance, inventory authority, selection, reservation, scheduling, dispatch, Runner execution, receipt, Audit, or production route was added; ADR-0039 remains planning-only and ADR-0114 remains Proposed/null.
- **DONE — Flutter explicit candidate scheduling-review receipt (cross-device plan §393)**: Snaplink Console exposes the existing inert pending Run-intent contract as an explicit Web/App/Mobile Sessions action only when the Gate receives the reviewed owner, candidate origin, and opt-in flag. The selected Conversation, Prompt bytes, aggregate version, and idempotency key are bound before one authenticated candidate POST; the result renders a metadata-only pending receipt with all execution authority false. Candidate 401 responses never replay through bearer refresh; the default Gate remains request-free and the action is absent. No Run, device selection, reservation, scheduler decision, dispatch, Runner, execution, Audit, or production route authority was added; ADR-0039 remains planning-only and ADR-0114 remains Proposed/null.
- **DONE — Pure Ed25519 device proof-of-possession verifier (cross-device plan §394)**: Forge Core adds `deviceidentity.VerifySignedProof`, a side-effect-free Ed25519 verifier with domain-separated deterministic signed bytes binding device/key/public-key digest, exact owner, challenge ID/digest, and proof validity window. It checks exact raw key/signature encodings, key digest against the already-bound device, then applies the existing owner/device/key/challenge/approval/credential/time/replay evaluator; malformed UTF-8 owner claims and binding or cryptographic drift fail closed. No challenge consumption, credential issuance or rotation, enrollment/approval persistence, heartbeat acceptance, inventory authority, selection, reservation, scheduling, dispatch, Runner, Run, receipt, Audit, or production route was added; the approval/lifecycle candidate remains injected-only, ADR-0039 remains planning-only, and ADR-0114 remains Proposed/null.
- **DONE — Flutter candidate resource and Run-status authorization boundary (cross-device plan §395)**: Snaplink Console's owner-scoped v1 inventory, lossless v2 inventory, and content-free Run-observation candidate GETs now pass `retryUnauthorized=false`, so a candidate `401` is never replayed with a rotated bearer. Sessions initial/manual refresh and stale snapshot retention remain unchanged, while scheduled change-feed polling now refreshes both the v1 and v2 inventory projections when explicitly enabled. Focused transport coverage proves one bearer request and zero refresh calls for all three paths, and the widget coverage proves scheduled v1 refresh. No registration, heartbeat, inventory authority, target selection, reservation, scheduling, dispatch, Runner execution, Run creation, receipt, Audit, or production route was added. ADR-0039 remains planning-only and ADR-0114 remains Proposed/null.
- **DONE — Cross-ecosystem Ed25519 device proof receiver parity (cross-device plan §396)**: The canonical `forge.device-identity-proof/ed25519/v1` signed proof vector is mirrored byte-for-byte and strictly consumed by Aero-ID, Aero-IM's audit connector, Aero-Vault's governance relay, and Snaplink Audit Governance. Receivers validate owner/device/key/challenge binding, raw base64url key/signature sizes, public-key digest, domain-separated deterministic payload, Ed25519 signature, validity windows, expected projection, and all-false authority; unknown/duplicate/trailing/signature/authority mutations fail closed. This remains cryptographic proof-input interoperability only: no challenge consumption, credential or enrollment/approval persistence, heartbeat acceptance, inventory authority, selection, reservation, scheduling, dispatch, Runner, Run, receipt, Audit, or production route was enabled. ADR-0039 remains planning-only and ADR-0114 remains Proposed/null.
- **DONE — Pure device-only credential lifecycle replacement plan (cross-device plan §397)**: Forge Core adds `devicecredential.Apply` for pure issue/revoke/rotate metadata plans. Exact owner/device/approval/credential/key/generation bindings and explicit one-second-to-one-hour validity windows are validated; revocation is terminal and rotation increments key generation. The value contains no bearer secret or token material, and only `owner_binding_matched` may be true; authentication, material creation, persistence, inventory, and execution authority remain false. The explicitly injected owner-scoped `POST /api/v1/device-enrollment-heartbeat/credential-candidate` applies the same plan through exact-image CAS into an optional private candidate and hides it from the live registry projection; focused tests cover strict owner/device/key/revision/JSON/scope checks, issue/rotate/revoke terminal behavior, persistence, and production 404 closure. No credential issuance, enrollment, heartbeat, inventory authority, placement, scheduling, dispatch, Runner, Run, receipt, Audit, or production route was enabled; ADR-0039 remains planning-only and ADR-0114 remains Proposed/null.
- **DONE — §398 cross-ecosystem device credential lifecycle receiver parity**: The canonical `forge.device-credential-lifecycle/v1` pure metadata fixture is mirrored byte-for-byte and strictly consumed by Aero-ID, Aero-IM's audit connector, Aero-Vault's governance relay, and Snaplink Audit Governance. Receivers validate all six issue/revoke/rotate cases, owner/device/key/generation bindings, credential state and validity windows, terminal rejection, and all-false authority; unknown/duplicate/trailing/secret-bearing/invalid-action/authority mutations fail closed. This adds no bearer material, caller authentication, persistence, heartbeat acceptance, inventory authority, selection, reservation, scheduling, dispatch, Runner, Run, receipt, Audit, or production route. ADR-0039 remains planning-only and ADR-0114 remains Proposed/null.
- **DONE — §399 cross-client credential lifecycle candidate decoder**: Forge Runtime CLI/TUI now expose only local `device credential-candidate-preview --input FILE|-` / `credential-candidate-preview --input FILE` readers for the strict `forge.device-credential-lifecycle/v1` response. Snaplink Console adds the same owner-bound metadata decoder and all-false authority checks. Unknown/duplicate/secret-bearing/owner/device/key/window/authority drift fails closed; no client issues the candidate POST or replays a Bearer, and no credential material, challenge, heartbeat, inventory, selection, reservation, scheduling, dispatch, Runner, receipt, Audit, or production route was enabled. ADR-0039 remains planning-only and ADR-0114 remains Proposed/null.
- **DONE — §400 cross-client credential lifecycle candidate authenticated transport**: Forge Runtime and Snaplink Console add explicitly invoked owner-scoped metadata-only POST adapters for `/api/v1/device-enrollment-heartbeat/credential-candidate`. Runtime validates request/response device, action, revision, and issue/rotate bindings; Snaplink Console requires an explicit candidate origin, owner binding, and `retryUnauthorized=false`, so a candidate 401 is never refreshed or replayed. Request bodies contain no bearer or credential material; startup, TUI sync, default Gate, and Core production routing remain closed. No challenge consumption, credential issuance, enrollment, heartbeat, inventory authority, selection, reservation, scheduling, dispatch, Runner, Run, receipt, Audit, or production route behavior was added; ADR-0039 remains planning-only and ADR-0114 remains Proposed/null.
- **DONE — Explicit Rust CLI/TUI credential lifecycle candidate command (cross-device plan §401)**: Forge Runtime exposes `remote credential-candidate preview --input FILE|-` and TUI `credential-candidate --input FILE` as explicit one-POST consumers for the injected metadata-only candidate. Strict bounded JSON and response binding reject drift, duplicate/unknown/secret-bearing fields, authority, and material; 401/network failures are not replayed. TUI startup/sync and local preview remain request-free, with `-` reserved for the CLI. No credential issuance/material, challenge/enrollment/heartbeat acceptance, inventory authority, selection/reservation, scheduling, dispatch, Runner, Run, receipt, Audit, or production route behavior was enabled; ADR-0039 remains planning-only and ADR-0114 remains Proposed/null.
- **DONE — Flutter Sessions Gate credential-candidate transport closure (cross-device plan §402)**: Snaplink Console now wires the explicit metadata-only credential lifecycle candidate through the shared Web/App/Mobile Sessions Gate with pinned owner, request, and candidate origin. Focused widget coverage proves one Bearer POST only when the opt-in flag and complete binding are present, the default Gate remains request-free, and a foreign-owner response is rejected. The stale Gate binding references and candidate panel analysis errors are fixed. No credential material, challenge/enrollment/heartbeat acceptance, inventory authority, selection/reservation, scheduling, dispatch, Runner, Run, receipt, Audit, or production route behavior was enabled; ADR-0039 remains planning-only and ADR-0114 remains Proposed/null.
- **DONE — Cross-ecosystem device credential candidate response parity (cross-device plan §403)**: Mirrored the canonical metadata-only credential candidate response into Aero-ID, Aero-IM, Aero-Vault, and Snaplink Audit Governance. Receiver contracts strictly enforce owner/device/revision, action-specific previous/next state bindings, safe windows, preview/publication flags, and all-false authority while rejecting unknown, duplicate, trailing, secret-bearing, binding, key, preview, publication, and authority mutations. No credential material, lifecycle persistence, challenge, enrollment, heartbeat, inventory authority, placement, selection, reservation, scheduling, dispatch, Runner, Run, receipt, Audit, or production route was added.
- **DONE — Lifecycle-registry client-instance view composition (cross-device plan §404)**: Added the private Core test/migration projection that reads one owner-bound lifecycle-registry source for both client-instance session and resource views, with five client kinds, sorted resources, owner/duplicate/transport/authority regressions, and production 404 closure. No registration, enrollment, heartbeat, inventory authority, target selection, reservation, scheduling, dispatch, Runner, Run, receipt, Audit, or production route was enabled; ADR-0039 remains planning-only and ADR-0114 remains Proposed/null.
- **DONE — Authenticated cross-client session projection acceptance (cross-device plan §405)**: The opt-in Snaplink JWT E2E creates two owner Conversations, serves five CLI/TUI/Web/App/Mobile declarations, verifies each CLI `remote sessions list --instance` local projection, sends a TUI Prompt, and reads it from a separate authenticated client. It records the HTTP boundary and rejects instance query leakage and device/placement/reservation/dispatch/Runner/execution requests; production candidate routing remains 404. No registration, authoritative instance metadata, scheduling, selection, reservation, dispatch, Runner, Run, receipt, Audit, or production device route was added; ADR-0039 remains planning-only and ADR-0114 remains Proposed/null.
- **DONE — Flutter resource-view display-only transport guard (cross-device plan §406)**: The explicit Snaplink Console client-instance/resource-view reader now requires the expected owner and `isDisplayOnly`, matching the session reader and Sessions screen. Focused transport coverage rejects a response that clears `read_only` and preserves one-shot 401/no-refresh behavior. No registration, Prompt/Run mutation, device authentication, enrollment, heartbeat, inventory authority, selection, reservation, scheduling, dispatch, Runner, receipt, Audit, or production route behavior was added; ADR-0039 remains planning-only and ADR-0114 remains Proposed/null.
- **DONE — Authenticated Console client-instance projection and Prompt acceptance (cross-device plan §407)**: The opt-in Forge Core/Flutter E2E crosses the real Snaplink JWT, two owner Conversations, and one five-kind CLI/TUI/Web/App/Mobile projection. The shared Sessions Gate restores the bearer, loads the candidate, filters to a declared instance, sends one owner-scoped Prompt, and an independent authenticated client reads it back. The recorder rejects instance-query leakage and device/placement/reservation/dispatch/Runner/execution requests; candidate origin/owner/flag remain harness-only and production remains 404. No registration, instance authority, Run/pending intent, selection, reservation, scheduling, dispatch, Runner, receipt, Audit, or production device route was added; ADR-0039 remains planning-only and ADR-0114 remains Proposed/null.
- **DONE — Runtime TUI client-instance Prompt write boundary (cross-device plan §408)**: Runtime TUI now blocks Prompt creation and retry when the selected Conversation is outside the active local client-instance/session declaration, without issuing HTTP and while retaining the pending idempotency tuple. No-filter and ordinary scope-filter semantics remain unchanged; no registration, heartbeat, inventory, selection, reservation, scheduling, dispatch, Runner, Run, receipt, Audit, or production route was added; ADR-0039 remains planning-only and ADR-0114 remains Proposed/null.
- **DONE — Authenticated resource-view-only client-instance Prompt acceptance (cross-device plan §409)**: The opt-in Forge Core/Flutter/Runtime acceptance harness uses only the owner-bound resource-view candidate to drive instance filtering and Prompt writes across TUI and the shared Web/App/Mobile Sessions surface. Two real owner Conversations are written and independently read back; session-view, instance-query, device, placement, reservation, dispatch, Runner, and execution requests are rejected, and production routes remain 404. No registration, heartbeat, inventory authority, selection, reservation, scheduling, dispatch, Runner, Run, receipt, Audit, or production route was added; ADR-0039 remains planning-only and ADR-0114 remains Proposed/null.
- **DONE — CLI client-instance Prompt read/write boundary (cross-device plan §410)**: Runtime CLI `remote prompts list/add` now support `--instance` and optional `--instance-view FILE|-`; strict local projection checks reject hidden Conversations before any Prompt request, while visible owner, CAS, and idempotency behavior remains unchanged. No instance query, registration, enrollment, heartbeat, inventory, selection, reservation, scheduling, dispatch, Runner, Run, receipt, Audit, or production route was added; ADR-0039 remains planning-only and ADR-0114 remains Proposed/null.
- **DONE — Runtime TUI client-instance Prompt read boundary (cross-device plan §411)**: TUI `open CONVERSATION_ID` and `older` now enforce the active local client-instance/session or resource projection before owner Conversation and Prompt-history GETs. Hidden or unvalidated Conversations are blocked without HTTP; no-instance and ordinary scope-filter semantics are preserved. No instance authorization, registration, enrollment, heartbeat, inventory, selection, reservation, scheduling, dispatch, Runner, Run, receipt, Audit, or production route was added; ADR-0039 remains planning-only and ADR-0114 remains Proposed/null.
- **DONE — Runtime TUI client-instance detail and sync read boundary (cross-device plan §412)**: TUI `detail/show CONVERSATION_ID` and selected-session `sync` now apply the active local client-instance/session or resource projection before owner Conversation and Prompt-history GETs. Hidden or unvalidated Conversations are blocked locally; no-instance and ordinary scope-filter semantics remain unchanged. No instance authorization, registration, enrollment, heartbeat, inventory, selection, reservation, scheduling, dispatch, Runner, Run, receipt, Audit, or production route was added; ADR-0039 remains planning-only and ADR-0114 remains Proposed/null.
- **DONE — Console client-instance Prompt history read boundary (cross-device plan §413)**: The shared Web/App/Mobile Sessions screen now guards deep-link selection, change-feed refresh, initial selection, and older Prompt history reads with the selected session/resource projection. Hidden Conversations issue no Prompt GET and clear the local Prompt panel; owner API and production routes remain unchanged. No instance authorization, registration, enrollment, heartbeat, inventory, selection, reservation, scheduling, dispatch, Runner, Run, receipt, Audit, or production route was added; ADR-0039 remains planning-only and ADR-0114 remains Proposed/null.
- **DONE — CLI client-instance Conversation detail read boundary (cross-device plan §414)**: Runtime CLI `remote sessions show` now supports `--instance` and optional `--instance-view FILE|-`, rejects hidden Conversations before any detail GET, and preserves visible owner-scoped and no-option behavior. No registration, enrollment, heartbeat, inventory, selection, reservation, scheduling, dispatch, Runner, Run, receipt, Audit, or production route was added; ADR-0039 remains planning-only and ADR-0114 remains Proposed/null.
- **DONE — Runtime TUI v1 inventory refresh boundary (cross-device plan §415)**: An explicit TUI `inventory read` now opts the process into refreshing the same owner-scoped v1 inventory observation during `sync`, with response validation, authorization-failure cleanup, and a focused HTTP-order test. Startup and ordinary sync remain request-free until the explicit read; no registration, enrollment, heartbeat, inventory authority, selection, reservation, scheduling, dispatch, Runner, Run, receipt, Audit, or production route was added; ADR-0039 remains planning-only and ADR-0114 remains Proposed/null.
- **DONE — Authenticated Runner dispatch-plan preview transport and four-ecosystem parity (cross-device plan §416)**: Forge Core's injected runner dispatch-plan preview now flows through explicit authenticated Runtime CLI/TUI and Snaplink Console preview adapters. They validate owner, conversation/run/Attempt/lease bindings, candidate origin, response shape, and all-false display-only authority, perform one POST with no 401 replay, and keep production construction 404. The canonical preview fixture is mirrored and strictly consumed by Aero-ID, Aero-IM, Aero-Vault, and Snaplink Audit Governance. Empty ready-candidate reasons encode as `[]`; no registry, lease issuance, reservation, scheduling, dispatch, Runner, Run, receipt, Audit, or production route was enabled; ADR-0039 remains planning-only and ADR-0114 remains Proposed/null.
- **DONE — Console Sessions Gate Runner dispatch-plan preview candidate (cross-device plan §417)**: The shared Web/App/Mobile Sessions Gate now wires an explicit Run/Attempt/lease dispatch-plan declaration into the authenticated candidate reader and display-only card. Owner/path/command/target/lease/time bindings are pinned and re-decoded; transient failures retain only the validated prior observation, authorization failures clear owner state, and the default Gate remains request-free. Focused widget coverage proves one bearer POST and the no-candidate default. No inventory authority, target selection, reservation, scheduling, dispatch, Runner, receipt, Audit, or production route was enabled; ADR-0039 remains planning-only and ADR-0114 remains Proposed/null.
- **DONE — Console Sessions Gate local Runner execution-readiness candidate (cross-device plan §418)**: The shared Web/App/Mobile Sessions Gate now consumes the explicit Prompt/Run/Attempt/lease local Runner preview request through one authenticated candidate POST. It validates path, intent, receipt, command, lease, timestamp, and all-false authority bindings, renders only metadata, and keeps argv/workspace/fencing/output/diagnostics and execution actions absent. The default Gate remains request-free; no device selection, reservation, dispatch, durable Run/receipt, or production route was enabled; ADR-0039 remains planning-only and ADR-0114 remains Proposed/null.
- **DONE — Console Sessions Gate v1 inventory candidate adapter (cross-device plan §419)**: Snaplink Console's shared Web/App/Mobile Gate now enables a caller-declared owner/origin/flag adapter for the existing owner-scoped v1 `/devices` observation. The adapter performs one authenticated, non-replayed GET, preserves explicit reader precedence, and feeds the strict read-only inventory panel; focused coverage proves exact request binding, default request-free behavior, and missing-origin closure. No enrollment, heartbeat, inventory authority, target selection, reservation, scheduling, dispatch, Runner, receipt, Audit, or production route was enabled; ADR-0039 remains planning-only and ADR-0114 remains Proposed/null.
- **DONE — Runtime pending Run-intent client-instance boundary (cross-device plan §420)**: Forge Runtime CLI `remote run-intents list/submit/timeline` now accept `--instance` with optional `--instance-view FILE|-` and validate the caller-declared session projection before any pending Run-intent GET/POST; hidden Conversations issue no private request while visible reads preserve owner, cursor, CAS, and idempotency behavior. TUI list, timeline, submit, retry, and explicit sync refresh reuse the same local guard and retain pending recovery tuples when blocked. Pending Run-intents remain inert metadata; no Run start, device selection, reservation, scheduling, dispatch, Runner, receipt, Audit, or production route was enabled; ADR-0039 remains planning-only and ADR-0114 remains Proposed/null.
- **DONE — Runtime Run read client-instance projection boundary (cross-device plan §423)**: Forge Runtime CLI `remote runs list/observed/timeline` now accept `--instance` with optional `--instance-view FILE|-` and reject hidden Conversations before private Run summary, observation, or timeline GETs. TUI Runs list, observed, timeline, and selected-Run sync reuse the active local projection; hidden sessions issue no private Run request and stale selected Run metadata is cleared without touching pending recovery state. Visible cursor, resume, and metadata-only behavior remains unchanged. Focused parser/dispatch/TUI tests and the contract script cover local-view request avoidance and hidden-request closure. No instance authority, registration, enrollment, heartbeat, inventory authority, selection, reservation, scheduling, dispatch, Runner, receipt, Audit, or production route was added; ADR-0039 remains planning-only and ADR-0114 remains Proposed/null.
- **DONE — Console Sessions Gate execution-consent preview candidate (cross-device plan §421)**: Snaplink Console now wires the explicit owner/origin/enable execution-consent preview candidate through the shared Web/App/Mobile Sessions Gate. The selected Conversation ID is carried into one strict read-only GET and checked on return; the request disables unauthorized replay, and a metadata-only card exposes project/profile/digest/TTL without any consent or execution action. Focused API and widget tests prove one exact candidate read, default zero candidate traffic, selected Conversation binding, and no refresh/replay; production authority routes remain closed under ADR-0039 and Proposed ADR-0114.
- **DONE — Local Runner execution-preview contract parity (cross-device plan §422)**: The canonical `forge.runner-local-execution-preview/v1` fixture is mirrored byte-for-byte and strictly consumed by Aero-ID, Aero-IM's audit connector, Aero-Vault, and Snaplink Audit Governance. Receiver tests reject unknown/duplicate fields, binding and numeric drift, selection mutations, and authority claims; the contract script compares all four copies and runs each focused suite. This is injected local Runner observation evidence only: no device authentication/enrollment, lease, reservation, scheduling, dispatch, remote transport, durable Run/receipt, Audit publication, or production route was enabled; ADR-0039 remains planning-only and ADR-0114 remains Proposed/null.
- **DONE — Console Sessions Run client-instance read boundary (cross-device plan §424)**: The shared Web/App/Mobile Sessions screen now rechecks the selected local client-instance projection inside Runs list and scheduled Run-observation refresh paths. Hidden Conversations clear Run/timeline/pending-intent and adjacent metadata before any `/runs` request; visible owner-scoped pagination and metadata behavior remain unchanged. Focused widget coverage records only the visible session's Prompt and Run reads through an instance filter. No instance authority, registration, enrollment, heartbeat, inventory authority, selection, reservation, scheduling, dispatch, Runner, durable Run/receipt, Audit, or production route was added; ADR-0039 remains planning-only and ADR-0114 remains Proposed/null.
- **DONE — Console Sessions pending Run-intent client-instance read/write boundary (cross-device plan §425)**: The shared Web/App/Mobile Sessions screen now rechecks the selected local client-instance projection before pending Run-intent list, older-page, timeline, and inert scheduling-review submission callbacks. Hidden Conversations clear pending Run-intent and Run-adjacent metadata before any `/run-intents` callback or candidate POST; visible owner, cursor, CAS, and idempotency behavior remains unchanged. Focused widget coverage records pending reads only for the visible Conversation and no hidden callback. Pending Run-intents remain inert metadata; no Run, device selection, reservation, scheduling, dispatch, Runner, receipt, Audit, or production route was added; ADR-0039 remains planning-only and ADR-0114 remains Proposed/null.
- **DONE — Console Sessions execution-consent client-instance read boundary (cross-device plan §426)**: The shared Web/App/Mobile Sessions screen now rechecks the selected local client-instance projection before and after an explicit execution-consent preview reader. Hidden Conversations produce no execution-consent callback, and an in-flight response cannot reattach after the projection changes; visible owner and Conversation binding behavior remains unchanged. The preview remains metadata-only; no consent grant, Run creation, device selection, reservation, scheduling, dispatch, Runner, receipt, Audit, or production route was added; ADR-0039 remains planning-only and ADR-0114 remains Proposed/null.
- **DONE — Console Sessions complete Run-resource client-instance boundary (cross-device plan §427)**: The shared Web/App/Mobile Sessions screen now checks the active client-instance projection around Run timeline/device observation, Run observation, preflight, Runner dispatch-plan, local readiness, and execution-reconciliation reads. Hidden Conversations are blocked before private transport and stale Run metadata is cleared; Prompt append and inert scheduling-review results are also rejected after an instance change. Focused widget coverage proves the strict Run observation callback remains visible-session-only. No device registration, enrollment, heartbeat, inventory authority, selection, reservation, scheduling, dispatch, Runner, receipt, Audit, or production route was added; ADR-0039 remains planning-only and ADR-0114 remains Proposed/null.
- **DONE — Console Sessions dynamic client-instance projection revocation boundary (cross-device plan §428)**: The shared Web/App/Mobile Sessions screen now reconciles explicit session/resource-view responses and injected preview replacements against the selected client-instance filter. When a refreshed projection hides the selected Conversation, Prompt history, Run summaries, timeline, pending Run-intent, and Run-adjacent candidate state are invalidated with generation bumps; if the Conversation later reappears, stale private metadata cannot return without a new owner read. Focused Flutter coverage proves hidden and restored projections do not resurrect the Run row or Prompt content. This remains a local unverified display boundary; no client registration, device enrollment/heartbeat, inventory authority, selection, reservation, scheduling, dispatch, Runner, Run, receipt, Audit, or production route was added; ADR-0039 remains planning-only and ADR-0114 remains Proposed/null.
- **DONE — Console Sessions static Run-resource client-instance render boundary (cross-device plan §429)**: The shared Web/App/Mobile Sessions view now gates caller-injected and cached Run-bound observations plus error/stale/loading cards on the selected client-instance projection. Hidden sessions cannot display a static content-free Run observation; focused Flutter coverage proves the card disappears after a refreshed projection hides the selected session. This remains an unverified local display boundary with no registration, enrollment, heartbeat, inventory authority, selection, reservation, scheduling, dispatch, Runner, Run, receipt, Audit, or production route authority; ADR-0039 remains planning-only and ADR-0114 remains Proposed/null.

- **DONE — Console Sessions removed client-instance projection empty boundary (cross-device plan §430)**: A selected instance that disappears from refreshed session/resource metadata now yields an empty local session projection and keeps the “All client instances” clearing entry; owner sessions are never shown by fallback. Focused Flutter coverage proves removal of the Web declaration leaves the Conversation hidden. This remains an unverified local display boundary with no registration, enrollment, heartbeat, inventory authority, selection, reservation, scheduling, dispatch, Runner, Run, receipt, Audit, or production route authority; ADR-0039 remains planning-only and ADR-0114 remains Proposed/null.

- **DONE — Runtime TUI missing client-instance projection fail-closed boundary (cross-device plan §431)**: An active TUI instance filter with an unavailable validated session/resource view now yields no visible owner Conversations and clears the selected local Prompt/Run panels; it never falls back to the full owner list. Focused Rust tests cover the matcher and rendered empty projection. This remains an unverified local display/write boundary with no registration, enrollment, heartbeat, inventory authority, selection, reservation, scheduling, dispatch, Runner, Run, receipt, Audit, or production route authority; ADR-0039 remains planning-only and ADR-0114 remains Proposed/null.
- **DONE — Console Sessions client-instance reader revocation boundary (cross-device plan §432)**: The Web/App/Mobile Sessions screen now retains a selected instance filter when its owner reader is revoked or replaced, clears private Prompt/Run state, and keeps the local projection empty until a validated view returns. A missing instance declaration cannot broaden the owner list. Focused Flutter coverage proves the revoked reader hides both Web and CLI sessions and leaves the stale-filter clear surface available. This remains an unverified local display boundary with no registration, enrollment, heartbeat, inventory authority, selection, reservation, scheduling, dispatch, Runner, Run, receipt, Audit, or production route authority; ADR-0039 remains planning-only and ADR-0114 remains Proposed/null.
- **DONE — Console Sessions resource-view reader revocation boundary (cross-device plan §433)**: The Web/App/Mobile Sessions screen now retains an active instance filter when its owner resource-view reader is revoked or replaced, clears private Prompt/Run state, and keeps the local projection empty until a validated declaration returns. Focused Flutter coverage proves the resource-view-only path hides both Web and CLI sessions after reader removal and leaves the stale-filter clear surface available. This remains an unverified local display boundary with no registration, enrollment, heartbeat, inventory authority, selection, reservation, scheduling, dispatch, Runner, Run, receipt, Audit, or production route authority; ADR-0039 remains planning-only and ADR-0114 remains Proposed/null.
- **DONE — Runtime TUI client-instance reader revocation boundary (cross-device plan §434)**: Forge Runtime now lets TUI users revoke one explicit `session-view` or `resource-view` reader while retaining the active instance filter. If no validated replacement remains, the Conversation projection is empty and private Prompt/Run state is cleared; non-authorizing refresh failures drop the affected stale candidate, while 401/403 clears the owner view. Focused Rust coverage proves explicit revocation and failed refresh never broaden the selected instance back to all owner sessions. This remains an unverified local display boundary with no registration, enrollment, heartbeat, inventory authority, selection, reservation, scheduling, dispatch, Runner, Run, receipt, Audit, or production route authority; ADR-0039 remains planning-only and ADR-0114 remains Proposed/null.
- **DONE — Cross-client Prompt CAS receipt sequencing (cross-device plan §435)**: Rust Runtime and Snaplink Console now require every successful owner-scoped Prompt append response to return exactly `expected_version + 1` within the JSON-safe boundary. Stale or skipped receipts are rejected before CLI/TUI/Flutter local state advances, while idempotent replay retains the same deterministic next version. Focused Rust request tests and Flutter API tests cover non-sequential responses. Prompt remains storage-only; no Run, device selection, reservation, scheduling, dispatch, Runner, receipt, Audit, or production route authority was added; ADR-0039 remains planning-only and ADR-0114 remains Proposed/null.
- **DONE — Cross-client Prompt replay receipt visibility (cross-device plan §436)**: Forge Runtime now requires the successful remote Prompt receipt's boolean `replayed` marker, rejecting missing or non-boolean values before clearing a pending write. The TUI and Snaplink Console display when a retry replayed an existing Prompt; stable idempotency-key Flutter/TUI journeys and Rust request coverage exercise this path. Prompt remains storage-only; no Run, device selection, reservation, scheduling, dispatch, Runner, receipt, Audit, or production route authority was added; ADR-0039 remains planning-only and ADR-0114 remains Proposed/null.
- **DONE — CLI Prompt replay receipt visibility (cross-device plan §437)**: Runtime CLI Prompt acknowledgements now derive from the validated boolean `replayed` receipt marker, distinguishing a fresh stored Prompt from an idempotent replay while retaining the JSON response for automation and a conservative missing-marker message. Focused Rust coverage exercises all display branches; Prompt remains storage-only with no Run, device selection, reservation, scheduling, dispatch, Runner, receipt, Audit, or production route authority; ADR-0039 remains planning-only and ADR-0114 remains Proposed/null.
- **DONE — Cross-ecosystem execution-lease checkpoint parity (cross-device plan §438)**: The canonical `forge.execution-lease-checkpoint/v1` value is mirrored byte-for-byte and strictly consumed by Aero-ID, Aero-IM, Aero-Vault, and Snaplink Audit Governance; receiver tests reject unknown/duplicate fields, authority mutations, invalid grant/terminal proof relationships, and preserve uncertain terminal evidence. Core and Runtime contract checks plus the cross-ecosystem script now cover the restart/fencing value. This is offline interoperability evidence only; no live lease issuance/persistence, device enrollment, reservation, scheduling, dispatch, Runner, receipt, Audit publication, or production route authority was added; ADR-0039 remains planning-only and ADR-0114 remains Proposed/null.
- **DONE — Runtime CLI/TUI execution-lease checkpoint preview (cross-device plan §439)**: Forge Runtime adds the offline `device execution-lease-checkpoint-preview --input FILE|-` command and matching request-free TUI preview. Both use the pure `LeaseState::from_checkpoint` validator, expose bounded grant/time and terminal/uncertain status, hide fencing/proof/digest/reason material, and reject unknown/duplicate fields or authority mutation. Focused parser, command, human-output, and TUI tests are wired into the contract script. No live lease restoration/persistence, device enrollment, reservation, scheduling, dispatch, Runner, receipt, Audit publication, or production route authority was added; ADR-0039 remains planning-only and ADR-0114 remains Proposed/null.
- **DONE — Console execution-lease checkpoint projection (cross-device plan §440)**: Snaplink Console's shared Web/App/Mobile Forge Sessions surface now strictly consumes the canonical `forge.execution-lease-checkpoint/v1` value as a local display-only card. Contract/widget coverage validates the four canonical outcomes, rejects unknown/duplicate fields, authority mutations, grant drift, and expectation drift, and keeps fencing/proof/digest/reason material out of the UI. No lease restoration/persistence, device enrollment, reservation, scheduling, dispatch, Runner, receipt, Audit publication, or production route authority was added; ADR-0039 remains planning-only and ADR-0114 remains Proposed/null.
- **DONE — Console local execution-lease checkpoint import (cross-device plan §441)**: The shared Web/App/Mobile Forge Sessions surface now imports a bounded local execution-lease checkpoint JSON through the platform picker or an injected reader, strictly re-decodes it, and renders the same metadata-only card used by the Runtime preview. Focused widget coverage proves the import path remains request-free and exposes no proof, fencing, digest, or reason material. No lease restoration/persistence, device enrollment, reservation, scheduling, dispatch, Runner, receipt, Audit publication, or production route authority was added; ADR-0039 remains planning-only and ADR-0114 remains Proposed/null.
- **DONE — Console local v2 device inventory import (cross-device plan §442)**: The shared Web/App/Mobile Forge Sessions surface now imports bounded `forge.device-inventory-observation/v2` JSON through the platform picker or an injected reader, strictly re-decodes it, and renders the existing metadata-only inventory panel with revision, generation, heartbeat, reservation, and GPU declarations. Focused widget coverage proves the import path remains request-free and does not call `/devices`; no enrollment, heartbeat acceptance, inventory authority, target selection, reservation, scheduling, dispatch, Runner, receipt, Audit publication, or production route authority was added. ADR-0039 remains planning-only and ADR-0114 remains Proposed/null.
- **DONE — Console local v2 placement evaluation import (cross-device plan §443)**: The shared Web/App/Mobile Forge Sessions surface now imports bounded `forge.device-inventory-placement-evaluation/v2` JSON through the platform picker or an injected reader, strictly re-decodes the candidate decisions, requirements, owner/time bindings, and all-false authority before rendering exclusion reasons. Focused widget coverage proves no `/devices` request and no target-selection control; no reservation, scheduling, dispatch, Runner, receipt, Audit publication, or production route authority was added. ADR-0039 remains planning-only and ADR-0114 remains Proposed/null.
- **DONE — Console local placement-batch evaluation import (cross-device plan §444)**: The shared Web/App/Mobile Forge Sessions surface now imports bounded `forge.device-inventory-placement-batch-evaluation/v1` JSON through the platform picker or an injected reader, strictly re-decodes candidate/error cases and null-selection authority, and renders the existing dry-run panel. Focused widget and contract coverage prove duplicate-key rejection and no `/devices` request; no registration, heartbeat, target selection, reservation, scheduling, dispatch, Runner, receipt, Audit publication, or production route authority was added. ADR-0039 remains planning-only and ADR-0114 remains Proposed/null.
- **DONE — Cross-ecosystem placement-batch evaluation receiver parity (cross-device plan §445)**: The canonical `forge.device-inventory-placement-batch-evaluation/v1` fixture is mirrored byte-for-byte and strictly consumed by Aero-ID, Aero-IM, Aero-Vault, and Snaplink Audit Governance. Receiver tests preserve the six candidate/error cases, deterministic exclusion reasons, null selected target, and all-false authority while rejecting unknown/duplicate/trailing fields and authority or selection mutations. This is offline interoperability evidence only; no device authentication, heartbeat acceptance, authoritative inventory, target selection, reservation, scheduling, dispatch, Runner, execution, receipt, Audit publication, or production route was enabled. ADR-0039 remains planning-only, ADR-0114 remains Proposed/null, and P3b/P4 remain gated.
- **DONE — Cross-ecosystem persisted-inventory placement-input parity (cross-device plan §446)**: The canonical `forge.device-inventory-placement-input/v1` fixture is mirrored byte-for-byte and strictly consumed by Aero-ID, Aero-IM, Aero-Vault, and Snaplink Audit Governance. Receivers cover twelve state/error cases, closed unknown policy attributes, owner/Runner bindings, safe timestamp handling, stable errors, all-false authority, and reject unknown/duplicate/trailing, owner, and authority mutations. This remains pure P3a interoperability evidence; no Runner authentication, heartbeat acceptance, authoritative inventory, target selection, reservation, scheduling, dispatch, execution, receipt, Audit publication, or production route was enabled. ADR-0039 remains planning-only, ADR-0114 remains Proposed/null, and P3b/P4 remain gated.
- **DONE — Cross-ecosystem persisted-inventory placement-evaluation parity (cross-device plan §447)**: The canonical `forge.device-inventory-placement-evaluation/v1` fixture is mirrored byte-for-byte and strictly consumed by Aero-ID, Aero-IM, Aero-Vault, and Snaplink Audit Governance. Receivers preserve the fixed policy/source case, revision/device/Runner binding, deterministic exclusion reasons, JSON-safe evaluation time, unverified markers, and all-false authority while rejecting unknown/duplicate/trailing, source, and authority mutations. This remains pure P3a interoperability evidence; no Runner authentication, heartbeat acceptance, authoritative inventory, target selection, reservation, scheduling, dispatch, execution, receipt, Audit publication, or production route was enabled. ADR-0039 remains planning-only, ADR-0114 remains Proposed/null, and P3b/P4 remain gated.
- **DONE — Runtime persisted-inventory placement-evaluation v1 preview (cross-device plan §448)**: Forge Runtime CLI and TUI now consume the canonical `forge.device-inventory-placement-evaluation/v1` through a bounded local file preview, recompute the online decision through the Rust persisted-inventory comparator, and render revision/device/Runner binding, deterministic reasons, and unverified markers. Duplicate/unknown/trailing input, source-fixture drift, policy mutations, and authority mutations fail closed; TUI file reads remain request-free and `-` remains CLI-only. No Runner authentication, heartbeat acceptance, authoritative inventory, target selection, reservation, scheduling, dispatch, execution, receipt, Audit publication, or production route was enabled. ADR-0039 remains planning-only, ADR-0114 remains Proposed/null, and P3b/P4 remain gated.
- **DONE — Console persisted-inventory placement-evaluation v1 preview (cross-device plan §449)**: The shared Snaplink Console Web/App/Mobile Forge Sessions surface now imports the canonical `forge.device-inventory-placement-evaluation/v1` through a bounded local reader or workspace picker, strictly rejects duplicate/unknown/trailing/oversized input plus invalid source-envelope, policy-shape, and authority fields, and renders a value-only panel with revision/device/Runner binding, deterministic reasons, unverified declarations, and requirements. Focused widget coverage proves injected/imported previews issue no `/devices` or placement request and expose no target-selection control. This remains pure P3a interoperability evidence; no Runner authentication, heartbeat acceptance, authoritative inventory, target selection, reservation, scheduling, dispatch, execution, receipt, Audit publication, or production route was enabled. ADR-0039 remains planning-only, ADR-0114 remains Proposed/null, and P3b/P4 remain gated.
- **DONE — Cross-ecosystem persisted-inventory placement-evaluation v2 parity (cross-device plan §450)**: The canonical `forge.device-inventory-placement-evaluation/v2` fixture is mirrored byte-for-byte and strictly consumed by Aero-ID, Aero-IM, Aero-Vault, and Snaplink Audit Governance. Receivers validate the nested lossless v2 observation, requirements, sorted decisions, source/time/owner binding, revision/generation/heartbeat counters, reservation/GPU declarations, and deterministic exclusion reasons while rejecting unknown, duplicate, trailing, selected-target, owner/time, observation, decision, and authority mutations. This is offline P3a interoperability evidence only; no Runner authentication, heartbeat acceptance, authoritative inventory, target selection, reservation, scheduling, dispatch, execution, receipt, Audit publication, or production route was enabled. ADR-0039 remains planning-only, ADR-0114 remains Proposed/null, and P3b/P4 remain gated.
- **DONE — Console local multi-instance resource-summary import (cross-device plan §451)**: Snaplink Console's shared Web/App/Mobile Forge Sessions surface now imports the complete `forgeos.device-resource-summary-contract/v1` fixture through a bounded local reader or platform workspace picker. The strict decoder rejects duplicate/unknown/trailing/oversized JSON, owner and nested inventory/placement drift, aggregate totals that do not recompute from declarations, selected targets, and authority-bearing values. A read-only panel exposes Conversation/Run binding, device and Runner counts, aggregate CPU/memory/storage/GPU totals, eligible counts, and the all-false/no-selection boundary. Focused contract and widget coverage proves injection/import remain request-free with no `/devices` or placement transport. This advances P3a cross-client resource perception only; no Runner authentication, heartbeat acceptance, authoritative inventory, target selection, reservation, scheduling, dispatch, execution, receipt, Audit publication, or production route was enabled. ADR-0039 remains planning-only, ADR-0114 remains Proposed/null, and P3b/P4 remain gated.
- **DONE — Console local client-instance resource-view import (cross-device plan §452)**: Snaplink Console's shared Web/App/Mobile Forge Sessions surface now imports the complete `forge.client-instance-resource-view/v1` observation through a bounded local reader or platform workspace picker. The existing strict decoder is reused and re-applied before display, preserving owner, sorted client-instance/session rows, sorted device/Runner resources, capacity bounds, and all-false authority; the imported view also drives the existing local instance filter without broadening it. Focused widget coverage proves no `/devices` or client-instance/resource candidate request and no scheduling control. This remains P3a cross-client resource perception only; no Runner authentication, heartbeat acceptance, authoritative inventory, target selection, reservation, scheduling, dispatch, execution, receipt, Audit publication, or production route was enabled. ADR-0039 remains planning-only, ADR-0114 remains Proposed/null, and P3b/P4 remain gated.
- **DONE — Console local client-instance session-view import (cross-device plan §453)**: Snaplink Console's shared Web/App/Mobile Forge Sessions surface now imports the complete `forge.client-instance-session-view/v1` observation through a bounded local reader or platform workspace picker. The existing strict decoder is re-applied before display, preserving owner, sorted instance and session IDs, bounded timestamps, and all-false authority; the imported view also drives the local instance filter without widening it. Focused widget coverage proves no `/devices` or client-instance/session candidate request and no Prompt, Run, or scheduling control. This remains P3a cross-client session perception only; no instance authentication, Prompt/Run mutation, device inventory authority, target selection, reservation, scheduling, dispatch, Runner execution, Audit publication, or production route was enabled. ADR-0039 remains planning-only, ADR-0114 remains Proposed/null, and P3b/P4 remain gated.

- **DONE — Five-client instance matrix parity (cross-device plan §454)**: The canonical `forge.client-instance-session-view/v1` and `forge.client-instance-resource-view/v1` fixtures now declare sorted CLI, TUI, Web, desktop App, and Mobile instances. The bytes are mirrored and strictly consumed by Forge Core, Runtime CLI/TUI, Snaplink Console, Aero-ID, Aero-IM, Aero-Vault, and Snaplink Audit Governance; Console's local import tests assert every client kind. The values remain unverified display declarations with all authority false, so no registration, heartbeat acceptance, selection, reservation, scheduling, dispatch, Runner execution, Run/receipt persistence, Audit publication, or production route was added. ADR-0039 remains planning-only, ADR-0114 remains Proposed/null, and P3b/P4 remain gated. Focused Go, Rust, Flutter, Aero-ID, Aero-IM, Aero-Vault, and Audit Governance contract tests pass.
- **DONE — Authenticated Web/App/Mobile client-instance Prompt parity (cross-device plan §455)**: The opt-in Snaplink JWT acceptance path now runs the shared Flutter Sessions surface as three independent owner-authenticated clients. Web, desktop App, and Mobile each select their declared instance, receive only its declared Conversation projection, and append one Prompt; a separate authenticated Runtime client verifies both Conversation histories. CLI still validates all five instance projections and TUI still writes within its selected instance. The acceptance rejects device/placement/reservation/dispatch/Runner/execution requests and confirms the production candidate route remains 404. This remains session/Prompt interoperability evidence with unverified candidate metadata; no device registration, heartbeat acceptance, inventory authority, scheduling, dispatch, Runner execution, Run/receipt persistence, or Audit publication was enabled. ADR-0039 remains planning-only, ADR-0114 remains Proposed/null, and P3b/P4 remain gated. The configured opt-in E2E passes.
- **DONE — Authenticated Web/App/Mobile resource-view projection parity (cross-device plan §456)**: The opt-in JWT acceptance path now runs the shared resource-view surface as independent Web, desktop App, and Mobile clients. Each consumes the strict five-client `forge.client-instance-resource-view/v1` candidate, sees the display-only device resource, filters only its declared Conversation, and appends one Prompt; a separate Runtime client verifies the three receipts with the TUI receipt. The Flutter screen preserves owner-level resource/session projections across Conversation switches and guards against a late owner-list refresh restoring the prior selection. Candidate resources remain unverified, production candidate routes remain 404, and no device/placement/reservation/dispatch/Runner/execution authority is enabled. ADR-0039 remains planning-only, ADR-0114 remains Proposed/null, and P3b/P4 remain gated. The configured E2E passes twice.
- **DONE — Authenticated Web/App/Mobile dispatch-plan projection parity (cross-device plan §457)**: The opt-in JWT acceptance path now runs independent Web, desktop App, and Mobile clients through the resource-to-dispatch preflight boundary. Each reads the strict five-instance resource view, confirms its own Conversation declaration and two display-only device rows, and posts one owner/Conversation/Run-bound `forge.runner-dispatch-plan-preview/v1` request. The response retains both candidates and one declarative ready count while selected target, reservation, execution, dispatch, and Audit authority remain null/false. Production candidate routes remain 404; no lease issuance or Runner operation was added. ADR-0039 remains planning-only, ADR-0114 remains Proposed/null, and P3b/P4 remain gated. The configured E2E passes twice.
- **DONE — Authenticated Web/App/Mobile execution-readiness projection parity (cross-device plan §458)**: The opt-in JWT acceptance path now runs independent Web, desktop App, and Mobile clients through the resource-to-execution preflight boundary. Each reads the strict five-instance resource view, confirms its own Conversation declaration and two display-only Runner resources, and posts one bound `forge.runner-local-execution-preview/v1` request. The injected adapter returns a completed redacted metadata receipt with command/Attempt/target/Run bindings while execution and Audit authority stay false. Production candidate routes remain 404; no Run, lease, reservation, remote Runner, or Audit operation was added. ADR-0039 remains planning-only, ADR-0114 remains Proposed/null, and P3b/P4 remain gated. The configured E2E passes.
- **DONE — Authenticated Web/App/Mobile execution-reconciliation projection parity (cross-device plan §459)**: The opt-in JWT acceptance path now runs independent Web, desktop App, and Mobile clients through the resource-to-restart-image boundary. Each reads the strict five-instance resource view, confirms its own Conversation declaration and two display-only Runner resources, and posts one owner/Conversation/Run-bound `forge.execution-reconciliation-observation/v1` restart image. The candidate returns `await_terminal` with redacted metadata and all authority bits false. Production resource and execution-reconciliation routes remain 404; no durable Run/Attempt/lease/registry/Runner read or mutation, retry, or Audit operation was added. ADR-0039 remains planning-only, ADR-0114 remains Proposed/null, and P3b/P4 remain gated. The configured E2E passes twice.
- **DONE — Activation-gated owner inventory route (cross-device plan §460)**: Forge Server now mounts authenticated `/api/v1/devices` v1/v2 reads only after a complete accepted device-fabric request, owner-private lifecycle registry path, and authenticated session configuration pass validation. The persisted lifecycle image is rechecked against the verified JWT owner and evaluated with the Coordinator clock per read; missing/proposed activation and malformed/private-file/owner drift fail closed, while the normal constructor remains 404. This adds P3a observation visibility only: no enrollment, heartbeat write, credential issuance, target selection, reservation, scheduling, dispatch, Runner, execution, or Audit authority was added, and P3b/P4 remain gated. Focused Config, route, and actual `Run` HTTP tests pass.
- **DONE — Activation-gated five-client session/resource views (cross-device plan §461)**: Forge Server optionally mounts authenticated client-instance/session and client-instance/resource views from a private `forge.client-instance-session-view/v1` declaration image only when the accepted Fabric gate and lifecycle image are present. Each read revalidates the declaration, exact JWT owner, and current lifecycle image; resource rows retain the persisted Runner observation and lease timestamps and keep all authority false. With the configured Runtime and Console E2E, the same accepted `Run` creates two owner Conversations, atomically refreshes declaration session IDs, verifies Web/App/Mobile instance-scoped Prompt appends through Runtime history reads, verifies TUI instance filtering plus Prompt append, and verifies CLI instance listing plus aggregate-version CAS Prompt append. Default and missing-image routes remain 404; no client registration, scheduling, dispatch, Runner execution, or Audit path was enabled.
- **DONE — Activation-gated registry placement preflight (cross-device plan §462)**: The accepted Fabric assembly optionally mounts owner-scoped `POST /api/v1/device-placement/registry-preview` beside the v1/v2 inventory and five-client views. It evaluates requirements against the freshly read lossless lifecycle registry with a Coordinator-clock sample, preserves candidate counters and exclusion reasons, and keeps selected target IDs null with all authority false. The actual production `Run` path is also read through the authenticated Runtime remote CLI when configured; the normal constructor and blocked/missing activation remain 404 or startup failures. No reservation, lease, scheduling mutation, dispatch, Runner, Run, receipt, or Audit path was enabled. ADR-0039 remains planning-only, ADR-0114 remains Proposed/null, and P4 remains gated.
- **DONE — Activation-gated lifecycle registry read (cross-device plan §463)**: Accepted Fabric now mounts owner-scoped `GET /api/v1/device-enrollment-heartbeat/lifecycle-registry` from the private lifecycle image. It strictly re-decodes and owner-checks the full lifecycle registry on every read; `PUT` and all enrollment/heartbeat/approval/credential mutation paths remain 404 or unmounted. The configured production `Run` is read through Runtime CLI `remote lifecycle-registry show` and the explicit TUI command, with all authority flags false. The ordinary constructor and blocked/missing activation remain closed; no registration, heartbeat write, credential issuance, reservation, scheduling, dispatch, Runner, Run, receipt, or Audit authority was added. ADR-0039 remains planning-only and ADR-0114 remains Proposed/null.
- **DONE — Accepted lifecycle registry read through Flutter Console (cross-device plan §464)**: Snaplink Console's shared Web/App/Mobile API now reads the owner-scoped lifecycle registry from the Accepted Fabric route with the same strict schema, owner, sorted device/Runner join, revision, heartbeat, capability, and display-only checks used by the candidate Gate. The production E2E reaches the real Forge Server with a Snaplink JWT and verifies both device rows and online/offline liveness. The ordinary constructor stays request-free; no enrollment, heartbeat write, credential issuance, selection, reservation, scheduling, dispatch, Runner, Run, receipt, or Audit authority was added. ADR-0039 remains planning-only and ADR-0114 remains Proposed/null.
- **DONE — Accepted registry placement preflight through Flutter Console (cross-device plan §465)**: Snaplink Console's shared Web/App/Mobile API now posts explicit requirements to the Accepted Fabric registry placement route and strictly validates owner, schema, evaluation mode, sorted device/Runner decisions, safe evaluation time, unverified markers, null selected target, and all-false authority. The production E2E reaches the real Forge Server with a Snaplink JWT and verifies both lifecycle candidates. This remains deterministic P3a comparison only: no lease, reservation, selection, scheduling mutation, dispatch, Runner, execution, Run, receipt, or Audit authority was added. ADR-0039 remains planning-only and ADR-0114 remains Proposed/null.
- **DONE — Accepted inventory resource perception through Flutter Console (cross-device plan §466)**: The shared Web/App/Mobile Sessions surface now reads both owner-scoped inventory projections from the Accepted Fabric assembly. The production E2E feeds the authenticated v1 response into the user-facing resource panel and consumes lossless v2 through the same API client, retaining device/Runner IDs, revision, generation, heartbeat sequence, and GPU shape while checking the dynamic Coordinator observation time. Both reads remain unverified/display-only with all authority false; no enrollment, heartbeat write, credential, selection, reservation, lease, dispatch, Runner, execution, Run, receipt, Audit, or default route authority was added. ADR-0039 remains planning-only and ADR-0114 remains Proposed/null.
- **DONE — Fail-closed production assembly for the OBSERVE stage (cross-device plan §467)**: Forge Server's device-fabric route assembler now accepts only `INVENTORY` and `OBSERVE`. Accepted OBSERVE mounts owner-scoped lifecycle, v1/v2 inventory, registry placement-preflight, and optional client-instance read projections; unsupported EXECUTE/MIGRATE/FEDERATE modes fail startup/assembly until dedicated routes exist. The pure staged gate remains unchanged and no enrollment, heartbeat write, credential, selection, reservation, lease, dispatch, Runner, execution, Run, receipt, Audit, or default route authority was added. ADR-0039 remains planning-only and ADR-0114 remains Proposed/null.
- **DONE — Accepted OBSERVE production Run coverage (cross-device plan §468)**: The real Forge Server `Run` path now starts an explicitly accepted OBSERVE request and serves authenticated owner-scoped lifecycle, v1/v2 inventory, registry placement-preflight, and optional five-client session/resource reads while rejecting lifecycle mutation. Owner binding, lossless observations, null selected targets, and all-false authority are covered by the production test. The fixture remains synthetic and default-off; no live enrollment, heartbeat, credentials, reservation, lease, dispatch, Runner, execution, Run, receipt, or Audit authority was added, and ADR-0039/0114/P3b/P4 remain gated.
- **DONE — Accepted EXECUTE admission assembly (cross-device plan §469)**: Forge Server now accepts a complete synthetic `EXECUTE + P4` gate and mounts the owner-scoped observation projections, consent/pending Run-intent/revocation handlers, and pure Attempt/lease, Runner-receipt, dispatch-plan, and reconciliation preflight projections through the authenticated production `Run` boundary. The assembly records explicit owner/CAS/idempotent intent and comparison packets only; it still does not select or reserve a device, issue a lease, create a Run, dispatch a Runner, execute work, or publish Audit. `OFF`/`INVENTORY`/`OBSERVE` behavior remains fail-closed/read-only, `MIGRATE`/`FEDERATE` remain unassembled, and ADR-0039/0114/P3b/live remote execution remain gated.
- **DONE — Runner transport admission verifier (cross-device plan §470)**: Forge Core adds a pure D3 HMAC envelope verifier matching the ecosystem Python Runner byte-for-byte. It validates canonical JSON payload bytes, method/path binding, bounded timestamp skew, lower-case signatures, bounded nonce replay, and fixed all-false authority metadata; malformed or stale requests fail closed and bad signatures do not consume a nonce. No secret store, registration, heartbeat, lease, selection, reservation, dispatch, execution, Audit, or production Runner route is mounted; the verifier is transport groundwork for a separately accepted EXECUTE/P4 adapter.
- **DONE — EXECUTE scheduler selection preview (cross-device plan §471)**: The accepted `EXECUTE + P4` production assembly now exposes an owner-bound scheduler preview over the lossless v2 inventory. It deterministically declares the first eligible `(device_id, instance_id)` and preserves a no-candidate reason when all observations are stale, reserved, or otherwise ineligible; `preview_only` remains true and placement, reservation, lease, execution, dispatch, and Audit authority remain false. The route reads no durable Run/Attempt/lease/registry state, opens no Runner transport, and remains 404 for default/OFF/INVENTORY/OBSERVE while MIGRATE/FEDERATE stay unassembled. A future adapter must revalidate the unverified observation and acquire a fenced lease before dispatch; ADR-0039 remains planning-only, ADR-0114 remains Proposed/null, and live P4 execution remains gated.
- **DONE — Runtime CLI/TUI scheduler preview client (cross-device plan §472)**: Forge Runtime adds `remote placement scheduler-preview --input FILE|-` and the TUI's `scheduler-selection-preview --input FILE`. The strict client validates Conversation/Run/Attempt bindings, owner and identifiers, selection reason/IDs, safe counters, and all-false authority; the one-shot POST is never retried and authorization failures clear the local TUI owner view. It only renders the accepted EXECUTE comparison and never creates a Run, lease, reservation, Runner transport, or dispatch effect; server default/OFF/INVENTORY/OBSERVE closure remains unchanged.
- **DONE — Scheduler-selection preview receiver parity (cross-device plan §473)**: The canonical `forge.scheduler-selection-preview/v1` response is mirrored and strictly decoded by Aero-ID, Aero-IM's audit connector, Aero-Vault's governance relay, and Snaplink Audit Governance. Each receiver enforces owner and Conversation/Run/Attempt bindings, safe counters, selected-pair/reason consistency, `preview_only=true`, and all-false placement/reservation/lease/execution/dispatch/Audit authority while rejecting unknown, duplicate, missing, trailing, unsafe, selected-pair, and authority mutations. This is interoperability evidence only: no Audit fact, lease, reservation, Runner contact, dispatch, or production route authority is enabled; the accepted `EXECUTE + P4` gate remains unchanged.
- **DONE — Console Web/App/Mobile scheduler-selection preview (cross-device plan §474)**: Snaplink Console's shared Flutter Sessions surface now posts the exact Conversation/Run/Attempt scheduler-preview request through an explicit candidate reader and strictly re-decodes the `forge.scheduler-selection-preview/v1` response before showing the deterministic selected pair or no-candidate reason. The default Gate remains request-free; only the explicit candidate flag/origin opens the one-shot POST, and the panel has no selection, reservation, lease, or dispatch action. No Run/Attempt/lease state, Runner contact, or Audit effect was added; the accepted `EXECUTE + P4` gate remains unchanged.
- **DONE — Accepted EXECUTE scheduler preview through Console clients (cross-device plan §475)**: The accepted `EXECUTE + P4` JWT production test now drives independent Web, desktop App, and Mobile Console API adapters against the real scheduler-preview route. Each submits the exact Conversation/Run/Attempt and requirements once and strictly validates the owner-bound no-candidate observation, counters, binding, and all-false authority. The fixture is intentionally stale, so no lease, reservation, Runner, dispatch, or Audit effect is possible; default Gate and non-EXECUTE routes remain closed.
- **DONE — EXECUTE fenced scheduler lease claim (cross-device plan §476)**: Forge Core adds an optional private lease-registry file and an `EXECUTE + P4`-only scheduler-lease route. The strict file CAS records owner/Conversation/Run/Attempt, candidate observation counters, fenced epoch/token, and exact idempotency replay; Runtime CLI/TUI and Console validate the same receipt and send the effectful POST once. Placement, reservation, and lease issuance are explicit while execution authorization, Runner dispatch, and Audit remain closed. Existing v2 observations expose unknown policy attributes, so the accepted route currently fails closed with `no_eligible_target` until a policy-complete source is separately approved; default/OFF/INVENTORY/OBSERVE/MIGRATE/FEDERATE remain closed.
- **DONE — Cross-ecosystem execution lease registry receipt parity (cross-device plan §477)**: The canonical `forge.execution-lease-registry/v1` receipt is mirrored byte-for-byte and strictly consumed by Aero-ID, Aero-IM, Aero-Vault, and Snaplink Audit Governance. Receiver tests enforce owner and Conversation/Run/Attempt bindings, safe inventory/heartbeat counters, grant epoch/token/window, and the authority split of placement/reservation/lease true with execution/dispatch/Audit false; unknown, duplicate, trailing, binding, and authority mutations fail closed. This remains interoperability evidence only; no receiver issues or renews a lease, reserves a target, contacts a Runner, dispatches work, or publishes Audit, and the policy-complete inventory requirement remains in force.
- **DONE — EXECUTE policy-complete scheduler lease source (cross-device plan §478)**: Forge Core accepts an optional strict 0600 owner-private `forge.device-placement-policy-registry/v1` image beside the lifecycle and lease registries. Each policy row binds device/Runner identity plus revision, generation, and heartbeat sequence and supplies only residency, trust, sandbox, and concurrency values missing from the lifecycle observation. With both explicit files under the accepted `EXECUTE + P4` gate, the scheduler lease route claims and idempotently replays the first eligible fenced lease; without the policy image it remains `no_eligible_target`. Placement, reservation, and lease issuance are enabled only at this boundary; execution authorization, Runner dispatch, command execution, renewal, and Audit publication remain closed.
- **DONE — Console Web/App/Mobile explicit scheduler lease candidate (cross-device plan §479)**: Snaplink Console's shared Sessions Gate now exposes the fenced scheduler-lease API only when the caller supplies the exact owner-bound request, candidate origin, fixed idempotency key, and explicit opt-in flag. It performs one authenticated POST, strictly re-decodes the bound receipt, and renders target/counter/epoch/window metadata while withholding the fencing token. The default Gate remains request-free and the screen does not retry or refresh a claim; execution authorization, Runner dispatch, and Audit publication remain false.
- **DONE — Accepted EXECUTE scheduler lease through the real multi-client boundary (cross-device plan §480)**: The production Forge `Run` path now claims and idempotently replays a policy-complete fenced lease through the real Snaplink JWT route. Opt-in Console Web/App/Mobile clients consume the same receipt, while Runtime CLI and TUI claim subsequent eligible targets with explicit/fresh keys and keep fencing material out of human output. Route allowlisting and Runtime key validation are covered. Placement, reservation, and lease issuance are proven across Core/Runtime/Console; Run/Attempt creation, command authorization, Runner dispatch, execution, renewal, and Audit remain gated by the accepted EXECUTE+P4 candidate.
- **DONE — Cross-client scheduler lease canonical replay (cross-device plan §481)**: The authenticated scheduler-lease route now hashes the strict decoded request projection, so raw Core HTTP, Console Web/App/Mobile, and Runtime Rust can reuse one idempotency key even when JSON member order differs; duplicate, unknown, trailing, and malformed fields still fail before hashing. The production E2E proves the common-key replay and then claims the next eligible target from TUI with a fresh key. Placement, reservation, and lease issuance remain the only enabled authority; Run/Attempt creation, command authorization, Runner transport, execution, renewal, release, and Audit remain gated.
- **DONE — Fenced scheduler lease renewal through Core and Runtime (cross-device plan §482)**: The accepted `EXECUTE + P4` lease registry now validates an active target proof, appends the next fencing epoch with a server-issued token, and replays the replacement for the same renewal key while rejecting expired, stale, foreign, malformed, and conflicting proofs. Runtime CLI/TUI and the Console API consume the renewal route; focused Go/Rust/Flutter coverage and the real Runtime CLI E2E pass. Placement, reservation, and lease authority remain the only enabled effect; command authorization, Runner transport, execution, release, and Audit remain gated.
- **DONE — Console Web/App/Mobile explicit scheduler lease renewal candidate (cross-device plan §483)**: The shared Snaplink Console Sessions Gate now exposes the authenticated scheduler-lease renewal through a separate explicit candidate. It binds the current Conversation/Run/Attempt/target proof, origin, fresh idempotency key, and returned epoch, sends one POST without replay, and keeps the fencing token out of the shared lease panel. The default Gate remains request-free; if claim and renewal declarations coexist, renewal takes precedence. Placement, reservation, and lease authority remain the only enabled effect; command authorization, Runner transport, execution, release, and Audit remain gated.
- **DONE — Fenced scheduler lease release through Core, Runtime, and Console (cross-device plan §484)**: The accepted `EXECUTE + P4` registry now exposes an owner-authenticated release POST. Core marks the exact active proof inactive with atomic CAS, preserves its epoch for stale-runner fencing, replays exact idempotency keys without another write, and rejects stale, expired, foreign, malformed, and cross-operation key conflicts. Runtime CLI/TUI and Snaplink Console Web/App/Mobile use strict FILE/request/response adapters and keep fencing material out of human output. The default Console Gate remains request-free; no Run/Attempt, command authorization, Runner dispatch, execution, or Audit authority was added.
- **DONE — Durable lease-bound Runner dispatch admission preview (cross-device plan §485)**: Accepted `EXECUTE + P4` now rechecks the owner-private fenced lease registry at an explicit Conversation/Run/Attempt admission-preview route. Core recomputes the direct-argv command digest and returns only current/active lease, Attempt-state, binding, rejection, and all-false authority metadata; released, stale, missing, foreign, malformed, and confused proofs fail closed, and fencing token/argv/workspace/output stay out of the response. Runtime CLI/TUI and Snaplink Console Web/App/Mobile send one strict authenticated POST and reject duplicate/unknown fields, digest/path drift, and authority mutation; TUI clears the local owner view after authorization failure. This is still a read-only admission recheck: no Run/Attempt creation, command authorization, reservation, Runner contact/dispatch, execution, lease renewal/release, or Audit publication was added; ADR-0039 remains planning-only, ADR-0114 remains Proposed/null, and P4 remains the separate execution gate.
- **DONE — Console Gate Runner dispatch admission candidate (cross-device plan §486)**: The shared Snaplink Web/App/Mobile Sessions Gate now accepts an explicit owner/Conversation/Run/Attempt admission request and reader, performs one authenticated metadata-only POST, strictly re-decodes the response, rejects binding/digest/authority drift, and retains only the last validated preview during refresh. The default Gate remains request-free; the selected client-instance projection cannot display hidden Conversations. The card exposes lease/Attempt predicates and reasons while withholding fencing token, argv, workspace, and execution actions. No Run/Attempt, command authorization, Runner dispatch, execution, lease renewal/release, or Audit authority was added; ADR-0039 remains planning-only and ADR-0114 remains Proposed/null.
- **DONE — Accepted EXECUTE Runner dispatch admission across clients (cross-device plan §487)**: The accepted production `Run` harness now creates one owner-private fenced lease, posts the exact command proof through the authenticated Runner dispatch-admission preview route, consumes the same metadata-only response with Runtime CLI/TUI when configured and the opt-in Flutter Web/App/Mobile API reader, then releases the lease and verifies the proof is read as inactive with a deterministic rejection. No Run/Attempt, command authorization, Runner transport/dispatch, execution, renewal, or Audit authority was added; fencing token, argv, workspace, and output remain withheld, and ADR-0039/ADR-0114/P4 remain gated.
- **DONE — Cross-client Runner transport admission preview contract (cross-device plan §488)**: Forge Core joins a verified D3 transport observation to fenced command/lease admission without network or execution side effects. Runtime Rust and Snaplink Console Web/App/Mobile strictly consume the canonical metadata-only fixture; Core tests sign/verify binding and path/payload plus lease rejection cases. Authority remains all false and no Runner socket, payload send, command authorization, Run/Attempt creation, lease mutation, execution, or Audit publication was added; ADR-0039/ADR-0114/P4 remain gated.
- **DONE — Cross-ecosystem Runner transport admission receiver parity (cross-device plan §489)**: Aero-ID, Aero-IM, Aero-Vault, and Snaplink Audit Governance now mirror and strictly decode the canonical transport-admission fixture. Their receiver tests reject unknown/duplicate/trailing fields, authority mutations, path confusion, malformed digests, and readiness drift while keeping the observation display-only. No Runner transport, payload send, lease mutation, command authorization, Run/Attempt creation, execution, or Audit publication was added; ADR-0039/ADR-0114/P4 remain gated.
- **DONE — Authenticated Runner transport admission preview boundary (cross-device plan §490)**: Accepted `EXECUTE + P4` Forge Server assembly now mounts an owner-scoped transport-admission preview POST beside dispatch admission. It re-decodes the caller's already verified D3 observation, re-reads the durable fenced lease, checks the supplied lease metadata against the persisted grant, and returns the canonical metadata-only value with Coordinator-clock evaluation. No device secret verification, Runner socket, payload send, command authorization, Run/Attempt mutation, lease mutation, execution, or Audit publication was added; default/OFF/INVENTORY/OBSERVE and ADR-0039/ADR-0114/P4 gates remain closed.
- **DONE — Runtime and Console transport admission clients (cross-device plan §491)**: Forge Runtime CLI/TUI now expose explicit `runner-transport-admission-preview` and selected-session remote-preview commands with strict owner/Conversation/Run/Attempt, command/lease/transport binding, rejection-order, and all-false authority checks before one authenticated POST; TUI authorization failure clears the owner view and human output omits proof material. Snaplink Console's shared Web/App/Mobile API adds the same origin-pinned request/response adapter with no unauthorized retry. No Gate startup request, device-secret verification, Runner socket, payload send, command authorization, Run/Attempt mutation, lease mutation, execution, or Audit publication was added; ADR-0039/ADR-0114/P4 remain gated.
- **DONE — Console Sessions Gate transport admission candidate (cross-device plan §492)**: The shared Snaplink Web/App/Mobile Sessions Gate now accepts an explicit owner-bound transport-admission request and reader, performs one authenticated metadata-only POST, strictly re-decodes owner/Attempt/command/target/epoch/path/payload bindings and all-false authority, and renders bounded transport metadata while withholding proof material. The default Gate remains request-free and hidden client-instance Conversations cannot display the preview. No device-secret verification, Runner socket, payload send, command authorization, Run/Attempt mutation, lease mutation, execution, or Audit publication was added; ADR-0039/ADR-0114/P4 remain gated.
- **DONE — Accepted EXECUTE transport admission across clients (cross-device plan §493)**: The accepted production `Run` harness now supplies a D3-signed-and-verified transport observation with one fenced lease to the authenticated route and consumes the same display-only response through Core, Runtime CLI/TUI when configured, and the opt-in Console Web/App/Mobile API harness. After lease release, the exact request returns an inactive admission with the deterministic rejection; proof and payload material remain withheld. No Runner socket, payload send, command authorization, Run/Attempt mutation, lease mutation through admission, execution, or Audit publication was added; ADR-0039/ADR-0114/P4 remain gated.
- **DONE — P4 and independent Runner execution boundary (cross-device plan §494)**: Forge Core now defines a pure `forge.runner-execution-boundary/v1` join over dispatch/transport admission, accepted `EXECUTE + P4`, and a separate Runner authority decision. The zero-value authority is disabled and an enabled decision must have distinct acceptance metadata; lease fencing, cancellation/uncertain-work, Vault artifact authorization, and Audit outbox evidence remain required. Active cancellation and `uncertain` effects fail closed, while only `not_started` or `reconciled` effects are startable. The result is always preview-only with all authority false and no token/argv/workspace/payload. App-server configuration rejects the authority declaration unless the complete gate is accepted; no live Runner route or effect was added, and ADR-0039/ADR-0114/P4 remain gated.
- **DONE — Authenticated Runner execution-boundary preview (cross-device plan §495)**: Accepted `EXECUTE + P4` assembly can optionally mount an owner-authenticated `runner-execution-boundary/preview` POST only when the server supplies a separate accepted Runner authority decision and the fenced lease registry. The server owns those gate values, re-reads the lease, recomputes dispatch/transport admission, and joins explicit cancellation/effect controls; release, cancellation, non-startable, and uncertain states remain non-ready. The ordinary constructor and authority-free assembly stay 404, and the response remains preview-only with all authority false and no fencing token/argv/workspace/payload. No Runner connection, payload send, command authorization, Run/Attempt mutation, lease mutation, execution, or Audit publication was added; Runtime/Console clients remain the next slice and ADR-0039/ADR-0114/P4 remain gated.
- **DONE — Runtime and Console execution-boundary clients (cross-device plan §496)**: Forge Runtime CLI/TUI now expose the authenticated execution-boundary preview with strict owner/path/Attempt/command/transport/effect validation, one POST, no unauthorized retry, and redacted output; Snaplink Console Web/App/Mobile has the matching origin-pinned API adapter and strict display-only model. Activation, authority, lease state, and evaluated time remain server-owned. No Gate startup request or live Runner connection, payload send, command authorization, Run/Attempt or lease mutation, execution, or Audit publication was added; ADR-0039/ADR-0114/P4 remain gated.
- **DONE — Console Sessions Gate execution-boundary candidate (cross-device plan §497)**: The shared Snaplink Web/App/Mobile Sessions Gate now accepts an explicit owner-bound execution-boundary request and reader for the selected Conversation/Run, pins the request and candidate origin, performs one authenticated preview POST, strictly re-decodes bindings and all-false authority, retains the last validated display-only observation, and renders bounded gate metadata without proof material. The default Gate remains request-free and hidden client-instance Conversations cannot display it. No Runner connection, payload send, command authorization, Run/Attempt or lease mutation, execution, or Audit publication was added; ADR-0039/ADR-0114/P4 remain gated.
- **DONE — Cross-ecosystem execution-boundary receiver parity (cross-device plan §498)**: The canonical `forge.runner-execution-boundary/v1` observation is mirrored byte-for-byte into Aero-ID, Aero-IM, Aero-Vault, Snaplink Audit Governance, and the Console fixture. Strict Go/Rust receivers accept only the bounded accepted preview and reject unknown/duplicate fields, authority mutation, or readiness drift. No device authentication, lease mutation, command authorization, Runner transport/dispatch, execution, Run/Attempt mutation, or Audit publication was added; ADR-0039/ADR-0114/P4 remain gated.
- **DONE — Accepted EXECUTE execution-boundary harness across clients (cross-device plan §499)**: Forge Core now creates one owner-private fenced lease, joins a verified D3 transport observation under accepted `EXECUTE + P4` and independent Runner authority test bindings, and serves the authenticated execution-boundary preview. Configured Runtime CLI/TUI and opt-in Console Flutter API tests consume the same request; after exact lease release, the repeated request is non-ready with the inactive-lease reason. Core emits an empty rejection array for ready responses. No Runner connection, payload send, command authorization, Run/Attempt or lease mutation through preview, execution, or Audit publication was added; ADR-0039/ADR-0114/P4 remain gated.
- **DONE — §500 Accepted EXECUTE reconciliation preview across clients**: The authenticated production `Run` harness now classifies one terminal-uncertainty restart image through Core and, when configured, Runtime CLI/TUI plus the Console Web/App/Mobile API. All consumers require `terminal_uncertain`, manual reconciliation, `automatic_retry=false`, exact owner/Run bindings, and all-false authority while omitting proof and terminal content. The slice remains pure observation with no retry, lease mutation, Runner contact, execution, or Audit; ADR-0039/ADR-0114/P4 remain gated.
- **DONE — §501 Accepted EXECUTE session Runner receipt observation across clients**: The real accepted `EXECUTE + P4` Run boundary now carries one completed content-free session receipt through Core and, when configured, Runtime CLI/TUI plus Console Web/App/Mobile API. All consumers preserve exact owner/Conversation/Prompt/Run and receipt bindings, `selected_target_id=null`, and all-false authority while omitting proof and payload data. The slice is an observation echo with no receipt persistence, Run/Attempt/lease mutation, target selection, Runner dispatch, retry, execution, or Audit; ADR-0039/ADR-0114/P4 remain gated.
- **DONE — §502 Accepted EXECUTE Run execution-evidence binding across clients**: The accepted `EXECUTE + P4` Run boundary now binds one content-free `RunObserved` value to the matching session Runner receipt through the authenticated `run-execution-evidence/preview` route. Core, Runtime CLI/TUI, and Console Web/App/Mobile strictly validate owner and Conversation/Prompt/Run bindings and the canonical all-false metadata projection while omitting proof, command, workspace, payload, and receipt content. This is a read-only evidence join with no Run/Attempt/lease mutation, target selection, Runner dispatch, retry, execution, or Audit; ADR-0039/ADR-0114/P4 remain gated.
- **DONE — §503 Console Sessions Gate execution-evidence candidate**: The shared Web/App/Mobile Gate now accepts explicit Run and session receipt observations, posts one authenticated `run-execution-evidence/preview` request only when the candidate is enabled, strictly re-decodes the response, and renders the metadata-only evidence card for the selected Run. The default Gate stays request-free; no evidence persistence, Run/Attempt/lease mutation, target selection, Runner dispatch, retry, execution, or Audit authority was added, and ADR-0039/ADR-0114/P4 remain gated.
- **DONE — §504 Accepted EXECUTE Run execution-evidence through the real Sessions Gate**: The Runtime-backed accepted harness now creates an owner-bound project Conversation, Prompt, and deterministic completed Run in the shared Hub, proving that the authenticated Console Sessions Gate discovers the durable session and Run through its ordinary list/read paths before issuing one opt-in evidence-preview POST. Core, Runtime CLI/TUI, Console API, and the real Flutter Gate consume the same metadata-only binding with all authority false. No evidence persistence, Attempt/lease/target mutation or selection, Runner contact/dispatch, command execution, or Audit publication was added; ADR-0039/ADR-0114/P4 remain gated.
- **DONE — §505 Accepted inventory and instance resources through the real Sessions Gate**: The accepted INVENTORY assembly now drives the authenticated Flutter Sessions Gate through the real Core HTTP boundary. Credential restoration, v1/v2 device observations, and the owner-scoped five-kind CLI/TUI/Web/App/Mobile resource view are rendered together and checked for both devices, Runner joins, and all-false authority. The candidate is explicit and opt-in; no enrollment/heartbeat write, credential, selection, reservation, lease, dispatch, Runner, execution, Run/receipt, or Audit effect was added; ADR-0039 remains planning-only, ADR-0114 remains Proposed/null, and P3b/P4 remain gated.
- **DONE — §506 Accepted scheduler lease through the real Sessions Gate**: The accepted `EXECUTE + P4` harness now seeds a Runtime-backed owner Conversation, Prompt, and completed Run for the cross-process lane, then replays the exact fenced scheduler lease through the authenticated Flutter Sessions Gate. The Gate renders target/epoch/window metadata while withholding the fencing token; placement, reservation, and lease authority remain the only enabled effects. No Run/Attempt, command authorization, Runner dispatch/execution, lease renewal/release, or Audit publication was added; ADR-0039 remains planning-only, ADR-0114 remains Proposed/null, and the live Runner/P4 effect gate remains required.
- **DONE — §507 Accepted Runner dispatch admission through the real Sessions Gate**: The accepted `EXECUTE + P4` harness now seeds a Runtime-backed owner Conversation, Prompt, and completed Run, claims one fenced lease, binds a command/Attempt proof, and drives the authenticated Flutter Sessions Gate through the durable session and Run discovery path before one `runner-dispatch-admission/preview` POST. Core, Runtime CLI/TUI, Console API, and Gate validate the same metadata-only observation; fencing token, argv, and workspace remain withheld, and releasing the lease fails closed. No command authorization, Runner contact/dispatch, payload, Run/Attempt/lease mutation through preview, execution, or Audit publication was added; ADR-0039 remains planning-only, ADR-0114 remains Proposed/null, and P4/live Runner effects remain gated.
- **DONE — §508 Accepted Runner transport admission through the real Sessions Gate**: The accepted `EXECUTE + P4` harness now seeds a Runtime-backed owner Conversation, Prompt, and completed Run, claims one fenced lease, binds a verified D3 transport observation, and drives the authenticated Flutter Sessions Gate through the durable session and Run discovery path before one `runner-transport-admission/preview` POST. Core, Runtime CLI/TUI, Console API, and Gate validate the same metadata-only observation; method/path, payload-byte, binding, and replay metadata are bounded, proof/digest/nonce/argv/workspace remain withheld, and releasing the lease fails closed. No device authentication, Runner connection/payload, command authorization, dispatch, Run/Attempt/lease mutation through preview, execution, or Audit publication was added; ADR-0039 remains planning-only, ADR-0114 remains Proposed/null, and P4/live Runner effects remain gated.
- **DONE — §509 Accepted Runner execution boundary through the real Sessions Gate**: The accepted `EXECUTE + P4` harness now seeds a Runtime-backed owner Conversation, Prompt, and completed Run, claims one fenced lease, binds a verified D3 transport observation under an independent accepted Runner authority decision, and drives the authenticated Flutter Sessions Gate through the durable session and Run discovery path before one `runner-execution-boundary/preview` POST. Core, Runtime CLI/TUI, Console API, and Gate validate the same all-false-authority observation; server-owned mode, activation, authority, admission, effect, and cancellation metadata are bounded, proof/argv/workspace/payload/output remain withheld, and releasing the lease fails closed. No Runner connection/payload, command authorization, dispatch, Run/Attempt/lease mutation through preview, execution, or Audit publication was added; ADR-0039 remains planning-only, ADR-0114 remains Proposed/null, and P4/live Runner effects remain gated.
- **DONE — §510 Accepted execution reconciliation through the real Sessions Gate**: The accepted `EXECUTE + P4` harness now seeds a Runtime-backed owner Conversation, Prompt, and completed Run, then drives the authenticated Flutter Sessions Gate through durable Run discovery before one caller-bound terminal-uncertainty `execution-reconciliation/preview` POST. Core, Runtime CLI/TUI, Console API, and Gate validate the same metadata-only classification (`terminal_uncertain`, manual reconciliation required, automatic retry false) while proof, terminal reason, receipt digest, argv, workspace, and output remain withheld. No retry, Run/Attempt/lease mutation, target selection/reservation, Runner contact/dispatch, execution, or Audit publication was added; ADR-0039 remains planning-only, ADR-0114 remains Proposed/null, and P4/live Runner effects remain gated.
- **DONE — §511 Accepted session Runner receipt through the real Sessions Gate**: The accepted `EXECUTE + P4` harness now seeds a Runtime-backed owner Conversation, Prompt, and completed Run, then drives the authenticated Flutter Sessions Gate through durable Run discovery with the same owner/Conversation/Prompt/Run-bound content-free receipt observation. Core, Runtime CLI/TUI, Console API, and Gate render completed disposition, receipt validity, selected-target absence, and all-false authority metadata while event payload, output, fencing token, and execution actions remain absent. No receipt persistence, target selection/reservation, Run/Attempt/lease mutation, Runner contact/dispatch, retry, execution, or Audit publication was added; ADR-0039 remains planning-only, ADR-0114 remains Proposed/null, and P4/live Runner effects remain gated.
- **DONE — §512 Accepted pending Run-intent through the real Sessions Gate**: The accepted `EXECUTE + P4` harness now grants owner-bound project execution consent, drives the authenticated Flutter Sessions Gate through the consent preview and durable Conversation/Prompt/Run reads, and submits one explicit scheduling-review POST through the inert `/run-intents` surface. The Gate renders a pending/created receipt with all execution flags false; Core reads the pending intent and Prompt back from Rust Hub and verifies the existing completed Run remains unchanged. No ordinary Run, target selection/reservation, lease mutation, Runner contact/dispatch, retry, execution, or Audit publication was added; ADR-0039 remains planning-only, ADR-0114 remains Proposed/null, and P4/live Runner effects remain gated.
- **DONE — §513 Accepted pending Run-intent readback through the real Sessions Gate**: After the scheduling-review POST succeeds, the authenticated Flutter Web/App/Mobile Sessions Gate refreshes the explicit owner-scoped pending Run-intent reader and renders the server-owned metadata projection plus payload-free timeline. The cross-process harness verifies aggregate-version advancement, pending/created receipt metadata, metadata-only authority, the `submitted` timeline marker, and no Prompt content inside the metadata card; Core/Rust readers observe the same pending intent and unchanged durable Run set. The refresh creates no ordinary Run, device selection/reservation, lease mutation, Runner contact/dispatch, retry, execution, or Audit; ADR-0039/ADR-0114/P4 remain gated.
- **DONE — §514 Accepted pending Run-intent candidate read transport through the Sessions Gate**: The opt-in pending Run-intent candidate now creates authenticated list, cursor-page, and payload-free timeline readers from the same owner/origin API adapter as the scheduling-review POST. Explicit readers retain precedence and the default Gate remains request-free; the real cross-process harness enables only the candidate flag and owner/origin and verifies that the Gate itself reads the server-created receipt and `submitted` timeline marker. No ordinary Run, device selection/reservation, lease mutation, Runner contact/dispatch, retry, execution, or Audit was added; ADR-0039/ADR-0114/P4 remain gated.
- **DONE — §515 External pending Run-intent convergence through the Sessions Gate**: An isolated Runtime CLI client now submits a second pending Run-intent after the Console Gate's review, and a fresh authenticated read-only Sessions Gate discovers that external receipt through its own candidate list/timeline adapter. Core verifies both pending receipts, the external Prompt, unchanged durable Runs, payload-free metadata/timeline, and no local Gate POST. No ordinary Run, target selection/reservation, lease mutation, Runner contact/dispatch, retry, execution, or Audit was added; ADR-0039/ADR-0114/P4 remain gated.
- **DONE — §516 Live pending Run-intent convergence through the Sessions Gate**: An open Flutter Sessions Gate now consumes the owner change feed after an isolated Runtime CLI process submits another pending Run-intent, refreshes external Prompt and metadata, and expands the matching payload-free timeline without a local POST. Core verifies the additional receipt/Prompt and unchanged durable Runs; scheduled refresh remains inert and ADR-0039/ADR-0114/P4 remain gated.
- **DONE — §517 Live pending Run-intent convergence through Runtime TUI**: An authenticated PTY-held Rust Runtime TUI opens a session and reads its pending Run-intent page while an independent Runtime CLI submits another pending intent. The already-open TUI consumes the owner change feed, refreshes metadata, lists the external receipt, and expands its payload-free timeline; request assertions prove only the external client writes. No ordinary Run, target selection/reservation, lease mutation, Runner contact/dispatch, retry, execution, or Audit was added; ADR-0039/ADR-0114/P4 remain gated.
- **DONE — §518 Live inventory refresh through Runtime TUI**: The accepted inventory activation harness keeps an authenticated Runtime TUI open on the v2 owner-scoped inventory, atomically replaces the same-owner lifecycle observation image with the next revision, and proves that TUI `sync` renders the new revision/heartbeat without restart or scope drift. The replacement remains a test-side observation image; no heartbeat listener, enrollment, credential, inventory authority, selection, reservation, scheduling, dispatch, Runner, execution, or Audit effect was added; ADR-0039/ADR-0114/P4 remain gated.
- **DONE — §519 Live inventory refresh through the Console Gate**: The accepted inventory activation harness keeps the authenticated Web/App/Mobile Sessions Gate mounted after Runtime TUI advances the same owner-scoped lifecycle image, atomically replaces the private image with the next revision at `0600`, and proves the Gate's bounded owner poll renders the new v2 revision/heartbeat without a local write or restart. The replacement remains a test-side observation image; no heartbeat listener, enrollment, credential, inventory authority, selection, reservation, scheduling, dispatch, Runner, execution, or Audit effect was added; ADR-0039/ADR-0114/P4 remain gated.
- **DONE — §520 Live client-instance/resource refresh through the Console Gate**: The accepted Gate harness exercises the composed owner-scoped client-instance/resource reader, atomically replaces the private client-instance and lifecycle images at `0600` while Web/App/Mobile remains mounted, and proves the bounded owner poll renders a new instance row plus the matching device revision/heartbeat without a local write or restart. The images remain test-side, display-only observations; no instance registration, Prompt authority, heartbeat listener, enrollment, credential, selection, reservation, scheduling, Runner, execution, or Audit effect was added; ADR-0039/ADR-0114/P4 remain gated.
- **DONE — §521 Live client-instance/session refresh through the Console Gate**: The accepted Gate harness enables the dedicated owner-scoped session reader beside the composed resource reader, renders the five CLI/TUI/Web/App/Mobile instance rows with their session IDs, and keeps the v1/v2 inventory panels in the same authenticated Web/App/Mobile view. It atomically replaces the private client-instance image while the Gate remains mounted and proves the bounded owner poll replaces the session rows with the new observed instance image without a local POST or restart. The session map remains a test-side read-only declaration; no instance registration, Prompt/device authority, heartbeat, enrollment, credential, target selection/reservation, scheduling, Runner dispatch, execution, or Audit publication was added; ADR-0039/ADR-0114/P4 remain gated.
- **DONE — §522 Live client-instance/session and resource refresh through Runtime TUI**: The accepted inventory activation harness keeps one authenticated Runtime TUI open with both owner-scoped client-instance/session and composed client-instance/resource views selected, atomically replaces the private five-client declaration image, and proves explicit TUI `sync` renders the refreshed CLI/TUI/Web/App/Mobile rows plus the joined device resource without restart or owner-scope drift. The declaration remains a test-side observation; no instance registration, Prompt/device authority, heartbeat, enrollment, credential, target selection/reservation, scheduling, Runner dispatch, execution, or Audit publication was added; ADR-0039/ADR-0114/P4 remain gated.
- **DONE — §523 Fresh client-instance/session and resource reads through Runtime CLI**: After the accepted TUI refresh, the harness atomically writes a third owner-scoped five-client declaration image and starts fresh authenticated Runtime CLI processes for the dedicated session and composed resource views. Both observe the new CLI/TUI/Web/App/Mobile rows and joined device row without reusing a prior image or broadening scope; the declaration remains display-only and no registration, Prompt/device authority, heartbeat, enrollment, credential, target selection/reservation, scheduling, Runner dispatch, execution, or Audit publication was added. ADR-0039/ADR-0114/P4 remain gated.
- **DONE — §524 Prompt write after live client-instance refresh through Runtime TUI**: The accepted production harness keeps an authenticated Runtime TUI open on an owner-scoped client-instance/session view, opens the Conversation declared by its TUI instance, atomically replaces the declaration with a new instance image, and runs `sync` in the same process. The TUI reselects the refreshed instance, reopens its declared Conversation, and appends a Prompt; an independent authenticated Runtime read verifies the Prompt in shared history. The declaration remains test-side display-only metadata and the Prompt uses the ordinary owner/CAS/idempotency boundary; no instance registration, heartbeat, enrollment, credential, target selection/reservation, scheduling, Runner dispatch, execution, or Audit publication was added. ADR-0039/ADR-0114/P4 remain gated.
- **DONE — §525 Prompt write after live client-instance refresh through the Console Gate**: The accepted Console Gate harness keeps the authenticated Web/App/Mobile surface mounted while it selects the old TUI instance, atomically replaces the owner-scoped client-instance declaration and lifecycle image, waits for bounded change-sync, and reselects the new TUI instance. It opens the same owner Conversation and appends a Prompt through the normal Console write path; an independent authenticated Runtime CLI read verifies the Prompt in shared history. The declaration remains test-side display-only metadata and the Prompt uses the ordinary owner/CAS/idempotency boundary; no instance registration, Prompt authority, heartbeat, enrollment, credential, target selection/reservation, scheduling, Runner dispatch, execution, or Audit publication was added. ADR-0039/ADR-0114/P4 remain gated.
- **DONE — §526 Prompt write after fresh client-instance refresh through Runtime CLI**: The accepted production harness writes a fourth owner-scoped client-instance declaration image and starts a fresh authenticated Runtime CLI for `client-cli-live-004`. The CLI reads the filtered session page, obtains the aggregate version, appends a Prompt with the instance binding and idempotency key, and independently reads shared Conversation history to verify the stored Prompt. The declaration remains test-side display-only metadata and the Prompt uses the ordinary owner/CAS/idempotency boundary; no instance registration, Prompt authority, heartbeat, enrollment, credential, target selection/reservation, scheduling, Runner dispatch, execution, or Audit publication was added. ADR-0039/ADR-0114/P4 remain gated.
- **DONE — §527 External CLI Prompt convergence through Runtime TUI**: The accepted production harness holds an authenticated Runtime TUI on `client-tui-live-005` while a fresh Runtime CLI reads the filtered `client-cli-live-005` session page, obtains the aggregate version, and appends a Prompt through the ordinary owner/CAS/idempotency boundary. The held TUI consumes the owner change feed, refreshes Prompt history, and renders the external Prompt after sync. The client-instance image remains test-side display-only metadata; no registration/enrollment, heartbeat or inventory authority, credential, target selection/reservation, scheduling, Runner dispatch, execution, or Audit publication was added. ADR-0039/ADR-0114/P4 remain gated.
- **DONE — §528 External CLI Prompt convergence through the Console Gate**: The accepted production harness writes a refreshed five-client declaration, opens the real Web/App/Mobile Sessions Gate on the Web instance, and has an independent Runtime CLI discover the owner-scoped session version and append a Prompt with its own instance binding. The mounted Gate consumes the owner change feed and renders the external Prompt in the selected session without a local Prompt POST. The instance declaration remains a test-side display-only observation; no registration/enrollment, heartbeat or inventory authority, credential, target selection/reservation, scheduling, Runner dispatch, execution, or Audit publication was added. ADR-0039/ADR-0114/P4 remain gated.
- **DONE — §529 Authenticated Runner execution-intent preview parity**: Forge Core now exposes an explicitly injected owner/path-bound `runner-execution-intent/preview` candidate, and Runtime CLI/TUI plus Snaplink Console Web/App/Mobile can post and strictly validate the same Prompt/Run/Attempt/Command binding. Responses are metadata-only with no selected target and all authority flags false; unknown/duplicate fields, owner/path drift, origin drift, response drift, and unauthorized POST replay fail closed. This is a handoff contract only: no command persistence, lease verification, device selection, reservation, scheduling, Runner transport, execution, or Audit publication was added; ADR-0039/ADR-0114/P4 remain gated.
- **DONE — §530 Accepted Runner execution-intent preview across clients**: The accepted `EXECUTE + P4` production assembly now carries one owner-bound Prompt/Run/Attempt/Command binding through the real Core HTTP route, Runtime CLI/TUI, and the opt-in Console Web/App/Mobile API adapter. The cross-client harness recomputes the same command digest and consumes the identical preview while fencing material, argv, workspace, target selection, reservation, command persistence, Runner transport, execution, and Audit remain absent; ADR-0039/ADR-0114/P4 remain gated.
- **DONE — §531 Accepted Runner execution-intent preview through the real Console Gate**: The authenticated Web/App/Mobile Sessions Gate now has a cross-process E2E path that discovers a Runtime-backed Conversation and Run, posts the explicit Runner intent candidate once, and renders the same owner-bound metadata card. Console's strict request decoder is shared by the Gate and API adapter; duplicate/unknown/binding/authority drift remains rejected, and fencing material, argv, workspace, device selection, reservation, Runner transport, execution, and Audit remain absent; ADR-0039/ADR-0114/P4 remain gated.
- **DONE — §534 Cross-ecosystem Runner execution-intent request receiver parity**: The canonical six-field request fixture is now mirrored into Aero-ID, Aero-IM, Aero-Vault, and Snaplink Audit Governance. Each receiver strictly rejects unknown, duplicate, and trailing fields, checks owner/Conversation/Prompt/Run/Attempt/Command identities, digest/idempotency equality, lease-proof binding, and null selected target, with mutation tests failing closed. This remains an authority-free handoff contract: no command persistence, lease/device authority, reservation, scheduling, Runner transport, execution, receipt, or Audit publication was added; ADR-0039/ADR-0114/P4 remain gated.
- **DONE — §535 Cross-ecosystem direct-argv digest recomputation**: Aero-ID, Aero-Vault, Snaplink Audit Governance, and Aero-IM now recompute the domain-separated `forge.runtime.runner-command.v1` SHA-256 over the strict direct-argv command bytes while consuming the canonical request. A canonical digest assertion and digest-drift mutations prove the receiver cannot accept a command whose repeated digest only matches a fixed label. This remains offline, authority-free validation with no command persistence, lease/device authority, reservation, scheduling, Runner transport, execution, receipt, or Audit publication; ADR-0039/ADR-0114/P4 remain gated.
- **DONE — §536 Cross-ecosystem direct-argv digest vector parity**: The canonical three-vector fixture now covers a baseline command, punctuation with an empty argument, and UTF-8 arguments. Forge Core, Runtime, Snaplink Console, Aero-ID, Aero-IM, Aero-Vault, and Snaplink Audit Governance strictly consume the same vectors and reject unknown, duplicate, and trailing JSON. This remains offline, authority-free interoperability evidence with no command persistence, lease/device authority, reservation, scheduling, Runner transport, execution, receipt, or Audit publication; ADR-0039/ADR-0114/P4 remain gated.
- **DONE — §537 Cross-ecosystem Runner terminal receipt outcome vectors**: The canonical terminal receipt fixture covers completed, failed, and uncertain outcomes. Core, Runtime, Snaplink Console, Aero-ID, Aero-IM, Aero-Vault, and Snaplink Audit Governance recompute the direct-argv digest, bind lease proof and half-open time windows, preserve the uncertain/manual-reconciliation flags, and reject unknown/duplicate/trailing wire drift plus digest/expiry mutations. This remains offline, authority-free evidence; no Runner transport, execution, receipt persistence, Audit publication, automatic retry, or production device authority was added. ADR-0039/ADR-0114/P4 remain gated.
- **DONE — §538 Cross-ecosystem session Runner receipt outcome vectors**: Added canonical completed/failed/uncertain `forge.session-runner-receipt-observation/v1` vectors with strict Core/Runtime/Console/Aero-ID/Aero-IM/Aero-Vault/Snaplink Audit Governance consumers. The vectors enforce owner/session and command/Attempt/target/digest bindings, all-false authority, null selected target, and manual-only uncertain handling. This remains offline interoperability evidence with no receipt persistence, lease/Runner effect, retry, or Audit authority.
- **DONE — §539 Console session Runner receipt outcome-vector import**: Snaplink Console's shared Web/App/Mobile Sessions surface now strictly consumes the canonical `forge.session-runner-receipt-vectors/v1` envelope through an optional local reader or workspace picker. The panel exposes completed, failed, and uncertain metadata while duplicate/trailing/unknown fields, expectation drift, and authority elevation fail closed; the default Gate remains reader-free and request-free. No receipt persistence, target selection, retry, Runner transport, execution, or Audit authority was added; ADR-0039/ADR-0114/P4 remain gated.
- **DONE — §540 Cross-ecosystem session Runner receipt history reduction**: Core and Runtime now reduce a bounded owner/Conversation/Prompt/Run history of unique Attempt observations in nondecreasing observed-time order; equal timestamps use Attempt ID as a deterministic tie-breaker. Failed outcomes may continue; completed and uncertain outcomes close the history, and uncertain remains manual reconciliation with `automatic_retry=false`. Runtime CLI/TUI, Console Web/App/Mobile, Aero-ID, Aero-IM, Aero-Vault, and Snaplink Audit Governance consume byte-identical history fixtures and reject wire, binding, ordering, lifecycle, summary, selected-target, and authority drift. The value remains read-only with no receipt persistence, lease/Runner effect, retry, dispatch, execution, or Audit authority; ADR-0039/ADR-0114/P4 remain gated.
- **DONE — §541 Authenticated session Runner receipt history and manual reconciliation projection**: Core now exposes an explicit owner/path-bound `runner-receipt-history/preview` candidate only through observation/accepted EXECUTE assembly; Runtime CLI/TUI and Console Web/App/Mobile use it only with explicit request/origin seams and recheck the same canonical reduction. The uncertain terminal summary also has a strict `forge.session-runner-reconciliation-projection/v1` fixture consumed by Aero-ID, Aero-IM, Aero-Vault, and Snaplink Audit Governance, with manual reconciliation required, automatic retry disabled, null selected target, and all authority false. Default constructors remain closed; no receipt persistence, retry, selection, reservation, lease effect, Runner transport/execution, or Audit publication was added; ADR-0039/ADR-0114/P4 remain gated.
- **DONE — §542 Core authenticated session Runner reconciliation projection**: Added a strict Core projector from canonical uncertain terminal receipt history plus an explicit authenticated `runner-reconciliation/preview` candidate mounted only in observation/accepted EXECUTE assembly. It preserves manual-only handling, automatic retry false, null target selection, preview-only output, and all-false authority; normal production construction remains closed and no persistence, scheduling, lease, Runner effect, or Audit publication was added.
- **DONE — §543 Cross-client authenticated session Runner reconciliation projection**: Runtime CLI/TUI now consume and strictly revalidate an explicit bounded `forge.session-runner-reconciliation-projection/v1` input, and the shared Snaplink Console Web/App/Mobile Sessions surface adds strict local import/display plus an opt-in owner/path-bound API/Gate candidate. Default construction stays request-free; unauthorized POSTs are not refreshed or replayed, and source/latest/binding/selection/authority drift fails closed. No receipt persistence, retry, target selection/reservation, scheduling, lease, Runner transport/execution, or Audit publication was added.
- **DONE — §544 Accepted EXECUTE+P4 Core session Runner reconciliation E2E**: The accepted Core server now runs the minimal authenticated two-step chain from `runner-receipt-history/preview` to `runner-reconciliation/preview`, checking the canonical history response and the uncertain/manual projection with `automatic_retry=false`, null selected target, and all-false authority. The harness leaves the lease registry and independent Runner authority unset, so it opens no transport, executes no argv, persists no receipt/Attempt, mutates no lease, retries no work, and publishes no Audit; ADR-0039/ADR-0114/P4 remain gated.
- **DONE — §545 Authenticated Runtime session Runner reconciliation consumers**: Runtime now separates the offline projection command from explicit authenticated `remote-preview` consumers. CLI and TUI read one bounded owner/Conversation/Run-bound receipt history, post it once to Core, and strictly recompute the pure manual projection; mock transport/TUI checks plus the accepted Core/Runtime CLI+TUI harness reject path, source, latest-value, target, and authority drift. The route remains stateless and never retries, selects/reserves, persists a receipt, dispatches/executes a Runner, or publishes Audit; ADR-0039/ADR-0114/P4 remain gated.
- **DONE — §546 Console authenticated session Runner reconciliation chain**: Snaplink Console now has an explicit opt-in API helper that first canonicalizes owner/Conversation/Run-bound receipt history through `runner-receipt-history/preview` and then derives reconciliation through `runner-reconciliation/preview`. API tests and the accepted Console harness verify request order, exact paths, bearer binding, canonical forwarding, and display-only output; the existing one-step API and default Gate remain unchanged/request-free. No receipt persistence, retry, target selection/reservation, scheduling, lease mutation, Runner transport/execution, or Audit publication was added; ADR-0039/ADR-0114/P4 remain gated.
- **DONE — §547 Console Gate authenticated session Runner reconciliation chain**: The shared Web/App/Mobile Gate now has a separately default-off two-hop history-chain candidate. It canonicalizes owner/Conversation/Run-bound history through `runner-receipt-history/preview`, then derives reconciliation through `runner-reconciliation/preview`; Gate tests and the accepted Runtime-backed Console harness verify both bearer-bound requests and one display-only projection. Existing one-step/default behavior remains unchanged/request-free; no receipt persistence, retry, target selection/reservation, scheduling, lease mutation, Runner transport/execution, or Audit publication was added; ADR-0039/ADR-0114/P4 remain gated.
- **DONE — §548 Runtime authenticated session Runner reconciliation chain**: Runtime CLI and TUI `session-runner-reconciliation remote-preview` now canonicalize the bounded owner/Conversation/Run-bound history through `runner-receipt-history/preview` before forwarding that exact response to `runner-reconciliation/preview`. Focused tests and the accepted Core/Runtime harness verify request order, bearer/path/body binding, canonical forwarding, pure manual projection validation, and TUI selected-Run mismatch rejection; the offline projection command remains request-free. No receipt persistence, retry, target selection/reservation, scheduling, lease mutation, Runner transport/execution, or Audit publication was added; ADR-0039/ADR-0114/P4 remain gated.
- **DONE — §549 Accepted scheduler selection preview through the Console Gate**: The accepted `EXECUTE + P4` inventory activation harness now drives the authenticated Web/App/Mobile Sessions Gate through the owner-scoped scheduler selection preview route backed by owner v2 resource inventory. The Gate pins Conversation/Run/Attempt and requirements, posts one opt-in preview, and renders the bounded no-candidate result with all authority flags false; the default Gate remains request-free. No reservation/lease, execution target authority, Runner dispatch/execution, Attempt/receipt persistence, or Audit publication was added; ADR-0039/ADR-0114/P4 remain gated.
- **DONE — §550 Accepted scheduler selection preview through Runtime TUI**: The accepted `EXECUTE + P4` inventory activation harness now drives the authenticated Runtime TUI through the same owner-scoped scheduler selection preview route already covered by Core, Runtime CLI, and the Console Gate. A PTY command posts one bounded Conversation/Run/Attempt request and verifies the human rendering of the deterministic no-candidate result with `preview_only=true` and all authority flags false. The TUI helper is opt-in behind `FORGE_RUNTIME_BIN`; it does not select a target, create a reservation/lease, dispatch or execute a Runner, persist an Attempt/receipt, or publish Audit; ADR-0039/ADR-0114/P4 remain gated.
- **DONE — §551 Accepted cross-client harness coverage in the contract script**: `scripts/test-forge-contracts.sh` now builds or reuses a real `forge-runtime` binary and runs the accepted inventory activation plus scheduler-lease and Runner preview chains with `FORGE_RUNTIME_BIN` and `FORGE_CONSOLE_E2E=1`, so the Runtime CLI/TUI and authenticated Web/App/Mobile Gate paths are exercised instead of only the no-runtime Core branch. The same script also invokes the accepted Runner execution-intent path and the focused receipt/evidence/reconciliation chains. This changes verification coverage only; all routes remain preview/read-only or explicitly lease-scoped under the existing accepted test fixture, with no live Runner transport, execution, receipt authority, or Audit publication. ADR-0039/ADR-0114/P4 remain gated.
- **DONE — §552 Policy-complete scheduler preview parity with the fenced lease**: The accepted `EXECUTE` route assembly now injects the same owner-private policy registry into both scheduler-selection preview and fenced scheduler-lease evaluation. When the strict 0600 policy image is present, preview and lease share residency, trust, sandbox, and concurrency checks plus exact inventory counter bindings; without it, the existing lifecycle-only preview remains display-only and lease remains fail-closed. Focused route, policy-join, and lease replay tests pass. No new Runner transport, command, receipt, Audit, or execution authority was added; ADR-0039 remains planning-only, ADR-0114 remains Proposed/null, and P4 remains gated.
- **DONE — §553 Accepted positive policy-complete scheduler preview across clients**: The accepted `EXECUTE + P4` activation harness now uses a fresh owner lifecycle image joined to the strict private policy registry and expects the deterministic `device-a/runner-a` candidate. Runtime CLI/TUI and Snaplink Console Web/App/Mobile API plus Sessions Gate each POST the same policy-complete preview and verify the selected pair while every preview authority bit remains false; the lifecycle-only no-candidate path remains covered separately. No reservation/lease claim, Runner transport/execution, receipt/Audit authority, or live device enrollment was added; ADR-0039/ADR-0114/P4 remain gated.
- **DONE — §554 Accepted cross-client scheduler lease lifecycle**: The accepted `EXECUTE + P4` scheduler-lease harness now renews one active fenced proof through Core, has independent Console Web/App/Mobile API clients replay the exact renewal receipt, and drives the real Sessions Gate through the final release with the returned epoch/token proof. The release panel preserves the epoch and withholds fencing material; all execution, dispatch, and Audit authority remains false. This adds no Runner transport/execution, receipt persistence, or enrollment/heartbeat authority; ADR-0039/ADR-0114/P4 remain gated.
- **DONE — §555 Console scheduler preview selected-Run stale guard**: The authenticated Console Sessions surface now refuses to POST the opt-in scheduler-selection preview when its selected Conversation or Run differs from the configured request, clearing stale projection state instead. Focused Gate coverage proves zero scheduler-preview requests on selected-Run mismatch, while the accepted policy-complete Web/App/Mobile Gate path remains green with all authority false; no target selection/reservation/lease, Runner transport/execution, Attempt/receipt persistence, enrollment, or Audit publication was added; ADR-0039/ADR-0114/P4 remain gated.
- **DONE — §556 Scheduler lease stale-epoch and idempotency-conflict fencing**: The authenticated Core lease route now has negative-path coverage proving that an old proof cannot release the replacement epoch after renewal, and that reusing a completed release idempotency key with a changed target is rejected as `idempotency_conflict`. The Console API test confirms a stale-epoch response is surfaced once without retry or replay. No Runner transport/execution, receipt persistence, Audit publication, enrollment, or heartbeat authority was added; ADR-0039/ADR-0114/P4 remain gated.
- **DONE — §557 Cross-ecosystem shared-session receiver parity**: Aero-ID, Aero-IM, Aero-Vault, and Snaplink Audit Governance now strictly consume byte-identical mirrors of `forge-shared-session-v1.json`, covering Conversation list/detail, Prompt history, owner-local change cursors, and storage-only append receipts. Unknown, duplicate, trailing, foreign-binding, role, cursor/order, and replay mutations fail closed. This remains compatibility evidence only; no session store, Prompt authority, device enrollment/inventory authority, scheduling, Runner transport/execution, receipt persistence, or Audit publication was added; ADR-0039/ADR-0114/P4 remain gated.
- **DONE — §558 Accepted inventory refresh convergence across Console API clients**: The accepted refresh harness now starts independent Web/App/Mobile Console API clients after replacing the private lifecycle image; v1, v2, and composed client-instance/resource readers converge on the same owner-bound revision, generation, heartbeat, liveness, and reservation metadata. This remains display-only and adds no enrollment/heartbeat authority, mutation, selection, scheduling, Runner transport/execution, receipt persistence, or Audit publication; ADR-0039/ADR-0114/P4 remain gated.
- **DONE — §559 Lease renewal fencing at dispatch admission**: The metadata-only Runner dispatch-admission preview now rechecks the durable lease after a scheduler renewal: the old epoch/token returns `lease_stale`, while the replacement proof returns a redacted admission observation with all dispatch/execution/Audit authority false. This fixes the read-adapter lookup ordering so a stale matching historical entry cannot bypass a newer epoch. No Runner transport/command execution, receipt persistence, Audit publication, enrollment, or heartbeat authority was added; ADR-0039/ADR-0114/P4 remain gated.
- **DONE — §560 Cross-ecosystem scheduler lease release receipt parity**: The canonical `forge.execution-lease-release/v1` terminal observation is mirrored and strictly consumed by Aero-ID, Aero-IM, Aero-Vault, and Snaplink Audit Governance. Receivers bind owner/Conversation/Run/Attempt, device/Runner instance, epoch, and release time, reject unknown/duplicate/missing/trailing, binding, unsafe-epoch, and authority mutations, and keep all placement/reservation/lease/execution/dispatch/Audit flags false; `replayed` remains available for exact idempotent replay. This is offline compatibility evidence only and adds no release, selection, reservation, Runner transport/execution, receipt persistence, retry, or Audit authority; ADR-0039/ADR-0114/P4 remain gated.
- **DONE — §561 Console Gate scheduler lease selected-scope stale guards**: The authenticated Web/App/Mobile Sessions Gate now applies the selected Conversation/Run display-scope guard to explicit scheduler lease claim, renewal, and release candidates. A configured candidate whose binding differs from the mounted selection is cleared without POST, retry, or replay; the empty/default Gate remains compatible. Focused Gate tests cover claim Run drift, renewal Conversation drift, and release Run drift. No Runner transport/execution, receipt persistence, Audit publication, enrollment, heartbeat authority, or live scheduler effect was added; ADR-0039/ADR-0114/P4 remain gated.
- **DONE — §562 Runtime TUI paired inventory/resource refresh convergence**: Runtime TUI's explicit v2 inventory and client-instance/resource readers now compare shared device/Runner identity plus revision, generation, and heartbeat counters as one pair. On mismatch, the previous pair is retained, the owner change cursor is not advanced, and the next `sync` retries the same boundary; a converged pair is persisted and displayed together. This is display consistency only: no enrollment/heartbeat or inventory authority, placement/reservation/lease/scheduling, Runner transport/execution, receipt, or Audit effect was added; ADR-0039 remains planning-only, ADR-0114 remains Proposed/null, and P4 remains gated.
- **DONE — §563 Runtime TUI paired client-instance session/resource refresh convergence**: Runtime TUI now compares the owner declaration and complete instance rows from the explicit session-view and resource-view readers, including session visibility, status, client kind, and observation time. A mismatched refresh restores the previous pair, keeps the client-instance filter fail-closed, and does not advance the owner change cursor; the next `sync` retries the same boundary. This is display consistency only and adds no Prompt authority, enrollment/heartbeat authority, inventory mutation, placement/reservation/lease/scheduling, Runner transport/execution, receipt, or Audit effect; ADR-0039/ADR-0114/P4 remain gated.
- **DONE — §564 Core Attempt lifecycle dispatch boundary**: Forge Core adds the pure `forge.runner-attempt-boundary/v1` check after the existing execution-boundary preview. It reuses the canonical Attempt state graph, exposes only `accepted→starting` and `starting→running` as dispatchable, and fails closed for invalid or terminal/preparatory edges while repeating redacted identity/epoch metadata with preview-only, all-false Attempt/reservation/execution/dispatch/Audit authority. No HTTP route, lease mutation, Runner transport/execution, receipt persistence, or Audit publication was added; ADR-0039/ADR-0114/P4 remain gated.
- **DONE — §565 Cross-ecosystem Runner command terminal receipt ABI parity**: Aero-ID, Aero-IM, Aero-Vault, and Snaplink Audit Governance now strictly consume byte-identical mirrors of the canonical `forge.runner-command-terminal-receipt/v1` fixture. Each receiver recomputes the domain-separated command digest, binds the terminal receipt to the same Attempt/target/epoch/fencing proof and observation window, validates disposition shape, and rejects unknown/duplicate/trailing, digest/proof/expiry, and authority mutations. This remains immutable offline interoperability evidence; no argv execution, Runner transport, command/receipt persistence, lease mutation, reservation, retry, or Audit publication was added; ADR-0039/ADR-0114/P4 remain gated.
- **DONE — §566 Runtime CLI offline Runner Attempt boundary consumer**: Forge Runtime adds `device runner-attempt-boundary-preview --input FILE|-`, a bounded strict decoder and metadata-only renderer for `forge.runner-attempt-boundary/v1`. Duplicate/unknown/trailing JSON, lifecycle/readiness drift, and authority elevation fail closed. The command uses no HTTP, lease read or mutation, Attempt persistence, Runner transport, command authorization, argv execution, or Audit publication; ADR-0039/ADR-0114/P4 remain gated.
- **DONE — §567 Cross-ecosystem Runner Attempt boundary receiver parity**: The canonical `forge.runner-attempt-boundary/v1` observation is mirrored byte-for-byte into Aero-ID, Aero-IM, Aero-Vault, and Snaplink Audit Governance. Strict receivers bind owner/Conversation/Run/Attempt/command/target/epoch and recompute lifecycle, readiness, rejection, preview, and all-false authority predicates; unknown, duplicate, trailing, lifecycle/readiness, and authority drift fail closed. This is offline ABI evidence only and adds no HTTP route, Attempt persistence, lease mutation, reservation, Runner transport, command authorization, argv execution, receipt persistence, or Audit publication; ADR-0039/ADR-0114/P4 remain gated.
- **DONE — §568 Console Web/App/Mobile Runner Attempt boundary display consumer**: Snaplink Console adds a shared strict Flutter model and read-only card for `forge.runner-attempt-boundary/v1`, so Web/App/Mobile render the same owner/Conversation/Run/Attempt/command/target and lifecycle metadata. Unknown, duplicate, trailing, lifecycle, rejection-order, and authority drift fail closed; no transition control, lease proof, argv, Runner action, HTTP request, Prompt/Attempt persistence, lease mutation, reservation, scheduling, execution, receipt persistence, or Audit publication was added; ADR-0039/ADR-0114/P4 remain gated.
- **DONE — §569 Console Sessions Gate scoped local Runner Attempt boundary projection/import**: Snaplink Console's shared Web/App/Mobile Sessions Gate now accepts an explicitly enabled local `forge.runner-attempt-boundary/v1` projection/import seam. A caller-declared owner/Conversation/Run/Attempt scope is required; selected-session drift, owner drift, lifecycle drift, and authority elevation fail closed, and imported stale state is cleared. The default Gate remains projection-disabled and request-free; no HTTP, lease, Prompt/Attempt persistence, reservation, Runner/argv, or Audit authority was added; ADR-0039/ADR-0114/P4 remain gated.
- **DONE — §570 Runtime TUI offline Runner Attempt boundary consumer**: Forge Runtime's interactive TUI exposes `runner-attempt-boundary-preview --input FILE` as a bounded local consumer of `forge.runner-attempt-boundary/v1`, reusing the strict decoder and metadata-only renderer. Unknown, duplicate, trailing, lifecycle/readiness, and authority drift fail closed; the file-only command preserves interactive stdin and performs no request. No Attempt persistence, lease read or mutation, selection, reservation, scheduling, Runner transport, command authorization, argv execution, receipt persistence, or Audit publication was added; ADR-0039/ADR-0114/P4 remain gated.
- **DONE — §571 Authenticated Runner Attempt boundary preview candidate**: Forge Core now mounts an opt-in owner/path-bound `runner-attempt-boundary/preview` candidate after the existing authenticated Runner execution-boundary preview. It re-reads the owner-private fenced lease, recomputes the redacted execution boundary, and projects the canonical Attempt lifecycle observation for one caller-declared transition; route tests cover the ready edge, proof redaction, and authority-free 404. The ordinary constructor remains closed and the candidate adds no Attempt persistence, lease mutation/renewal, reservation, scheduling, Runner transport, command authorization, argv execution, receipt persistence, or Audit publication; ADR-0039/ADR-0114/P4 remain gated.
- **DONE — §572 Canonical Prompt append request/receipt parity**: Forge Core's pure `forge.prompt-append-receipt/v1` projection binds an owner-scoped Conversation, expected version, user role, Prompt identity/version/time, and content/idempotency digests while omitting Prompt content. Runtime and Console recompute and strictly consume the same metadata-only envelope; Aero-ID, Aero-IM, Aero-Vault, and Snaplink Audit Governance consume byte-identical mirrors and reject unknown/duplicate/trailing, digest/binding/version/content-disclosure, and authority drift. This is compatibility evidence only: no HTTP route, Prompt store, Run, device selection, reservation, dispatch, scheduling, Runner, or Audit effect was added; ADR-0039/ADR-0114/P4 remain gated.
- **DONE — §573 Runtime authenticated Runner Attempt boundary consumers**: Runtime CLI `remote placement runner-attempt-boundary-preview --input FILE|-` and TUI `runner-attempt-boundary-remote-preview --input FILE` now post the explicit owner/Conversation/Run/Attempt/transition request once to Core and strictly consume the canonical Attempt boundary observation. Selected-session binding, lifecycle/readiness, response identity/epoch, redaction, `preview_only`, and all-false authority are enforced; 401 is not retried. No Attempt persistence, lease mutation, reservation, scheduling, Runner transport/argv execution, receipt persistence, enrollment/heartbeat, or Audit publication was added; ADR-0039/ADR-0114/P4 remain gated.
- **DONE — §574 Console authenticated Runner Attempt boundary candidate**: Snaplink Console Web/App/Mobile now share a strict origin-pinned API adapter and an explicit Sessions Gate candidate for the existing `runner-attempt-boundary/preview` projection. The adapter validates the owner/path/Attempt/command/target/epoch/state/transition binding, posts once with unauthorized retry disabled, and the Screen rejects selected-scope, response, authority, and stale-generation drift. The default Gate remains request-free and the candidate has no Attempt/Prompt persistence, lease mutation, device selection/reservation, scheduling, Runner transport/argv execution, receipt persistence, or Audit publication; ADR-0039/ADR-0114/P4 remain gated.
- **DONE — §575 Core Attempt boundary stale-lease fencing**: The authenticated `runner-attempt-boundary/preview` candidate now has focused stale-epoch coverage: after the owner-private lease is renewed, a request carrying the previous Attempt proof returns `lease_stale` before lifecycle preview. The test exercises only the existing durable read adapter and route error mapping; no lease mutation is performed by the preview route and no Runtime/Console client surface is changed. No Runner transport/command execution, receipt persistence, Audit publication, enrollment, or heartbeat authority was added; ADR-0039/ADR-0114/P4 remain gated.
- **DONE — §576 Accepted Runner Attempt boundary cross-client evidence**: The accepted `EXECUTE + P4` harness sends one owner/path-bound Attempt boundary request through authenticated Core, Runtime CLI/TUI, and Snaplink Console's shared Web/App/Mobile API/Sessions Gate. It verifies the same redacted lifecycle binding and all-false authority at each client, then compares the durable lease image before and after preview to prove no lease mutation. Attempt persistence, reservation, scheduling, Runner transport/argv execution, receipt persistence, and Audit publication remain absent; production construction stays 404/default-off and ADR-0039/ADR-0114/P4 remain gated.
- **DONE — §577 Console authenticated Prompt append receipt consumer**: Snaplink Console's shared Web/App/Mobile API now exposes an explicit `appendPromptReceipt` adapter over the authenticated owner-scoped Prompt append endpoint. It validates owner, Conversation path, user role, expected aggregate version, Prompt identity, and exact idempotency-key binding before returning the canonical content-free `forge.prompt-append-receipt/v1` observation with `content_included=false` and all downstream authority false. The POST is single-shot, including on HTTP 401; the default Sessions Gate remains request-free and does not wire this method. The existing Prompt append write is the only storage operation; no Run/device selection/reservation, scheduling, Runner transport/execution, receipt persistence, or Audit publication was added; ADR-0039/ADR-0114/P4 remain gated.
- **DONE — §578 Runner Attempt boundary route negative closure**: The authenticated `runner-attempt-boundary/preview` candidate now has focused HTTP boundary coverage for non-POST method rejection with `Allow: POST`, query rejection before body interpretation, and unknown-proof `lease_not_found` handling. This keeps the owner/path-bound Attempt lifecycle projection bounded and proof-backed without adding a client surface or route effect; no Attempt/Prompt persistence, lease mutation, reservation, scheduling, Runner transport/argv execution, receipt persistence, enrollment, heartbeat, or Audit publication was added; ADR-0039/ADR-0114/P4 remain gated.
- **DONE — §579 Core Prompt append response binding closure**: The authenticated Conversation Prompt append route now validates the backend response against the requested Conversation, exact user content, `user` role, Prompt identity, JSON-safe timestamp, and `expected_version + 1` aggregate-version receipt before serializing it. Focused HTTP regressions reject foreign Conversation, role/content/identity/timestamp/version drift with a sanitized `502`; valid ceiling values remain accepted. This is response-boundary hygiene only: no new route, storage, Run, device selection, reservation, scheduling, Runner, receipt persistence, or Audit behavior was added; ADR-0039/ADR-0114/P4 remain gated.
- **DONE — §580 Runtime Prompt append receipt response binding closure**: Forge Runtime's explicit `remote prompts ... --receipt` path now binds the authenticated Prompt append response to the exact Conversation, `user` role, submitted content, Prompt identity, JSON-safe timestamp, and `expected_version + 1` aggregate version before projecting the content-free `forge.prompt-append-receipt/v1` observation. Foreign Conversation, role/content/identity, timestamp, CAS, and replay-marker drift fail closed; POST remains single-shot and all Run/device/reservation/dispatch/execution/Audit authority flags remain false. No default Sessions Gate request or Run start was added; ADR-0039/ADR-0114/P4 remain gated.
- **DONE — §581 Runtime TUI Prompt append receipt consumer**: Forge Runtime's interactive TUI now sends Prompts through the authenticated `append_prompt_receipt` projection. Pending writes retain the selected Conversation, CAS version, content, and idempotency key across an uncertain write; explicit retry consumes nested `receipt.aggregate_version` and `receipt.replayed`, refreshes owner-visible Prompt history, and reports that no Run was started. The selected client-instance display guard remains before the request, while 401/403/conflict handling preserves existing pending-write and local-view rules. Focused coverage exercises successful and idempotent-replay receipts. No Run/device selection/reservation, scheduling, Runner transport/execution, lease mutation, receipt persistence, or Audit publication was added; ADR-0039/ADR-0114/P4 remain gated.
- **DONE — §582 Runtime CLI paired inventory/resource convergence consumer**: `remote inventory show-converged` now performs two explicit authenticated GETs for the owner-scoped lossless v2 inventory and composed client-instance/resource view, strictly validates both, and fails closed on device/Runner identity or revision/generation/heartbeat drift before returning a read-only convergence envelope. All authority predicates remain false; no observation persistence, enrollment/heartbeat, target selection, reservation, scheduler lease, dispatch, Runner execution, receipt, or Audit effect was added; ADR-0039/ADR-0114/P4 remain gated.
- **DONE — §583 Console Sessions Gate Prompt append receipt opt-in**: Snaplink Console Web/App/Mobile now exposes an explicit owner-bound Prompt append receipt submitter. The Gate constructs the authenticated adapter only with the owner, candidate origin, and opt-in flag; the Screen re-decodes and rebinds the content-free receipt to the selected Conversation/CAS/content/idempotency key, then refreshes ordinary Prompt history. Selected client-instance drift fails closed, confirmed receipts clear pending input, and the default Gate remains request-free; no Run/device selection/reservation, scheduling, Runner transport/execution, lease mutation, receipt persistence, or Audit publication was added; ADR-0039/ADR-0114/P4 remain gated.
- **DONE — §584 Aero-IM Runner execution-intent request receiver parity closure**: Aero-IM's audit connector now strictly consumes the canonical forge-runner-execution-intent-request/v1 fixture alongside Aero-ID, Aero-Vault, and Snaplink Audit Governance. It rejects unknown/duplicate/trailing JSON, binds owner/Conversation/Prompt/Run/Attempt/Command/idempotency/lease-proof metadata, recomputes the domain-separated Runner command digest, and fails closed on owner, binding, digest, or selected-target drift. This closes the receiver implementation gap under §534 while remaining offline and authority-free: no command persistence, lease/device authentication, enrollment, heartbeat, selection, reservation, scheduling, Runner transport/execution, receipt, or Audit publication was added; ADR-0039/ADR-0114/P4 remain gated.
- **DONE — §585 Runtime CLI paired client-instance session/resource convergence**: `remote client-instances show-converged` performs two explicit authenticated GETs for the owner-bound session and composed resource views, reuses both strict response validators, and fails closed when the owner declaration or any client-instance row drifts. It returns a display-only convergence envelope and adds no client registration/heartbeat, Prompt or session authority, inventory mutation, device selection/reservation, scheduler lease, Runner transport/execution, receipt persistence, or Audit publication; ADR-0039/ADR-0114/P4 remain gated.
- **DONE — §586 Console paired inventory/resource convergence candidate**: Snaplink Console's shared Web/App/Mobile Sessions Gate now accepts an explicitly enabled owner-bound paired reader for lossless v2 inventory plus the composed client-instance/resource view. It performs two authenticated GETs, strictly joins device/Runner identity with revision, generation, and heartbeat sequence, and fails closed on owner, shape, counter, or authority drift while the default Gate remains request-free. This is display consistency only: no enrollment/heartbeat, session or Prompt authority, inventory mutation, selection/reservation, scheduler lease, Runner transport/execution, receipt persistence, or Audit publication was added; ADR-0039/ADR-0114/P4 remain gated.
- **DONE — §587 Runtime TUI explicit paired client-instance convergence**: `client-instances show-converged` performs one authenticated session-view GET and one resource-view GET, commits both local projections only after the owner declaration and complete instance rows converge, and retains the previous pair on non-authority drift while clearing the local owner view on 401/403. The command remains display-only with no client registration/heartbeat, Prompt/session authority, inventory mutation, device selection/reservation, scheduler lease, Runner transport/execution, receipt persistence, or Audit publication; ADR-0039/ADR-0114/P4 remain gated.
- **DONE — §588 Console shared paired client-instance session/resource observation**: Snaplink Console's shared Web/App/Mobile Sessions Gate now accepts an explicit owner-bound reader that performs one authenticated GET for the session view and one for the resource view, strictly re-decodes both, and feeds the existing session/resource panels only after owner and complete instance rows converge. Drift fails closed and the last validated pair is marked stale; the default Gate remains request-free with all authority flags false. No client registration/heartbeat, Prompt/session authority, inventory mutation, device selection/reservation, scheduler lease, Runner transport/execution, receipt persistence, or Audit publication was added; ADR-0039/ADR-0114/P4 remain gated.
- **DONE — §589 Cross-ecosystem client-instance convergence fixture parity**: The paired client-instance session/resource envelope is frozen in `forge-client-instance-session-resource-convergence-v1.json`. Forge Runtime's authenticated CLI reader, Snaplink Console's Web/App/Mobile reader test, Aero-ID, Aero-IM, Aero-Vault, and Snaplink Audit Governance all consume the same owner-bound source pair, reject unknown/duplicate envelope fields and owner/row drift, and keep authority predicates false. This is compatibility evidence only: no client-instance authentication, observation persistence, device selection/reservation, scheduler lease, Runner dispatch/execution, receipt persistence, or Audit publication was added; ADR-0039/ADR-0114/P4 remain gated.
- **DONE — §590 Cross-ecosystem inventory/resource convergence fixture parity**: The owner-bound `forge.device-inventory-resource-convergence/v1` envelope is frozen in `forge-device-inventory-resource-convergence-v1.json`. Runtime CLI/TUI, Snaplink Console, Aero-ID, Aero-IM, Aero-Vault, and Snaplink Audit Governance consume the same v2 inventory plus client-instance/resource pair, bind owner/device/Runner identity and revision/generation/heartbeat counters, and reject unknown, duplicate, authority, owner, and lifecycle drift while keeping authority predicates false. This is display-only compatibility evidence: no inventory persistence, enrollment/heartbeat authority, target selection, reservation, scheduler lease, Runner transport/execution, receipt persistence, or Audit publication was added; ADR-0039/ADR-0114/P4 remain gated.
- **DONE — §591 Shared-session fixture mirror and paired CLI instance projection**: Snaplink Console now keeps a byte-identical local mirror of the canonical `forge-shared-session-v1.json` envelope, and the contract script compares that mirror before running the Console Conversation/Prompt contract test. Runtime CLI accepts the paired `forge.client-instance-session-resource-convergence/v1` envelope as a strict local `--instance-view`, filters the owner session list, permits a visible-instance Prompt POST, and rejects an invisible Conversation before any write request. No API, authentication, session storage, Prompt authority, Run creation, device inventory mutation, scheduling, Runner effect, receipt persistence, or Audit publication was added; ADR-0039/ADR-0114/P4 remain gated.
- **DONE — §592 Console initial client-instance selection boundary**: Snaplink Console's shared Web/App/Mobile Sessions Gate accepts an optional `initialClientInstanceID` display hint. With an explicit owner-bound session/resource observation, it loads the observation before the first Conversation page, filters the local list, and hydrates Prompt/Run detail only for a declared session; unknown or unavailable instances perform no private history read. The hint remains display-only and does not authenticate instances, grant Prompt/session authority, mutate inventory, select/reserve/schedule devices, dispatch or execute Runner work, persist receipts, or publish Audit; ADR-0039/ADR-0114/P4 remain gated.
- **DONE — §593 Authenticated execution-intent preview binds durable Prompt and Run references**: The accepted Coordinator execution-intent preview now re-reads owner-scoped Runtime Prompt and Run pages and fails closed unless the requested Prompt exists and the requested Run is bound to it. Reads use existing bounded page limits with a 64-page cap and reject malformed, unavailable, or cycling pages. The projection remains selected-target-null and all-false authority; it creates no Run/command, mutates no lease, selects/reserves no device, opens no Runner transport, executes no argv, persists no receipt, and publishes no Audit. ADR-0039/ADR-0114/P4 remain gated.
- **DONE — §594 Authenticated paired client-instance projection proof**: Forge Core's opt-in Snaplink JWT integration test now reads the owner-bound session and composed resource views with one bearer, checks identical CLI/TUI/Web/App/Mobile rows and the joined device/Runner resource, and verifies all authority predicates remain false. The routes remain test-mux-only; no client registration/authentication, observation persistence, Prompt/session authority, inventory enrollment/heartbeat, target selection/reservation, scheduler lease, Runner transport/execution, receipt persistence, or Audit publication was added; ADR-0039/ADR-0114/P4 remain gated.
- **DONE — §595 Accepted Runner preview binds durable owner-scoped Run references**: The accepted `EXECUTE + P4` execution-boundary and Attempt-boundary previews now re-read bounded owner-scoped Runtime Run pages before reading the private lease. Missing, foreign, malformed, unavailable, cycling, or overlong references fail closed; Runtime-backed E2Es require a real binary and value-only tests retain a nil backend. No Run/Attempt/lease mutation, reservation, scheduler state, Runner transport, command authorization, argv execution, receipt persistence, or Audit publication was added; ADR-0039/ADR-0114/P4 remain gated.
- **DONE — §596 Accepted Runner admission previews bind durable owner-scoped Run references**: Accepted `EXECUTE + P4` dispatch-admission and transport-admission previews now re-read bounded owner-scoped Runtime Run pages before evaluating the fenced lease or verified transport observation. Missing, foreign, malformed, unavailable, cycling, or overlong references fail closed; production injects the Runtime backend while value-only tests remain backend-free. No new Run/Attempt/lease mutation, reservation, Runner connection/payload, command authorization, argv execution, receipt persistence, or Audit publication was added; ADR-0039/ADR-0114/P4 remain gated.
- **DONE — §597 Cross-ecosystem durable Run-binding negative parity**: Aero-ID, Aero-Vault, Snaplink Audit Governance, Aero-IM, Snaplink Console, and Forge Runtime now share explicit negative coverage for `forge-runner-execution-intent-request/v1`: a foreign `run_reference.run_id` or `run_reference.prompt_id` is rejected before the authority-free preview can be consumed. No Run/Prompt authority, inventory enrollment/heartbeat, selection/reservation, scheduler lease, Runner transport/command execution, receipt persistence, or Audit publication was added; ADR-0039/ADR-0114/P4 remain gated.
- **DONE — §598 Runtime TUI scheduler requests respect the selected client-instance projection**: Forge Runtime's TUI now checks the active client-instance session projection before posting scheduler-selection preview, lease claim, lease renewal, or lease release requests. A request whose `conversation_id` is not declared by the selected instance is rejected locally with no POST; no instance filter preserves existing caller-supplied behavior. This is display-scope validation only and adds no registration/heartbeat, inventory, selection/reservation, scheduler, Runner, lease, receipt, or Audit authority; ADR-0039/ADR-0114/P4 remain gated.
- **DONE — §599 Scheduler lease claims bind durable owner-scoped Run references**: The accepted `EXECUTE + P4` scheduler lease claim now re-reads the owner-scoped Runtime Run projection before evaluating inventory or creating a fenced lease. Missing, foreign, malformed, unavailable, cycling, or overlong references fail closed; production injects the Runtime backend while value-only route tests remain backend-free. Renewal and release stay available for existing lease cleanup. No Run creation, Runner transport/command execution, receipt persistence, or Audit publication was added; ADR-0039/ADR-0114/P4 remain gated.
- **DONE — §600 Scheduler lease renewal requires a live Run while release survives deletion**: Accepted `EXECUTE + P4` lease renewal now re-reads the owner-scoped Runtime Run before extending a fenced reservation and fails closed when that Run is missing or malformed. Release deliberately keeps the owner-scoped durable Conversation/Run/Attempt proof path without a Runtime read, so Run deletion cannot strand cleanup. No Run/Attempt mutation, Runner transport/command execution, receipt persistence, or Audit publication was added; ADR-0039/ADR-0114/P4 remain gated.
- **DONE — §601 Scheduler selection previews bind durable owner-scoped Run references**: Accepted `EXECUTE + P4` scheduler selection preview now re-reads the owner-scoped Runtime Run before evaluating inventory or returning a candidate; missing, foreign, malformed, unavailable, cycling, and overlong references fail closed. Production supplies the Runtime backend, and policy-complete preview shares the owner-bound policy image used by lease evaluation. No Run/Attempt or lease mutation, Runner transport/command execution, receipt persistence, or Audit publication was added; ADR-0039/ADR-0114/P4 remain gated.
- **DONE — §602 Inventory/resource convergence compares complete capacity declarations**: Forge Runtime CLI/TUI and Snaplink Console now compare owner, lifecycle, OS/architecture, CPU, memory, storage, and GPU count/available-memory summaries between paired inventory/resource observations, in addition to revision/generation/heartbeat counters. Drift fails closed with no inventory, enrollment/heartbeat, selection/reservation, scheduler, Runner, receipt, or Audit authority; ADR-0039/ADR-0114/P4 remain gated.
- **DONE — §603 Console scheduler candidates respect selected client-instance scope**: Snaplink Console's shared Sessions surface now clears scheduler preview, lease claim, renewal, and release candidates without POST when the selected instance lacks the Conversation, the projection is unavailable, or Conversation/Run selection drifts. The default Web/App/Mobile Gate remains request-free; no instance registration/heartbeat, inventory, lease, Runner, receipt, or Audit authority was added; ADR-0039/ADR-0114/P4 remain gated.
- **DONE — §604 TUI session refresh commits only the selected client-instance projection**: Forge Runtime TUI validates the selected client-instance projection before requesting Conversations, filters each page to the declared `session_ids`, and commits only visible rows. A missing or invalid projection fails closed before any request; owner authorization and server pagination remain unchanged. This is display-scope validation only with no registration/heartbeat, inventory, Prompt/session storage, selection/reservation, scheduler lease, Runner transport/execution, receipt, or Audit authority; ADR-0039/ADR-0114/P4 remain gated.
- **DONE — §605 Cross-ecosystem Attempt lifecycle receiver parity**: The canonical `forge.attempt-lifecycle/v1` graph is mirrored into Aero-ID, Aero-IM, Aero-Vault, and Snaplink Audit Governance. Receivers strictly decode the envelope, reject unknown/duplicate/trailing JSON, recompute accepted/rejected transitions, and keep every authority predicate false; mutation tests reject lifecycle or authority drift. This is offline compatibility evidence only with no Attempt persistence, lease/device authority, selection/reservation, Runner transport/command execution, receipt persistence, or Audit publication; ADR-0039/ADR-0114/P4 remain gated.
- **DONE — §606 Cross-ecosystem Attempt boundary rejection-reason parity**: The four offline `forge.runner-attempt-boundary/v1` receivers now recompute Core's exact non-ready reasons and fail closed on omitted or invented values; ready observations still require no reasons. No Attempt/lease persistence, target selection, reservation, Runner transport/execution, receipt persistence, or Audit publication was added; ADR-0039/ADR-0114/P4 remain gated.
- **DONE — §607 TUI client-instance pair refresh fails closed until convergence**: Runtime TUI commits selection only after paired client-instance session/resource observations converge and retains the previous pair without advancing the change cursor on drift. No registration/heartbeat, inventory, Prompt/session storage, target selection/reservation, scheduler lease, Runner transport/execution, receipt, or Audit authority was added; ADR-0039/ADR-0114/P4 remain gated.
- **DONE — §608 Console refreshes the selected client-instance projection before owner reads**: Shared Web/App/Mobile Sessions refreshes the explicit instance projection before owner change, Conversation, Prompt, or Run reads and fails closed on stale/revoked selection; the default Gate remains request-free. No registration/heartbeat, inventory, selection/reservation, scheduler, Runner, receipt, or Audit authority was added; ADR-0039/ADR-0114/P4 remain gated.
- **DONE — §609 Runtime CLI validates ordinary Prompt append responses**: The normal `remote prompts add` response is strictly decoded and bound to the requested Conversation, Prompt, content, replay marker, and sequential CAS version before it is returned. No Run/device/lease/Runner/receipt/Audit authority was added; ADR-0039/ADR-0114/P4 remain gated.
- **DONE — §610 Console validates ordinary Prompt append responses**: Snaplink Console's ordinary `appendPrompt` path now rejects empty, overlong, or control-bearing Prompt identities and preserves exact Conversation, role, content, timestamp, replay-marker, and sequential-CAS binding before exposing the response to Web/App/Mobile. No Run/device/lease/Runner/receipt/Audit authority was added; ADR-0039/ADR-0114/P4 remain gated.
- **DONE — §611 Runtime validates remote Runner receipt/reconciliation previews at the client boundary**: The Runtime CLI now revalidates Conversation/Run URL bindings, request echoes, canonical receipt-history reductions, reconciliation projections, and preview-only authority for remote session Runner receipt, history, and reconciliation methods. Foreign or drifted responses fail closed; no Runner transport, command execution, registration, heartbeat, lease, or Audit authority was added; ADR-0039/ADR-0114/P4 remain gated.
- **DONE — §612 Console refreshes selected instance/resource proof before Prompt writes**: Opt-in Web/App/Mobile Prompt submission now force-refreshes the selected client-instance projection and configured inventory/resource convergence before invoking the submitter. Revoked sessions, stale/error projections, or resource drift fail closed without a Prompt POST; the default Gate remains request-free. No registration/heartbeat, Run, reservation, scheduler, Runner, receipt, or Audit authority was added; ADR-0039/ADR-0114/P4 remain gated.
- **DONE — §613 Runtime binds scheduler lease and reconciliation responses to requests**: Runtime's authenticated scheduler selection, lease claim/renew/release, and execution-reconciliation clients now validate request shape, Conversation/Run/Attempt/target/epoch bindings, canonical reductions, and preview-only authority at the HTTP client boundary. Foreign or drifted responses fail closed; no unrestricted dispatch, Runner transport, command execution, registration, heartbeat, or Audit authority was added; ADR-0039/ADR-0114/P4 remain gated.
- **DONE — §614 Core binds Conversation detail responses to the requested ID**: Forge Core's owner-scoped Runtime bridge and Conversation detail route now require the returned Conversation identity to equal the requested ID before exposing it to CLI/TUI/Web/App/Mobile consumers. A foreign backend response fails closed as an invalid runtime response; no session, Prompt, Run, device, lease, Runner, registration, heartbeat, or Audit authority was added; ADR-0039/ADR-0114/P4 remain gated.
- **DONE — §615 Console requires the next fencing epoch on scheduler renewal**: Snaplink Console's shared Web/App/Mobile scheduler-lease renewal adapter now requires the returned grant epoch to equal the submitted epoch plus one, matching Runtime's request-bound validator. A response that skips a fencing step fails closed before reaching the Sessions surface; the single-shot POST, target binding, and default-off candidate gate remain unchanged. No new lease authority, Runner transport, command execution, registration, heartbeat, or Audit effect was added; ADR-0039/ADR-0114/P4 remain gated.
- **DONE — §616 Runtime TUI accepts server-authenticated owner claims without requiring `client_id`**: Forge Runtime's content-free Prompt receipt projection now derives its local owner binding from the validated JWT issuer, subject, tenant, Forge audience/scopes, and expiry. Explicit access tokens used by CLI/TUI can therefore complete Prompt writes when the server-authenticated JWT omits `client_id`; saved-login credential storage still requires that client binding. The shared-session TUI E2E now passes through the real Go Coordinator and Rust Hub; no Run, device, lease, Runner, registration, heartbeat, or Audit authority was added; ADR-0039/ADR-0114/P4 remain gated.
- **DONE — §617 Runtime TUI inventory/resource convergence command**: `inventory show-converged` now performs one authenticated v2 inventory GET plus one client-instance/resource-view GET, validates the complete owner/device/revision/generation/heartbeat/capacity join, renders both observations, and commits them atomically to local TUI state. Drift keeps prior snapshots and never renders a mixed pair; no registration/heartbeat, inventory mutation, target selection, reservation, scheduler lease, Runner transport/execution, receipt, or Audit authority was added; ADR-0039/ADR-0114/P4 remain gated.
- **DONE — §618 Runtime TUI inventory convergence reconciles selected instance state**: After a successful `inventory show-converged` refresh, the TUI now reconciles the local client-instance filter against the newly committed resource view and clears selected Prompt/Run panels when the instance no longer declares the selected session. This closes a stale local projection edge only; no registration/heartbeat, inventory mutation, target selection, reservation, scheduler lease, Runner transport/execution, receipt, or Audit authority was added; ADR-0039/ADR-0114/P4 remain gated.
- **DONE — §619 Runtime TUI keeps paired client-instance refreshes atomic**: When both client-instance session and resource views are explicitly open, TUI `sync` now reads and validates both observations before rendering or committing either one. A non-authority failure on the second read retains the previous validated pair and leaves the change cursor untouched; a single-view refresh keeps its prior affected-view clearing behavior, and authorization failures still clear the owner view. No registration/heartbeat, inventory mutation, target selection, reservation, scheduler lease, Runner transport/execution, receipt, or Audit authority was added; ADR-0039/ADR-0114/P4 remain gated.
- **DONE — §620 Conversation creation responses remain bound across Runtime, Core, and Console**: Forge Runtime, Forge Core, and Snaplink Console now require an authenticated Conversation creation response to preserve the requested scope and exact title while carrying a structurally valid identity and JSON-safe timestamps. A valid but foreign or drifted response fails closed before CLI/TUI/Web/App/Mobile state can select it; no new session authority, device inventory, target selection, lease, Runner transport/execution, receipt, or Audit behavior was added; ADR-0039/ADR-0114/P4 remain gated.
- **DONE — §621 Runtime binds remote Run execution-evidence previews at the client boundary**: Authenticated Runtime CLI/TUI execution-evidence preview now validates the supplied Run/receipt pair against the URL before POST and revalidates the returned content-free evidence against the same pair, canonical reduction, and display-only authority before returning it. Foreign, drifted, or authority-bearing responses fail closed even for direct client callers; no Runner transport, command execution, receipt persistence, device selection, lease, registration, heartbeat, or Audit authority was added; ADR-0039/ADR-0114/P4 remain gated.
- **DONE — §622 Runtime binds remote session device-observation previews at the client boundary**: Authenticated Runtime CLI/TUI session device-observation preview now validates the caller-supplied declaration before POST, requires its Conversation/Run pair to match the URL, and revalidates the canonical response against that request before returning it. Foreign, drifted, malformed, or authority-bearing responses fail closed even for direct client callers; no live inventory, enrollment/heartbeat, target selection, reservation, scheduler lease, Runner transport/command execution, receipt, or Audit authority was added; ADR-0039/ADR-0114/P4 remain gated.
- **DONE — §623 Runtime binds offline placement dry-run previews at the client boundary**: Authenticated Runtime CLI/TUI placement preview now validates the caller declaration before POST and canonicalizes the returned offline result against the exact owner, timestamp, device set, derived decisions, and all-false authority before returning it. Foreign, drifted, malformed, or authority-bearing responses fail closed even for direct client callers; no target selection, reservation, scheduler lease, dispatch, Runner transport/command execution, receipt, inventory mutation, enrollment/heartbeat, or Audit authority was added; ADR-0039/ADR-0114/P4 remain gated.
- **DONE — §624 Runtime binds registry placement previews at the client boundary**: Authenticated Runtime CLI/TUI registry placement preview now validates the requirements request before POST and canonicalizes the returned owner-scoped v2 evaluation before returning it. Selected targets, authority elevation, malformed decisions, and response drift fail closed even for direct client callers; no target selection, reservation, scheduler lease, dispatch, Runner transport/command execution, receipt, inventory mutation, enrollment/heartbeat, or Audit authority was added; ADR-0039/ADR-0114/P4 remain gated.
- **DONE — §625 Runtime binds Runner dispatch-admission previews at the client boundary**: Authenticated Runtime CLI/TUI dispatch-admission preview now validates the full lease-bound request and requires its Conversation/Run pair to match the URL before POST, then revalidates the returned admission against the same request, command digest, lease proof, attempt state, and all-false dispatch authority. URL/request binding drift, foreign or malformed responses, digest drift, and authority elevation fail closed even for direct client callers; no Runner transport, command execution, dispatch effect, enrollment/heartbeat, receipt, or Audit authority was added; ADR-0039/ADR-0114/P4 remain gated.
- **DONE — §626 Runtime binds Run/Attempt/lease dispatch preflight at the client boundary**: Authenticated Runtime CLI/TUI preflight now validates the complete request and requires its Conversation/Run pair to match the URL before POST, then revalidates the returned candidate/lease/intent projection against the same request and all-false authority. URL/request binding drift, foreign or malformed responses, selected-target/lease/intent drift, and authority elevation fail closed even for direct client callers; no live enrollment/heartbeat, Runner transport/command execution, dispatch effect, receipt, or Audit authority was added; ADR-0039/ADR-0114/P4 remain gated.
- **DONE — §627 Runtime binds Runner transport-admission previews at the client boundary**: Authenticated Runtime CLI/TUI transport-admission preview now validates the lease/command/transport request and requires its Conversation/Run pair to match the URL before POST, then revalidates transport path/payload, lease and command binding, preview-only state, and all-false authority before returning it. URL/request drift, foreign responses, path/payload drift, malformed values, or authority elevation fail closed even for direct client callers; no transport opening, heartbeat, command execution, dispatch effect, enrollment, receipt, or Audit authority was added; ADR-0039/ADR-0114/P4 remain gated.
- **DONE — §628 Runtime binds Runner execution-intent previews at the client boundary**: Authenticated Runtime CLI/TUI execution-intent preview now validates the owner/Conversation/Prompt/Run/Attempt/target/command request and requires its Conversation/Run pair to match the URL before POST, then revalidates the exact canonical intent response with preview-only and all-false authority. Foreign owner, Prompt, Run, Attempt, target, command, URL, or authority drift fails closed even for direct client callers; no command execution, Runner opening, heartbeat, dispatch effect, enrollment, receipt, or Audit authority was added; ADR-0039/ADR-0114/P4 remain gated.
- **DONE — §629 Runtime binds Runner execution-boundary previews at the client boundary**: Authenticated Runtime CLI/TUI execution-boundary preview now validates the owner/Conversation/Run/Attempt/command/transport request and requires its Conversation/Run pair to match the URL before POST, then revalidates readiness, transport binding, command/target identity, preview-only state, and all-false authority in the canonical response. URL/request drift, foreign binding, readiness drift, or authority elevation fails closed even for direct client callers; no Runner opening, transport, command execution, dispatch effect, enrollment/heartbeat, receipt, or Audit authority was added; ADR-0039/ADR-0114/P4 remain gated.
- **DONE — §630 Runtime binds local Runner execution-readiness previews at the client boundary**: Authenticated Runtime CLI/TUI local Runner readiness preview now validates the owner/Conversation/Prompt/Run/Attempt/target/command/lease request and requires its Conversation/Run-intent pair to match the URL before POST, then revalidates the canonical metadata-only receipt, executor invocation marker, preview-only state, and all-false authority. URL/request drift, foreign identity, receipt binding, or authority elevation fails closed even for direct client callers; no production execution, Runner transport, dispatch effect, enrollment/heartbeat, receipt persistence, or Audit authority was added; ADR-0039/ADR-0114/P4 remain gated.
- **DONE — §631 Runtime binds Runner Attempt-boundary previews at the client boundary**: Authenticated Runtime CLI/TUI Attempt-boundary preview now validates the owner/Conversation/Run/Attempt/command/transport/transition request and requires its Conversation/Run pair to match the URL before POST, then revalidates lifecycle transition, owner and command/target/epoch binding, preview-only state, and all-false authority in the canonical response. URL/request drift, foreign identity, lifecycle drift, or authority elevation fails closed even for direct client callers; no Attempt persistence, lease mutation, reservation, dispatch, Runner transport/command execution, enrollment/heartbeat, receipt, or Audit authority was added; ADR-0039/ADR-0114/P4 remain gated.
- **DONE — §632 Runtime binds Runner dispatch-plan previews at the client boundary**: Authenticated Runtime CLI/TUI dispatch-plan preview now strictly validates the nested plan envelope before POST, including owner/Conversation/Run/Attempt/intent/lease relationships, then revalidates the canonical candidate, lease state, selected-target null, and all-false authority against that same plan. URL/request drift, candidate or lease summary drift, selection, malformed values, or authority elevation fail closed for direct client callers; no scheduler effect, lease mutation, dispatch, Runner transport/command execution, enrollment/heartbeat, receipt, or Audit authority was added; ADR-0039/ADR-0114/P4 remain gated.
- **DONE — §633 Runtime binds session Runner receipt observation requests at the client boundary**: The authenticated Runtime receipt observation client now reuses the canonical `validate_value` and `conversation_and_run` checks before POST, requiring the caller's Conversation/Run pair to match the URL and rejecting malformed observations without transport. Existing response canonical, binding, and all-false authority checks remain in place; direct client mock coverage rejects URL drift, malformed requests, foreign response binding, and authority drift. This remains a read-only observation preview with no receipt persistence, Runner/dispatch/heartbeat/enrollment authority; ADR-0039/ADR-0114/P4 remain gated.
- **DONE — §634 Runtime binds session Runner receipt history requests at the client boundary**: The authenticated Runtime receipt-history client now reuses the canonical `validate_value` and `conversation_and_run` checks before POST, requiring the caller's Conversation/Run pair to match the URL and rejecting malformed histories without transport. Existing response canonical reduction, binding, and all-false authority checks remain in place; direct client coverage rejects Conversation/Run URL drift and malformed requests. This remains a read-only history preview with no receipt persistence, Runner/dispatch/heartbeat/enrollment authority; ADR-0039/ADR-0114/P4 remain gated.
- **DONE — §635 Runtime binds session Runner reconciliation requests at the client boundary**: The authenticated Runtime reconciliation client now reuses the canonical receipt-history `validate_value` and `conversation_and_run` checks before POST, requiring the caller's Conversation/Run pair to match the URL and rejecting malformed histories without transport. Existing response canonical projection validation and all-false authority checks remain in place; direct client coverage rejects Conversation/Run URL drift and malformed requests. This remains a read-only reconciliation preview with no receipt persistence, Runner/dispatch/heartbeat/enrollment authority; ADR-0039/ADR-0114/P4 remain gated.
- **DONE — §636 Runtime binds Conversation import responses at the client boundary**: The authenticated Runtime import client preserves the existing title/prompts/Idempotency-Key wire shape while requiring an exact four-field response envelope. The returned Conversation must satisfy the strict Global-scope identity/title/timestamp validator, aggregate_version must be positive and JSON-safe, imported_prompt_count must match the submitted transcript, and replayed must be boolean; unknown, malformed, foreign, or binding-drift responses fail closed. Existing confirm-preview validation reuses the same helper. This remains storage-only Conversation import with no device/Runner authority; ADR-0039/ADR-0114/P4 remain gated.
- **DONE — §637 Core binds Runtime bridge Prompt CAS receipts**: Forge Core's Runtime bridge now requires an owner-scoped Prompt append response to return exactly `expected_version + 1` within the JSON-safe ceiling before exposing the typed result. Stale, skipped, or unsafe receipts fail closed for direct bridge callers while the HTTP route keeps its final transport check; no Run/device authority, lease, scheduling, dispatch, Runner transport/execution, registration/heartbeat, receipt persistence, or Audit publication was added; ADR-0039/ADR-0114/P3b/P4 remain gated.
- **DONE — §638 Core/Runtime/Console bind fresh pending Run-intent Prompt CAS receipts**: Forge Runtime's authenticated CLI/TUI client, Forge Core's Runtime bridge and accepted execution HTTP boundary, and Snaplink Console's shared Web/App/Mobile API now require a positive JSON-safe `expected_version` and a fresh `SubmitOwnedPromptRunIntent` receipt at exactly `expected_version + 1`. Stale, skipped, zero, or unsafe fresh receipts fail closed before a shared-session projection advances, while replayed submissions preserve their historical Prompt, intent, profile, and aggregate receipt. No Run/device authority, target selection, lease, scheduling, dispatch, Runner transport/execution, registration/heartbeat, receipt persistence, or Audit publication was added; ADR-0039/ADR-0114/P3b/P4 remain gated.
- **DONE — §639 Console drops a selected session when owner refresh no longer proves it**: Snaplink Console's shared Web/App/Mobile Sessions surface now revalidates a selected Conversation through the owner-scoped detail read when a refreshed first page omits it. Only a current owner response keeps the selection; deleted, revoked, foreign, or unverifiable rows clear the selection and private Prompt/Run details while retaining the fresh page. No device enrollment/heartbeat, inventory, Run, scheduling, lease, Runner transport/execution, receipt persistence, or Audit authority was added; ADR-0039/ADR-0114/P3b/P4 remain gated.
- **DONE — §640 Runtime TUI drops a selected session when owner refresh no longer proves it**: Forge Runtime's interactive TUI now revalidates a selected Conversation through the authenticated owner detail route when a refreshed first page omits it. Only a successful current owner response keeps the selected entry and its Prompt/Run projection; deleted, revoked, foreign, or unverifiable detail responses commit the fresh page, clear the selected entry and private history, and fail closed. No device enrollment/heartbeat, inventory authority, Run, scheduling, lease, Runner transport/execution, receipt persistence, or Audit authority was added; ADR-0039/ADR-0114/P3b/P4 remain gated.
- **DONE — §641 Inventory/resource joins bind Runner observation time**: Forge Runtime CLI/TUI, Snaplink Console Web/App/Mobile, and the mirrored Aero-ID/Aero-IM/Aero-Vault/Audit Governance consumers now require inventory v2 `device.snapshot_observed_at_ms` to equal the paired resource-view device `observed_at_ms`. Owner, Runner identity, lifecycle counters, and capacity matches with a stale or cross-snapshot observation time fail closed; standalone client-instance time remains independent. This is read-only freshness binding only and adds no enrollment/heartbeat, inventory write, target selection, reservation, scheduler lease, dispatch, Runner transport/execution, receipt persistence, or Audit authority; ADR-0039/ADR-0114/P3b/P4 remain gated.
- **DONE — §642 Runtime TUI refreshes the selected instance before private session reads**: Forge Runtime's interactive TUI now refreshes an explicitly selected client-instance/session or resource observation before owner Conversation, Prompt history, Run timeline, or pending Run-intent reads. A revoked, failed, or non-converged observation blocks that sync before private reads can use the previous instance image; with no active instance filter, the default TUI remains request-free. This is a local owner/session projection boundary only and adds no enrollment/heartbeat, inventory write, target selection, reservation, scheduling, lease, dispatch, Runner transport/execution, receipt persistence, or Audit authority; ADR-0039/ADR-0114/P3b/P4 remain gated.
- **DONE — §643 Aero mirrors reject malformed client-instance rows before convergence**: Aero-ID, Aero-Vault, and Snaplink Audit Governance receiver mirrors now validate the owner tuple and each session/resource client-instance row before exact convergence comparison. Bounded/canonically ordered instance and session identifiers, accepted client kinds/statuses, positive JSON-safe observation times, and non-null arrays are required, so identical malformed rows cannot appear converged; no enrollment/heartbeat, inventory write, target selection, reservation, scheduling, lease, dispatch, Runner transport/execution, receipt persistence, or Audit authority was added; ADR-0039/ADR-0114/P3b/P4 remain gated.
- **DONE — §644 Runtime TUI and Console drop selected sessions after private read rejection**: Runtime TUI and Snaplink Console now remove the cached selected Conversation and clear its Prompt/Run projection after a deterministic owner rejection or foreign/malformed Prompt, Run, or Run-timeline response. Other owner rows remain available, while transient timeout, rate-limit, and server failures retain the existing retry/stale-error path. Prompt pagination and Run timeline reads use the same fail-closed boundary; no enrollment/heartbeat, inventory, target selection, reservation, scheduling, lease, Runner transport/execution, receipt persistence, or Audit authority was added; ADR-0039/ADR-0114/P3b/P4 remain gated.
- **DONE — §645 Aero-IM rejects malformed client-instance rows before convergence**: Aero-IM's receiver-side client-instance/session/resource contract now applies bounded owner and identifier syntax, accepted client-kind and lifecycle-status vocabularies, and positive JSON-safe observation times before a composed view can be treated as converged. Resource device and Runner identifiers follow the same bounds, and identical malformed rows fail closed in both source views; no enrollment/heartbeat, inventory write, target selection, reservation, scheduling, lease, dispatch, Runner transport/execution, receipt persistence, or Audit authority was added; ADR-0039/ADR-0114/P3b/P4 remain gated.
- **DONE — §646 Runtime CLI binds instance-filtered private reads to converged resource observations**: Forge Runtime CLI `remote sessions`, `prompts`, `runs`, and pending Run-intent commands now read both owner-bound session-view and resource-view candidates when `--instance` is used without a local `--instance-view`, requiring identical owner/client-instance rows before any private Conversation, Prompt, Run, or pending Run-intent request. Resource drift fails closed while explicit local files remain strict offline display declarations; no enrollment/heartbeat, inventory write, target selection, reservation, scheduling, lease, dispatch, Runner transport/execution, receipt persistence, or Audit authority was added; ADR-0039/ADR-0114/P3b/P4 remain gated.
- **DONE — §647 Runtime TUI binds explicit inventory/resource observations before shared-session writes and scheduler previews**: When both inventory v2 and client-instance resource observations are explicitly open, Runtime TUI Prompt and pending Run-intent submission/retry plus scheduler selection preview, lease claim, and lease renewal now require the existing owner/device/lifecycle/capacity/GPU/observation-time convergence check. Drift fails locally before a request or pending write; one-sided/no observations preserve compatibility and lease release remains available for cleanup. This is read-only planning hygiene with no enrollment/heartbeat, target selection, reservation, dispatch, Runner transport/execution, receipt, or Audit authority; ADR-0039/ADR-0114/P3b/P4 remain gated.
- **DONE — §648 Core binds Conversation import receipts at the HTTP boundary**: Forge Core's authenticated Conversation import route now revalidates the typed backend receipt against the submitted title and transcript size before serializing it to CLI, TUI, Web, App, or Mobile consumers. The receipt must carry a bounded Conversation ID, Global scope, the exact title, JSON-safe timestamps and aggregate version, and an imported prompt count equal to the request; foreign or drifted backend receipts fail closed even when a non-Rust or injected backend bypasses the Runtime bridge validator. This remains storage-only Conversation import with no device enrollment/heartbeat, inventory, target selection, reservation, scheduling, lease, dispatch, Runner transport/execution, receipt persistence, or Audit authority; ADR-0039/ADR-0114/P3b/P4 remain gated.
- **DONE — §649 Console keeps owner inventory proof across client-instance filter changes**: Snaplink Console now clears only private Conversation/Prompt/Run state when a local client-instance filter changes, preserving the owner-scoped v1/v2 inventory and registry observations needed by the explicit resource-convergence guard. Concurrent opt-in v2 inventory and client-resource refreshes are coalesced; a write boundary reuses the current validated converged pair, while missing/loading/stale/failed/drifted reads remain fail-closed. Explicit candidate HTTP readers use wall-clock deadlines so real IO is not expired by a caller's virtual frame clock. This is observation-state hygiene only and adds no enrollment/heartbeat, inventory authority, target selection, reservation, scheduling, lease, dispatch, Runner transport/execution, receipt persistence, or Audit authority; ADR-0039/ADR-0114/P3b/P4 remain gated.
- **DONE — §650 Console revokes private state across shared-session instance filters**: Snaplink Console's shared Web/App/Mobile Sessions surface now invalidates private Prompt/Run details on every client-instance filter transition, including when the same Conversation remains declared by both instances, marks the selected owner session for a subsequent authenticated re-read, and retains owner-scoped inventory/resource observations. Regression coverage proves the shared session panel survives, hidden Prompt/Run state is absent, and a refreshed instance projection cannot resurrect stale private metadata. No enrollment/heartbeat, inventory authority, target selection, reservation, scheduling, lease, dispatch, Runner transport/execution, receipt persistence, or Audit authority was added; ADR-0039/ADR-0114/P3b/P4 remain gated.
- **DONE — §651 Runtime TUI revokes private state across instance filters**: Forge Runtime TUI now clears Prompt history, selected Run/timeline/observation, and pending Run-intent page/timeline metadata when a validated local client-instance filter actually changes, including instance-to-instance, clear, and instance-to-scope transitions. It preserves the selected owner Conversation, owner-scoped inventory/resource/client-instance observations, and pending Prompt/Run-intent/create writes; invalid or unknown instance selectors leave state and transport untouched. The 15 focused and 152 full TUI tests prove the shared-session, zero-request boundary. No enrollment/heartbeat, inventory authority, target selection, reservation, scheduling, lease, dispatch, Runner transport/execution, receipt persistence, or Audit authority was added; ADR-0039/ADR-0114/P3b/P4 remain gated.
- **DONE — §652 Console rejects divergent independent client-instance observations**: Snaplink Console Web/App/Mobile now requires independently configured session-view and resource-view readers to match owner declarations and the complete client-instance row image before deriving an instance filter or rehydrating private Conversation/Prompt/Run state. Missing, loading, stale, failed, or drifted pairs remain fail-closed; a divergent observation may remain display-only but cannot drive the filter or private reads, and scheduled-refresh coverage proves a changed observation time leaves the session projection empty without a second Prompt read. Final visibility checks also revoke an in-flight Run or pending Run-intent response that returns after the selected pair enters a reader gap. The combined reader and default Gate behavior remain unchanged; no enrollment/heartbeat, inventory mutation, target selection, reservation, scheduling, lease, dispatch, Runner transport/execution, receipt persistence, or Audit authority was added; ADR-0039/ADR-0114/P3b/P4 remain gated.
- **DONE — §653 Runtime TUI exposes non-converged client-instance state**: Forge Runtime TUI now retains the last owner-bound session/resource observation metadata for display while rendering an explicit non-convergence warning that blocks an active client-instance filter and its private Prompt/Run projection until both snapshots converge. A missing side or refresh failure keeps the warning and prevents a mixed pair; a later converged refresh clears it. With no active instance filter, the existing owner-read behavior remains compatible. Focused client-instance tests (51) and instance-filter tests (15) cover drift, failed second reads, retained metadata, recovery, and zero-request private-read blocking. No enrollment/heartbeat, inventory mutation, target selection, reservation, scheduling, lease, dispatch, Runner transport/execution, receipt persistence, or Audit authority was added; ADR-0039/ADR-0114/P3b/P4 remain gated.
- **DONE — §654 Forge Core closes the execution-adjacent production route matrix**: Table-driven constructor and configured-`Run` regressions probe Runner execution-intent, dispatch-plan, dispatch-admission, transport-admission, execution-boundary, Attempt-boundary, reconciliation, and evidence previews plus scheduler selection and lease claim/renew/release paths. All 12 remain the standard 404 under ordinary authenticated construction and the real configured server; explicit device-fabric EXECUTE assembly remains the only mount seam. No Runner, reservation, dispatch, lease mutation, transport, execution, receipt, or Audit authority was enabled; ADR-0039/ADR-0114/P4 remain gated.
- **DONE — §655 Runtime TUI closes the instance-filter gap for Runner metadata previews**: All fourteen selected-session Runner metadata/preview commands now call the existing client-instance visibility guard before their first request, including local Runner readiness and execution/Attempt, admission, receipt, reconciliation, evidence, preflight, and lease paths. Hidden or non-converged sessions stop locally with no request; no-filter behavior remains compatible. No enrollment/heartbeat, inventory authority, target selection, reservation, scheduling, lease mutation, dispatch, Runner transport/execution, receipt persistence, or Audit authority was added; ADR-0039/ADR-0114/P3b/P4 remain gated.
- **DONE — §656 Console atomically converges Run and Runner-receipt observations**: Snaplink Console's shared Web/App/Mobile Sessions Gate now forwards an explicit independent receipt reader only as an opt-in companion to the Run reader. The Screen strictly re-decodes and atomically accepts the pair only when owner, Conversation, Prompt, Run summary, Attempt, Command, target, digest, disposition, observation time, and reconciliation metadata converge; failures and late responses revoke both halves, and execution evidence uses the converged receipt. The default Gate remains request-free and no receipt persistence, device selection, reservation, scheduling, lease, dispatch, Runner transport/execution, enrollment/heartbeat, or Audit authority was added; ADR-0039/ADR-0114/P4 remain gated.

- **DONE — §657 Native Mobile client-instance session/resource acceptance**: Android and iOS native acceptance inputs now carry an explicit `client_instance_id` plus owner-bound session/resource observations. Strict Python, Kotlin, Swift, and Flutter validation rejects missing or hidden Conversations, non-mobile rows, owner/instance-row drift, non-display-only authority, malformed device capacity, and unsafe extra fields before the first Conversation or Prompt request. The host-side Go → Snaplink JWT → test-only candidate route → Flutter journey mounts only read candidates, proves both native cold starts read the converged pair before listing or writing the shared Conversation, and keeps device writes, Run-intents, placement, dispatch, and execution paths untouched. Production constructors remain default-off; no enrollment/heartbeat, inventory mutation, scheduling, Runner, receipt, or Audit authority was added; ADR-0039/ADR-0114/P4 remain gated.
- **DONE — §658 Runtime CLI instance-filtered execution-consent preview**: Forge Runtime `remote execution-consent preview` now accepts an explicit `--instance` and optional strict local `--instance-view`. Online filtering reads the paired authenticated client-instance session/resource views before the consent GET, rejects hidden membership or observation drift without sending a consent request, and keeps local-view mode request-free; the existing unfiltered consent preview remains unchanged. This is a display/planning boundary only: no grant, Run, target, reservation, lease, dispatch, Runner, receipt, enrollment, heartbeat, or execution authority was added; ADR-0039/ADR-0114/P4 remain gated.
- **DONE — §659 Optional device-observation scope/profile parity**: The canonical Forge/Snaplink profile now names `forge:devices:read` separately and carries a disabled `forge-device-observer` profile whose scopes combine Conversation read with device observation. Snaplink registers the optional scope and inactive observer while keeping `forge-cli` and `forge-console` conversation-only; owner-parity tests reject device scope on default clients and verify the opt-in observer's owner/audience tuple. Forge Core, Runtime, and Console strict consumers stay aligned, and Console exposes the observer profile without using it by default. No production session/resource route, enrollment, inventory authority, scheduling, dispatch, Runner, receipt, or execution authority was enabled; ADR-0039/ADR-0114/P4 remain gated.
- **DONE — §660 Snaplink JWT client-instance pair authorization boundary**: Forge Core's opt-in client-instance convergence E2E now uses a real Snaplink JWT with both read scopes to fetch owner-bound session/resource projections, compares all five CLI/TUI/Web/App/Mobile instance rows and the read-only device observation, then repeats the journey with a conversation-only token. The latter may read session metadata but receives `403` for resource-view before the resource source is called; the ordinary production constructor remains `404` for both paths even for the full-scope token. This proves scope separation and source non-touching without enabling production routes, enrollment, inventory mutation, scheduling, dispatch, Runner, receipt, or execution authority; ADR-0039/ADR-0114/P4 remain gated.
- **DONE — §661 Runtime CLI scheduler preview respects the selected client-instance pair**: `remote placement scheduler-preview` now accepts `--instance INSTANCE_ID` with optional strict local `--instance-view FILE|-`. Online filtering reads the authenticated session/resource pair and requires the requested Conversation to be declared by the converged instance before the scheduler-preview POST; hidden membership, row drift, malformed observations, or invalid selectors fail closed without a candidate request. A local display view avoids candidate reads, while the unfiltered command keeps its single POST. This remains a planning-only comparison with no target selection, reservation, lease, dispatch, Runner, receipt, enrollment, heartbeat, or execution authority; ADR-0039/ADR-0114/P4 remain gated.
- **DONE — §662 Snaplink JWT client-instance scheduler-preview convergence E2E**: Forge Core now has an opt-in real Snaplink JWT journey that reads the five CLI/TUI/Web/App/Mobile instance rows, joins the same owner-bound device/Runner resource image, and posts one planning-only scheduler preview whose selected pair matches that resource image. A token without the scheduler-preview scope is rejected before Run, inventory, policy, or source calls, and the ordinary production constructor remains `404`; the contract gate runs this E2E with its explicit opt-in flag. No reservation, lease, dispatch, Runner execution, enrollment, heartbeat, receipt, or live P4 authority was enabled; ADR-0039/ADR-0114/P4 remain gated.
- **DONE — §663 Console execution-readiness refresh respects the selected client-instance pair**: Snaplink Console's shared Web/App/Mobile local Runner execution-readiness preview now refreshes the selected client-instance session/resource projection and any separately configured inventory/resource observation immediately before invoking its explicit candidate reader. A missing, stale, drifted, or revoked pair clears the readiness preview and prevents the candidate POST; selected Conversation/Run changes remain fail-closed. Focused Flutter coverage proves a second pair refresh hides the selected session and sends no execution-readiness request. This remains a metadata-only preflight with no Runner transport/argv execution, lease mutation, reservation, scheduling, enrollment/heartbeat, receipt persistence, or Audit authority; ADR-0039/ADR-0114/P4 remain gated.
- **DONE — §664 CLI instance-filtered Prompt write joins the converged resource pair**: The real Forge Core session-projection journey now mounts both owner-bound client-instance observations and drives the Rust Runtime CLI through a visible `client-cli-001` Prompt append only after the session/resource pair converges. A second CLI attempt against a Conversation hidden from `client-web-001` is rejected before the Prompt POST, while the existing TUI and shared Web/App/Mobile writes continue to land in the same owner history. No new session or Prompt authority, Run creation, inventory mutation, target selection, reservation, scheduling, lease, dispatch, Runner transport/execution, receipt persistence, enrollment/heartbeat, or Audit authority was added; ADR-0039/ADR-0114/P4 remain gated.
- **DONE — §665 Runtime CLI scheduler lease lifecycle respects the selected client-instance pair**: Forge Runtime `remote placement scheduler-lease`, `scheduler-lease-renew`, and `scheduler-lease-release` now accept `--instance INSTANCE_ID` with an optional strict local `--instance-view FILE|-`. Online claim/renew/release reads the authenticated session/resource pair before the candidate POST and rejects hidden, malformed, or drifted Conversations without sending the lease request; local views avoid candidate reads, while unfiltered commands preserve the legacy POST. Parser, idempotency, visible/hidden guard, response-binding, and TUI fencing-token withholding tests pass. The slice does not enable production lease routes, enrollment/heartbeat, inventory mutation, target selection beyond the explicitly accepted candidate, Runner dispatch/transport/execution, receipt persistence, or Audit authority; ADR-0039/ADR-0114/P4 remain gated.
- **DONE — §666 Runtime CLI instance-filtered Runner dispatch-plan convergence E2E**: Forge Core now drives the real Rust Runtime CLI through a Snaplink JWT with `--instance client-cli-001`, requiring the owner-bound session/resource pair before the planning-only Runner dispatch-plan POST. The journey checks request order, owner and complete five-client row convergence, display-only resource binding, false authority, hidden `client-web-001` rejection before POST, and the ordinary production dispatch-plan route's 404. This adds no target selection, reservation, lease, dispatch, Runner transport/execution, receipt persistence, enrollment/heartbeat, or Audit authority; ADR-0039/ADR-0114/P4 remain gated.
- **DONE — §667 Snaplink JWT Runtime CLI scheduler-lease pair convergence E2E**: Forge Core now drives the real Rust Runtime CLI through the opt-in candidate mux for scheduler lease claim and renewal with `--instance client-cli-001`. Each visible operation reads the owner-bound five-instance session/resource pair before the lease POST, while hidden `client-web-001` claim and renewal fail closed after only the two pair reads. The ordinary production constructor remains `404` for claim, renewal, and release; all response authority remains limited to placement/reservation/lease predicates, with no Runner transport/execution, enrollment/heartbeat, receipt, Audit, or live P4 effect; ADR-0039/ADR-0114/P4 remain gated.
- **DONE — §668 Snaplink JWT Runtime CLI scheduler-lease release pair convergence E2E**: The same opt-in candidate-mux journey now drives `scheduler-lease-release --instance client-cli-001` through a real Snaplink JWT after the renewed fenced proof is returned. The visible release reads session-view then resource-view before one release POST and validates the owner, Conversation/Run/Attempt, target, epoch, release timestamp, and zero authority; hidden `client-web-001` release reads only the pair and fails before POST. The ordinary production constructor remains `404` for claim, renewal, and release; no Runner transport/execution, enrollment/heartbeat, receipt, Audit, or live P4 effect was enabled; ADR-0039/ADR-0114/P4 remain gated.
- **DONE — §669 Console dispatch-plan preview revalidates the selected session/resource pair before POST**: Snaplink Console's Web/App/Mobile Sessions Gate now refreshes the selected client-instance session and resource observations immediately before the opt-in Runner dispatch-plan candidate. A revoked or drifted Conversation in either side clears the candidate and sends no dispatch-plan request; focused Flutter coverage proves both readers are called again and the POST remains absent. The default Gate remains request-free, and no Runner transport/execution, enrollment/heartbeat, lease mutation, reservation, or live P4 effect was enabled; ADR-0039/ADR-0114/P4 remain gated.

- **DONE — §670 Runtime TUI real JWT inventory/resource and client-instance Runner candidate convergence**: Forge Core now drives the real Rust Runtime TUI through an opt-in Snaplink JWT journey that reads v2 inventory, converges client-instance session/resource views, selects `client-tui-001`, and posts one planning-only Runner dispatch-plan candidate. A hidden `client-web-001` projection stops before the candidate POST after the same owner-bound reads, while the ordinary production dispatch-plan route remains `404`. This proves the TUI candidate chain without enabling target selection, reservation, lease, dispatch, Runner transport/execution, receipt persistence, enrollment/heartbeat, or Audit authority; ADR-0039/ADR-0114/P4 remain gated.
- **DONE — §671 Console Sessions Gate accepts a real JWT client-instance pair before dispatch-plan POST**: Forge Core now drives Snaplink Console's shared Web/App/Mobile Sessions Gate against a real JWT candidate API. The visible `client-cli-001` path refreshes and validates both session and resource observations before exactly one dispatch-plan POST; hidden `client-web-001` and session/resource drift fail closed after the pair reads with no POST. The ordinary production session/resource/dispatch constructors remain `404`; no Runner transport/execution, enrollment/heartbeat, reservation/lease mutation, receipt, Audit, or live P4 authority was enabled; ADR-0039/ADR-0114/P4 remain gated.
- **DONE — §672 Runtime TUI scheduler-preview pair convergence**: Forge Core now drives the real Rust Runtime TUI with a Snaplink JWT through `client-instances show-converged`, an explicit client-instance filter, and one planning-only scheduler-preview POST. The visible `client-web` path proves conversation-list startup plus session/resource pair reads before the POST; hidden `client-tui` membership reads the same pair and fails closed without a scheduler request. The ordinary production scheduler-preview constructor remains `404`; no target selection, reservation, lease, Runner, dispatch, receipt, enrollment/heartbeat, Audit, or live P4 authority was enabled; ADR-0039/ADR-0114/P4 remain gated.
- **DONE — §673 Runtime TUI scheduler-lease claim pair convergence**: Forge Core now drives the real Rust Runtime TUI with a Snaplink JWT through session-view and explicit resource-view reads before a visible client-instance scheduler-lease claim. The candidate uses a second eligible resource after the CLI lifecycle's historical fencing epochs, renders only the redacted lease receipt, and proves a hidden instance stops after the pair reads with no lease POST. The ordinary production claim/renew/release constructors remain `404`; no Runner transport/execution, enrollment/heartbeat, receipt persistence, Audit publication, or live P4 authority was enabled; ADR-0039/ADR-0114/P4 remain gated.
- **DONE — §674 Console Web/App/Mobile Prompt writes converge independent session/resource readers**: The real JWT Console journey now enables both candidate readers for each Web, desktop App, and Mobile instance. The screen scrolls the long projection list before filtering, refreshes both observations before the visible owner-scoped Prompt POST, and proves a hidden Conversation never receives a Prompt POST. A widget regression covers the no-initial-instance case with independent readers; this remains display/read convergence and storage-only Prompt authority, with production candidate routes still disabled.
- **DONE — §675 Console scheduler-preview pair gate**: Snaplink Console's opt-in scheduler-preview candidate now has focused coverage proving the selected instance re-reads independent session/resource observations before one planning-only POST, while a hidden selected Run remains request-free. The default Gate and production scheduler-preview constructor remain closed; no reservation, lease, Runner, execution, receipt, or Audit authority is added.
- **DONE — §676 Console scheduler lease joins composed inventory/resource convergence**: The shared Web/App/Mobile Sessions Gate now treats a composed owner-bound inventory/resource observation as insufficient by itself when a separate client-instance session/resource pair is configured. Before an explicit scheduler candidate operation, it requires both fresh snapshots and compares the complete resource image and owner; missing, stale, failed, or drifted state returns a fail-closed error before the claim/renew/release/preview/metadata candidate request. Focused Flutter coverage proves valid composed-pair ordering and revision drift with zero lease POST. Production constructors remain default-off/404; no enrollment/heartbeat, inventory mutation, Runner transport/execution, receipt, Audit, or live P4 authority was added.
- **DONE — §677 Audit Governance archives the dispatch-preflight request ABI**: Snaplink Audit Governance now mirrors and strictly receives `forge-run-attempt-lease-dispatch-preflight-request-v1`, binding owner/Conversation/Run/Attempt, placement, Runner intent, idempotency, and lease target metadata while rejecting unknown/duplicate/trailing fields, selected-target mutation, lease-target drift, and any non-zero authority. This is archival compatibility evidence only and does not authenticate devices, select or reserve capacity, issue or mutate leases, dispatch or execute Runner work, persist receipts, or publish Audit authority; ADR-0039/ADR-0114/P4 remain gated.
- **DONE — §678 Real JWT Console Web/App/Mobile scheduler-lease projection**: Forge Core's opt-in acceptance mux now drives the shared Snaplink Console Sessions Gate with one real JWT across Web, desktop App, and Mobile. Each visible instance reads inventory-v2/resource plus session/resource observations before exactly one idempotent fenced lease replay; a hidden Conversation reads only the owner-bound pair and sends zero lease POSTs. Conversation child routes are mounted only in the test mux, and the ordinary production session/resource/inventory/lease constructors remain 404/default-off; no enrollment/heartbeat, inventory mutation, new scheduling authority, Runner transport/execution, receipt persistence, Audit publication, or live P4 effect was added; ADR-0039/ADR-0114/P4 remain gated.
- **DONE — §679 Console Run/Attempt preflight joins the selected client-instance pair**: Snaplink Console's shared Web/App/Mobile Sessions Gate now re-reads the owner-bound client-instance session/resource projection before the opt-in Run/Attempt lease dispatch preflight and Runner Attempt-boundary candidate readers. A hidden, revoked, stale, or drifted selected Conversation fails closed with zero candidate POSTs; focused Flutter coverage proves converged pair ordering and hidden-Run preflight blocking. Production constructors remain default-off/404; no enrollment/heartbeat, inventory mutation, target selection, reservation, lease mutation, Runner transport/execution, receipt persistence, Audit publication, or live P4 effect was added; ADR-0039/ADR-0114/P4 remain gated.
- **DONE — §680 Aero-ID archives the dispatch-preflight request ABI**: Aero-ID's Audit Governance connector now mirrors and strictly consumes `forge-run-attempt-lease-dispatch-preflight-request-v1`, binding owner/Conversation/Run/Attempt, placement, Runner intent, idempotency, and lease target/epoch metadata while rejecting unknown/duplicate/trailing fields, selected-target mutation, lease-target drift, and authority elevation. This remains an offline archival receiver with no JWT, route, database, lease, scheduling, Runner, receipt, or Audit authority; ADR-0039/ADR-0114/P4 remain gated.
- **DONE — §681 Aero-IM and Aero-Vault archive the dispatch-preflight request ABI**: Aero-IM's Rust Audit connector and Aero-Vault's Go Audit Governance receiver now mirror the canonical `forge-run-attempt-lease-dispatch-preflight-request-v1` fixture byte-for-byte and reject unknown/duplicate/trailing fields, selected-target mutation, lease-target drift, and authority elevation. Both receivers remain offline archival checks with no device authentication, target selection, reservation, lease, scheduling, Runner, receipt, or Audit authority; ADR-0039/ADR-0114/P4 remain gated.
- **DONE — §682 Runtime CLI/TUI pins the dispatch-preflight request boundary**: Forge Runtime's authenticated CLI/TUI consumer now pins the canonical dispatch-preflight request bytes by SHA-256 and rejects unknown, duplicate, trailing, selected-target, lease-target, and authority mutations before any candidate request. This remains a bounded metadata-only client boundary; production routes, Runner transport/execution, lease mutation, and P4 authority remain closed.
- **DONE — §683 Real JWT Console Web/App/Mobile Run/Attempt/lease preflight projection**: Forge Core's opt-in acceptance mux now drives the shared Snaplink Console Sessions Gate with one real JWT across Web, desktop App, and Mobile. Each visible instance re-reads its owner-bound client-instance session/resource pair and inventory/resource image before exactly one Run/Attempt/lease preflight POST; a hidden Web instance re-reads the pair but sends zero inventory or preflight POSTs. Conversation child routes remain test-mux-only and production pair, inventory, and preflight constructors remain 404/default-off; no enrollment/heartbeat, inventory mutation, target selection, reservation, lease mutation, Runner transport/execution, receipt persistence, Audit publication, or live P4 authority was added; ADR-0039/ADR-0114/P4 remain gated.
- **DONE — §684 Runtime TUI preflight joins the explicit inventory/resource observation boundary**: Forge Runtime's authenticated TUI now checks its process-local owner-bound inventory-v2/resource pair immediately before the Run/Attempt/lease preflight candidate. A drifted owner, device, lifecycle, capacity, GPU, or observation-time image fails closed with zero POSTs; the existing selected client-instance pair guard remains in front of the candidate. This is a local display/read boundary only; no enrollment/heartbeat, inventory mutation, target selection, reservation, lease mutation, Runner transport/execution, receipt persistence, Audit publication, or P4 authority was added; ADR-0039/ADR-0114/P4 remain gated.
- **DONE — §685 Runtime CLI preflight applies the selected client-instance pair**: `remote run-attempt-lease-dispatch-preflight-preview` now accepts `--instance INSTANCE_ID` with optional `--instance-view FILE|-`. Online mode reads the owner-bound session/resource pair before the metadata-only candidate POST and rejects a hidden Conversation without sending it; local view mode performs the same display filter without candidate reads, while the legacy unfiltered command remains unchanged. This is a planning/read boundary only; no inventory mutation, target selection, reservation, lease mutation, Runner transport/execution, receipt persistence, Audit publication, or P4 authority was added; ADR-0039/ADR-0114/P4 remain gated.
- **DONE — §686 Real JWT Runtime CLI preflight projection**: Forge Core now drives the Runtime CLI through a real Snaplink JWT and the opt-in candidate mux. A visible `client-cli-001` request reads the owner-bound session/resource pair before one metadata-only Run/Attempt/lease preflight POST; a hidden Conversation re-reads the pair and emits zero candidate POSTs, while the ordinary production pair and preflight constructors remain `404`. This adds no inventory mutation, target selection, reservation, lease mutation, Runner transport/execution, receipt persistence, Audit publication, or P4 authority; ADR-0039/ADR-0114/P4 remain gated.
- **DONE — §687 Real JWT Runtime TUI preflight projection**: Forge Core now drives the Runtime TUI through a real Snaplink JWT and the opt-in candidate mux. A visible `client-tui-001` session performs the owner-bound inventory/resource and client-instance session/resource reads before one metadata-only Run/Attempt/lease preflight POST; a hidden Conversation re-reads the same observations and emits zero candidate POSTs, while the ordinary production preflight constructor remains `404`. This adds no inventory mutation, target selection, reservation, lease mutation, Runner transport/execution, receipt persistence, Audit publication, or P4 authority; ADR-0039/ADR-0114/P4 remain gated.
- **DONE — §688 Authenticated owner change SSE boundary**: Forge Core now exposes a scoped, owner-derived `/api/v1/conversation-changes/stream` read path over the existing dense Conversation change metadata. It replays immediately available rows as one SSE frame, long-polls with a bounded server-side wait for a later change, returns `204` when the wait expires, and preserves existing cursor, owner, scope, cancellation, and malformed-response fail-closed behavior. It carries no Prompt body, Run state, device inventory, enrollment/heartbeat, scheduling, Runner, receipt, or execution authority; ADR-0039/ADR-0114/P4 remain gated.
- **DONE — §689 Cross-client change-stream consumer boundary**: Forge Core now proves the owner-scoped Conversation SSE boundary through a real Snaplink JWT, including derived-owner and cursor/event binding. Snaplink Console Web/App/Mobile adds an explicit `conversationChangesStream` adapter for one strictly validated `conversation_changes` event or empty `204`; the Sessions UI keeps its existing polling default. The ordinary constructor remains fail-closed without a Runtime backend; no Prompt, Run, device, lease, Runner, receipt, or execution authority is added, and ADR-0039/ADR-0114/P4 remain gated.

### Cross-device plan §690 — Runtime CLI/TUI explicit Conversation SSE consumer

Forge Runtime now exposes the explicit `remote changes stream` command and the
TUI `changes stream` command over the authenticated owner-scoped Conversation
SSE path. Both clients use one bounded request (`wait_ms` 0..10000), require the
`text/event-stream` media type, parse exactly one `conversation_changes` frame,
and bind its canonical SSE id to the validated dense change page. Unknown or
duplicate fields, duplicate JSON keys, malformed frames, unsafe/noncanonical
IDs, page drift, and wrong content types fail closed. An empty `204` is returned
as `timed_out=true` without advancing the saved/in-process cursor. An explicit
cursor remains one-off; saved checkpoints advance only after a valid advancing
page. Existing polling and bounded watch commands remain the defaults. This is
read-only P2 delivery evidence and adds no Prompt, Run, device registration,
enrollment, heartbeat, inventory authority, target selection, reservation,
scheduling, dispatch, Runner, receipt, or Audit authority; ADR-0039/ADR-0114
remain gated and P4 remains separately gated.

### Cross-device plan §691 — Console Sessions opt-in SSE consumer

Snaplink Console's shared `ForgeSessionsScreen` now accepts the explicit
`enableConversationChangesStream` flag and bounded stream wait. After the
initial owner snapshot, the screen permits one foreground stream request at a
time, reconnects after a successful empty `204`, and falls back to the existing
adaptive JSON polling path after transport, authorization-refresh, malformed
SSE, or cursor validation failure. The stream reuses the existing Conversation
merge and required history refresh; the owner-local cursor is persisted only
after those operations succeed. Lifecycle pause, sign-out, disposal, and flag
changes cancel pending reconnects. The default Sessions Gate and existing
callers leave the flag off and retain polling behavior. Focused widget tests
cover stream merge/cursor advancement, 204 reconnect, and malformed-stream
fallback. This adds read delivery only; no Prompt/Run mutation, device
registration/enrollment/heartbeat, inventory authority, target selection,
reservation, scheduling, dispatch, Runner, receipt, or Audit authority was
added. ADR-0039/ADR-0114 remain gated and P4 remains separate.

### Cross-device plan §692 — Runtime TUI change-feed instance freshness gate

When a TUI caller has selected `instance:INSTANCE_ID`, `changes watch` and
`changes stream` now refresh the owner-bound client-instance session/resource
observations before requesting the owner change feed. A failed or non-converged
pair, or a Conversation revoked from the previously selected instance, stops
before the feed request and leaves the local cursor unchanged. The default
unfiltered TUI path remains request-compatible. This is read/display freshness
only; no Prompt/Run mutation, enrollment/heartbeat, inventory authority,
target selection, reservation, scheduling, dispatch, Runner, receipt, or Audit
authority was added. ADR-0039/ADR-0114 remain gated and P4 remains separate.

### Cross-device plan §693 — Real JWT Runtime CLI/TUI Conversation SSE E2E

An opt-in Forge Core integration now drives both Runtime CLI and TUI with a
real Snaplink JWT against the owner-scoped Conversation SSE route. It verifies
the authenticated `Accept: text/event-stream` boundary, dense cursor/page
delivery, normal TUI startup sync, and zero POST/device/execution effects.
This remains P2 read delivery only; ADR-0039/ADR-0114 remain gated and P4
remains separately governed.

### Cross-device plan §694 — Real JWT Flutter Console Conversation SSE E2E

An opt-in Forge Core integration now launches the shared Flutter Console API
test with a real Snaplink JWT against the owner-scoped Conversation SSE route.
It verifies the authenticated `Accept: text/event-stream` boundary, Bearer
authentication, dense cursor/page delivery, owner binding, and exactly one
read-only GET with no POST or device/execution effect. The Sessions UI stream
remains explicitly opt-in and polling remains the default for Web/App/Mobile
callers. This adds no Prompt/Run mutation, enrollment/heartbeat, inventory
authority, target selection, reservation, scheduling, dispatch, Runner,
receipt, or Audit authority; ADR-0039/ADR-0114 remain gated and P4 remains
separately governed.

### Cross-device plan §695 — Console Sessions Gate SSE projection

The shared Snaplink Console `ForgeSessionsGate` now exposes explicit
`enableConversationChangesStream` and `conversationChangesStreamWaitMS`
options, defaulting to `false` and `15000`, and forwards them to the existing
`ForgeSessionsScreen`. Gate widget coverage proves default construction issues
no SSE request; explicit opt-in uses the bounded stream and falls back to the
existing JSON polling path on failure. This remains P2 read delivery only and
adds no Prompt/Run mutation, device enrollment/heartbeat, inventory authority,
target selection, reservation, scheduling, dispatch, Runner, receipt, Audit,
or P4 authority.

### Cross-device plan §696 — Real JWT Chromium Conversation SSE E2E

An opt-in Forge Core/Web harness serves the built Flutter Web app through a
same-origin test wrapper, opens a real Chromium tab, seeds a real Snaplink JWT,
and performs a browser `fetch` against the owner-scoped Conversation SSE route.
It strictly validates the SSE event, dense cursor/page, Bearer header, and zero
POSTs, while the ordinary production constructor remains fail-closed. This is
read-only Web delivery evidence; ADR-0039/ADR-0114 remain gated and P4 remains
separately governed.

### Cross-device plan §697 — Console selected-instance revocation closes the change feed

The shared Console Sessions screen now requires the latest selected
client-instance session/resource observation to contain the selected instance
before consuming either the SSE or JSON change feed. If a refresh removes the
instance, the screen fails closed before transport, keeps the owner-local
cursor unchanged, and preserves the existing private Prompt/Run revocation
path. Cursor persistence is checked before publishing an advanced in-memory
cursor. Focused coverage proves zero feed requests after removal. This remains
read-only display fencing with no Prompt/Run, device, scheduling, Runner,
receipt, Audit, or P4 authority.

### Cross-device plan §698 — Native mobile Conversation SSE evidence

The opt-in Android-host/iOS-host shared-session lifecycle now restores the
owner-local cursor through the native credential path and consumes one
authenticated `/conversation-changes/stream` page on the second cold start.
The host harness binds the SSE event, cursor, owner, and Prompt change while
retaining exactly the existing storage-only Prompt writes. The normal mobile
Sessions Gate remains polling/default-off. This adds no device
enrollment/heartbeat, inventory mutation, target selection, reservation,
scheduling, Runner, receipt, Audit, or P4 authority; ADR-0039/ADR-0114 remain
gated.

### Cross-device plan §699 — Mobile inventory-v2 observation chain

The opt-in Android-host/iOS-host cold-start journey now reads the authenticated
lossless `/api/v1/devices/observations/v2` candidate on both native starts.
Flutter and the Go harness bind the verified owner, `runner-a/device-a`
revision/generation/heartbeat tuple, reservation and multi-GPU values, and
all-false authority. The exact request allowlist still excludes
enrollment/heartbeat writes, target selection, reservation, scheduling,
Runner, receipt, and Audit effects. Production inventory remains
default-off/404 and ADR-0039/ADR-0114/P4 remain gated.

### Cross-device plan §700 — Inventory/resource convergence revalidates manually constructed v2 observations

Snaplink Console now round-trips locally constructed inventory-v2 and
client-instance/resource observations through their strict decoders before
treating them as a converged display boundary. Envelope, owner, capacity,
lifecycle, GPU, revision/generation/heartbeat, and all-false authority drift
fails closed; regression coverage proves a tampered inventory envelope cannot
be used as a freshness proof. No inventory mutation, enrollment/heartbeat,
target selection, reservation, scheduling, Runner, receipt, Audit, or P4
authority was added; ADR-0039/ADR-0114/P4 remain gated.

### Cross-device plan §701 — Client-instance session/resource convergence revalidates nested observations

The Console Gate's injected client-instance session/resource pair now rechecks
the nested display envelopes and complete instance image before exposing a
converged local filter. A hand-built outer `converged/read_only` envelope with
schema, owner, row, or authority drift fails closed; regression coverage
proves malformed nested rows cannot bypass the pair boundary. No Prompt/Run
mutation, inventory mutation, enrollment/heartbeat, target selection,
reservation, scheduling, Runner, receipt, Audit, or P4 authority was added;
ADR-0039/ADR-0114/P4 remain gated.

### Cross-device plan §702 — Runtime convergence joins revalidate nested observations

Forge Runtime CLI/TUI client-instance and inventory/resource join helpers now
rerun the strict source validators at the join boundary before comparing owner,
instance, lifecycle, capacity, and GPU fields. Authority-bearing or manually
assembled nested observations fail closed even when shared resource fields
match; focused unit coverage proves both joins reject nested authority
mutation. This remains a read-only metadata boundary with no registration,
heartbeat, inventory mutation, target selection, reservation, scheduling,
Runner transport/execution, receipt, Audit, or P4 authority; ADR-0039/ADR-0114
remain gated and P4 remains separately governed.

### Cross-device plan §703 — Mobile planning-only scheduler-preview handoff

The opt-in Android-host/iOS-host cold-start journey now calls one authenticated
`/api/v1/device-placement/scheduler-preview` POST after the strict
client-instance session/resource and lossless inventory-v2 observations have
converged. Its test-only scheduler image is fresh and unreserved, selects
`device-a`/`runner-a`, and requires `preview_only=true` with all authority
predicates false; the display inventory remains reserved so it cannot be read
as scheduling authority. The request allowlist has no enrollment, heartbeat,
reservation, lease, dispatch, Runner, or execution effect. Production preview
remains default-off/404; ADR-0039 remains planning-only, ADR-0114 remains
Proposed/null, and P4 requires a separate accepted execution/security decision.

### Cross-device plan §704 — Lifecycle candidate routes remain closed on ordinary Coordinator

The ordinary authenticated Conversation/Coordinator constructor now has exact
404 regression coverage for lifecycle registry, heartbeat, approval, and
credential candidate paths, even when lifecycle scopes are present in the
bearer. Enrollment, heartbeat, approval, credential, inventory mutation,
Runner, and execution authority remain behind their separately activated
candidate seams; ADR-0114 remains Proposed/null and no live lifecycle authority
was enabled.

### Cross-device plan §705 — Real JWT Runtime CLI scheduler-preview pair convergence

The opt-in Forge Core E2E now drives the Rust Runtime CLI with a real Snaplink
JWT and `--instance client-web`. It proves the visible request order
`session-view → resource-view → scheduler-preview`, binds the returned
owner/Conversation/Run/Attempt and selected `device-a`/`runner-a`, and
requires `preview_only` plus all-false authority. A hidden `client-tui`
instance performs exactly the two pair reads and fails closed without POST;
under-scoped access still returns 403 and the ordinary production constructor
remains 404. This is planning-only evidence with no reservation, lease,
dispatch, Runner, enrollment, heartbeat, receipt, Audit, or live P4 authority;
ADR-0039/ADR-0114/P4 remain gated.

### Cross-device plan §706 — Runtime CLI scheduler-preview inventory/resource convergence

Online Runtime CLI `remote placement scheduler-preview --instance` now
completes the existing owner-bound client-instance session/resource scope
before calling the strict inventory-v2/resource-view convergence reader. A
visible instance therefore orders
`session-view → resource-view → inventory-v2 → resource-view → scheduler-preview`;
owner, device/Runner, revision, generation, heartbeat, capacity, GPU,
lifecycle, and observation-time drift fail closed before the planning-only
POST. Hidden instances stop after the pair scope, local `--instance-view`
remains offline, and unfiltered preview remains a single POST. No reservation,
lease, dispatch, Runner execution, enrollment, heartbeat, receipt, Audit, or
live P4 authority was enabled; ADR-0039/ADR-0114/P4 remain gated.

### Cross-device plan §707 — Runtime CLI Prompt-write inventory/resource guard

Online `remote prompts add/receipt --instance INSTANCE_ID` now completes the
owner-bound client-instance session/resource pair and then the strict
inventory-v2/resource-view convergence reader before the Prompt POST. Hidden
instances fail before any inventory or Prompt request; owner, device/Runner,
revision/generation/heartbeat, capacity, GPU, lifecycle, or observation drift
fails closed before POST. Prompt history reads, unfiltered commands, and local
`--instance-view` remain compatible and offline. No Run, target selection,
reservation, lease, dispatch, Runner execution, enrollment, heartbeat, receipt,
Audit, or live P4 authority was enabled; ADR-0039/ADR-0114/P4 remain gated.

### Cross-device plan §708 — Runtime TUI Prompt-write inventory/resource freshness

An explicitly selected Runtime TUI instance now refreshes the already-open
owner-bound inventory-v2/resource pair immediately before a Prompt append or
retry. The pair is committed atomically only after strict convergence; drift,
malformed envelopes, or authorization failure retain the pending write and stop
before the Prompt POST. Missing or one-sided opt-in observations keep the
existing request-free compatibility path.

This is a Prompt-write freshness guard only. It adds no Run, target selection,
reservation, lease, dispatch, Runner execution, enrollment, heartbeat, receipt,
Audit, or live P4 authority; ADR-0039 remains planning-only, ADR-0114 remains
Proposed/null, and P4 requires separate accepted execution and security
governance.

### Cross-device plan §709 — Runtime TUI scheduler-preview inventory/resource freshness

When a selected Runtime TUI instance has explicitly opened both owner-bound
inventory-v2 and client-instance/resource observations, its planning-only
`scheduler-selection-preview` refreshes the strict pair immediately before
POST. The real-JWT E2E proves visible pair/open/refresh/POST ordering and
hidden fail-closed behavior before inventory or POST. Missing or one-sided
observations remain request-free; no reservation, lease, dispatch, Runner,
execution, enrollment, heartbeat, receipt, Audit, or live P4 authority was
added.

### Cross-device plan §710 — Real JWT Console Prompt inventory/resource freshness

The opt-in shared Console Web/App/Mobile Prompt E2E now injects the existing
owner-bound inventory/resource convergence candidate. It requires the visible
client's client-instance pair and inventory-v2/resource convergence observation
before its storage-only Prompt POST, while rejecting hidden Prompt POSTs.
Default Gate construction and production routes remain closed; no Run,
reservation, lease, dispatch, Runner execution, enrollment, heartbeat, receipt,
Audit, or live P4 authority was added.

### Cross-device plan §711 — Console Prompt write refreshes inventory/resource immediately before POST

When the opt-in shared Console Web/App/Mobile Prompt path has the composed
owner-bound inventory/resource reader, it now forces a fresh pair read at the
Prompt write boundary. A resource revision or client-instance image that drifts
after the initial display refresh is rejected before the storage-only submitter
is invoked; the default Gate and one-sided compatibility paths remain
request-free. Focused Flutter coverage proves the second read and zero Prompt
submitter calls on drift.

This is read freshness evidence around Prompt storage only. It adds no Run,
target selection, reservation, lease, dispatch, Runner execution,
enrollment, heartbeat, receipt, Audit, or live P4 authority; ADR-0039 remains
planning-only, ADR-0114 remains Proposed/null, and P4 requires separate
accepted execution and security governance.

### Cross-device plan §712 — Console scheduler-preview forces fresh inventory/resource evidence

The opt-in Console Web/App/Mobile planning-only scheduler-selection-preview
path now forces a new owner-bound inventory/resource pair immediately before
its candidate POST, even when the display cache is healthy. A revision drift
from that forced read fails closed with zero preview POSTs; focused Flutter
coverage proves the reread counts and the drift boundary. Default Gate
construction and production routes remain closed. No reservation, lease,
dispatch, Runner execution, enrollment, heartbeat, receipt, Audit, or live P4
authority was added; ADR-0039 remains planning-only, ADR-0114 remains
Proposed/null, and P4 requires separate accepted execution and security
governance.

### Cross-device plan §713 — Real-JWT Console scheduler-preview inventory/resource convergence

The accepted test-only Console Web/App/Mobile scheduler Gate now supplies the
same owner-bound inventory-v2 and client-instance resource candidates that the
planning-only preview must observe. The Gate performs those authenticated reads
before rendering the selection and fails closed if either candidate is missing
or non-convergent; the ordinary production constructor remains default-off.
This adds no enrollment, heartbeat, reservation, lease, dispatch, Runner
execution, receipt, Audit, or live P4 authority; ADR-0039 remains
planning-only, ADR-0114 remains Proposed/null, and P4 retains its separate
execution/security gate.

### Cross-device plan §714 — CLI pending Run-intent inventory/resource freshness

Online CLI pending Run-intent submission with `--instance` now refreshes the
strict owner-bound inventory-v2/resource pair after the client-instance
session/resource projection and before the candidate POST. Resource drift
rejects the submission with zero pending-intent POSTs; local `--instance-view`,
unfiltered reads, and ordinary compatibility paths remain unchanged. No Run,
reservation, lease, dispatch, Runner execution, enrollment, heartbeat, receipt,
Audit, or live P4 authority was enabled; ADR-0039/ADR-0114/P4 remain gated.

### Cross-device plan §715 — CLI scheduler lease claim/renew inventory/resource freshness

Online CLI scheduler lease claim and renewal scoped with `--instance` now
refresh the strict owner-bound inventory-v2/resource pair after the
client-instance projection and before the candidate POST. Inventory/resource
read failure or drift yields zero claim/renew POSTs; local `--instance-view`,
unfiltered operations, and lease release compatibility remain unchanged. No
live reservation, lease authority, dispatch, Runner execution, enrollment,
heartbeat, receipt, Audit, or P4 authority was enabled; ADR-0039/ADR-0114/P4
remain gated.

### Cross-device plan §716 — TUI pending Run-intent inventory/resource freshness

An explicitly selected Runtime TUI instance with both owner-bound observations
open now refreshes the inventory-v2/resource pair immediately before a pending
Run-intent submit or retry. Drift, malformed responses, or authorization
failure retain the pending intent and stop before POST; missing or one-sided
observations keep the existing request-free compatibility path. No ordinary
Run, reservation, lease, dispatch, Runner execution, enrollment, heartbeat,
receipt, Audit, or live P4 authority was enabled; ADR-0039/ADR-0114/P4 remain
gated.

### Cross-device plan §717 — TUI scheduler lease claim/renew inventory/resource freshness

An explicitly selected Runtime TUI instance with both owner-bound observations
open now refreshes the inventory-v2/resource pair immediately before scheduler
lease claim or renewal and rechecks Conversation visibility after the refresh.
Drift, malformed responses, or instance revocation yield zero candidate POSTs;
release, unfiltered, and one-sided compatibility paths remain unchanged. No
live reservation, lease authority, dispatch, Runner execution, enrollment,
heartbeat, receipt, Audit, or P4 authority was enabled; ADR-0039/ADR-0114/P4
remain gated.

### Cross-device plan §718 — Runner metadata previews refresh inventory/resource

Online Runtime CLI Runner execution-intent, dispatch-plan, and Run/Attempt/lease
preflight previews now read the owner-bound client-instance pair, then a
converged inventory-v2/resource pair, before a candidate POST; the refreshed
resource image must still contain the selected Conversation. Runtime TUI applies
the same refresh and visibility recheck to execution-intent, dispatch-plan, and
preflight previews, and the shared Console Web/App/Mobile execution-intent Gate
refreshes its selected client-instance and inventory/resource observations before
the candidate reader. Drift, malformed observations, or instance revocation
yield zero candidate requests; local/offline, unfiltered, one-sided, and default
request-free paths remain compatible. This adds metadata freshness evidence only
and no enrollment/heartbeat, inventory mutation, target selection, reservation,
scheduler lease, dispatch, Runner transport/execution, receipt, Audit, or P4
authority; ADR-0039/ADR-0114/P4 remain gated.

### Cross-device plan §719 — Instance-scoped change-feed projections

Runtime CLI `changes list/watch/stream` and TUI change-feed commands now accept
an explicit client-instance filter. They refresh the owner-bound session/resource
declaration, output and apply only Conversation rows declared by that instance,
and still advance the owner cursor across hidden rows. The shared Console
Web/App/Mobile Sessions feed follows the same rule: hidden-instance changes are
acknowledged without triggering private Prompt/Run hydration. Owner-wide
defaults, local/offline views, and unfiltered compatibility remain unchanged.

This is a display projection over caller-supplied observations, not instance
authorization. It adds no enrollment/heartbeat authority, inventory mutation,
target selection, reservation, scheduler lease, dispatch, Runner
transport/execution, receipt persistence, Audit, or P4 authority. ADR-0039
remains planning-only, ADR-0114 remains Proposed/null, and P4 requires
separate accepted execution and security governance.

### Cross-device plan §720 — Candidate heartbeat-to-inventory/resource journey

The Forge Core candidate harness now POSTs a proof-bound heartbeat into the
private lifecycle image, reconstructs the accepted read-only activation from
that persisted image, and verifies owner-scoped inventory-v2 plus
client-instance session/resource projections across the restart boundary. It
also proves heartbeat replay/CAS rejection, foreign-owner isolation, all-false
authority, and that the accepted assembly does not expose the heartbeat write
route.

This remains a candidate-only pre-activation journey. No production
enrollment/heartbeat route, inventory mutation, reservation, scheduler lease,
dispatch, Runner transport/execution, receipt, Audit, or P4 authority was
enabled. ADR-0039 remains planning-only, ADR-0114 remains Proposed/null, and
P4 requires separate accepted execution and security governance.

### Cross-device plan §721 — Accepted fabric assembly rechecks review evidence

The accepted Forge Core device-fabric route assembly now has a focused
construction boundary proving that a persisted lifecycle image cannot bypass
review state. Proposed or planning-only ADR-0114 metadata, missing owner
approval/revocation evidence, missing heartbeat CAS/freshness evidence, or
missing inventory owner-scope evidence blocks assembly before any owner-scoped
inventory or client-instance read route is mounted.

This is a fail-closed review boundary only. It does not transition ADR state or
mount enrollment/heartbeat writes, inventory mutation, reservation, scheduler
lease, dispatch, Runner transport/execution, receipt, Audit, or P4 authority.
ADR-0039 remains planning-only, ADR-0114 remains Proposed/null, and P4
requires separate accepted execution and security governance.

### Cross-device plan §722 — Accepted lifecycle mutation surfaces stay closed

Accepted `INVENTORY` and `OBSERVE` assemblies now have regression coverage
showing that heartbeat, approval, credential, and lifecycle-registry
replacement writes remain `404`. The accepted image exposes read-only
owner-scoped projections only; the candidate mutation handlers continue to
require an injected test store and are not treated as production device
identity transport.

This preserves the enrollment boundary while the independent device-credential
transport is still designed and governed. No live enrollment/heartbeat,
inventory mutation, reservation, scheduler lease, dispatch, Runner
transport/execution, receipt, Audit, or P4 authority was enabled. ADR-0039
remains planning-only, ADR-0114 remains Proposed/null, and P4 requires
separate accepted execution and security governance.

### Cross-device plan §723 — Signed heartbeat candidate proof transport boundary

An injected Forge Core candidate-only `heartbeat-signed` route now verifies
the persisted owner/device binding, Ed25519 proof, and heartbeat digest against
a server challenge before a lifecycle registry CAS update. It preserves other
devices and returns preview-only, all-false authority; ordinary and accepted
assemblies retain `404` for the signed write. Authoritative challenge
issuance/consumption,
credential enrollment, production heartbeat, inventory authority, scheduling,
Runner transport/execution, receipts, Audit, and P4 remain gated.

### Cross-device plan §724 — Runtime TUI keeps instance-hidden creates out of private projection

Runtime TUI Conversation creation now checks the returned owner session against
the currently converged client-instance display declaration before selecting it
or retaining private Prompt/Run state. A newly created session absent from the
selected instance remains unselected and is filtered from the refreshed list;
the focused journey proves one create and one list read with no detail, Prompt,
or Run read. Invalid or divergent observations fail closed before the create
request. The owner-wide create API remains storage-only and has no membership
writer or instance authority.

### Cross-device plan §725 — Accepted non-EXECUTE execution evidence remains closed

Accepted `INVENTORY` and `OBSERVE` route assemblies now have a focused matrix
proving Runner receipts, receipt history, reconciliation, Attempt, scheduler,
dispatch, transport, and execution-boundary candidates remain `404` even with
a persisted lifecycle image. The separate EXECUTE/P4 gate remains the only
assembly allowed to mount those planning/evidence candidates; no receipt
persistence, Runner transport, live execution, or ADR status change was made.
### Cross-device plan §726 — Candidate challenge issuance and one-time signed heartbeat consumption

The injected Forge Core lifecycle candidate now issues one owner-scoped,
server-clocked challenge at a time, binds its digest and bounded expiry to the
private lifecycle image, and refuses an active reissue. The signed heartbeat
candidate must present that persisted, unconsumed challenge, a matching
Ed25519 proof, and a heartbeat digest bound to the challenge; successful
candidate CAS marks the challenge consumed and advances the lifecycle image.
Generic lifecycle replacement rejects challenge injection, and the response
remains preview-only with all authority predicates false. The older unsigned
heartbeat candidate remains a separate injected fixture seam; it is absent
from production and accepted assemblies.

Ordinary Coordinator and accepted `INVENTORY`/`OBSERVE` assemblies keep both
candidate writes closed at `404`. Credential enrollment, production heartbeat,
inventory mutation, scheduling, Runner transport/execution, receipts, Audit,
and P4 remain gated; ADR-0039 remains planning-only and ADR-0114 remains
Proposed/null.

### Cross-device plan §729 — Runtime CLI instance-scoped Conversation create preflight

`remote sessions create` now accepts `--instance INSTANCE_ID` with an optional
strict `--instance-view FILE|-`. Online creation first reads and converges the
owner-bound client-instance session/resource observations; malformed, unknown,
or non-converged declarations fail before the single owner-wide Conversation
POST. A local declaration keeps the path offline while applying the same strict
projection check. The resulting create remains storage-only: it does not assert
instance membership or select private Prompt/Run state, and it adds no device,
scheduling, Runner, receipt, Audit, or P4 authority.

### Cross-device plan §730 — Real JWT CLI instance-scoped Conversation create journey

The opt-in Forge Core five-client projection journey now drives the built
Runtime CLI through a real Snaplink JWT for
`remote sessions create --instance client-cli-001`. Its recorder proves the
owner-bound `session-view → resource-view → one Conversation POST` order and
validates the returned owner Conversation. The same run still covers the
Console Web/App/Mobile and TUI projections, and rejects device, scheduler,
Runner, and execution side requests. The create remains owner-wide storage
only; no membership writer or execution authority is introduced.

### Cross-device plan §731 — CLI instance-create resource-scope rejection

The real JWT projection journey also retries `remote sessions create --instance`
with only Conversation read/write scopes. The session-view read succeeds, the
resource-view candidate rejects the missing `forge:devices:read` scope with 403,
and the CLI emits no Conversation result or owner-wide POST. This keeps the
instance-create preflight from turning display selection into write authority.

### Cross-device plan §732 — Runtime TUI instance-scoped create requires a converged resource pair

A selected Runtime TUI client-instance filter now blocks its owner-wide
Conversation create until both owner-bound session-view and resource-view
observations are present and converged. A one-sided or drifted pair remains
request-free, while ordinary owner-wide TUI create and the hidden-response
projection check remain compatible. This adds no membership writer,
enrollment/heartbeat, inventory mutation, scheduling, Runner, receipt, Audit,
or P4 authority; ADR-0039/ADR-0114/P4 remain gated.

### Cross-device plan §733 — Runtime TUI instance-scoped Conversation create freshness guard

With an active client-instance filter, TUI creation now refreshes and strictly
converges the owner-bound session/resource pair immediately before the
owner-wide POST. Pair drift, malformed observations, instance revocation, or
authorization failure blocks the POST and keeps the pending write available for
explicit retry; a successful owner-wide create outside the refreshed declaration
remains unselected. No membership writer, enrollment/heartbeat, inventory
mutation, scheduling, Runner, receipt, Audit, or P4 authority was added.

### Cross-device plan §734 — Real JWT Runtime TUI instance-scoped Conversation create journey

The opt-in Forge Core five-client projection journey now drives the
interactive Runtime TUI with a real Snaplink JWT through
`client-instances show-converged`, an explicit `client-tui-001` filter, and an
instance-scoped `create`. The recorder proves startup session listing, initial
session/resource convergence, a second fresh pair immediately before one
owner-wide Conversation POST, and the post-create owner refresh; the newly
created Conversation is absent from the declaration and remains unselected.
No membership writer, device enrollment/heartbeat, inventory mutation,
scheduling, Runner, receipt, Audit, or P4 authority was added; ADR-0039/
ADR-0114/P4 remain gated.
### Cross-device plan §735 — Real JWT Runtime TUI instance-scoped Prompt freshness

The opt-in Forge Core five-client projection journey now drives the
authenticated Runtime TUI through converged client-instance and
inventory/resource observations before opening a declared Conversation. TUI
refreshes the owner-bound inventory/resource pair again immediately before
the storage-only Prompt POST; the recorder proves the read ordering, one
visible Prompt POST, and no device, scheduler, Runner, or execution side
request. No membership writer, enrollment/heartbeat, inventory mutation,
scheduling, Runner, receipt, Audit, or P4 authority was added; ADR-0039/
ADR-0114/P4 remain gated.
### Cross-device plan §736 — Real JWT Runtime TUI instance-scoped change stream

The opt-in Forge Core journey now drives `changes stream --instance` through
a real Snaplink JWT after converged client-instance reads. The recorder proves
a fresh pair immediately before the SSE GET; hidden Conversation changes advance
the owner cursor without entering the selected session projection, and no
Prompt, device, scheduler, Runner, or execution request is emitted. No
membership writer, enrollment/heartbeat, inventory mutation, scheduling,
Runner, receipt, Audit, or P4 authority was added; ADR-0039/ADR-0114/P4
remain gated.

### Cross-device plan §737 — Real JWT Runtime CLI instance-scoped change stream

The opt-in Forge Core journey now drives `remote changes stream --instance`
through a real Snaplink JWT after a fresh owner-bound session/resource pair.
The JSON response contains only the selected Conversation change while
`scanned_through_cursor` advances across the hidden row; the recorder proves
the exact read order and no Prompt, device, scheduler, Runner, or execution
request. No membership writer, enrollment/heartbeat, inventory mutation,
scheduling, Runner, receipt, Audit, or P4 authority was added; ADR-0039/
ADR-0114/P4 remain gated.

### Cross-device plan §738 — Real JWT Runtime CLI execution-intent convergence

The opt-in Forge Core execution-intent convergence journey now launches the
real Runtime CLI with a Snaplink JWT and an explicit `client-cli` instance.
Before the metadata-only intent POST, the CLI reads and converges the
owner-bound client-instance session/resource pair, then the lossless
inventory-v2/resource pair. The recorder fixes the exact five-request order;
a hidden `client-tui` stops after the first two reads with no inventory or
intent request. The response remains selected-target-null and all authority
false, and command argv, workspace, and fencing material stay out of output.
The ordinary production constructor remains 404; no inventory mutation,
selection, reservation, lease mutation, Runner transport/execution, receipt
persistence, Audit publication, or live P4 authority was added. ADR-0039/
ADR-0114/P4 remain gated.

### Cross-device plan §739 — Real JWT Console Web/App/Mobile execution-intent Gate convergence

The opt-in Forge Core journey now drives the shared Snaplink Console Gate with
one real JWT across Web, desktop App, and Mobile. Each visible client refreshes
the owner-bound client-instance session/resource pair, reads the lossless
inventory-v2 image, refreshes the resource image again, and posts exactly one
metadata-only Runner execution-intent candidate. Hidden and resource-drifted
instances remain candidate-free; the ordinary production constructors stay
`404`. This adds no inventory mutation, target selection, reservation, lease,
Runner transport/execution, receipt, Audit, or P4 authority; ADR-0039/
ADR-0114/P4 remain gated.

### Cross-device plan §740 — Console scheduler-preview target/resource binding

The shared Console scheduler-preview projection now validates that every
returned device and Runner instance is present in the current owner-bound
resource observation before displaying the candidate. A target absent from the
resource image, including a fetched or static response, is rejected and cannot
cross the Sessions projection. This is display-only binding hygiene; it adds no
reservation, scheduler lease, dispatch, Runner transport/execution, receipt,
Audit, or P4 authority, and ADR-0039/ADR-0114/P4 remain gated.

### Cross-device plan §741 — Real JWT Runtime TUI execution-intent convergence

The opt-in Forge Core journey now drives the real Runtime TUI through a
Snaplink JWT, an explicit `client-tui-001` filter, and the planning-only Runner
execution-intent candidate. The visible TUI refreshes the owner-bound
session/resource pair, refreshes the lossless inventory-v2/resource pair, and
posts one metadata-only intent; a hidden `client-web-001` stops before POST.
The ordinary production constructor remains `404`, and no target selection,
reservation, lease mutation, Runner transport/execution, receipt, Audit, or P4
authority was added; ADR-0039/ADR-0114/P4 remain gated.

### Cross-device plan §742 — Real JWT Console Web/App/Mobile scheduler-preview target/resource Gate

The opt-in Forge Core journey now drives the shared Console Gate with one real
Snaplink JWT across Web, desktop App, and Mobile. Each visible selected
instance refreshes the owner-bound session/resource pair and the
inventory/resource image before one planning-only scheduler-preview POST. The
Gate strictly binds the returned `device-a / runner-a` target to that resource
image before rendering the preview; authority remains all false and the card
contains no fencing or lease token. Hidden membership and resource/session
drift remain request-free, while ordinary production constructors remain
`404`. No reservation, lease, Runner transport/execution, receipt, Audit, or
P4 authority was added; ADR-0039 remains planning-only and ADR-0114 remains
Proposed/null.

### Cross-device plan §743 — Console Runner dispatch-plan target/resource binding

The shared Console Web/App/Mobile Sessions projection now revalidates every
Runner dispatch-plan candidate target, including the intent target, against the
current owner-bound resource image before display. Canonical device IDs and
adapter-exposed Runner instance IDs are treated as identities of the same
observed resource row; owner drift and foreign targets fail closed. The real
JWT Gate journey covers visible, hidden, session/resource drift, and target
resource drift while the no-resource-reader compatibility path remains
request-compatible. This remains planning-only: no target selection,
reservation, lease, Runner transport/execution, receipt, Audit, or P4 authority
was added; ADR-0039 remains planning-only and ADR-0114 remains Proposed/null.

### Cross-device plan §744 — Runtime TUI local Runner preview resource/target guard

An explicitly selected Runtime TUI client instance now requires an owner-bound
converged inventory/resource pair before the injected local Runner
execution-readiness preview. The pair is refreshed immediately before the
candidate POST, Conversation visibility is rechecked, and the request target
must match either the observed device ID or Runner instance ID. Hidden sessions,
missing or drifted pairs, and foreign targets remain zero-POST. Focused Rust
coverage exercises these fail-closed paths; production routes, local executor
authority, receipts, reservation, Runner transport, Audit, and P4 authority
remain closed; ADR-0039 remains planning-only and ADR-0114 remains Proposed/null.

### Cross-device plan §745 — Runtime TUI dispatch-plan target/resource guard

An explicitly selected Runtime TUI client instance now requires the converged
inventory-v2/resource pair before the planning-only Runner dispatch-plan
candidate. After the fresh pair and Conversation visibility checks, the lease
target, intent target, and every placement device ID must match an observed
device ID or Runner instance ID; missing pairs and foreign targets remain
zero-POST. Unfiltered behavior remains compatible. No reservation, lease
mutation, dispatch, Runner transport/execution, receipt, Audit, or P4 authority
was added; ADR-0039 remains planning-only and ADR-0114 remains Proposed/null.

### Cross-device plan §746 — Runtime TUI dispatch-admission target/resource guard

The selected-instance Runtime TUI dispatch-admission preview now requires the
converged inventory-v2/resource pair, refreshes it immediately before POST,
rechecks Conversation visibility, and rejects a lease-proof target absent from
the owner-bound device/Runner resource image. Missing pairs and foreign targets
remain zero-POST; unfiltered behavior remains compatible. No reservation, lease
mutation, dispatch, Runner transport/execution, receipt, Audit, or P4 authority
was added; ADR-0039 remains planning-only and ADR-0114 remains Proposed/null.

### Cross-device plan §747 — Runtime TUI transport-admission target/resource guard

The selected-instance Runtime TUI transport-admission preview now applies the
same converged inventory-v2/resource refresh and Conversation visibility fence,
then requires the command lease-proof target to exist in the owner-bound
device/Runner resource image. Missing pairs and foreign targets remain
zero-POST; unfiltered behavior remains compatible. No Runner connection,
payload transport, reservation, lease mutation, execution, receipt, Audit, or
P4 authority was added; ADR-0039 remains planning-only and ADR-0114 remains
Proposed/null.

### Cross-device plan §748 — Console Runner admission target/resource gate

The shared Console Web/App/Mobile Sessions projection now refreshes the
selected client-instance's owner-bound inventory/resource observation before a
Runner dispatch-admission or transport-admission candidate. Configured empty
or drifted resource images, owner mismatch, and targets absent from either the
observed device ID or Runner instance ID remain zero-POST and hidden; static
and fetched candidates use the same gate, while the no-resource-reader
compatibility path remains unchanged. No Runner transport, payload execution,
enrollment, reservation, Audit, or P4 authority was added; ADR-0039 remains
planning-only and ADR-0114 remains Proposed/null.

### Cross-device plan §749 — Native Android/iOS admission evidence boundary

The Console Android/iOS native contract harness now validates metadata-only
dispatch and transport admission candidates against the owner-bound resource
image. It accepts only matching device/Runner target identities with owner
alignment, `preview_only`, valid admission bindings, and all-false authority;
foreign targets, owner/authority drift, direct Runner dispatch, and payload
transport paths fail closed. The host harness is request-free and does not
start ADB, Xcode, HTTP, or a Runner; native platform journeys remain explicit
opt-in and default-off. No reservation, execution, receipt, Audit, or P4
authority was added; ADR-0039/ADR-0114/P4 remain gated.

### Cross-device plan §750 — Runtime CLI Runner admission target/resource gate

Non-interactive Runtime CLI dispatch-admission and transport-admission previews
now accept an explicit `--instance` projection only after refreshing the
owner-bound client-instance session/resource pair and the inventory-v2/resource
pair. The lease-proof target must match an observed device ID or Runner
instance ID; missing, malformed, foreign, or drifted observations remain
zero-POST. Unfiltered input remains compatible, and local `--instance-view`
accepts only a validated resource or converged fixture so the target binding
is observable. No Runner transport, payload execution, reservation,
lease mutation, receipt, Audit, or P4 authority was added; ADR-0039/ADR-0114/P4
remain gated.

### Cross-device plan §751 — Real JWT Runtime CLI Runner admission convergence

An opt-in Forge Core E2E now drives both Runtime CLI dispatch-admission and
transport-admission previews with a real Snaplink JWT and explicit
`--instance client-cli-001`. Each visible command proves
`session-view → resource-view → inventory-v2 → resource-view → one admission
POST`; after the resource image changes to a foreign target, the same commands
stop after the four read-only observations with zero admission POSTs. The
candidate uses a persisted lease and owned Run reference but never contacts a
Runner; the normal production constructor remains `404` and no reservation,
execution, receipt, Audit, or P4 authority was added.

### Cross-device plan §752 — Runtime CLI/TUI execution-boundary target/resource gate

Runtime CLI and TUI Runner execution-boundary previews now keep a selected
client instance behind the owner-bound session/resource pair and refreshed
inventory-v2/resource pair before the metadata-only candidate. The lease-proof
target must match an observed device ID or Runner instance ID; foreign targets,
missing observations, and one-sided TUI projections stop with zero POST. CLI
unfiltered input remains compatible and local `--instance-view` accepts only a
validated resource or converged fixture. No Runner transport, payload
execution, reservation, lease mutation, receipt, Audit, or P4 authority was
added;
ADR-0039 remains planning-only and ADR-0114 remains Proposed/null.

### Cross-device plan §753 — Runtime TUI execution-boundary target/resource guard

The selected Runtime TUI execution-boundary preview now requires the opened
owner-bound session/resource declaration and a refreshed, converged
inventory-v2/resource pair before its metadata-only candidate. Conversation
visibility is rechecked after refresh, and the request target must match an
observed device ID or Runner instance ID. Missing, one-sided, drifted, or
foreign observations remain zero-POST; unfiltered TUI behavior stays
compatible. No Runner transport, execution, reservation, lease mutation,
receipt, Audit, or P4 authority was added; ADR-0039 remains planning-only and
ADR-0114 remains Proposed/null.

### Cross-device plan §754 — Android shared-session origin and resource boundary

The opt-in Android shared-session coordinator now validates an owner-bound
selected client-instance session/resource pair and accepts only HTTPS or
loopback HTTP origins before bearer input enters the explicit emulator path.
Public HTTP, paths, credentials, query/fragment, invalid ports, and resource
drift fail before ADB. Host-only tests remain request-free; no enrollment,
heartbeat, inventory mutation, scheduling, Runner, execution, receipt, Audit,
or P4 authority was added; ADR-0039 remains planning-only and ADR-0114 remains
Proposed/null.

### Cross-device plan §755 — Console Web/App/Mobile execution-boundary target/resource gate

The shared Console Sessions surface now refreshes the selected client-instance
session/resource observation immediately before the metadata-only Runner
execution-boundary candidate. The boundary target must match either an
owner-bound device ID or Runner instance ID from that resource image; a
foreign target, selected mobile resource drift, or revoked session remains
zero-POST and hidden. Paired session/resource observations are also used for
target binding, while the default candidate remains disabled and the
no-resource-reader compatibility path is unchanged. No Runner transport,
payload execution, reservation, lease mutation, receipt, Audit, or P4
authority was added; ADR-0039 remains planning-only and ADR-0114 remains
Proposed/null.

### Cross-device plan §756 — Real JWT Console execution-boundary convergence

The accepted Forge Core EXECUTE harness now mounts the owner-scoped
client-instance session/resource projection for Web, desktop App, and Mobile
and drives a dedicated Flutter E2E with the real Snaplink JWT. The test proves
each selected instance sees the same Conversation and owner-bound device or
Runner instance target before one metadata-only execution-boundary POST; the
response remains display-only with all authority false. No Runner transport,
payload execution, reservation, lease mutation, receipt, Audit, or P4
authority was added; ADR-0039 remains planning-only and ADR-0114 remains
Proposed/null.

### Cross-device plan §757 — Console Web/App/Mobile Attempt-boundary target/resource gate

The shared Console Sessions surface now validates the selected Runner Attempt
target against freshly refreshed owner-bound client-instance session/resource
and inventory/resource images before invoking the metadata-only Attempt-boundary
candidate. Device IDs and Runner instance IDs are accepted only from the same
owner resource image; foreign targets stop with zero POST, and a response target
is checked again against the current image. No Attempt persistence, reservation,
Runner transport, execution, receipt, Audit, or P4 authority was added; ADR-0039
remains planning-only and ADR-0114 remains Proposed/null.

### Cross-device plan §758 — Runtime CLI/TUI Attempt-boundary target/resource gate

Runtime CLI Attempt-boundary previews now support `--instance` and refresh the
owner-bound session/resource pair followed by the inventory-v2/resource pair
before the metadata-only POST. The selected Conversation must remain visible,
and the command target must match an observed device ID or Runner instance ID.
Local `--instance-view` accepts only validated resource or converged fixtures;
session-only declarations cannot establish target membership. Runtime TUI
requires opened session/resource and inventory observations, refreshes the
inventory/resource pair, and rechecks visibility and target membership before
POST. Missing observations, hidden or revoked sessions, malformed or drifted
resources, and foreign targets fail closed; unfiltered behavior remains
compatible. Oversized CLI argument, dispatch, and help files were split along
their existing command boundaries. No Attempt persistence, reservation, lease
mutation, Runner transport/execution, receipt, Audit, or P4 authority was added;
ADR-0039 remains planning-only and ADR-0114 remains Proposed/null.

Validation for §758: the final Runtime CLI binary passed all 1,060 unit tests,
and the independent fresh-context review passed. The standalone structural
gate still reports 44 violations in files outside this increment. Formal
`forge accept` completed with `REJECTED` because its candidate journal lost
watch coverage; its per-criterion results therefore do not establish a valid
whole-repository verdict. This increment does not claim full acceptance.

### Runtime standalone preview — version identity and repeatable local packaging

Runtime CLI now supports `--version` / `-V` and an exact JSON name/version
object without opening or migrating Hub state. The version comes from Cargo;
the preview manifest separately records source revision, selected-source
fingerprint, dirty paths, toolchain, dependency requirements, and binary digest.
`scripts/package_forge_runtime.py` builds the pinned Linux x86_64 target with
locked offline dependencies, strips a copy, runs the standalone offline smoke,
and produces an unsigned local archive plus checksums without replacing existing
artifacts. The smoke checks actual read contents, edit effects, zero process exit,
and durable results against loopback provider fixtures; it makes no real model
request. Python packaging regressions are wired into the existing CI workflow.

Validation: 1,067 CLI unit tests, 3 version integration tests, 14 Python regression
tests, and all 10 smoke checks on the final release binary passed. Fresh-context
review passed after fixing pinned-toolchain selection, failed-read false positives,
and concurrent output-directory replacement. Changed Rust files pass formatting;
governance passes 13 checks after restoring the existing domain registration
file's required 0644 mode. Full-repository formatting, structural/architecture
checks, and strict Clippy remain blocked by existing findings outside this
increment (44 structural violations; Clippy stops on 55 domain errors).
This local preview does not claim full acceptance, cross-distribution validation,
signing, publication, or stable-release readiness.

### Runtime preview quality — Domain lint and formatting cleanup

The Domain production library's 55 strict Clippy findings are resolved through
explicit error contracts, checked history-count conversions, and smaller pure
validation/projection helpers. Independent boolean authority fields retain their
frozen wire shape with narrowly scoped, documented lint allowances. Validation
order, output ordering, and authority values remain unchanged; history boundary
coverage now includes empty, single, maximum, oversized, and unrepresentable
counts. No boundary-checker whitelist or pinned metadata was changed.

Runtime dispatch and TUI write paths are split along their existing operations,
and the instance-filter fixture is separated from its tests. Request order,
visibility checks, failure recovery, and output-error propagation remain intact.
Whole-workspace Cargo formatting now passes, and the structural file violations
fall from 44 to 43, all outside the files changed by this increment.

Validation: strict Domain library Clippy and whole-workspace Cargo formatting
pass. Domain all-target tests report 715 passed and 3 failed in the existing
Attempt-request boundary snapshots: two dependency-manifest fingerprints and
one exact lifecycle module declaration. Their checker inputs are byte-identical
to HEAD; this increment does not repin them. Strict CLI Clippy now reaches the
interfaces crate and reports 269 errors outside the changed files. Repository
architecture checks still report 12 package, 2 fan-in, 1 naming, and 540
function-length findings; none identify a changed file. Fresh-context Domain
and Runtime reviews pass. Full repository acceptance remains unestablished.

The CLI binary and selected device/version integration targets pass 1,089 tests;
the final scheduler-preview cleanup also passes 6 focused tests. All 14 Python
packaging regressions and 10 standalone smoke checks pass. The refreshed local
archive is `dist/forge-runtime-0.1.0-preview-d44379b77509-linux-x86_64.tar.gz`
(10,420,272 bytes; SHA-256
`310c923f8e23f7300d6c72bbca2b314f191768e9b0e8462c427055afc7ca2b4e`).
It remains an unsigned Linux x86_64 developer preview requiring glibc 2.39 and
D-Bus, tested with loopback provider fixtures only; no publication or real-model
validation is claimed.

### Runtime preview quality — CLI lint and reviewed boundary snapshots

The production CLI now passes strict Clippy with the pinned, locked/offline
toolchain; all 269 findings from the previous increment are resolved. Device,
remote command, and TUI operations are split along existing validation,
rendering, and refresh boundaries. Wire fields and Serde type names, error
messages, request/validation order, authorization clearing, rollback snapshots,
writer-error propagation, and cursor commit order are preserved. Whole-workspace
Cargo formatting and the 13 governance checks pass. Independent fresh-context
reviews of the device/boundary, remote, and TUI changes all pass.

The three stale Attempt-boundary tests are repaired after reviewing their
historical inputs: the CLI manifest added `futures-util` for bounded SSE reads;
the exact test include host added only base64/time imports; the registered
execution leaves already existed. The preview consumer's exact digest reflects
its reviewed equivalent function extraction. The consumer scanner and exact
module declarations remain restrictive, with five new regression tests for
manifest changes, aliases/globs, module registration, include hosts, and preview
source paths/bytes. The tokenizer was split without changing lexical rules.
Domain all-target tests now pass all 723 cases across 22 targets, including all
27 Attempt-boundary checks.

Repository structure violations fall from 43 to 31, and function-length
findings fall from 540 to 446; no finding identifies a changed file. The
remaining 12 package, 2 fan-in, 1 naming, and 446 function-length findings still
prevent whole-repository acceptance. Strict production CLI lint success does
not claim all-target/test lint success or stable-release acceptance.

Final validation: the CLI binary unit tests plus all 15 device integration
targets and the version target pass 1,130/1,130 cases across 17 targets. All
14 Python packaging regressions and 10 smoke checks on the refreshed binary
pass. The new archive is
`dist/forge-runtime-0.1.0-preview-389127c43562-linux-x86_64.tar.gz`
(10,464,343 bytes; SHA-256
`2249c0ef8fa6e070b1b433a290b14780bfbf6f4621872219c0a71c93cbf5622f`).
The stripped executable is 27,309,984 bytes. It remains an unsigned local Linux
x86_64 preview requiring glibc 2.39 and D-Bus, validated with loopback provider
fixtures only. Existing archives are preserved; no real-model validation,
cross-distribution validation, signing, or external publication occurred.
