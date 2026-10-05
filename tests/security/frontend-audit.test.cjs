const { test } = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const { verifyInstalled, installedBraces, assessAudit } = require('./frontend-audit.cjs');
const verified = installedBraces(process.cwd()).map(verifyInstalled);
const advisory = {
  github_advisory_id: 'GHSA-vfj7-8cjw-p6xm', module_name: 'braces',
  vulnerable_versions: '<=3.0.3', severity: 'high', cwe: 'CWE-674',
  patched_versions: null, patched_versions_unpublished: true,
  findings: [{ version: '3.0.3', paths: ['miniapp>@tarojs/helper>chokidar>braces'] }],
};
const audit = () => ({ advisories: { one: structuredClone(advisory) }, metadata: { vulnerabilities: { high: 1, low: 0 } } });

test('raw finding stays visible, only exact source mitigation is recognized', () => {
  assert.deepEqual(assessAudit(audit(), 1, verified), { rawAdvisories: 1, sourceBoundMitigations: 1, unmitigated: 0 });
  assert.equal(assessAudit({ advisories: {}, metadata: { vulnerabilities: { high: 0 } } }, 0, verified).rawAdvisories, 0);
});

test('changed/new advisories, wrong statuses, missing proofs and audit errors fail closed', () => {
  for (const key of ['github_advisory_id', 'module_name', 'vulnerable_versions', 'severity', 'cwe', 'patched_versions']) {
    const input = audit(); input.advisories.one[key] = 'changed';
    assert.throws(() => assessAudit(input, 1, verified));
  }
  for (const code of [0, 2, null]) assert.throws(() => assessAudit(audit(), code, verified));
  assert.throws(() => assessAudit(audit(), 1, []));
  assert.throws(() => assessAudit({ error: { message: 'network error' } }, 1, verified));
  const duplicate = audit(); duplicate.advisories.two = structuredClone(advisory); duplicate.metadata.vulnerabilities.high = 2;
  assert.throws(() => assessAudit(duplicate, 1, verified));
  const badCount = audit(); badCount.metadata.vulnerabilities.high = 0;
  assert.throws(() => assessAudit(badCount, 1, verified));
  const badVersion = audit(); badVersion.advisories.one.findings[0].version = '3.0.2';
  assert.throws(() => assessAudit(badVersion, 1, verified));
});

test('missing guards, modified upstream, extra files and source symlinks fail', () => {
  const original = installedBraces(process.cwd())[0];
  const temp = fs.mkdtempSync(path.join(os.tmpdir(), 'xhup-braces-proof-'));
  try {
    const copy = path.join(temp, 'braces');
    for (const mutation of ['guard', 'upstream', 'extra', 'symlink']) {
      fs.cpSync(original, copy, { recursive: true });
      assert.equal(verifyInstalled(copy).upstreamFiles, 10);
      if (mutation === 'guard') {
        const file = path.join(copy, 'lib/compile.js');
        fs.writeFileSync(file, fs.readFileSync(file, 'utf8').replace('depth > 64', 'depth > 64000'));
      } else if (mutation === 'upstream') fs.appendFileSync(path.join(copy, 'index.js'), '\n// changed\n');
      else if (mutation === 'extra') fs.writeFileSync(path.join(copy, 'new.js'), 'extra');
      else fs.symlinkSync(path.join(copy, 'index.js'), path.join(copy, 'linked.js'));
      assert.throws(() => verifyInstalled(copy));
      fs.rmSync(copy, { recursive: true });
    }
  } finally { fs.rmSync(temp, { recursive: true, force: true }); }
});

test('http-cache-semantics does not serve authenticated/non-storable replies through max-stale', () => {
  const candidates = fs.readdirSync('node_modules/.pnpm').filter(p => p.startsWith('http-cache-semantics@4.3.0_patch_hash='));
  assert.equal(candidates.length, 1, 'one patched cache policy must be installed');
  const CachePolicy = require(path.resolve('node_modules/.pnpm', candidates[0], 'node_modules/http-cache-semantics'));
  const request = { url: 'https://example.invalid/private', method: 'GET', headers: { authorization: 'synthetic-user-a' } };
  for (const control of ['private, max-age=0', 'no-store', 'max-age=0']) {
    const policy = new CachePolicy(request, { status: 200, headers: { 'cache-control': control } }, { shared: true });
    const other = { ...request, headers: { authorization: 'synthetic-user-b', 'cache-control': 'max-stale' } };
    assert.equal(policy.storable(), false);
    assert.equal(policy.satisfiesWithoutRevalidation(other), false);
    assert.equal(policy.evaluateRequest(other).response, undefined);
  }
  const publicPolicy = new CachePolicy(request, { status: 200, headers: { 'cache-control': 'public, max-age=3600' } }, { shared: true });
  assert.equal(publicPolicy.storable(), true);
  assert.equal(publicPolicy.satisfiesWithoutRevalidation(request), true);
});
