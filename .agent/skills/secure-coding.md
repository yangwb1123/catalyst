# Skill: secure-coding

> 把身份、对象、字段、租户、输入、资源和供应链都视为安全边界。

## 职责与触发 (Responsibility & triggers)

用于任何外部输入、认证授权、租户、敏感数据、文件/URL/命令、第三方依赖或外部副作用实现。纯函数也必须保持输入边界契约，但可不启动完整威胁模型。

## 输入契约 (Inputs)

- Actor、资产、数据分类、信任边界、数据流、权限矩阵、依赖清单和滥用场景。
- 身份或数据归属不明时默认拒绝，不以网络位置或调用方自报作为信任。

## 执行 SOP (Procedure)

1. 用 STRIDE/滥用案例识别伪造、篡改、抵赖、泄露、拒绝服务和提权路径。
2. 按认证 → 功能 → 对象 → 数据范围 → 字段顺序授权；输入和输出分别 allowlist。
3. 跟踪不可信数据到 SQL、命令、路径、URL、模板、反序列化和日志 sink。
4. 为分页、批量、上传、解析深度、并发、执行时间、第三方次数和金钱成本设上限。
5. 最小化敏感数据，设计加密、脱敏、保留、擦除、备份和审计；密钥只经 secret broker 引用。
6. 审查新增依赖的来源、维护、漏洞、许可证、构建脚本、锁定和退出计划。

## 输出契约 (Outputs)

- Threat Model、Authorization/Data Classification Matrix、control delta、安全测试和残余风险。
- `security_tenancy_privacy` 决策记录；critical/high finding 必须阻断并独立复审。

## 规则、禁止与权限 (Rules & boundaries)

- 禁仅凭角色判断资源访问、自动绑定敏感字段、记录 secret/PII、信任第三方响应、自制密码算法。
- 禁默认 fail-open；解析失败、Reviewer 输出畸形或依赖鉴权不可用时进入 UNKNOWN/拒绝。
- 安全扫描、生产数据访问和攻击性测试必须受任务、环境和授权范围限制。

## 自动化与验收 (Automation & acceptance)

- 运行 secret/SAST/SCA、对象与字段授权、注入/SSRF、资源消耗、依赖失败和租户隔离测试。
- 验收要求：信任边界与授权可验证、敏感数据最小化、资源成本有界、未执行检查明确阻断相应高风险就绪。

## 直接参考 (References)

- `docs/design/ai-engineering-os/backend-decision-standard.md#安全租户与隐私`
- OWASP/NIST 官方来源由 task Context 按需装载。
