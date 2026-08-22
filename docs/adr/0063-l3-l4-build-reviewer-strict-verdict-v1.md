# ADR-0063 — L3/L4 Build Reviewer Strict Verdict v1

- 状态：已采纳（2026-08；窄合同实现已交付，完整工作树验收证据由当前 sprint 收口）
- 范围：`forge run`（以及 `forge evolve auto` 转交的一次性 Build）的 Build workflow、本地串行 orchestration、
  checkpoint/chain resume、universal scaffold/upgrade 的治理资产传播；普通 Evolve 只继承 materiality 状态绑定，不启用 Build contract
- 关联：ADR-0037、`.agent/workflows/build.yml`、`.agent/agents/reviewer.md`

## 背景

ForgeOS 已有 Build Reviewer 和 `VERDICT:` 文本约定，但历史解析把缺失、畸形或执行器错误当作 advisory 输出继续运行。
这种 fail-open 行为对低影响兼容路径有价值，却不能满足已经由调用方声明为 L3/L4 的 required review：跳过 Reviewer、从其后
恢复、并行绕开定向回修、伪造 provider envelope 或让旧 runtime 忽略新字段，都可能把“没有有效批准”降级成放行。

本 ADR 只关闭这一条本地编排边界。它不把风险声明变成可信事实，也不把 Agent 名称、模型档位、fresh-context 标志或一行
verdict 变成身份、独立性、质量或审批证明。

## 决策

### 1. Materiality 是调用方声明，不是分类权威

CLI 接受一个显式 `--materiality L0|L1|L2|L3|L4`。省略时必须保存为
`materiality_not_bound`，不得从 mode、lifecycle、workflow 名、diff、路径、文件数量、Reviewer 文本或模型输出自动推断。

- `L3`/`L4`：只在 `stage: build` 启用本 ADR 的 strict reviewer boundary；
- `L0`/`L1`/`L2`：保留既有 Reviewer advisory/fail-open 兼容行为；
- `materiality_not_bound`：诚实表示未绑定，仍走既有兼容行为，不得冒充低风险判定。

声明值没有签名、issuer、Policy Decision、Evidence、Approval 或仓库事实绑定。恶意或错误调用方可以低报；该问题留给后续
authenticated classification/Grant/PDP，而不是由本 ADR 隐式解决。

### 2. Workflow 以显式 `reviewer_v1` 选择合同

canonical Build workflow 的 Reviewer phase 必须声明：

```yaml
verdict_contract: reviewer_v1
```

当 materiality 为 L3/L4 时，runtime 必须在启动任一 Agent 前验证 Build workflow 恰有一个 `reviewer_v1` phase。该 phase 必须：

- `agent: reviewer`、`readonly: true`、`fresh_context: true`；
- 位于 `qa_v1` 之前；
- 不得通过 `optional_for` 跳过，不得 `writes_adr`、`emits` 或 `feeds_forward`；
- 失败时 `loop_back` 到一个更早、不可跳过、可写且 `agent: implementer` 的 phase；
- mode policy 即使原本不要求 Reviewer，也必须在本次 L3/L4 Build 中把它提升为 required。

自定义 Build workflow 缺少、多于一个、错位或弱化该 phase 时失败关闭。L3/L4 workflow 初载和恢复重建只走 native
workflow loader；兼容 YAML shim 不能在严格验证前执行。无法识别 `reviewer_v1` 的旧 runtime 必须把它当 unknown contract
拒绝，而不是静默忽略；因此新治理资产要求匹配的新 host `forge` binary。

### 3. Strict verdict 只接受两个 exact final-line token

在 L3/L4 Build 的 `reviewer_v1` phase，语义 payload 的最后一个非空行必须顶格、逐字节等于以下之一：

```text
VERDICT: APPROVE
VERDICT: REQUEST_CHANGES
```

findings 和解释可写在该行之前，但整个 payload 中只能出现一个顶格 exact verdict 行；裁决行之后只允许空行。不接受多个
verdict、裁决行的 Markdown fence、列表符、前后缀、缩进、大小写变体、Unicode 仿形、非法 UTF-8 或 trailing prose。
`REQUEST_CHANGES` 必须按 workflow 的定向 loop-back 回到 implementer；
`APPROVE` 才可继续 QA。

Claude executor 的 raw stdout 还必须先满足其成功 `result` envelope；error/timeout/cancel/partial/malformed/duplicate/trailing
envelope 均失败关闭。custom command executor 不套 Claude framing，但仍必须返回上面的 exact raw token。executor failure、空输出、
dry-run synthetic output、budget/loop-back exhaustion 与 parser error 都不得被转换成批准。

### 4. 所有入口共享同一不可绕过边界

L3/L4 strict Build 只能使用支持定向 loop-back 的串行编排：

- `Run`/`RunFrom` 在验证 materiality、workflow shape 和恢复位置后才可执行；
- `RunParallel` 必须在启动 wave 前拒绝 `reviewer_v1`，因为它不能表达失败后的串行回修；
- 从 Reviewer 之后的 phase 开始必须拒绝，不能用 `--from` 跳过 required verdict；从 Reviewer 本身开始仍会执行该裁决，允许恢复；
- resume 必须绑定 materiality、workflow digest、mode/lifecycle、phase cursor 与资源进度；
- 缺失、旧版本、null、非法或与 CLI 冲突的恢复 materiality 必须在 trace/Agent 启动前拒绝；
- L3/L4 chain 不得把已记录的旧 advisory Build 当作 strict-approved Build 重放。

checkpoint/chain state 的作用是 crash recovery consistency，不是认证。原子发布、bounded regular-file、owner/mode/link/path identity
检查可减少误写和常见别名攻击，但 same-UID caller 或管理员仍可删除、替换或回滚整个 state snapshot。没有外部签名、monotonic witness、
TPM/HSM 或远端 ledger 时，不得声称 tamper-proof 或 authenticated resume。

### 5. 低等级和未绑定路径保持兼容

`L0`–`L2` 与 `materiality_not_bound` 不启用 `reviewer_v1` strict parsing，保留既有 mode skip、advisory output parser
和 fail-open review 行为。workflow 中存在 `reviewer_v1` 声明不等于每次运行都严格；runtime 根据本次 caller-declared
materiality 选择 effective contract。这个执行语义兼容不等于旧持久化格式可恢复：旧 checkpoint/chain state 没有可信的
materiality 与本 ADR 所需 binding，runtime 无法证明它来自低等级运行，因此对所有等级都只允许诊断读取、不得猜测升级或恢复。

这个兼容边界是显式的，不是安全保证：未绑定不代表低风险，低等级也未经本 ADR 认证。需要 strict review 的调用方必须明确
声明 L3/L4，并使用理解本 ADR 的 runtime。

### 6. Scaffold/upgrade 只传播治理资产

fresh init 和 legacy upgrade 必须复制 ADR、`build.yml`、Reviewer role card，并把 ADR 纳入 scaffold ledger。验证至少证明：

- copied Build workflow 包含唯一 `verdict_contract: reviewer_v1`；
- copied Reviewer card 同时写明 L3/L4 fail-closed 与低等级/未绑定 fail-open；
- ADR 如实保留 caller-declared、unknown-old-runtime、非认证边界；
- scaffold/upgrade 不安装或替换 `forge-core`、`forge-runtime`、`forge-kernel` 或宿主 `forge` binary。

因此“文件已被复制”不证明目标项目实际拥有兼容 runtime，更不证明某次变更已经获得有效 review。

## 明确不提供

本 ADR 不提供：

- materiality 的自动计算、可信 producer、签名或防低报；
- implementer/reviewer 的人、进程、模型、provider、credential 或组织身份认证；
- cryptographic separation of duties，或 `fresh_context`/`agent: reviewer` 名称之外的独立性证明；
- finding schema、严重度归一化、ReviewCase、dissent/closure、review quality 或事实正确性证明；
- verdict 对 source revision、diff、ContextPackage、policy、prompt、model、artifact 或 test evidence digest 的绑定；
- human Approval、Grant/PDP、release permission、transition、completion 或 effect authority；
- Review/CTO/Evolve 等其它 workflow 的全面 strict verdict；
- 对同一 UID、root、磁盘回滚、恶意 host binary 或外部 executor 的防篡改保证。

`VERDICT: APPROVE` 只是一条在本地 orchestration 边界内通过语法检查的控制信号。它不能覆盖 harness/QA 失败，也不能独立证明
代码正确、安全、可维护或已获人类批准。

## 兼容性与迁移

1. 新 runtime 读取旧 workflow 时，L3/L4 因缺少 `reviewer_v1` 失败关闭；L0–L2/未绑定保持原行为。
2. 旧 runtime 读取新 workflow 时，unknown `reviewer_v1` 必须失败关闭；若某个更旧的非合规 runtime 忽略 unknown 字段，不能将其
   执行结果作为本 ADR 的证据。
3. 旧 checkpoint/chain state 可用于诊断；由于它既没有可信 materiality，也没有本 ADR 所需 binding，任何等级都不得从旧格式
   猜测升级或恢复。新运行的低等级 review 行为仍按 §5 保持 advisory 兼容。
4. upgrade 更新治理资产与 ledger，不迁移、伪造或安装 runtime/state，也不把历史 advisory verdict 提升为批准。

## 验收判据

- L3/L4 对缺失/重复/错形 Reviewer、mode skip、`--from` 跳过、parallel、dry-run、executor error、malformed verdict 与 loop budget
  exhaustion 均在 QA 放行前失败关闭；
- exact APPROVE 继续 QA，exact REQUEST_CHANGES 定向回 implementer 后重新 review；
- L0–L2/未绑定的既有 advisory/fail-open 行为有回归保护；
- materiality 在 checkpoint/chain、resume 冲突和 workflow digest 变化下不可静默降级或陈旧重放；
- fresh scaffold 与 legacy upgrade 的 ADR/workflow/role/ledger 字节及非 runtime-install 边界有定向 Node 回归。

这些判据只验收本 ADR 的窄编排切片；完整 `ReviewCase`、身份/职责分离、finding closure、digest-bound approval 和治理 Kernel
仍留在后续 Wave。
