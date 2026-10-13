// Executes the compiled Taro/React application, not a replacement component.
// Explicit synthetic native host: NOT WeChat/DevTools/device acceptance.
const assert = require("node:assert/strict");
const fs = require("node:fs");
const os = require("node:os");
const path = require("node:path");
const { brotliDecompressSync } = require("node:zlib");

const dist = path.resolve(process.argv[2] || path.join(__dirname, "../dist"));
const retainEvidence = Boolean(process.argv[3]);
const output = retainEvidence
  ? path.resolve(process.argv[3])
  : fs.mkdtempSync(path.join(os.tmpdir(), "xhup-weapp-runtime-"));
if (retainEvidence) fs.mkdirSync(output);
const storage = new Map([["xhup.miniapp.state.v2", JSON.stringify({ version: 2, state: {
  progress: { "去:quts": { attempts: 1, correct: 0, wrong: 1, streak: 0,
    mastery: 0, lastSeenAt: 1, avgLatencyMs: 1000 } },
  daily: {}, keyErrors: {}, settings: { practiceIntent: "blind" },
} })]]);
const errors = [], patches = [], listeners = new Map();
let app, page, pendingRead;
process.on("uncaughtException", error => errors.push(String(error)));
process.on("unhandledRejection", error => errors.push(String(error)));
const windowInfo = {
  windowWidth: 390, windowHeight: 844, screenWidth: 390, screenHeight: 844,
  pixelRatio: 1, statusBarHeight: 20,
  safeArea: { left: 0, top: 20, right: 390, bottom: 824, width: 390, height: 804 },
};
global.wx = {
  getSystemInfoSync: () => ({ ...windowInfo, platform: "devtools",
    system: "synthetic Node host", SDKVersion: "3.10.0", version: "synthetic" }),
  getWindowInfo: () => windowInfo,
  getDeviceInfo: () => ({ platform: "devtools", system: "synthetic Node host" }),
  getAppBaseInfo: () => ({ SDKVersion: "3.10.0", version: "synthetic", theme: "light" }),
  getAccountInfoSync: () => ({ miniProgram: { appId: "synthetic-test" } }),
  canIUse: () => false,
  getStorageSync: key => storage.get(key) || "",
  setStorageSync: (key, value) => storage.set(key, value),
  removeStorageSync: key => storage.delete(key),
  // The native API returns void, even though the Taro declaration says Promise.
  getFileSystemManager: () => ({ readCompressedFile(options) { pendingRead = options; } }),
  onAppShow: callback => listeners.set("show", callback),
  onAppHide: callback => listeners.set("hide", callback),
  onWindowResize: callback => listeners.set("resize", callback),
  nextTick: callback => queueMicrotask(callback),
};
global.getApp = () => app;
global.getCurrentPages = () => page ? [page] : [];
global.App = config => { app = config; };
global.Page = config => {
  page = { ...config, route: "pages/session/index", setData(data, callback) {
    patches.push(data);
    if (callback) queueMicrotask(callback);
  } };
};
global.Component = config => config;
global.Behavior = config => config;
const delay = () => new Promise(resolve => setTimeout(resolve, 10));
async function until(predicate, label) {
  const deadline = Date.now() + 5000;
  while (!predicate() && Date.now() < deadline && errors.length === 0) await delay();
  assert.deepEqual(errors, [], label + " runtime errors");
  assert.ok(predicate(), label);
}

(async () => {
  // Module-boundary test: native globals must precede these generated imports.
  require(path.join(dist, "app.js"));
  assert.ok(app, "compiled App registered");
  app.onLaunch?.call(app, { path: "pages/session/index", query: {}, scene: 0 });
  global.__wxRoute = "pages/session/index";
  require(path.join(dist, "pages/session/index.js"));
  assert.ok(page, "compiled Page registered");
  page.onLoad?.call(page, { mode: "mixed", src: "weak" });
  await until(() => pendingRead !== undefined, "local dataset read requested");
  await until(() => patches.length > 0, "native page mounted before data arrived");
  fs.writeFileSync(path.join(output, "loading-patches.json"), JSON.stringify(patches, null, 2));
  const before = process.memoryUsage();
  const start = performance.now();
  const bytes = brotliDecompressSync(fs.readFileSync(path.join(dist, pendingRead.filePath)));
  pendingRead.success({ data: Uint8Array.from(bytes).buffer, errMsg: "readCompressedFile:ok" });
  await until(() => JSON.stringify(patches).includes("去"), "canonical review target rendered");
  assert.ok(!JSON.stringify(patches).includes("quts"), "blind practice must not serialize its answer");
  const report = {
    scope: "compiled Taro/React; synthetic native host; NOT device acceptance",
    elapsed_ms: performance.now() - start, before, after: process.memoryUsage(),
    patches: patches.length, errors,
  };
  fs.writeFileSync(path.join(output, "patches.json"), JSON.stringify(patches, null, 2));
  fs.writeFileSync(path.join(output, "result.json"), JSON.stringify(report, null, 2));
  page.onUnload?.call(page);
  await delay();
  assert.deepEqual(errors, [], "page teardown");
  console.log(JSON.stringify(report));
  if (!retainEvidence) fs.rmSync(output, { recursive: true });
  process.exit(0);
})().catch(error => {
  errors.push(String(error));
  fs.writeFileSync(path.join(output, "failure.json"), JSON.stringify({ errors, patches }, null, 2));
  console.error(error, "Evidence:", output);
  process.exit(1);
});
