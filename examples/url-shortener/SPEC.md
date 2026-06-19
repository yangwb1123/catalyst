# 样板应用 — URL Shortener (ForgeOS dogfood)

> ForgeOS 第一个**被它自己造、也被它自己治理**的真实应用。mode=balanced(跳过重 Discover)。

## 目的
最小但真实的 URL 短链服务:长 URL → 短码,按短码重定向。用来端到端验证 ForgeOS 的
Build→Review→Accept 脊柱,并接受 ForgeOS harness 同等执法(≤500/≤50、测试必过、零第三方依赖)。

## API
- `POST /shorten` body `{"url":"..."}` → `201 {"code":"00000","short":"/00000"}`
- `GET /:code` → `302` 重定向到原 URL;未知码 → `404`
- 非法 URL → `400`

## 架构(Clean Architecture,依赖向内)
```
interface(http) → application(service) → domain(shortcode/url)
                          ↘ infrastructure(store) 实现 application 定义的 Store 端口
```
- `src/domain/shortcode.mjs` — 纯函数,无依赖。
- `src/domain/url.mjs` — 纯函数,无依赖。
- `src/application/shortener-service.mjs` — 依赖 domain + 注入的 store 端口;**禁止 import infrastructure**。
- `src/infrastructure/memory-store.mjs` — 实现 store 端口(Map),无依赖。
- `src/interface/http-server.mjs` — Node `http`,无第三方依赖。

## 契约(供并行实现对齐)
- shortcode:`codeForId(id, len=5): string`(base62,确定性);`isValidCode(s): boolean`。
- url:`isValidUrl(s): boolean`(仅 http/https);`normalizeUrl(s): string`。
- store 端口:`save(code, url): void`、`get(code): string|undefined`、`has(code): boolean`。
- service:`createService(store)` → `{ shorten(url): {code}, resolve(code): string|undefined }`;非法 URL 抛 `Error`。

## 安全权衡(已接受)
- **开放重定向(open redirect)**:`GET /:code` 会 302 跳到任意存储的 URL —— 这是短链服务的固有特性。
  在**写入时**收敛:仅接受绝对 `http:` / `https:` URL(`javascript:`/`data:`/`file:` 等一律 `400`),故重定向不会逃逸到非 web scheme。
- **可枚举的顺序短码**:短码由确定性自增 base62 计数器产生(`00000`、`00001`…),可被猜测/枚举。
  为保持短码简短且演示可复现而接受;生产环境如需不可猜测短码,应改用随机种子或随机/带密钥的码生成器。

## Acceptance(机器可判)
- test_pass:`node --test "examples/url-shortener/test/*.test.mjs"` 全绿(quoted glob — Node 26 下 `node --test <目录>` 会假红:目录形式把整个目录当成一个失败的合成测试、实际 0 用例运行,故必须用带引号的 glob)
- arch_violations==0:`domain/*` 不得 import 上层;`service` 不得 import infrastructure
- size:全部文件 ≤500 行、函数 ≤50 行(ForgeOS gate 强制)
- 零第三方依赖(仅 Node stdlib)

## 模块分工(并行)
- **A1**:`domain/shortcode.mjs` + `domain/url.mjs` + tests —— 纯函数,无依赖
- **A2**:`infrastructure/memory-store.mjs` + test —— 无依赖
- **A3(待 A1/A2)**:`application/shortener-service.mjs` + `interface/http-server.mjs` + service/http test + README
- **A4**:fresh reviewer → 然后 accept
