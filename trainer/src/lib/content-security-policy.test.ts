import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";

const config = JSON.parse(readFileSync("src-tauri/tauri.conf.json", "utf8"));
const capabilities = JSON.parse(readFileSync("src-tauri/capabilities/default.json", "utf8"));
function directives(policy: string): Map<string, string[]> {
  return new Map(policy.split(";").map((part) => {
    const [name, ...sources] = part.trim().split(/\s+/);
    return [name, sources];
  }));
}

describe("packaged Trainer CSP configuration (not a native WebView test)", () => {
  it("enforces local-only scripts, data and native IPC without dynamic code", () => {
    const policy = directives(config.app.security.csp);
    expect(policy.get("default-src")).toEqual(["'self'"]);
    expect(policy.get("script-src")).toEqual(["'self'"]);
    expect(policy.get("connect-src")).toEqual(["'self'", "ipc:", "http://ipc.localhost"]);
    expect(policy.get("font-src")).toEqual(["'self'"]);
    expect(policy.get("img-src")).toEqual(["'self'", "data:"]);
    expect(config.app.security.csp).not.toMatch(/unsafe-eval|https:|ws:|\*/);
    expect(config.app.security.dangerousDisableAssetCspModification).toBeUndefined();
  });

  it("blocks plugins, frames and external form navigation", () => {
    const policy = directives(config.app.security.csp);
    for (const name of ["object-src", "frame-src", "form-action"]) {
      expect(policy.get(name)).toEqual(["'none'"]);
    }
    expect(policy.get("base-uri")).toEqual(["'self'"]);
  });

  it("limits inline allowance to styles needed by Motion and React layouts", () => {
    const policy = directives(config.app.security.csp);
    expect(policy.get("style-src")).toEqual(["'self'", "'unsafe-inline'"]);
    for (const [name, sources] of policy) {
      if (name !== "style-src") expect(sources).not.toContain("'unsafe-inline'");
    }
  });

  it("isolates local development HMR and does not expand native privileges", () => {
    expect(directives(config.app.security.devCsp).get("connect-src")).toEqual([
      "'self'", "ipc:", "http://ipc.localhost", "ws://localhost:1420", "ws://localhost:1421",
    ]);
    expect(capabilities.permissions).toEqual(["core:default"]);
    expect(capabilities.remote).toBeUndefined();
  });
});
