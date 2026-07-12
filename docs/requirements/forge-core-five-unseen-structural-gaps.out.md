交叉验证完成。结果已写入 `/home/u1/catalyst/docs/requirements/forge-core-five-unseen-structural-gaps.out.md`。

## 关键发现总结

你的五个方向中，**只有 2 个是真实未覆盖的**，另外 3 个的核心缺口已被已有文档识别：

| 方向 | 实际覆盖 | 关键证据 |
|------|---------|---------|
| ① 格式版本写而不读 | ❌ 已覆盖 | `five-verifiable-code-level-gaps.md:223-320` 完全相同论点 |
| ② 双 YAML 解析器 | ❌ 已覆盖 | `forgeos-five-hidden-architectural-gaps-2026-07-10.md:369-438` + `2026-07-11-codegrounded-five-systemic-gaps.md:132-193` |
| ③ 二进制不自包含 | ❌ 已覆盖 | `production-product-gaps-v43.md:100-170` 完整"三运行时门槛"分析 |
| ④ 无语义自校验恢复 | ✅ **真实未覆盖** | 与 `forges-five-hidden-product-quality-gaps.md` 方向一互补（写入保护 vs 读取验证） |
| ⑤ Scorecard IPC 管线 | ✅ **真实未覆盖** | 无一篇已有文档将 scorecard 建模为多进程 IPC 链并分析其可靠性 |

建议的差异化定位：
- **方向①-③** → 缩小为实施路线图深化（"如何修复"而非"发现了什么"），引用已有文档
- **方向④-⑤** → 保留为全新方向，标记与相邻文档的互补关系
