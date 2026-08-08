# 错误模型

## 编排层(forge-core)

`internal/orchestrator/exec_error.go` 定义四类错误:

| Kind | 语义 | 重试 |
|---|---|---|
| `KindConfig` | 永久配置故障(无 Build/空 argv/沙箱缺 runner/未知沙箱类型) | 否 |
| `KindTimeout` | 超时(仅 `context.DeadlineExceeded`) | 是 |
| `KindFailed` | 命令干净地非零退出(含取消 `context.Canceled`—— 调用方裁决) | 否 |
| `KindOverloaded` | 供应商过载(可选 ClassifyOverload 判定) | 是(退避) |

宿主路径与沙箱路径分类一致(`errors.Is(runCtx.Err(), DeadlineExceeded)`
判定超时,禁止消息字符串匹配)。

## 沙箱层(docker/firecracker)

- exit 125(docker daemon 故障)→ `KindConfig`,不冒充 guest 判定。
- guest 非零退出 → `KindFailed`,退出码透传。
- marker 读取瞬时错误 → 有界重试(3 次 × 100ms)后才升级 Config。
- 超时:docker 清理孤儿容器(`docker rm -f`);firecracker 进程组 SIGKILL。

## 运行时层(forge-runtime)

`HubStoreError` 分类:NotFound / Conflict / Corrupt / Unavailable;
协议错误 `ProviderError { code, message, retryable }`(配额/限速/网络/
CLI 横幅,见 `docs/reviews/reviews/*/` 评审中的失败签名表)。

## 文档约定

失败必须 fail-closed(宁可拒绝,不可伪装通过);诚实 N/A 标注不可验证项。
