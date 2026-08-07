# ADR-0036 — Wave-parallel 调度存储与编排(v20–v22)

- 状态:已接受(2026-08)
- 关联:ADR-0031(被动 successor 候选)、ADR-0032(effectful dispatch)、
  ADR-0035(拓扑就绪选择)、Sprint 66–76

## 背景

ADR-0035 允许同 wave 的多个后继节点拓扑就绪;但存储层仍残留串行链的
per-run 一次性墙,wave 并行在三个层面死锁:
1. successor_candidates 表 `graph_run_id UNIQUE`(v17,每 run 仅一个候选)
2. provider-request 表列级 `UNIQUE(graph_run_id)` / `UNIQUE(schedule_id)`
   (v18,每 run 仅一个 provider request)
3. dispatch lifecycle 表列级 `UNIQUE(graph_run_id)`(v16,每 run 仅一个
   lifecycle)

架构评审(Stage 01)将其列为 High 缺陷(Findings 1a/1b),并指出候选层
身份检查(`reject_existing_candidate_identity`)仍按 run/schedule 一次性
拒绝(声明了 v20 语义却未实施)。

## 决策

### v20(候选 per-node 槽位)

- successor_candidates 表重建:移除 `UNIQUE(graph_run_id)`(每 run 唯一),
  保留 `UNIQUE(graph_run_id, node_id, attempt)` 与
  `UNIQUE(schedule_id, execution_ordinal, attempt)`。
- 身份检查同步:run/schedule 级一次性检查移除,per-node/per-ordinal/
  per-request 槽位保留。

### v21(零 receipts 候选)

ADR-0035 的证据绑定:候选只携带**直接前驱**的 receipts;同 wave 兄弟
(空直接前驱集)的候选携带 0 条 receipts。v20 CHECK
`predecessor_receipt_count BETWEEN 1 AND 31` 放宽为 `0 AND 31`;Go
`buildSuccessorRequest` 过滤 receipts 到 direct predecessors;Rust
`predecessors_valid`/`ordinal_slot_valid`/`predecessor_count_valid` 对齐
(required 覆盖为核心,receipts 数量 0..=31)。结构签名与 v20 相同(CHECK
文本不参与结构 digest)。

### v22(effectful 多节点 dispatch)

- provider-request 表:移除列级 `UNIQUE(graph_run_id)` /
  `UNIQUE(schedule_id)`;per-node 表级槽位(既有)保留。
- dispatch lifecycle 表:移除列级 `UNIQUE(graph_run_id)`,新增表级
  `UNIQUE(graph_run_id, node_id, attempt)`。
- store 适配:
  - provider-request 身份检查移除 run/schedule 一次性检查(与候选层
    对称);
  - run-binding 校验改为**多行遍历**,每行用自身 contract **轻量解码**
    (不触发 run 重查,否则递归回本校验器 → 栈溢出);
  - lifecycle run-binding 同。
- 迁移为单 batch(provider + lifecycle 同一条 SQL 串),保持 schema 链
  单 batch/版本约束。

### 编排命令(Go + Rust)

- `forge graph-scheduled-ready-nodes`:输出拓扑就绪节点清单(JSON)。
- `forge graph-scheduled-node-contract --target-node NODE_ID`:为指定节点
  生成候选(物化)。
- `group graph run scheduled-contract wave-admit GRAPH_RUN_ID
  --schedule-sha256 … --predecessor-receipt …`:一次计划 → 逐节点物化 →
  admit 落库。

## 后果

- 一个 Graph Run 可承载:多候选(每节点一槽)、多 provider-request(每节点
  一槽)、多 dispatch lifecycle(每节点一槽)—— wave 并行的存储层完整。
- 失败语义保持 fail-closed:per-node 槽位冲突仍拒绝;binding 校验遍历
  全部行,任何一行损坏即拒绝 run 读取。
- 真执行(dispatch execute 并发)仍需要 OpenAI Responses 端点;LiteLLM
  转译的 id 漂移(defect)使成功路径在本环境不可达(见
  docs/external-resource-verification.md)。
- 迁移链:v17→v18→v19→v20→v21→v22,全部数据保留;旧库按版本逐个升级,
  结构 digest 链锁死(每版本签名固定)。

## 备选方案与拒绝原因

1. **保留 per-run 墙,wave 仅限候选层**:effectful 多节点 dispatch 不可能,
  与 ADR-0032"管线 ordinal-agnostic"的声明矛盾 —— 拒绝。
2. **v22 分两个迁移 batch(provider、lifecycle)**:schema 链假设每版本一
  batch(load_expected_schema 按 batch 数遍历),两个 batch 使版本计数
  越界 —— 合并为单 batch。
3. **binding 校验复用调用方 contract**:多 request 场景下传入的 contract
  是 initial 节点的,校验错对契约且递归回 run 检查 —— 栈溢出(实测),
  改为每行轻量自载。
