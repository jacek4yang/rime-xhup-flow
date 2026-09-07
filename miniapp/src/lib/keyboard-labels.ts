/**
 * 键帽标签推导(纯逻辑):从规范数据的双拼参考与形码聚合构建
 * 键 → 参考标签。与桌面端 OnScreenKeyboard 的 buildKeyLabels 同一套
 * 规则:只读真实数据,没有映射就留空,绝不发明"..."占位。
 */

import type { DoublePinyinReference } from "@xhup/trainer-core";

/** 单键的双拼参考标签。 */
export type KeyLabels = {
  initials: string[];
  finals: string[];
};

/**
 * 从双拼参考构建 键 → {声母, 韵母} 标签;不做任何硬编码映射。
 * 键顺序无关;标签按数据出现顺序排列。
 */
export function buildKeyLabels(
  reference: DoublePinyinReference,
): Map<string, KeyLabels> {
  const map = new Map<string, KeyLabels>();
  const ensure = (key: string): KeyLabels => {
    const existing = map.get(key);
    if (existing) return existing;
    const created: KeyLabels = { initials: [], finals: [] };
    map.set(key, created);
    return created;
  };
  for (const { initial, key } of reference.initials) {
    ensure(key).initials.push(initial);
  }
  for (const { final, key } of reference.finals) {
    ensure(key).finals.push(final);
  }
  return map;
}

/**
 * 键帽缩写:韵母多于一个时只显示第一个 + "+",完整对照在键详情页。
 * 规则化缩写,绝不使用省略号截断(教育内容不可不完整)。
 */
export function compactFinals(finals: readonly string[]): string {
  if (finals.length === 0) return "";
  return finals.length > 1 ? `${finals[0]}+` : finals[0];
}

/** 声母标签:多个声母(如 zh/ch/sh 归并键)用空格并列。 */
export function compactInitials(initials: readonly string[]): string {
  return initials.join(" ");
}

/**
 * QWERTY 三行(经典错位排布;键序为固定几何,与数据无关)。
 */
export const KEY_ROWS: readonly string[][] = [
  [..."qwertyuiop"],
  [..."asdfghjkl"],
  [..."zxcvbnm"],
];
