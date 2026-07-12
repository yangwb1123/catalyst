已保存到 `docs/requirements/2026-07-12-five-post-scan-architectural-extension-directions.md`（16.9 KB）。

几点观察供你参考：

**文档定位清晰**——列出了 13 个「饱和领域」明确声明不重复，确保每个方向确实从不同角度切入。这在已经 ~200 篇分析的仓库里是必要的自律。

**五个方向的代码证据质量高**，我特别看重：

| 方向 | 最强证据点 |
|------|-----------|
| **一 · 错误灰色地带** | `classifyRunErr` 的 `default → KindFailed` 将资源耗尽类错误硬塞进「不可重试」——这是最真实的 silent degradation 入口 |
| **二 · project.yml schema** | `mode.go` 零校验 + 零值容忍策略被直接定性为 TCB 缺口——确实，lifecycle 拼写错误可以静默 bypass production 覆盖 |
| **三 · Trace 格式进化** | `_format` 字段写死 `v1` 且消费者从不检查——格式演进能力实质为零。`mode_snapshot` 的缺失是仿真的前提条件缺口 |
| **四 · 红线自动门控** | `arch-check` 不在 CC hook 路径上（只有 `gate.mjs`）——agent 可以在编辑循环中不断犯戒直到 `forge accept` 才发现 |
| **五 · 子进程最小权限** | `buildEnv()` 调用 `os.Environ()` 全量继承——API key + SSH agent 全部透传给子进程，这是最小权限原则的明显缺口 |

**优先级收敛建议合理**——方向二（project.yml schema）确实成本最低收益最高，适合作为零信任治理的基线。方向二+四+一覆盖了「配置可信·红线自动·错误可诊断」的三角基底，是合理的 Sprint 排列。

有一个细微的补充点：方向二的建议方向 2 提到「未知 lifecycle 默认 production 略过零值」——这个默认策略虽然安全，但与当前 `mode.go` 的零值「全开向后兼容」设计意图冲突。建议在 ADR 中记录这个转向，因为它是从 **fail-open**（容忍错误配置继续运行）到 **fail-closed**（错误配置阻止运行）的设计哲学迁移。
