import { createHash } from 'node:crypto';
import { createRequire } from 'node:module';
import { readFileSync, realpathSync } from 'node:fs';
import { dirname, relative, resolve, sep } from 'node:path';

const SOURCE_EXTENSIONS = ['.ts', '.tsx', '.mts', '.cts', '.js', '.jsx', '.mjs', '.cjs'];

export async function analyzeTypescriptTarget(repoRoot, target) {
  const projectPath = resolve(repoRoot, target.project);
  const targetRoot = resolve(repoRoot, target.root);
  const boundaryProblems = [
    realpathEscape(projectPath, repoRoot),
    realpathEscape(targetRoot, repoRoot),
  ].filter(Boolean);
  if (boundaryProblems.length > 0) return inconclusive(target, boundaryProblems.join('; '));
  const loaded = loadTypeScript(repoRoot, target);
  if (loaded.error) return inconclusive(target, loaded.error);
  const ts = loaded.ts;
  const config = ts.readConfigFile(projectPath, ts.sys.readFile);
  if (config.error) return inconclusive(target, formatDiagnostic(ts, config.error));
  const parsed = ts.parseJsonConfigFileContent(config.config, ts.sys, dirname(projectPath), undefined, projectPath);
  if (parsed.errors.length > 0) {
    return inconclusive(target, parsed.errors.map((item) => formatDiagnostic(ts, item)).join('; '));
  }
  const program = ts.createProgram({ rootNames: parsed.fileNames, options: parsed.options });
  const analyzed = analyzeProgram(ts, program, parsed.options, targetRoot, target.id);
  if (analyzed.files.length === 0) analyzed.diagnostics.push(`target ${target.id}: TypeScript project owns no source under ${target.root}`);
  return {
    adapter: 'typescript', targetId: target.id,
    status: analyzed.diagnostics.length === 0 ? 'analyzed' : 'inconclusive',
    files: analyzed.files,
    diagnostics: [...new Set(analyzed.diagnostics)].sort(),
    toolchain: loaded.compiler,
  };
}

function analyzeProgram(ts, program, options, targetRoot, targetId) {
  const files = [];
  const diagnostics = [];
  const programPaths = new Set();
  for (const source of program.getSourceFiles()) {
    if (!inside(source.fileName, targetRoot)) continue;
    const escaped = realpathEscape(source.fileName, targetRoot);
    if (escaped) diagnostics.push(escaped);
    const localPath = portable(relative(targetRoot, source.fileName));
    programPaths.add(localPath);
    const extraction = extractSource(ts, source, options, targetRoot);
    diagnostics.push(...extraction.diagnostics);
    files.push({
      path: portable(relative(targetRoot, source.fileName)),
      absolutePath: source.fileName,
      isTest: isTestPath(source.fileName),
      edges: extraction.edges,
      metrics: extraction.metrics,
    });
  }
  diagnostics.push(...inventoryOmissions(ts, targetRoot, programPaths, targetId));
  files.sort((a, b) => a.path.localeCompare(b.path));
  return { files, diagnostics };
}

function inventoryOmissions(ts, targetRoot, programPaths, targetId) {
  const diagnostics = [];
  try {
    const discovered = ts.sys.readDirectory(targetRoot, SOURCE_EXTENSIONS, undefined, ['**/*']);
    for (const sourcePath of discovered.sort()) {
      const escaped = realpathEscape(sourcePath, targetRoot);
      if (escaped) diagnostics.push(escaped);
      const localPath = portable(relative(targetRoot, sourcePath));
      if (!programPaths.has(localPath)) {
        diagnostics.push(`${localPath}: source file is omitted by the configured TypeScript program`);
      }
    }
  } catch (error) {
    diagnostics.push(`target ${targetId}: cannot inventory source root (${error.code ?? error.message})`);
  }
  return diagnostics;
}

function loadTypeScript(repoRoot, target) {
  const manifest = findProjectManifest(repoRoot, resolve(repoRoot, target.project));
  if (manifest.error) return { error: `target ${target.id}: ${manifest.error}` };
  const declared = manifest.declared;
  try {
    const require = createRequire(manifest.path);
    const compilerPath = require.resolve('typescript');
    const packagePath = require.resolve('typescript/package.json');
    const ts = require('typescript');
    return {
      ts,
      compiler: {
        version: ts.version,
        declared,
        manifest: portable(relative(repoRoot, manifest.path)),
        entrypoint: portable(relative(dirname(packagePath), compilerPath)),
        sha256: `sha256:${createHash('sha256').update(readFileSync(compilerPath)).digest('hex')}`,
      },
    };
  } catch (error) {
    return {
      error: `target ${target.id}: declared TypeScript compiler API is unavailable (${error.code ?? error.message}); install the project dependency`,
    };
  }
}

function findProjectManifest(repoRoot, projectPath) {
  const root = resolve(repoRoot);
  let cursor = dirname(projectPath);
  let sawManifest = false;
  while (inside(cursor, root)) {
    const path = resolve(cursor, 'package.json');
    try {
      const data = JSON.parse(readFileSync(path, 'utf8'));
      sawManifest = true;
      const declared = ['dependencies', 'devDependencies', 'peerDependencies']
        .map((field) => data[field]?.typescript)
        .find((value) => typeof value === 'string' && value.length > 0);
      if (declared) return { path, data, declared };
    } catch (error) {
      if (error.code !== 'ENOENT' && error.code !== 'ENOTDIR') {
        return { error: `cannot read project package.json ${portable(relative(root, path))} (${error.message})` };
      }
    }
    const parent = dirname(cursor);
    if (parent === cursor) break;
    cursor = parent;
  }
  return { error: sawManifest
    ? 'no project-owned package.json at or above the TypeScript project directly declares typescript'
    : 'no project-owned package.json exists at or above the TypeScript project' };
}

function extractSource(ts, source, options, targetRoot) {
  const edges = [];
  const diagnostics = [];
  const metrics = {
    physicalLines: source.getLineAndCharacterOfPosition(source.end).line + 1,
    topLevelDeclarations: source.statements.filter((node) => !isImportLike(ts, node)).length,
    imports: 0, exports: 0, stateCalls: 0, effectCalls: 0,
    eventHandlers: 0, branchPoints: 0,
  };
  for (const statement of source.statements) metrics.exports += exportCount(ts, statement);
  visit(source);
  edges.sort(edgeOrder);
  return { edges, diagnostics, metrics };

  function visit(node) {
    const declaration = moduleDeclaration(ts, node);
    if (declaration) addEdge(declaration);
    const dynamic = dynamicImport(ts, node);
    if (dynamic?.specifier) addEdge(dynamic);
    else if (dynamic?.computed) diagnostics.push(`${portable(relative(targetRoot, source.fileName))}: computed dynamic import is unresolved`);
    if (ts.isCallExpression(node)) classifyCall(ts, node, metrics);
    if (isHandlerDeclaration(ts, node)) metrics.eventHandlers += 1;
    if (isBranch(ts, node)) metrics.branchPoints += 1;
    ts.forEachChild(node, visit);
  }

  function addEdge(raw) {
    metrics.imports += 1;
    const resolved = ts.resolveModuleName(raw.specifier, source.fileName, options, ts.sys).resolvedModule;
    if (!resolved) {
      if (looksInternal(raw.specifier, options)) diagnostics.push(`${portable(relative(targetRoot, source.fileName))}: unresolved internal import ${raw.specifier}`);
      return;
    }
    const targetFile = resolved.resolvedFileName;
    if (!inside(targetFile, targetRoot)) return;
    edges.push({
      specifier: raw.specifier,
      targetPath: portable(relative(targetRoot, targetFile)),
      kind: raw.kind,
      importedSymbols: raw.importedSymbols.sort(),
    });
  }
}

function moduleDeclaration(ts, node) {
  if (ts.isImportDeclaration(node) && ts.isStringLiteralLike(node.moduleSpecifier)) {
    return {
      specifier: node.moduleSpecifier.text,
      kind: importKind(ts, node.importClause),
      importedSymbols: importSymbols(ts, node.importClause),
    };
  }
  if (ts.isExportDeclaration(node) && node.moduleSpecifier && ts.isStringLiteralLike(node.moduleSpecifier)) {
    return {
      specifier: node.moduleSpecifier.text,
      kind: node.isTypeOnly ? 'type' : 'reexport',
      importedSymbols: exportSymbols(ts, node.exportClause),
    };
  }
  if (ts.isImportEqualsDeclaration(node) && ts.isExternalModuleReference(node.moduleReference)) {
    const expression = node.moduleReference.expression;
    if (expression && ts.isStringLiteralLike(expression)) {
      return { specifier: expression.text, kind: node.isTypeOnly ? 'type' : 'value', importedSymbols: [node.name.text] };
    }
  }
  return null;
}

function dynamicImport(ts, node) {
  if (!ts.isCallExpression(node)) return null;
  const isImport = node.expression.kind === ts.SyntaxKind.ImportKeyword;
  const isRequire = ts.isIdentifier(node.expression) && node.expression.text === 'require';
  if (!isImport && !isRequire) return null;
  const arg = node.arguments[0];
  if (!arg || !ts.isStringLiteralLike(arg)) return { computed: true };
  return { specifier: arg.text, kind: isImport ? 'dynamic_literal' : 'value', importedSymbols: [] };
}

function importKind(ts, clause) {
  if (!clause) return 'side_effect';
  if (clause.isTypeOnly) return 'type';
  const bindings = clause.namedBindings;
  if (bindings && ts.isNamedImports(bindings) && bindings.elements.length > 0
      && bindings.elements.every((item) => item.isTypeOnly)) return 'type';
  return 'value';
}

function importSymbols(ts, clause) {
  if (!clause) return [];
  const out = [];
  if (clause.name) out.push('default');
  const bindings = clause.namedBindings;
  if (bindings && ts.isNamespaceImport(bindings)) out.push('*');
  if (bindings && ts.isNamedImports(bindings)) out.push(...bindings.elements.map((item) => item.propertyName?.text ?? item.name.text));
  return out;
}

function exportSymbols(ts, clause) {
  if (!clause) return ['*'];
  if (ts.isNamespaceExport(clause)) return [`* as ${clause.name.text}`];
  return clause.elements.map((item) => item.propertyName?.text ?? item.name.text);
}

function exportCount(ts, node) {
  if (ts.isExportDeclaration(node)) {
    if (!node.exportClause) return 1;
    if (ts.isNamespaceExport(node.exportClause)) return 1;
    return node.exportClause.elements.length;
  }
  const modifiers = ts.canHaveModifiers(node) ? ts.getModifiers(node) : undefined;
  if (!modifiers?.some((item) => item.kind === ts.SyntaxKind.ExportKeyword)) return 0;
  if (ts.isVariableStatement(node)) return node.declarationList.declarations.length;
  return 1;
}

function classifyCall(ts, node, metrics) {
  const name = callName(ts, node.expression);
  if (/^(useState|useReducer|ref|reactive|signal|StateProvider|NotifierProvider)$/i.test(name)) metrics.stateCalls += 1;
  if (/^(useEffect|useLayoutEffect|watch|watchEffect|addPostFrameCallback)$/i.test(name)) metrics.effectCalls += 1;
}

function callName(ts, expression) {
  if (ts.isIdentifier(expression)) return expression.text;
  if (ts.isPropertyAccessExpression(expression)) return expression.name.text;
  return '';
}

function isHandlerDeclaration(ts, node) {
  if (ts.isFunctionDeclaration(node) && node.name) return /^(handle|on)[A-Z_]/.test(node.name.text);
  if (!ts.isVariableDeclaration(node) || !ts.isIdentifier(node.name)) return false;
  return /^(handle|on)[A-Z_]/.test(node.name.text)
    && Boolean(node.initializer && (ts.isArrowFunction(node.initializer) || ts.isFunctionExpression(node.initializer)));
}

function isBranch(ts, node) {
  return ts.isIfStatement(node) || ts.isConditionalExpression(node)
    || ts.isForStatement(node) || ts.isForInStatement(node) || ts.isForOfStatement(node)
    || ts.isWhileStatement(node) || ts.isDoStatement(node) || ts.isCaseClause(node)
    || ts.isCatchClause(node)
    || (ts.isBinaryExpression(node) && [
      ts.SyntaxKind.AmpersandAmpersandToken, ts.SyntaxKind.BarBarToken,
      ts.SyntaxKind.QuestionQuestionToken,
    ].includes(node.operatorToken.kind));
}

function isImportLike(ts, node) {
  return ts.isImportDeclaration(node) || ts.isImportEqualsDeclaration(node);
}

function looksInternal(specifier, options) {
  if (specifier.startsWith('.')) return true;
  const paths = Object.keys(options.paths ?? {});
  return paths.some((pattern) => {
    const prefix = pattern.split('*')[0];
    return prefix.length > 0 && specifier.startsWith(prefix);
  });
}

function isTestPath(path) {
  const portablePath = portable(path);
  return /(?:^|\/)(__tests__|test|tests)(?:\/|$)/.test(portablePath)
    || /\.(?:test|spec)\.[cm]?[jt]sx?$/.test(portablePath);
}

function realpathEscape(file, root) {
  try {
    const actualRoot = realpathSync(root);
    const actualFile = realpathSync(file);
    return inside(actualFile, actualRoot) ? null : `${portable(file)}: realpath escapes target root`;
  } catch (error) {
    return `${portable(file)}: cannot resolve realpath (${error.code ?? error.message})`;
  }
}

function inside(path, root) {
  const full = resolve(path);
  const base = resolve(root);
  return full === base || full.startsWith(base + sep);
}

function edgeOrder(a, b) {
  return `${a.targetPath}\0${a.kind}\0${a.specifier}`.localeCompare(`${b.targetPath}\0${b.kind}\0${b.specifier}`);
}

function formatDiagnostic(ts, diagnostic) {
  return ts.flattenDiagnosticMessageText(diagnostic.messageText, ' ');
}

function inconclusive(target, message) {
  return { adapter: target.adapter, targetId: target.id, status: 'inconclusive', files: [], diagnostics: [message] };
}

function portable(path) {
  return path.split(sep).join('/');
}
