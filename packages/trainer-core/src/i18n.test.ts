import { describe, expect, it } from "vitest";
import { dictionaryKeys, LANGUAGES, translate, type Language } from "./i18n";

describe("i18n 字典", () => {
  it("中英字典键完全对齐(zh 为基准)", () => {
    const zhKeys = dictionaryKeys("zh");
    const enKeys = dictionaryKeys("en");
    expect(enKeys).toEqual(zhKeys);
    expect(zhKeys.size).toBeGreaterThan(40);
  });

  it("translate 支持插值", () => {
    expect(translate("zh", "common.wrongCount", { n: 3 })).toBe("错 3 次");
    expect(translate("en", "common.wrongCount", { n: 3 })).toBe("3 wrong");
  });

  it("两种语言都有全部键(遍历抽查)", () => {
    for (const language of LANGUAGES as readonly Language[]) {
      expect(translate(language, "nav.today").length).toBeGreaterThan(0);
      expect(translate(language, "practice.group.sentences").length).toBeGreaterThan(0);
      expect(translate(language, "settings.backup").length).toBeGreaterThan(0);
    }
  });
});
