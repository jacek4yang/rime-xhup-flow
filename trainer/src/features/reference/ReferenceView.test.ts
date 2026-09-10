import { describe, expect, it } from "vitest";
import { buildTrainerIndex, type TrainerDataset, type TrainerEntry } from "@xhup/trainer-core";
import { buildReferenceResults } from "./ReferenceView";

function entry(
  char: string,
  code: string,
  options: Partial<TrainerEntry> = {},
): TrainerEntry {
  return {
    char,
    code,
    length: code.length as 2 | 3 | 4,
    readings: ["x"],
    frequencyScore: 10,
    rimeWeight: 1,
    scope: "core",
    codeSource: "canonical-reading-shape",
    sources: [],
    statuses: [],
    ...options,
  };
}

function dataset(): TrainerDataset {
  return {
    schemaVersion: 4,
    packageVersion: "test",
    entries: [
      entry("提", "ti", { readings: ["ti"], frequencyScore: 100 }),
      entry("示", "ui", { readings: ["shi"], frequencyScore: 100 }),
      entry("词", "ci", { readings: ["ci"], frequencyScore: 100 }),
      entry("嗯", "og", {
        readings: [],
        frequencyScore: 1,
        scope: "core",
        codeSource: "official-attested",
        sources: ["flypy-official-ix"],
        statuses: ["official"],
      }),
      entry("诶", "ei", {
        readings: [],
        frequencyScore: 1,
        scope: "extended",
        codeSource: "official-attested",
        sources: ["flypy-official-ix"],
        statuses: ["official-outside-core"],
      }),
    ],
    words: [],
    level1Shortcuts: [],
    primaryShortcuts: [],
    fixedFirstShortcuts: [],
    sentences: [],
    doublePinyin: { initials: [], finals: [], zeroInitials: [] },
  };
}

describe("reference reachability search", () => {
  it("显示事实字符编码及其来源层", () => {
    const index = buildTrainerIndex(dataset());
    expect(buildReferenceResults(index, "嗯")).toContainEqual(
      expect.objectContaining({ target: "嗯", code: "og", detail: expect.stringContaining("official") }),
    );
    expect(buildReferenceResults(index, "诶")).toContainEqual(
      expect.objectContaining({ target: "诶", code: "ei", detail: expect.stringContaining("official") }),
    );
  });

  it("对固定词表外文本逐字合成音码", () => {
    const index = buildTrainerIndex(dataset());
    expect(index.dataset.words.every((word) => word.word !== "提示词")).toBe(true);
    expect(buildReferenceResults(index, "提示词")).toContainEqual(
      expect.objectContaining({ target: "提示词", code: "tiuici", kind: "open" }),
    );
  });
});
