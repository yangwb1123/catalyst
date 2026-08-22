# ADR Governance

## 职责与触发

当设计需要记录一个非显然的架构取舍、长期约束、跨 Sprint 决策，或明确后续 validation/revisit 义务时，使用本 Skill。ADR-0067 只交付“新建 Proposed ADR v2”边界：既有 ADR 不做 v2 解析、retro-validation 或迁移；universal checker 不扫描 repository，Go `writes_adr` 仍保留既有 baseline integrity snapshot。Accepted、Rejected、Superseded、Deprecated 的状态机与 compliance 仍是后续能力。

ADR 记录一份声明性决策提案，不证明事实正确、实现符合、审批有效或影响分析完整。owners 是 caller/author 声明的责任引用，approvers 是所需复核者引用；二者都不是认证 principal、ApprovalRecord、SoD 或授权。

## 输入契约

- 明确的四位 ADR sequence 与规范文件名 `ADR-NNNN-lowercase-hyphen-slug.md`；
- caller-declared owner/approver、scope、Claim/Evidence、affected Graph node、implementation references；
- 可读的 Context、Decision、Consequences、Validation、Limitations 正文；
- proposal time，以及适用时的 expiry、supersedes、assumptions、risks 与 revisit triggers。

Claim/Evidence ID 只按 ADR-0045 identifier 语法记录，不读取或认证 record；`graph-node-<sha256>` 只按 ADR-0065 identity 形状记录，不解析 GraphSnapshot 或推断 coverage。空 affected-node 集表示“未声明/未知”，绝不能解释为 no impact。

## 执行 SOP

1. 仅创建当前 `writes_adr` attempt 允许的一个新文件；不得修改、删除、重命名或补写基线 ADR。
2. 文件使用 LF-only 严格 framing：`---`、一行 exact compact canonical JSON、`---`、一个空行、随后 exact Markdown body。禁止 BOM、CR、YAML alias/tag、pretty JSON、duplicate/unknown key 或尾随 framing 数据。
3. frontmatter 固定 `status=proposed`；`accepted_at_unix_ms` 与 `acceptance_id` 必须为 null，`superseded_by` 必须为空。不得从 `.approved`、CLI flag、actor hint、local marker 或 human gate 猜出 approval。
4. 令 `adr_id`、文件名前缀和正文第一行的 ADR number 完全相同；正文 title 必须与 frontmatter title 完全相同。正文必须包含且只按合同顺序表达 Context、Decision、Consequences、Validation、Limitations。
5. set-like arrays 必须已按 raw UTF-8 bytes 排序且唯一；叙述性数组保持作者顺序。Validation owner 必须存在于 declared owners。
6. 先对 exact body bytes 计算 domain-separated `body_sha256`；再把 frontmatter 的 `self_sha256` 置空，对 canonical frontmatter、NUL 与 exact body 计算 domain-separated self digest。不得用文件名、Git hash或普通 SHA 替代冻结 preimage。
7. 用 strict semantic checker 重建完整 document；Schema-only PASS 不够。任何 shape、bounds、reference、heading、order、digest 或 filename drift 都必须在 artifact/receipt/accepted output 发布前失败关闭。
8. 在 handoff 中明确哪些 owner/approver、Claim/Evidence、affected node、validation 与 revisit 信息仍未解析或认证。

## 输出契约

成功输出一个 exact `forgeos.architecture-decision-record/v2` Proposed Markdown document。唯一正面含义是：该文件满足 proposed-only v2 的 framing、结构、引用形状、排序、bounds 与 content digest 合同。

它不表示 ADR 已 Accepted/Rejected/Superseded/Deprecated，不产生 ApprovalRecord、TransitionReceipt、Grant、permission、truth、Graph edge、Architecture Compliance、implementation completion、persistence authority 或 effect。

## 规则、禁止与权限

- 禁止原地修改 accepted legacy ADR，或把旧 ADR 静默“升级”为 v2。
- 禁止把 author-declared owner/approver 当作 authenticated identity、authority 或 separation of duties。
- 禁止把 local positive approval marker、`--approved`、human gate、actor hint、时间窗口或 hash PASS 当作 ADR acceptance。
- 禁止解析 Claim/Evidence/Graph references 后补写 frontmatter；本切片只验证 exact reference shape。
- 禁止从空 reference 集推断无证据、无风险、无影响或无需审批。
- 禁止省略 rollback、validation、limitations 或 revisit 义务来制造 ready/complete 结论。
- Universal checker 只读显式文件或 golden；不得扫描并拒绝既有 ADR。Go `writes_adr` runtime 只验证当前 attempt 唯一新增文件。
- Scaffold 只复制合同、golden、Python checker/tests、Skill 与治理元数据；不安装 Catalyst-only Go host，不迁移项目 ADR，也不生成 approval/state。

## 自动化与验收

## ADR-0074 portable Proposed-document validation branch

从 repository root 执行以下 exact argv：

```bash
python3 -I -B skills/adr-governance/scripts/check_package.py
python3 -I -B skills/adr-governance/scripts/validate_declared_proposed_adr.py ADR-NNNN-slug.md < DOCUMENT.md
```

第二条命令只接受 exactly one caller-supplied lexical basename，并把 stdin 原始文档读到
explicit EOF。该 basename 仅是独立 lexical label，不证明物理文件、repository path 或 identity；
从 frontmatter 推导它会使 filename binding 失真。Portable branch 不扫描 repository、不 author、
repair、reseal、accept、supersede 或 persist ADR，不复制 Go `writes_adr` runtime，也不安装 host Skill。
只有 exit 0、empty stderr 与 exact authority-neutral marker + one LF 才是成功；`-I/-B` 与 checker
不认证 interpreter、system site、host 或 publisher，也不提供 atomic check-to-use。

```bash
python3 -B harness/architecture_decision_record_v2_check.py --golden <repo-root>
python3 -B harness/architecture_decision_record_v2_check.py --file <ADR-v2.md>
cd forge-core && go test ./internal/adrv2 ./cmd/forge
```

验收必须覆盖 canonical/framing/BOM/CR/duplicate/unknown/float/Unicode、文件名/ID/heading/title、body/self digest、sorted-unique refs、declared validation owner、proposed-only null/empty fields、bounds、symlink/hardlink/TOCTOU、artifact/receipt/Observe 前失败，以及 old ADR 只参与既有 baseline integrity snapshot、从不被当作 v2 解析或 retro-validate。最终完成权威仍仅属于 `forge accept`。
