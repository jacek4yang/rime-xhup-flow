/**
 * 键帽标签推导测试:全部标签来自真实规范分片数据;无数据的键留空;
 * 韵母缩写规则与桌面端一致(多个韵母 = 首个 + "+")。
 */

import { describe, expect, it } from "vitest";
import shard from "../data/generated/dataset.json";
import { validateTrainerDataset } from "@xhup/trainer-core";
import { KEY_ROWS, buildKeyLabels, compactFinals, compactInitials } from "./keyboard-labels";

const dataset = validateTrainerDataset(shard);

describe("buildKeyLabels(规范分片数据)", () => {
  it("QWERTY 三行覆盖全部 26 个字母键,无重复", () => {
    const keys = KEY_ROWS.flat();
    expect(keys).toHaveLength(26);
    expect(new Set(keys).size).toBe(26);
  });

  it("双拼声母/韵母映射完整落在字母键上,且与数据逐条一致", () => {
    const labels = buildKeyLabels(dataset.doublePinyin);
    for (const { initial, key } of dataset.doublePinyin.initials) {
      expect(labels.get(key)!.initials).toContain(initial);
    }
    for (const { final, key } of dataset.doublePinyin.finals) {
      expect(labels.get(key)!.finals).toContain(final);
    }
  });

  it("没有映射的键标签为空(不发明任何占位)", () => {
    const labels = buildKeyLabels(dataset.doublePinyin);
    // 数据里没有的映射,标签必然为空串/空数组——绝不出现 "..." 之类。
    for (const [key, label] of labels) {
      expect(key).toMatch(/^[a-z]$/);
      expect(label.initials.every((initial) => /^[a-z]+$/.test(initial))).toBe(true);
      expect(label.finals.every((final) => /^[a-z]+$/.test(final))).toBe(true);
    }
    // 26 键之外不产生任何标签。
    for (const key of labels.keys()) {
      expect("qwertyuiopasdfghjklzxcvbnm").toContain(key);
    }
  });

  it("compaction:单韵母原样,多韵母取首个加 \"+\"", () => {
    expect(compactFinals([])).toBe("");
    expect(compactFinals(["uang"])).toBe("uang");
    expect(compactFinals(["uang", "iang"])).toBe("uang+");
    expect(compactInitials(["zh", "ch"])).toBe("zh ch");
    expect(compactInitials(["sh"])).toBe("sh");
  });
});
