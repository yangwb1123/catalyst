// Execution and evidence interpretation for project command plans.
//
// Planning remains in project.mjs. This module deliberately knows only the
// plan data contract and the injected argv executor, keeping tool execution
// policy cohesive and independently testable.
const PASS = 'PASS';
const FAIL = 'FAIL';
const NA = 'N-A';
const APPLICABLE = 'applicable';
const INAPPLICABLE = 'inapplicable';
const NO_TOOL = 'no_tool';

function noToolDetail(plan, reason) {
  const label = plan.label ?? (plan.profile ? `${plan.lang}/${plan.profile}` : plan.lang);
  return `${label}: ${reason} (tool unavailable; ${plan.operation} not run)`;
}

function unrunnableTool(plan, output) {
  const text = output ?? '';
  if (plan.lang === 'rust' && plan.operation === 'lint') {
    return /no such command.*clippy|clippy.*(?:not installed|unavailable)|rustup component add clippy/i.test(text);
  }
  if (plan.lang !== 'java') return false;
  return /GradleWrapperMain|maven-wrapper|distribution.*(?:download|unavailable)|no plugin found for prefix ['"]?checkstyle|task ['"]?check['"]? not found/i.test(text);
}

function clippedOutput(output) {
  const text = String(output ?? '').trim();
  if (text.length <= 800) return text;
  return `${text.slice(0, 300)} … ${text.slice(-480)}`;
}

// Successful exit is necessary but not sufficient for a project-test PASS.
// Return a positive observable execution count, 0 for a recognized zero-test
// run, or null when the runner output cannot prove that any test executed.
export function observedTestCount(plan, output) {
  const text = String(output ?? '');
  if (plan.lang === 'go') {
    const names = text.split(/\r?\n/).filter(
      (line) => /^(?:Test|Benchmark|Fuzz|Example)\S*$/.test(line.trim()),
    );
    if (names.length > 0) return new Set(names).size;
    return /^(?:ok|\?)\s+\S+|\[no test files\]/m.test(text) ? 0 : null;
  }

  const counts = [];
  const patterns = plan.lang === 'rust'
    ? [/\brunning\s+(\d+)\s+tests?\b/gi]
    : plan.lang === 'python'
      ? [/\b(\d+)\s+passed\b/gi, /\bRan\s+(\d+)\s+tests?\b/gi, /\bno tests ran\b/gi]
      : plan.lang === 'node'
        ? [
          /^# tests\s+(\d+)\s*$/gmi,
          /\bTests:\s+.*?(\d+)\s+total\b/gi,
          /\b(\d+)\s+passing\b/gi,
        ]
        : [
          /\bTests run:\s*(\d+)\b/gi,
          /\b(\d+)\s+tests?\s+(?:completed|successful)\b/gi,
        ];
  for (const pattern of patterns) {
    for (const match of text.matchAll(pattern)) {
      counts.push(match[1] === undefined ? 0 : Number(match[1]));
    }
  }
  return counts.length > 0 ? Math.max(...counts) : null;
}

function successfulPlanResult(plan, result, exec, label, command) {
  if (plan.operation !== 'test') {
    return { lang: plan.lang, status: PASS, detail: `${label}: ${command} PASS` };
  }
  let evidence = result;
  if (plan.evidenceArgs) {
    evidence = exec(plan.cmd, plan.evidenceArgs, {}, plan.root);
    if (!evidence?.ok) {
      return {
        lang: plan.lang,
        status: FAIL,
        category: APPLICABLE,
        detail: `${label}: ${command} passed but test enumeration failed`,
      };
    }
  }
  const count = observedTestCount(plan, evidence.out);
  if (count === 0) {
    return {
      lang: plan.lang,
      status: FAIL,
      category: APPLICABLE,
      detail: `${label}: ${command} exited 0 but executed 0 tests`,
    };
  }
  if (count === null) {
    return {
      lang: plan.lang,
      status: NA,
      category: NO_TOOL,
      detail: `${label}: ${command} exited 0 but emitted no observable test count`,
    };
  }
  return {
    lang: plan.lang,
    status: PASS,
    category: APPLICABLE,
    detail: `${label}: ${command} PASS (${count} test(s) observed)`,
  };
}

export function executeCommandPlan(plan, exec) {
  const label = plan.label ?? plan.lang;
  if (!plan.applicable) {
    return {
      lang: plan.lang, status: NA, category: INAPPLICABLE, detail: `${label}: ${plan.reason}`,
    };
  }
  if (!plan.configured) {
    return {
      lang: plan.lang, status: NA, category: NO_TOOL, detail: `${label}: ${plan.reason}`,
    };
  }
  const probe = exec(plan.cmd, plan.probeArgs, {}, plan.root);
  if (!probe?.ok) {
    return {
      lang: plan.lang,
      status: NA,
      category: NO_TOOL,
      detail: noToolDetail(plan, `${plan.tool} is missing or cannot execute`),
    };
  }
  const result = exec(plan.cmd, plan.args, {}, plan.root);
  if (!result?.ok && unrunnableTool(plan, result?.out)) {
    return {
      lang: plan.lang,
      status: NA,
      category: NO_TOOL,
      detail: noToolDetail(plan, 'required wrapper/component is not configured'),
    };
  }
  const command = [plan.cmd, ...plan.args].join(' ');
  if (result?.ok) return successfulPlanResult(plan, result, exec, label, command);
  const output = clippedOutput(result?.out);
  const suffix = output ? ` — ${output}` : '';
  return {
    lang: plan.lang,
    status: FAIL,
    category: APPLICABLE,
    detail: `${label}: ${command} FAIL (exit ${result?.code ?? 'unavailable'})${suffix}`,
  };
}
