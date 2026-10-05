// GHSA-vfj7-8cjw-p6xm: installed transitive package, no replacement mock.
const { test } = require('node:test');
const assert = require('node:assert/strict');
const { createRequire } = require('node:module');
const { resolve } = require('node:path');
const mini = createRequire(resolve('miniapp/package.json'));
const helper = createRequire(mini.resolve('@tarojs/helper/package.json'));
const chokidar = createRequire(helper.resolve('chokidar/package.json'));
const braces = chokidar('braces');
const guarded = error => error instanceof RangeError &&
  error.message === 'braces nesting exceeds safety limit (64)';

for (const method of ['compile', 'expand', 'stringify']) {
  test(`${method}: deeply nested patterns fail with a bounded diagnostic`, () => {
    for (const depth of [65, 128, 4096]) {
      const input = '{'.repeat(depth) + 'x,y' + '}'.repeat(depth);
      assert.throws(() => braces[method](braces.parse(input)), guarded);
    }
  });
  test(`${method}: caller-supplied deep and cyclic ASTs are guarded`, () => {
    let ast = { type: 'text', value: 'x' };
    for (let i = 0; i < 1000; i++) ast = { type: 'root', nodes: [ast] };
    assert.throws(() => braces[method](ast), guarded);
    const cycle = { type: 'root', nodes: [] };
    cycle.nodes.push(cycle);
    assert.throws(() => braces[method](cycle), guarded);
  });
}

test('normal globs, ranges, escapes, nested alternatives and invalid literals are preserved', () => {
  assert.deepEqual(braces.expand('a{b,c}d'), ['abd', 'acd']);
  assert.equal(braces.compile('a{b,c}d'), 'a(b|c)d');
  assert.deepEqual(braces.expand('{1..3}'), ['1', '2', '3']);
  assert.deepEqual(braces.expand('a{b,{c,d}}'), ['ab', 'ac', 'ad']);
  assert.equal(braces.stringify(braces.parse('src/**/{a,b}.js')), 'src/**/{a,b}.js');
  assert.deepEqual(braces.expand('a{b'), ['a{b']);
  assert.deepEqual(braces.expand('a\\{b,c\\}'), ['a{b,c}']);
  const safe = '{'.repeat(20) + 'x' + '}'.repeat(20);
  assert.equal(braces.stringify(braces.parse(safe)), safe);
});
