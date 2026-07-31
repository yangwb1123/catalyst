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

`run start/list/show` 已跨进程工作。新 Run 最多加载所选 user Prompt 之前 16 条完整 lowercase `user`/`assistant` causal 消息，历史正文总量严格限制为 512 KiB；Run answer 永远锚定原 user，即使晚于后续 user 才 crash-repair，多 Run 同源也会保留 source+最新 bounded answers，损坏关联失败关闭。孤立 assistant 前缀会丢弃，当前 Prompt 只追加一次。journal 严格验证 user/turn/tool/result/terminal 全状态机；`run_finished.completed` 必须等于最后 committed、无 tool call 的 assistant。完成写回由 validated Run 授权，在一个事务内创建 assistant Prompt 与唯一关联，不再依赖可伪造的内部 key 约定。相同终态重试在 API key preflight 前完成，不调用 provider/tool，只修复缺失 writeback；incomplete 与 pending-tool 失败关闭。

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

## Sprint 43（✅ 完成）— Strict Build QA verdict handshake

ADR 0014 为 Build QA 增加独立、失败关闭的 `qa_v1` 机器裁决，而不改变普通 Reviewer、Evolve 或 Release 的既有宽松/专用契约。Build 的 QA phase 必须显式声明 `verdict_contract: qa_v1`，并在所有 mode 下保留自身的 `test` gate；mode 不能跳过 QA。命令执行器保留未经清洗的原始输出，普通可执行文件只接受末个非空行精确等于 `QA_VERDICT: ACCEPTED` 或 `QA_VERDICT: REJECTED`；被解析为 Claude 的命令还必须先给出唯一、完整、成功且非 error 的 JSON result envelope，纯文本不得冒充成功 envelope。缺失、空白包装、尾随 prose、畸形 envelope、裸 CR 或未知 token 都不会制造批准。

`REJECTED` 只能回到更早、可写且不可被 mode 跳过的实现 phase；资产检查与运行时都会验证 target，默认三次 loop-back 预算耗尽后中止。dry/echo 无真实 Agent 输出，因此会在 QA 失败关闭；带严格 QA 的并行执行在启动任何 Agent 前被拒绝。QA 接受不会生成或改写 Deploy/Build/Rollback validation receipt，Release 的 operator-pinned executable、严格 JSON/verdict 与人工批准边界保持不变。

专项 Go 正反例、命令执行 E2E 与 `-race` 全绿，两份独立 fresh-context 契约复核均 **APPROVE**。经用户明确授权，真实 `codex exec --ephemeral --sandbox workspace-write` 从提交 `8b03cd1` 构建静态 Forge 二进制（SHA-256 `e07b8d987d285689a9b15d9a7c7268adcc36c0f8b68c2245fa32d00c8e115f57`），通过原生编译 Agent 子进程和真实 Node gate 跑完 **16/16** 黑盒场景：6 条成功/兼容路径、10 条失败关闭/前置拒绝路径；release receipt 30-byte sentinel 字节不变。Codex 沙箱宿主拒绝 7 个与本功能无关的 sealed-memfd release 测试，但主环境随后完整 `forge accept` 覆盖并通过全部 Go **1040 tests**、Python **68 tests**、Node **378 tests**、Rust 各 workspace、go-taskd 22 tests 与 url-shortener 47 tests，最终 **ACCEPTED**（9 pass、0 fail、2 个诚实 N/A）。

## Sprint 45（✅ 完成）— Durable lifecycle promotion migration

ADR 0016 把 lifecycle-driven dynamic migration 落成显式、可审计的持久事件：`forge migrate --to-lifecycle production [--apply]` 默认 dry 且零写入，只有 `--apply` 才持久晋升。Explorer 的真实非生产→production 边会在同一事务中变为 engineering、追加五个既有治理欠债任务并写入 production；balanced/engineering/cto 只改变 lifecycle，ROADMAP 字节不变；已在 production 且无回执的仓库保持精确 NOOP，不臆造历史迁移。旧的 `forge migrate --to engineering` 也复用同一事务内核。无显式参数时 run/evolve 读取 `.agent/project.yml`，显式 mode/lifecycle 仍只是本次调用覆盖且永不改写 selector；等待链对隐式 selector 漂移失败关闭。

两类操作共享 `.forge/run.lock`、一个 canonical pending intent 与各自独立的 terminal receipt。事务按 intent→ROADMAP→project commit point→receipt→移除 intent 的顺序 durable publish，精确绑定 before/after bytes、权限位与 digest；失败后只有匹配操作能确定性 roll-forward。所有预览、状态和 busy probe 都是 bounded、side-effect-free read；symlink/hardlink/FIFO、非 canonical/超限状态、跨操作伪造、tracked `.forge/**` provenance、selector/marker/receipt 漂移均在 mutation 或 workflow/agent/trace/checkpoint 前拒绝。`forge status [--json]` 会报告 pending、完成的 operation ID 与恢复命令；竞争提示明确禁止 unlink 锁路径，避免创建第二锁命名空间。

最终安全审查 **APPROVE**，无 Blocker/Major。专项 Go、`-race`、随机顺序、`go vet`、Windows/Darwin 交叉编译、553-source arch-check 与 12 项治理检查全绿。隔离提交树的完整 `forge accept` 两次均 **ACCEPTED**（9 pass、0 fail、2 个诚实 N/A），覆盖 Forge Go **1111 tests**、Python **68 tests**、Node **378 tests**、全部 Rust workspace、go-taskd 22 tests 与 url-shortener 47 tests。真实 Codex 首轮在提交 `29d2689` 的 13 场景黑盒验收中得到 12/13，并发现 busy 路径缺少 never-unlink 警告；修复提交 `2a72acb` 后，Codex 从干净 checkout 离线构建 VCS-pinned 二进制（SHA-256 `e20bfa8f1636f2e1d5a9ad9a926f071921e3a07a1970b60d3007e17715dd2f98`），完整复跑 **13/13 PASS**。独立 held-flock 复现 4 ms 失败关闭、tracked bytes/modes 不变、无回执，解锁后一次重试产生恰好五个不同 marker；前后 `git status --porcelain` 均为空，未调用真实 provider。

## Sprint 49（✅ 完成）— Evidence-backed Evolve scan contract

ADR 0020 把 `workflow_depth.evolve` 从 iteration budget 延伸为可执行的内容契约。shipped Evolve workflow 的唯一首 phase 显式声明 `scan_contract: evolve_scan_v1`，且必须 readonly/observe、无 gate/dependency/write、`feeds_forward:true`；有效 mode×lifecycle `EvolveDepth` 同时进入提示、dry narration 与裁决，生命周期 floor 可加严扫描但永不放宽独立 mutation authority。最终非空行只接受 `EVOLVE_SCAN_V1: {compact JSON}`。六个固定维度、finding/clear/unavailable 状态、opportunity 派生关系、depth-specific 规则与 JSON shape 全部失败关闭；证据 locator 必须指向当前仓库内已有、非 symlink、非空、≤1 MiB 的 UTF-8 regular file 与有效行号。thorough 必须覆盖全部六维、不得 unavailable，且每个 finding 都生成带共同 locator 的 candidate task；opportunistic 只报告有直接证据的 obvious opportunity，standard/advisory 不伪造 completeness 或 implementation authority。

完整 canonical report 以 64 KiB 原子上限进入后续 prompt，不能退化为普通 800-rune summary；原始输出上限为 1 MiB。checkpoint v3 把 phase cursor、canonical scan report、预算 cap/累计整数微美元、Agent-call cap/consumed 和 loop-back cap/consumed 一并持久化。串行在 spawn 前 write-ahead 预留 call，失败 attempt、verdict、正常前进和 loop-back 都先持久化；scan 完成后 resume 重验 report/evidence 并恢复 feed-forward，不再调用 Agent。恢复参数必须匹配原 cap，负成本拒绝，超大有限成本在 checkpoint/trace 同步饱和到 `MaxInt64` 而不溢出为负。serial mid-iteration 状态不得切换 parallel；native parallel 仅声明 iteration-boundary checkpoint，中断 iteration 可整体重放。未声明该 capability 的 legacy workflow 保持原行为。

实现经独立复审确认无 Blocker/Major。`go test -count=1 ./...`、完整 `-race`、`go vet`、20 项 Python workflow checks、12 项治理检查、8 项 arch-check、文件规模 gate 与 `git diff --check` 全绿。按用户要求，真实 `codex exec --ephemeral --sandbox workspace-write` 从最终源码构建二进制（SHA-256 `64a7ff7134f55df9f93213b37aac51814feb31193f195e3bc8c72b7543cb0ae1`），仅通过公共 CLI 和确定性离线 fake provider 跑完 **13/13 cases、77/77 assertions、26 次 CLI invocation**；额外验证 phase0 已耗 call 的 serial checkpoint 在任何 Agent 前拒绝 `--resume --parallel`，以及 `total_cost_usd:1e308` 在 checkpoint/trace 同为 `MaxInt64`、恢复后仍耗尽且不再调用 provider。该黑盒没有发起真实 Claude 或付费模型请求，因此不把 deterministic provider 的通过冒充 live-provider 质量证明。

## 下一前沿(需外部资源 / 后续阶段 / 投机增强 / 明确非目标,非本环境可完整验证)
- **真点火** `--agent-cmd=claude`:**multi-agent running to completion 已坐实**(Sprint 25:真 claude 多-agent 跑到 converge MET,增量级 + 版本级)。完整旋钮:四维资源护栏 + 成本三维(phase/时间/美元)+ 任务注入 + 写权限 + 模型路由 + 工作目录 + retry + loop-back;诚实分工:agent 自治增量绿、人确认版本竣工。docs/ignition.md 有完整配方 + 实测
- **需外部资源(框架已就绪)**:~~SCA/CVE 漏洞库 OSV/NVD(差 DB)~~ **已解决,见 Sprint 32**。2026-07-27 重新实测:LiteLLM 已安装,但仅发现 Anthropic 一家凭证且网络受限,缺第二厂商凭证所以无法做跨厂商验证；`firecracker`/`jailer` 均未安装，`/dev/kvm` 存在但当前用户不可读写。Docker daemon 可达（Server 29.6.1），rootless Podman 4.9.3/runc 也可查询，但容器 runtime 不是 Firecracker microVM 的等价证明，Forge 也尚未接入任何 sandbox runner。上述能力维持 blocked，所有非 `none` sandbox 请求当前会在宿主执行前失败关闭。〔真 cost/latency telemetry **已达成**——S26 真 claude 补齐真 token/cost/latency 数据,scorecard 三维真值落盘〕
- **投机增强(做即违反反 gold-plating 纪律)**:embedding 语义检索(TF-IDF 已工作,增量仅真点火时体现)
- **后续阶段**:Web UI/`forge-web` 仍属于 v3 目标架构，但不在当前 CLI/声明式核心交付阶段
- **独立大特性,非接线小修(Sprint 30 复核后从 GAP 改判)**:`internal/routing` 的完整多维评分器(complexity/dependency/context/business-impact)接入真实执行路径(目前只喂手动 `forge route` CLI)——包自身文档已自我标注为「v2+ Router service」。
- **明确后续契约**:QA 当前的机器阻断来自 required `test` gate；自由文本 QA report 的第二套严格 verdict handshake 尚未设计，不能把 prose 当批准。`on_approved` 当前只路由；若未来要在批准时物化 `.agent/*`，必须先采纳 producer/source mapping、新鲜度与原子提交契约，不能恢复已被 `forge check` 禁止的无主 `on_approved.emit`。
- **`readonly`/`on_rejected` 的真 claude 进程验证——用户已明确决策终止于此(2026-07-03)**:两机制均已真实实现(非声明未接线);readonly 路径限定按官方文档契约构造 + 单测坐实 argv、on_rejected 用 fake-agent 脚本端到端坐实目标阶段、失败保留与成功消费语义,但都未过真实付费 `claude` 进程验证运行时行为。征询用户是否授权花真实 API 预算推进最后一道经验验证,用户选择「单测已足够,就此打住」——非遗留缺口,是知情决策后的终态;若未来有人想补这道验证,预算授权需重新征询。
- **需求清单本身**:`docs/FUNCTIONAL_REQUIREMENTS_AUDIT.md`(Sprint 30 起 + Sprint 31 修订)是本仓当前唯一的显式功能需求清单,derived from 项目自己的声明源头;后续 sprint 如声明新机制,应同步补一行,不要让清单本身漂移回「不存在」。

历史勘误：Sprint 32 记录的 `/dev/kvm` 存在且可读写是 2026-07-16 当时的探测；
2026-07-27 该设备仍存在，但当前用户已无读写权限。旧 Sprint 31 的 deny-before-allow/Edit+Write
描述也已由本 Sprint 的 `dontAsk` + exact `Edit` 契约取代。
Sprint 31 的“loop-back 开始即消费 rejection”也是历史行为；当前契约为失败保留、
成功完成 rework 后消费。

**stop_condition:** roadmap 完成度 / 闸门全绿(非「继续 N 轮」)。
