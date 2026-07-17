// Tests for harness/sca_fetch.mjs's pure logic (rangesOf/preferredId/
// mergeConservative/severityOf/mergeVulnInto). No network I/O — fetchAdvisoriesFor
// and main() are exercised live only by actually running the tool (see
// docs/... / CURRENT_SPRINT.md Sprint 32 for the manual verification record);
// this suite covers the transform/merge logic these fixtures were designed to
// stress, including the real edge cases an independent review caught: a
// reintroduced-vulnerability record with TWO disjoint windows in one range, and
// a PyPI-style bare pre-release suffix (`5.2b1`, no `-`/`+` separator).
import { test } from 'node:test';
import assert from 'node:assert/strict';
import {
  rangesOf, preferredId, mergeConservative, severityOf, mergeVulnInto,
} from './sca_fetch.mjs';
import { compareVersions } from './sca.mjs';

// --- rangesOf -----------------------------------------------------------------

test('rangesOf: a single introduced->fixed cycle yields one window', () => {
  const affected = [{ ranges: [{ type: 'ECOSYSTEM', events: [{ introduced: '0' }, { fixed: '5.4' }] }] }];
  assert.deepEqual(rangesOf(affected), [{ introduced: '0', fixed: '5.4' }]);
});

test('rangesOf: a reintroduced vulnerability (TWO disjoint windows in one range) yields BOTH, not just the last', () => {
  const affected = [{
    ranges: [{
      type: 'ECOSYSTEM',
      events: [
        { introduced: '0' }, { fixed: '1.0' },
        { introduced: '2.0' }, { fixed: '3.0' },
      ],
    }],
  }];
  assert.deepEqual(rangesOf(affected), [
    { introduced: '0', fixed: '1.0' },
    { introduced: '2.0', fixed: '3.0' },
  ], 'the earlier [0,1.0) window must not be silently dropped');
});

test('rangesOf: an unterminated trailing introduced (no fix yet) stays OPEN', () => {
  const affected = [{ ranges: [{ type: 'ECOSYSTEM', events: [{ introduced: '2.0' }] }] }];
  assert.deepEqual(rangesOf(affected), [{ introduced: '2.0', fixed: undefined }]);
});

test('rangesOf: scans EVERY affected entry and EVERY range, not just the first', () => {
  const affected = [
    { ranges: [{ type: 'ECOSYSTEM', events: [{ introduced: '0' }, { fixed: '1.0' }] }] },
    { ranges: [{ type: 'ECOSYSTEM', events: [{ introduced: '2.0' }, { fixed: '3.0' }] }] },
  ];
  assert.deepEqual(rangesOf(affected), [
    { introduced: '0', fixed: '1.0' },
    { introduced: '2.0', fixed: '3.0' },
  ]);
});

test('rangesOf: a non-ECOSYSTEM/SEMVER range type is ignored', () => {
  const affected = [{ ranges: [{ type: 'GIT', events: [{ introduced: 'abc123' }] }] }];
  assert.deepEqual(rangesOf(affected), []);
});

test('rangesOf: no affected/ranges/events -> empty, never throws', () => {
  assert.deepEqual(rangesOf(undefined), []);
  assert.deepEqual(rangesOf([]), []);
  assert.deepEqual(rangesOf([{ ranges: [] }]), []);
});

// --- preferredId ----------------------------------------------------------

test('preferredId: prefers a GHSA-* alias over the record\'s own PYSEC-native id', () => {
  const vuln = { id: 'PYSEC-2020-96', aliases: ['CVE-2020-1747', 'GHSA-6757-jp84-gxfx'] };
  assert.equal(preferredId(vuln), 'GHSA-6757-jp84-gxfx');
});

test('preferredId: a GHSA-native record keeps its own id (no better alias)', () => {
  const vuln = { id: 'GHSA-6757-jp84-gxfx', aliases: ['CVE-2020-1747'] };
  assert.equal(preferredId(vuln), 'GHSA-6757-jp84-gxfx');
});

test('preferredId: no GHSA anywhere -> falls back to the record\'s own id', () => {
  const vuln = { id: 'PYSEC-2020-96', aliases: ['CVE-2020-1747'] };
  assert.equal(preferredId(vuln), 'PYSEC-2020-96');
});

// --- mergeConservative ------------------------------------------------------

test('mergeConservative: picks the EARLIER introduced and the LATER fixed (widest window)', () => {
  const merged = mergeConservative({ introduced: '5.1', fixed: '5.3.1' }, { introduced: '5.1b7', fixed: '5.2' });
  assert.equal(compareVersions(merged.introduced, '5.1b7'), 0, 'the earlier (pre-release) bound wins');
  assert.equal(compareVersions(merged.fixed, '5.3.1'), 0, 'the later fixed bound wins');
});

test('mergeConservative: an open-ended (fixed: null) side always wins — "no fix yet" beats any concrete fixed version', () => {
  const merged = mergeConservative({ introduced: '0', fixed: '5.0' }, { introduced: '0', fixed: null });
  assert.equal(merged.fixed, null, 'no-fix-yet must dominate a concrete fixed version in either argument position');
  const reversed = mergeConservative({ introduced: '0', fixed: null }, { introduced: '0', fixed: '5.0' });
  assert.equal(reversed.fixed, null, 'order must not matter');
});

test('mergeConservative: equal introduced values do not misfire (tie-break is inconsequential)', () => {
  const merged = mergeConservative({ introduced: '1.0', fixed: '2.0' }, { introduced: '1.0', fixed: '2.0' });
  assert.equal(compareVersions(merged.introduced, '1.0'), 0);
  assert.equal(compareVersions(merged.fixed, '2.0'), 0);
});

// --- severityOf --------------------------------------------------------------

test('severityOf: prefers database_specific.severity', () => {
  assert.equal(severityOf({ database_specific: { severity: 'CRITICAL' } }), 'CRITICAL');
});

test('severityOf: falls back to a CVSS score string when database_specific is absent', () => {
  assert.equal(severityOf({ severity: [{ score: 'CVSS:3.1/AV:N' }] }), 'CVSS:3.1/AV:N');
});

test('severityOf: UNKNOWN when neither is present — never fabricated', () => {
  assert.equal(severityOf({}), 'UNKNOWN');
});

// --- mergeVulnInto (the full per-vuln fold) -----------------------------------

test('mergeVulnInto: two aliased records describing the SAME window collapse into one merged entry', () => {
  const dedup = new Map();
  const dep = { name: 'PyYAML', ecosystem: 'PyPI' };
  const ghsaRecord = {
    id: 'GHSA-6757-jp84-gxfx',
    aliases: ['CVE-2020-1747'],
    affected: [{ ranges: [{ type: 'ECOSYSTEM', events: [{ introduced: '5.1b7' }, { fixed: '5.3.1' }] }] }],
    database_specific: { severity: 'CRITICAL' },
  };
  const pysecRecord = {
    id: 'PYSEC-2020-96',
    aliases: ['CVE-2020-1747', 'GHSA-6757-jp84-gxfx'],
    affected: [{ ranges: [{ type: 'ECOSYSTEM', events: [{ introduced: '5.1' }, { fixed: '5.3.1' }] }] }],
  };
  mergeVulnInto(dedup, dep, ghsaRecord);
  mergeVulnInto(dedup, dep, pysecRecord);
  assert.equal(dedup.size, 1, 'the alias pair must collapse to ONE record, not two');
  const [record] = [...dedup.values()];
  assert.equal(record.id, 'GHSA-6757-jp84-gxfx');
  assert.equal(compareVersions(record.vulnerable.introduced, '5.1b7'), 0, 'the earlier bound wins across the merge');
  assert.equal(record.severity, 'CRITICAL');
});

test('mergeVulnInto: two DISJOINT windows on the same vuln stay as separate records (not falsely widened)', () => {
  const dedup = new Map();
  const dep = { name: 'example-pkg', ecosystem: 'PyPI' };
  const vuln = {
    id: 'GHSA-fake-0000-0000',
    affected: [{
      ranges: [{
        type: 'ECOSYSTEM',
        events: [{ introduced: '0' }, { fixed: '1.0' }, { introduced: '2.0' }, { fixed: '3.0' }],
      }],
    }],
    database_specific: { severity: 'HIGH' },
  };
  mergeVulnInto(dedup, dep, vuln);
  assert.equal(dedup.size, 2, 'a reintroduced vuln must produce two separate advisory records, not one widened range');
  const windows = [...dedup.values()].map((r) => r.vulnerable);
  assert.ok(windows.some((w) => compareVersions(w.fixed, '1.0') === 0));
  assert.ok(windows.some((w) => compareVersions(w.introduced, '2.0') === 0));
});

test('mergeVulnInto: a vuln with no usable range is skipped, not guessed', () => {
  const dedup = new Map();
  mergeVulnInto(dedup, { name: 'x', ecosystem: 'PyPI' }, { id: 'GHSA-noop', affected: [] });
  assert.equal(dedup.size, 0);
});

// --- parseVer bare PEP 440 pre-release regression (via compareVersions) ------

test('compareVersions: a bare PyPI pre-release suffix (no "-"/"+" separator) parses and orders correctly', () => {
  // Real OSV data for PyYAML uses exactly this shape (5.1b7, 5.2b1, 5.4b1) — this
  // was a real bug an independent review caught: the old parser required a
  // "-"/"+" separator, so these versions were silently UNPARSEABLE and sorted as
  // -Infinity regardless of which side of a comparison they landed on.
  assert.equal(compareVersions('5.2b1', '5.2'), -1, 'a beta pre-release sorts below its release');
  assert.equal(compareVersions('5.4b1', '5.4b2'), -1, 'beta identifiers compare in order');
  assert.equal(compareVersions('5.1b7', '5.1'), -1, 'a beta sorts below the bare release with the same numeric parts');
  assert.equal(compareVersions('6.0', '5.4b1'), 1, 'a real release correctly outranks an earlier bare-suffix pre-release');
});
