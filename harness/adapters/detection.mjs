// Source-language detection for executable harness adapters.
//
// This intentionally does not reuse arch/scan.mjs: the architecture scanner
// understands only languages whose imports/functions it can analyse, while an
// acceptance adapter only needs to identify a source ecosystem. Coupling the
// two made adding a Rust/Java adapter a false declaration: .rs/.java files were
// filtered out before adapters.mjs could classify them.
//
// Zero dependencies; symlink directories are not followed and generated/vendor
// trees are skipped. A scan rooted inside a fixture still works, while a scan of
// the repository skips its testdata subtree.
import { readdirSync } from 'node:fs';
import { extname, join } from 'node:path';

const LANG_BY_EXT = new Map([
  ['.go', 'go'],
  ['.mjs', 'typescript'], ['.cjs', 'typescript'], ['.js', 'typescript'],
  ['.jsx', 'typescript'], ['.ts', 'typescript'], ['.tsx', 'typescript'],
  ['.py', 'python'],
  ['.rs', 'rust'],
  ['.java', 'java'],
]);

const SOURCE_SKIP_DIRS = new Set([
  '.git', '.forge', '.gradle', '.idea', '.venv', 'node_modules', 'vendor',
  'dist', 'build', 'coverage', 'target', 'out', 'testdata', '__pycache__',
]);

export const ADAPTER_LANGS = ['go', 'java', 'python', 'rust', 'typescript'];

export function langForExt(ext) {
  return LANG_BY_EXT.get(ext) ?? null;
}

export function walkAdapterSources(root, acc = [], options = {}) {
  const readDir = options.readDir ?? readdirSync;
  let entries;
  try {
    entries = readDir(root, { withFileTypes: true });
  } catch (err) {
    if (!options.onError) throw err;
    options.onError(root, err);
    return acc;
  }
  for (const entry of entries) {
    if (entry.isDirectory()) {
      if (!SOURCE_SKIP_DIRS.has(entry.name)) {
        walkAdapterSources(join(root, entry.name), acc, options);
      }
      continue;
    }
    if (!entry.isFile()) continue;
    if (langForExt(extname(entry.name))) acc.push(join(root, entry.name));
  }
  return acc;
}

export function detectLanguages(root, options = {}) {
  const langs = new Set();
  for (const file of walkAdapterSources(root, [], options)) {
    const lang = langForExt(extname(file));
    if (lang) langs.add(lang);
  }
  return [...langs].sort();
}

export function hasLanguageSource(root, lang, options = {}) {
  return walkAdapterSources(root, [], options)
    .some((file) => langForExt(extname(file)) === lang);
}
