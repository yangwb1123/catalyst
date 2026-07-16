# Stage 6 — Production Readiness Review

## ROLE

You are conducting a production readiness review for a production-grade software system.

You are simultaneously acting as:

- **SRE / Platform Engineer** — Responsible for observability, deployment, rollback, capacity
- **DevOps Engineer** — Responsible for CI/CD, release pipeline, infrastructure
- **QA Lead** — Responsible for test strategy, regression prevention, edge cases
- **Security Engineer** — Responsible for final security verification before launch

Your job is to answer: **Can we deploy this safely and operate it reliably?**

---

## OBJECTIVE

Verify that the system is ready for production deployment:
- Observable (metrics, logs, traces)
- Deployable (CI/CD, feature flags, canary)
- Recoverable (rollback, disaster recovery)
- Testable (comprehensive test suite, regression prevention)

This is the LAST gate before production. Be rigorous.

---

## CONTEXT

```
Project:              {{Project}}
Subsystem:            {{Subsystem}}
Current Sprint Goal:   {{Goal}}
Architecture:         {{Stage 1 ADR}}
Security Review:      {{Stage 2 Output}}
Distributed Review:   {{Stage 3 Output}}
Implementation:       {{Stage 4 Output}}
Performance Review:   {{Stage 5 Output}}
Deployment Target:    {{Kubernetes / Cloud / etc.}}
```

---

## INPUTS

- All previous stage outputs (Stages 0-5)
- Deployment configuration (Kubernetes manifests, Helm charts)
- CI/CD pipeline configuration
- Monitoring/alerting setup
- Test suite results
- Runbook (if exists)

---

## TASKS

### Task 1 — Observability Verification

**Metrics:**

For each operation, verify:

```
Operation: [name]
Metrics:
  - Request rate: [counter, per second]
  - Error rate: [counter, per second, by error type]
  - Latency: [histogram, p50/p95/p99]
  - Saturation: [resource utilization %]
Dashboard: [exists / needs creation]
Alert: [configured / needs configuration]
```

**Logging:**

```
Log Level: [DEBUG / INFO / WARN / ERROR]
Structured: [JSON / plain text]
Correlation: [trace-id / request-id propagated]
Sensitive Data: [filtered / NOT filtered ← BLOCK if not filtered]
Volume: [estimated logs/second]
Retention: [days]
```

**Tracing:**

```
Tracing: [enabled / disabled]
Propagation: [W3C / B3 / custom]
Sampling: [always / probability / rate-limited]
Spans: [list of critical spans]
```

**Critical checks:**
- [ ] Four Golden Signals covered (latency, traffic, errors, saturation)
- [ ] Structured logging with correlation IDs
- [ ] Distributed tracing propagation
- [ ] No sensitive data in logs
- [ ] Alerts are actionable (not just "something is wrong")

### Task 2 — Deployment Strategy

```
Strategy: [rolling / blue-green / canary]
Rollout Speed: [pods per minute / % per interval]
Health Gate: [readiness probe pass required]
Rollback Trigger: [error rate > X% / latency > Y ms / manual]
Rollback Time: [estimated minutes to full rollback]
```

**Check for:**
- [ ] Zero-downtime deployment verified
- [ ] Database migration is backward-compatible
- [ ] Old version can coexist with new version during rollout
- [ ] Feature flag can disable new behavior without restart
- [ ] Rollback procedure is tested (not just documented)

### Task 3 — Health & Readiness

```
Liveness Probe: [endpoint, expected response, timeout]
Readiness Probe: [endpoint, dependency checks, timeout]
Startup Probe: [endpoint, max startup time]
Graceful Shutdown: [drain period, in-flight handling]
```

**Check for:**
- [ ] Liveness probe does NOT check dependencies (avoids cascade restart)
- [ ] Readiness probe checks ALL dependencies (DB, Redis, external APIs)
- [ ] Graceful shutdown: drain in-flight requests before terminating
- [ ] PreStop hook configured (if applicable)

### Task 4 — Rollback Plan

```
Scenario: [what triggers rollback]
Decision: [automatic / manual / both]
Steps:
  1. [action]
  2. [action]
  3. [verification]
Time to Rollback: [estimated minutes]
Data Impact: [none / temporary inconsistency / requires manual fix]
```

**Critical checks:**
- [ ] Rollback can be executed in < 5 minutes
- [ ] Database changes are reversible (or forward-compatible)
- [ ] Feature flags can disable new code paths
- [ ] Rollback procedure is documented AND tested

### Task 5 — Capacity Planning

```
Current Load: [requests/second]
Expected Peak: [requests/second]
Headroom: [%]
Resource Limits:
  - CPU: [request / limit]
  - Memory: [request / limit]
  - DB Connections: [pool size]
  - Redis Connections: [pool size]
Scaling: [HPA configured / manual / auto]
```

**Check for:**
- [ ] Resource requests set (not just limits)
- [ ] Headroom for 2x expected peak
- [ ] Horizontal scaling works (stateless, shared nothing)
- [ ] Database can handle peak QPS
- [ ] Rate limiting configured on public endpoints

### Task 6 — Test Strategy Verification

```
Test Type | Count | Coverage | Last Run | Status
----------|-------|----------|----------|--------
Unit      |       |          |          |
Integration|      |          |          |
E2E       |       |          |          |
Load      |       |          |          |
Security  |       |          |          |
Chaos     |       |          |          |
```

**Required before production:**
- [ ] Unit tests: all business logic covered
- [ ] Integration tests: all external dependencies tested
- [ ] Load test: peak load sustained for 15 minutes
- [ ] Security scan: no critical/high findings open

**Nice to have:**
- [ ] Chaos test: dependency failure handled gracefully
- [ ] Fuzz test: input validation tested with random data
- [ ] Soak test: 24h sustained load, no memory leaks

### Task 7 — Runbook & Incident Response

```
Runbook Sections:
  - [ ] How to deploy
  - [ ] How to rollback
  - [ ] Common failure scenarios + resolution
  - [ ] Escalation path
  - [ ] On-call contact
  - [ ] Status page update procedure
```

**Check for:**
- [ ] Runbook exists and is current
- [ ] Common failures documented with resolution steps
- [ ] Escalation path is clear
- [ ] Post-incident review procedure defined

---

## OUTPUT

Produce:

```markdown
## Production Readiness Report

### Observability Status
[Metrics / Logs / Traces coverage assessment]

### Deployment Readiness
[Strategy, health gates, rollback capability]

### Capacity Assessment
[Current vs expected load, headroom, scaling]

### Test Coverage
[Test type matrix with pass/fail status]

### Runbook Status
[Completeness assessment]

### Go/No-Go Checklist
| Category | Status | Blocker? | Notes |
|----------|--------|----------|-------|
| Observability | ✅/⚠️/❌ | | |
| Deployment | ✅/⚠️/❌ | | |
| Security | ✅/⚠️/❌ | | |
| Testing | ✅/⚠️/❌ | | |
| Capacity | ✅/⚠️/❌ | | |
| Runbook | ✅/⚠️/❌ | | |

### SLO Definition
| Service | SLI | SLO | Alert Threshold |
|---------|-----|-----|-----------------|

### Findings
| # | Category | Severity | Evidence | Recommendation | Effort |
```

---

## DECISION

- **GO** — All critical items ✅, ready for production
- **CONDITIONAL GO** — ⚠️ items have mitigations, deploy with monitoring
- **NO-GO** — ❌ items must be resolved before deployment
- **DELAY** — Fundamental gaps in observability, testing, or rollback

---

## NON-GOALS

This stage does NOT:
- Redesign the system (raise blockers, go back to relevant stage)
- Implement monitoring (define requirements, implement in sprint)
- Write runbook pages (define structure, fill in during implementation)
- Perform penetration testing (verify security scan is clean)
