/**
 * app.css 静态布局审计(无浏览器环境的守卫测试):
 *
 * - 键盘几何预算:按 app.css 实际解析出的键宽/键缝/行缩进推算三行
 *   总宽,必须装进会话页内容区(750rpx 屏宽 − 左右内边距);
 * - 安全区:键盘与会话页底部必须带 env(safe-area-inset-bottom);
 * - 降级方式:全表禁止 text-overflow: ellipsis(键帽参考等内容
 *   变化时按规则丢弃,绝不显示"...";教育内容不可不完整);
 * - 绝对宽度守卫:不允许任何规则设置超过守卫上限的 px 绝对宽度
 *   (rpx 随屏宽等比缩放不受限)。
 *
 * 解析是宽容的文本级正则;若未来 CSS 结构化重构导致解析失配,
 * 断言会直接失败并指出是哪条规则,属于期望内的守卫行为。
 */

import { readFileSync } from "node:fs";
import { join } from "node:path";
import { describe, expect, it } from "vitest";

// 测试总是从 miniapp 包根运行(pnpm --filter),不用 import.meta
// (Taro tsconfig 的 module 目标不允许)。
const css = readFileSync(join(process.cwd(), "src", "app.css"), "utf8");

/** 抓取某个选择器块内的声明(忽略嵌套与注释,宽容匹配)。 */
function declarationsOf(selector: string): string {
  const pattern = new RegExp(`(^|[\\s,.])${selector}\\s*\\{([^}]*)\\}`, "m");
  const match = pattern.exec(css);
  if (!match) throw new Error(`app.css 中找不到选择器 .${selector}`);
  return match[2];
}

function rpxValue(declarations: string, property: string): number {
  const match = new RegExp(`${property}\\s*:\\s*(-?[0-9.]+)rpx`).exec(declarations);
  if (!match) throw new Error(`找不到声明 ${property}(rpx)`);
  return Number(match[1]);
}

/** 解析 padding 简写的水平内边距(第二个值;top right bottom)。 */
function horizontalPaddingRpx(declarations: string): number {
  const match = /padding\s*:\s*([^;]+);/.exec(declarations);
  if (!match) throw new Error("找不到 padding 简写声明");
  const values = [...match[1].matchAll(/(-?[0-9.]+)rpx/g)].map((m) => Number(m[1]));
  if (values.length < 2) throw new Error("padding 简写少于两个值,无法取水平内边距");
  return values[1];
}

describe("app.css 静态布局审计", () => {
  it("教学键盘三行宽度都在会话页内容区(702rpx)之内", () => {
    const keyWidth = rpxValue(declarationsOf("kb-key"), "width");
    const gap = rpxValue(declarationsOf("kb-row"), "gap");
    const indent1 = rpxValue(declarationsOf("kb-row-1"), "padding-left");
    const indent2 = rpxValue(declarationsOf("kb-row-2"), "padding-left");

    // 内容区:屏宽 750rpx − 会话页左右内边距(从 .session-page 解析)。
    const pagePadding = horizontalPaddingRpx(declarationsOf("session-page"));
    const contentWidth = 750 - pagePadding * 2;

    const rows = [
      { keys: 10, indent: 0 },
      { keys: 9, indent: indent1 },
      { keys: 7, indent: indent2 },
    ];
    for (const [index, { keys, indent }] of rows.entries()) {
      const width = indent + keys * keyWidth + (keys - 1) * gap;
      expect(width, `键盘第 ${index} 行总宽 ${width}rpx 超出内容区`).toBeLessThanOrEqual(
        contentWidth,
      );
    }
    // 错位缩进应为整数个键距(键宽 + 缝),保持经典错位观感。
    const pitch = keyWidth + gap;
    expect(indent1 % pitch).toBe(0);
    expect(indent2 % pitch).toBe(0);
    expect(indent2).toBeGreaterThan(indent1);
  });

  it("键帽参考在键帽宽度内,超宽裁切且绝无省略号", () => {
    const keyWidth = rpxValue(declarationsOf("kb-key"), "width");
    const ref = declarationsOf("kb-ref");
    const refMax = rpxValue(ref, "max-width");
    expect(refMax).toBeLessThanOrEqual(keyWidth);
    expect(ref).toContain("overflow: hidden");
    expect(ref).toContain("white-space: nowrap");
    // 全表不允许出现省略号截断(教育内容降级 = 丢弃,不是 "...")。
    expect(css).not.toContain("text-overflow");
    expect(css).not.toContain("…");
  });

  it("键盘与会话页底部都有全面屏安全区内边距", () => {
    expect(declarationsOf("kb")).toContain("env(safe-area-inset-bottom)");
    expect(declarationsOf("session-page")).toContain("env(safe-area-inset-bottom)");
  });

  it("编码槽位容器允许换行(句子长码可达 40 槽)", () => {
    expect(declarationsOf("slots")).toContain("flex-wrap: wrap");
  });

  it("没有任何规则设置超过守卫上限的 px 绝对宽度", () => {
    // 守卫上限取桌面端窄窗 768px:小程序里出现更大的 px 固定宽度
    // 只可能是误植(rpx 是本项目的固定几何单位)。
    const GUARD_PX = 768;
    const offenders = [...css.matchAll(/width\s*:\s*([0-9.]+)px/g)]
      .map((match) => Number(match[1]))
      .filter((value) => value > GUARD_PX);
    expect(offenders).toEqual([]);
  });

  it("会话滚动容器存在且定高(短屏上统计芯片可滚动到视口内)", () => {
    const scroll = declarationsOf("session-scroll");
    expect(scroll).toContain("height: 100vh");
  });
});
