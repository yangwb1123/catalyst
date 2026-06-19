# ForgeOS — Current Sprint

## Sprint 1–4 (✅ 完成)
- **S1 声明式治理层**:9 agent / 7 skill / 4 workflow / mode 矩阵 / 路由 / 评估 schema / 适配器 / BOOTSTRAP / 2 ADR;gate 切 `block`。
- **S2 验证脊柱**:多 agent 跑通 `build.yml`(plan→impl×2→gate→reviewer→fix);消除 SoT 漂移。
- **S3 Evaluation Stop 闸门**:`forge accept` 聚合判 ACCEPTED/REJECTED,诚实标 n/a。
- **S4 Dogfood 真实应用**:`examples/url-shortener` 经完整 pipeline(architect→3 implementer→fresh reviewer→fix)端到端建成;reviewer 揪出"app 未被 accept gate 覆盖"并补全;现 39 app 测试被 `forge accept` 实际执法。

## 🏁 里程碑:ForgeOS 工厂已证可用
v0+v1 完成 + **首个真实产品被端到端造出并自我治理**。harness 三工具(gate/check/accept)+ 28 自测(check 12 / gate 8 / accept 8);app 39 测试;整仓 `block` 模式全绿。

## 下一前沿(需你定向)
- **A** 多语言验证:用 Python/Go 再造一个小应用,验证 polyglot harness 适配器(适配器已声明、未在真实代码上跑过)
- **B** v2 groundwork:真实 forge-core 编排器(本环境有 Go 无 YAML stdlib / 拉模块要网络的摩擦,需先解决)
- **C** 给 url-shortener 加真实功能(持久化 / 自定义别名 / 点击统计 / 过期)
- **D** 以此里程碑收尾

**stop_condition:** roadmap 完成度 / 闸门全绿(非「继续 N 轮」)。
