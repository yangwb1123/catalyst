# ForgeOS 扩展路线图(Extension Roadmap)

> **本文件是前瞻性提案**:基于对当前代码库的一次全局扫描,提出未来最高价值的扩展方向与「为什么」。
> 项目的**版本纪律事实源**仍是 [`.agent/ROADMAP.md`](.agent/ROADMAP.md)(v0✅ / v1✅ / v2 进行中 / v3;
> 其勾选比被 `forge-core` 的 `converge` 实算为收敛信号,**勿与本文件混淆**)。本文件不替代它,
> 而是为其 v2/v3 段提供资深架构师 / PM 视角的细化、取舍与优先级。

## 基线:已实现的能力(扫描确认)
- **Harness 执法层(真拦)**:`gate.mjs`(体积)、`arch-check.mjs`(layering 真解析 import + package/fanin/cognitive/naming)、`check.py`(治理完整性)、`acceptance.mjs`(`forge accept` Stop 闸门,聚合 + 诚实 N/A)。
- **forge-core 运行时骨架(纯 Go stdlib 零依赖)**:asset loader · 状态机 orchestrator · gate fail-closed · CommandExecutor(能驱动真进程)· converge(活收敛)· LoopEngine(自治循环 + anti-doom-loop tripwire)· prompt/Context Engine v1。
- **声明式治理层**:9 角色卡 · 8 skill · 4 workflow(Discover→Design→Build→Evolve)· mode×lifecycle 矩阵 · 多维路由策略 · eval/scorecard schema。
- **Dogfood**:`url-shortener`(Node, 47 测试)+ `go-taskd`(Go, 22 测试)两个 Clean-Arch app,被 `forge accept` 真 gate;polyglot 治理已验证成立。

## 结构性缺口(一句话诊断)
**治理设计 ~85% 完整,运行时覆盖 ~50%。** 脊柱、中枢旋钮、Clean-Arch 实时执法都已落地;但「真·24h 无人值守不腐化」所需的**运行时韧性、学习闭环、长程记忆、polyglot 执法补完、安全合规**这五块,正是「声明强、落地弱」的高杠杆地带。下列 5 个方向逐一补齐它们。

---

## 方向一 · 韧性运行时(P0)— 从「能跑一次」到「敢跑 24 小时」  ✅ 已交付

> **状态(2026-06-19,dogfood 实现)**:超时/取消 · 错误分类(`ExecError` + `Retryable` 驱动 `Engine.MaxRetries` 重试)· 结构化 trace(`internal/trace`,JSONL)· 断点续跑(`internal/persist` checkpoint 原子写 + `--resume`,畸形硬报错)· 治理配套(`.forge/` 入 .gitignore/SKIP_DIRS)。fresh reviewer APPROVE,整仓全绿(go test 9 包 + gate/arch-check/check/accept)。剩「真点火 `--agent-cmd=claude`」属运维(需凭证/预算)。
**现状**:forge-core 能端到端跑通 `forge run/evolve`,且 anti-doom-loop tripwire 已挡住「反复无进展」这一类自治杀手。但 agent 阶段**默认 dry-run**;`--agent-cmd=claude` 真实执行器机制已建+测、本机 `claude` CLI 已证可调,**尚未点火**。

**要补**
- 〔核心〕**点火真实 agent**:接通 `--agent-cmd=claude`,从 dry-run 迈到真无人值守(机制已就位,差凭证/预算/守护)。
- 〔edge〕**超时 + 取消**:`command_executor.go:34` 的 `CombinedOutput()` **无超时上界**——agent 卡死会无限阻塞拖垮全链路。需 `context.WithTimeout` + SIGTERM,每个 phase 可配超时。
- 〔edge〕**断点续跑 / 状态持久化**:`orchestrator.go`、`loop.go` 状态全在内存,崩溃即从头 replay,丢失已完成 phase 与 git 进度。需每 phase/iteration checkpoint(位置 + signals),崩溃后 resume 而非 replay。
- 〔edge〕**错误分类**:`command_executor.go` 把「CLI 不存在 / 进程崩溃 / exit≠0 / 磁盘满」混为一个 error,循环无法决定「重试 vs 人工 vs 停机」。需 typed error(transient/permanent/config)驱动恢复策略。
- 〔核心〕**Observability/trace**:运行时只有 `Log func(string)` 文本日志,24h 盲飞、多层子进程无法关联。需结构化 trace(先 JSONL、后 OTel):iteration/gate/agent 耗时、token、错误。

**为什么**:这是第一性目标「24h 无人值守」的地基。tripwire 只挡住了「空转」一类杀手,而「agent 卡死 / 容器重启 / API 限流」三类同样会让无人值守崩盘,目前**全无防护**。没有这层,「自治」只能在人盯着时短跑。

---

## 方向二 · 学习闭环(P0)— Eval → Scorecard → Router 真闭环  ✅ 已交付

> **状态(2026-06-19,dogfood)**:scorecard 记 trajectory(`avg_iterations`/`rework_rate`)· forge-core 读 scorecard 补 `history-tiebreak`(决策链完整,`forge route --scorecard` 历史可观测)· converge 认 acceptance per-criterion(`test_pass` 等,NA→unmet 诚实)。fresh reviewer APPROVE(独立验证 merge 加权数学 + 3 个 honesty 点)。剩「trajectory 自动采集源(真 agent/trace)」随真点火接通。
**现状**:声明式 schema 完整(`eval/acceptance.schema.yml` + `routing/scorecard.schema.yml` + `policy.yml` 的 history 段),`harness/scorecard.mjs`/`scorecard-update.mjs` 有纯函数写入器,forge-core routing 有多维打分 + 硬 Opus 底线。**但闭环没闭合**:scorecard 无自动聚合;Router 决策时**不查** history(只有硬规则);`converge.go:47` 只认 `roadmap_completion`+`gates_status`,其余 metric 一律判 unmet。

**要补**
- 〔核心〕**Trajectory scorecard**:从「记结果」升级为「记过程」——哪个 agent×mode×task-type、几轮收敛、返工几次、reviewer 打回几次、命中哪条闸门。
- 〔核心〕**Eval Engine 自动聚合**:每轮把 acceptance 结果 + reviewer verdict 落库进 scorecard,无需人工。
- 〔核心〕**Router 真查 history**:`policy.yml` 的 `history.tiebreak_on` 在选档时真消费 scorecard,让「Sonnet 实现通过率 78%」驱动「同类任务自动升 Opus」。
- 〔成本〕**converge metric 扩展**:把 `converge.go` 的 `default→unmet` 改为可扩展 metric 注册,认 coverage 趋势 / gap-report / 自定义指标。

**为什么**:这是扫描三路一致指向的**最高杠杆未闭环**,也是 ForgeOS 真正的**护城河**——让它「越用越聪明」,是大厂通用 agent 碾不平的领域资产。成本上从「总用 Opus 保险」→「数据驱动档位」,可省 15–30% token。契合 2026 共识:**evals 是 AI 产品的核心竞争力,且要测 trajectory 而非只看最终答案**。

---

## 方向三 · Context / Memory 引擎(P1)— 让 agent「记得为什么」  ✅ 已交付

> **状态(2026-06-19,dogfood)**:检索器 `internal/prompt/retrieve.go`(TF·IDF-lite top-K,确定性)· 跨会话记忆 `internal/memory`(JSONL,fault-tolerant,过 -race)· Context Engine 升级为「**硬约束始终注入** + 检索式相关上下文 + memory 注入」,evolve 每轮写运行轨迹。fresh reviewer APPROVE(/tmp 副本对抗证明 6 条硬约束在 6 种查询下不被检索过滤)。**dogfood 纪律**:接线令 main.go 达 499/500 → 按「先拆分」抽出 `evolve.go`(499→299,零行为变化)。剩 embedding 检索(v3)+ 真 agent 语义发现(真点火)。
**现状**:Context Engine v1(`forge-core/internal/prompt`)已注入项目 ground truth:`docs/adr` 的 ADR 标题 + `.agent/AGENTS.md` 前 6 条硬约束(`leadingBullets`),容错。但注入是**固定、全量、浅**的——不按 task 检索、无跨会话记忆、无 RAG。

**要补**
- 〔核心〕**检索式上下文**:从「固定注入」→「按当前 task 检索相关 ADR / 代码符号 / 历史决策」(先 grep/symbol index,再 embedding)。
- 〔核心〕**跨会话记忆**:持久化「已发现的 gap / 已做的设计决策 / 已踩的坑」,喂回 evolve 循环,避免连续 N 轮重复发现同一问题。
- 〔核心〕**RAG over `.agent/` + `docs/adr` + git history**:新接手的 agent 快速获得项目背景,不必每轮从头分析。

**为什么**:第一性目标「不腐化」的技术核心。24h 自治到第 8 小时,agent 会拆掉自己第 2 小时建的墙——因为它**不记得为什么建**。长期记忆是 evolve 循环真正收敛、而非在同一批 gap 上震荡的前提。契合 2026 共识:**context engineering > prompt engineering,长时自治靠检索而非更长的 system prompt**。

---

## 方向四 · Polyglot 执法补完 + Harness 硬化与提速(P1)  ✅ 核心已交付

> **状态(2026-06-19,dogfood)**:**消除两个最大「声明但未执法」gap** —— `arch-check` 加 function-length + circular-dependency 机器执法(policies.yml TODO→ENFORCED,6→8 checks)。加检查时抓到 2 个真违规 → **原则性拆函数**(绝不放宽)。fresh reviewer 抓出 brace-matcher 多行字符串**假阴性**(执法器假绿)→ 已修(跨行字符串状态)并加决定性 fixture;acceptance Stop 闸门现覆盖 arch 测试(canary 证明)。两处上帝文件(main.go 499、scan.mjs 499)按「先拆分」纪律拆分。剩:性能(collect 并行)+ probeTests 语言自适应——价值在大仓/fork,本仓难验证,不为不可验证的优化镀金(lifecycle 纪律),记为后续。
**现状**:harness 三件套真执法、adapters/{go,python,typescript}.yml 已声明各语言 lint/test/coverage 命令。**但**:函数行数 ≤50、循环依赖 = 0 在 `policies.yml:4,6` 是 **TODO,从未执法**;adapters/*.yml **纯声明、无任何消费者**(grep 不到引用);多处 fail-open(`gate.mjs` 文件读失败静默 `continue`;`scan.mjs` 自写 YAML 解析器对引号/多冒号/流式写法脆弱;`arch-check.mjs` checkDrift 正则可被注释行干扰);`acceptance.mjs` collect() **串行 spawn 5+ 进程**、gate/arch-check/check **各自重复遍历整棵源树**、`probeTests` 硬编码 3 条命令非自适应语言。

**要补**
- 〔核心〕**执法补完**:实装 adapter 加载器,把函数行数(eslint/ruff/golangci-lint)、循环依赖(madge/import-linter/go 适配器)从声明接成可执行;probeTests 按项目语言自适应。
- 〔edge〕**fail-closed 硬化**:文件读失败计为违规而非静默跳过;YAML 解析换稳健方案或加固;checkDrift 防注释干扰。
- 〔性能〕**并行 + 单次扫描缓存**:collect() 改并行 probe;统一一次 `walk(ROOT)` 供 gate/arch-check/check 共用,消除重复 IO。预期 `forge accept` ~10s→~5s,在 evolve 24h 循环里收益随迭代数线性放大。

**为什么**:polyglot 的「领域化、确定性、不撒谎的 verifier」是 ForgeOS 对 Devin/Factory 的差异化,但函数/循环依赖执法缺位 + 适配器悬空让承诺打折。性能上,`forge accept` 是开发者反馈循环 + evolve 自治循环的**关键热路径**,串行重复扫描是可量化的浪费。本方向一举覆盖你要的 **edge cases + 性能**两类。

---

## 方向五 · 安全与合规闸门(P2)— 企业采纳的付费楔子  ✅ 核心已交付

> **状态(2026-06-19,dogfood)**:secret 扫描 `harness/secret-scan.mjs`(5 模式 + key-name gate 低假阳性,`security_findings` 从 N/A→真查 PASS,**纳入 LOAD_BEARING**——有 secret 即 REJECT)· 风险分类器 `forge-core/internal/risk`(特征→risk 级别,给 `critical→Opus` 硬规则提供输入源,`--risk` override 只升不降)。fresh reviewer agent 遇 API 故障 → 主控亲自审 + **对抗运行验证**(检出真 AWS/GitHub secret、不误报正常代码、critical→Opus 实测、纯叶子)。**dogfood 亮点**:方向四的 function-length 执法在并行开发中抓到 risk_test 的 113 行测试函数 → 被迫重构为包级表。剩 SCA/CVE 依赖漏洞扫描(需漏洞库,v3)。
**现状**:Router 有 `safety_override`(risk≥critical 强制 Opus,已实测 security→opus)。**但**:**无风险分类器**——`policy.yml` 的 risk 维度列了 blast_radius/reversibility/prod_traffic 信号,却无工具从代码变更**计算** risk,硬规则「等着输入却没有输入源」;**无安全闸门**——acceptance 的 `security_findings` 标 N/A(无 SAST/SCA/secret 扫描),对 OWASP 2025-12 发布的 Agentic Top10(excessive agency / tool misuse / prompt injection)无任何对应检查。

**要补**
- 〔核心〕**安全闸门类**:secret 扫描 + 依赖漏洞(SCA)+ SAST,接入 acceptance 成 criterion(从 N/A → 真查)。
- 〔核心〕**风险分类器**:从 diff 特征(是否改支付/权限/迁移?blast radius?可逆?)推导 risk 级别,给 Router 的 critical→Opus 硬规则提供真实输入。
- 〔合规〕**审计轨迹**:复用方向一的 trace,沉淀为可审计记录(谁 / 哪个 agent / 何时 / 改了什么 / 过了哪些闸门)。

**为什么**:这是 Factory/Devin 还没占死、CISO 最愿付费的位置。**监管时间窗紧迫**:EU AI Act 高风险义务 **2026-08-02 生效**(罚款达全球营收 7%),审计轨迹是硬要求。安全必须从「快速迭代下被省略的人工判断」变成「自动闸门」,才扛得住无人值守。

---

## 优先级与依赖

| 方向 | 优先级 | 覆盖维度 | 依赖 | 一句话杠杆 |
|---|---|---|---|---|
| 一 韧性运行时 | **P0** | 核心 + edge | 无(地基) | 没有它,「24h 自治」是 PPT |
| 二 学习闭环 | **P0** | 核心 + 成本 | 弱依赖①(trace 供数据) | 越用越聪明,真护城河 |
| 三 Context/Memory | P1 | 核心 | 弱依赖② | 长程不腐化的技术核心 |
| 四 Polyglot + 硬化提速 | P1 | edge + 性能 | 无(独立可并行) | 兑现差异化 + 提速热路径 |
| 五 安全合规闸门 | P2 | 核心 + 合规 | 依赖①(trace) | 企业付费楔子 + 监管时间窗 |

## 收敛建议
- **只做一件**:方向一(韧性运行时)——它是其余一切的地基,且把「只差点火」的 forge-core 真正点着。
- **做前两件**:① + ②——「敢跑 24h」+「越跑越聪明」,正是 ForgeOS 第一性目标的最小完整闭环。
- 方向四独立、不阻塞任何其他方向,可作为**低风险硬化轨**随时并行插入。
- 方向五在拿企业场景前可缓,但 EU AI Act 时间窗使其不宜晚于 v3。
