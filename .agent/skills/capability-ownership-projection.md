# Planning Capability Ownership Projection — exact declaration adapter

## 职责与触发

ADR-0069 交付 `forgeos.planning-capability-ownership-projection/v1` 的 bounded、planning-only pure projection。需要核对 caller 明确提供的 catalog 与 ownership mapping 是否覆盖 140 个 unique fine capabilities、每项是否恰有一个 declared primary owner，或需要派生逻辑 `.agent/skills/<owner>.md` locator 时使用本 Skill。

本 Skill 是治理检查入口，不是 mapping 中 38 个 package 之一，不拥有其中任何 fine capability，也不表示那些 package、portable `SKILL.md` 或 host adapter 已存在。

## 输入契约

- 产品 CLI 只有 `forge capability-ownership project --catalog FILE|- --mapping FILE|-`；两个 option 可交换，且 exactly one（恰好一个）输入必须来自 stdin。
- projector 只消费这两个显式 source 的 exact bytes；不得搜索 ambient repository、环境、Registry、plugin 或 fallback path。
- catalog 与 mapping 保持 `planning_only`、`executable:false`。raw source SHA-256 只绑定内容，不认证作者、owner、repository currentness 或 provenance。
- Python `validate` 与 `--golden` 是 universal/internal checker surface，不是产品 `forge` CLI。

## 执行 SOP

1. 先按 ADR-0069 frozen YAML profile 验证完整 source stream、closed shapes、bounds 与 exact source hashes。
2. 全量重算 17 nodes、145 occurrences、140 unique capabilities、38 packages 与 140 bindings；catalog unique capability set 必须与 mapping includes set 完全相等，且每个 capability 恰有一个 primary owner。
3. 为每个 binding 派生 `.agent/skills/` + `owner_skill` + `.md`，同时固定 `physical_resolution:not_performed` 与 `skill_availability:not_evaluated`；不得 stat、open、parse 或生成目标。
4. 重建 source → request → binding → projection digest chain，并逐字节比较 compact canonical output。
5. 参数错误返回 2；input 或 semantic rejection 返回 1。所有 argument/input/semantic validation 都在第一次 stdout write 前完成，所以这些失败 stdout 为零字节。成功输出 exact canonical projection 加一个 LF。
6. 底层 stdout short/write error 返回 1；stream 不能事务化，已经写出的 partial bytes 不是有效 canonical artifact，调用方必须丢弃。

## 输出与权限边界

唯一正结果是 `PROJECTED_PLANNING_CAPABILITY_OWNERSHIP_ONLY`。它只证明 supplied planning declarations 在 frozen profile 下形成 complete unique declared ownership 与 logical adapter refs。

它不解析或声明任何 adapter file、Skill/package、`SKILL.md`、`agents/openai.yaml`、script、asset 或 implementation 存在；不修改 ADR-0068 singleton Registry，不认证 owner/authority，不激活 Grant/PDP，不构造 CapabilityInvocation，不选择或执行 implementation，不加载 plugin，不做 runtime routing、persistence、transition 或 effect attestation。逻辑 locator 即使碰巧对应仓库中的同名 Markdown，也仍保持 unresolved declaration。

## Source-only 与 scaffold 边界

Universal scaffold 复制 exact catalog、mapping、ADR、Schema、golden、Python pure projector/checker 和 tests。它不从 38 个 declared owner names 生成物理 `.agent/skills/*.md`，不复制 Catalyst-only `forge-core` Go implementation，也不把 scaffold 中既有同名 Skill 当作 projection resolution 或 availability evidence。

## 自动化与验收

- Schema：`docs/contracts/planning-capability-ownership-projection-v1.schema.json`
- Golden：`docs/contracts/fixtures/planning-capability-ownership-projection-v1.json`
- Universal checker：`python3 -B harness/planning_capability_ownership_projection/check.py --golden REPO_ROOT`
- Catalyst Go：`go test ./internal/planningownership`
- 最终完成权只属于 `forge accept`；ADR 状态继续是 `proposed`，shadow detector 与 planning projection 都不能替代 acceptance 或 runtime authority。
