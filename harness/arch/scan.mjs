// ForgeOS arch-scan — shared, mostly-pure library for the architecture checks.
//
// Single responsibility: turn a source tree into a normalized model the four
// checks consume — a list of {file, lang, layer, dir, imports[]} records plus a
// resolved import graph (edges file -> target with the target's layer).
//
// Zero third-party deps (node: builtins only). Pure helpers (classifyLang,
// classifyLayer, extractImports, parseRules) take their inputs explicitly so
// they are unit-testable without touching the filesystem; only walkSource and
// scan() do I/O.
import { readFileSync, readdirSync, statSync } from 'node:fs';
import { join, extname, relative, dirname, sep } from 'node:path';
// Function-body parsing (the function-length budget's heuristic extractor) lives
// in a sibling leaf module to keep both files under the file-size budget. scan()
// calls extractFunctions; it is re-exported so existing `from './scan.mjs'`
// importers (arch-check's tests) keep working unchanged.
import { extractFunctions } from './scan-functions.mjs';
export { extractFunctions };

// Directories never worth scanning (build output, vcs, vendored, fixtures,
// forge-core's git-ignored runtime state in .forge/).
export const SKIP_DIRS = new Set([
  'node_modules', '.git', 'dist', 'build', '.next', 'coverage',
  'vendor', 'testdata', '__pycache__', '.forge',
]);

// Source extensions we understand, mapped to a coarse language tag.
const LANG_BY_EXT = new Map([
  ['.go', 'go'], ['.mjs', 'js'], ['.cjs', 'js'], ['.js', 'js'],
  ['.jsx', 'js'], ['.ts', 'ts'], ['.tsx', 'ts'], ['.py', 'py'],
]);

// --- pure classification -----------------------------------------------------

// classifyLang: extension -> language tag, or null when unrecognized.
export function classifyLang(file) {
  return LANG_BY_EXT.get(extname(file)) ?? null;
}

// isTestFile: best-effort, per-language test-file detection. Test files are
// scanned for layering (a test that violates direction is still a smell) but
// excluded from package export/size budgets where noted by each check.
export function isTestFile(file) {
  const base = file.split(/[\\/]/).pop();
  return (
    base.endsWith('_test.go') ||
    base.endsWith('.test.mjs') || base.endsWith('.test.js') ||
    base.endsWith('_test.py') || base.startsWith('test_') ||
    file.split(/[\\/]/).includes('test')
  );
}

// classifyLayer: map a file's path onto a canonical layer using rules.dir_aliases.
// The layer is the FIRST path segment (outermost-to-innermost as written on
// disk) that has an alias; unmapped files get layer null (excluded from
// layering checks — e.g. forge-core's internal/<pkg> dirs are not layered).
export function classifyLayer(relPath, aliases) {
  for (const seg of relPath.split(/[\\/]/)) {
    if (Object.prototype.hasOwnProperty.call(aliases, seg)) return aliases[seg];
  }
  return null;
}

// --- pure import extraction --------------------------------------------------

// extractImports: dispatch to the per-language extractor. Returns an array of
// raw import specifiers (the string between the quotes / after `import`).
export function extractImports(text, lang) {
  if (lang === 'go') return extractGoImports(text);
  if (lang === 'js' || lang === 'ts') return extractJsImports(text);
  if (lang === 'py') return extractPyImports(text);
  return [];
}

// Go: both block form `import ( "a"\n "b" )` and single `import "a"`.
function extractGoImports(text) {
  const out = [];
  const block = /import\s*\(([\s\S]*?)\)/g;
  let m;
  while ((m = block.exec(text)) !== null) {
    for (const line of m[1].split('\n')) {
      const q = line.match(/"([^"]+)"/);
      if (q) out.push(q[1]);
    }
  }
  const single = /^\s*import\s+(?:[A-Za-z0-9_.]+\s+)?"([^"]+)"/gm;
  while ((m = single.exec(text)) !== null) out.push(m[1]);
  return out;
}

// JS/TS import-graph edges. Beyond `import ... from`, bare `import '...'`, and
// `require('...')`, this ALSO captures the re-export and dynamic forms that
// barrel/index modules use — without them a layering violation or import cycle
// routed THROUGH a re-export is invisible to the graph (a real false negative):
//   * `export ... from '...'`        — `export { a } from`, `export * from`,
//                                       `export * as ns from` (one `from` regex)
//   * `import('...')`                — dynamic import (await import / import().then)
// All feed the SAME resolve->layer pipeline as static imports (no new mechanism).
function extractJsImports(text) {
  const out = [];
  // `import ... from '...'` AND `export ... from '...'` (incl. `export *` /
  // `export * as ns`): both end in `<clause> from '<spec>'`, so one regex with an
  // `import|export` head covers static imports and every re-export form.
  const from = /(?:import|export)\s+[\s\S]*?\sfrom\s+['"]([^'"]+)['"]/g;
  const bare = /import\s+['"]([^'"]+)['"]/g;
  // Dynamic `import('...')`: the `import` keyword followed directly by `(`
  // distinguishes the call form from the static `import x from` above.
  const dyn = /\bimport\s*\(\s*['"]([^'"]+)['"]\s*\)/g;
  const req = /require\(\s*['"]([^'"]+)['"]\s*\)/g;
  for (const re of [from, bare, dyn, req]) {
    let m;
    while ((m = re.exec(text)) !== null) out.push(m[1]);
  }
  return out;
}

// Python: `import a.b.c` and `from a.b import c`. We keep the module path; the
// leading-dot relative form (`from .store import x`) is preserved for resolution.
function extractPyImports(text) {
  const out = [];
  const imp = /^\s*import\s+([A-Za-z0-9_.]+)/gm;
  const from = /^\s*from\s+([A-Za-z0-9_.]+)\s+import\s+/gm;
  let m;
  while ((m = imp.exec(text)) !== null) out.push(m[1]);
  while ((m = from.exec(text)) !== null) out.push(m[1]);
  return out;
}

// --- pure export counting (Go) ----------------------------------------------

// countGoExports: exported (Capitalized) top-level func/method/type/var/const.
// Used by package-check's max_exports budget. Pure over the file text.
export function countGoExports(text) {
  let n = 0;
  for (const line of text.split('\n')) {
    if (
      /^func\s+[A-Z]/.test(line) ||
      /^func\s+\([^)]*\)\s+[A-Z]/.test(line) ||
      /^type\s+[A-Z]/.test(line) ||
      /^var\s+[A-Z]/.test(line) ||
      /^const\s+[A-Z]/.test(line)
    ) {
      n += 1;
    }
  }
  return n;
}

// --- minimal YAML reader for .arch/rules.yaml --------------------------------
// Not a general YAML parser — just the nested-map + list shapes rules.yaml uses
// (2-space indentation, `key: value`, `key:` parents, `- item` lists). Keeps the
// whole toolchain zero-dep instead of leaning on PyYAML/yaml2json.
export function parseRules(text) {
  const root = {};
  // Each frame: {indent, parent, key} where parent[key] is the container the
  // frame's children populate. The root frame's container is `root` itself.
  const stack = [{ indent: -1, parent: null, key: null, container: root }];
  for (const raw of text.split('\n')) {
    const line = stripComment(raw);
    if (!line.trim()) continue;
    const indent = line.length - line.trimStart().length;
    const item = line.trim();
    while (stack.length > 1 && indent <= stack[stack.length - 1].indent) stack.pop();
    const top = stack[stack.length - 1];
    if (item.startsWith('- ')) {
      appendListItem(top, item.slice(2));
    } else {
      addMapEntry(stack, top, indent, item);
    }
  }
  return root;
}

// A list item turns its enclosing container into an array on first sight and
// appends the coerced value. The container was opened by a bare `key:` parent.
function appendListItem(top, value) {
  if (!Array.isArray(top.container)) {
    const arr = [];
    top.parent[top.key] = arr;
    top.container = arr;
  }
  top.container.push(coerce(value));
}

// A `key: value` (or bare `key:`) entry. Bare keys open a new child container
// frame; valued keys assign a coerced scalar into the current container.
function addMapEntry(stack, top, indent, item) {
  const [rawKey, ...rest] = item.split(':');
  const key = rawKey.trim();
  const value = rest.join(':').trim();
  if (value === '') {
    const child = {};
    top.container[key] = child;
    stack.push({ indent, parent: top.container, key, container: child });
  } else {
    top.container[key] = coerce(value);
  }
}

function stripComment(line) {
  // Strip `#` comments not inside quotes (rules.yaml uses no `#` in values).
  let inQuote = false;
  for (let i = 0; i < line.length; i += 1) {
    const c = line[i];
    if (c === '"' || c === "'") inQuote = !inQuote;
    else if (c === '#' && !inQuote) return line.slice(0, i);
  }
  return line;
}

function coerce(v) {
  const s = v.replace(/^['"]|['"]$/g, '');
  if (/^-?\d+$/.test(s)) return Number(s);
  return s;
}

// --- filesystem walk ---------------------------------------------------------

// walkSource: recursively collect source-file absolute paths under root,
// skipping SKIP_DIRS. Tolerant of broken symlinks / permission errors.
export function walkSource(root, acc = []) {
  let entries;
  try {
    entries = readdirSync(root);
  } catch {
    return acc;
  }
  for (const name of entries) {
    if (SKIP_DIRS.has(name)) continue;
    const full = join(root, name);
    let st;
    try { st = statSync(full); } catch { continue; }
    if (st.isDirectory()) walkSource(full, acc);
    else if (classifyLang(full)) acc.push(full);
  }
  return acc;
}

// --- import resolution -------------------------------------------------------

// resolveTarget: turn one raw import specifier into a target descriptor
// {kind, dir, layer} where kind is 'internal' (resolved to a tree dir) or
// 'external' (stdlib / third-party / unresolved). Layer is derived from the
// resolved dir via aliases; external targets carry layer null.
export function resolveTarget(spec, fromFile, ctx) {
  const { root, aliases, goModules } = ctx;
  if (spec.startsWith('.')) return resolveRelative(spec, fromFile, root, aliases);
  const goDir = resolveGoModule(spec, goModules);
  if (goDir) {
    const rel = relative(root, goDir);
    return { kind: 'internal', dir: goDir, rel, layer: classifyLayer(rel, aliases) };
  }
  return { kind: 'external', dir: null, rel: spec, layer: null };
}

// Relative JS/py imports resolve against the importing file's directory.
function resolveRelative(spec, fromFile, root, aliases) {
  const base = dirname(fromFile);
  const norm = pyDots(spec);
  const targetPath = join(base, norm);
  const targetDir = extname(targetPath) ? dirname(targetPath) : targetPath;
  const rel = relative(root, targetPath);
  return {
    kind: 'internal', dir: targetDir, rel,
    layer: classifyLayer(relative(root, targetDir), aliases),
  };
}

// Python `from . import x` / `from ..pkg import y` -> filesystem path fragment.
function pyDots(spec) {
  if (!/^\.+[A-Za-z]/.test(spec) && !/^\.+$/.test(spec) && spec.includes('/')) return spec;
  const lead = spec.match(/^\.+/);
  if (!lead) return spec;
  const ups = lead[0].length - 1;
  const rest = spec.slice(lead[0].length).replace(/\./g, sep);
  return join('../'.repeat(ups) || './', rest);
}

// resolveGoModule: a Go import path under a known module prefix maps to a dir
// inside that module. Returns the absolute dir or null when not internal.
function resolveGoModule(spec, goModules) {
  for (const { module, dir } of goModules) {
    if (spec === module) return dir;
    if (spec.startsWith(module + '/')) return join(dir, spec.slice(module.length + 1));
  }
  return null;
}

// findGoModules: locate every go.mod under root and read its module path, so Go
// imports can be resolved to on-disk dirs. Returns [{module, dir}].
export function findGoModules(root) {
  const mods = [];
  for (const f of walkAll(root)) {
    if (!f.endsWith('go.mod')) continue;
    try {
      const m = readFileSync(f, 'utf8').match(/^\s*module\s+(\S+)/m);
      if (m) mods.push({ module: m[1], dir: dirname(f) });
    } catch { /* unreadable go.mod — skip */ }
  }
  // Longest module path first so nested modules resolve before their parent.
  return mods.sort((a, b) => b.module.length - a.module.length);
}

// walkAll: like walkSource but returns ALL files (needed to find go.mod).
function walkAll(root, acc = []) {
  let entries;
  try { entries = readdirSync(root); } catch { return acc; }
  for (const name of entries) {
    if (SKIP_DIRS.has(name)) continue;
    const full = join(root, name);
    let st;
    try { st = statSync(full); } catch { continue; }
    if (st.isDirectory()) walkAll(full, acc);
    else acc.push(full);
  }
  return acc;
}

// --- top-level scan ----------------------------------------------------------

// scan: build the full model for `root`. Returns { root, files, modules }.
// Each file record:
//   { file, rel, lang, layer, dir, isTest, exports, imports[], functions[] }
// where each import is { spec, kind, dir, rel, layer } and each function is
// { name, line, lines } (see extractFunctions).
export function scan(root, rules) {
  const aliases = (rules?.architecture?.dir_aliases) ?? {};
  const goModules = findGoModules(root);
  const ctx = { root, aliases, goModules };
  const files = [];
  for (const file of walkSource(root)) {
    const rel = relative(root, file);
    const lang = classifyLang(file);
    let text;
    try { text = readFileSync(file, 'utf8'); } catch { continue; }
    const imports = extractImports(text, lang).map((spec) => ({
      spec, ...resolveTarget(spec, file, ctx),
    }));
    files.push({
      file, rel, lang,
      dir: dirname(file),
      layer: classifyLayer(rel, aliases),
      isTest: isTestFile(file),
      exports: lang === 'go' ? countGoExports(text) : 0,
      imports,
      functions: extractFunctions(text, lang),
    });
  }
  return { root, files, modules: goModules };
}
