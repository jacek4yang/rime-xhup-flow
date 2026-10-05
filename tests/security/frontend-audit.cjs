// Never hide raw advisories. One source-bound local mitigation; all others fail.
const fs = require('node:fs');
const path = require('node:path');
const assert = require('node:assert/strict');
const { createHash } = require('node:crypto');
const { createRequire } = require('node:module');
const { spawnSync } = require('node:child_process');
const pins = require('./braces-upstream.json');
const advisoryId = 'GHSA-vfj7-8cjw-p6xm';
const sha256 = data => createHash('sha256').update(data).digest('hex');

function verifyInstalled(root) {
  const files = [];
  function visit(dir) {
    for (const entry of fs.readdirSync(dir, { withFileTypes: true })) {
      const file = path.join(dir, entry.name);
      assert(!entry.isSymbolicLink(), 'unexpected symlink in braces source');
      if (entry.isDirectory()) visit(file);
      else files.push(path.relative(root, file).split(path.sep).join('/'));
    }
  }
  visit(root);
  assert.deepEqual(files.sort(), Object.keys(pins.files).sort(), 'braces inventory drift');
  for (const file of files) {
    let bytes = fs.readFileSync(path.join(root, file));
    if (['lib/compile.js', 'lib/expand.js', 'lib/stringify.js'].includes(file)) {
      let source = bytes.toString('utf8');
      const fn = file.endsWith('stringify.js') ? 'stringify' : 'walk';
      const declaration = `const ${fn} = (node, parent = {}, depth = 0) => {\n    // Bound recursive AST traversal even for caller-supplied ASTs.\n    if (depth > 64) {\n      throw new RangeError('braces nesting exceeds safety limit (64)');\n    }`;
      const recursive = `${fn}(child, ${fn === 'stringify' ? '{}' : 'node'}, depth + 1)`;
      for (const needle of [declaration, recursive]) assert.equal(source.split(needle).length, 2, 'missing/changed depth guard');
      source = source.replace(declaration, `const ${fn} = (node, parent = {}) => {`)
        .replace(recursive, fn === 'stringify' ? 'stringify(child)' : 'walk(child, node)');
      bytes = Buffer.from(source);
    }
    assert.equal(sha256(bytes), pins.files[file], `upstream identity drift: ${file}`);
  }
  return { version: '3.0.3', upstreamFiles: files.length, boundedDepth: 64 };
}

// Walk reachable installed packages rather than accepting stale pnpm store entries.
// Read package manifests without executing dependency code or ignoring exports rules.
function installedBraces(root) {
  const roots = ['.', 'trainer', 'miniapp', ...fs.readdirSync(path.join(root, 'packages')).map(p => 'packages/' + p)]
    .map(p => path.join(root, p)).filter(p => fs.existsSync(path.join(p, 'package.json')));
  const workspaceConfig = fs.readFileSync(path.join(root, 'pnpm-workspace.yaml'), 'utf8');
  const queue = roots.map(p => [fs.realpathSync(p), true]);
  const seen = new Set();
  const matches = [];
  while (queue.length) {
    const [dir, workspace] = queue.pop();
    if (seen.has(dir)) continue;
    seen.add(dir);
    const manifest = JSON.parse(fs.readFileSync(path.join(dir, 'package.json'), 'utf8'));
    if (manifest.name === 'braces') {
      assert.equal(manifest.version, '3.0.3', 'unreviewed braces version');
      matches.push(dir);
    }
    const dependencies = { ...manifest.dependencies, ...manifest.optionalDependencies, ...manifest.peerDependencies,
      ...(workspace ? manifest.devDependencies : {}) };
    const requireFrom = createRequire(path.join(dir, 'package.json'));
    for (const name of Object.keys(dependencies)) {
      const found = requireFrom.resolve.paths(name + '/package.json')?.map(p => path.join(p, name))
        .find(p => fs.existsSync(path.join(p, 'package.json')));
      if (found) queue.push([fs.realpathSync(found), false]);
      else {
        const explicitlyRemoved = workspaceConfig.split('\n').some(line =>
          line.trim() === `'${manifest.name}>${name}': '-'`);
        assert(explicitlyRemoved || manifest.optionalDependencies?.[name] || manifest.peerDependencies?.[name], `missing dependency ${name} of ${manifest.name}`);
      }
    }
  }
  assert(matches.length > 0, 'no installed braces found');
  return matches;
}

function assessAudit(audit, exitCode, verified) {
  assert(!audit.error && audit.advisories && audit.metadata?.vulnerabilities, 'invalid/error audit response');
  const entries = Object.values(audit.advisories);
  assert.equal(Object.values(audit.metadata.vulnerabilities).reduce((a, b) => a + b, 0), entries.length, 'audit count mismatch');
  assert.equal(exitCode, entries.length ? 1 : 0, 'unexpected audit exit status');
  for (const item of entries) {
    assert.equal(item.github_advisory_id, advisoryId, 'unmitigated advisory');
    assert.equal(item.module_name, 'braces');
    assert.equal(item.vulnerable_versions, '<=3.0.3');
    assert.equal(item.severity, 'high');
    assert.equal(item.cwe, 'CWE-674');
    assert.equal(item.patched_versions, null, 'published upstream fix must replace local mitigation');
    assert.equal(item.patched_versions_unpublished, true);
    assert(verified.length > 0 && verified.every(v => v.version === '3.0.3' && v.boundedDepth === 64 && v.upstreamFiles === 10));
    assert(item.findings?.length > 0);
    for (const finding of item.findings) {
      assert.equal(finding.version, '3.0.3');
      assert(finding.paths?.length > 0 && finding.paths.every(p => p.endsWith('>braces')));
    }
  }
  assert(entries.length <= 1, 'duplicate/unreviewed advisory');
  return { rawAdvisories: entries.length, sourceBoundMitigations: entries.length, unmitigated: 0 };
}

function main() {
  const result = spawnSync('pnpm', ['audit', '--json'], { encoding: 'utf8', maxBuffer: 16 * 1024 * 1024 });
  if (result.error) throw result.error;
  // Retain raw output even on a registry/network/parse error. No `|| true` gate.
  fs.mkdirSync('artifacts/security', { recursive: true });
  fs.writeFileSync('artifacts/security/pnpm-audit.json', result.stdout || '');
  if (result.stderr) process.stderr.write(result.stderr);
  const verified = installedBraces(process.cwd()).map(verifyInstalled);
  const report = assessAudit(JSON.parse(result.stdout), result.status, verified);
  fs.writeFileSync('artifacts/security/frontend-mitigation.json', JSON.stringify({ ...report, verified }, null, 2) + '\n');
  console.log(JSON.stringify(report));
}
if (require.main === module) main();
module.exports = { verifyInstalled, installedBraces, assessAudit };
