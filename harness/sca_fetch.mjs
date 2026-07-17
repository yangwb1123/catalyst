#!/usr/bin/env node
// ForgeOS sca_fetch — a MANUAL, operator-run maintenance tool that refreshes
// .agent/security/advisories.json from the real OSV API (https://osv.dev).
//
// HONESTY — why this is a separate script, never invoked by forge accept/gate:
// the harness's gate path (gate.mjs/check.py/acceptance.mjs, and sca.mjs within
// it) is host-independent and network-free by design — a `forge accept` run
// must be deterministic and work offline/in CI with no egress. Fetching a live
// vulnerability feed on every gate run would trade that determinism for
// freshness and make the gate flaky on transient network failures. Instead:
// this script is a one-off/periodic REFRESH tool an operator runs by hand
// (`node harness/sca_fetch.mjs`) to regenerate the on-disk snapshot that
// sca.mjs then reads deterministically. Same posture as vendoring a lockfile.
//
// What it does: discover this repo's real dependency manifests (via sca.mjs's
// own discoverManifests/parseManifest — no re-implementation), query the OSV
// API once per unique {name, ecosystem} pair for the package's FULL known
// advisory history (not just vulns matching the currently-pinned version, so
// a future version bump within an already-known-bad range is still caught),
// transform each OSV record into sca.mjs's simplified DB schema
// ({id, package, ecosystem, vulnerable:{introduced, fixed}, severity}), and
// write .agent/security/advisories.json. OSV commonly returns the same real
// vulnerability twice — once under its GHSA-* record and once under a
// PYSEC-*-native record that merely ALIASES the same GHSA id — and the two
// records' own `affected[].ranges` frequently disagree slightly on the exact
// boundary (e.g. introduced "5.1b7" vs "5.1"). We dedupe on the CANONICAL id
// (preferredId), not on the raw range, and when two records collapse into one
// canonical id we keep the MOST CONSERVATIVE merged range: the earliest
// `introduced` and the latest `fixed` (an open-ended/no-fix-yet record beats
// any concrete fixed version) via sca.mjs's own compareVersions — so a real
// vulnerability is never under-reported because one of its two aliased
// sources happened to describe a narrower window.
import { readFileSync, writeFileSync } from 'node:fs';
import { join, dirname } from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';
import { discoverManifests, parseManifest, compareVersions } from './sca.mjs';

const HARNESS_DIR = dirname(fileURLToPath(import.meta.url));
const ROOT = dirname(HARNESS_DIR);
const OSV_QUERY_URL = 'https://api.osv.dev/v1/query';

// uniqueDeps: manifests -> deduped [{name, ecosystem}], across every manifest
// in the repo (a dep required by two manifests is queried once).
export function uniqueDeps(root) {
  const seen = new Map();
  for (const m of discoverManifests(root)) {
    let text;
    try { text = readFileSync(m.file, 'utf8'); } catch { continue; }
    for (const d of parseManifest(text, m.kind)) {
      seen.set(`${d.ecosystem}:${d.name}`, { name: d.name, ecosystem: d.ecosystem });
    }
  }
  return [...seen.values()];
}

// severityOf: an OSV vuln record -> a short string. Prefers the curated
// database_specific.severity (GHSA's CRITICAL/HIGH/MEDIUM/LOW); falls back to
// the first CVSS vector string when present; else 'UNKNOWN' (never fabricated).
export function severityOf(vuln) {
  if (vuln.database_specific && vuln.database_specific.severity) {
    return vuln.database_specific.severity;
  }
  if (Array.isArray(vuln.severity) && vuln.severity.length > 0) {
    return vuln.severity[0].score || 'UNKNOWN';
  }
  return 'UNKNOWN';
}

// rangesOf: an OSV `affected[]` array -> ALL {introduced, fixed} windows found
// across EVERY ECOSYSTEM/SEMVER-type range in EVERY affected entry (not just
// the first). OSV events within one range are a chronological list; most
// packages have exactly one introduced->fixed cycle, but a package whose
// vulnerability was reintroduced has TWO (e.g. [introduced,fixed(,introduced,
// fixed)] — two disjoint vulnerable windows in the same range object). Each
// `introduced` opens a window; the next `fixed` closes it (pushed as a
// completed window); an `introduced` with no closing `fixed` yet stays open
// (fixed: undefined = "no fix yet"). Returning ALL windows (not just the last
// or the first range) avoids silently discarding an earlier vulnerable window
// — this DB schema still only models plain [introduced, fixed) half-open
// windows per entry, so each window becomes its own DB record rather than
// being collapsed into one (collapsing would either drop a real window or
// falsely widen the range to cover an actually-safe gap between windows).
export function rangesOf(affected) {
  const windows = [];
  for (const a of affected || []) {
    for (const r of a.ranges || []) {
      if (r.type !== 'ECOSYSTEM' && r.type !== 'SEMVER') continue;
      let introduced;
      for (const ev of r.events || []) {
        if (ev.introduced !== undefined) {
          if (introduced !== undefined) windows.push({ introduced, fixed: undefined }); // unterminated -> still open
          introduced = ev.introduced;
        } else if (ev.fixed !== undefined && introduced !== undefined) {
          windows.push({ introduced, fixed: ev.fixed });
          introduced = undefined;
        }
      }
      if (introduced !== undefined) windows.push({ introduced, fixed: undefined });
    }
  }
  return windows;
}

// preferredId: a vuln's OSV id + aliases -> the CANONICAL id used as the
// dedup key. Prefers a GHSA-* alias (GitHub Advisory Database ids are the most
// widely cross-referenced) so a GHSA-native record and its PYSEC-native alias
// record collapse onto the same key even though their own top-level `id`s
// differ; else falls back to the record's own id.
export function preferredId(vuln) {
  const ghsa = (vuln.aliases || []).find((a) => a.startsWith('GHSA-'));
  if (ghsa) return ghsa;
  if (vuln.id && vuln.id.startsWith('GHSA-')) return vuln.id;
  return vuln.id;
}

// mergeConservative: combine two {introduced, fixed} windows for the SAME
// canonical vulnerability into the widest (most conservative) merged window —
// min(introduced), max(fixed) — using sca.mjs's own compareVersions so this
// script never re-implements semver ordering. `fixed: null/undefined` means
// "no fix yet" (open-ended) and always wins over any concrete fixed version.
export function mergeConservative(a, b) {
  const introduced = compareVersions(a.introduced ?? '0', b.introduced ?? '0') <= 0
    ? (a.introduced ?? '0') : (b.introduced ?? '0');
  let fixed;
  if (a.fixed == null || b.fixed == null) {
    fixed = null;
  } else {
    fixed = compareVersions(a.fixed, b.fixed) >= 0 ? a.fixed : b.fixed;
  }
  return { introduced, fixed };
}

async function fetchAdvisoriesFor(dep) {
  const res = await fetch(OSV_QUERY_URL, {
    method: 'POST',
    body: JSON.stringify({ package: { name: dep.name, ecosystem: dep.ecosystem } }),
  });
  if (!res.ok) {
    throw new Error(`OSV query failed for ${dep.ecosystem}:${dep.name} — HTTP ${res.status}`);
  }
  const body = await res.json();
  return body.vulns || [];
}

// mergeVulnInto: fold one OSV vuln record for `dep` into the running `dedup`
// map. A vuln can carry MULTIPLE disjoint windows (rangesOf above); each
// window gets its own dedup key (canonical id + window INDEX) so two
// aliased records (GHSA/PYSEC) describing the SAME window still collapse via
// mergeConservative, while two genuinely DISJOINT windows on the same vuln
// stay separate DB records rather than being falsely widened into one range
// that would also cover the (actually-safe) gap between them. Honest limit:
// window index is a heuristic pairing key across aliased records — it assumes
// both records enumerate their windows in the same order, true for every
// record this script has seen so far (each has exactly one window) but not
// guaranteed in general for a package with a reintroduced vulnerability.
export function mergeVulnInto(dedup, dep, vuln) {
  const windows = rangesOf(vuln.affected);
  if (windows.length === 0) return; // no usable ECOSYSTEM/SEMVER range — skip, don't guess
  const id = preferredId(vuln);
  const severity = severityOf(vuln);
  windows.forEach((range, i) => {
    const key = `${dep.ecosystem}|${dep.name}|${id}|${i}`;
    const existing = dedup.get(key);
    const incoming = { introduced: range.introduced ?? '0', fixed: range.fixed ?? null };
    if (existing) {
      dedup.set(key, {
        ...existing,
        vulnerable: mergeConservative(existing.vulnerable, incoming),
        severity: existing.severity !== 'UNKNOWN' ? existing.severity : severity,
      });
    } else {
      dedup.set(key, { id, package: dep.name, ecosystem: dep.ecosystem, vulnerable: incoming, severity });
    }
  });
}

async function main() {
  const root = process.argv[2] || ROOT;
  const outPath = process.argv[3] || join(root, '.agent', 'security', 'advisories.json');
  const deps = uniqueDeps(root);
  if (deps.length === 0) {
    console.log('sca_fetch: no dependencies found in any manifest — nothing to query.');
    return;
  }
  const dedup = new Map(); // key: ecosystem|name|canonicalId -> record
  for (const dep of deps) {
    console.log(`sca_fetch: querying OSV for ${dep.ecosystem}:${dep.name} ...`);
    const vulns = await fetchAdvisoriesFor(dep);
    for (const vuln of vulns) mergeVulnInto(dedup, dep, vuln);
  }
  const advisories = [...dedup.values()];
  const out = {
    _meta: {
      source: 'https://osv.dev (OSV API v1/query, package-only — full known history per dep, not filtered to the current pin)',
      generated_by: 'node harness/sca_fetch.mjs (manual/periodic refresh, never run by forge accept/gate)',
      dependencies_queried: deps,
    },
    advisories,
  };
  writeFileSync(outPath, `${JSON.stringify(out, null, 2)}\n`);
  console.log(`sca_fetch: wrote ${advisories.length} advisory record(s) for ${deps.length} dependenc(ies) -> ${outPath}`);
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  main().catch((err) => {
    console.error(`sca_fetch: ERROR — ${err && err.stack ? err.stack : err}`);
    process.exit(1);
  });
}
