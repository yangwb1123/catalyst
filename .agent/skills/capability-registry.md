# Capability Registry — exact declared-resolution adapter

## 职责与触发

ADR-0068 只交付 `forgeos.capability-registry/v1` 的单例、只读、authority-neutral 验证与解析。任务需要验证明确提供的 Registry bytes，或把一个明确的 capability reference 与当前单例声明精确比较时，使用本 Skill。当前唯一 key 是 `local-go-package-impact-prescan/1`，Registry digest 是 `23b9acd4133598cd1404c78c71f694b4a99c398652e95c21896a507be5ecacf4`。

## 输入契约

- 只接受 caller 显式给出的 compact canonical JSON；不搜索 repository、环境、catalog 或 plugin。
- `forge capability-registry validate --registry FILE|-` 只验证 exact frozen Registry。
- `forge capability-registry resolve --registry FILE|- --request FILE|-` 要求恰好一个输入来自 stdin。
- `expected_contract:null` 只比较 ID、opaque version 与 digest；非空时只允许 v1 singleton，并必须先与 reference 投影一致；仅当 digest 已匹配时才要求与 Registry contract 字节完全一致。
- `catalog_binding:null` 表示当前 foundation capability 不归属 planning catalog package；它不是 coverage 豁免。

## 执行 SOP

1. 先验证 Registry exact canonical bytes、完整 digest chain 与 singleton physical profile。
2. 按 ID → opaque version → contract digest → optional exact contract bytes 的固定顺序解析。
3. 只输出四种 declared-resolution outcome；不做 SemVer、latest、alias、fallback 或 implementation preference。
4. `repository-reader/1` 的 64 个 `8` 是历史 opaque 引用；当前只按普通未注册 ID 返回 negative assessment，绝不特殊授权。
5. 物理检查只读取 Registry 明确声明的 refs，并拒绝 symlink、special file、集合缺漏与 pre/post identity drift。

## 输出与权限边界

正结果严格是 `RESOLVED_DECLARED_CAPABILITY_REFERENCE_ONLY`。它不认证 Registry/owner，不激活 Grant/PDP，不构造 CapabilityInvocation，不评价 rule/gate/proof/permission，不选择或执行 implementation/test，不加载 plugin，不做 runtime routing、persistence、transition 或 effect。

`docs/design/ai-engineering-os/capability-catalog.v1.yml` 仍是 `planning_only` 的 140-item catalog。Registry v1 不投影该 catalog，不生成 catalog→package adapter，也不证明 catalog completeness。`resolved_exact` 不是 availability、PASS、authorization、permission、invocation 或 authority。

## 自动化与验收

- Schema：`docs/contracts/capability-registry-v1.schema.json`
- Golden：`docs/contracts/fixtures/capability-registry-v1.json`
- Python checker：`python3 -B harness/capability_registry_contract/check.py --golden REPO_ROOT`
- Go：`go test ./internal/capabilityregistry`
- 最终完成权只属于 `forge accept`；shadow detector 与解析结果都不能替代 completion authority。
