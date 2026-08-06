# External Resource Verification — 2026-08-05 local deployment

Both previously BLOCKED-EXTERNAL resources were re-probed and verified on this
host on 2026-08-05. Network, sudo (NOPASSWD), /dev/kvm (ACL-writable for u1),
and local model endpoints are all available.

## 1. Firecracker microVM sandbox — VERIFIED

Deployment:

- `firecracker-v1.7.0-x86_64` downloaded from the official GitHub release and
  installed to `/usr/local/bin/firecracker` (with `jailer`).
- `/dev/kvm` is ACL-writable for the current user (`crw-rw----+`, `+` ACL),
  confirmed by an open-write probe; `sudo` is available as a fallback.
- Boot assets: official Firecracker quickstart `vmlinux.bin` (x86_64) plus a
  locally built 64 MiB ext4 rootfs containing a static busybox, `/init` and
  `/sbin/init` scripts, and console/null device nodes.

Runtime verification (real microVM, KVM backend):

```
PUT /boot-source (vmlinux.bin, console=ttyS0)      → 204
PUT /drives/rootfs (ext4, root device)             → 204
PUT /actions InstanceStart                          → 204
guest log:
  EXT4-fs (vda): mounted filesystem
  VFS: Mounted root (ext4 filesystem) on device 254:0
  FORGEOS-FIRECRACKER-VERIFIED: x86_64 microVM booted via KVM
  FORGEOS-SANDBOX-PROBE: OK
```

The guest kernel booted, mounted the ext4 root, executed the guest init
script, emitted the verification line, and powered off. This is the full
KVM-backed microVM path ForgeOS's future sandbox runner needs.

Honest boundary: this verifies Firecracker + KVM on this host. Wiring a
`sandbox: firecracker` runner into forge-core's executor (seccomp/jailer
config, drive provisioning, API lifecycle management) is still an
architecture-level integration task, not yet performed.

## 2. LiteLLM cross-vendor routing — VERIFIED

Deployment:

- `litellm` (pyenv 3.10.7) already installed; a system instance serves
  `deepseek-v4-flash`/`deepseek-v4-pro` on `:4000` via opencode.ai.
- A dedicated cross-vendor instance was started on `:4001` with two
  different vendor backends:
  - vendor A: `openai/deepseek-v4-flash` → `http://127.0.0.1:4000/v1`
    (the deepseek gateway)
  - vendor B: `openai/qwen3.5:0.8b` → `http://localhost:11434/v1`
    (local Ollama, zero credentials)

Runtime verification:

```
GET /v1/models → ["deepseek-flash", "local-qwen"]     (both vendors routed)
vendor A deepseek-flash chat completion → RateLimitError from the upstream
  deepseek account (monthly quota; the request WAS routed to the deepseek
  model group — "Received Model Group=deepseek-v4-flash")
vendor B local-qwen chat completion → 200
  {"model":"local-qwen","system_fingerprint":"fp_ollama", ...}
```

Vendor B completed a real local inference through LiteLLM; vendor A was
routed correctly and failed only on the upstream account quota, not on
routing. Cross-vendor model routing through LiteLLM is verified.

Honest boundary: the "Anthropic" credential on this host is actually the
deepseek gateway (`ANTHROPIC_BASE_URL` → `:4000`, model `deepseek-v4-flash`),
so the verified vendor pair is deepseek + local Ollama. A second paid vendor
credential remains unavailable; the routing mechanism itself is proven.

## 3. Status change

- `Firecracker-compatible out-of-band sandbox`: BLOCKED-EXTERNAL → host
  VERIFIED (integration wiring still outstanding).
- `Cross-vendor LiteLLM validation`: BLOCKED-EXTERNAL → VERIFIED (deepseek +
  local Ollama; quota-limited upstream noted).
- OSV SCA DB: already resolved (Sprint 32).

## Forge × LiteLLM Responses 转译实测(2026-08)

LiteLLM `:4001` 的 `/v1/responses` 端点把 Responses 请求转译为上游
chat-completions。forge 的 OpenAI Responses adapter(纯流式,单请求)实测:

1. **互通确认**:请求编码 / SSE 解析 / 流式文本 / 事件语义全部与真实端点
   工作;`reasoning_effort: none`(→ Ollama `think:false`)后得到标准纯
   `output_text` 流(无 reasoning 事件)。
2. **转译缺陷(上游,forge 非缺陷)**:流式 item 与 completed 快照的
   message id 不一致(`msg_…` vs `chatcmpl-…`);思考模式下输出全进
   reasoning、assistant message 为空。真实 OpenAI Responses 不产生这些。
3. **forge 防漂移校验实证**:上述 id 漂移被 terminal-consistency 守卫
   正确拦截(`provider_protocol`/"terminal output did not match streamed
   assistant events")。这证明防漂移防御在真实(有缺陷的)转译下工作。
4. **测试**:`live_gateway_reasoning_round_trip_via_local_gateway`
   (env-gated,`FORGE_LIVE_GATEWAY_ENDPOINT`;无端点诚实 skip)。
   成功路径完整执行需真实 OpenAI Responses 端点(或 LiteLLM 修复 id 一致性)。

环境:LiteLLM :4001(deepseek-flash + local-qwen,`reasoning_effort: none`);
Ollama :11434(qwen3.5:0.8b);系统 LiteLLM :4000(deepseek 网关,月度限额)。
