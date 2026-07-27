// ForgeOS arch-scan / function-body parsing — the function-length budget's
// heuristic extractor, split out of scan.mjs to keep both files comfortably
// under the file-size budget. Pure (no I/O), zero third-party deps. A leaf of
// scan.mjs: scan() imports extractFunctions from here; nothing here imports back.

// --- pure function extraction (HEURISTIC) ------------------------------------
// extractFunctions: best-effort declared functions with body line counts, for the
// function-length budget. Returns [{name, line, lines}] — `line` is the 1-based
// start, `lines` the inclusive body span.
// HONESTY: this is a regex + brace/indent heuristic, NOT a real AST parser. It
// targets DECLARED functions (Go `func`, Rust `fn`, JS
// `function`/arrow-binding/method, Python `def`); inline anonymous callbacks
// count as part of their ENCLOSING
// function, not separately. Brace counting threads a lexer state across lines
// (braceDelta/matchBrace), so braces inside multi-line Go raw strings / JS
// template literals (backtick) and `/* */` block comments are skipped — closing
// the multi-line false-NEGATIVE where a >N-line body was miscounted to <=N.
// Multi-line arrow signatures (`const f = (\n…\n) => {`) are recognized too (see
// multiLineArrowName). KNOWN GAP (pinned by a test, not pretended): a `{` inside
// a JS `${ }` interpolation is opaque string text and skipped — this only
// UNDER-counts, never a false PASS. A real linter is a v3 enhancement (ROADMAP).
export function extractFunctions(text, lang) {
  if (lang === 'go' || lang === 'rust' || lang === 'js' || lang === 'ts') {
    return braceFunctions(text, lang);
  }
  if (lang === 'py') return indentFunctions(text);
  return [];
}

// Header detectors for brace-delimited languages, each returning a function name
// (or null). Declaration-only, so inline callbacks add no phantom functions — the
// enclosing function's brace span already covers them.
// The trailing `[[(]` admits a Go generic free function `func Map[T any](...)`
// (the name is followed by a type-parameter `[`) alongside the ordinary `(` param
// list. Go has NO `func Name<` form, so the previous `[<(]` (a stray `<`) matched
// nothing useful and DROPPED every generic free function — a >50-line generic body
// then bypassed the function-length budget entirely. Generic METHODS are
// unaffected: the receiver `(...)` is consumed by the optional group above, then
// the param-list `(` satisfies the class.
const GO_HEADER = /^\s*func\s+(?:\([^)]*\)\s*)?([A-Za-z0-9_]+)?\s*[[(]/;
const RUST_HEADER =
  /^\s*(?:pub(?:\([^)]*\))?\s+)?(?:const\s+)?(?:async\s+)?(?:unsafe\s+)?fn\s+([A-Za-z0-9_]+)/;
const JS_HEADER =
  /^\s*(?:export\s+)?(?:default\s+)?(?:async\s+)?function\s*\*?\s*([A-Za-z0-9_$]+)?/;
const JS_BOUND = // `const f = (..) =>` / `const f = function` / `f = async (..) =>`
  /^\s*(?:export\s+)?(?:const|let|var)\s+([A-Za-z0-9_$]+)\s*=\s*(?:async\s+)?(?:function|\([^)]*\)\s*(?::[^=]+)?=>|[A-Za-z0-9_$]+\s*=>)/;
const JS_METHOD = // class/object method `name(args) {` — excludes keywords like if/for/while/switch/catch
  /^\s*(?:async\s+|static\s+|get\s+|set\s+|\*\s*)*([A-Za-z0-9_$]+)\s*\([^;]*\)\s*\{/;
const JS_KEYWORD = new Set(['if', 'for', 'while', 'switch', 'catch', 'return', 'function']);
// JS_BOUND_OPEN: an arrow binding whose parenthesized signature OPENS but does
// not complete on this line — `const f = (` / `export const f = async (`, no `=>`
// yet. Captures the name; multiLineArrowName confirms a `=>` follows before the
// body `{` so a parenthesized expression `const x = (\n a+b\n);` is NOT taken as
// a function. Closes the multi-line-arrow gap the single-line JS_BOUND missed.
const JS_BOUND_OPEN =
  /^\s*(?:export\s+)?(?:const|let|var)\s+([A-Za-z0-9_$]+)\s*=\s*(?:async\s+)?\([^)]*$/;

function jsHeaderName(line) {
  let m = line.match(JS_HEADER);
  if (m) return m[1] ?? '(anonymous)';
  m = line.match(JS_BOUND);
  if (m) return m[1];
  m = line.match(JS_METHOD);
  if (m && !JS_KEYWORD.has(m[1])) return m[1];
  return null;
}

// multiLineArrowName: for a binding whose arrow signature spans lines, return its
// name iff a `=>` precedes the body `{` (an arrow, not a `(expr)`). Scans only the
// ~12-line window matchBrace allows.
function multiLineArrowName(lines, start) {
  const m = lines[start].match(JS_BOUND_OPEN);
  if (!m) return null;
  for (let i = start; i < lines.length && i - start <= 12; i += 1) {
    const line = lines[i];
    const arrow = line.indexOf('=>');
    const brace = line.indexOf('{');
    if (arrow !== -1 && (brace === -1 || arrow < brace)) return m[1]; // arrow before body
    if (brace !== -1) return null; // body opened with no preceding `=>` — not an arrow
  }
  return null;
}

// braceFunctions: walk lines; on a header (incl. a multi-line arrow binding),
// match its `}` by brace depth, then skip past the body so a nested declaration
// is NOT reported as a second top-level function.
function braceFunctions(text, lang) {
  const lines = text.split('\n');
  const out = [];
  for (let i = 0; i < lines.length; i += 1) {
    let name;
    if (lang === 'go') name = goHeaderName(lines[i]);
    else if (lang === 'rust') name = rustHeaderName(lines[i]);
    else name = jsHeaderName(lines[i]);
    if (name === null && lang !== 'go' && lang !== 'rust') {
      name = multiLineArrowName(lines, i);
    }
    if (name === null) continue;
    const end = matchBrace(lines, i, lang);
    if (end === null) continue; // no body found (e.g. interface method decl) — skip
    out.push({ name, line: i + 1, lines: end - i + 1 });
    i = end; // resume after this function's body; nested funcs are inside the span
  }
  return out;
}

function goHeaderName(line) {
  const m = line.match(GO_HEADER);
  return m ? (m[1] ?? '(closure)') : null;
}

function rustHeaderName(line) {
  return line.match(RUST_HEADER)?.[1] ?? null;
}

// matchBrace: from header line `start`, find the line index of the `}` closing
// the function's first `{`, or null if no `{` opens within ~12 lines (a body-less
// declaration). THREADS a lexer state across lines via braceDelta() so braces in
// a multi-line backtick string or `/* */` block comment do NOT inflate the span.
function matchBrace(lines, start, lang) {
  let depth = 0;
  let opened = false;
  let state = { quote: null, block: false }; // multi-line string/comment carried across lines
  for (let i = start; i < lines.length; i += 1) {
    const r = braceDelta(lines[i], lang, state);
    state = r.state;
    if (r.open) opened = true;
    depth += r.delta;
    // Close only when depth returns to <=0 OUTSIDE any open string/comment.
    if (opened && depth <= 0 && !state.quote && !state.block) return i;
    if (!opened && i - start > 12) return null; // signature should open within ~12 lines
  }
  return opened ? lines.length - 1 : null;
}

// braceDelta: net `{`/`}` on one line, ignoring chars inside strings, `//`/`#`
// line comments, and `/* */` block comments. Threads `state = {quote, block}` IN
// and OUT so multi-line constructs carry across lines: a backtick quote (Go raw
// string / JS template) and an open block comment persist; single-line `"`/`'`
// reset at EOL (unless trailing `\`). `open` flags a real `{`; `lang` selects
// backtick escapes (JS template honors `\`; a Go raw string does not).
function braceDelta(line, lang, state = { quote: null, block: false }) {
  let delta = 0;
  let open = false;
  let { quote, block } = state;
  for (let i = 0; i < line.length; i += 1) {
    const c = line[i];
    if (block) { // inside /* ... */ — only `*/` ends it; everything else skipped
      if (c === '*' && line[i + 1] === '/') { block = false; i += 1; }
      continue;
    }
    if (quote) {
      // A Go raw string (backtick) takes no escapes; every other recognized
      // quoted string honors `\`.
      if (c === '\\' && !(lang === 'go' && quote === '`')) { i += 1; continue; }
      if (c === quote) quote = null;
      continue;
    }
    if (c === '/' && line[i + 1] === '*') { block = true; i += 1; continue; } // /* block
    if (c === '"' || c === "'" || c === '`') { quote = c; continue; }
    if (c === '/' && line[i + 1] === '/') break; // JS/Go line comment
    if (c === '#') break; // Python line comment (harmless for brace langs)
    if (c === '{') { delta += 1; open = true; }
    else if (c === '}') delta -= 1;
  }
  // `"`/`'` do not span lines (except a trailing `\` continuation): drop an
  // unterminated one so a stray quote cannot poison the rest of the body. A
  // backtick (multi-line raw/template) and a block comment intentionally carry.
  if ((quote === '"' || quote === "'") && !line.endsWith('\\')) quote = null;
  return { open, delta, state: { quote, block } };
}

// indentFunctions (Python): a `def` owns every following line that is blank or
// indented deeper than the `def` keyword, up to the next line at <= its indent.
// Nested defs are counted independently (each is a function worth bounding), so
// here we do NOT skip the body — every `def` line is its own entry. The optional
// `async ` prefix is matched (NOT captured by the indent group): without it every
// FastAPI/asyncio `async def` — the standard async form — was INVISIBLE to the
// function-length budget, so a 500-line coroutine never tripped the 50-line cap.
function indentFunctions(text) {
  const lines = text.split('\n');
  const out = [];
  for (let i = 0; i < lines.length; i += 1) {
    const m = lines[i].match(/^(\s*)(?:async\s+)?def\s+([A-Za-z0-9_]+)/);
    if (!m) continue;
    const indent = m[1].length;
    let end = i;
    for (let j = i + 1; j < lines.length; j += 1) {
      const t = lines[j];
      if (t.trim() === '') continue; // blank lines belong to the body
      if (t.length - t.trimStart().length <= indent) break; // dedent ends the body
      end = j;
    }
    out.push({ name: m[2], line: i + 1, lines: end - i + 1 });
  }
  return out;
}
