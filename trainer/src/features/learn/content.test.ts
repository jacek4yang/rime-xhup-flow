/**
 * 学习中心测试:章节内容完整性守卫与形码聚合的纯函数行为。
 */

import { describe, expect, it } from "vitest";
import type { TrainerEntry } from "@/lib/trainer-data";
import { LEARN_CHAPTERS, validateChapters } from "./content";
import { buildShapeKeyStats, topShapeKeys } from "./shape-explorer";

describe("learn chapters", () => {
  it("章节结构完整:id 唯一、每章有正文、列表非空", () => {
    expect(validateChapters(LEARN_CHAPTERS)).toEqual([]);
  });

  it("覆盖从入门到精通的五个等级", () => {
    const levels = new Set(LEARN_CHAPTERS.map((chapter) => chapter.level));
    expect([...levels]).toEqual(["beginner", "basic", "intermediate", "advanced", "mastery"]);
  });

  it("练习 CTA 的标签都指向合法的模式键", () => {
    for (const chapter of LEARN_CHAPTERS) {
      for (const section of chapter.sections) {
        if (section.kind === "practice") {
          expect(section.label).toMatch(/^practice\.mode/);
        }
      }
    }
  });

  it("关键理论均有讲解:双拼/形码/记忆法/简码/组句", () => {
    const text = LEARN_CHAPTERS.map((chapter) =>
      chapter.sections
        .map((section) =>
          section.kind === "text"
            ? section.paragraphs.join("")
            : section.kind === "list"
              ? section.items.join("")
              : "",
        )
        .join(""),
    ).join("");
    for (const keyword of ["双拼", "形码", "零声母", "一级简码", "以字带根", "组句", "静态"]) {
      expect(text).toContain(keyword);
    }
  });
});

function entry(char: string, code: string, frequencyScore = 1): TrainerEntry {
  return {
    char,
    code,
    length: code.length as 2 | 3 | 4,
    readings: [],
    frequencyScore,
    rimeWeight: frequencyScore,
  };
}

describe("shape explorer aggregation", () => {
  it("按全码第 3、4 位聚合首形/次形,按频率取例字", () => {
    const stats = buildShapeKeyStats([
      entry("低", "dped", 300),
      entry("店", "dand", 200),
      entry("阿", "aaed", 100),
      entry("啊", "aakd", 50),
      // 非 4 码条目不参与形键聚合。
      entry("行", "xk", 999),
    ]);
    const e = stats.find((stat) => stat.key === "e");
    expect(e).toBeDefined();
    expect(e!.firstCount).toBe(2);
    expect(e!.firstSamples[0]).toEqual({ char: "低", code: "dped" });
    expect(e!.firstSamples.map((sample) => sample.char)).toEqual(["低", "阿"]);
    const d = stats.find((stat) => stat.key === "d");
    expect(d!.secondCount).toBe(4);
    expect(d!.secondSamples[0]?.char).toBe("低");
  });

  it("perKey 截断与 topShapeKeys 按组内规模排序", () => {
    const stats = buildShapeKeyStats(
      [entry("字一", "aaad", 500), entry("字二", "baad", 400), entry("阿", "aaed", 100)],
      1,
    );
    const a = stats.find((stat) => stat.key === "a");
    expect(a!.firstCount).toBe(2);
    expect(a!.firstSamples).toHaveLength(1);
    expect(a!.firstSamples[0]!.char).toBe("字一");
    // 次形 d 组有 3 个字,是信息量最大的键。
    const top = topShapeKeys(stats, 1);
    expect(top).toHaveLength(1);
    expect(top[0]!.key).toBe("d");
  });
});
