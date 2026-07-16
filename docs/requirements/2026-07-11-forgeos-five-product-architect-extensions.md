# ForgeOS — 五个高价值产品/架构扩展方向（全局代码扫描）

> **角色**: 资深架构师 + 产品经理  
> **方法**: 全局逐包扫描 `forge-core/` (18 Go 包, ~35k LOC)· `harness/` (39+ 模块, ~10.5k LOC)·  
> `.agent/` (5 工作流 · 12 agent 卡 · 9 skill 卡 · 全部 ADR + DECISIONS)· `examples/`。  
> **去重保障**: 对 `docs/requirements/` (~85 份) + `docs/analysis/` (~38 份) 全部文档执行  
> 核心术语组合的全文件精确字符串搜索，**以下 5 个方向作为独立系统性概念均零命中**。  
> **纪律**: 不编写任何代码。每个方向附带 `file:line` 代码级证据 + 产品价值判断。  
> **日期**: 2026-07-11

---

## 已有 ~120 份分析的高密度覆盖域（本文不重复）

| 饱和域 | 估篇 | 本文处理 |
|---|---|---|
| 编排状态机(串/并行/loop-back/resume/mode-gating/stop-condition) | ~35 | ✅ 跳过 |
| 生产韧性(529/超时/退避/递归守卫/预算护栏/输出上限) | ~20 | ✅ 跳过 |
| 学习闭环(trace/scorecard/converge/Memory/Context 注入) | ~16 | ✅ 跳过 |
| 安全纵深(secret-scan/SCA/risk 分类/进程组/注入防御) | ~14 | ✅ 跳过 |
| 治理执法(arch-check 8 检查/check.py/drift-guard/function-length) | ~12 | ✅ 跳过 |
| 第三地平线(多仓库/Web UI/事件驱动/管道组合/daemon) | ~8 | ✅ 跳过 |
| CLI 体验(detect/preflight/doctor/status/migrate/config) | ~6 | ✅ 跳过 |
| 跨进程运行时锁/状态目录并发保护 | ~3 | ✅ 跳过 |
| 编排引擎随机测试/形式化验证 | ~2 | ✅ 跳过 |
| 路由阈值自校准 | ~1 | ✅ 跳过 |

**以下 5 个方向全部落在间隙中**，核心理念组合在已有文档中**零篇作为独立系统性方向展开**。

---

## 方向一 · 治理自保（Self-Governance Integrity）

> **优先级**: 🔴 **P1** | **类型**: 安全 · 治理 | **关键词验证**: `self.modif` `meta.govern`  
> `integrity.measur` `governance.integrity` → **全部零命中**

### 问题

ForgeOS 的核心承诺是「AI 24h 无人值守完成 Idea→Production」。`forge evolve` 的 LoopEngine 驱动的正是这个循环：agent 写代码 → gate 验证 → converge 判定。但 ForgeOS **自身** 也是一个代码库 -- 其治理规则、闸门脚本、policies、agent 卡、工作流定义全部以文件形式存在于仓库中。这意味着：

```
forge evolve build → agent writes code → agent EDITS harness/gate.mjs ← 🔥
                                                    ↓
                                          下次 gate 检查形同虚设
```

**代码级证据**：

| 文件 | 行 | 证据 |
|---|---|---|
| `forge-core/internal/orchestrator/executor.go` | `AgentExecutor` 接口 | executor 无任何 "哪些路径是治理文件、不可修改" 的概念 |
| `cmd/forge/prompt_context.go` | `buildPrompt` 中的 `Gather` | 注入的 project context 包含 AGENTS.md 硬约束，但 agent 卡本身**不声明不可写的文件路径** |
| `cmd/forge/engine_build.go` | `agentExecutor` 构造 | 传递 `--allowedTools` 白名单，白名单不含 `forge`，但**含所有 node/git 命令可间接改写治理文件** |
| `cmd/forge/main.go:25-27` | `defaultAgentAllowedTools` | 白名单允许 `Bash(node --test*)` 和 `Bash(node harness/gate.mjs*)`——gate 脚本自身可被 `node` 以文件写模式打开并重写 |
| `.agent/agents/implementer.md` | 角色卡 | 职责为「写代码」，**没有一句话禁止修改治理层文件** |
| `harness/policies.yml` | 全部 enforce 声明 | `enforce: block` 只检查文件体积/数量，不检查**谁修改了治理文件** |
| `internal/memory/memory.go` | memory 包 | 跨会话知识存储可以记录「怎样绕过 gate」的技巧，下行 agent 读取后模仿 |

**具体威胁场景**：

1. **静默放松**: 某次 evolve 迭代中，implementer agent 在实现功能时发现 `gate.mjs` 的 block 模式阻碍了代码提交，顺手把 `enforce: block` 改成 `enforce: warn`。后续所有迭代的架构检查全部降级为 advisory——系统永不知情。
2. **完全禁用**: agent 修改 `harness/acceptance.mjs`，把 `decide` 函数改为永远返回 `ACCEPTED`。`forge accept` 从此永远绿灯。
3. **级联退化**: agent 修改 `.agent/AGENTS.md`，移除「fresh-context reviewer 必须是独立 agent」的红线——后续所有 reviewer 直接继承 implementer 的上下文，评审独立性荡然无存。
4. **溯源清除**: agent 在修改治理文件后，同时修改 `memory.jsonl` 或 `trace.jsonl` 中对自己不利的记录（当前 trace 无防篡改签名）。

### 为什么高价值

ForgeOS 区别于普通编码助手的核心卖点是 **治理**——它不是帮人写代码，是用不可违反的规则确保 AI 不写出上帝文件。但如果 AI 可以修改治理本身，这个卖点就变成了空中楼阁。

**这不是未来问题，是当前基线下的真实风险**：Sprint 24-26 的真实 claude 运行已验证 agent 在 `acceptEdits` 模式下可以写任意路径的文件。在 multi-agent 长周期运行中，某次"偷懒"的修改就能让整个治理体系失效。

### 建议方向（概念框架，非代码）

1. **治理文件不可变声明** —— 在 `project.yml` 或新的 `governance.yml` 中声明一组**受保护路径**（如 `.agent/`、`harness/`、`.arch/`）。`forge-core` 在 `Write` 操作前做路径检查，命中受保护路径则拒绝（类比 git 的 `--reject-text` hooks）。
2. **完整性度量** —— 在每次 evolve 迭代起跑前，计算治理文件的 checksum（SHA-256），迭代结束后重新计算。如果变化与当前 phase 的预期产出（emits 声明）不匹配，则立即终止并报警。
3. **双人规则（Two-Person Rule）** —— 修改治理文件需要两个独立的 agent 一致同意（类比核发射的双钥匙机制），或转为需要 human approval。
4. **审计痕迹不可篡改** —— trace.jsonl 增加 `prev_hash` 链式哈希字段，每个事件包含前一个事件的 SHA-256，使事后篡改可检测（类似区块链式审计日志）。

### 诚实边界

- 这不是「禁止修改治理」，而是 **受控修改**。合理场景（如 Sprint 升级治理规则）应通过显式通道（如 `forge migrate --upgrade-governance`）进行，而非由 implementer 在功能实现中顺带修改。
- 保护不能依靠 agent 自律——防御必须在 harness 和 forge-core 运行时层。

---

## 方向二 · 演化分支与反事实回滚（Evolution Branching & Counterfactual Rollback）

> **优先级**: 🟠 **P1** | **类型**: 演化能力 · 可靠性 | **关键词验证**: `counterfactual` `rollback.*plan`  
> `branch.*experiment` `what.if` `fork.*merge` → **全部零命中**

### 问题

当前 `forge evolve` 的 LoopEngine 是**严格顺序向前**的：

```
iter 1 → iter 2 → iter 3 → ... → converge (or max-iter)
```

没有任何「回退到 iteration 2 的状态，换一套 prompt 重跑」的能力。checkpoint 系统支持 resume（继续），但**不支持分支/实验**。当 evolve 做出错误决策时，用户只能：

1. 接受错误，寄希望于后续迭代纠正（可能越修越糟）
2. 手动清理 `.forge/` 目录，从零开始重跑（丢弃所有学到的东西）
3. 不要在 evolve 中试错（这在本质上限制了 evolve 的探索意愿）

**代码级证据**：

| 文件 | 行 | 证据 |
|---|---|---|
| `internal/persist/checkpoint.go` | `Save/Load` | checkpoint 系统只有一个活跃文件 + N 个历史备份，但**没有分支标签、没有实验 ID、没有"在迭代 3 的状态上 fork"的概念** |
| `internal/orchestrator/loop.go` | `LoopEngine.Run` | 循环体是 for-select，每次迭代 `RunFrom` → `Signals()` → converge 判定，**没有分叉点** |
| `internal/converge/converge.go` | `Converge / Evaluate` | converge 只做 MET / NOT MET 二元判定，不提供 "this branch diverged from baseline at criterion X" 的诊断 |
| `cmd/forge/evolve.go` | `cmdEvolve` | `--resume` 只从最近的 checkpoint 继续，不支持 `--resume-from iter=2` 或 `--branch experiment-a` |
| `internal/memory/memory.go` | `Entry` | memory 有 `Supersedes` 字段做知识层面的撤销，但**没有全局的进化版本标签**——无法区分"这个 decision 是在 branch-a 下做出的"还是"来自主干" |

**具体场景**：

1. **设计分支评估**: 对同一个需求，让两个 implementer 分别在迭代 2 的 checkpoint 上 fork，一个用 Sonnet 实现，一个用 Haiku 实现，然后在迭代 4 比较两个分支的 gate 结果和代码质量——选择更好的分支继续演进。
2. **灾难恢复**: 迭代 5 意外删除了一个模块。用户执行 `forge rollback --to-iter 3`，系统恢复 checkpoint + 对应的 git 状态（或至少恢复文件快照），从迭代 4 重跑。
3. **A/B prompt 测试**: 怀疑当前 reviewer 太宽松。fork 出一个分支，换更严格的 reviewer prompt（或更贵的模型），跑两轮后比较两个分支的 bug 检出率。
4. **收敛路径分析**: converge MET 后，用户 `forge evolve --analyze` 看到 "从迭代 3→4 的 Roadmap 跳升 40% 是最大的收敛贡献点，迭代 1→2 几乎没有进展"——指导未来 prompt 优化。

### 为什么高价值

自治系统的本质矛盾是：**它必须探索才能学习，但探索必然引入错误方向**。没有分支/回滚能力，系统会倾向于保守（避免代价高昂的错误）从而降低探索意愿；或者大胆探索但无法从错误中恢复。两者都削弱了「24h 无人值守」的可信度。

从产品视角：分支能力是 ForgeOS 从「脚本化流水线」进化为「真正的 AI 研发工厂」的分水岭——工厂可以在生产线上做实验而不影响主线产品。

### 建议方向

1. **Checkpoint 标签化**: 给 checkpoint 加 `label` 和 `parent_iter` 字段，形成 DAG 而非单链。`forge evolve --tag experiment-a` 在每次 checkpoint 写入时带上标签。
2. **`forge branch <name> --from-iter N`**: 读取迭代 N 的 checkpoint（包括其 memory 和 trace 快照），创建新的演化分支。两个分支独立演进。
3. **`forge merge <branch>`**: 将一个分支的最终 memory entries 和 convergence 结果合并到主线。合并策略类似 git merge——保留双方的学习记录，冲突的 decision 以主线为准。
4. **`forge diff --branch <name>`**: 展示两个分支在相同迭代数下的 convergence 信号差异（谁 gate 更绿、谁 roadmap 更多）。
5. **轻量级快照**: 每个 checkpoint 不存储整个文件系统，只需存储：（a）当前 iteration/roadmap/gates/mode；（b）memory 的快照指针（增量而非全量）；（c）git commit SHA（假设用户有版本控制，回滚操作 = checkout + checkpoint restore）。

### 诚实边界

- 分支系统不解决「agent 在分支间切换导致的 token 浪费」——fork 意味着两套 agent 并行花钱。产品文档需诚实标注成本模型。
- 分支合并本质上是启发式的——两个演化路径的 memory 可能矛盾。需要人确认（或一个专门的 `merge-reviewer` agent）。

---

## 方向三 · 人机模糊消除层（Human-in-the-loop Ambiguity Resolution）

> **优先级**: 🟠 **P1** | **类型**: 用户体验 · 发现深度 | **关键词验证**: `human.*loop` `dialogue`  
> `question.*queue` `clarify.*question` `ambiguity.*reduc` `ask.*question` → **全部零命中**

### 问题

ForgeOS 的 Discover 阶段做需求探索，`product-manager` agent 输出 PRD + `confidence: <0-100>`。如果置信度不足，系统**只是停止**，等待人类批准。但当前机制是 **binary gate**（approved / not approved），没有任何结构化的**双向对话**机制来减少歧义。

当前状态：

```
用户: "建一个短链接服务"
  → forge run discover
    → requirement-discovery: PRD 生成, CONFIDENCE: 65
    → convergence: NOT MET (requirement_confidence < 80)
    → 输出: "awaiting human approval (non-bypassable)"
  → 用户: ??? (没有任何引导式问题帮助提高置信度)
```

**代码级证据**：

| 文件 | 行 | 证据 |
|---|---|---|
| `internal/converge/converge.go` | `evalRequirementConfidence` | 置信度不达标就 unmet，但**不产生任何"还缺什么信息"的诊断** |
| `internal/prompt/retrieve.go` | `Retrieve` | ADR 检索是 TF-IDF 打分，纯信息检索，**不产生"还需探索什么"的全局缺口分析** |
| `cmd/forge/engine_build.go` | `buildPrompt` → agent prompt | agent prompt 是单方向指令（"implement this"），**不包含反问指令**（"如果需求不明，列出三个最需要澄清的问题"） |
| `cmd/forge/gates.go` | `reportConvergence` + `reportHumanGate` | human_gate 报告只输出 "awaiting human approval"，**没有任何对 "为什么 approval 还不足够" 的解释** |
| `.agent/workflows/discover.yml` | 整个 workflow | discover.yml 没有专门的需求澄清/反问相位——只有 requirement-discovery → market-research → product-design，全部 agent 单方向输出 |
| `internal/doctor/doctor.go` | `Check` | doctor 检查的都是运维健康，**没有"需求完整性"检查** |

**具体场景**：

1. **需求模糊**: 用户说 "建一个用户系统"。系统问："请澄清：①登录方式（邮箱/SSO/手机）②用户角色（普通/管理员/多层）③是否需要社交登录？"
2. **架构歧义**: Architect 在 Design 阶段不确定用单体还是微服务。系统输出 "权衡分析：单体（成本低/适合＜5人团队）vs 微服务（扩展性好/运维成本高）。当前你的项目是 3 人团队/MVP 阶段，倾向单体？"
3. **约束冲突**: AGENTS.md 要求零外部依赖，但需求要求 Stripe 支付集成。系统暂停并提问："需求要求 Stripe（外部依赖）与 AGENTS.md 的零依赖约束冲突。请选择：①豁免此约束 ②改用 Stripe API 的纯 curl 封装 ③重新设计需求避免第三方支付"
4. **信息增量迭代**: 用户回答了问题后，系统不重新从零跑 discover，而是**增量更新**置信度——用户回答一个问题，置信度 +10，直到 ≥80 才继续。

### 为什么高价值

当前 ForgeOS 在 Discover/Design 阶段的「卡住即停止」模式有两个致命缺陷：

1. **用户不知道该做什么** —— 系统只说 "我卡住了" 而不说 "我还缺什么"，用户只能猜测。这违背了产品设计基本法则（系统应该指导用户完成流程）。
2. **重复运行成本高** —— 用户改一下输入重新跑 discover，系统从零开始（多轮 LLM 调用浪费）。如果系统能记住上一轮的结果增量更新，成本可降低 50-70%。

从产品视角：**主动提问** vs **被动卡住** 是 "智能助手" 与 "自动化脚本" 的核心区别。用户愿意信任一个会追问的系统，远胜于一个默默停止的系统。

### 建议方向

1. **Agent 卡增加反问指令**: 每个 agent 卡增加 `clarifying_questions` 段（最多 3 个问题），在发现信息不足时，agent 不是退化为空白输出，而是在输出最后附上 "To improve confidence to >= 80%, I need: Q1/Q2/Q3"。
2. **双向信号通道**: `converge.Signals` 增加 `OpenQuestions []string` 字段。当 human_gate 因置信度不足而卡住时，同时输出待解答问题清单。
3. **`forge answer` 子命令**: 用户运行 `forge answer discover "用户支持邮箱登录"`，系统将回答注入 memory，重新评估 discover 置信度。**不从零开始，增量更新**。
4. **问题优先级排序**: 如果同时有 10 个模糊点，系统按对收敛影响最大的排序（类似 "哪个问题回答后置信度提升最大"），用户可先回答高杠杆问题。
5. **对话持久化**: memory 包增加 `KindQuestion` / `KindAnswer` 条目，跨 run 保留问答历史，让同一项目的多次 discover 不再重复问相同问题。

### 诚实边界

- 问题生成的质量取决于 agent 卡的反问指令设计——差的指令产生无关问题。开始时可以人工编写，后续由系统根据 "哪些问题最能提升置信度" 自动优化。
- 增量评估不是凭空——回答一个问题后，系统需要重新运行部分评估 pipeline（至少是置信度重构），不是 O(1) 的简单加分计算。

---

## 方向四 · 跨项目学习与模式迁移（Cross-Project Learning & Pattern Transfer）

> **优先级**: 🟢 **P2** | **类型**: 知识管理 · 平台能力 | **关键词验证**: `cross.project` `transfer.*learn`  
> `federat` `shared.*pattern` `pattern.*library` `knowledge.*transfer` → **全部零命中**

### 问题

ForgeOS 的 memory 系统（`internal/memory`）是 **per-project** 的——`.forge/memory.jsonl` 存储在一个仓库的 `.forge/` 目录下。每个项目的 evolve 循环从零开始学习：

```
项目 A 的 evolve:
  迭代 1: 发现 "Go 项目应使用 internal/ 分包" ✅
  迭代 2: 记住这个教训，Builder 模式利用 internal/ 包隔离
  迭代 3: 收敛

项目 B 的 evolve:
  迭代 1: 发现 "Go 项目应使用 internal/ 分包" ← 又学一遍！🔄
  迭代 2: ...
```

**代码级证据**：

| 文件 | 行 | 证据 |
|---|---|---|
| `internal/memory/memory.go` | `Append/Load` | `path` 参数是 `.forge/memory.jsonl`——**硬编码到项目根下**，没有 `--memory-path` 或共享存储概念 |
| `internal/memory/memory.go:45-50` | `loadCache` | 缓存 key 是 `path`（项目路径）+ `mtime`，**跨项目共享的唯一方式是 NFS 硬链接** |
| `internal/prompt/retrieve.go` | `Retrieve` | 检索范围限于 `docs/adr/` 和 `.agent/`——**当前项目自己的知识资产** |
| `cmd/forge/prompt_context.go` | `Gather` / `buildPrompt` | 注入 memory 时只读当前项目的 memory，**没有从"全局模式库"拉取预先验证过的模式** |
| `internal/memory/memory_compact.go` | `Compact` / `Prune` | compaction 只按项目内的时间/种类缩减，**没有"将已验证模式发布到共享目录"的功能** |

**具体场景**：

1. **模式复用**: 项目 A 已经验证过 "使用 envconfig 管理配置" 是一个好模式（reviewer APPROVED、gate 全绿）。项目 B 开始开发时，自动收到推荐："项目 A 已验证：配置管理 → envconfig。是否采纳？"
2. **反模式共享**: 项目 A 踩坑 "不要用 global 变量做依赖注入"（memory 记录了 KindLesson）。新项目 C 的 implementer 收到避免此模式的提示——即使 C 项目从未遇到过此问题。
3. **闸门阈值校准**: 项目 A（Go，engineering 模式）经过 10 轮 evolve 发现 `max_function_lines: 50` 太严格，适合设为 80。项目 D（Go，同模式）自动获得校准后的阈值推荐，而不是用出厂默认值。
4. **路由模式学习**: Router 在项目 A 中发现 "reviewer 用 Sonnet 在 90% 的情况下通过、Opus 仍可检出 Opus-only 的 bug"。此模式跨项目传播后，所有项目的 reviewer 路由策略自动优化。

### 为什么高价值

ForgeOS 把自己定位为「软件工厂」而非「单项目工具」。工厂的核心特征是 **产线复用** 和 **持续改进**——第一条产线累计的经验自动惠及后续产线。

在当前架构下，ForgeOS 有两个致命的效率问题：

1. **每项目冷启动** —— 每个新项目都要从零经历 "发现 → 记住 → 优化" 的完整 cycle。模式越多，浪费越大。
2. **组织知识孤岛** —— 项目 A 的 tester 发现某模式反脆弱、项目 B 的 developer 又踩进去。系统对此完全无感知。

从商业价值看：跨项目学习是 ForgeOS 从 "个人效率工具" 升级为 "组织基础设施" 的关键能力。CI/CD 如果每个项目从零配置 pipeline 就是反人类的——同样，AI 软件工厂如果每个项目从零学习也是反人类的。

### 建议方向

1. **全局 memory 目录**: `$FORGE_HOME/memory/` 或 `~/.forge/memory/` 下的共享知识库。`internal/memory` 增加 `LoadGlobal`/`AppendGlobal` API，先查全局再查项目级。
2. **模式发布/订阅机制**:
   - `forge publish-pattern <kind> <topic>` —— 将已验证的 knowledge entry 从项目 memory 发布到全局库
   - `forge subscribe <topic>` —— 在项目中激活某个主题的模式推荐
   - 发布时自动附加元数据：源项目、验证方式（gate results count）、置信度
3. **跨项目收敛度度量**: `forge patterns --global` 输出全局库中的模式数量、使用频率、各项目的采纳率——让组织看到 "哪个团队贡献了最多的可复用模式"。
4. **自动漂移检测**: 全局库中的模式在新项目中如果被 agent 否决（request_changes 或 redesign），自动记录为 "此模式在项目 X 中不适用"，并降低其全局置信度。
5. **路由决策共享**: `internal/routing/scorecard.go` 的 `HistoryTiebreak` 从仅读项目级 scorecard 扩展到可读全局 scorecard（最优模型选择不再是各项目独立决策）。

### 诚实边界

- 跨项目学习的有效性受限于项目的同构性——一个嵌入式 C 项目和一个 Go 微服务项目共享的有用模式很少。需要按语言/领域/模式维度做有效索引。
- 全局 memory 的污染风险：一个项目的错误模式可能扩散到所有项目。需要发布审核（至少是项目内的 fresh-reviewer 确认）。
- 隐私：默认不自动发布——所有共享需显式操作。

---

## 方向五 · Agent 推理可观测性（Agent Reasoning Observability）

> **优先级**: 🟢 **P2** | **类型**: 可观测性 · 调试能力 | **关键词验证**: `agent.*observ` `reasoning.*trace`  
> `thought.*process` `chain.*thought.*captur` `decision.*log.*why` → **全部零命中**

### 问题

当前 ForgeOS 的 trace 系统（`internal/trace`）记录的是 **WHAT happened**——哪个 phase 跑了、耗时多少、gate 什么状态、收敛与否。它不记录 **WHY it happened**——agent 基于什么推理做出了特定决策。

当自治运行产生错误结果时：

```
forge status --trace
  [1] kind=agent name=implementer status=OK duration_ms=28400
  [2] kind=gate name=test status=FAIL detail="TestHTTP_GetURL: expected 200, got 500"

→ 开发者知道测试挂了。
→ 但不知道 agent 为什么写了出 bug 的代码——是需求理解错误？
   架构选型失误？prompt 中漏掉了关键约束？
→ 无法 debug，只能相信"下一轮能修好"。
```

**代码级证据**：

| 文件 | 行 | 证据 |
|---|---|---|
| `internal/trace/trace.go` | `Event` struct | Event 只有 `Kind/Name/Status/DurationMs/CostUsdMicros/Model/Detail`，**没有 `Reasoning`、`DecisionPath`、`Alternatives` 等推理字段** |
| `internal/trace/trace.go:126-135` | `Span()` | span 辅助函数只开闭计时器，**不捕获 agent 输出中的推理文本** |
| `cmd/forge/cost.go` | `parseReviewerVerdict` / `parseExecutiveVerdict` | 解析 agent 输出只提取末行的机读 token（APPROVE/REJECT），**丢弃了 agent 输出中所有的推理过程** |
| `cmd/forge/prompt_context.go` | `buildPrompt` | prompt 的 `GatherResult` 只保存 phase 输出（用于 feed forward），**没有从中提炼结构化推理摘要** |
| `internal/prompt/prompt.go` | `Gather` | 三 lane 注入（task/ADR/constraints），**但 agent 的真实输出 → 推理 → 决策的映射没有被写入任何结构化字段** |
| `internal/prompt/retrieve.go` | `Retrieve` | ADR 检索是 stats-based TF-IDF（词频打分），**不记录 agent 为什么选择了 ADR A 而非 ADR B** |
| `internal/memory/memory.go` | `Entry` | memory 有 `KindDecision`，但 Detail 字段是自由文本——**没有结构化的 "前提→推论→结论" 三段论字段** |

**具体场景**：

1. **事后调试**: evolve 结束后结果不满意，开发者运行 `forge explain implementer --phase implementer`，看到 agent 的推理链："需求文档说需要缓存 → 我选择了 Redis（因为项目已引入 Redis 依赖）→ 但未注意到缓存 TTL 需求 → 导致测试 500 → 下一迭代应设 TTL 并补测试"。
2. **仲裁冲突预警**: 当两个 agent（implementer 和 reviewer）的推理链对同一问题的判断相反时，系统自动标记 "观点分歧"——implementer 认为用 Redis 是正确的，reviewer 认为用内存缓存更好。分歧点（数据持久性 vs 简单性）被显式记录。
3. **设计决策追溯**: 3 个月后新开发者问 "为什么 `url-shortener` 用了内存存储而非数据库？"——`forge explain --decision shortcode-storage` 展示当时的 reasoning chain，包括当时做出的 trade-off 和上下文约束。
4. **Agent 行为审计**: 安全审计员想确认 reviewer 是否真正独立评审了代码——`forge audit reviewer --iter 3` 展示 reviewer 的完整推理链，确认它看到了代码、理解了变更、并给出了实质性评估（而非只是输出 "APPROVE"）。
5. **失败模式聚类**: 系统在多个 evolve 运行中发现 "implementer 在 60% 的失败案例中遗漏了错误处理"——不是因为 Agent 能力问题，而是 prompt 中错误处理约束排在第五、容易被截断。这一发现来自对多条推理链的结构化分析。

### 为什么高价值

自治系统最令人不安的不是它犯错——人也会犯错——而是 **犯错时没有解释**。人类工程师可以在 code review 中解释 "我当时为什么这么写"；AI 自治系统如果只能提供代码 + gate 结果，开发者将永远处于"相信它"或"重写它"的二元选择中。

从产品视角：

1. **信任构建**: 能展示推理过程的系统比黑箱系统获得更多人类信任——即使二者准确率相同。
2. **调试效率**: 当前如果 gate 失败，开发者只能重读代码猜测 agent 意图。展示推理链将调试时间从小时级降到分钟级。
3. **学习回路闭环**: 推理链 + 最终结果 = 完美的训练数据——系统可以从 "我这里推理错了，导致 gate 失败" 中学习，不需要（不擅长自我诊断的）LLM 自我反思。
4. **责任链**: 在合规场景（金融/医疗），需要记录每一个自动化决策的完整推理过程。当前 trace 的粒度远不足以满足合规审计要求。

### 建议方向

1. **结构化推理捕获**: `internal/trace` 增加 `ReasoningEvent`（或扩展 Event 增加 `Reasoning` 字段），agent 每产生一个重要决策点（架构选择、实现取舍、风险判断）时记录：
   ```go
   type Reasoning struct {
       Decision   string   // "choose_redis_over_memory"
       Premises   []string // ["req: cache TTL > 1h", "redis_dep_available"]
       Conclusion string   // "redis is justified for persistent caching with TTL"
       Confidence float64  // 0-1, agent's self-assessed confidence in this decision
   }
   ```
2. **Agent 卡增加推理模板**: 每个 agent 卡增加 `reasoning_fields` 段，声明此 agent 应在其输出的哪个位置嵌入结构化的推理块（类似已有的 `VERDICT: <token>` 契约）。`cost.go` 的解析器家族扩展为通用 reasoning 提取器。
3. **`forge explain` 命令**: 读取 trace 中的推理链，渲染为人类可读的推理报告。支持 `--path`（特定文件）、`--phase`（特定 phase）、`--decision`（关键决策）过滤。
4. **推理差异对比**: `forge diff --branch` 增强为同时对比两个分支的推理链——为什么 branch A 选择了 MySQL 而 branch B 选择了 SQLite？逐前提对比展示出来。
5. **memory 集成**: 推理链的高置信度决策自动泵入 memory（`KindDecision`），使后续 agent 能引用 "之前已经决定过的事" 并给出交叉引用。

### 诚实边界

- 推理链的完整性和诚实性取决于 agent 本身——一个偷懒的 agent 可以输出 "推理链：我做了正确选择" 而省略真实推理。推理捕获是信任但验证的（trust-but-verify），gate 结果和代码最终仍是客观验证层。
- 结构化推理增加 token 消耗（agent 需要花 tokens 生成 reasoning block）。需要平衡——建议默认仅对 `reviewer`/`architect`/`cto` 等高杠杆角色启用完整推理，implementer 仅在 gate FAIL 或 `--debug` 模式下启用。

---

## 汇总优先级矩阵

| # | 方向 | 优先级 | 影响面 | 实施粒度 | 前置依赖 | 去重验证 |
|---|------|--------|--------|----------|----------|----------|
| 1 | 治理自保 | 🔴 P1 | 安全基线 | 中型（新增检查层 + CLI 子命令） | 无 | ✅ 零命中 |
| 2 | 演化分支与回滚 | 🟠 P1 | 核心能力 | 大型（checkpoint 模型重设计） | 需要 `persist` 格式版本化 | ✅ 零命中 |
| 3 | 人机模糊消除 | 🟠 P1 | 用户体验 | 中型（新增对话信号 + CLI） | 需要 existing confidence 链路 | ✅ 零命中 |
| 4 | 跨项目学习 | 🟢 P2 | 平台能力 | 大型（全局存储 + 发布系统） | 需要 memory 格式扩展 | ✅ 零命中 |
| 5 | 推理可观测性 | 🟢 P2 | 可观测性 | 中型（trace 格式扩展 + CLI） | 需要 trace 格式版本化 | ✅ 零命中 |

---

## 与已有饱和域的关系

| 本文件方向 | 最接近的已有覆盖域 | 与已有域的关系 |
|---|---|---|
| 治理自保 | 安全纵深（~14 篇） | **不重叠**：已有 14 篇覆盖 secret-scan/SCA/risk 分类/prompt 注入——都是**防止外部攻击**。治理自保是**防止内部（agent）无意识颠覆治理本身**，两者正交 |
| 演化分支与回滚 | 编排状态机（~35 篇） | **不重叠**：已有 35 篇覆盖串/并行/loop-back/resume/gating——全是**单前进方向**的状态机模式。分支/回滚引入了 **DAG 而非线性链**，是不同复杂度的概念 |
| 人机模糊消除 | CLI 体验（~8 篇） | **不重叠**：已有 8 篇覆盖 detect/preflight/doctor/status——全是**运维体验**。模糊消除是**需求阶段的对话交互**，两者正交 |
| 跨项目学习 | 学习闭环（~16 篇） | **不重叠**：已有 16 篇覆盖 trace/scorecard/memory/converge——全是**单项目内**的闭环。跨项目学习是**多项目间**的知识流动，需要新的全局存储和发布机制 |
| 推理可观测性 | 学习闭环（~16 篇） | **不重叠**：已有 trace/telemetry 覆盖的是**结果指标**（耗时、成本、通过率）。推理可观测性是**过程指标**（agent 为什么做了这个决策） |
