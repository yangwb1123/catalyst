# ForgeOS — 高价值扩展方向分析

> **扫描范围**:全局代码库 (`forge-core/` 18 Go 包 · `harness/` 40+ 执法/工具文件 · `.agent/` 12 角色卡/9 skill/5 workflow · `docs/` 全量产物)
> **方法**:逐包读代码 + 通读 CURRENT_SPRINT 31 个 sprint 记录 + FUNCTIONAL_REQUIREMENTS_AUDIT + 对照 PROJECT/ARCHITECTURE/ROADMAP 声明
> **立场**:资深架构师/产品经理,关注「项目下一个结构性增值点」,非「已有功能的再叙述」
> **要求**:不写代码,只分析和论证

---

## 前言:当前定位

ForgeOS 已经走完 **31 个 sprint**,核心循环(Discover→Design→REVIEW→Build→Evolve)和 5 引擎(Orchestrator / Router / Context / Memory / Evaluation)均已落地并全绿。真点火(`--agent-cmd=claude`)多 agent 自治协调已坐实(增量+版本级 converge MET),学习循环三维真数据(quality+latency+cost)落盘。治理闸门 8 检查全部机器执法。

这不是「从零开始的早期项目」,而是 **核心能力已夯实、下一阶段应当往「规模/协作/智能化」演进** 的系统。本文提出 5 个方向,每个方向都满足:
1. 在当前代码库上有**明确的落地路径**(非纯学术空想)
2. 与已有 60+ 分析文档**非同质化**(非已被反复覆盖的领域)
3. 属于**「做了就打开新能力象限」** 的结构性增长,而非边角修补

---

## 方向一 · Multi-Agent Shared Workspace:显式共享工作区与冲突协调层

### 为什么需要

当前 `forge run/evolve` 的多 agent 协作模型是**管道式顺序传递**:phase→phase→phase,输出经 `emits:`/`feeds_forward` 以文件或字符串传给下游。并行编排(`--parallel` + `depends_on`)虽已就绪但无 workflow 启用——**关键障碍不在于编排能力,而在于没有共享工作区模型**:

- **冲突盲区**:两个并行的 implementer 可能修改同一文件,系统毫无感知或协调机制
- **信息孤岛**:一个 agent 发现的上下文(如「这个模块已有测试覆盖率 80%」)无法自然地让另一个 agent 感知,只能通过刻意写文件+下游读取
- **重复劳动**:多个 agent 各自独立检索同一上下文(ROADMAP/架构),虽然有 Context Cache 缓解,但缓存的只是检索结果,不是 agent 自己的推理产物
- **无结构化协作协议**:当前 agent 间的「通信」只有 `feeds_forward`(单向、线性的文本注入),没有双向讨论、协商或联合推理

### 落地形态

一个新的 **`internal/workspace/` 包**,提供:

1. **结构化共享工作区(Shared Artifact Graph)**:不依赖纯文件系统,而是在 `.forge/workspace/` 下维护一个 agent 可读写的结构化 KV 存储,每个 artifact 有类型/所有者/版本/依赖关系。实现路径:复用已有的 `internal/persist`(checkpoint) + `internal/trace`(事件序列)原语,在其上建主题层。

2. **冲突检测与合并(Conflict Detector)**:对并行 agent 的修改,在写入工作树前做 three-way diff(基于 `git merge-file` 或 Go 的 `diff` 包),检测重叠修改。检测到冲突时:有 `--parallel` 的 workflow 自动进入 merge 阶段(调一个新 agent 来解决冲突),无 `--parallel` 的串行 path 零影响。**关键设计原则**:这层必须是可选插拔的——默认串行 path 完全不走它,确保向后兼容。

3. **Agent Discovery 通道(轻量 MCP 风格)**:agent 可以在 workspace 里 publish `Finding`/`Question`/`Constraint` 三种结构化消息,下游 agent 在 prompt 注入时除了 `feeds_forward` 的强制上下文,还多一个「workspace 中与我相关的最新发现」通道。**诚实边界**:不是实时双向通信,是「写-消费」模型,每 phase 起跑时拉一次。

### 为什么是 P1 而非 P3

- 并行编排已就绪但无人敢用——缺的就是这层
- 真点火多 agent 已坐实,下一步必定撞上「agent 不知道队友在做什么」的问题
- 基础设施( trace/persist/git 集成)已全部就绪,只差 `workspace` 这层抽象
- **不破坏任何现有路径**:串行 workflow 完全不走它,零回归风险

---

## 方向二 · 结构化的观察性管道:从「写 JSONL」到「可查询/可比较/可回溯」

### 为什么需要

当前 telemetry 架构是**写-侧重**:
- `trace.jsonl`:有 phase 级 latency/cost/verdict/tier 的丰富事件流
- `scorecards.json`:有按 (model,task_type) 聚合的质量/延迟/成本统计
- **但没有任何结构化查询接口**:跨 run 比较需要手写 grep/jq;`forge status` 只看当前目录;无法回答「上周所有 production lifecycle 项目的平均 gate 通过率」「哪个模型在哪个 phase 上表现最差」「最近 20 次 evolve 迭代的收敛速度趋势」

这导致:
- 学习循环的数据面虽在工作,但**决策面仍靠直觉**:改进路由策略(如给某个 phase 换模型)时,没有跨 run 数据支撑
- **Human-in-the-loop 是盲人摸象**:审批者看 `forge status` 得到一个 snapshot,看不到项目演变趋势
- **问题排查靠 SSH + grep**:在远程 CI 或无人值守场景下,问题排查需要登录机器 grep JSONL

### 落地形态

1. **`forge trace` 子命令族**(在已有的 `internal/trace` 包上建查询层):
   - `forge trace list` — 列出所有 trace session(按时间/项目/workflow/mode 筛选)
   - `forge trace show <id>` — 单次 run 的 phase 级时间线、cost breakdown、裁决路径
   - `forge trace compare <id1> <id2>` — 两边并联对比(phase 耗时/成本/模型/裁决)
   - **存储层复用**现有的 `trace.go` Event 类型,仅增索引(session→phase 映射,可 B-tree 或 SQLite)

2. **`forge scorecard query`**(在已有的 `internal/routing/scorecard.go` 上建):
   - 不需 jq 就能问「哪个模型在 reviewer phase 上 APPROVE 率最高」「haiku 在 implementer 上的平均成本」
   - 输入:filter(phase/task_type/model/time window),输出:表或 JSON
   - **存量数据即用**:现有 `scorecards.json` 已是最佳 format,只是没有查询 API

3. **`forge run --notify` webhook**(可选):run 结束时 POST JSON 摘要到外部(如 Slack webhook / CI summary),让无人值守场景有可见性出口。**诚实标注**:非核心能力,是纯消费端扩展;核心是让 forge trace 自己就是够好的查询端。

### 为什么是 P1

- **数据已在,缺的是查询**:不产生新数据,只是给既有数据装一个 API 面——纯收益,低成本
- **直接解锁「远程/CI 场景」的可观测性**:当前无人值守靠的是 `echo logs`,有了 `forge trace` 才能真正「看一眼就知道 run 的状态」
- **为未来的自动路由优化铺路**:多维路由(G3)如果不需要人工分析 scorecard 数据,就需要程序化查询接口——`forge trace query` 就是这层

---

## 方向三 · Workspace Sandbox / Atomic Rollback:agents 操作工作树的安全护栏

### 为什么需要

当前 agent 直接在项目工作树上操作:
- implementer 的 `acceptEdits` 直接写文件——如果 agent 写了错误代码或破坏了测试,唯一的恢复方式是 `git checkout`(人手工介入)
- 没有「run 级原子性」:一次 `forge evolve` run 中途失败后,可能部分 phase 的文件已落地、部分未落地,工作树处于不一致状态
- `forge resume` 从 checkpoint 续跑,但 checkpoint 只记录 phase 进度,不记录工作树的快照
- 现有的四维安全护栏(recursion/budget/timeout/output-cap)防的是**资源失控**,不防**数据污染**

这在工作树小、跑得少时不明显。但真点火 24h 多迭代场景下,**一次 agent 的错误写操作就能污染整个项目的代码+测试+文档**,且可能直到数小时后才被发现(在下一次 gate 或 review 阶段)。

### 落地形态

利用已有的 `git` 基础设施,在现有 `internal/persist` 旁加一个 `internal/snapshot` 层:

1. **Pre-Run Snapshot**:在 `forge run/evolve` 起跑前自动 `git stash push --include-untracked -m "forge-snapshot-<run_id>"`,记录工作树的干净状态。**设计约束**:只对 git-tracked 项目生效(ForgeOS 自己的 `policies.yml` 已假设 git 存在,这不增加新依赖);非 git 项目跳过(诚实 N/A)。

2. **Auto-Cleanup Policy**:snapshot 在 run 成功(converge MET + gates green)后自动 drop;run 失败后保留供检查,可用 `forge rollback <run_id>` 恢复。**关键安全闸门**:rollback 标记必须由 CLI 确认(`--apply`),避免自动回滚覆盖用户手动修改。

3. **`forge rollback <run_id|--last>`**:列出现有 snapshot,选择回滚。回滚后 checkpoint 也对应失效(因果一致性:你回滚了文件,不能 resume 到一个需要修改后文件的 checkpoint)。

4. **与 checkpoint 的关系**:snapshot 和 checkpoint 是正交的——checkpoint 管「which phase to resume」,snapshot 管「what the working tree looked like」。两者的交集(`resume+rollback`)需要程序化检查:如果回滚后的工作树不符合 checkpoint 的 phase 进度,forge 应明确告知并拒绝 resume。

### 为什么是 P1

- **对「无人值守 24h 跑」是核心保障**:目前安全护栏管资源不管数据,这里补上「数据一致性」
- **纯利用 git 现有基础设施**:不发明存储引擎,go-git 或纯 shell 调用即可实现
- **不改变任何 phase/agent 的行为代码**:snapshot/rollback 是完全在外的编排层,agent 不知道自己被隔离
- **增量成本极低,安全收益极高**:几行 git 命令,就能让「跑崩了」从「工作树污染了」变成「一键恢复」

---

## 方向四 · 多项目 / 舰队治理:从单项目自治到组织级编排

### 为什么需要

ForgeOS 当前的核心场景是**单项目自治**:
- `project.yml` 定义本项目 mode/lifecycle
- `modes.yml` 定义全局策略
- `forge-init` 复制治理资产到新项目

但项目级的愿景是「软件工厂」——一个组织同时运行多个项目,需要:

- **跨项目策略一致性**:组织级要求所有 production lifecycle 项目必须有 security review,如何确保?当前只能靠「每个项目自己遵守」。
- **舰队级闸门视图**:CTO 想看到所有项目的 `forge accept` 状态、战略缺口、版本腐化程度——当前只能在每个项目单独跑
- **策略继承与覆盖**:组织级策略(base)→ 部门级(team) → 项目级(project),三层继承,下层可收紧不可放松。参考 ADR 0003(agent-os submodule)的机制,但 ADR 0003 只解决了资产共享,没有解决**分层策略计算**。
- **Bulk 操作**:对一个策略更新(如「所有项目 coverage 阈值从 60 提到 70」),需要在 N 个项目上生效——当前每项目手动改。

### 落地形态

1. **`forge fleet` 子命令**(纯新增,不修改现有子系统):
   - `forge fleet status` — 扫描目录下所有包含 `.agent/project.yml` 的项目,聚合报告每个项目的 mode/lifecycle/`forge accept` 状态
   - `forge fleet gate` — 对所有项目跑完整 Stop 闸门,聚合 ACCEPTED/REJECTED 统计
   - `forge fleet apply <policy-file>` — 对匹配 selector 范围的一组项目,更新其 `project.yml`/`modes.yml` 的指定字段

2. **分层策略引擎**(新 `internal/policy` 包或扩展现有 `internal/mode`):
   - 继承计算:project.yml 的 `extends: [org-base, team-internal]` → 解析器合并多级策略,取最严值
   - 引用声明方式:引用已存在的 YAML 文件(如 `extends: [.agent/policies/org-base.yml]`),不发明新语言
   - **诚实边界**:不自动写远程项目文件(`forge fleet apply` 默认 dry-run),不改写 forge-init 的资产复制路径

3. **增量价值**:在不引入 fleet 概念时,所有现有 forge 子命令完全不变;`forge fleet` 单独存在、可选使用。

### 为什么是 P1 而非 P3(v3)

- ADR 0003(agent-os submodule)已是 Proposed 状态,说明组织级共享已经在路线图上
- `forge-init` 的 copy-anywhere 治理已就绪(复制全套资产 + 全自测 ACCEPTED),复制能力有了,缺的就是「查看/管理已复制的项目」的视角
- **Fleet 是「软件工厂」的天然产品面**:单项目治理是「自动化 devops」,多项目舰队治理才是「工厂」
- **不依赖外部资源**:不需要 LiteLLM/Firecracker/DB,纯 YAML 扫描 + 聚合,是一个现在就能做的纯编排功能

---

## 方向五 · 自适应模式治理:从静态 mode/lifecycle 到动态策略调谐

### 为什么需要

当前 `mode × lifecycle` 中枢旋钮是**静态配置**的:
- 在 `project.yml` 写死 `mode: engineering, lifecycle: mvp`
- `forge migrate --to engineering` 是人工触发的一次性迁移
- 没有任何机制让系统**根据真实质量信号自动调整治理严格度**

这意味着:
- 一个项目在 `mode: explorer` 下长期跑但实际质量已足够(`coverage > 80%`,gate 持续全绿),却得不到 engineering 的完整保护
- 一个项目 `lifecycle: production` 但测试覆盖率从 80 跌到 30,系统不会自动发出警告或加强闸门
- **mode 切换依赖「有人想起来去跑 `forge migrate`」**,这在无人值守自治愿景下是断裂点

当前的治理信号已经足够丰富(coverage / gate PASS 率 / function-length 违规数 / 架构违规趋势 / 代码测试比),差的是**一条从信号到 mode 建议/自动调整的反馈路径**。

### 落地形态

1. **`forge recommend-mode`**(新子命令,或 `forge doctor` 的扩展):
   - 扫描项目最近 N 次 `forge accept` 结果 + scorecard + arch-check 历史 + 覆盖趋势
   - 输出建议:「基于最近 10 次 gate 全绿 + coverage 稳定 75%+,建议从 balanced 升级到 engineering」
   - **非自动执行**:输出建议,需要 `--apply` 或人确认才写 project.yml

2. **质量触发回滚(自动降级)**:如果 `mode: engineering` + `lifecycle: production` 下连续 N 次 `forge accept` REJECTED 且 coverage 跌破阈值,`forge run gate` 输出警告并建议临时降级到 `balanced`。**设计原则**:自动降级可以,但每次降级写审计日志(`.forge/.degradation-log`),升级永远需要 human approval。

3. **Lifecycle 推进信号**:聚合信号(覆盖率/测试数/架构违规数 trend/gate 通过率)判断项目是否达到 next lifecycle 的门槛——输出建议而非自动执行。

### 为什么是 P1

- **与现有信号系统完全兼容**:不产生新信号,只消费已经存在的 `converge.Signals` + scorecard + arch-check 结果
- **填补了「无人值守治理」的最后一段缺口**:安全护栏管资源、闸门管质量、checkpoint 管恢复——但谁来管治理配置本身是否匹配项目真实状态?自适应模式就是回答
- **降低认知负荷**:开发者不需要理解 mode/lifecycle 的复杂矩阵,系统帮他们判断并建议
- **低成本**:纯读取现有数据做统计 + 阈值判断,不需要新引擎或外部依赖

---

## 优先级评估

| # | 方向 | 优先级 | 风险/收益 | 与现有代码关系 |
|---|---|---|---|---|
| 1 | **Shared Workspace** | P1 | 解锁并行编排;中风险(设计需谨慎);收益高 | 新 `internal/workspace/` 包;零改现有串行路径 |
| 2 | **Observability Pipeline** | P1 | 低风险(纯查询层,不碰状态);收益中-高 | 在 `internal/trace` 上建 `forge trace`;全增量 |
| 3 | **Workspace Sandbox/Rollback** | P1 | 最低风险(git 操作);高收益(数据安全) | 新 `internal/snapshot` 包;零改 agent 逻辑 |
| 4 | **Fleet Governance** | P2 | 中风险(组织级功能需要设计);收益中 | 新 `forge fleet` + `internal/policy`;完全可选 |
| 5 | **Adaptive Mode Tuning** | P2 | 低风险(只读信号+建议);收益运营层面高 | 在 `forge doctor` 基础上扩展;零改现有 mode 逻辑 |

### 建议收敛顺序

**第一优先(方向三→方向二)**:Sandbox/Rollback 提供**数据安全保障**(无人值守的先决条件),Observability 提供**可观测性**(得知项目状态的能力)。两者都是纯增量、零风险、高杠杆。

**第二优先(方向一)**:Shared Workspace 打开并行编排的真正使用场景。选择这个时机——有了安全层(rollback)和可见性(observability)之后,再引入共享工作区,风险可控。

**第三优先(方向四+五)**:Fleet 和 Adaptive Mode 是 ForgeOS 从「单项目工具」进化为「组织级平台」的临门两脚,但它们的设计需要更多组织场景的输入,适合在产品打磨成熟后并行启动。

---

## 附录:与既有 60+ 分析的关系

本文不否定已有的大量分析(`docs/requirements/` 下 60+ 篇),而是聚焦在**已有分析尚未深度覆盖**的方向:

- **Shared Workspace**:已有分析讨论「并行编排」(方向五子项),但从未深入讨论并行 agent 的冲突协调与共享上下文问题
- **Observability Pipeline**:已有分析讨论 telemetry 数据采集(SCA/cost/latency),但从未设计**结构化查询 API**
- **Sandbox/Rollback**:已有分析讨论 checkpoint/resume(方向四)、四维安全护栏(方向四子项),但从未设计**工作树原子性保障**
- **Fleet Governance**:已有分析简单提及「跨项目」(expansion-five-product-blindspots.md 一行),但从未系统设计分层策略引擎和舰队命令
- **Adaptive Mode**:已有分析偶有提及(genuine-architectural-gaps-v28.md 一行等),但从未做基于真实质量信号的自动调谐设计

五个方向均属**已有能力纵深的自然延伸,而非凭空发明的「新功能」**——每个方向都能在当前代码库中找到明确的切入点和可复用原语。
