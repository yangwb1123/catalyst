验证完成。已输出至 `docs/requirements/2026-07-11-five-genuine-systemic-boundaries.out.md`。

## 验证结论速览

| 维度 | 结果 |
|------|------|
| **5 个方向核心洞见** | 全部有效 — 文档对"哪里有问题"的判断准确 |
| **代码证据准确性** | ⚠️ 方向一有 **3/5 引用失准**（最严重：gate.go 的 `ProbeAll` 代码示例完全伪造） |
| **Novelty 声明** | ✅ 方向一/二/三/五零命中；⚠️ 方向四声称零命中但实际有 2 篇侧栏提及 |
| **最稳健的方向** | ③ 上下文预算 + ⑤ 产品遥测 |
| **最需修正的方向** | ① 门拓扑 — Go/JS 代码示例需全面重写 |

### 关键发现

1. **`gate.go` 的 `ProbeAll` 完全不同**——文档说是 `for range` 逐个遍历，实际是**单次 `spawnSync` 调用 `acceptance.mjs --json`**，解析 JSON 返回。顺序性问题实际在 JS 层（`spawnSync` 同步阻塞），不在 Go 层。

2. **命名/映射多处错位**——`probeArch()` 实际跑的是 `check.py`，`probeArchitecture()` 才跑 `arch-check.mjs`。文档的 8-probe 列表实际是 10+。

3. **方向二运行时复杂度被高估**——mode policy 在 Go 中**硬编码为 `baseline` 表**（`mode_policy.go`），`modes.yml` 主要被 `validate/migrate` 等静态工具消费，不是运行时动态从 YAML 解析。

4. **方向四 novelty 声明需调整**——"零命中"应为侧栏 2 处提及（task injection / 模板注入），非独立方向展开。

5. **方向三 memory 描述部分过时**——当前 `boundMemory()` 已在 `len > 32` 时做 recency+relevance 混合排序，不再是纯"最新 32 条"。
