# AGENTS.md — 工程宪法

> 接管本仓库的 AI agent 的工程宪法;ForgeOS 自身遵守,经 `forge-init` 继承给每个被治理项目。
> 三档红线:🔴 **硬闸门**(`forge accept` 自动执法,违反即 REJECTED)· 🟡 **规范**(fresh-context Reviewer 裁量判)· 📋 **纪律**(工作方式约束)。
> 阈值是「触发审查」信号,非机械砍刀。无对应工具的检查诚实标 N/A,绝不伪造为通过。

## 🔴 硬闸门 — forge accept 自动拦截

- 单文件 ≤ 500 行 · 根目录 ≤ 15 文件
- 依赖方向:interfaces → application → domain 单向向内;domain 绝不 import 外层
- 函数 ≤ 50 行 · 循环依赖 = 0
- 包大小 · 生产扇入 · 认知负荷不超阈 · 禁技术角色目录名(utils/common/manager/handler/impl …)
- agent/skill/workflow/路由/mode 声明:全部交叉引用可解析,无悬挂引用、无声明-实现漂移
- 无硬编码 secret · test/app-test 全绿 · lint/coverage/sca 有工具则真查、缺则诚实 N/A

> 全部硬闸门均已机器执法,零 TODO;详情见 `forge accept` 输出。

## 🟡 规范 — Reviewer 独立裁量

- **单一职责**:一文件/一模块只做一件事;组合优先于继承;禁 God Object;禁根目录堆业务代码
- **Honesty 红线**:无工具项诚实标 N/A,不伪造通过;声明未实现的承诺属 GAP,不可默认为「已实现」
- **反镀金**:不做声明中不存在的特性;修复缺口前不顺手发明新行为

## 📋 纪律 — 工作方式约束

- **先拆分,再继续**:命中任何阈值即停新增,先重构,复检达标再继续
- **Reviewer 必须 fresh-context 独立 Agent**:实现者绝不审自己的代码;每轮 Review 使用独立干净的 Agent 会话
- **Host-agnostic 执法**:带外 gate(`forge accept`)是真相之源;宿主 hook(如 edit-time 加速器)只是可选的快速反馈,不替代完整闸门
- **复杂设计记 ADR**:架构决策、非显而易见的权衡、跨 sprint 的推理写入 `docs/adr/`
- **真点火需显式授权**:LLM 调用的真实预算消耗需获得用户明确许可,默认 dry-run 安全
- **新 ADR 使用 Proposed-only v2**:ADR-0067 只要求当前 `writes_adr` 新候选满足 exact JSON frontmatter/Markdown/digest 合同；owner/approver/Claim/Evidence/Graph 引用均为未认证声明。既有 ADR 仍参与 baseline integrity snapshot，但不做 v2 解析、retro-validation 或迁移；结构 PASS 不能冒充 Accepted、ApprovalRecord、compliance 或 lifecycle authority
- **Capability Registry 保持 authority-neutral**:ADR-0068 的 Registry wire 仍为 `staged`，ADR 状态仍为 `proposed`。已交付的 Go/Python strict evaluator、显式输入 CLI 与 physical checker 只验证/解析唯一 `local-go-package-impact-prescan/1` 声明；不得读取 ambient catalog/repository、生成 adapter、选择/执行 implementation、激活 Grant/PDP、构造 CapabilityInvocation 或做 runtime routing
- **Planning ownership 保持 logical-only**:ADR-0069 状态仍为 `proposed`。已交付的 Python/Go pure projector 与 `forge capability-ownership project --catalog FILE|- --mapping FILE|-` 只验证 caller-supplied exact planning sources 的 140-item unique primary-owner coverage，并派生 unresolved `.agent/skills/*.md` locator；不得据此声明文件/Skill/implementation 存在、生成 owner Skill/host adapter、修改 ADR-0068 Registry 或产生 Grant/PDP/CapabilityInvocation/runtime authority
- **Project snapshot 保持 bounded observation**:ADR-0070 状态仍为 `proposed`。Linux live producer、strict checker 与 portable `project-snapshot` Skill 只形成 fixed-policy、two-endpoint local Git worktree observation；必须保持 atomic/currentness/freshness/system completeness 为 false/UNKNOWN、coverage PARTIAL/NOT_OBSERVED/NOT_PERFORMED，且不得把 path exclusion 冒充 content secret scan，把 scaffold copy 冒充 runtime/host installation，或据此产生 GraphSnapshot、Registry mutation、Grant/PDP/CapabilityInvocation、truth/persistence/effect authority
- **Portable Context Engineering 保持 supplied-bytes only**:ADR-0071 状态仍为 `proposed`。Closed `context-engineering` package 只以零参数 stdin adapter 装配 caller-supplied exact ContextPackage request；不得发现 source、调用 provider/model、编译 live prompt、安装 host Skill，或产生 truth/instruction/Grant/PDP/Approval/completion/persistence/routing/effect authority。`-I` 不隔离 system site/stdlib/interpreter/host，package check 与随后 assembly 也非原子 check-to-use。
- **Portable Evidence Claim validation 保持 validation-only**:ADR-0072 状态仍为 `proposed`。Closed `evidence-claim-management` package 只验证 already-authored exact canonical record-set stdin bytes；不观察或创建、修复、排序、封印、返回或持久化 records，不访问 ambient source/journal/semantic view/proposal，不安装 host Skill，不产生 truth/instruction/Grant/PDP/Approval/completion/routing/transition/execution/effect authority。Portable prose 不进入 authenticated context routes；`-I` 和 package check 仍不是 host isolation、publisher authentication 或 atomic check-to-use。
- **Portable Policy Authority assessment 保持 declaration-only**:ADR-0073 状态仍为 `proposed`。Closed `policy-authority` package 的两个独立零参数 adapter 只比较 caller-supplied exact CapabilityGrant/ApprovalRecord declared-assessment request；不新增 combined envelope，不签发/批准/激活 Grant，不查询 policy/approval/revocation/usage state，不调用 ADR-0057/0058、Kernel/PDP/PEP 或 executor，不安装 host Skill，也不产生 effective approval、authorization、permission、completion、persistence、routing、transition 或 effect authority。Portable prose 不进入 authenticated context routes；`-I/-B` 与 package check 不是 host isolation、publisher authentication 或 atomic check-to-use。
- **Portable ADR Governance validation 保持 Proposed-only**:ADR-0074 状态仍为 `proposed`。Closed `adr-governance` package 只用 exactly one caller-supplied lexical basename 与 exact document stdin bytes 复用 ADR-0067 validator；basename 不证明物理 file/repository identity。不得新增 request envelope、扫描 repository、author/repair/reseal/accept/supersede/persist ADR、复制 Go `writes_adr` runtime、安装 host Skill，或产生 identity/ownership/approval/truth/compliance/lifecycle/completion/execution/effect authority。Portable prose 不进入 authenticated context routes；`-I/-B` 与 checker 不认证 host/publisher，也非 atomic check-to-use。

## 机器可读规范入口

- `engineering/activation.yml`:v1 shadow 默认值和 canonical refs；旧项目无显式绑定时也安全降级为 shadow
- `engineering/disciplines.yml`:14 个 Prompt/Context/Memory/Tool/Planning/Loop/Reflection/Graph/Harness/Eval/Knowledge/Evolution/State/Contract 学科状态
- `engineering/rules.yml` + `engineering/detectors.yml`:原子规则、强度、例外与 detector；Error 只能绑定 `forge accept` 的真实载重探针
- `engineering/context-routes.yml`:typed predicate、固定合并代数、预算阻断/省略回执和不可信内容 lane（当前仅 shadow policy）
- `engineering/workflow-profiles.yml`:L0–L4 风险到 W0–W3 保障下限；只增强既有 workflow，不创建第二套 DAG
- `eval/completion-evidence.schema.yml`:只封装 source-bound 结构化执行观察，禁止写 `completed/accepted/verdict`；最终裁决只归 `forge accept`
- `engineering/backend-decision-gates.yml`:后端逐触发器 L1–L4/W1–W3 的业务不变量、模型边界、持久化、契约、算法、并发、可靠性、安全、容量、运维、迁移和演进决策合同（当前 shadow）
- `eval/backend-decision-package.schema.yml`:要求 14 个维度逐项 `addressed/not_applicable/blocked`，区分事实/假设/证据；不得自行批准或宣告完成
- `skills/backend-engineering.md` 及其 Domain/Data/API/Reliability/Security/Performance/Ops adapters：按路径和 capability 路由，不要求简单 CRUD 机械生成全部模型层
- `engineering/frontend-design-gates.yml` + `frontend-profiles.yml`:前端场景/Profile/Page Pattern、业务链路、状态×权限×数据×系统动作、Token、响应式、可访问性、动效/性能和视觉证据合同（当前 shadow）
- `eval/frontend-design-package.schema.yml`:分离 artifact 与 proof claim，截图绑定 source/build/fixture/environment；只判结构有效性，不授予 UI 质量或完成裁决
- `skills/{information-interaction-design,design-system-accessibility,frontend-client-engineering}.md`:复用现有 capability ownership；产品风格是 Profile，React/Vue/Flutter/RN 是条件化平台映射，不制造平行 Skill 树
- `skills/ui-geometry.md`:条件化 supporting procedural adapter；用 `business_ui_composition` 把角色/任务/状态/数据语义绑定到区域、轴、分组、间距、线条、形状与响应式关系，并可接收声明式 `geometry_measurement_receipts`。它不新增 capability owner，也不证明浏览器/原生 Runner 真执行、视觉质量或完成
- `engineering/governance-contracts.yml` + `skills/{evidence-claim-management,context-engineering,policy-authority,knowledge-graph-curation,capability-registry,capability-ownership-projection}.md`:EvidenceRecord/KnowledgeClaim v1 identity/canonical/state、本地 exact append journal、ADR-0054 rebuildable declared semantic view、ADR-0055 authority-free ContextPackage pure builder及ADR-0056 CapabilityGrant contract-only envelope/declared assessment；ContextPackage 固定 `instruction_allowed=false`，pure Grant evaluator 固定无 authorization/permission/effect attestation。ADR-0059 已交付 ApprovalRecord v1 contract-only exact wire、caller-time declared assessment 与 Grant ref 投影；它不导入 marker/flag/actor hint，不认证或产生 effective approval。ADR-0060 已交付 TransitionReceipt v1 contract-only：只冻结 declared state graph/receipt/predecessor/recovery/evaluator 和 Grant/Approval reference comparison，不认证 controller/current state/precondition，不写 ledger 或推进 transition；其 Accepted 状态不产生 Transition authority。
- `skills/project-snapshot/` + `.agent/skills/project-snapshot.md`:ADR-0070 的 pure decoder 仅 source-portable，Linux-only capture adapter 只调用显式 `forge project-snapshot capture` 并 strict validate bounded observation；package/scaffold 不携带 Catalyst Go runtime、不安装宿主能力、不授予 worktree/process 权限，unsupported host 或 runtime 不存在必须 exit 3/`not_executed`，已存在但不兼容/执行失败必须 exit 1，且无 fallback
- `skills/context-engineering/` + `.agent/skills/context-engineering.md`:ADR-0071 只包装 ADR-0055 已冻结的 authority-free pure builder；closed manifest、isolated Python stdin adapter 与 package checker 不改变 ContextPackage wire，也不提供 live tokenizer/provider/model/PDP/runtime/persistence。Portable Skill prose不进入 authenticated context route。
- `skills/policy-authority/` + `.agent/skills/policy-authority.md`:ADR-0073 只包装 ADR-0056/0059 已冻结的两个 authority-neutral pure declared evaluator；closed manifest、两个 explicit-EOF exact-stdin adapter 与 checker 不改变 wire，也不提供 issuance/effective Approval/live policy/PDP/PEP/runtime/persistence。Portable Skill prose不进入 authenticated context route。
- `skills/adr-governance/` + `.agent/skills/adr-governance.md`:ADR-0074 只包装 ADR-0067 已冻结的 Proposed-document pure validator；closed manifest、one-basename-argv explicit-EOF stdin adapter 与 checker 不改变 wire，也不提供 authoring/acceptance/compliance/lifecycle/runtime/persistence。Portable Skill prose不进入 authenticated context route。
- `skills/change-impact-cost-risk/` + `.agent/skills/change-impact-cost-risk.md`:ADR-0076 只包装 ADR-0062 已冻结的 lexical ImpactPreScan；closed manifest、one zero-argument exact-request explicit-EOF adapter 与 checker 不改变 wire，也不提供 raw graph authoring、live capture、完整 Impact/Cost/Risk、materiality/safety、runtime、route 或 persistence。Portable Skill prose不进入 authenticated context route。
  ADR-0061 已交付 KnowledgeUpdateProposal contract-only exact closure/create/supersede wire 与 Grant/Context/artifact declared comparison；它不认证 proposer/Grant/Context/Evidence，不读取 current head、仲裁 conflict/freshness/policy，不产生 truth/adoption/authorization/permission/persistence/apply/receipt/execution/effect，其 Accepted 状态不产生 Knowledge authority。ADR-0068 交付唯一 staged singleton Registry 的 strict validator/resolver/physical checker/CLI；ADR-0069 独立交付 exact planning source→complete unique owner→logical adapter-ref pure projection，不修改 Registry、不解析/生成物理 Skill，也不产生 invocation/Grant/PDP/runtime authority。ADR-0065 已交付的 GraphSnapshot profile 只把 caller-supplied exact ADR-0053 bytes 纯投影为 PARTIAL module/package graph，完整保留 unresolved/coverage/freshness UNKNOWN；ADR-0066 的独立 profile 只增加 package-scoped lexical test source-set node 与 module→test edge，Go/test coverage 都保持 PARTIAL，且不把 source presence 冒充 test case/execution/result/coverage/verification。两者都不是 live producer、完整 System Knowledge Graph、G3、Assessment Join、Impact/Cost/Risk 或 authority。
  ADR-0057 只开放 operator 部署的独立非 Agent `forge-kernel` 之 `authenticated_bootstrap_repo_read_grant_issuance_v1`：repo 外 pinned root/key/state、signed policy/request、bootstrap `repository-reader/v1` + `repo.read` exact paths、local/development/test、小预算/TTL、signed Grant + durable signed receipt。ADR-0058 已交付 `authenticated_bootstrap_repo_read_execution_v1`：独立 execution root、signed execution policy/invocation、Linux amd64/arm64 `openat2` exact-byte read、reservation→intent→terminal single-use ledger、无 raw persistence 与 receipt-only replay；cooperative timeout 和管理员完整 state rollback caveat 必须显式保留。Mutable bytes 只 best-effort clear，不能证明 string/GC/kernel/downstream copy secure erasure、process isolation 或 HSM。`0600`/effective-UID 不是 OS principal/HSM 隔离；其余 Kernel/PDP/authenticated Approval/revocation/general effect 仍不可用。已交付的 ADR-0051 local gate、ADR-0052 local Evolve locator 与 ADR-0053 `selected-module-all-regular-go-files-union-v1` dependency graph observation 都只允许显式 opt-in 的 Catalyst Go API。
  三者默认 capture 关闭，不签发 PASS/scan/build/architecture judgment、completion 或 persistence；共享 source 只是 bounded-interval observation，不认证 Git。引用闭包的 1024 records/16 MiB/256 depth 只防资源耗尽；scaffold 不安装 runtime/root/key/state，缺兼容 binary 时必须 `not_executed`，`forge accept` 从不是 issuer、Transition/Knowledge/Graph authority 或 execution authority

## 阅读顺序

`BOOTSTRAP → .agent/{PROJECT · ARCHITECTURE · ROADMAP · CURRENT_SPRINT} → 本文件 → engineering/context-routes.yml → 任务相关知识 → 代码`

Portable Knowledge Graph Curation 只 source-distribute ADR-0065/0066 已有的两个 exact-request partial projectors；两条 wire 独立、zero-argument、explicit-EOF，且保持 PARTIAL/UNKNOWN、无 route/runtime/persistence/impact/authority。

Portable Change Impact Cost Risk 只 source-distribute ADR-0062 已有的 exact seven-field lexical ImpactPreScan projector；system impact 恒 UNKNOWN，且无 raw graph/wrapper、route/runtime/persistence、完整 Impact/Cost/Risk/materiality/safety 或 authority。

WorkIntent v1 Proposed candidate 只允许 ADR-0077/0078 的 exact Python/Go/Rust structural parity、Registry v32 candidate metadata、checker-only shadow 与 source-only Python distribution；Go/Rust 保持 Catalyst-only，所有 scope arrays 和 context routes 不增加 WorkIntent。Valid/digest parity 不是 accepted semantic authority，不关闭 G0，也不产生 authentication、reference resolution、route/runtime、Run/RunJournal/lifecycle、Approval/Grant、persistence 或 effect。

Authenticated ADR approval v1 Proposed structural prerequisite 只允许 ADR-0079/0080 的 caller-supplied structure/digests/relations、Registry v33 candidate metadata、checker-only shadow 与 dependency-free Python source-only distribution。它不验证 Ed25519、不认证或授权、不签发 receipt、不消费或证明 external root pin、trusted time/revocation currentness，也不提供 CAS、durability、Accepted lifecycle、G0 closure、Skill、route、scope/evaluator/producer/runtime、persistence 或 effect；future Go service、production keys/state 不进入分发。

Authenticated ADR lifecycle v1 Proposed candidate 采用 Registry v34 双边界：ADR-0081 只作为 Catalyst-repository-only Go approval-authority evidence，ADR-0082 只作为 dependency-free Python structural candidate。完整 scope SHA-256 仍为 `8ba82b638e8031f0d1be2b9ea6d522a4b9cf064a4ed532e1f0d3281f2dfe874c`，只新增一个 checker-only shadow；无 Skill、route、kind/evaluator/producer/runtime。Source distribution 不复制 Go authority、production key/state；structural success、`StoredAuthorization` prerequisite 或 `forge accept` 都不执行 ADR lifecycle transition。

Registry v35 lifecycle authority evidence 由 ADR-0084/0085 单独冻结：真实 Go lifecycle authority 仍只存在于 Catalyst、依赖显式外部 trust 与 opaque `StoredAuthorization`；generated project 只接收 source-only 决策与 governance 检查，不复制 Go、root/key/seed/state/receipt/ledger，不新增 route、Skill 或 runtime profile。

ADR-0087 unverified legacy read import 仅登记 caller-supplied Memory/ADR exact bytes 的只读投影候选。Registry v36 不提供 detector stdin，不扩展 scope、route、Skill、runtime 或 authority；operator 必须显式 pipe request 并关闭 EOF，Catalyst Go parity 不进入 source distribution。

ADR-0089 在 active Registry v37 只登记 ADR-0088 Kernel operational reference subclosure 的结构候选；scope digest 保持不变，detector 只执行 pinned golden，Python exact source 可分发，Catalyst Go/Rust parity、runtime、route、Skill、authority 与完整 Kernel ABI 均不进入交付。

ADR-0090/0091 的 repository slice 已在 active Registry v38 完成交付，但两份 ADR 继续保持 Proposed，完整 scope mapping 与其 SHA-256 保持不变。它只登记 Kernel structural reference-family：CognitiveAtom v2、DecisionTransaction v1 及对五类 operational records 的单向引用闭包；detector 仍只执行 pinned golden，exact19 分发仅复制 dependency-free Python core/governance。Catalyst exact13 Go、flat exact9 Rust 与共享 `lib.rs` registration 不分发；22 项 attestations 均为 false，不新增 Skill、route、runtime、PDP 或 controller。该窄 roadmap 项已通过正式 `forge accept` 并完成；ADR-0038 仍为 ADOPTED-PARTIAL，DecisionCapsule、AuthorizedTransactionSpec、authenticated PDP 与 rolling controller 继续开放。

ADR-0092/0093 在 active Registry v39 只登记 Decision Capsule 四对象 structural replay repository Candidate；两份 ADR 始终保持 Proposed/null，完整 scope mapping 与 SHA-256 不变。Checker-only shadow 只执行 pinned golden，exact19 分发只复制 dependency-free Python core/governance；Catalyst exact15 Go、exact14 Rust 与共享 `lib.rs` registration 不分发。32 项 attestations、effect replay/history rewrite 与七项 completion 均为 false，无 Skill、route、runtime、Reflection consumer、PDP 或 controller。该窄项仍待独立复核与正式 `forge accept`，不得冒充完整 DecisionCapsule、ADR-0038、AuthorizedTransactionSpec 或 authority/runtime delivery。
