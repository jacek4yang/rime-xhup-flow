/**
 * 按本地日历日累计的练习统计(跨平台共享的进度 Schema 一部分)。
 *
 * 该类型原位于桌面训练器的 zustand store 中;提取到共享核心后,
 * 桌面端与小程序端使用同一份按日统计结构,备份 Schema 也因此跨平台一致。
 */

/** 按本地日历日累计的统计。 */
export type DailyStats = {
  practiceMs: number;
  questions: number;
  keystrokes: number;
  wrongKeyEvents: number;
  bestStreak: number;
  /** 完成的汉字数(V2;CPM 分母)。 */
  chars: number;
  /** 退格修正次数(V2)。 */
  corrections: number;
};

export function emptyDailyStats(): DailyStats {
  return {
    practiceMs: 0,
    questions: 0,
    keystrokes: 0,
    wrongKeyEvents: 0,
    bestStreak: 0,
    chars: 0,
    corrections: 0,
  };
}

/** 运行时窄化:未知/损坏字段整体回退为空统计(逐字段迁移由调用方负责)。 */
export function isDailyStats(value: unknown): value is DailyStats {
  if (typeof value !== "object" || value === null) return false;
  const v = value as Record<string, unknown>;
  const numericKeys: (keyof DailyStats)[] = [
    "practiceMs",
    "questions",
    "keystrokes",
    "wrongKeyEvents",
    "bestStreak",
    "chars",
    "corrections",
  ];
  return numericKeys.every((k) => typeof v[k] === "number" && Number.isFinite(v[k]));
}
