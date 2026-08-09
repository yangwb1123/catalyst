# ADR-0040: Honest coverage enforcement (draft)

- Status: **Proposed (draft — v2)** — 待 CTO 综合裁决 + HUMAN APPROVAL 后生效
- Date: 2026-08-09
- Supersedes: `.agent/eval/acceptance.schema.yml` 中 coverage 判据的
  `required: false`(未接线)状态;`modes.yml` 阈值声明不变。

## Context

`forge accept` 的 acceptance 判据列表中,coverage 长期是唯一「声明了但从不执法」的项:

- `.agent/policies/modes.yml:48/69/87` 声明 `coverage_threshold 0(explorer)/ 60(balanced)/ 80(engineering)`;
  80(engineering 档);
- `.agent/eval/acceptance.schema.yml:45` 却把 coverage 判据设为 `required: false`,
  注释明言「no coverage tool wired」;scan(2026-08-08, thorough)确认阈值存在但
  **从未有任何 gate 读过它们**。

这制造了一个诚实性缺口:「有工具则真查,缺则诚实 N/A」是 AGENTS.md 硬闸门的明文;
阈值声明了六个月的指标,却既没有被查,也没有被「缺工具」诚实标注——它只是静默不执法。
这导致 coverage 这项指标在 ForgeOS 自身及其治理仓里都处于「声明 ≠ 现实」的漂移状态。

## 2. 决策(proposal)

采纳 **honest coverage enforcement**,逐仓、按工具可得性分层:

1. **保留探针式实现**:coverage 判据走真实适配器探针(如 pytest-cov 之于 Python)。
   探针**可运行** → 实际阈值比较(阈值按 `mode × lifecycle` 从 modes.yml 读取);
   探针**不可运行**(无工具 / host 缺依赖 / 概念不存在)→ 如实报 N/A,并附原因,N/A
   既不算 PASS 也不计「已满足」——与 lint 现有语义完全一致。
2. **阈值缺省且保守**:默认行为 = 只有「可测得且低于阈值」才 FAIL;任何「测不到」都
   不因 N/A 放行 load-bearing 业务(不改变 SIX 载重判据集合,coverage 仍非 load-bearing)。
3. **明确口径**:ForgeOS 自身是 polyglot 仓,coverage 探针只对**被测语言**(如
   Python harness)行使,不存在对应工具的 Go/Rust/Node 路径如实 N/A 并列出原因;
   「跨语言无条件 60/80%」不得臆造。
4. **与 mode/lifecycle 联动**:阈值来自 modes.yml 中枢旋钮(explorer 0 / balanced 60 /
   engineering 80),不得在 acceptance 内再写死。

反对方(记录):接入后,若某语言工具在 CI 与本地行为不一致,可能出现「本地绿、CI 黄」的
新波动;裁定 = 阈值仅诚实读数,不升级为 admission blocker(与 lint 现状一致),任何偏差
都须走 secret-scan 相同「真实探针、N/A 显式」纪律,杜绝伪 PASS。

## 3. 链接

- 缺口来源:`docs/design/gap-analysis-2026-08-09.md` G4b;
- 上游契约:`harness/adapters/{py,go,rs,ts}.yml` 探针路径(执行由 harness 主循环负责);
- 同步修改:`.agent/eval/acceptance.schema.yml`(coverage 判据 required 语义与 N/A 规则)、
  `harness/acceptance.mjs` 接线、`.agent/policies/modes.yml`(如有阈值微调)。

## 4. 后果

- 正面:coverage 指标从「声明但静默」变「可审计」;符合「有工具则真查」红线;audit 只需
  看一个 harness 输出。
- 负面:接入成本(探针适配 + golden 测试),以及 CI/本地读数差异引入的审查噪音;均可被
  保守阈值与 N/A 语义吸收。
- 批准路径:CTO 综合裁决(成本/风险)→ HUMAN APPROVAL → implementer 落地 → accept 复验。
  未批准前维持现状(诚实 N/A),不列为缺陷。

---

_状态机:DRAFT → CTO review → Approved/Rejected。实施后更新本文件 Status 并链接 fix commit。_