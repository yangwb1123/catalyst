# URL Shortener (ForgeOS dogfood)

A minimal but real URL shortening service: turn a long URL into a short code,
then redirect by that code. Built by ForgeOS and governed by the same ForgeOS
harness (≤500 lines/file, ≤50 lines/function, tests must pass, zero third-party
dependencies — Node stdlib only).

## API

| Method & path   | Request body        | Success                                   | Errors |
| --------------- | ------------------- | ----------------------------------------- | ------ |
| `POST /shorten` | `{"url":"..."}`     | `201 {"code":"00000","short":"/00000"}`   | `400 {"error":...}` on invalid JSON, missing `url`, or non-http(s) URL |
| `GET /:code`    | —                   | `302` redirect with `Location: <url>`     | `404` on unknown code |

Only absolute `http:` / `https:` URLs are accepted (an explicit scheme is
required; a scheme-less `example.org` is rejected with `400`). Accepted URLs are
canonicalized before storage (e.g. `https://example.org` → `https://example.org/`).

## Architecture (Clean Architecture — dependencies point inward)

```
interface (http) → application (service) → domain (shortcode / url)
                          ↘ infrastructure (store) implements the Store port
```

| File                                      | Layer          | Depends on                          |
| ----------------------------------------- | -------------- | ----------------------------------- |
| `src/domain/shortcode.mjs`                | domain         | nothing (pure)                      |
| `src/domain/url.mjs`                      | domain         | nothing (pure)                      |
| `src/application/shortener-service.mjs`   | application    | domain + injected Store **port**    |
| `src/infrastructure/memory-store.mjs`     | infrastructure | nothing (implements the Store port) |
| `src/interface/http-server.mjs`           | interface      | application service (injected)      |

Key rules enforced here:

- **domain** is pure and imports no other layer.
- **application** depends on the domain and a Store *port*; it never imports
  `infrastructure` — the concrete store is injected via `createService(store)`.
- **interface** holds no business logic; it parses HTTP, dispatches, serializes,
  and calls the application service. `createServer(service)` returns an
  *unstarted* `http.Server`, so the caller owns `.listen()`.

The composition root wires the layers together:

```js
import { createServer } from './src/interface/http-server.mjs';
import { createService } from './src/application/shortener-service.mjs';
import { createMemoryStore } from './src/infrastructure/memory-store.mjs';

const server = createServer(createService(createMemoryStore()));
server.listen(3000);
```

## Security trade-offs (accepted)

This demo deliberately accepts two known properties; both are documented here so
they are explicit choices rather than oversights:

- **Open redirect.** `GET /:code` issues a `302` to an arbitrary stored URL. This
  is inherent to any URL shortener — redirecting to attacker-supplied targets is
  the feature. It is mitigated at *write* time: only absolute `http:` / `https:`
  URLs are stored (`javascript:`, `data:`, `file:`, etc. are rejected with `400`),
  so the redirect can never escalate to a non-web scheme. Consumers that need
  more (allow-lists, interstitial warning pages) should layer it on top.
- **Enumerable short codes.** Codes are issued from a deterministic, sequential
  base62 counter (`00000`, `00001`, …), so they are guessable and let a third
  party enumerate the full set of stored URLs. This keeps codes short and the
  mapping reproducible for the dogfood demo; a production service wanting
  unguessable codes would seed the counter randomly or use a keyed/random code
  generator instead.

## Running the tests

From the repository root:

```sh
node --test "examples/url-shortener/test/*.test.mjs"
```

This runs the full suite (domain, infrastructure, application, and HTTP e2e).
The quoted glob is deliberate: on Node 26 the directory form
(`node --test examples/url-shortener/test/`) falsely reports red — it treats the
whole directory as a single failing synthetic test and runs zero real cases.
Quoting keeps the shell from expanding the glob so Node's own test runner
resolves it.
The structural / acceptance gates are run with:

```sh
node harness/gate.mjs        # per-file line cap + root file count
node harness/acceptance.mjs  # ForgeOS acceptance verdict
```
