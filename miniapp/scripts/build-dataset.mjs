/** Lossless offline dataset compiler. No sampling, runtime dependency or bytecode. */
import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { existsSync, mkdirSync, readFileSync, renameSync, rmSync, writeFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";
import { brotliCompressSync, brotliDecompressSync, constants } from "node:zlib";

const scriptPath = fileURLToPath(import.meta.url);
const root = resolve(dirname(scriptPath), "../..");
export const SOURCE = resolve(root, "trainer/public/generated/xhup_flow_trainer.json");
export const OUTPUT = resolve(root, "miniapp/src/data/generated");
const FILE = "trainer.json.br";
const MANIFEST = "dataset-manifest.json";
const MAX_COMPRESSED = 1024 * 1024;
const MAX_DECODED = 16 * 1024 * 1024;
const sha256 = (bytes) => createHash("sha256").update(bytes).digest("hex");

function inputs(source) {
  const bytes = readFileSync(source);
  const data = JSON.parse(bytes.toString("utf8"));
  if (data.schemaVersion !== 4) throw new Error("Unsupported canonical dataset schema");
  // JSON syntax is ASCII. Escape UTF-16 units, preserving supplementary pairs.
  // The runtime can decode without TextDecoder or another bundled dependency.
  const ascii = Buffer.from(bytes.toString("utf8").replace(/[^\x00-\x7f]/g,
    (character) => "\\u" + character.charCodeAt(0).toString(16).padStart(4, "0")), "ascii");
  assert.deepEqual(JSON.parse(ascii.toString("ascii")), data, "Lossy dataset conversion");
  if (ascii.length > MAX_DECODED) throw new Error("Decoded dataset exceeds 16 MiB budget");
  return { ascii, identity: {
    format: 1,
    encoding: "ascii-json-brotli",
    source: { sha256: sha256(bytes), schemaVersion: data.schemaVersion, packageVersion: data.packageVersion },
    compiler: { node: process.version, brotli: process.versions.brotli, sha256: sha256(readFileSync(scriptPath)) },
    counts: Object.fromEntries(Object.entries(data).filter(([, value]) => Array.isArray(value)).map(([key, value]) => [key, value.length])),
  } };
}

function verify(output, expected) {
  const manifest = JSON.parse(readFileSync(resolve(output, MANIFEST), "utf8"));
  const { artifact, ...identity } = manifest;
  assert.deepEqual(identity, expected.identity, "Stale dataset source or compiler identity");
  assert.equal(artifact.file, `data/${FILE}`, "Unregistered dataset path");
  const compressed = readFileSync(resolve(output, FILE));
  assert.equal(compressed.length, artifact.bytes, "Dataset size mismatch");
  assert.ok(compressed.length <= MAX_COMPRESSED, "Compressed dataset exceeds 1 MiB budget");
  assert.equal(sha256(compressed), artifact.sha256, "Dataset artifact hash mismatch");
  assert.equal(artifact.decodedBytes, expected.ascii.length, "Decoded size mismatch");
  assert.equal(artifact.decodedSha256, sha256(expected.ascii), "Decoded identity mismatch");
  assert.deepEqual(brotliDecompressSync(compressed, { maxOutputLength: MAX_DECODED }), expected.ascii,
    "Compiled dataset differs from canonical source");
  return manifest;
}

export function buildDataset({ source = SOURCE, output = OUTPUT, check = false, force = false } = {}) {
  const expected = inputs(source);
  if (check) return verify(output, expected);
  if (!force && existsSync(resolve(output, MANIFEST))) {
    try { return verify(output, expected); } catch { /* Rebuild; never use an invalid cache. */ }
  }
  const compressed = brotliCompressSync(expected.ascii, { params: {
    [constants.BROTLI_PARAM_QUALITY]: 11,
    [constants.BROTLI_PARAM_MODE]: constants.BROTLI_MODE_TEXT,
  } });
  assert.ok(compressed.length <= MAX_COMPRESSED, "Compressed dataset exceeds 1 MiB budget");
  assert.deepEqual(brotliDecompressSync(compressed), expected.ascii, "Brotli roundtrip failed");
  const manifest = { ...expected.identity, artifact: {
    file: `data/${FILE}`, bytes: compressed.length, sha256: sha256(compressed),
    decodedBytes: expected.ascii.length, decodedSha256: sha256(expected.ascii),
  } };
  mkdirSync(output, { recursive: true });
  for (const [name, bytes] of [[FILE, compressed], [MANIFEST, JSON.stringify(manifest, null, 2) + "\n"]]) {
    const path = resolve(output, name);
    writeFileSync(path + ".tmp", bytes);
    renameSync(path + ".tmp", path);
  }
  rmSync(resolve(output, "dataset.json"), { force: true }); // obsolete generated demonstration subset
  return verify(output, expected);
}

if (process.argv[1] && import.meta.url === pathToFileURL(resolve(process.argv[1])).href) {
  const args = process.argv.slice(2);
  if (args.some((arg) => !["--check", "--force"].includes(arg))) throw new Error("Unknown dataset build argument");
  const result = buildDataset({ check: args.includes("--check"), force: args.includes("--force") });
  console.log(`miniapp canonical dataset: ${result.artifact.bytes} compressed bytes; ${JSON.stringify(result.counts)}`);
}
