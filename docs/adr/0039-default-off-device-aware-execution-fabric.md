# ADR-0039 — Default-off Device-Aware Execution Fabric

- 状态：已接受（2026-08）
- 范围：执行平面目标抽象与渐进路线；planning-only，不授权任何远程访问
- 关联：ADR-0037、ADR-0038、`docs/design/ai-engineering-os/device-aware-execution-fabric.md`

## 背景

未来任务可能需要本机、异构设备、CI、集群或模型 endpoint，但当前 ForgeOS 只有本地 process、Docker/Firecracker 等基础；
完整 coding-workspace exchange、Rust runtime OS sandbox、远程 registry/runner/placement/lease/migration 均未实现。

若业务代码继续假设本机路径、localhost、共享文件系统和同步 shell，将来接设备会大规模重构；若现在直接用 SSH 拼命令，
又会掩盖分布式失败、身份、数据和副作用风险。

## 决策

1. 引入独立、可关闭、默认 OFF 的 Device-Aware Execution Fabric。接口可位置透明，失败必须显式。
2. 分离 Logical Agent、TaskNode、DecisionTransaction、ExecutionAttempt、ExecutionTarget；Agent 不永久绑定设备。
3. 现在先冻结 ExecutionTarget/Attempt、ArtifactRef、EnvironmentDigest、PlacementConstraint、Effect/Mobility、Lease/Checkpoint/
   Evidence ABI；LocalExecutionTarget 保持现有行为。
4. 设备身份来自 key/certificate/attestation；IP/hostname/SSH alias 仅是 locator。能力带 probe evidence/version/expiry，静态能力
   与动态状态分开。
5. Placement 先做硬过滤，再比较可靠性、数据/模型局部性、DAG/通信，最后才可在同等质量目标间考虑成本。Fabric 不改变
   目标或技术方案。
6. 远程 attempt 使用资源 reservation、lease epoch、fencing token、content-addressed Artifact 和 execution evidence；状态/
   checkpoint 外部化。LOST/取消超时不等于 FAILED/已停止。
7. 不承诺 exactly-once；使用幂等 key、effect receipt、reconciliation、compensation。不可逆或 effect unknown 进入 quarantine，
   禁止自动 retry/migrate/speculate。
8. SSH 只作 enrollment/bootstrap、诊断和早期结构化 MVP；正常路径是主动注册的 mTLS Runner。默认关闭 LAN scan、自动安装、
   arbitrary shell、peer transfer 和迁移。
9. ForgeOS 不因 Fabric 获得 production credential/apply/deploy 权限；生产继续由外部 operator authority。

## 模式与实施顺序

`OFF → INVENTORY → OBSERVE → EXECUTE → MIGRATE → FEDERATE`，每级仍需独立 policy/Grant。实施为：Local ABI → static
inventory/read-only probe/placement dry-run → structured SSH low-risk MVP → Runner/lease/fencing → scheduler/controllers → safe
checkpoint/reconciliation → federation last。

## 权限与诚实边界

- 本 ADR 不允许扫描网络、导入/复制私钥、连接 SSH、注册设备、远程执行或迁移；
- 沙箱不能防不可信设备管理员读取任务，因此 trust/data residency 是硬过滤；
- probe 不能安装/修复；成本默认 observe-only；fallback 必须重新可行性检查和授权；
- external message/real cost/模式升级/数据跨域必须回 00 影响评估和人类授权。

## 后果

正面：现在消除本机隐式假设，未来可以安全接异构算力、跨平台验证和模型服务；任务、证据、资源和失败可审计。成本：需要
identity/PKI、registry、CAS、Runner protocol、scheduler、leases、controllers、reconciliation、zero-trust policy 和 chaos tests。

## 被拒方案

1. `enable_remote=true` + SSH shell：权限粗、注入、状态/取消/恢复不可证明；
2. IP/hostname 当身份：地址变化或复用会混淆主体；
3. 所有设备共享可变目录：一致性、锁、跨平台语义和缓存污染；
4. 所有失败自动 retry/migrate：会重复真实副作用；
5. 一开始做 P2P/Federation：复杂度先于价值和内核稳定性。

## 重审触发器

- LocalExecutionTarget ABI 无法覆盖现有 sandbox/runner 行为；
- 目标 trust/数据驻留无法在 placement 前确定；
- lease/fencing/reconciliation 无法阻止迟到 attempt 提交；
- 远程 Fabric 会扩大已冻结的 production non-goal。

## 当前实现边界（2026-09）

Runtime TUI 可以把已显式打开的 inventory v2 与 client-instance resource
observation 当作同一份只读快照进行一致性检查。两份观察同时存在且 owner、设备、生命周期、能力或观察时间不一致时，Prompt、pending
Run-intent 以及 scheduler selection preview/lease claim/renew 在本地 fail-closed，且不会发出请求；单侧观察仍保持兼容，lease release 保留用于清理。
这只是客户端显示与 planning-only 请求边界，不改变本 ADR 的 OFF/INVENTORY/OBSERVE 约束，也没有授予 enrollment、heartbeat、reservation、dispatch、Runner 或执行 authority。

Console 的 Web/App/Mobile Sessions 端在切换本地 client-instance 过滤器时保留
owner 级 inventory/resource 观察，只清理私有 Conversation/Prompt/Run 投影；并对并发的显式 inventory/resource 刷新做合并，失败仍保持 stale、fail-closed。
这同样只是观察状态管理，不改变默认关闭或任何设备执行 authority。

同时，Console 在同时启用独立的 client-instance/session-view 与
client-instance/resource-view reader 时，只有 owner 声明和完整实例行（含
client kind、status、session membership、observed_at_ms）完全一致才使用本地
过滤器。缺失、loading、stale、失败或 drift 会让选中的 projection
fail-closed，并阻止私有 Prompt/Run 重新 hydration；被拒绝的刷新只保留为
display metadata 也不能驱动过滤器或私有读取，后续刷新仍可替换成同一份
收敛快照；如果 Run 或 pending Run-intent 读取在 reader gap 期间返回，
最终可见性检查会丢弃该私有响应。这是观察一致性边界，不增加 enrollment、heartbeat、
inventory mutation、reservation、dispatch、Runner 或执行 authority。

Runtime TUI 对独立 client-instance/session-view 与
client-instance/resource-view 观察也保留最后的 display metadata，并明确
显示 pair 尚未收敛；只要存在 active client-instance filter，过滤与私有
Prompt/Run projection 就保持 fail-closed，直到后续刷新得到相同 owner 和
完整实例行。没有 active filter 时继续保留既有 owner-read 兼容行为。这仍
只是本地观察状态，不增加 enrollment、heartbeat、inventory mutation、
reservation、dispatch、Runner 或执行 authority。

Forge Core 还以 production-constructor 回归矩阵锁定执行相邻候选路径的
默认关闭：Runner execution-intent、dispatch-plan、dispatch-admission、
transport-admission、execution-boundary、Attempt-boundary、execution-
reconciliation、execution-evidence preview，以及 scheduler selection 与
lease claim/renew/release 在普通认证会话构造下均返回标准 404。只有显式
device-fabric EXECUTE assembly 才能挂载这些候选。这是路由闭合证据，不授予
Runner、reservation、lease mutation、dispatch、transport、execution、receipt
persistence 或 Audit authority。

Runtime TUI 进一步在所有十四个 selected-session Runner metadata/preview
命令的首个请求前复用 client-instance 可见性门禁，覆盖 local Runner
readiness、execution/Attempt boundary、dispatch/transport admission、receipt
history/reconciliation/evidence、preflight、execution consent 与 lease
preview。隐藏或未收敛的 Conversation 在本地返回并明确不发请求；无过滤器
时保留原有 owner-read 兼容路径。这只是本地显示/read 边界，不增加
enrollment、heartbeat、inventory mutation、target selection、reservation、
lease mutation、dispatch、Runner transport/execution、receipt persistence 或
Audit authority。

Console Web/App/Mobile Sessions 端现在也可以显式注入独立的 Runner receipt
observation reader，但只有它与 Run observation reader 并发返回并通过同一
convergence check 时才提交任一私有投影。owner、Conversation、Prompt、Run
摘要以及 Attempt、Command、target、digest、disposition、观察时间和
`uncertain/reconciliation_required` 必须完全一致；读取失败、漂移、选中项
变化或迟到响应会清空/拒绝这对值，execution evidence 只能使用已收敛的
receipt。Gate 默认不注入 reader，因此仍 request-free。这是 display-only
观察一致性边界，不授予 receipt persistence、device enrollment/heartbeat、
inventory、target selection、reservation、scheduling、lease、dispatch、
Runner transport/execution 或 Audit authority。

Native Android/iOS acceptance 现要求显式 `client_instance_id` 与 owner-bound
session/resource view pair。只有声明该 Conversation 的可见 `mobile` 实例才
能继续读取或写入 Prompt；缺失/隐藏 membership、非 mobile 行、owner 或实例行
漂移、非 display-only authority、非法容量、重复键或额外字段会在首个
Conversation/Prompt 请求前 fail-closed。Go host journey 只挂载 test-only 的
read candidate，并通过 Snaplink JWT 证明两次 native cold start 先读取收敛 pair。
生产构造器仍 default-off；此切片没有启用 enrollment/heartbeat、inventory
mutation、device write、Run-intent、placement、scheduling、Runner、receipt、
dispatch 或 execution authority。ADR-0039 仍 planning-only，ADR-0114 仍
Proposed/null，P4/live Runner effects 继续单独 gated。

Runtime CLI 的 `remote execution-consent preview` 现支持显式
`--instance INSTANCE_ID` 和可选的本地 `--instance-view FILE|-`。在线筛选会先
读取并校验同一 owner 下的 session/resource view pair，只有完整实例行收敛且
该实例声明 Conversation 时才发送 consent GET；隐藏、缺失、非法或漂移会在
发送前 fail-closed。显式本地 view 只做严格离线校验并避免候选读取；无
`--instance` 的旧路径仍只发送原有单个 consent GET。该命令仍是只读的
planning/display seam，没有 grant、Run、target、reservation、lease、dispatch、
Runner、receipt、enrollment、heartbeat 或 execution authority；ADR-0039 仍
planning-only，ADR-0114 仍 Proposed/null，P4/live Runner effects 继续 gated。

认证 profile 现单独声明 `forge:devices:read` 作为 device-observation scope，
并描述一个 scopes 为 Conversation read 加 device read 的
`forge-device-observer`。Snaplink 分布式 seed 将该 client 保持 `active: false`；
`forge-cli` 与 `forge-console` 仍只允许并默认请求 Conversation read/write。
owner-parity 测试证明默认 client 申请 device scope 会被拒绝，只有 test-only
显式激活的 observer 才能取得同 issuer、subject、tenant 和 Forge audience 的
只读 token。Core、Runtime 与 Console 的 strict profile consumers 已同步，
Console 不会把该 profile 加入默认登录 URL。该配置声明不会挂载生产
session/resource route，也不会授予 enrollment、inventory mutation、target
selection、reservation、scheduling、lease、dispatch、Runner、receipt 或
execution authority；Aero-ID、Aero-IM、Aero-Vault 与 Audit Governance 仍只消费
离线审计/receiver contract。ADR-0039 仍 planning-only，ADR-0114 仍
Proposed/null，P4/live Runner effects 继续 gated。

Forge Core 的 opt-in convergence E2E 现在跨过真实 Snaplink JWT：同时携带
`forge:conversations:read` 与 `forge:devices:read` 的 token 可以读取 owner-bound
session/resource pair，并验证 CLI/TUI/Web/App/Mobile 五类实例行和只读
device/Runner 观察；只有 Conversation read 的 token 仍可读 session metadata，
但在 resource source 被调用前就因独立 scope gate 得到 `403`。即使 full-scope
token 通过普通 production constructor 访问，两条路径也继续返回标准 `404`。
这只是认证 scope/source boundary 证据，没有启用生产 enrollment、heartbeat、
inventory mutation、target selection、reservation、scheduling、lease、dispatch、
Runner、receipt 或 execution authority；ADR-0039 仍 planning-only，ADR-0114 仍
Proposed/null，P4/live Runner effects 继续 gated。

Runtime CLI 的 `remote placement scheduler-preview` 现支持
`--instance INSTANCE_ID` 和可选的严格本地 `--instance-view FILE|-`。在线模式先
读取同一 owner 下收敛的 session/resource pair，只有请求中的 Conversation 被
目标实例声明后才发送 scheduler-preview POST；隐藏 membership、非法或漂移的
观察会在候选请求前 fail-closed。本地 view 只做 display-only 校验并避免 pair
读取；省略 `--instance` 仍保持原有单次 POST。该切片仍不选择 target、不创建
reservation/lease、不 dispatch 或执行 Runner，也不产生 receipt、enrollment、
heartbeat 或 execution authority；ADR-0039 仍 planning-only，ADR-0114 仍
Proposed/null，P4/live Runner effects 继续 gated。

Forge Core 现在补充了一条 opt-in 的真实 Snaplink JWT scheduler-preview
convergence E2E：它读取完整的 CLI/TUI/Web/App/Mobile client-instance
session/resource pair，再以同一 owner 下的 device/Runner inventory 与 policy
image 发送一次 planning-only scheduler preview，并校验返回的 selected pair
与 resource observation 一致。缺少 scheduler-preview scope 的 token 会在
Run、inventory、policy 或 client-instance source 被调用前得到 `403`；完整 scope
token 通过普通 production constructor 仍得到标准 `404`。contract gate 以显式
环境变量运行该测试；此证据不启用 reservation、lease、dispatch、Runner
execution、enrollment、heartbeat、receipt 或 P4 authority。ADR-0039 仍
planning-only，ADR-0114 仍 Proposed/null，P4/live Runner effects 继续 gated。

Console Web/App/Mobile 的 local Runner execution-readiness preview 现会在调用显式
candidate reader 前刷新 selected client-instance session/resource pair，以及独立配置的
inventory/resource observation。pair 缺失、过期、漂移或撤销会清空 readiness projection，
并在 candidate POST 前 fail-closed；Conversation/Run 选择变化同样不能复用旧结果。
Flutter 回归验证第二次 pair refresh 隐藏 selected session 时没有发送
execution-readiness 请求。该切片仍是 metadata-only preflight，没有 Runner transport/argv
execution、lease mutation、reservation、scheduling、enrollment/heartbeat、receipt
persistence 或 Audit authority；ADR-0039 仍 planning-only，ADR-0114 仍 Proposed/null，
P4/live Runner effects 继续 gated。

Forge Core 的真实 JWT client-instance session projection E2E 现在也驱动 Rust
Runtime CLI 完成一次带 `--instance client-cli-001` 的 Prompt append。CLI 在
Prompt POST 前读取并校验 owner-bound session/resource pair；把同一写入改为
`client-web-001` 未声明的 Conversation 时，会在 pair guard 处失败并且不发送
Prompt 请求。TUI、Web、App、Mobile 的既有写入仍汇入同一 owner Conversation
历史。该切片仍是 storage-only Prompt boundary，没有新增 session/Prompt authority、
Run、inventory mutation、target selection、reservation、scheduling、lease、dispatch、
Runner、receipt persistence、enrollment/heartbeat 或 Audit authority；ADR-0039 继续
planning-only，ADR-0114/P4 继续 gated。

Runtime CLI 的 scheduler lease claim、renew、release 现在也接受显式的
`--instance INSTANCE_ID` 与可选本地 `--instance-view FILE|-`。在线路径会在
candidate POST 前读取并校验同一 owner 的 session/resource pair；隐藏、非法或
漂移的 Conversation 会在 lease 请求发出前 fail-closed。本地 view 保持严格的
display-only 离线路径，省略筛选参数继续兼容原有单次请求；idempotency key、
lease proof/response binding 与 TUI fencing-token 隐藏保持不变。该切片没有打开
生产 lease route、enrollment/heartbeat、inventory mutation、Runner dispatch/transport/
execution、receipt persistence 或 Audit authority；ADR-0114/P4 继续 gated。

Forge Core 的 opt-in Runtime CLI E2E 现在以真实 Snaplink JWT 驱动
`remote runner-dispatch-plan-preview --instance client-cli-001`。CLI 必须先
读取并校验 owner-bound client-instance session/resource pair，才可提交
planning-only dispatch-plan candidate；使用未声明 Conversation 的
`client-web-001` 会在 POST 前 fail-closed，普通 production dispatch-plan route
仍为 404。该证据保持 display-only，不新增 target selection、reservation、lease、
dispatch、Runner transport/execution、enrollment/heartbeat、receipt persistence 或
Audit authority；ADR-0114/P4 继续 gated。

§667 增加了同一 owner-bound pair 对 Runtime CLI scheduler lease claim 与
renewal 的真实 Snaplink JWT 证据。可见的 `client-cli-001` 操作必须按
session-view → resource-view → lease POST 收敛；隐藏的 `client-web-001`
claim/renewal 只允许两次 pair read，随后在 lease POST 前 fail-closed。普通
production constructor 对 claim、renewal、release 仍返回 404；这项证据不打开
Runner transport/execution、enrollment/heartbeat、receipt、Audit 或 live P4
authority。

§668 将同一证据扩展到 scheduler lease release。带有 `--instance
client-cli-001` 的可见 release 必须先按 session-view → resource-view 收敛，再
发送一次 release POST，并校验 renewed fencing proof 对应的 owner、Conversation、
Run、Attempt、target、epoch、release timestamp 与全 false authority；隐藏的
`client-web-001` 只允许两次 pair read，随后在 release POST 前 fail-closed。普通
production constructor 对 claim、renewal、release 仍返回 404；该切片不打开
Runner transport/execution、enrollment、heartbeat、receipt、Audit 或 live P4
authority。

§669 将 Console Web/App/Mobile 的 Runner dispatch-plan candidate 也绑定到
selected client-instance 的 session/resource pair。Gate 在 candidate reader/POST
前重新读取两侧观察；任一侧撤销 Conversation、过期或漂移都会清空 candidate
并且不发送 dispatch-plan 请求。默认 Gate 仍 request-free；该切片不打开
Runner transport/execution、enrollment、heartbeat、reservation、lease mutation、
receipt、Audit 或 live P4 authority。

§670 增加 Runtime TUI 的真实 Snaplink JWT candidate 链路证据。PTY TUI 先读取
v2 inventory，再读取并收敛 client-instance session/resource pair，选择
`client-tui-001` 后才发送一次 planning-only Runner dispatch-plan POST；隐藏的
`client-web-001` 只完成 owner-bound 观察读取并在候选 POST 前停止。普通 production
dispatch-plan constructor 仍返回 404。该切片不打开 target selection、reservation、
lease、dispatch、Runner transport/execution、receipt、enrollment、heartbeat、Audit
或 live P4 authority。

§671 将同一 pair 约束落实到 Console Web/App/Mobile Sessions Gate 的真实 JWT
candidate journey。可见的 `client-cli-001` 必须刷新并校验 session-view 与
resource-view 后才发送一次 dispatch-plan POST；隐藏的 `client-web-001` 以及
session/resource 任一侧漂移都会在 pair read 后 fail-closed，且不发送 POST。普通
production session/resource/dispatch constructor 仍返回 404。该切片不打开 Runner
transport/execution、enrollment/heartbeat、reservation、lease mutation、receipt、
Audit 或 live P4 authority。

§672 增加 Runtime TUI scheduler-preview 的真实 Snaplink JWT pair 证据。PTY
TUI 通过 `client-instances show-converged` 读取会话与 resource-view，选择
`client-web` 后才发送一次 planning-only scheduler-preview POST；隐藏的
`client-tui` 在同一 pair read 后因 Conversation 不属于该实例而 fail-closed。
普通 production scheduler-preview constructor 仍返回 404。该切片不打开 target
selection、reservation、lease、Runner、dispatch、receipt、enrollment/heartbeat、
Audit 或 live P4 authority。

§673 将同一 pair 约束扩展到 Runtime TUI scheduler-lease claim。可见实例先读取
session-view 与显式 resource-view，再提交一次只返回脱敏 fencing 信息的 lease
claim；隐藏实例完成 pair read 后不发送 lease POST。验收使用第二个合规资源以尊重
历史 fencing epoch 的持久占用语义。普通 production claim/renew/release constructor
仍返回 404；该切片不打开 Runner transport/execution、enrollment/heartbeat、receipt、
Audit 或 live P4 authority。

§674 将独立 session-view/resource-view reader 的真实 JWT 验收扩展到 Console
Web、桌面 App 与 Mobile。每个界面先收敛两侧只读观察，再在长列表中定位实例会话并
执行一次 owner-scoped Prompt 写入；隐藏会话不会产生 Prompt POST。无初始实例选择的
双 reader widget 回归也证明 owner 会话仍可见。该切片没有新增会话或 Prompt 以外的
authority，candidate route 仍只在 opt-in harness 挂载，production constructor 仍关闭。

§675 为 Console scheduler-preview candidate 增加独立 pair gate 回归。选定实例在
planning-only POST 前重新读取 session-view 与 resource-view，并且只发送一次预览；
隐藏 Run 的实例在本地 fail-closed，不触发 scheduler 请求。默认 Gate 与 production
scheduler-preview constructor 继续关闭，没有新增 reservation、lease、Runner、执行、
receipt 或 Audit authority。

§676 将 Console scheduler candidate 的资源边界收紧到同一 owner 的组合
inventory/resource 与独立 client-instance session/resource pair。组合读者自身收敛并不代表
Sessions 当前选中的实例仍引用同一 revision、generation、heartbeat 与设备集合；因此在
scheduler preview、lease claim/renew/release 及同一 metadata 操作进入 candidate reader 前，
Gate 会确认两份新鲜快照一致，缺失、错误、过期或漂移均保持零 POST。有效快照继续只打开
既有 opt-in candidate；production constructor、enrollment/heartbeat、inventory mutation、
Runner transport/execution、receipt、Audit 与 P4/live authority 仍关闭。

§677 为 Audit Governance 增加 `forge-run-attempt-lease-dispatch-preflight-request-v1`
的严格归档接收器与 canonical fixture。接收器绑定 owner、Conversation、Run、Attempt、
placement、Runner intent、幂等键与 lease target，拒绝未知/重复/尾随字段、选中 target、
lease target 漂移及非零 authority；它不认证设备、不选择或预留容量、不发放或变更 lease，
也不触发 Runner、receipt 或 Audit 发布。该切片只是跨生态 ABI 兼容证据，ADR-0039、
ADR-0114 与 P4 继续 gated。

§678 在 Core 的 opt-in acceptance mux 中以真实 Snaplink JWT 驱动 Console Web、桌面
App 与 Mobile 的 scheduler lease projection。每个可见实例先读取 inventory-v2/resource
和 session/resource，再对同一 fenced lease 做一次幂等 replay；隐藏 Conversation 只读取
owner-bound pair，保持零 lease POST。Conversation 子路径只在 test mux 注册，普通生产
session/resource/inventory/lease constructor 仍为 404/default-off；enrollment/heartbeat、
inventory mutation、额外 scheduling authority、Runner transport/execution、receipt、Audit
与 live P4 effect 均未打开。

§679 将同一 owner-bound client-instance session/resource refresh 边界扩展到 Console 的
Run/Attempt lease dispatch preflight 与 Runner Attempt-boundary candidate reader。Web/App/
Mobile Sessions Gate 在候选 POST 前重新读取 pair；隐藏、撤销、过期或 drift 的 selected
Conversation 在本地 fail-closed，保持零 preflight/Attempt-boundary POST。该切片仍是
planning/display seam，没有打开 enrollment/heartbeat、inventory mutation、target selection、
reservation、lease mutation、Runner transport/execution、receipt persistence、Audit authority
或 live P4；ADR-0114 仍 Proposed/null。

§680 将 `forge-run-attempt-lease-dispatch-preflight-request-v1` 的严格归档接收器
扩展到 Aero-ID Audit Governance connector。它镜像 canonical fixture，绑定 owner、
Conversation/Run/Attempt、placement、Runner intent、幂等键及 lease target/epoch，拒绝
未知/重复/尾随字段、selected target、lease target 漂移和 authority elevation；仍是
offline archival receiver，不新增 JWT、路由、数据库、lease、调度、Runner、receipt 或
Audit authority，ADR-0039、ADR-0114 与 P4 继续 gated。

§681 将同一 `forge-run-attempt-lease-dispatch-preflight-request-v1` 归档 ABI 扩展到
Aero-IM Rust Audit connector 与 Aero-Vault Go Audit Governance receiver。两端均与 canonical
fixture 字节一致，并拒绝未知/重复/尾随字段、selected target、lease target 漂移及
authority elevation；这仍是 offline archival 校验，不增加设备认证、target selection、
reservation、lease、调度、Runner、receipt 或 Audit authority，ADR-0039、ADR-0114 与 P4
继续 gated。

§682 将 dispatch-preflight request boundary 固定在 Forge Runtime CLI/TUI。认证客户端对
canonical fixture 做 SHA-256 原始字节校验，并在发送 candidate request 前拒绝未知、重复、
尾随、selected target、lease target 漂移及 authority mutation。该切片仍是 bounded metadata-only
client boundary，不开放 production route、Runner transport/execution、lease mutation 或 P4
authority，ADR-0039、ADR-0114 与 P4 继续 gated。

§687 将同一边界扩展到真实 Snaplink JWT 驱动的 Runtime TUI。可见
`client-tui-001` 在 metadata-only Run/Attempt/lease preflight POST 前先读取 owner-bound
inventory/resource 与 client-instance session/resource pair；隐藏 Conversation 会重新读取这些
观察但在 candidate POST 前 fail-closed，普通 production preflight constructor 继续 `404`。该验收
不增加 inventory mutation、target selection、reservation、lease mutation、Runner
transport/execution、receipt persistence、Audit publication 或 P4 authority；ADR-0039、ADR-0114
与 P4 继续 gated。

§688 在现有 owner-local dense change feed 之上增加只读认证 SSE/long-poll 边界
`/api/v1/conversation-changes/stream`。它使用已验证的 Snaplink owner 与
`forge:conversations:read` scope，先重放当前连续 metadata change，随后以有界服务器等待轮询
后续 change；无新 change 时返回 `204`，取消、非法 cursor、scope 缺失或后端不安全响应继续
fail-closed。该传输不携带 Prompt 内容，不增加 Run、设备 inventory、enrollment/heartbeat、调度、
Runner、receipt 或 execution authority；ADR-0039、ADR-0114 与 P4 继续 gated。

§689 将该 change stream 通过真实 Snaplink JWT 集成验收，检查 owner、cursor、SSE event 与
metadata-only payload 的绑定；Snaplink Console Web/App/Mobile 增加显式
`conversationChangesStream` 只读适配器，严格接受单个 `conversation_changes` 事件或空 `204`，并
拒绝 malformed、重复字段、游标漂移和 unsafe 数值；Sessions UI 默认轮询保持不变。没有 Runtime
backend 的普通 constructor 仍返回 `conversation_service_unavailable` 并 fail-closed。该验收不增加
Prompt、Run、设备、lease、Runner、receipt 或 execution authority；ADR-0039、ADR-0114 与 P4 继续
gated。

§683 将 Run/Attempt/lease dispatch preflight 的真实 JWT 投影验收扩展到 Console Web、桌面
App 与 Mobile。Forge Core 的 opt-in acceptance mux 驱动共享 Sessions Gate；每个可见实例在
一次 preflight POST 前重新读取 owner-bound client-instance session/resource pair 与
inventory/resource image。隐藏 Web 实例仍会重新读取 pair，但在本地 fail-closed，不发送 inventory
或 preflight POST。Conversation 子路径只在 test mux 挂载，production pair、inventory 与
preflight constructor 继续 404/default-off；该切片不增加 enrollment/heartbeat、inventory
mutation、target selection、reservation、lease mutation、Runner transport/execution、receipt、
Audit 或 live P4 authority，ADR-0039、ADR-0114 与 P4 继续 gated。

§684 将同一 inventory/resource 观察边界接入 Forge Runtime TUI 的 Run/Attempt/lease
dispatch preflight。TUI 在 candidate POST 前检查已显式打开的 owner-bound inventory-v2 与
client-instance/resource pair；owner、device、lifecycle、capacity、GPU 或 observation-time
漂移均 fail-closed，保持零 POST，既有 selected client-instance pair guard 仍先执行。该切片
只是本地 display/read boundary，不增加 enrollment/heartbeat、inventory mutation、target
selection、reservation、lease mutation、Runner transport/execution、receipt、Audit 或 P4
authority，ADR-0039、ADR-0114 与 P4 继续 gated。

§685 将相同的 selected client-instance 边界接入 Forge Runtime CLI 的
`remote run-attempt-lease-dispatch-preflight-preview`。命令接受 `--instance INSTANCE_ID`
及可选 `--instance-view FILE|-`；在线模式在 metadata-only candidate POST 前读取并收敛
owner-bound session/resource pair，隐藏 Conversation 直接 fail-closed 且不发送 POST；本地
view 模式只做本地 display filter，未指定 `--instance` 的旧路径保持兼容。该切片不增加
inventory mutation、target selection、reservation、lease mutation、Runner transport/execution、
receipt、Audit 或 P4 authority，ADR-0039、ADR-0114 与 P4 继续 gated。

§686 将该 CLI 边界接入 Forge Core 的真实 Snaplink JWT opt-in E2E。可见
`client-cli-001` 在一次 metadata-only Run/Attempt/lease preflight POST 前读取 owner-bound
session/resource pair；隐藏 Conversation 会重新读取 pair 但保持零 candidate POST，普通
production pair 与 preflight constructor 继续 `404`。该验收不增加 inventory mutation、target
selection、reservation、lease mutation、Runner transport/execution、receipt、Audit 或 P4
authority，ADR-0039、ADR-0114 与 P4 继续 gated。

## 690. Runtime opt-in owner change SSE consumer (2026-09-24)

Forge Runtime CLI/TUI now consume the accepted Core owner-scoped Conversation
change stream only when the caller explicitly selects `remote changes stream` or
TUI `changes stream`. The client bounds the server wait to 10 seconds, requires
`text/event-stream`, parses one strict `conversation_changes` event, and binds
its id to the validated dense page. Empty `204` responses preserve the current
checkpoint; valid advancing pages update the saved cursor only when no explicit
cursor was supplied. Default list/watch polling remains unchanged.

This slice is transport-only. It does not register or heartbeat devices, mutate
inventory, select or reserve capacity, acquire leases, schedule or dispatch
work, contact a Runner, create a Run, persist a receipt, or publish Audit. ADR-
0114 remains Proposed/null and P4 still requires separate accepted execution and
security governance.

## 691. Console Sessions opt-in owner change SSE consumer (2026-09-24)

Snaplink Console's shared `ForgeSessionsScreen` can now opt into one bounded
owner change-stream request at a time after its initial snapshot. A successful
`204` reconnects without advancing the cursor. Transport, authorization-refresh,
framing, and cursor failures disable the stream for that screen instance and
return to the existing adaptive JSON polling path. Conversation merge,
required history refresh, and owner-local cursor persistence remain the same
success gate; lifecycle pause, sign-out, disposal, and configuration changes
cancel pending reconnects. The default Gate and callers leave the flag disabled.

This is read-only delivery. It adds no Prompt/Run mutation, device enrollment or
heartbeat, inventory authority, target selection, reservation, scheduling,
Runner transport/execution, receipt, or Audit. ADR-0114 remains Proposed/null
and P4 requires separate accepted execution/security governance.

## 692. Runtime TUI change-feed instance freshness gate (2026-09-24)

An explicitly selected Runtime TUI instance now refreshes its owner-bound
session/resource observations before `changes watch` or `changes stream` sends
the owner change-feed request. A failed or non-converged pair, or a
Conversation revoked from the previously selected instance, stops before
transport and preserves the local cursor. The unfiltered path remains
compatible. This is a read/display freshness guard only; no enrollment,
heartbeat, inventory mutation, scheduling, reservation, lease, Runner,
receipt, Audit, or P4 authority is enabled.

## 693. Real JWT Runtime CLI/TUI Conversation SSE E2E (2026-09-24)

Forge Core's opt-in integration test now drives both Runtime CLI and
line-oriented TUI through a real Snaplink JWT and the owner-scoped
`conversation-changes/stream` route. It verifies the SSE media type, dense
cursor/page delivery, normal TUI startup sync, and absence of POST or device/
execution requests. This is read-only P2 evidence; no device, scheduling,
Runner, receipt, Audit, or P4 authority is enabled.

## 694. Real JWT Flutter Console Conversation SSE E2E (2026-09-24)

Forge Core's opt-in integration now launches the shared Flutter Console API
test with a real Snaplink JWT against the owner-scoped
`/api/v1/conversation-changes/stream` route. It verifies the authenticated
SSE media type, Bearer header, owner-bound dense page/cursor, and exactly one
read-only GET. The existing Sessions UI stream remains explicitly opt-in and
polling remains the default for Web/App/Mobile callers.

This is transport evidence only. It adds no Prompt/Run mutation, device
registration or heartbeat, inventory authority, target selection, reservation,
scheduling, dispatch, Runner transport/execution, receipt, Audit, or P4
authority. ADR-0114 remains Proposed/null and P4 still requires separate
accepted execution/security governance.

## 695. Console Sessions Gate SSE projection (2026-09-24)

The shared Snaplink Console Gate now exposes explicit
`enableConversationChangesStream` and `conversationChangesStreamWaitMS` options
and forwards them to the Sessions screen. Defaults remain `false` and `15000`,
so existing Web/App/Mobile construction continues using JSON polling. Focused
Gate coverage proves both default request silence and explicit stream fallback.

This remains read-only delivery. It adds no Prompt/Run mutation, device
registration or heartbeat, inventory authority, target selection, reservation,
scheduling, dispatch, Runner transport/execution, receipt, Audit, or P4
authority. ADR-0114 remains Proposed/null.

## 696. Real JWT Chromium Conversation SSE E2E (2026-09-24)

An opt-in Forge Core/Web harness serves the built Flutter Web app through a
same-origin test wrapper, opens a real Chromium tab, seeds a real Snaplink JWT,
and fetches `/api/v1/conversation-changes/stream`. It validates the SSE media
type, Bearer header, strict event/cursor page, owner binding, and zero POSTs;
the ordinary production constructor remains fail-closed.

This is Web transport evidence only. No device, inventory, enrollment,
heartbeat, scheduling, Runner, receipt, Audit, or P4 authority is enabled.
ADR-0114 remains Proposed/null and P4 requires separate accepted execution and
security governance.

## 697. Console selected-instance revocation closes the change feed (2026-09-24)

The shared Sessions screen now requires the latest selected client-instance
observation to contain the selected instance before consuming either the SSE or
JSON owner change feed. Removal or revocation fails closed before transport and
keeps the in-memory cursor unchanged; the owner-local cursor is published only
after its persistent store accepts the new value. Focused coverage proves zero
feed requests after the selected row disappears.

This is display/read fencing only. No Prompt/Run mutation, device enrollment or
heartbeat, inventory authority, target selection, reservation, scheduling,
Runner transport/execution, receipt, Audit, or P4 authority was added.
ADR-0114 remains Proposed/null.

## 698. Native mobile Conversation SSE evidence (2026-09-24)

The opt-in Android-host/iOS-host shared-session lifecycle now restores the
owner-local cursor through the native credential path and consumes one
authenticated `/conversation-changes/stream` page on the second cold start.
The host harness binds the SSE event, cursor, owner, and Prompt change while
retaining exactly the existing storage-only Prompt writes. The normal mobile
Sessions Gate remains polling/default-off.

This is read-only delivery evidence. It adds no device enrollment or
heartbeat, inventory mutation, target selection, reservation, scheduling,
Runner transport/execution, receipt, Audit, or P4 authority. ADR-0114 remains
Proposed/null.

## 699. Mobile inventory-v2 observation chain (2026-09-24)

The opt-in Android-host/iOS-host cold-start journey now reads the authenticated
lossless `/api/v1/devices/observations/v2` candidate on both native starts.
Flutter and the Go harness bind the verified owner, `runner-a/device-a`
revision/generation/heartbeat tuple, reservation and multi-GPU values, and
all-false authority. The exact request allowlist excludes enrollment or
heartbeat writes, target selection, reservation, scheduling, Runner,
receipt, and Audit effects; the production inventory route remains
default-off/404.

This is read-only inventory observation evidence. ADR-0114 remains
Proposed/null and no P3b enrollment or P4 execution authority is enabled.

## 700. Inventory/resource convergence revalidates manually constructed v2 observations (2026-09-24)

Snaplink Console now round-trips locally constructed inventory-v2 and
client-instance/resource observations through their strict decoders before
treating them as a converged display boundary. Envelope, owner, capacity,
lifecycle, GPU, revision/generation/heartbeat, and all-false authority drift
fails closed; regression coverage proves a tampered inventory envelope cannot
be used as a freshness proof.

This is a local read/display guard. It adds no inventory mutation,
enrollment/heartbeat, target selection, reservation, scheduling, Runner,
receipt, Audit, or P4 authority. ADR-0114 remains Proposed/null.

## 701. Client-instance session/resource convergence revalidates nested observations (2026-09-24)

The Console Gate's injected client-instance session/resource pair now rechecks
the nested display envelopes and complete instance image before exposing a
converged local filter. A hand-built outer `converged/read_only` envelope with
schema, owner, row, or authority drift fails closed; malformed nested rows
cannot bypass the pair boundary.

This is local read/display fencing only. No Prompt/Run mutation, inventory
mutation, enrollment/heartbeat, target selection, reservation, scheduling,
Runner, receipt, Audit, or P4 authority was added. ADR-0114 remains
Proposed/null.

## 702. Runtime convergence joins revalidate nested observations (2026-09-24)

Forge Runtime CLI/TUI client-instance and inventory/resource join helpers now
rerun the strict source validators at the join boundary before comparing owner,
instance, lifecycle, capacity, and GPU fields. Authority-bearing or manually
assembled nested observations fail closed even when shared resource fields
match; focused unit coverage proves both joins reject nested authority
mutation.

This is a read-only metadata boundary. It adds no registration, heartbeat,
inventory mutation, target selection, reservation, scheduling, Runner
transport/execution, receipt, Audit, or P4 authority. ADR-0114 remains
Proposed/null.

## 703. Mobile planning-only scheduler-preview handoff (2026-09-24)

The opt-in Android-host/iOS-host cold-start journey now consumes one
authenticated `scheduler-preview` POST after the client-instance
session/resource pair and lossless inventory-v2 observation have converged. A
separate test-only scheduler image is fresh and unreserved, selects
`device-a`/`runner-a`, and returns `preview_only=true` with every authority
predicate false. The display inventory remains reserved, making the separation
between observation and planning explicit.

This remains planning evidence only. The allowlist contains no enrollment,
heartbeat, reservation, lease, dispatch, Runner, or execution operation; the
production scheduler route remains default-off/404. ADR-0114 remains
Proposed/null and P4 requires separate accepted execution and security
governance.

## 704. Lifecycle candidate routes remain closed on ordinary Coordinator (2026-09-24)

The ordinary authenticated Conversation/Coordinator constructor now has exact
404 regression coverage for lifecycle registry, heartbeat, approval, and
credential candidate paths, including requests carrying the lifecycle scopes.
The live enrollment, heartbeat, approval, credential, inventory mutation,
Runner, and execution seams remain unregistered until their own activation
decision; ADR-0114 remains Proposed/null.

## 705. Real JWT Runtime CLI scheduler-preview pair convergence (2026-09-24)

The opt-in Forge Core E2E now drives the Runtime CLI with a real Snaplink JWT
and an explicit `client-web` instance filter. The visible path reads the
owner-bound client-instance session and resource views before one
planning-only scheduler-preview POST, then validates owner,
Conversation/Run/Attempt, selected `device-a`/`runner-a`, `preview_only`, and
all-false authority. A hidden `client-tui` path performs only the two pair
reads and fails closed without a POST; an under-scoped token remains 403 and
the ordinary production constructor remains 404.

This is authenticated planning evidence only. It creates no reservation,
lease, dispatch, Runner, enrollment, heartbeat, receipt, Audit, or live P4
authority. ADR-0114 remains Proposed/null.

## 706. Real JWT Runtime CLI scheduler-preview inventory/resource convergence (2026-09-24)

The opt-in Forge Core E2E now drives the Runtime CLI's explicit
`--instance` scheduler-preview path through the owner-bound inventory-v2 and
resource-view convergence reader after the client-instance session/resource
pair. A visible instance therefore reads
`session-view → resource-view → inventory-v2 → resource-view` before the
planning-only scheduler-preview POST; owner, device/Runner, revision,
generation, heartbeat, capacity, GPU, lifecycle, and observation-time drift
fail closed before the POST. Hidden instances stop after the pair scope and
local `--instance-view` remains offline.

This is read/planning evidence only. It adds no reservation, lease, dispatch,
Runner, enrollment, heartbeat, receipt, Audit, or live P4 authority; ADR-0114
remains Proposed/null and P4 still requires separately accepted execution and
security governance.

## 707. Runtime CLI Prompt-write inventory/resource guard (2026-09-24)

Online Runtime CLI `remote prompts add/receipt --instance` now performs the
owner-bound client-instance session/resource pair and then the strict
inventory-v2/resource-view convergence read before sending the Prompt POST.
Hidden instances fail before inventory or Prompt transport; owner,
device/Runner, revision/generation/heartbeat, capacity, GPU, lifecycle, and
observation-time drift fail closed. Prompt history reads, unfiltered writes,
and local `--instance-view` remain compatible, with the local view staying
offline.

This is a read freshness guard around the existing storage-only Prompt write.
It adds no Run, target selection, reservation, lease, dispatch, Runner,
enrollment, heartbeat, receipt, Audit, or live P4 authority. ADR-0114 remains
Proposed/null and P4 still requires separately accepted execution and security
governance.

## 708. Runtime TUI Prompt-write inventory/resource freshness (2026-09-24)

An explicitly selected Runtime TUI instance now refreshes the already-open
owner-bound inventory-v2/resource pair immediately before a Prompt append or
retry. The pair is committed atomically only after strict convergence; drift,
malformed envelopes, or authorization failure retain the pending write and
stop before the Prompt POST. Missing or one-sided opt-in observations keep the
existing request-free compatibility path.

This is a read freshness guard around the existing storage-only Prompt write.
It adds no Run, target selection, reservation, lease, dispatch, Runner,
enrollment, heartbeat, receipt, Audit, or live P4 authority. ADR-0114 remains
Proposed/null and P4 requires separate accepted execution and security
governance.

## 709. Runtime TUI scheduler-preview inventory/resource freshness (2026-09-24)

When an explicitly selected Runtime TUI instance has already opened both
owner-bound inventory-v2 and client-instance/resource observations, its
planning-only `scheduler-selection-preview` refreshes that strict pair before
POST. The visible real-JWT E2E orders the client-instance pair, the explicit
inventory/resource open, the fresh inventory/resource refresh, and then one
preview POST; a hidden instance stops after its client-instance pair with no
inventory read or POST. Missing or one-sided observations remain request-free.

This remains a read/planning freshness boundary. It adds no reservation, lease,
dispatch, Runner execution, enrollment, heartbeat, receipt, Audit, or live P4
authority. ADR-0114 remains Proposed/null and P4 requires separate accepted
execution and security governance.

## 710. Real JWT Console Prompt inventory/resource freshness (2026-09-24)

The opt-in shared Console Web/App/Mobile Prompt acceptance now injects the
existing owner-bound inventory/resource convergence candidate. Each visible
client refreshes its client-instance pair and requires an inventory-v2/resource
convergence read before the storage-only Prompt POST; the recorder rejects any
hidden Prompt POST. Default Gate construction and ordinary production candidate
routes remain closed.

This is only a freshness check around Prompt storage. It adds no Run,
reservation, lease, dispatch, Runner execution, enrollment, heartbeat, receipt,
Audit, or live P4 authority. ADR-0114 remains Proposed/null and P4 requires
separate accepted execution and security governance.

## 711. Console Prompt writes force a fresh inventory/resource proof (2026-09-24)

When the opt-in Console Web/App/Mobile Prompt path has a composed
inventory/resource reader, it now refreshes that owner-bound pair immediately
before invoking the storage-only Prompt submitter. A newly drifted resource
image therefore clears the write boundary and produces zero Prompt POSTs; the
existing default Gate and one-sided compatibility paths remain unchanged.

This is a read freshness guard around Prompt storage. It adds no Run, target
selection, reservation, lease, dispatch, Runner execution, enrollment,
heartbeat, receipt, Audit, or live P4 authority. ADR-0114 remains Proposed/null
and P4 requires separate accepted execution and security governance.

## 712. Console scheduler-preview forces a fresh inventory/resource proof (2026-09-24)

The opt-in Console Web/App/Mobile planning-only scheduler-selection-preview
path now forces an owner-bound inventory/resource reread immediately before its
candidate POST, even when an earlier display snapshot is healthy. If the forced
resource image changes its revision or otherwise fails convergence, the preview
is rejected before POST; focused Flutter coverage proves both rereads and zero
preview requests on drift.

This remains a read/planning freshness check. It adds no reservation, lease,
dispatch, Runner execution, enrollment, heartbeat, receipt, Audit, or live P4
authority. ADR-0114 remains Proposed/null and P4 requires separate accepted
execution and security governance.

## 713. Real-JWT Console scheduler-preview reads inventory/resource candidates (2026-09-24)

The accepted test-only Console Web/App/Mobile scheduler Gate now opens the
authenticated owner-bound inventory-v2 and client-instance resource candidates
before displaying the planning-only scheduler preview. A missing, unauthorized,
or non-convergent observation prevents the selection panel from being shown;
the ordinary production constructor remains default-off.

This is cross-client observation evidence only. It adds no enrollment,
heartbeat, reservation, lease, dispatch, Runner execution, receipt, Audit, or
live P4 authority. ADR-0114 remains Proposed/null and P4 requires separate
accepted execution and security governance.

## 714. CLI pending Run-intent writes refresh inventory/resource (2026-09-24)

An online CLI pending Run-intent submission scoped with `--instance` now reads
the strict owner-bound inventory-v2/resource-view pair after the client-instance
session/resource projection and before the candidate POST. A resource revision,
heartbeat, or capability drift rejects the submission with zero pending-intent
POSTs; local `--instance-view` and unfiltered compatibility paths remain
request-compatible.

This is a candidate freshness guard around the inert Run-intent receipt. It
adds no ordinary Run, reservation, lease, dispatch, Runner execution,
enrollment, heartbeat, receipt, Audit, or live P4 authority. ADR-0114 remains
Proposed/null and P4 requires separate accepted execution and security
governance.

## 715. CLI scheduler lease claim/renew refresh inventory/resource (2026-09-24)

Online CLI scheduler lease claim and renewal scoped with `--instance` now read
the strict owner-bound inventory-v2/resource-view pair after the client-instance
projection and before the candidate POST. Inventory/resource read failure or
revision, heartbeat, capability, or observation drift rejects the candidate with
zero claim/renew POSTs; local `--instance-view`, unfiltered operations, and lease
release compatibility remain request-compatible.

This is a freshness guard around candidate lease metadata. It adds no live
reservation or lease authority, dispatch, Runner execution, enrollment,
heartbeat, receipt, Audit, or P4 authority. ADR-0114 remains Proposed/null and
P4 requires separate accepted execution and security governance.

## 716. TUI pending Run-intent refreshes inventory/resource (2026-09-24)

An explicitly selected Runtime TUI instance with both owner-bound inventory-v2
and client-instance/resource observations open now refreshes that pair
immediately before a pending Run-intent submit or retry. Drift, malformed
responses, or authorization failure retain the pending intent and stop before
the candidate POST; missing or one-sided observations retain the existing
request-free compatibility path.

This is a read freshness guard around the inert Run-intent receipt. It adds no
ordinary Run, reservation, lease, dispatch, Runner execution, enrollment,
heartbeat, receipt, Audit, or P4 authority. ADR-0114 remains Proposed/null and
P4 requires separate accepted execution and security governance.

## 717. TUI scheduler lease claim/renew refreshes inventory/resource (2026-09-24)

An explicitly selected Runtime TUI instance with both owner-bound inventory-v2
and client-instance/resource observations open now refreshes that pair
immediately before scheduler lease claim or renewal and rechecks Conversation
visibility after the refresh. Drift, malformed responses, or instance
revocation yields zero candidate POSTs; release, unfiltered, and one-sided
compatibility paths remain unchanged.

This is a read freshness guard around candidate lease metadata. It adds no live
reservation or lease authority, dispatch, Runner execution, enrollment,
heartbeat, receipt, Audit, or P4 authority. ADR-0114 remains Proposed/null and
P4 requires separate accepted execution and security governance.

## 718. Runner metadata previews refresh inventory/resource (2026-09-24)

Online Runtime CLI Runner execution-intent, dispatch-plan, and Run/Attempt/lease
preflight previews now read the owner-bound client-instance pair, then a
converged inventory-v2/resource pair, before a candidate POST; the refreshed
resource image must still contain the selected Conversation. Runtime TUI applies
the same refresh and visibility recheck to execution-intent, dispatch-plan, and
preflight previews. The shared Console Web/App/Mobile execution-intent Gate also
refreshes its selected client-instance and inventory/resource observations before
the candidate reader. Drift, malformed observations, or instance revocation
yield zero candidate requests; local/offline, unfiltered, one-sided, and default
request-free paths remain compatible.

This adds metadata freshness evidence only. It adds no enrollment/heartbeat
authority, inventory mutation, target selection, reservation, scheduler lease,
dispatch, Runner transport/execution, receipt persistence, Audit, or P4
authority. ADR-0114 remains Proposed/null and P4 requires separate accepted
execution and security governance.

## 719. Instance-scoped change-feed projections (2026-09-24)

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
transport/execution, receipt persistence, Audit, or P4 authority. ADR-0114
remains Proposed/null and P4 requires separate accepted execution and security
governance.

## 720. Candidate heartbeat-to-inventory/resource journey (2026-09-24)

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

## 721. Accepted fabric assembly rechecks review evidence (2026-09-24)

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

## 722. Accepted lifecycle mutation surfaces stay closed (2026-09-24)

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

## 723. Signed heartbeat candidate proof transport boundary (2026-09-24)

An injected Forge Core candidate mux now exposes a signed heartbeat transport
that verifies the persisted owner/device binding, Ed25519 signature, and
heartbeat digest bound to the server challenge before a lifecycle registry CAS
update. The candidate preserves other device states and returns preview-only,
all-false authority. Ordinary and accepted assemblies keep the signed write
route closed with `404`.

Authoritative challenge issuance/consumption, real credential enrollment, production
heartbeat, inventory authority, scheduling, Runner transport/execution,
receipt, Audit, and P4 authority remain outside this planning-only ADR.
ADR-0114 remains Proposed/null.

## 724. Runtime TUI keeps instance-hidden creates out of private projection (2026-09-24)

Runtime TUI Conversation creation now checks the returned owner session against
the converged local client-instance display declaration before selecting it or
retaining private Prompt/Run state. A newly created session absent from the
selected instance remains unselected and is removed by the filtered refresh;
invalid or divergent observations fail closed before the create request.

The owner-wide create API remains storage-only and this slice adds no
membership writer or instance authority, enrollment/heartbeat, inventory
mutation, reservation, scheduler lease, dispatch, Runner transport/execution,
receipt, Audit, or P4 authority. ADR-0114 remains Proposed/null.

## 725. Accepted non-EXECUTE execution evidence remains closed (2026-09-24)

Forge Core now covers the accepted route boundary for `INVENTORY` and
`OBSERVE`: Runner receipt, receipt-history, reconciliation, Attempt, scheduler,
dispatch, transport, and execution-boundary candidates remain `404` even when
an owner-scoped lifecycle image is configured. Those planning and evidence
surfaces require the separately reviewed EXECUTE/P4 assembly.

This is regression evidence only. It changes no route behavior, receipt
persistence, Runner transport, live execution authority, or ADR status.

## 726. Candidate challenge issuance and one-time signed heartbeat consumption (2026-09-24)

The injected lifecycle candidate now provides an owner-scoped
`challenge-candidate` POST. It uses the server observation clock, bounded TTL,
random challenge identifier, heartbeat digest, and lifecycle file CAS to keep
one active challenge per device. A signed heartbeat must match the persisted
unconsumed challenge and digest; a successful candidate CAS marks it consumed.
Generic lifecycle replacement rejects challenge fields so challenge issuance
cannot be bypassed by the broad candidate writer.

The earlier unsigned heartbeat candidate remains a separate injected fixture
seam for the pre-signed journey; it is not mounted by ordinary or accepted
assemblies and is not a production device-authentication path.

This remains a candidate-only planning boundary. Ordinary and accepted
`INVENTORY`/`OBSERVE` assemblies return `404`, the response is preview-only
with all authority flags false, and no credential enrollment, production
heartbeat, inventory mutation, scheduling, Runner, receipt, Audit, or P4
authority is enabled. ADR-0114 remains Proposed/null.

## 727. Challenge expiry, reissue, and CAS race boundaries (2026-09-24)

Forge Core now tests exact-expiry reissue, replacement after a challenge is
consumed and persisted, bounded TTL and `uint64` overflow rejection, and a
concurrent file-CAS race that permits exactly one active challenge. These are
candidate regression boundaries only; the challenge remains owner-scoped,
preview-only, and all-false, and production enrollment/heartbeat writes stay
closed.

## 728. Console instance-hidden Conversation creates remain private-read free (2026-09-24)

The shared Snaplink Console Web/App/Mobile Sessions screen refreshes the
selected owner-bound client-instance session/resource projection before its
owner-wide Conversation create. A returned Conversation absent from that
declaration is retained only as an owner storage result: it is not selected,
placed in the URL, or used for Prompt/Run hydration or private state. A
focused Flutter test proves one create POST and zero Prompt/Run reads. This
does not add an instance-membership writer, enrollment, heartbeat, or
execution authority; ADR-0114 remains Proposed/null.

## 729. Runtime CLI instance-scoped Conversation create preflight (2026-09-24)

Runtime CLI `remote sessions create` now accepts an optional client-instance
selector and strict local declaration. Online calls converge the owner-bound
session/resource observations before the one owner-wide Conversation POST;
unknown, malformed, or divergent declarations fail closed, and a local file
keeps the check offline. The create remains storage-only and does not assert
instance membership, select private Prompt/Run state, or grant enrollment,
inventory, scheduling, Runner, receipt, Audit, or P4 authority. ADR-0114 remains
Proposed/null.

## 730. Real JWT CLI instance-scoped Conversation create journey (2026-09-24)

The opt-in Forge Core five-client projection journey now drives the built
Runtime CLI through a real Snaplink JWT for
`remote sessions create --instance client-cli-001`. It records and validates
the owner-bound session-view and resource-view reads followed by exactly one
owner-wide Conversation POST, while retaining the existing Console and TUI
projection checks. This is authenticated storage/API evidence only; it does not
add instance membership, device or inventory mutation, scheduling, Runner
execution, receipt, Audit, or P4 authority. ADR-0114 remains Proposed/null.

## 731. CLI instance-create resource-scope rejection (2026-09-24)

The authenticated five-client projection journey also retries
`remote sessions create --instance` with only Conversation read/write scopes.
The session-view read succeeds, resource-view returns 403 for missing
`forge:devices:read`, and no owner-wide Conversation POST is sent. This is an
authorization regression boundary only; it adds no membership, inventory,
scheduling, Runner, receipt, Audit, or P4 authority. ADR-0114 remains
Proposed/null.

## 732. Runtime TUI instance-scoped create requires a converged resource pair (2026-09-24)

A selected Runtime TUI client-instance filter now blocks its owner-wide
Conversation create until both owner-bound session-view and resource-view
observations are present and converged. A one-sided or drifted pair remains
request-free, while ordinary owner-wide TUI create and the hidden-response
projection check remain compatible. This is a fail-closed display/write
boundary only; it adds no membership writer, enrollment/heartbeat, inventory
mutation, scheduling, Runner, receipt, Audit, or P4 authority. ADR-0114 remains
Proposed/null.

## 733. Runtime TUI instance-scoped Conversation create freshness guard (2026-09-24)

With an explicit client-instance filter active, Runtime TUI now refreshes and
strictly converges the owner-bound session/resource pair immediately before an
owner-wide Conversation create. Pair drift, malformed observations, instance
revocation, or authorization failure stops before the Conversation POST while
the pending storage write remains retryable. A created Conversation outside the
refreshed display declaration remains unselected. This does not add an instance
membership writer, enrollment/heartbeat, inventory mutation, scheduling,
Runner transport/execution, receipt, Audit, or P4 authority. ADR-0114 remains
Proposed/null.

## 734. Real JWT Runtime TUI instance-scoped Conversation create journey (2026-09-24)

The opt-in Forge Core five-client projection journey now drives the
interactive Runtime TUI with a real Snaplink JWT through
`client-instances show-converged`, an explicit `client-tui-001` filter, and an
instance-scoped `create`. The recorder proves startup session listing, initial
session/resource convergence, a second fresh pair immediately before one
owner-wide Conversation POST, and the post-create owner refresh. The returned
Conversation is absent from the fixture declaration and remains outside the
selected instance projection and unselected. This is authenticated
storage/API evidence only; no membership writer, enrollment/heartbeat,
inventory mutation, scheduling, Runner, receipt, Audit, or P4 authority was
added. ADR-0114 remains Proposed/null.

## 735. Real JWT Runtime TUI instance-scoped Prompt freshness (2026-09-24)

The opt-in Forge Core projection journey now drives the authenticated Runtime
TUI through converged client-instance and inventory/resource observations
before opening a declared Conversation. TUI refreshes the owner-bound
inventory/resource pair immediately before the storage-only Prompt POST; the
HTTP recorder proves the exact read ordering, one visible Prompt POST, and no
device, scheduler, Runner, or execution side request. This is authenticated
storage/API evidence only; no membership writer, enrollment/heartbeat,
inventory mutation, scheduling, Runner, receipt, Audit, or P4 authority was
added. ADR-0114 remains Proposed/null.

## 736. Real JWT Runtime TUI instance-scoped change stream (2026-09-24)

The opt-in Forge Core journey now drives Runtime TUI
`changes stream --instance` through a real Snaplink JWT after converged
client-instance reads. The recorder proves a fresh session/resource pair
immediately before the owner-scoped SSE GET; hidden Conversation changes advance
the owner cursor without entering the selected session projection, while the
visible row is applied exactly once. No Prompt, device, scheduler, Runner, or
execution request is emitted. This is read-only projection evidence; no
membership writer, enrollment/heartbeat, inventory mutation, scheduling,
Runner, receipt, Audit, or P4 authority was added. ADR-0114 remains
Proposed/null.


## 737. Real JWT Runtime CLI instance-scoped change stream (2026-09-24)

The opt-in Forge Core journey now drives Runtime CLI
`remote changes stream --instance` through a real Snaplink JWT after a fresh
owner-bound session/resource pair. The JSON response contains only the selected
Conversation change while `scanned_through_cursor` advances across the hidden
row; the recorder proves the exact read order and no Prompt, device, scheduler,
Runner, or execution request. This is read-only projection evidence; no
membership writer, enrollment/heartbeat, inventory mutation, scheduling,
Runner, receipt, Audit, or P4 authority was added. ADR-0114 remains
Proposed/null.

## 738. Real JWT Runtime CLI instance-filtered Runner execution-intent convergence (2026-09-24)

The opt-in Forge Core journey now drives the real Runtime CLI with an explicit
client-instance filter through the owner-bound session/resource pair and a
fresh inventory-v2/resource pair before one metadata-only Runner
execution-intent candidate POST. A hidden Conversation stops after the first
pair reads; selected targets remain null, all authority predicates remain
false, and fencing material, workspace, and argv stay out of the response.
The adjacent dispatch-plan CLI/TUI journeys assert the same fresh
inventory/resource ordering. This is authenticated planning evidence only;
the ordinary production constructors remain `404`, and no inventory mutation,
reservation, lease mutation, Runner transport/execution, receipt, Audit, or
P4 authority was added. ADR-0114 remains Proposed/null.

## 739. Real JWT Console Web/App/Mobile execution-intent Gate convergence (2026-09-24)

The opt-in Forge Core journey now drives the shared Snaplink Console Gate with
one real JWT across Web, desktop App, and Mobile. Each visible client refreshes
the owner-bound client-instance session/resource pair, reads the lossless
inventory-v2 image, refreshes the resource image again, and posts exactly one
metadata-only Runner execution-intent candidate. Hidden and resource-drifted
instances remain candidate-free; the ordinary production constructors stay
`404`. This is authenticated planning evidence only: no inventory mutation,
target selection, reservation, lease mutation, Runner transport/execution,
receipt, Audit, or P4 authority was added. ADR-0114 remains Proposed/null.

## 740. Console scheduler-preview target/resource binding (2026-09-24)

The shared Console scheduler-preview projection now rejects fetched or static
candidates whose device or Runner instance is absent from the current
owner-bound resource observation before display. This remains display-only
binding hygiene and adds no reservation, scheduler lease, dispatch, Runner
transport/execution, receipt, Audit, or P4 authority. ADR-0114 remains
Proposed/null.

## 741. Real JWT Runtime TUI execution-intent convergence (2026-09-24)

The opt-in Forge Core journey now drives the real Runtime TUI through a
Snaplink JWT, an explicit `client-tui-001` filter, and the planning-only Runner
execution-intent candidate. The visible TUI refreshes the owner-bound
session/resource pair, refreshes the lossless inventory-v2/resource pair, and
posts one metadata-only intent; a hidden `client-web-001` stops before POST.
The ordinary production constructor remains `404`. This is authenticated
planning evidence only: no target selection, reservation, lease mutation,
Runner transport/execution, receipt, Audit, or P4 authority was added.
ADR-0114 remains Proposed/null.

## 742. Real JWT Console Web/App/Mobile scheduler-preview target/resource Gate (2026-09-24)

The opt-in Forge Core journey now drives the shared Console Web/App/Mobile
Sessions Gate with one real Snaplink JWT. Each visible selected client instance
refreshes the owner-bound session/resource pair and inventory/resource image
before one planning-only scheduler-preview POST. The returned device and
Runner instance must be present in the same resource image before the Gate
renders the deterministic target; authority remains all false and no
lease/fencing token is shown. Hidden membership and resource/session drift
remain request-free, while ordinary production constructors remain `404`.
No reservation, lease, dispatch, Runner transport/execution, receipt, Audit, or
P4 authority was added; ADR-0114 remains Proposed/null.

## 743. Console Runner dispatch-plan target/resource binding (2026-09-24)

The shared Console Web/App/Mobile Sessions projection now revalidates every
Runner dispatch-plan candidate target, including the intent target, against the
current owner-bound resource image before display. Canonical device IDs and
adapter-exposed Runner instance IDs are treated as identities of the same
observed resource row; owner drift and foreign targets fail closed. The real
JWT Gate journey covers visible, hidden, session/resource drift, and target
resource drift while the no-resource-reader compatibility path remains
request-compatible. This remains planning-only: no target selection,
reservation, lease, Runner transport/execution, receipt, Audit, or P4 authority
was added; ADR-0114 remains Proposed/null.

## 744. Runtime TUI local Runner preview resource/target guard (2026-09-24)

An explicitly selected Runtime TUI client instance now requires an owner-bound
converged inventory/resource pair before the injected local Runner
execution-readiness preview. The pair is refreshed immediately before the
candidate POST, Conversation visibility is rechecked, and the request target
must match either the observed device ID or Runner instance ID. Hidden sessions,
missing or drifted pairs, and foreign targets remain zero-POST. Focused Rust
coverage exercises these fail-closed paths; production routes, local executor
authority, receipts, reservation, Runner transport, Audit, and P4 authority
remain closed, and ADR-0114 remains Proposed/null.

## 745. Runtime TUI dispatch-plan target/resource guard (2026-09-24)

An explicitly selected Runtime TUI client instance now requires the converged
inventory-v2/resource pair before the planning-only Runner dispatch-plan
candidate. After the fresh pair and Conversation visibility checks, the lease
target, intent target, and every placement device ID must match an observed
device ID or Runner instance ID. Missing pairs and foreign targets produce no
candidate POST; unfiltered behavior remains compatible. This is a local
display and candidate boundary only: no reservation, lease mutation, dispatch,
Runner transport/execution, receipt, Audit, or P4 authority was added. ADR-0039
remains planning-only and ADR-0114 remains Proposed/null.

## 746. Runtime TUI dispatch-admission target/resource guard (2026-09-24)

The selected-instance Runtime TUI dispatch-admission preview now requires the
converged inventory-v2/resource pair, refreshes it immediately before POST,
rechecks Conversation visibility, and rejects a lease-proof target absent from
the owner-bound device/Runner resource image. Missing pairs and foreign targets
produce no candidate POST; unfiltered behavior remains compatible. This is a
local candidate boundary only: no reservation, lease mutation, dispatch,
Runner transport/execution, receipt, Audit, or P4 authority was added. ADR-0039
remains planning-only and ADR-0114 remains Proposed/null.

## 747. Runtime TUI transport-admission target/resource guard (2026-09-24)

The selected-instance Runtime TUI transport-admission preview now applies the
same converged inventory-v2/resource refresh and Conversation visibility fence,
then requires the command lease-proof target to exist in the owner-bound
device/Runner resource image. Missing pairs and foreign targets produce no
candidate POST; unfiltered behavior remains compatible. This is a local
candidate boundary only: no Runner connection, payload transport, reservation,
lease mutation, execution, receipt, Audit, or P4 authority was added. ADR-0039
remains planning-only and ADR-0114 remains Proposed/null.

## 748. Console Runner admission target/resource gate (2026-09-24)

The shared Console Web/App/Mobile Sessions projection now refreshes the
selected client-instance's owner-bound inventory/resource observation before a
Runner dispatch-admission or transport-admission candidate. Configured empty
or drifted resource images, owner mismatch, and targets absent from either the
observed device ID or Runner instance ID remain zero-POST and hidden; static
and fetched candidates use the same gate, while the no-resource-reader
compatibility path remains unchanged. This is a display and candidate boundary
only: no Runner transport, payload execution, enrollment, reservation, Audit,
or P4 authority was added. ADR-0039 remains planning-only and ADR-0114
remains Proposed/null.

## 749. Native Android/iOS admission evidence boundary (2026-09-24)

The Console native contract harness validates metadata-only dispatch and
transport admission candidates against the owner-bound resource image for both
Android and iOS evidence. A candidate is accepted only when its owner matches
the read-only resource owner, its target names an observed device or Runner
instance, `preview_only` and admission bindings are true, and every authority
flag remains false; foreign targets and owner/authority drift fail closed. The
native trace allowlist contains only client-instance/resource observations and
the two admission preview endpoints, rejecting direct Runner dispatch and
payload transport. The harness runs without ADB, xcodebuild, HTTP, or a Runner
connection; platform journeys remain opt-in and default-off. No reservation,
execution, receipt, Audit, or P4 authority was added. ADR-0039 remains
planning-only and ADR-0114 remains Proposed/null.

## 750. Runtime CLI Runner admission target/resource gate (2026-09-24)

Non-interactive Runtime CLI dispatch-admission and transport-admission previews
now accept an explicit `--instance` projection only after refreshing the
owner-bound client-instance session/resource pair and the inventory-v2/resource
pair. The lease-proof target must match an observed device ID or Runner
instance ID; missing, malformed, foreign, or drifted observations remain
zero-POST. Unfiltered input remains compatible, and local `--instance-view`
accepts only a validated resource or converged fixture so the target binding
is observable. This remains a metadata-only candidate boundary: no
Runner transport, payload execution, reservation, lease mutation, receipt,
Audit, or P4 authority was added. ADR-0039 remains planning-only and ADR-0114
remains Proposed/null.

## 751. Real JWT Runtime CLI Runner admission convergence (2026-09-24)

An opt-in Forge Core E2E now drives both Runtime CLI dispatch-admission and
transport-admission previews with a real Snaplink JWT and explicit
`--instance client-cli-001`. Each visible command proves
`session-view → resource-view → inventory-v2 → resource-view → one admission
POST`; after the resource image changes to a foreign target, the same commands
stop after the four read-only observations with zero admission POSTs. The
candidate uses a persisted lease and owned Run reference but never contacts a
Runner; the normal production constructor remains `404`. No reservation,
execution, receipt, Audit, or P4 authority was added. ADR-0039 remains
planning-only and ADR-0114 remains Proposed/null.

## 752. Runtime CLI/TUI execution-boundary target/resource gate (2026-09-24)

Runtime CLI and TUI Runner execution-boundary previews now keep a selected
client instance behind the owner-bound session/resource pair and the refreshed
inventory-v2/resource pair before the metadata-only candidate. The lease-proof
target must match an observed device ID or Runner instance ID; foreign targets,
missing observations, and one-sided TUI projections stop with zero POST. CLI
unfiltered input remains compatible and local `--instance-view` accepts only a
validated resource or converged fixture. No Runner transport, payload
execution, reservation, lease mutation, receipt, Audit, or P4 authority was
added. ADR-0039 remains planning-only and ADR-0114 remains Proposed/null.

## 753. Runtime TUI execution-boundary target/resource guard (2026-09-24)

The selected Runtime TUI execution-boundary preview requires the opened
owner-bound session/resource declaration and a refreshed, converged
inventory-v2/resource pair before its metadata-only candidate. Conversation
visibility is rechecked after refresh, and the target must match an observed
device ID or Runner instance ID. Missing, one-sided, drifted, or foreign
observations remain zero-POST; unfiltered behavior stays compatible. No Runner
transport, execution, reservation, lease mutation, receipt, Audit, or P4
authority was added. ADR-0039 remains planning-only and ADR-0114 remains
Proposed/null.

## 754. Android shared-session origin and resource boundary (2026-09-24)

The opt-in Android shared-session coordinator validates the owner-bound selected
client-instance session/resource pair and allows only HTTPS or loopback HTTP
before bearer input enters the explicit emulator path. Public HTTP, paths,
credentials, query/fragment, invalid ports, and resource drift fail before ADB.
Host-only tests remain request-free. No enrollment, heartbeat, inventory
mutation, scheduling, Runner, execution, receipt, Audit, or P4 authority was
added. ADR-0039 remains planning-only and ADR-0114 remains Proposed/null.

## 755. Console Web/App/Mobile execution-boundary target/resource gate (2026-09-24)

The shared Console Sessions surface refreshes the selected client-instance
session/resource observation immediately before the metadata-only Runner
execution-boundary candidate. The target must match an owner-bound device ID or
Runner instance ID from that resource image; foreign targets, selected mobile
resource drift, and revoked sessions remain zero-POST and hidden. Paired
session/resource observations bind the target, while the default candidate stays
disabled and the no-resource-reader compatibility path remains unchanged. No
Runner transport, payload execution, reservation, lease mutation, receipt,
Audit, or P4 authority was added. ADR-0039 remains planning-only and ADR-0114
remains Proposed/null.

## 756. Real JWT Console execution-boundary convergence (2026-09-24)

The accepted Forge Core EXECUTE harness mounts the owner-scoped client-instance
session/resource projection for Web, desktop App, and Mobile and drives a
dedicated Flutter E2E with the real Snaplink JWT. Each selected instance must
observe the Conversation and the owner-bound device or Runner instance target
before one metadata-only execution-boundary POST; the response remains
display-only with all authority false. No Runner transport, payload execution,
reservation, lease mutation, receipt, Audit, or P4 authority was added.
ADR-0039 remains planning-only and ADR-0114 remains Proposed/null.

## 757. Console Web/App/Mobile Attempt-boundary target/resource gate (2026-09-24)

The shared Console Sessions surface validates the selected Runner Attempt
target against freshly refreshed owner-bound client-instance session/resource
and inventory/resource images before invoking the metadata-only
`runner-attempt-boundary/preview` candidate. Device IDs and Runner instance IDs
are accepted only from the same owner resource image; foreign targets stop with
zero POST, and the returned target is checked again against the current image.
This remains a display-only boundary: no Attempt persistence, reservation,
Runner transport, execution, receipt, Audit, or P4 authority was added.
ADR-0039 remains planning-only and ADR-0114 remains Proposed/null.

\n