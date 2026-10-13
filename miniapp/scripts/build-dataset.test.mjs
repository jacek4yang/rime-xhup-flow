import assert from "node:assert/strict";
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "node:test";
import { brotliDecompressSync } from "node:zlib";
import { buildDataset, SOURCE } from "./build-dataset.mjs";

function fixture(t) {
  const directory = mkdtempSync(join(tmpdir(), "xhup-dataset-build-"));
  t.after(() => rmSync(directory, { recursive: true, force: true }));
  const source = join(directory, "canonical.json");
  const output = join(directory, "compiled");
  const canonical = JSON.parse(readFileSync(SOURCE, "utf8"));
  const data = {
    ...canonical,
    entries: canonical.entries.slice(0, 2),
    words: canonical.words.slice(0, 2),
    primaryShortcuts: canonical.primaryShortcuts.slice(0, 2),
    fixedFirstShortcuts: canonical.fixedFirstShortcuts.slice(0, 2),
  };
  const save = () => writeFileSync(source, JSON.stringify(data));
  save();
  return { directory, source, output, data, save };
}
const artifact = (output) => readFileSync(join(output, "trainer.json.br"));
const manifestPath = (output) => join(output, "dataset-manifest.json");

test("independent builds preserve every field, including supplementary Han and optional tone readings", (t) => {
  const f = fixture(t);
  f.data.entries[0].char = "𠀀";
  f.data.entries[0].toneReading = "lǜ";
  delete f.data.entries[1].toneReading;
  f.save();
  const other = join(f.directory, "independent");
  buildDataset(f);
  buildDataset({ source: f.source, output: other });
  assert.deepEqual(artifact(f.output), artifact(other));
  assert.deepEqual(readFileSync(manifestPath(f.output)), readFileSync(manifestPath(other)));
  const decoded = brotliDecompressSync(artifact(f.output));
  assert.ok(decoded.every((byte) => byte <= 127));
  assert.deepEqual(JSON.parse(decoded.toString("ascii")), f.data);
});

test("source changes reject stale artifacts without mutating them; rebuilding uses the changed source", (t) => {
  const f = fixture(t);
  buildDataset(f);
  const before = artifact(f.output);
  f.data.packageVersion = "2.1.0-rc.999";
  f.save();
  assert.throws(() => buildDataset({ ...f, check: true }), /Stale dataset/);
  assert.deepEqual(artifact(f.output), before);
  buildDataset(f);
  assert.equal(JSON.parse(brotliDecompressSync(artifact(f.output))).packageVersion, f.data.packageVersion);
});

test("corrupt and missing artifacts are rejected, not accepted as a valid cache", (t) => {
  const f = fixture(t);
  buildDataset(f);
  const original = artifact(f.output);
  const corrupt = Buffer.from(original);
  corrupt[0] ^= 255;
  writeFileSync(join(f.output, "trainer.json.br"), corrupt);
  assert.throws(() => buildDataset({ ...f, check: true }), /hash mismatch/);
  assert.deepEqual(artifact(f.output), corrupt);
  buildDataset(f);
  assert.deepEqual(artifact(f.output), original);
  rmSync(join(f.output, "trainer.json.br"));
  assert.throws(() => buildDataset({ ...f, check: true }), /ENOENT/);
  buildDataset(f);
  assert.deepEqual(artifact(f.output), original);
});

test("compiler identity and registered artifact paths cannot be substituted", (t) => {
  const f = fixture(t);
  buildDataset(f);
  const original = readFileSync(manifestPath(f.output), "utf8");
  const manifest = JSON.parse(original);
  manifest.compiler.sha256 = "0".repeat(64);
  writeFileSync(manifestPath(f.output), JSON.stringify(manifest));
  assert.throws(() => buildDataset({ ...f, check: true }), /Stale dataset/);
  const substituted = JSON.parse(original);
  substituted.artifact.file = "../elsewhere.br";
  writeFileSync(manifestPath(f.output), JSON.stringify(substituted));
  assert.throws(() => buildDataset({ ...f, check: true }), /Unregistered dataset path/);
});

test("unsupported, malformed and oversized input fails instead of falling back to old output", (t) => {
  const f = fixture(t);
  buildDataset(f);
  const original = artifact(f.output);
  f.data.schemaVersion = 99;
  f.save();
  assert.throws(() => buildDataset(f), /Unsupported canonical dataset schema/);
  writeFileSync(f.source, "{");
  assert.throws(() => buildDataset(f), SyntaxError);
  f.data.schemaVersion = 4;
  f.data.packageVersion = "x".repeat(16 * 1024 * 1024);
  f.save();
  assert.throws(() => buildDataset(f), /16 MiB budget/);
  assert.deepEqual(artifact(f.output), original);
});
