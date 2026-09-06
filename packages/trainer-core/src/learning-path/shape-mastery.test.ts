import { describe, expect, it } from "vitest";
import {
  analyzeShapeMastery,
  expectedShapeKeys,
} from "./shape-mastery";
import { emptyProgress, type ItemProgress } from "../learning/progress";
import { makeIndex } from "../testing/fixtures";

function progress(overrides: Partial<ItemProgress> = {}): ItemProgress {
  return { ...emptyProgress(), ...overrides };
}

describe("expectedShapeKeys", () => {
  it("全码第 3、4 位为形键;3 码只有首形;2 码无形键", () => {
    expect(expectedShapeKeys("aaed")).toEqual({ shape1: "e", shape2: "d" });
    expect(expectedShapeKeys("xkg")).toEqual({ shape1: "g", shape2: null });
    expect(expectedShapeKeys("xk")).toEqual({ shape1: null, shape2: null });
  });
});

describe("analyzeShapeMastery", () => {
  it("shapeAccuracy 是条目级代理:无错完成 / 见过条目", () => {
    const index = makeIndex();
    // fixture:好:hkn(3 码,首形 n)、好:hknc(4 码,首形 n 次形 c)、
    // 我:wop(3 码,首形 p)、我:wopd(4 码,首形 p 次形 d)。
    const report = analyzeShapeMastery({
      index,
      progressById: {
        "好:hkn": progress({ attempts: 2, wrong: 0 }),
        "好:hknc": progress({ attempts: 3, wrong: 1 }),
        "我:wop": progress({ attempts: 4, wrong: 4 }),
        "我:wopd": progress({ attempts: 1, wrong: 0 }),
      },
      keyErrors: {},
    });
    // shape1:见过 n/p 各 2 条,全错完成 n 1/2、p 1/2 → 0.5。
    expect(report.shapeAccuracy.shape1).toBeCloseTo(0.5);
    // shape2:见过 c/d 各 1 条,全错完成 1/2 → 0.5。
    expect(report.shapeAccuracy.shape2).toBeCloseTo(0.5);
  });

  it("无样本时 shapeAccuracy 为 null(不显示假数据)", () => {
    const index = makeIndex();
    const report = analyzeShapeMastery({ index, progressById: {}, keyErrors: {} });
    expect(report.shapeAccuracy.shape1).toBeNull();
    expect(report.shapeAccuracy.shape2).toBeNull();
    expect(report.weakShapeKeys).toEqual([]);
  });

  it("薄弱形键:期望形键关联的已练条目错误率排序", () => {
    const index = makeIndex();
    const report = analyzeShapeMastery({
      index,
      progressById: {
        // 首形 n:两条都出错 → wrongRate 1。
        "好:hkn": progress({ attempts: 2, wrong: 2 }),
        "好:hknc": progress({ attempts: 2, wrong: 2 }),
        // 首形 p:两条都干净(但 keyErrors 有记录)→ wrongRate 0。
        "我:wop": progress({ attempts: 2, wrong: 0 }),
        "我:wopd": progress({ attempts: 2, wrong: 0 }),
      },
      keyErrors: { n: 5, p: 1 },
    });
    expect(report.weakShapeKeys[0]).toMatchObject({
      key: "n",
      role: "shape1",
      exposure: 2,
      wrongItems: 2,
      wrongRate: 1,
    });
    const pKey = report.weakShapeKeys.find((entry) => entry.key === "p");
    expect(pKey).toMatchObject({ keyErrors: 1, wrongRate: 0 });
  });

  it("confusedKeys 只到键级:列出误按键并标注是否形键(不臆测配对)", () => {
    const index = makeIndex();
    const report = analyzeShapeMastery({
      index,
      progressById: {
        "好:hkn": progress({ attempts: 2, wrong: 2 }),
      },
      keyErrors: { n: 7, q: 3, Z: 2, b: 0 },
    });
    // Z 非小写、b 为 0,均被过滤;按次数降序。
    expect(report.confusedKeys).toEqual([
      { actual: "n", count: 7, isShapeKey: true },
      { actual: "q", count: 3, isShapeKey: false },
    ]);
  });

  it("limit 截断列表", () => {
    const index = makeIndex();
    const report = analyzeShapeMastery({
      index,
      progressById: { "好:hkn": progress({ attempts: 1, wrong: 1 }) },
      keyErrors: { n: 2, q: 2, w: 2, e: 2 },
      limit: 1,
    });
    expect(report.confusedKeys).toHaveLength(1);
  });
});
