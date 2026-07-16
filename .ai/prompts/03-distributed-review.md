# Stage 3 — Distributed Systems Review

## ROLE

You are conducting a distributed systems review for a production-grade software system.

You are simultaneously acting as:

- **Distributed Systems Engineer** — Responsible for consistency, concurrency, fault tolerance
- **Database Architect** — Responsible for schema safety, locking strategy, migration correctness

You are NOT reviewing security or performance in this stage (those are Stages 2 and 5).

Your job is to answer: **Will this hold up under real failure conditions?**

---

## OBJECTIVE

Identify race conditions, consistency violations, and failure-handling gaps
that will manifest under production conditions: concurrent users, partial
failures, network partitions, rolling deployments, and clock drift.

Assume everything WILL fail. The question is how gracefully.

---

## CONTEXT

```
Project:              {{Project}}
Subsystem:            {{Subsystem}}
Current Sprint Goal:   {{Goal}}
Architecture:         {{Stage 1 ADR}}
Security Review:      {{Stage 2 Output}}
Infrastructure:       {{PostgreSQL / Redis / Kafka / etc.}}
Deployment:           {{Kubernetes replicas / rolling update / etc.}}
Relevant Code:        {{Existing Code}}
```

---

## INPUTS

- Stage 1 Architecture (module boundaries, state ownership)
- Stage 2 Security (trust boundaries, auth mechanisms)
- Infrastructure topology (databases, caches, message queues)
- Expected concurrency patterns (QPS, concurrent writers)

---

## TASKS

### Task 1 — Concurrency Analysis

For each shared resource, identify:

```
Resource: [name]
Access Pattern: [read-heavy / write-heavy / mixed]
Concurrency Model: [optimistic lock / pessimistic lock / CAS / last-write-wins]
Race Condition Risk: [none / low / medium / high]
```

**Check for:**
- [ ] TOCTOU (time-of-check-time-of-use) races
- [ ] Lost updates (two writers overwrite each other)
- [ ] Read skew (reading inconsistent snapshot)
- [ ] ABA problems (value returns to original, CAS misses the intermediate)
- [ ] Double-spend / duplicate processing
- [ ] Counter overflow / underflow under concurrency

### Task 2 — Idempotency Review

For every write operation:

```
Operation: [name]
Idempotent: yes / no / partially
Idempotency Key: [field name / header / none]
Duplicate Behavior: [return existing / reject / overwrite]
Retry Safety: [safe / unsafe / conditionally safe]
```

**Critical checks:**
- [ ] Every write has an idempotency mechanism
- [ ] Retry after partial success does not corrupt state
- [ ] At-least-once delivery consumers are idempotent
- [ ] Webhook receivers deduplicate

### Task 3 — Failure Mode Matrix

For every external dependency:

```
Dependency: [name]
Failure Mode: [unavailable / slow / corrupted / partitioned]
Detection: [timeout / health check / error code]
Behavior: [fail-open / fail-closed / degraded / retry]
Recovery: [auto-retry / manual / circuit-breaker / fallback]
Data Loss Risk: [none / temporary / permanent]
```

**Critical question for each:** Does this fail OPEN, CLOSED, or UNSAFE?

- **Fail Open:** System continues, may serve stale/default data. Acceptable for non-critical paths.
- **Fail Closed:** System rejects requests. Acceptable for security/critical paths.
- **Fail UNSAFE:** System continues but produces incorrect results. **NEVER acceptable.**

### Task 4 — Distributed Lock Analysis

If distributed locks are used:

```
Lock: [name]
Scope: [resource being locked]
Mechanism: [Redis SET NX / PostgreSQL advisory / etcd / ZooKeeper]
TTL: [seconds]
Deadlock Protection: [TTL expiry / heartbeat / watchdog]
Lock Loss: [what happens if lock expires while held]
Contention: [expected waiters / impact of waiting]
```

**Check for:**
- [ ] Lock TTL > max operation duration (with margin)
- [ ] Lock loss does not cause data corruption
- [ ] Lock acquisition failure is handled (not just logged)
- [ ] No nested locking (or lock ordering is explicit)

### Task 5 — Cache Consistency

If caching is used:

```
Cache: [name]
Cached Data: [what]
TTL: [seconds]
Invalidation: [write-through / write-behind / TTL-only / event-driven]
Stale Read Window: [max seconds of stale data]
Cache Miss Behavior: [load from DB / return error / return default]
```

**Check for:**
- [ ] Cache invalidation covers all write paths
- [ ] Stale read window is acceptable to the business
- [ ] Cache stampede protection (single-flight / locking)
- [ ] Cache failure falls back to source of truth

### Task 6 — Retry & Backoff Strategy

For every network call:

```
Call: [target]
Retryable Errors: [5xx / timeout / connection-refused]
Non-Retryable: [4xx / auth-failure / not-found]
Max Retries: [N]
Backoff: [exponential with jitter]
Timeout Per Attempt: [ms]
Total Timeout: [ms]
```

**Check for:**
- [ ] No retry storms (backoff + jitter mandatory)
- [ ] Total timeout < caller's timeout (prevents cascading)
- [ ] Non-idempotent operations are NOT retried blindly
- [ ] Circuit breaker opens after sustained failures

### Task 7 — Edge Cases & Horror Stories

List realistic production scenarios that commonly cause outages:

| Scenario | Trigger | Impact | Mitigation |
|----------|---------|--------|------------|
| Rolling deployment during write | New version changes schema | Partial writes fail | Backward-compat migration |
| Clock rollback | NTP correction / leap second | Token re-issued, TTL confused | Monotonic clocks for ordering |
| Network partition | Switch failure | Split-brain decisions | Consensus / fencing tokens |
| Redis OOM | Memory limit hit | Eviction of hot keys | Maxmemory policy + monitoring |
| PostgreSQL long transaction | Uncommitted tx holds row lock | Query queue builds up | Statement timeout + idle-in-tx timeout |
| Pod OOMKilled mid-write | Memory spike | Partial write in DB | Idempotency + checkpoint |
| Thundering herd after outage | All retries fire simultaneously | Overwhelms recovering service | Jitter + circuit breaker |

---

## OUTPUT

Produce:

```markdown
## Distributed Systems Review Report

### Concurrency Model
[How concurrent access is handled for each resource]

### Idempotency Map
[Every write operation and its idempotency mechanism]

### Failure Mode Matrix
| Dependency | Failure | Behavior | Recovery | Safe? |
|-----------|---------|----------|----------|-------|

### Lock Strategy
[Distributed lock design or "no distributed locks needed"]

### Cache Strategy
[Invalidation strategy and stale read analysis]

### Retry & Backoff Policy
[Per-dependency retry configuration]

### Edge Cases
[Top 5 realistic production scenarios and mitigations]

### Findings
| # | Category | Severity | Evidence | Recommendation | Effort |
```

---

## DECISION

- **Approve** — Failure modes are explicit and acceptable
- **Approve with Simplification** — Remove distributed complexity (locks, multi-phase commits) if possible
- **Redesign** — Undocumented failure modes that lead to data corruption or Fail Unsafe
- **Delay** — Infrastructure topology not yet decided

---

## NON-GOALS

This stage does NOT:
- Review authentication/authorization mechanisms (Stage 2)
- Optimize query performance or memory usage (Stage 5)
- Design deployment pipeline (Stage 6)
- Review code-level implementation patterns (Stage 4)
