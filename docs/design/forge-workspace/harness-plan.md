# `harness` 收敛与实施计划

> 状态：**Proposed / 增量收敛方案，非当前目录变更声明**
> 日期：2026-08-21
> 目标：保留带外信任边界，把 Harness 从“第二套产品内核”收敛为独立 Verification Plane。

## 1. 不变原则

Harness 必须：

- 在 Forge App、Go Control Plane 或 Rust Runtime 损坏时仍可独立运行；
- 只消费显式 repository root、Snapshot、Artifact、contract fixture 或 VerificationRequest；
- 输出原始 observation 和有界 VerificationReceipt；
- 诚实区分 pass、fail、inconclusive、not_executed/N-A；
- 不调用 Agent 来判断自己的载重检查；
- 不读取/写入 `control.db` 或 `runtime.db` 私有表；
- 不推进 WorkItem/Change，不签发 Grant/Approval，不自行采用 Knowledge。

收敛不是减少验证强度。安全、授权、身份、路径和 canonical digest 等信任边界仍应保留独立实现和 adversarial fixture。

## 2. 目标职责

```text
harness/
  acceptance/       end-to-end completion checks
  security/         secret, SCA, path, supply-chain, effect boundary
  architecture/     dependency, cycle, ownership, size/complexity signals
  conformance/      canonical schemas, cross-language golden/adversarial
  governance/       .agent/config/reference integrity verification
  adapters/         ecosystem test/lint/build/coverage runners
```

脚手架、升级事务和产品数据迁移属于 `tools/`；产品领域 projector/evaluator 属于唯一 domain owner；canonical schema/fixture 属于 `contracts/`。迁移期 Harness 可保留 compatibility wrapper。

## 3. 当前目录分类策略

### 3.1 Keep in Harness

| 当前能力 | 目标位置 | 原因 |
|---|---|---|
| `gate.mjs`, `policies.yml` | `architecture/structure` | 带外结构 gate |
| `acceptance*.mjs` | `acceptance/` | 顶层独立验收 |
| `adapters/` | `adapters/` | 生态工具执行适配 |
| `arch/`, `frontend-architecture/` | `architecture/` | 依赖与代码结构验证 |
| secret/SCA/coverage probes | `security/`/`adapters/` | 外部工具观察 |
| `.agent` reference/routing checks | `governance/` | 治理资产完整性 |
| security-critical contract checker | `conformance/security` | 独立信任边界 |

### 3.2 Move canonical source, keep checker

以下 `*_contract` 目录中的 normative schema/golden 应逐步迁到 `contracts/`，Harness 保留独立 decoder/checker：

- Approval/Grant/Transition/Knowledge contracts；
- Context/WorkIntent/ProjectSnapshot/GraphSnapshot；
- Kernel operational/decision/capsule；
- Artifact/Command observation evidence；
- capability registry/ownership projection。

迁移前先验证谁是 semantic owner。仅移动文件不算收敛；必须保证 schema 只有一个 normative source，Harness checker 不再重新定义业务默认值。

### 3.3 Move out of Harness

| 当前能力 | 目标 owner | 迁移方式 |
|---|---|---|
| `scaffold/` 和 upgrade transaction | `tools/scaffold` | Harness 留 smoke/acceptance wrapper |
| live project observation producer | Go/Rust observer | Harness 验证 captured Artifact |
| product planning/impact projector | Platform Core/Go | Harness 只做 conformance fixture |
| knowledge/current-state selection | Go Knowledge authority | Harness 不选 truth/winner |
| runtime action/provider behavior | Rust Runtime | Harness 用 fake/fixture 验证协议 |
| product completion join | Go Reconciler | Harness 只返回检查 Receipt |

`governance_engineering/` 必须逐文件分类，不能整目录粗暴移动：pure independent validator 可保留；live producer、product projector、scaffold helper 分别迁给 owner。

## 4. Verification 协议

### 4.1 VerificationRequest

```text
verification_id
idempotency_key
space/project/change/work_item refs
project_snapshot_ref
change_manifest_ref
artifact_refs[]
required_checks[]
policy_profile/version
limits { timeout, bytes, processes }
requested_at
```

请求不可携带“期望 PASS”或可被 checker 当作 authority 的自由文本结论。

### 4.2 CheckObservation

```text
check_id/version
status: pass|fail|inconclusive|not_executed
started_at/finished_at
tool/version/environment
input_refs
command_or_probe_summary
output_artifact_refs
finding_refs
applicability_reason
```

### 4.3 VerificationReceipt

Receipt 绑定 immutable request digest、所有 observation digest、Harness version、terminal status、coverage 和 limits。它不包含 `change_completed=true`。

## 5. Harness 执行模型

```text
decode + strict validate request
→ materialize isolated verification workdir
→ revalidate Artifact/Snapshot digests
→ select explicitly required checks
→ run each through bounded adapter
→ capture raw output as Artifact
→ derive CheckObservation
→ join only requested verification status
→ emit VerificationReceipt
```

默认不自动降级：required tool 缺失返回 `not_executed`；是否阻断由请求 policy profile 和 Control Plane acceptance join 决定，Harness 不把生产要求擅自改成 advisory。

## 6. 命令面收敛

保留开发者兼容命令：

```text
forge gate
forge check
forge accept
```

内部目标入口：

```text
harness verify --request FILE --output FILE
harness conformance --suite SUITE
harness architecture --profile PROFILE --root DIR
harness acceptance --root DIR
```

不要求立即重写为单一 Harness binary。Node/Python dispatcher 可以继续，但必须有统一 exit/status/receipt contract；生态工具仍由 adapter 调用。

## 7. Policy 与 checker 边界

- Policy 定义适用条件、required checks 和阻断强度；
- Checker 只观察和验证，不解释产品 Outcome；
- Adapter 只执行工具并规范化原始结果，不自行豁免；
- Acceptance 聚合真实 probes，但不能把 N/A 当 pass；
- threshold 是 review trigger 或明确 hard policy，不能靠文件拆分伪装健康；
- fresh-context Reviewer 的判断与 deterministic Harness observation 分开保存。

## 8. 实施任务

| ID | 任务 | 依赖 | 规模 | 验收 |
|---|---|---|---|---|
| H-01 | 全量模块 Keep/Move/Generate/Deprecate inventory | F0 | M | 每文件/目录有 owner 和 consumer |
| H-02 | 统一 check status/error/limit vocabulary | PC-02 | M | Node/Python exact fixture |
| H-03 | Platform Core conformance runner | PC-06–08 | M | valid/adversarial matrix |
| H-04 | VerificationRequest/Receipt dispatcher | PC-04 | L | immutable digest/receipt |
| H-05 | existing acceptance adapter 封装为 observations | H-02–04 | L | 不丢 raw Artifact/N-A |
| H-06 | security/architecture/governance 目标目录 facade | H-01 | M | 旧命令仍通过 |
| H-07 | canonical schemas/goldens 迁到 `contracts/` | H-01, PC-01 | XL | 单 normative source |
| H-08 | scaffold 迁到 `tools/` | H-01 | L | fresh/upgrade fixtures 不回归 |
| H-09 | live producer/projector 迁给 owner | H-01 | XL | Harness 只消费 Artifact |
| H-10 | 删除重复业务 evaluator | H-07–09 | L | conformance/acceptance 仍独立 |
| H-11 | Harness 自身 package/coverage/complexity review | H-06 | M | 无新 God dispatcher |

## 9. 迁移波次

### Wave H0 — Inventory only

不移动文件，冻结分类和 compatibility tests。任何新 Harness 文件必须声明 category、owner、input、output 和 authority boundary。

### Wave H1 — Unified receipt

现有 gate/check/accept 输出同时适配 CheckObservation/VerificationReceipt，旧文本输出保持。

### Wave H2 — Canonical source extraction

从最小 Platform Core v1 wires 开始迁 schema/golden；每次只迁一个 contract family，Go/Rust/Harness golden 全绿后继续。

### Wave H3 — Tooling extraction

迁 scaffold/upgrade/producers，Harness 留 compatibility facade 和真实 acceptance test。

### Wave H4 — Delete duplication

只有 consumer 已迁、fixture/receipt 等价、两个兼容窗口结束后，删除重复 evaluator 和旧入口。

## 10. 测试矩阵

- conformance：valid/malformed/duplicate/unknown/oversize/canonical mismatch；
- adapter：tool exists/missing/version mismatch/timeout/signal/large output；
- receipt：tampered request/artifact/observation、partial check、N/A；
- isolation：path traversal、symlink、environment leak、secret output、process cleanup；
- compatibility：现有 `forge gate/check/accept` 文本和 exit policy；
- failure injection：Harness kill、disk full、output limit、workdir cleanup；
- product E2E：Control immutable request → Harness receipt → Reconciler join；
- independence：故意损坏 App/Control read model 后 Harness 仍能验证固定输入。

## 11. 完成条件

- Harness 只拥有验证与适配，不拥有产品 workflow state；
- canonical schema 只有一个 normative source；
- security-critical checker 仍是独立实现；
- scaffold/live producer/product projector 已有明确 owner；
- VerificationReceipt 能被 Control 消费但不能自授完成；
- `forge gate/check/accept` 兼容且 N/A 语义不回退；
- Harness 自身测试、架构、安全和完整 acceptance 全绿。

代码量下降不是单独成功指标；真正成功是边界清晰、独立性保留、重复语义减少且验证强度不下降。
