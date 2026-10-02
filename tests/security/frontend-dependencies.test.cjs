// Source-bound integration checks for the two pnpm dependency adapters.
// No public network or real user files; fixtures use a temporary repository/server.
const { test } = require("node:test");
const assert = require("node:assert/strict");
const { createRequire } = require("node:module");
const { mkdtempSync, writeFileSync, readFileSync, existsSync, rmSync } = require("node:fs");
const { tmpdir } = require("node:os");
const { join, resolve } = require("node:path");
const { execFileSync } = require("node:child_process");
const http = require("node:http");
const mini = createRequire(resolve("miniapp/package.json"));
const cli = createRequire(mini.resolve("@tarojs/cli/package.json"));
const download = cli("download-git-repo");
const runner = createRequire(mini.resolve("@tarojs/webpack5-runner/package.json"));
const Plugin = runner("./dist/plugins/MiniPlugin.js").default;
const fetchRepo = (source, target, options = {}) => new Promise((ok, fail) =>
  download(source, target, options, error => error ? fail(error) : ok()));

test("clone preserves checkout and refuses injected options before execution", async () => {
  const root = mkdtempSync(join(tmpdir(), "xhup-clone-"));
  try {
    const source = join(root, "source");
    execFileSync("git", ["init", "-b", "master", source], { stdio: "ignore" });
    writeFileSync(join(source, "fixture.txt"), "public test fixture");
    execFileSync("git", ["-C", source, "add", "."]);
    execFileSync("git", ["-C", source, "-c", "user.name=Test", "-c",
      "user.email=test@example.invalid", "commit", "-m", "fixture"], { stdio: "ignore" });
    const target = join(root, "target");
    await fetchRepo("direct:" + source + "#master", target, { clone: true });
    assert.equal(readFileSync(join(target, "fixture.txt"), "utf8"), "public test fixture");
    assert.equal(existsSync(join(target, ".git")), false);
    for (const ref of ["--upload-pack=evil", "-c", "master\n--config=evil"]) {
      const rejected = join(root, "rejected");
      await assert.rejects(fetchRepo("direct:" + source + "#" + ref, rejected,
        { clone: true }));
      assert.equal(existsSync(rejected), false);
    }
    await assert.rejects(fetchRepo("direct:" + source, join(root, "custom"),
      { clone: true, git: "/untrusted/executable" }), /Unsafe/);
    await assert.rejects(fetchRepo("not-a-repository", join(root, "invalid")));
  } finally { rmSync(root, { recursive: true, force: true }); }
});

test("maintained archive downloader extracts locally and rejects traversal", async () => {
  const root = mkdtempSync(join(tmpdir(), "xhup-archive-"));
  const good = readFileSync(join(__dirname, "fixtures", "template.zip"));
  const bad = readFileSync(join(__dirname, "fixtures", "traversal.zip"));
  assert.ok(bad.includes(Buffer.from("../outside.txt")), "fixture contains literal traversal");
  const server = http.createServer((req, res) => {
    res.setHeader("Content-Type", "application/zip");
    res.end(req.url === "/good.zip" ? good : bad);
  });
  await new Promise(ok => server.listen(0, "127.0.0.1", ok));
  try {
    const base = "direct:http://127.0.0.1:" + server.address().port;
    await fetchRepo(base + "/good.zip", join(root, "good"));
    assert.equal(readFileSync(join(root, "good", "README.txt"), "utf8"), "public template");
    await assert.rejects(fetchRepo(base + "/bad.zip", join(root, "bad"), { strip: 0 }));
    assert.equal(existsSync(join(root, "outside.txt")), false);
  } finally {
    await new Promise(ok => server.close(ok));
    rmSync(root, { recursive: true, force: true });
  }
});

function pluginFixture(template) {
  const plugin = Object.create(Plugin.prototype);
  plugin.options = {
    template: {
      isSupportRecursive: true,
      buildTemplate: () => template,
      buildCustomComponentTemplate: () => "<view> wrapper </view>",
    },
    sourceDir: "/unused", fileType: { templ: ".wxml" },
    combination: { config: { minifyXML: { collapseWhitespace: true } } },
  };
  plugin.independentPackages = new Map([["sub", { pages: [] }]]);
  plugin.pages = new Set();
  plugin.generateConfigFile = () => {};
  plugin.generateXSFile = () => {};
  plugin.getComponentName = value => value;
  plugin.getTemplatePath = value => value + ".wxml";
  return plugin;
}
const compiler = { webpack: { sources: { RawSource: class {
  constructor(text) { assert.equal(typeof text, "string"); this.text = text; }
  source() { return this.text; }
} } } };

test("independent-package hook awaits minification and isolates compilations", async () => {
  const plugin = pluginFixture("<view>   public test   </view>");
  const a = { __name: "sub", assets: {} }, b = { __name: "sub", assets: {} };
  await Promise.all([plugin.generateIndependentMiniFiles(a, compiler),
    plugin.generateIndependentMiniFiles(b, compiler)]);
  for (const compilation of [a, b]) {
    assert.equal(compilation.assets["sub/base.wxml"].source(), "<view>public test</view>");
    assert.ok(compilation.assets["sub/custom-wrapper.wxml"]);
  }
});

test("minifier failure rejects the build hook rather than emitting incomplete assets", async () => {
  const plugin = pluginFixture("<view broken=\"");
  await assert.rejects(plugin.generateIndependentMiniFiles({ __name: "sub", assets: {} }, compiler));
});
