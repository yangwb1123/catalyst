# ForgeOS 扩展路线图(Extension Roadmap)

> **本文件是前瞻性提案**:基于对当前代码库的全局扫描,提出最高价值的扩展方向与「为什么」。
> 项目的**版本纪律事实源**仍是 [`.agent/ROADMAP.md`](.agent/ROADMAP.md)(v0✅ / v1✅ / v2 进行中 / v3);
> 本文件不替代它,而是为其 v2/v3 段提供资深架构师 / PM 视角的细化、取舍与优先级。
>
> **本轮扫描方法**:三位架构师独立全局深扫(forge-core / harness / 真点火-pipeline),每条诊断均带
> 代码 `file:line` 证据 + 实跑证伪;**最高置信度信号是跨-agent 共识**(方向一被两位独立发现为同一真 bug)。

## 已交付基线(扫描确认)

**第一轮五方向全部交付**:① 韧性运行时(超时/取消 · `ExecError` 重试 · trace JSONL · checkpoint/resume)
② 学习闭环(scorecard trajectory + `history-tiebreak` + converge per-criterion 框架)③ Context/Memory
(retrieve TF·IDF + memory JSONL + Context Engine 注入)④ Polyglot 执法(arch-check 8 检查 + function-length/
circular 机器执法)⑤ 安全合规(secret-scan + risk 分类器 + SCA 框架)。

**真点火接通(2026-06-20)**:`--agent-cmd=claude` 真 multi-agent 自治跑到 converge MET(增量+版本级);
真跑暴露并修**八个 gap**(task 注入 / 写权限 / 模型路由 / cwd / budget / trace-latency / cost-telemetry /
reviewer-gate-信号);**Learning loop 三维真数据**(quality+latency+cost)落盘;**pipeline 数据流注入**
(gate 裁决→reviewer · planner 拆分→下游,避污染);CC PostToolUse edit-time gate;agent-os 仓化 ADR 0003。

## 结构性新诊断(真点火接通后)

第一轮把治理设计 + 运行时**基础设施**做到约 90%,但其中大量是用 **echo/stub 验证**为「机制就绪」。真点火
接通真 claude 后,**下一层 gap 浮现** —— 它们正是 echo 测不出、只在真 LLM 多-agent 长跑下暴露的四类:
**反馈环的最后一环断裂、执法器的假阴盲区、真 claude 的失败模式、真点火的成本/墙钟浪费**。下列 5 个方向
逐一补齐,把 ForgeOS 从「真点火能在 throwaway 验证」推向「敢 24h 无人值守真跑」。

---

## 方向一 · 接通 reviewer 裁决回流 —— 脊柱的质量反馈环现在是断的(P0,真 bug)

**诊断(两位架构师独立印证)**:`build.yml` 的 reviewer/qa phase 声明了 `on_fail:{action:loop_back,
target_phase:implementer}`,但 orchestrator 的 loop-back **只在 gate phase 触发** —— `RunFrom`
(`orchestrator.go:203-213`)只对 `len(p.RequiredGates)>0` 的 phase 调 `gateOutcome`;reviewer 是 agent
phase(`required_gates:[]`),走 `runAgentPhase`,其 `OnFail` **被解析、存储、然后静默丢弃**。reviewer 的
REQUEST-CHANGES 输出也无人解析(`observeFor` 只对 feeds_forward phase 记,而 reviewer 故意不设 feeds_forward
以免污染 fresh-context)。

**为什么 P0**:reviewer 是 vision Build 脊柱(`…→Reviewer→QA`)的关键质量裁决层,真点火下烧 opus 钱产出裁决,
却**对控制流零影响** —— 多-agent 协作退化成「implementer 写完只要 gate 绿就过,reviewer 形同虚设」。这是
"声明 vs 实现"漂移的典型(正是 Sprint 12 审计要抓的那类),且是 ForgeOS 自纠能力的核心缺口。

**方向**:让 agent phase 也能触发 loop-back —— reviewer card 产出机读 verdict(末行 `VERDICT:
REQUEST_CHANGES`),经 `Observe` sink 解析(类似 `cost.go` 解析 claude JSON),在 `runAgentPhase` 后判
verdict + phase 的 `OnFail`,复用现成的 `gateOutcome`/loopBacks 预算机制定向跳回;reviewer 的"具体问题"
经一条**不污染 fresh-context 的单向边**注入 implementer(不回灌给下次 reviewer)。

## 方向二 · 闭合 Learning loop 最后一环 —— scorecard 测了数据却无人读(P0,vision 护城河)

**诊断(产消三接头全断,构成一条断裂的闭环)**:① **消费端**:真点火选 `--model` 走
`PhaseTier→routing.TierFor`(`routing.go:62`),**完全不读 scorecard**(只 opus-floor + agentTier +
modeDefault);`HistoryTiebreak`/`LoadScorecards` 唯一调用点是独立的 `forge route` 人工命令(`route.go:165`),
真点火路径从不调。② **生产端**:`scorecards.json` **文件根本不存在**,`scorecard-update.mjs` 能写它但
**真跑后无人自动调**(header 自承 "auto-collection NOT wired yet")。③ **归因断层**:cost 事件是
`Kind:"agent", Name:phase`(`cost.go:64`)按 phase 名记、不按 model 名,而 scorecard 主键是
`(model, task_type)` —— trace 里没有"哪个 model billed 了这个 phase"。

**为什么 P0**:这是 vision「Eval→记分卡→Router 回灌」整个学习闭环的**字面最后一环**,也是 ForgeOS 真正的
护城河(越用越聪明)。框架(schema/merge/decay/tiebreak/route 展示)第一轮全交付了,但 `forge route` 里那段
`HistoryTiebreak` 当前是**纯装饰**(读的文件永不存在、结果从不喂给真 `--model`)。差的就是把产消三个接头接上。

**方向**:(a) `forge run/evolve` 收尾自动 shell `scorecard-update.mjs`(传 `--trace .forge/trace.jsonl` +
每 phase 实际 routed model + iteration/rework 信号);(b) cost/agent trace 事件**附带 model 字段**(让
producer 按 model 归因);(c) `PhaseTier` 增可选 `scorecardPath`,对同档候选跑 `HistoryTiebreak`(v1 单候选
下是 no-op + observability,接头通了,v3 扩 `tiers.models` 即生效)。

## 方向三 · 根治执法器假阴盲区 —— 对"治理 OS"比任何功能缺失都伤根本(P0,治理地基)

**诊断(多处真红线经执法器盲区漏网,本仓已活体命中)**:
- **arch-check 看不见 JS/TS `export-from` / `export *` / 动态 `import()`**(`scan.mjs:92-102` 只匹配
  import…from / require)—— barrel 文件(依赖聚合首选)对导入图**隐形**,任何经再导出的层级违规/循环依赖
  **直接绕过** layering+circular 两大检查。**活体证据**:`acceptance.mjs:32` 自己就是 `export {…} from`,
  此刻就没记进图。
- **函数长度闸门对 Python `async def` 全失明**(`scan-functions.mjs:159` 正则 `^(\s*)def` 不匹配
  `async def`)—— FastAPI/asyncio 每个异步函数的标准写法,500 行也不触发 50 行红线。
- **secret-scan 结构性扫不到 `.env`**(`secret-scan.mjs` 用 `extname(".env")===""` 门控)—— 凭证泄露第一
  现场,而 `security_findings` 是 load-bearing,闸门会**自信报 PASS**。
- **acceptance 主测试 glob 非对称 fail-open**(`acceptance.mjs:78` 只判 `.ok`,零匹配 glob exit 0 → 可
  **零测试假绿**,keystone `test_pass` 通过)。
- **check.py 漏检 workflow 控制流**(悬挂 `target_phase`/`model_tier` 过校验 PASS 但 loop 跳向不存在的
  phase)+ 双 YAML 解析器分裂(执法器手写 `parseRules` 与校验器 PyYAML 看不同的文件)。

**为什么 P0**:ForgeOS 把「带外 gate 为真相之源」立为宪法。执法器**假阴**(红线漏网而报 PASS)直接动摇整个
治理可信度 —— 它让 agent 以为过了闸门、实则红线在裸奔。多个盲区**本仓已活体命中**(export-from 边、
`test_enforce.mjs` 已被 forge-init 漏复制)。修复成本极低(几行正则 / basename 匹配),杠杆最高。

**方向**:`extractJsImports` 加 export-from/export-star/dynamic-import 三条正则;`indentFunctions` 正则加
`(?:async\s+)?`;secret-scan 按 basename 匹配 `.env`/`.npmrc`/`id_rsa` + 补 provider-anchored 模式;
acceptance 把 fail-closed 的 `count>0` 守卫套到 `harness/test_*` glob;check.py 加 workflow 控制流引用检查
(`target_phase`/`model_tier`/`next_stage` ∈ 已知集);forge-init 加 copy-list 完整性守卫(防漂移)。

## 方向四 · 真长跑韧性 —— 从「echo 验证机制就绪」到「敢 24h 真 claude 无人值守」(P1)

**诊断(第一轮韧性 ✅ 但用 echo/stub 验证,真 claude 失败模式未覆盖)**:
- **claude 失败模式覆盖不全**(`command_executor.go` `classifyRunErr` 把非零 exit 一律 `KindFailed`
  不可重试)—— rate-limit/529 过载(长跑最高频)、budget 烧穿、OAuth 过期全被一刀切成"永久失败、立即
  abort",**一次 529 就让几小时的 run 死**。echo 不会 529,所以之前测不出。
- **checkpoint 粒度只到迭代号**(`checkpoint.go:45` 无 phase 索引;`loop.go:111` checkpoint 写在
  `onIteration`、崩在迭代中途整迭代无 checkpoint)—— resume 从 phase 0 整轮重放,真 claude 下崩在 qa 会把
  planner+implementer+reviewer 全部重跑(每 phase 真实计费 ~$0.18)。
- **子进程信号缺进程组**(`command_executor.go:110` 注释自认只 SIGKILL 直接 child)—— 真 claude spawn 的
  MCP/bash 孙子进程成孤儿继续烧 token,四维护栏的"时间"维实际不生效。
- **prompt 注入无总预算**(各 lane 独立 cap 但无合计上界,memory lane `prompt_context.go:247` 完全无
  cap)—— evolve 长跑 memory 单调增长,prompt 膨胀直到超 claude 上下文窗。

**为什么 P1**:真点火的卖点是 24h 无人值守(vision 核心),但当前韧性是 echo/stub 验证的 —— echo 不 529、不
fork 孙子、不烧 budget、不让 prompt 膨胀。这些是真 LLM 长跑**最高频**的失败模式,当前几乎全无防护。

**方向**:扩 `ExecError` 分类(从 exit code/stderr 识别 rate-limit→retryable 带退避、budget-cap→降档或
报人);checkpoint 增 `PhaseIndex`/`AgentCalls`/`StartPhase` + 每 phase 落一次;Linux `SysProcAttr{Setpgid}`
+ 进程组 SIGTERM→SIGKILL;`buildPrompt` 加总 token 预算 + memory 最近-N 分页(平台隔离 + 零依赖,`syscall`
是 stdlib)。

## 方向五 · 真点火性能/成本 —— 削墙钟 + 削 token 账单(P1)

**诊断**:
- **编排器完全串行**(`orchestrator.go:203` 单循环阻塞 `Exec.Execute`,`asset.Phase` 无 `depends_on`)——
  真 claude 下每 phase 分钟级墙钟,Discover(scan/market/能力矩阵)与 fan-out implementer 天然可并行,串行
  让 24h 预算大量花在"等待"。
- **每 evolve 迭代重跑完整 acceptance 套件**(`gates.go:205`→全 harness 自测 + 全 example-app 测试 + arch
  + secret)—— 一次迭代通常只动少数文件,却全量重跑,随仓库长大线性增长。
- **context 检索每 phase 全量重算**(`prompt_context.go:234` 每 phase 调 `Gather`,重读 ROADMAP/ADR/
  AGENTS.md + 重 tokenize 检索)—— run 内这些输入不变,却 N phase 重算 N 遍;ROADMAP 还被读 2-3 遍/phase。

**为什么 P1**:真点火"烧钱/慢"。串行编排是吞吐量级杠杆(基础设施已铺路:trace 已加锁、ledger 已 run-scoped),
增量测试 + context 缓存是纯收益低风险优化。三者在 evolve 24h 多迭代下收益线性放大。

**方向**:`asset.Phase` 加 `depends_on` + 编排器按依赖拓扑分层、同层 goroutine+errgroup 并行(ledger/
agentCalls 加锁,注释已预案);增量测试选择(按 git diff 改动文件只跑相关套件,risk 包 `FromChangedPaths`
可复用);run-scoped `contextCache`(不变上下文构建一次,各 phase 复用)。诚实:并行需 architect 拍板
(并发复杂度 vs 串行简单性),增量测试不能跳过版本级全量 accept。

---

## 优先级与收敛建议

| 方向 | 优先级 | 类别 | 一句话杠杆 |
|---|---|---|---|
| 一 reviewer 裁决回流 | **P0** | 功能+边界(真 bug) | 脊柱质量反馈环现在是断的,两位架构师独立印证 |
| 二 Learning loop 回灌 | **P0** | 功能 | vision 护城河的字面最后一环,产消三接头全断 |
| 三 执法器盲区根治 | **P0** | 边界(假阴) | 治理 OS 地基,本仓活体命中,几行正则高杠杆 |
| 四 真长跑韧性 | P1 | 边界 | 24h 卖点,真 claude 失败模式 echo 测不出 |
| 五 真点火性能/成本 | P1 | 性能 | 削墙钟+token,基础设施已铺路 |

**收敛建议**:
- **若只做一件**:**方向三(执法器盲区)** —— 成本最低(几行正则)、杠杆最高(治理可信度地基),且本仓已活体
  命中、不修就是已知红线在裸奔。
- **做前三件(全 P0)**:一+二+三 —— 分别闭合「质量反馈环 / 学习反馈环 / 执法可信度」,正是 ForgeOS
  「自纠 + 自学习 + 真治理」三大支柱各自的临门一脚,且都属「框架/脊柱已在、差最后一接」的高确定性投入。
- 方向四/五随真点火 24h 长跑的实际投产需求推进;方向四的 claude 失败模式分类**不应晚于首次真·无人值守长跑**
  (一次未分类的 529 即可让数小时的 run 全废)。

> **诚实边界**:本文件是设计提案,所有方向**只读扫描得出、未写任何实现代码**。三份扫描均诚实排除了「已实现
> 项」(八 gap/telemetry/pipeline 注入已核实为真做了,不列为缺失)与「不可验证的镀金」(如 sca 全库索引优化
> 需先接入真 OSV-DB)。各方向落地前仍走 ForgeOS 自身的 fresh-context review + 全闸门验收纪律。
