我现在掌握了全局。这个文档已经存在于 `docs/requirements/` 中，并且 `.out.md` 有一个完整的代码接地验证报告。让我提供一个独立的综合评估——既和原文对话，也和验证结果对话。

---

## 独立综合评估

我已经阅读了原文（~500 行）和验证报告（~300 行）。以下是三个层次的反应：

### 層次一 · 核心论点我同意（这是新缺口）

原文的方向判断——ForgeOS 对**自己的**代码质量极其严格，但对**它生产的**代码（示例、starter）没有同等标准——是成立的。验证报告修正了一些误判（url-shortener 的 URL 验证、starter 的测试覆盖），但**没有推翻任何一个方向的核心命题**：

| 方向 | 验证后核心命题仍成立? | 我的判断 |
|------|:---:|---------|
| ① 工厂输出质量 | ✅ | go-taskd `main.go` 的 `log.Fatalf` + 无 `Shutdown` + 无信号处理是无可辩驳的缺失 |
| ② 架构模式不一致 | ✅ | 两个示例确实有不同模式，且没有正式的 ForgeOS 应用模式文档 |
| ③ CI 管线语义 | ✅ | 只有 `build` 被 dry-run，`discover/design/review/evolve` 零覆盖 |
| ④ Starter 质量 | ✅ | 虽有测试覆盖但论据有偏差。不过核心论点（starter 不展示 ForgeOS 的完整价值）仍成立 |
| ⑤ 多语言覆盖 | ✅ | 四语言目标 Gap 真实，只是引用来源需从 ADR 0002 改为 DECISIONS.md D1 |

### 層次二 · 验证指出的 7 个事实错误有一半需要原文撤回

验证报告发现的 7 个分歧中，**以下 3 个是实质性错误**：

1. **`url-shortener` URL 验证**: 原文声称"无验证、`javascript:alert(1)` 被接受"——实际代码有 `ALLOWED_PROTOCOLS` + `isValidUrl` + `canonicalizeUrl`，运行验证拒绝两种攻击。**这是个事实错误，应撤回。**

2. **ADR 0002 引用**: 该文件不存在，四语言栈声明在 `DECISIONS.md` D1。**这是引用错误，应修正。**

3. **dry-run 等价于"echo binary compiles"**: 原文低估了 dry-run 的语义——它实际遍历 phase state machine、mode-gating、agent-to-tier 路由。**表述应修正。**

以下 2 个是**程度偏差**（方向正确但夸大）：

4. **Starter "零代码"**: 实际有 `greet()` + 测试，但确实是占位符级别。**"零代码" 改为 "minimal placeholder"**。

5. **Starter 测试缺失**: `test_forge-init.mjs` 已经在生成的项目上跑 `forge accept`。**"零项目专属测试" 不准确。**

### 層次三 · 我看到的额外盲点——方向① 可能低估了 server.go 的质量

验证报告发现了一个被原文忽略的亮点：`go-taskd/internal/httpapi/server.go` 有 `maxBody`(1 MiB)、`recoverMiddleware` (panic→500)、`json.Decoder` + `MaxBytesReader` + `DisallowUnknownFields`。这意味着：

**方向①的真正靶心不是"示例代码质量差"，而是"composition root 没有生产就绪"。**

- `internal/httpapi/server.go` 的内部代码是不错的
- `internal/service/service.go` 的领域逻辑也不差
- 问题集中在 **`main.go` 的 10 行代码**：`log.Fatalf` + `http.ListenAndServe` + 无 `signal.Notify`

这意味着方向①的修复成本比原文估计的更低——不需要重写整个示例，只需要改 **~20 行 main.go**（添加 signal.NotifyContext + http.Server.Shutdown + /health 端点）。

### 优先级修正建议

基于验证结果，我建议的调整：

| 方向 | 原优先级 | 建议 | 原因 |
|------|:--------:|:----:|------|
| ① 工厂输出质量 | P1 | **维持 P1** | 核心论点成立，且修复成本极低（~20 行 main.go）— 性价比最高 |
| ③ CI 管线语义 | P1 | **维持 P1** | 关键回归风险。但 dry-run 实际做的工作比原文说的多 |
| ② 架构模式不一致 | P2 | **维持 P2** | 方向正确但非阻塞 |
| ④ Starter 质量 | P2 | **降为 P3** | `test_forge-init.mjs` 已经验证了 forge accept 通过。改进 starter 是产品选择而非 bug 修复 |
| ⑤ 多语言覆盖 | P3 | **维持 P3** | 长期路线图事项 |

### 最该立刻做的三件事

1. **修复 `examples/go-taskd/main.go`**（~20 行）：`signal.NotifyContext` + `http.Server.Shutdown` + `GET /health`——这是验证报告同意的最高性价比改动。

2. **在 CI 中添加 `forge run discover --executor dry` / `forge run design --executor dry`**：这是原文方向③的最低成本第一步——5 行 YAML 让 3 个未覆盖的 workflow 进入回归保护。

3. **修正本文档中的 3 个事实错误**（url-shortener URL 验证、ADR 引用、dry-run 表述）并重新提交。

---

需要我执行这三件事中的任何一件吗？最直接的是（3）——我可以在本文中做出精准的修正。或者如果方向是行动，我可以开始（1）的代码改动。
