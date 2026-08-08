# Device-Aware Execution Fabric：设备感知执行织网

> 状态：已采纳的目标设计，`planning_only`，默认关闭。当前仓库只有本地 process、Docker/Firecracker runner 等执行基础；
> 尚无 Device Registry、SSH inventory、远程 Runner RPC、Placement、Lease、迁移或联邦能力；现有 Go runner 也不等于
> 完整 coding-workspace exchange，Rust runtime 的 OS sandbox/runner 集成仍未完成。本文不授权扫描网络、读取 SSH 凭证、
> 登录设备或执行远程命令。

## 1. 原则与职责边界

目标是把 `Agent → 本机 shell` 演进为：

```text
Decision Kernel → immutable TaskSpec / PlacementRequest
                    ↓
             Execution Fabric Control Plane
                    ↓
      Local Target / Device Runner / Cluster / Model Service
                    ↓
       ExecutionEvidence / Artifact / Attempt Receipt
```

核心原则：**调用接口位置透明，分布式失败语义显式**。上层请求 Capability，不依赖设备名；Fabric 必须暴露网络、离线、
重复、LOST、环境漂移、数据移动和副作用不确定性。

- Decision Kernel 决定目标、可行方案、规则、权限与 proof；不能直接执行；
- Fabric 决定目标是否可放置、在哪里/如何受控执行、资源/lease/failure；不能改变目标或技术方案；
- Evidence System 校验实际运行身份与结果；不能自授成功；
- Governance Kernel/PDP 签发 CapabilityGrant；生产仍由外部 operator authority；
- Controller 维护 desired/actual state 并 reconcile；CLI 只提交 desired state，不直接偷改任务。

Logical Agent、TaskNode、DecisionTransaction、ExecutionAttempt、ExecutionTarget 五层必须分离。Agent 不永久绑定设备，一台
设备也不隐含某个角色或权限。

## 2. 分级开关

```yaml
device_fabric:
  mode: OFF                 # OFF|INVENTORY|OBSERVE|EXECUTE|MIGRATE|FEDERATE
  discovery:
    static_config: true
    self_registration: false
    import_ssh_config: false
    lan_active_scan: false
  probe: {basic: false, runtime: false, validation: false, benchmark: false, auto_install: false}
  execution: {remote: false, arbitrary_shell: false, sandbox_required: true}
  migration: {stateless: false, checkpointed: false, side_effecting: false}
  peer_transfer: false
  resources: {observe: true, affect_placement: false, affect_solution: false}
  security: {new_device_human_approval: true, mtls: true, short_lived_credentials: true}
```

模式单调增加能力但不隐式授权：

| Mode | 能力 |
|---|---|
| OFF | 仅 `LocalExecutionTarget`；Fabric 远程模块不加载 |
| INVENTORY | 手工记录 locator/labels；不连接、不探测 |
| OBSERVE | 获准目标只读探测；不执行任务、不修复环境 |
| EXECUTE | 对获准、可沙箱、effect policy 允许的 Task 远程执行 |
| MIGRATE | 仅声明可移动且有 checkpoint/idempotency/fencing 的任务迁移 |
| FEDERATE | 多 control domain/集群/租户；最后实现 |

进入某个 mode 不等于允许 discovery/probe/execute/migrate/data transfer。每项还需要 policy、Grant 和 exact target scope。
启用更高模式、产生 external message/real cost、扩大网络或数据驻留边界必须回 00 做影响评估并取得明确授权，不能由
Scheduler 为了“更快/更便宜”自行开启。

## 3. 稳定对象模型

### ExecutionTarget v1

```yaml
target_id: target-...
kind: local|device|vm|cluster|service|cloud_batch|model_endpoint
identity_ref: device-identity-...
capability_snapshot_ref: ...
trust_zone: ...
transports: []
protocol_versions: []
labels: {}
status: ready|cordoned|draining|maintenance|offline|stale|quarantined
```

顶层接口为 `probe/reserve/execute/cancel/collectEvidence`；实现可为 Local、SSH bootstrap、Runner RPC、Kubernetes、CI 或
ModelEndpoint adapter。调用方不能假设 POSIX 路径、localhost、共享进程空间或同一文件系统。

### TargetIdentity / DeviceIdentity v1

所有 ExecutionTarget 使用 `TargetIdentity`；physical device subtype 扩展为 `DeviceIdentity`。稳定身份来自目标公钥/证书
指纹，条件允许时来自 TPM/Secure Enclave attestation；IP、hostname、SSH alias 只是可变 locator。Enrollment 要求 key
proof-of-possession、设备 owner 明示同意、enroller authority、challenge nonce、防重放 receipt；Identity 保存 key fingerprint、
trust domain/level、attestation evidence/TTL、rotation/revocation history。新目标默认 pending，需人类/组织 enrollment authority
批准。无硬件 attestation 时明确降低允许的数据/effect ceiling，而不是把“未证明”当可信。

### TargetCapabilitySnapshot v1

静态能力：OS/arch/CPU/memory/accelerator、CUDA/ROCm/Metal/Vulkan、runtime/compiler/container/sandbox、disk、model formats。
动态状态：available CPU/memory/VRAM/disk、load/temperature/battery/power、queue、interactive/busy、cached/loaded models、
network observations。每项绑定 probe version、Evidence、observed/valid-until 和 confidence；过期能力不参与硬可行性判断。

探测分 P0 identity/basic、P1 runtime、P2 minimal capability validation、P3 benchmark、P4 bounded telemetry。探测与修复/安装
是不同 Capability；发现缺 Docker 不允许 probe 自动安装。

### NetworkLink v1

资源图边保存 same-host/LAN/VPN/WAN/relay、bandwidth/latency、metered/encrypted、trust crossing、observed_at/expiry。
跨设备事件顺序用 event sequence、lease epoch、fencing token、Lamport/HLC；wall clock 只作展示/审计，timeout 使用单调时钟。

## 4. TaskPlacement v1

每个 TaskNode 声明：required/preferred capabilities、OS/arch、CPU/memory/accelerator/disk；Artifact inputs；数据分类、驻留、
允许传输/目标；effect class；mobility；sandbox/image/runtime lock；affinity/anti-affinity；deadline；target hint/required。

`target_hint` 可被 scheduler 拒绝并解释；`target_required` 是硬约束且仍不能越过 trust/data/Grant。设备标签描述能力/策略，
工作流不能硬编码“AMD 节点”“Mac mini”等具体名称。

## 5. Placement Dry-run 与调度

任何非本地 attempt 先产生 `PlacementPlan`：candidate targets、逐项 feasible/rejected reason、能力证据新鲜度、数据/模型/
cache 局部性、预计 transfer/time 区间、环境风险、selected/fallback、参与决策的资源开关。Dry-run 不是 reservation 或成功。

使用字典序而不是一个总分：

1. 硬过滤：只读 Placement Grant、trust/data residency、runtime/resource、effect、status、deadline、sandbox；
2. 质量/可靠性：兼容、历史 success/LOST、证据新鲜度、回滚与隔离；
3. 数据/模型局部性：Artifact、image、dependency、model cache/load；
4. DAG/通信：关键路径、queue、共址、affinity/anti-affinity、`data_size/bandwidth + latency×rounds`；
5. 可选资源：只有 `affect_placement=true` 后，时间/Token/金钱/能耗才能在同等质量可行目标之间排序。

Placement Grant 不是第二种授权真值，而是 Kernel/PDP 签发的标准 `CapabilityGrant(effect=placement.plan,
targets=query-scope, reserve=false, execute=false)`；它只允许读取获准的 target/capability/trust 摘要并生成 dry-run。00 先提交
planning GrantRequest，Kernel/PDP 才能在 Node08 前签发；Node08/Agent 不自授。选定 target 后，Kernel/PDP 根据 immutable
TaskSpec + PlacementPlan、target identity/attestation/capability/environment
digests、attempt ID 和 effect/data/network scope 签发 per-attempt `CapabilityGrant` 与 issuance receipt；reserve 原子消费 Grant
中的单次 reservation allowance，并产出 `ReservationReceipt` 与 `GrantUsageReceipt`。issuance receipt 只证明签发事实，不被消费。
fallback/重放必须 revoke/fence 旧 Grant/lease，并对新 target 重签，解决“先有 target 还是先有 Grant”的循环。

`affect_solution=true` 需要独立 policy/Approval，仍不能降低正确性、安全、Evidence 或模型能力底线。实现与独立验证可用
anti-affinity 降低相关失败；大数据 producer/consumer 可 affinity 共址。

## 6. 预约、Lease 与执行证据

资源必须原子预约后再 staging：

```yaml
lease_id: lease-...
task_node_id: ...
attempt_id: ...
target_id: ...
world_version: ...
lease_epoch: ...
fencing_token: ...
reserved: {cpu: ..., memory: ..., accelerator_memory: ..., disk: ...}
acquired_at: ...
renew_by: ...
expires_at: ...
status: reserved|running|released|expired|revoked
```

旧 attempt 恢复网络后，fencing token 不是最新就不能提交状态/effect result。CPU 可有限 overcommit；memory/VRAM 严格预约；
disk 保留安全水位；per-device/per-capability queue 和 concurrency 做 admission/backpressure。

Runner 必须使用单调时钟维护本地 lease watchdog，并在 `renew_by` 前完成续租；无法向 control plane 续租时，必须先本地
self-fence：撤销 task credential/egress、停止 effect adapter、终止并确认 task process/sandbox，再释放资源。task credential、
egress permit 和外部 adapter token 的 TTL 不得晚于 lease expiry，且绑定 attempt、lease epoch 与 fencing token；网络分区不能让
旧 attempt 持续占用凭据、出口或提交副作用。所有外部 effect adapter 必须在目标 authority 端校验 fencing/idempotency key；
无法执行该校验的 effect 必须声明 pinned、禁止自动 retry/fallback，并在 LOST 后进入人工对账。新 attempt 只有在旧 lease 已
fence 且资源重新原子预约后才能启动；“控制面已过期”本身不等于“旧进程已停止”。

`ExecutionEvidence` 绑定 Task/Attempt/Target/Lease、code/source tree、environment、input/output Artifact digest、runner/tool/model/
protocol versions、structured logs、result `passed/failed/inconclusive/lost` 和 verifier。Canonical `AttemptReceipt` 记录 state/
reason/start/terminal/cleanup refs；`EffectReceipt` 记录 intent/idempotency key、target authority domain/environment、observed effect/
uncertainty、reconciliation owner。远端说“成功”不是证据；摘要或 freshness 不匹配进入 INCONCLUSIVE/
RECONCILIATION_REQUIRED，不能投影为 FAILED 或 PASS。

## 7. Artifact 与环境

状态外部化：Task/Agent session、计划图、events、code snapshot、Artifact、Checkpoint、Evidence 和 Decision Capsule 都存于
control-plane governed storage；设备本地只是 cache/attempt workspace。

跨目标交换使用 Git/workspace snapshot + content-addressed Artifact Store，不依赖共享可变目录/SSHFS。Artifact 有 hash、size、
media/schema、sensitivity/residency、producer attempt、integrity/provenance、retention；传输支持去重/分块/校验/清理，transport
（SFTP/HTTP/rsync/P2P）不提供身份。

任务绑定 lockfile、container/image digest、model、Runner/toolchain 和 environment fingerprint；缺少可复现环境时明确降级/
拒绝，不在远端静默安装。`StagingReceipt` 绑定 base tree、Artifact、path allowlist、mount/network/credential profile；远端只能
上传 immutable `WorkspaceDelta` 与 `PublishReceipt`。把 delta 合并/应用到权威 workspace 是单独的 `ApplyDelta` Capability，
需要 CAS base tree、冲突检查、独立写 Grant 和 `ApplyReceipt`；远程 executor 不直接改 control-plane 工作树。

## 8. SSH 与 Runner

SSH 仅适合 enrollment/bootstrap、Runner 升级、获批诊断和早期结构化 MVP。禁止 Scheduler 拼接任意 shell 字符串并解析
stdout；SSH TaskSpec 使用 argv、cwd、env allowlist、timeouts、limits、input/output contracts，默认 sandbox。

首次远程执行前必须实际验证 sandbox profile：只读/可写 mounts、workspace path、network/DNS egress、process namespace、CPU/
memory/disk/PID、credential injection/removal 和 cleanup；验证绑定 Runner/OS/profile digest，失败关闭。SSH 的“隔离目录”不等于
sandbox，无法满足 `sandbox_required`。

隔离强度有不可下调的 policy floor：所有 remote executable 至少使用经验证的 OS/container isolation；其中任何未受信代码、
AI 生成代码、未知依赖或 L2+ 可执行工作负载必须使用 microVM-equivalent 隔离（独立 kernel/VM boundary，或经安全评审证明
等效的专用 sandbox）。需要 secret、敏感数据或外部 effect 的任务还要叠加更高的数据/effect policy，不能用
“已在容器中”自动放行。task sandbox 不得看到 Runner control channel、mTLS/device identity key、SSH agent/socket、host secret
broker 或 lease-signing credential；Runner transport 位于 sandbox 外，任务只通过窄化 broker 得到 task-scoped、短期、可撤销
的 capability handle。若目标无法证明所需 isolation profile，placement 必须拒绝而不是降级执行。

成熟路径由 Runner 生成设备密钥并主动 mTLS 连接 control plane，完成注册、心跳、拉取 Task、结构化日志、lease/fencing、
Artifact/Evidence。主动连接适应 NAT；SSH key 留在 Secret Broker/HSM/OS keychain/agent，只有 sandbox 外的 privileged transport
adapter 可取得不可导出的 opaque handle。Task payload/sandbox 不得到 key、socket、control credential 或可兑换它们的引用。

导入 SSH config 只读 Host/HostName/Port/User/ProxyJump/IdentityFile **引用**；默认不连接。LAN active scan 独立关闭，不能
默认扫描网段/端口或自动登录。

## 9. Zero Trust 与数据驻留

Target enrollment 不代表访问所有项目。每次 attempt 需要 task+target 绑定的 CapabilityGrant：allowed actions/resources/data
classes、effect ceiling、network origins、Artifact refs、expiry/budget。设备 trust zone 必须满足数据要求；沙箱保护设备免受任务
影响，却不能防设备管理员读取任务内容，因此 untrusted target 不接收 confidential/secret/regulated 数据或 secret。

网络 egress 由 target-side PEP 默认拒绝，只放行 Grant 中精确 DNS name/IP/port/protocol、TLS identity、direction、byte/time
上限；每次 transfer 生成 Artifact/NetworkTransfer evidence。数据策略绑定 region/zone、allowed store、local-only、retention 和
secret workload；DNS/redirect/proxy 不能绕过 origin/data ceiling。

Production write/deploy/migration 仍由外部 operator；Fabric 不能因远程能力存在而取得生产凭据。Production adapter 必须
绑定外部 authority domain + exact environment，核验 Approval/OperatorGrant 后导入 `OperatorReceipt`，并由 G8 验证业务/数据/
观察窗口；Fabric 自身不能签发或伪造该 receipt。探测、安装、execution、
peer transfer、migration、break-glass 各自分权。Cordon 停新任务；Drain 只迁移可移动任务；Revoke 使 lease/grant/credential
失效并触发 reconciliation。

## 10. 失败、移动与副作用

Mobility：

- stateless：可在新 target 重算；
- restartable：从头重试；
- checkpointable：从已验证 Checkpoint 恢复；
- pinned：本地 key/设备/生产连接等原因不可迁移。

Agent/LLM 不承诺恢复隐藏 token state，只保存完整 context、已完成 Transactions、tool results、capsule/checkpoint 后重新规划。

分布式执行不声称 exactly-once。基础语义是 at-least-once scheduling + idempotency key + lease/fencing + effect receipt +
reconciliation。设备 LOST 时，只有 effect-free 的 pure/read-only 任务可在旧 lease 过期、authority-side 新 fencing epoch 已建立，
且新 target 获得 fresh Grant/原子 reservation 后重试；这不证明旧进程已经停止，只保证它不能提交结果或取得有效
credential/egress/effect authority。幂等写使用相同 key；checkpoint 恢复；可补偿任务先对账；不可逆或 effect unknown 必须
quarantine，不自动 retry/migrate/speculate。

Speculative execution 默认关闭，只允许 pure/read-only/sandboxed、结果可比较的 Task；发送消息、写 DB、deploy、支付等禁止。
失败目标不能自动 fallback 到 trust/quality 更低设备；fallback 必须仍通过全套硬过滤并取得新 lease/grant。

## 11. Controller / Reconciliation

CLI/API 提交 DesiredTargetState（注册、标签、cordon/drain、placement、replica/task intent）；Controller 观察 Actual State，生成
有界 DecisionTransaction，经 Grant 后 action，再验证：`Observe→Compare→Plan→Authorize→Act→Verify→Reconcile`。

设备状态：`UNREGISTERED→ENROLLED→PROBING→READY↔CORDONED→DRAINING→MAINTENANCE`，旁路 OFFLINE/STALE/QUARANTINED。
任何非 terminal Attempt 与 Runner/control plane 失联时先进入 LOST，不得沿成功/失败捷径投影。Attempt 主路径：
`CREATED→PLACED→RESERVED→STAGING→RUNNING→VERIFYING→SUCCEEDED/FAILED/INCONCLUSIVE`；
`INCONCLUSIVE|LOST→RECONCILIATION_REQUIRED→RECONCILING→SUCCEEDED|FAILED|QUARANTINED`。QUARANTINED 只能由有权 controller
在人工/专用对账取得 effect、process 与 Artifact 证据后转回 RECONCILING，再落到 SUCCEEDED、FAILED 或 CANCELLED。已确认
terminal（SUCCEEDED/FAILED/CANCELLED）且 Artifact/Evidence 持久化后才进入 CLEANING/CLEANED。LOST 不等于 FAILED，
QUARANTINED 也不等于 terminal success/failure。

Cancel 是覆盖在非 terminal Attempt 上的协议：`CANCEL_REQUESTED→CANCEL_ACKNOWLEDGED→TERMINATION_VERIFIED→CANCELLED`；
未确认、超时或连接断开进入 RECONCILIATION_REQUIRED，不能自证进程或 effect 已停止，也不能直接清理。`CleanupReceipt` 绑定
attempt/lease/fencing、删除范围、保留 Artifact、执行者和 postflight evidence。
发生 external effect uncertainty、incident/legal hold、取证需求或可恢复 checkpoint 时保留隔离 workspace，直到 retention/
人工处置策略明确；cleanup 与迟到 result/重连通过 fencing/CAS 解决竞态。

设备重连先报告 attempts/checkpoints，按 lease epoch/fencing 对账，终止过期任务，再接受可验证 checkpoint。Controller 循环有
generation/world version、幂等 reconciliation、backoff/jitter、budget/tripwire；actual 永不靠命令自报直接改成 desired。

## 12. AI / Model Capability

Model endpoint 额外声明 model/backend（CUDA/ROCm/Metal/Vulkan/CPU）、format/quantization、context、cached/loaded、KV/resource、
tokens/sec/first-token latency 的证据和有效期。优先把它暴露成有 schema 的 `model.invoke` Capability，Agent 控制面可保持本地；
只有确有必要才移动整个 Agent workspace/context。模型 placement 仍受 sensitivity、quality floor 和 provenance 约束。

## 13. 与 AADM、00–16 和插件生态的连接

- AADM 生成 TaskPlacement、DiscretionEnvelope 和 proof obligations；Fabric 不重算业务目标；
- 00 将 target/data/effect 影响纳入 pre-scan；Assessment Join 将远程 failure/trust/residency/transfer 纳入 final risk；
- 08 规划 affinity、attempt、fallback、validation 与 `GrantRequestSet`；09/12/13 消费 execution evidence；
- 10–13 可在独立 target/environment 复验；14 只打包，外部 operator 仍执行生产；15 对 LOST/quarantine/incident reconcile；
- 16 Reflection 分析 placement bias、environment drift、LOST/retry/成本和 Evidence 质量，只提交改进提案；
- ExecutionTarget/Capability adapter 以后可作为签名插件，但插件不能绕过 Registry/schema/PDP/gates。

## 14. 渐进实施

0. **Kernel/Fabric ABI。** 先让本地调用统一经过 ExecutionTarget、Attempt、ArtifactRef、EnvironmentDigest、Effect/Mobility、
   PlacementConstraint 和 Evidence；Local adapter 行为不变。冻结 contract/fixtures，不增加远程 effect。
1. **Inventory/Observe。** 静态配置、可选 SSH config import、proof-of-possession/owner consent、基础 attestation/TTL/rotation
   （缺失则降低 ceiling）、身份/labels、只读分级 probe、UI/CLI dry-run；无主动扫描。
2. **结构化 SSH MVP。** 指定目标、通过前述 verified sandbox profile、隔离 workspace、argv TaskSpec、timeout/cancel、Artifact、
   environment/code digest；低风险 only。
3. **Runner。** 主动注册/mTLS/heartbeats、dynamic state、reservation/lease/fencing、structured Evidence、short-lived Grant。
4. **Scheduler/Controllers。** capability/data/model/topology aware placement、affinity、cordon/drain、backpressure/reconcile。
5. **Recovery/Migration。** checkpoint、LOST reconciliation、idempotency/compensation、safe migration；side-effecting 默认禁用。
6. **Federation。** 最后才做 P2P、多 control plane、Kubernetes/Nomad/Slurm、quota、cross-domain/federated attestation 和
   gang scheduling；基础 target attestation 不能推迟到此阶段。

## 15. 验收与反模式

必须证明：OFF 模式零 DNS、listener、SSH/config import、registry mutation 或远程连接；inventory 不探测；probe 不安装；
地址变化不改变 identity；key proof/owner consent/attestation TTL/rotation 生效；stale capability 不可放置；
不可信 target 拿不到敏感 Artifact/secret；每次远程 result 绑定 code/env/input；双重 reservation 不超卖；旧 fencing token 拒绝；
lease watchdog 在分区/续租失败时先 self-fence，task credential/egress TTL 不越过 lease；外部 effect adapter 执行 fencing/幂等；
LOST/INCONCLUSIVE effect 不自动重试；placement→per-attempt Grant 无循环且 fallback 重签；风险对应的 sandbox floor 失败关闭，
task 看不到 Runner/mTLS credential；远端 delta 不能越权 apply；egress/data residency 被 PEP 执法；生产 OperatorReceipt/G8
边界不变；local adapter 与冻结 ABI contract tests 兼容。

禁止：一个 `enable_remote` 开全部权限、IP 当身份、主动扫描/自动登录、SSH 任意 shell、设备自报即可信、任务状态只在设备、
共享可变目录、所有失败自动重试、成本降低质量、Agent 永久绑设备、初期 P2P mesh、probe 自动修复、Fabric 自行改变方案。
