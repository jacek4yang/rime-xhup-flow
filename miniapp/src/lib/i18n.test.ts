/**
 * 小程序本地增量字典测试:zh/en 键对齐 + 翻译回落语义。
 */

import { describe, expect, it } from "vitest";
import { translateUi, uiDictionaryKeys } from "./ui-i18n";

describe("小程序增量 i18n", () => {
  it("zh 与 en 键集合完全对齐", () => {
    const { zh, en } = uiDictionaryKeys();
    expect(en).toEqual(zh);
  });

  it("先查增量表,再回落核心表", () => {
    expect(translateUi("zh", "stats.today")).toBe("今日概览");
    expect(translateUi("en", "stats.today")).toBe("Today at a glance");
    // 核心键照常工作。
    expect(translateUi("zh", "stats.title")).toBe("统计");
    expect(translateUi("en", "common.accuracy")).toBe("Accuracy");
  });

  it("插值占位与核心约定一致", () => {
    expect(translateUi("zh", "import.errorInvalid", { reason: "版本不符" })).toBe(
      "备份无效:版本不符",
    );
    expect(translateUi("en", "import.errorInvalid", { reason: "bad kind" })).toBe(
      "Invalid backup: bad kind",
    );
  });
});
