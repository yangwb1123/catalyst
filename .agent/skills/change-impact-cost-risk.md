# Change Impact Cost Risk

## 职责与触发

当调用方已经持有一份 exact canonical ADR-0062 七字段 request，需要离线重建
selected Go module 内的词法 package 反向依赖预扫描时，使用本 Skill 的 portable
branch。当前交付只回答 supplied ADR-0053 observation 内的 lexical closure；宽泛名称
不表示完整 Change Impact、Cost、Risk、materiality、safety 或 Assessment。

完整 System Knowledge Graph、多 surface extractor、coverage/staleness、selected build、
API/event、data/migration、deployment/operations、owner/ADR policy、其它语言与 runtime
reachability 仍未交付。`system_impact_status` 必须保持 `unknown`。

## 输入契约

Portable projector 只接受 compact canonical UTF-8 JSON 的既有 ADR-0062 request，且字段
恰为：

```text
api_version
canonicalization
changed_paths
graph_observation_base64url
graph_observation_sha256
request_sha256
run_id
```

调用方必须在 stdin 发送不带 LF 的 exact bytes，随后显式关闭 EOF；上限 24 MiB。
不得传 raw/parsed graph、golden fixture、existing envelope、GraphSnapshot、
ChangeImpactReport、Cost/Risk object、wrapper、tagged union、mode 或 dispatcher。输入不得
排序、去重、补字段、修 digest、repair 或 fallback。

## 执行 SOP

从 repository root 先检查 package，再运行 projector：

```text
python3 -I -B skills/change-impact-cost-risk/scripts/check_package.py
python3 -I -B skills/change-impact-cost-risk/scripts/project_local_go_package_impact_prescan.py < REQUEST.json
```

Checker 可选接受一个 `PACKAGE_ROOT`。检查与使用不是原子操作；检查后必须保护 package
bytes，或在受保护边界内重新检查。Projector 接受 zero arguments，读取至 explicit EOF，
并要求 derived envelope 的 embedded request canonical bytes 与 stdin 每个 byte 相等。

只有 exit 0、empty stderr 和完整 canonical existing envelope 加一个 LF 才是成功。Exit 1、
exit 2、无 EOF、partial stdout、write/flush failure 或其它输出均不得解释为成功。

## 输出契约

成功 envelope 保留既有结果：

```text
LOCAL_GO_PACKAGE_IMPACT_PRESCAN_ONLY (exact ADR-0053 lexical reverse dependency closure; system impact unknown; no selected-build, truth, authority, completion, persistence, execution, or effect attestation)
```

`complete_within_observation` 只表示 supplied selected-module lexical observation 对该 request
没有本合同定义的 local closure gap。零 reachable dependents 或该状态都不表示 system
complete、safe、no impact、low Cost、low Risk、accepted、compliant 或 authorized。

## 规则、禁止与权限

- 不读取 live repository、Git、clock、environment、credential、provider、network、database、
  journal 或 Memory，也不隐式调用 ADR-0053 producer、Go toolchain、build 或 test。
- 不认证 caller、graph、project、repository、host、publisher、interpreter 或 source freshness。
- 不生成 GraphSnapshot、final ChangeImpactReport、Cost、Risk、materiality、gate、
  AssessmentReceipt、Evidence、Claim、Approval 或 authority。
- 不安装 host Skill，不新增 authenticated route、detector projector、runtime profile、
  persistence、transition、execution 或 effect。
- `-I`/`-B` 只约束 Python import search 与 bytecode write；不隔离 system site、stdlib、
  interpreter startup、OS、host 或 publisher。
- Vendoring parity 只覆盖 15 个逐字节锁定的 semantic leaves；四个 lean initializer 不代表
  complete source-tree parity。

## 自动化与验收

Package checker 必须保持 checker-only shadow/non-load-bearing；projector 不得进入 detector。
Portable `skills/change-impact-cost-risk/SKILL.md` 不进入 context routes；本 repository adapter
仅提供 source instruction。验收必须覆盖 exact golden、N/N+1、explicit EOF、16 MiB decoded
Base64URL、48 MiB output、short write、nonblocking writer-open、hostile cwd/PYTHONPATH/modules、
manifest closure/race、source Schema/golden/semantic leaf parity，以及 Python/Go ADR-0062 parity。

