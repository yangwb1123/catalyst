# Policy and Authority

## 职责与触发

当任务需要构造、检查或比较 caller-supplied `CapabilityGrant v1` envelope 及其 declared assessment 时，使用本 Skill。ADR-0056
只交付 contract-only wire、effect/scope vocabulary 与 authority-neutral declared-relation evaluator；它不认证 issuer、key、proof、principal、policy、
ApprovalRecord 或 digest preimage，也不签发、批准、激活、撤销、预留、消费或执行 Grant。

节点将产生 read/write/process/network/migration/release 等 effect，但除 ADR-0057 的单一 authenticated bootstrap repo-read 签发 profile 与
ADR-0058 的单一 exact-byte repo-read 执行 profile 外，Governance Kernel/PDP、effective Approval、revocation 与 durable usage ledger 仍不可用；不符合任一精确
profile 时停止并报告 authority unavailable。不得用
structurally valid envelope、自述 role、workflow、Agent 输出或本 Skill 冒充 permission。

## 输入契约

- exact compact canonical `forgeos.capability-grant/v1` envelope；
- exact `forgeos.capability-grant-declared-assessment-request/v1`，包含显式 `evaluated_at_unix_ms`、expected binding 与 `requested_action` candidate；
- 单一 effect 必须来自下列按 UTF-8 字节序冻结的 21 项 closed vocabulary：`approval.decide`、`approval.request`、`knowledge.apply`、
  `knowledge.propose`、`migration.apply`、`migration.generate`、`network.read`、`network.write`、`placement.plan`、`policy.propose`、
  `policy.write`、`process.exec`、`release.execute`、`release.plan`、`repo.read`、`repo.write`、`secrets.read`、`target.execute`、
  `target.inventory`、`target.probe`、`target.reserve`；
- caller-declared subject、task/run/change/node/capability、effect、allow/deny scope、budget、source/context/policy/GrantRequest digest、nullable
  impact/plan/risk digest、validity、
  approval refs、authority-proof placeholder、separation-of-duty 与 usage-policy fields；
- instance validation 时提供内含原始 canonical Grant 的 request 与 expected assessment 两个文件，不从 ambient repository、clock、identity provider 或 environment 补值。

缺少 exact canonical bytes、必需 binding、deny/allow ordering、proof placeholder、SoD、budget 或时间字段时失败关闭。Opaque digest/ref 只证明字符串形状，
不证明 preimage、签名、批准、风险、source、context、policy 或 capability registry entry 存在。

## 执行 SOP

1. 严格解码后逐字节 canonical 重编码；拒绝 duplicate/unknown field、float、非 int64、非 canonical JSON、控制字符和资源越界；raw input、
   programmatic object 与 self-digest preimage 分别校验时，完整 canonical 文档本身也必须满足对应 byte ceiling，不能只用清空 self-digest 字段后的 preimage 代替。
2. 验证 canonical kind 只能是 `CapabilityGrant`；拒绝 `AuthorityGrant`、`AgentCapabilityGrant` 及其它 alias。
3. 校验 closed effect vocabulary、effect 与 tagged scope-kind 对应、`scope.allow` 的 OR resource groups（资源不可跨 clause 拼接）、flat typed
   `scope.deny` resource list、`scope_kind`/canonical-resource UTF-8 顺序与唯一性；deny 在任一 allow clause 前绝对优先，只报告声明关系。
   `migration.generate` 的 optional environment 是 clause qualifier：allow clause 与 requested action 必须同时省略，或同时携带完全相同的 environment。
4. 校验 repo path、process command、network origin、secret ref、environment、governance object 与 execution target 的 lexical/bounded shape；
   `process.exec` request 的 command `timeout_ms` 必须与 proposed usage `timeout_ms` 相等；IPv4-mapped IPv6、IPv6 zone ID 与
   DNS-tagged canonical IPv4 必须失败关闭；secret `version_ref` 只能是 ASCII visible immutable identifier，且大小写不敏感地拒绝
   `active|current|latest` moving alias；不读取或解析真实目标。
5. 校验 budget、`bootstrap_planning|plan_finalization` issuance phase、declared binding、24-hour maximum window、`transferable=false`、approval-ref ordering 与 SoD declarations；`plan_finalization` 必须带 impact/plan/risk digest。不读取进程时钟。`external_operator` 只能是 `authority_proof.issuer.authority_class`，不能冒充 principal type 或已认证身份。
6. 重算 Grant、requested action、assessment request 与 assessment 的 domain-separated digest/self-digest；authority proof bytes 仅按 ADR-0056 的 exact preimage 规则处理。
7. 由 request 完整重建 declared assessment。deny、scope miss、binding drift、超 budget 或窗口外只改变 declared relation/reason；effect mismatch
   必须绑定 `scope=outside_declared_scope`，reason codes 必须按 UTF-8 字节排序、唯一且省略 effect mismatch 已涵盖的 scope reason，不产生
   authorization decision。
8. 校验 expected assessment 必须逐字节等于重建结果；任何把 decision/attestation/result 改为 authority-bearing 的 mutation 失败关闭。

## 输出契约

唯一成功输出是 exact declared assessment，固定包含：

```text
ASSESSED_DECLARATIONS_ONLY (no issuer authentication, policy decision, approval, revocation, usage, preflight, authorization, permission, persistence, execution, or effect attestation)
```

并保持 `assessment_mode=authority_neutral_declared_envelope_only`、`authorization_decision=none`、所有 authority/approval/revocation/usage state
为 `not_evaluated`、`permission_attestation=false`、`effect_attestation=false`。`inside_declared_window`、`covered_by_declaration` 或
`at_or_below_declared_ceiling` 都不是 allow、admit、preflight pass 或有效授权。

## ADR-0059 ApprovalRecord contract-only

当输入是 exact canonical `forgeos.approval-record/v1`、
`forgeos.approval-record-declared-assessment-request/v1` 与 expected assessment 时，使用 ADR-0059 的 pure evaluator；固定
`assessment_mode=authority_neutral_declared_approval_only`。它只比较 caller 明示的 approver/subject、authority binding、source/context/policy/
plan/impact/risk/artifact bindings、decision、scope、conditions、RiskAcceptance refs、SoD declarations 与显式 `evaluated_at_unix_ms`。
禁止从 `.forge/<stage>.approved`、`--approved`、`actor_hint`、workflow status、session、environment 或 ambient clock 导入记录或身份。

严格验证 closed shape、canonical bytes、资源界限、排序/唯一性、detached-proof identity、scope/time/SoD 内部一致性及四个 domain-separated
digests。Proof-shaped base64url、principal tuple、inside-window、declared revocation 尚未到时，均不是现实身份、签名、authority、有效批准或 consent。
唯一成功结果为：

```text
ASSESSED_APPROVAL_DECLARATIONS_ONLY (no approver or authority authentication, attestation or SoD proof verification, condition or RiskAcceptance validation, revocation evaluation, policy decision, effective approval, authorization, permission, persistence, transition, execution, or effect attestation)
```

输出必须保持 `effective_approval_state=not_evaluated`、`authorization_decision=none`、`permission_attestation=false`；approver、authority proof、
condition、RiskAcceptance、revocation registry 和 SoD proof state 全部为 `not_evaluated`，persistence/transition/effect attestation 全为 false。
CapabilityGrant 的唯一兼容投影是 `(approval_id, approval_sha256, authority_domain)` exact relation；reference match 不改变 ADR-0056
`approval_state=not_evaluated`，也不激活 Grant。Scaffold/upgrade 仅复制 ADR/schema/fixture/Python pure checker/tests/governance wiring；不复制
Go/Rust runtime、key、authority registry、revocation/condition/risk state 或 approval store。

## ADR-0073 portable declaration-assessment branch

Portable branch 只接收 caller-supplied exact canonical declared-assessment request bytes；从 repository root 执行以下 exact argv，先以
`python3 -I -B skills/policy-authority/scripts/check_package.py` 验证 closed package，再按 contract 精确选择一个零参数入口：

```text
python3 -I -B skills/policy-authority/scripts/assess_declared_capability_grant.py
python3 -I -B skills/policy-authority/scripts/assess_declared_approval_record.py
```

两入口都必须读到 explicit EOF，成功只输出 computed exact canonical assessment + one LF；只有 exit 0、stderr empty 与 exact bytes 同时满足才算成功。
不得新增 combined dispatch envelope，不得把 assessment output 的 `approval_state` 或 `effective_approval_state` 误作 source Grant/record 字段。
每个 assessment-defined attestation field 保持 false；execution unavailable and unattested，合同中不存在 `execution_attestation` 字段。

该 branch 不读 repository/environment/clock/identity/policy/approval store/revocation/usage/runtime，不调用 ADR-0057/0058、Kernel、PDP、PEP 或 executor，
不安装 host Skill，也不签发、批准、激活、撤销、预留、消费、持久化或执行。Portable prose 不进入 authenticated context routes；package check 与
随后 adapter invocation 不是 atomic check-to-use。缺 `-I` 或 `-B`、不完整 EOF、输入/加载/输出失败都必须停止，且不能用 ambient state fallback。

## ADR-0060 TransitionReceipt accepted contract-only slice

当输入是 exact canonical `forgeos.transition-receipt/v1`、显式 predecessor/target、
`forgeos.transition-receipt-declared-assessment-request/v1` 与 expected assessment 时，使用 ADR-0060 pure evaluator；固定
`assessment_mode=authority_neutral_declared_transition_only`。它只比较 caller 声明的 frozen edge、predecessor chain、state continuity、
precondition、applicability、rework/resume recovery、caller time、CapabilityGrant ref 与 ApprovalRecord refs。`listed_declared_edge`、PASS/NA、
reference equality 或 nondecreasing time 都不是 allow、eligible、transitioned、completed 或 state mutation。

禁止导入 workflow label、`.forge` marker、caller flag、actor_hint、local journal、Go/Rust terminal receipt、Hub state、ambient clock、key、policy 或
ledger。唯一成功结果是：

```text
ASSESSED_TRANSITION_DECLARATIONS_ONLY (no controller, actor, Grant, Approval, evidence, waiver, precondition or state authentication; no policy decision, authorization, persistence, transition, ledger, execution, effect or completion attestation)
```

输出必须保持 `controller_authentication_state=not_evaluated`、`grant_state=not_evaluated`、`approval_state=not_evaluated`、
`ledger_state=not_evaluated`、`authorization_decision=none`、`permission_attestation=false`、`persistence_attestation=false`、
`transition_attestation=false`、`effect_attestation=false` 与 `completion_attestation=false`。Grant compatibility 不得向 ADR-0056 的 frozen 21-effect
vocabulary 添加 `lifecycle.transition`；Approval compatibility 不得把 `approve` 或 matching ref 解释为 effective approval。该合同切片为
Accepted contract-only slice；其完成裁决不产生 Transition authority。Scaffold/upgrade 仅复制 ADR/schema/fixture/Python pure checker/tests/governance wiring，不复制 Go/Rust runtime、
controller、ledger、key、authority state 或 transition executor。

## ADR-0057 窄签发 profile

ADR-0057 不改变上述 pure evaluator。它另交付 Catalyst-only runtime profile
`authenticated_bootstrap_repo_read_grant_issuance_v1`，wire policy profile 固定为 `bootstrap_planning_repo_read_only_v1`。只有 operator 以非 Agent
authority 部署独立 `forge-kernel`，并在 repository 外显式 pin trust root、issuer key 与 durable state 后，Kernel 才能认证 exact signed policy 与
signed GrantRequest。签发范围同时满足以下全部条件：

- `issuance_phase=bootstrap_planning`、`capability_id=repository-reader`、`capability_version=1`、`effect=repo.read`；
- scope 只有 canonical exact repo paths，environment 只有 `local|development|test`，TTL 与预算受 policy 小上限约束；
- signature profile API 为 `forgeos.ed25519-domain-sha256-profile/v1`，signature object 的 profile ID 为 `forgeos.ed25519-domain-sha256/v1`，trust root、policy、request 与 receipt 分别使用已冻结 v1 API；
- 输出只有 signed `CapabilityGrant` 与 durable signed `GrantIssuanceReceipt`，terminal ledger result 只有 `stored|exact_replay`；receipt 不证明 read effect 已执行。

Trust root/policy/key 必须由 operator 带外提供，Kernel 不提供 key generation、provisioning 或 rotation。Runtime 仅支持 Unix（Unix-only），非 Unix 在读取 authority 输入或 key 前失败关闭。Authority root 必须是无 symlink 祖先的绝对规范路径；authority/repository resolved endpoint 必须按祖先 filesystem identity 双向不重叠，caller repository absolute source、首次 resolved path 与 opened directory identity 在整个 session 保持稳定绑定，不能退化为大小写敏感的文本比较。root/state directory exact `0700`，state/leaf 为封闭相对路径，所有叶子是 euid-owned、single-link、无特殊权限位的 exact `0600` regular file，不安全的现有路径不得被 chmod 修复后继续。`0600` 与 effective-UID 检查只是本地文件约束，
不是独立 OS principal、HSM 或 production key custody；同一 UID 的本地/loopback 测试不能冒充完整 production trust boundary。Scaffold/upgrade 只复制
schema、fixture、结构 checker 与本 Skill，不安装 `forge-kernel`、root、key 或 state；缺兼容外部 runtime 时结果必须是 `not_executed`。
公开 golden 中的 known public fixture root/key 只能用于合同复现；生产 root decoder 必须拒绝 exact fixture root 及任何包含其中任一 public key 的变体，issuer 也必须独立拒绝 fixture issuer key。
Signed ledger 的 clock high-water 只相对当前 snapshot 检测 wall-clock rollback。V1 没有 TPM、remote witness 或 external monotonic anchor；能替换 authority
state 的本地管理员仍可回放旧但签名合法的 ledger，因此 receipt/ledger 不得被描述为抵抗该类 state rollback。

`plan_finalization`、其余 20 effects、其他 capability、staging/production、Approval、revocation、usage/reservation、pre/postflight、PEP/effect execution、
ContextPackage/provider/Transition/Knowledge 集成、key lifecycle、remote/HA/multitenancy 与完整 Kernel/PDP 仍 unavailable。`forge accept` 只拥有完成裁决，
不是 issuer、PDP 或 permission source。

## ADR-0058 窄执行 profile

ADR-0058 另交付窄 profile `authenticated_bootstrap_repo_read_execution_v1`；它不改变 ADR-0056 pure evaluator，也不能把 ADR-0057 issuance policy
重解释为执行同意。Operator 必须在 repo 外 pin 一个独立 execution trust root；其 `execution_policy_sign`、`execution_receipt_sign`、
`execution_request_auth` 三组 key ID/principal/public key 两两不同，且不得与 issuance root 的任一 key 重合。只有 exact signed execution policy 的
`allow/activate_once` 加 exact signed invocation 才能预留；signed `deny/do_not_activate` 不产生 reservation。

V1 key usage 闭集为：`execution_policy_sign` 只签 ExecutionPolicy，`execution_request_auth` 只签 Invocation，`execution_receipt_sign` 恰好只签
domain-separated UsageReceipt 与 complete UsageLedger snapshot。不得交叉签 Policy/Invocation/issuance/其它 artifact；新增 key 或扩大 usage 必须新 profile
并由 operator 外部 pin。

Execution authority root/state 继承 ADR-0057 的 resolved ancestor identity 双向 repo 隔离、无 symlink absolute root、exact `0700` directories、
closed relative state/leaves，以及 effective-UID-owned/single-link/no-special-bits exact `0600` regular leaves；Kernel 不生成或 provisioning key。

执行范围仍只有 `bootstrap_planning`、`repository-reader/v1`、`repo.read`、`local|development|test`。Manifest 必须按 UTF-8 path bytes 严格排序，
包含 1..16 个与 Grant/requested action 完全相同的 exact path，并固定每个 regular file 的 raw byte length 与 SHA-256；top-level
`.git|.forge` 以大小写不敏感方式禁止；path 最多 4096 UTF-8 bytes / 256 components，Unicode/space 保留 exact byte identity。Repository
filesystem 只允许 ext2/3/4、XFS、Btrfs、tmpfs、overlayfs、ZFS 的 frozen statfs magic，FUSE/network filesystem 失败关闭。不能把 opaque
`source_revision|source_tree_sha256|context_sha256` 当成 Git、Context reassembly 或 bytes proof。Linux-only runtime 仅支持 amd64/arm64；initial 与
post-read reopen 都先以 `O_PATH|O_CLOEXEC` confined `openat2` probe + fstat regular/exact-size/nlink1，再以
`O_RDONLY|O_NONBLOCK|O_CLOEXEC|O_NOATIME|O_NOCTTY` active open，并要求 SameFile 与 invariants 重验。两次均使用
`RESOLVE_BENEATH|RESOLVE_NO_XDEV|RESOLVE_NO_SYMLINKS|RESOLVE_NO_MAGICLINKS`，不得 fallback 普通 walker。EUID 非 owner、无 `CAP_FOWNER` 或 FS
不支持 `O_NOATIME` 时失败关闭，绝不能 fallback 到会更新 atime 的 open。

静态 FIFO/device 永不 active open；`O_NONBLOCK` 避免 race 后 FIFO 无限阻塞，`O_NOCTTY` 避免 controlling TTY。但 probe 与 active open 间并发替换
仍可能触发 FIFO rendezvous/driver side effect。Repo 无 special nodes、untrusted writer 无 `CAP_MKNOD` 且执行期间没有不可信 namespace writer 是
operator 部署前提；v1 不 attest 这些前提或 driver isolation。

Pre-reservation platform check 只能是 build-tag 固定的 GOOS/arch 判断，不得 probe cwd 或任何 filesystem。Bound repository 的 visible-superblock 与
openat2 preflight 必须移到 durable `reserved_no_repo_io` 后、`effect_intent` 前；失败只能写 signed quarantine，不能先碰 repo 再补 reservation。

`fstatfs` 只识别直接可见的 superblock magic，不认证 allowed filesystem 的物理 locality，也不认证 overlayfs lower/upper backing；local backing 是
operator 部署前提。`network_bytes=0` 只表示本 effect 未发显式网络请求，不能证明 backing storage 没有网络。

唯一 usage group 是 `reserved_no_repo_io -> effect_intent -> completed|failed_consumed|quarantined`，reservation 也可直接 quarantine。Reservation 必须在
fresh Invocation 的半开窗口内，且在 `BindRepository|VerifyRepository` 等任何 repository metadata I/O 前 durable persist/reopen；intent 必须在首个 read
前 durable persist/reopen。已 fresh reservation
的 intent/terminal timestamp 可越过 Invocation expiry；这不放宽 successful `elapsed_ms<=timeout`。Active tail 永不 resume 或 reread，只能写 signed
quarantine。容量必须在 reservation 前为 intent 与 terminal 留足空间。Reservation、intent、quarantine、terminal 每次签名前都独立采 wall clock 并
推进 high-water；clock failure 不得伪造旧时间，active tail 留给下次调用 signed quarantine。

Timeout 在 durable intent 后、首读前开始且只是 cooperative timeout。Pinned content reader 在 statfs/openat2/stat/read/reopen 前、间、后检查；
grantstate 的 repository source/identity revalidation 是 composite，只整体前后检查。任一 blocked kernel op 都可越预算，但返回后 timeout 优先于其它
read outcome，必须 `failed_consumed` 且不交付内容。

完成态只有在 terminal persistence + strict reopen 后首次返回 receipt、content-free metadata 与 raw base64url result。Ledger 不持久化 raw；之后的
receipt-only replay 必须在 manifest/repository/clock/receipt-seed access 前，以 exact canonical policy+invocation pair，或两个 raw lowercase 64-hex
self-digest 命中 terminal group；digest-only miss、mixed identity、单边或冲突 match 都必须在 manifest 前失败关闭。Replay 返回相同 receipt/metadata 与
`execution_result=null`。Crash、short write 或首次 delivery 不确定时 raw 不可恢复，严禁重读。Runtime 可 best-effort clear mutable raw buffers，但 Go base64 string、
GC、kernel 或 downstream copy 的 secure erasure 不可证明；不得声称 secure-erasure、process-isolation 或 HSM attestation。

Reservation/intent/terminal 的 pre/post-publish 六格 fault matrix 必须收敛到 completed 或 quarantine，任何格都不得造成第二次 repository read；
post-publish uncertainty 不得以返回错误猜测 commit 状态，后续只能从 strict reopened ledger replay/quarantine。

Execution ledger high-water 仍只相对当前 signed snapshot；能整体替换 state 的管理员可回放旧 snapshot，v1 无 TPM、remote witness 或 external
counter。Pinned execution root、receipt signing key 与既有 usage-ledger namespace 是不可分割部署：root/key 变化时旧 ledger 必须失败关闭；fresh
root/state 不继承 spent history，也不能消费先前 namespace 下的 Grant。V1 不支持 root/key rotation、trust-epoch migration 或 usage-state
clear/rebase；连续性轮换必须另立 profile/ADR，并由外部 witness 迁移完整 single-use history。公开 fixture authority 必须由 production decoder 按
exact fixture root 和任一 fixture public key 拒绝。Scaffold/upgrade 只复制 ADR、schema、
fixture、Python structural checker 与 governance test，不安装 binary、root、key 或 state；无兼容外部 runtime 必须记录 `not_executed`。
Approval/revocation、write/network/process/secret/target、staging/production、Context reassembly、general PDP、remote/HA/multitenancy 均未开放；
`forge accept` 不是 execution authority。

## 规则、禁止与权限

- Agent、role、Skill、workflow、fixture、schema validation 和 self-declared issuer 都不能 mint authority。
- ADR-0056 pure evaluator 禁止输出或持久化 `authorized`、`allowed`、`admitted`、`active`、`grant_valid`、`preflight_passed`、`permission_granted` 或等价状态；不得把它与 ADR-0057/0058 Kernel receipt 混用。
- `authority_proof`、Approval refs、issuer/requester/subject 与 SoD 只做 envelope-level shape/declared-identity 检查；不验证签名、现实主体或控制关系。
- `issued_at/not_before/expires_at` 只提供显式 caller-time relation；没有 revocation/usage view 时永远不能推导 effective Grant。
- production `migration.apply|release.execute` envelope 必须声明 `external_operator` issuer 与非空 approval refs；这仍不验证 issuer/approval，
  不改变 `production_effects: forbidden`，也不授予生产凭据或远程动作。
- 本 Skill 与 ADR-0056 pure checker 无 repository、filesystem target、process、network、provider、secret broker、Hub/SQLite、journal 或外部 effect 权限，不能自行生成 ADR-0057 receipt；只有已正确带外部署的 Kernel runtime 能生成并持久化它。

## 自动化与验收

```bash
python3 -B harness/capability_grant_contract_check.py --golden <repo-root>
python3 -B harness/capability_grant_contract_check.py <repo-root> <canonical-request.json> <canonical-assessment.json>
python3 -B harness/approval_record_contract_check.py --golden <repo-root>
python3 -B harness/approval_record_contract_check.py <repo-root> <canonical-request.json> <canonical-assessment.json>
python3 -B harness/transition_receipt_contract_check.py --golden <repo-root>
python3 -B harness/transition_receipt_contract_check.py <repo-root> <canonical-request.json> <canonical-assessment.json>
python3 -S -B harness/bootstrap_grant_issuance_contract/check.py --golden <repo-root>
python3 -S -B harness/bootstrap_repo_read_execution_contract/check.py --golden <repo-root>
```

Python、Go、Rust 必须对 exact golden 产生 byte-identical Grant/requested-action/request/assessment digests 与 declared relations。正反测试至少覆盖 aliases、
duplicate/unknown/noncanonical/oversize、effect/scope mismatch、path/origin/command bounds、deny precedence、phase/budget/time/TTL/transferability、SoD、
proof/approval/digest mutation、binding drift，以及所有 authority/effect escalation。最终完成权威仍仅属于 `forge accept`。
