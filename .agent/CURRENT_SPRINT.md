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

## 下一前沿(需外部资源 / 投机增强 / 架构外,非本环境可完整验证)
- **真点火** `--agent-cmd=claude`:机制 + **资源安全护栏四维完整**(深度 recursion / 数量 budget / 时间 timeout / 内存 output-cap,Sprint 20–22)+ retry + loop-back + 凭证透传 已就位,差凭证 + 预算确认即可安全启用(方向①②③的真采集源/真语义发现随此解锁)
- **需外部资源(框架已就绪)**:SCA/CVE 漏洞库 OSV/NVD(差 DB)· 真 cost/latency telemetry(差真 token 数据)· 跨厂商池 LiteLLM(差多厂商 keys)· Firecracker 沙箱(差 KVM/特权)
- **投机增强(做即违反反 gold-plating 纪律)**:embedding 语义检索(TF-IDF 已工作,增量仅真点火时体现)
- **架构外**:Web UI(偏离 CLI/声明式核心)
- **结构债**:acceptance.mjs 499/500(下次加 probe 前先拆 probe 族,reviewer flag)

**stop_condition:** roadmap 完成度 / 闸门全绿(非「继续 N 轮」)。
