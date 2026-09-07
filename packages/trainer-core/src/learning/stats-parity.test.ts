/**
 * 统计口径一致性测试(桌面端 / 小程序同源数值验证)。
 *
 * 给定一份固定序列化的用户状态(完整备份文档,经共享核心 importBackup
 * 校验后导入),断言两端统计页展示的每个数字:准确率 / KPM / CPM /
 * 键字比 / 退格修正 / 掌握度分桶 / 键位错误 Top 5 / 按日聚合。
 * 同一份 fixture + 同一套共享核心纯函数 ⇒ 任何平台算出完全相同的数值。
 *
 * 已知口径差异(如实记录,不在测试里掩盖):
 * - 掌握度分桶边界(0-19 / 20-39 / 40-59 / 60-79 / 80-100)内联在桌面端
 *   StatsView 组件里,不在共享核心中;本测试按同一公式复刻并断言。
 * - 桌面端 TrendBars 的按日 KPM/CPM 有 practiceMs < 1000 的额外保护;
 *   共享核心 kpm/cpm 只挡零值。低于 1 秒的练习日两端会不同(见下)。
 * - 连续练习天数(dayStreak)以「今天」为基准回溯,含真实时钟,不可
 *   确定性地断言;其输入只有 daily 记录本身,两端一致。
 */

import { describe, expect, it } from "vitest";
import {
  accuracy,
  cpm,
  formatDuration,
  formatPercent,
  kpm,
  localDateKey,
} from "./stats";
import { BACKUP_KIND, BACKUP_VERSION, importBackup } from "./backup";
import { emptyDailyStats } from "./daily-stats";

/** 固定 fixture:序列化的用户状态(与导出备份文档同构)。 */
const FIXTURE_JSON = JSON.stringify({
  kind: BACKUP_KIND,
  version: BACKUP_VERSION,
  createdAt: 1788710400000,
  settings: {
    theme: "light",
    hintMode: "on-error",
    difficulty: "daily",
    sessionLength: 30,
    lastMode: "double",
  },
  progress: {
    "char-2-a": { attempts: 12, correct: 10, wrong: 2, streak: 3, mastery: 5, lastSeenAt: 1788624000000, avgLatencyMs: 1400 },
    "char-2-b": { attempts: 8, correct: 6, wrong: 2, streak: 2, mastery: 20, lastSeenAt: 1788624000000, avgLatencyMs: 1600 },
    "char-3-c": { attempts: 20, correct: 18, wrong: 2, streak: 4, mastery: 45, lastSeenAt: 1788710400000, avgLatencyMs: 1200 },
    "char-4-d": { attempts: 30, correct: 28, wrong: 2, streak: 5, mastery: 79, lastSeenAt: 1788710400000, avgLatencyMs: 1100 },
    "char-4-e": { attempts: 44, correct: 42, wrong: 2, streak: 7, mastery: 80, lastSeenAt: 1788710400000, avgLatencyMs: 1000 },
    "char-4-f": { attempts: 60, correct: 60, wrong: 0, streak: 12, mastery: 100, lastSeenAt: 1788710400000, avgLatencyMs: 900 },
    // 未开始条目(attempts=0):导入校验边界会丢弃,任何端都不会统计。
    "char-3-empty": { attempts: 0, correct: 0, wrong: 0, streak: 0, mastery: 0, lastSeenAt: null, avgLatencyMs: null },
  },
  daily: {
    "2026-08-24": { practiceMs: 360000, questions: 15, keystrokes: 300, wrongKeyEvents: 15, bestStreak: 6, chars: 100, corrections: 5 },
    // 低于 1 秒的练习日:专门用于记录与桌面端按日 KPM/CPM 的口径差异。
    "2026-08-25": { practiceMs: 800, questions: 2, keystrokes: 8, wrongKeyEvents: 0, bestStreak: 2, chars: 2, corrections: 0 },
    "2026-09-05": { practiceMs: 720000, questions: 30, keystrokes: 720, wrongKeyEvents: 36, bestStreak: 9, chars: 240, corrections: 12 },
    "2026-09-06": { practiceMs: 1200000, questions: 40, keystrokes: 960, wrongKeyEvents: 48, bestStreak: 12, chars: 320, corrections: 18 },
  },
  keyErrors: { q: 9, j: 7, k: 5, a: 3, i: 2, u: 1 },
  lessonEvidence: {},
  confusions: {},
});

/** 经共享核心校验边界导入后的状态(与桌面/小程序 store 持久化同构)。 */
const imported = importBackup(FIXTURE_JSON);

describe("统计口径一致性:序列化 fixture → 共享核心 → 展示数值", () => {
  it("fixture 是当前版本的合法备份文档", () => {
    expect(Object.keys(imported.progress)).toHaveLength(6);
    expect(Object.keys(imported.daily)).toHaveLength(4);
  });

  it("今日键来自本地日历日(与桌面端 localDateKey 同源)", () => {
    expect(localDateKey(new Date(2026, 8, 6))).toBe("2026-09-06");
  });

  it("今日指标:准确率 95% / KPM 48 / CPM 16 / 键字比 3.00 / 修正 18", () => {
    const today = imported.daily["2026-09-06"];

    // 准确率:共享核心 accuracy(键级)。
    expect(accuracy(today.keystrokes, today.wrongKeyEvents)).toBeCloseTo(0.95);
    expect(formatPercent(accuracy(today.keystrokes, today.wrongKeyEvents))).toBe("95%");

    // KPM:共享核心 kpm(练习时长 20 分钟 → 960 / 20)。
    expect(kpm(today.keystrokes, today.practiceMs)).toBeCloseTo(48);

    // CPM:共享核心 cpm(320 字 / 20 分钟)。
    expect(cpm(today.chars, today.practiceMs)).toBeCloseTo(16);

    // 键字比 = keystrokes / chars(两端各自内联,分母为零必须回 null)。
    const keysPerChar = today.chars > 0 ? today.keystrokes / today.chars : null;
    expect(keysPerChar).toBeCloseTo(3);
    expect(keysPerChar === null ? "—" : keysPerChar.toFixed(2)).toBe("3.00");

    // 其余直接读按日统计字段。
    expect(today.questions).toBe(40);
    expect(today.keystrokes).toBe(960);
    expect(today.corrections).toBe(18);
    expect(formatDuration(today.practiceMs)).toBe("20 分钟");
  });

  it("零输入日如实显示空值,不产生假数据", () => {
    const empty = emptyDailyStats();
    expect(accuracy(empty.keystrokes, empty.wrongKeyEvents)).toBeNull();
    expect(formatPercent(null)).toBe("—");
    expect(kpm(empty.keystrokes, empty.practiceMs)).toBeNull();
    expect(cpm(empty.chars, empty.practiceMs)).toBeNull();
  });

  it("按日聚合:指定日期的完整 DailyStats 逐字段一致", () => {
    expect(imported.daily["2026-09-05"]).toEqual({
      practiceMs: 720000,
      questions: 30,
      keystrokes: 720,
      wrongKeyEvents: 36,
      bestStreak: 9,
      chars: 240,
      corrections: 12,
    });
  });

  it("近 14 天趋势条:当日分钟数 = round(practiceMs / 60000)", () => {
    // 桌面端 TrendBars 的 valueOf 内联了同一取整,两端条形等比一致。
    expect(Math.round(imported.daily["2026-09-05"].practiceMs / 60000)).toBe(12);
    expect(Math.round(imported.daily["2026-08-25"].practiceMs / 60000)).toBe(0);
  });

  it("掌握度分桶:0-19/20-39/40-59/60-79/80-100 → [1,1,1,1,2]", () => {
    // 桌面端 StatsView 内联公式:attempts === 0 跳过;
    // bucket = min(4, floor(mastery / 20))。核心暂未承载该边界,这里复刻。
    const buckets = [0, 0, 0, 0, 0];
    for (const itemProgress of Object.values(imported.progress)) {
      if (itemProgress.attempts === 0) continue;
      buckets[Math.min(4, Math.floor(itemProgress.mastery / 20))] += 1;
    }
    expect(buckets).toEqual([1, 1, 1, 1, 2]);
    expect(buckets.reduce((sum, count) => sum + count, 0)).toBe(6);
  });

  it("错误维度:keyErrors 按次数降序取前 5", () => {
    const top5 = Object.entries(imported.keyErrors)
      .sort(([, a], [, b]) => b - a)
      .slice(0, 5);
    expect(top5).toEqual([
      ["q", 9],
      ["j", 7],
      ["k", 5],
      ["a", 3],
      ["i", 2],
    ]);
  });

  it("已知口径差异:practiceMs < 1000 的练习日,桌面端按日 KPM/CPM 显示为空而共享核心有值", () => {
    const shortDay = imported.daily["2026-08-25"];
    // 桌面端 StatsView:last14Days 对 practiceMs < 1000 直接置 null。
    expect(shortDay.practiceMs).toBeLessThan(1000);
    // 共享核心 kpm 只挡零输入/零时长,因此这里会算出数值。
    // 小程序统计页如采用共享核心口径,短练习日会显示数值而非空;
    // 该差异已在此如实记录,不影响长练习日(≥1s)的完全一致。
    expect(kpm(shortDay.keystrokes, shortDay.practiceMs)).toBeCloseTo(600, 6);
    // CPM 两端口径恰好一致:核心与桌面端都在 <1s 时返回 null。
    expect(cpm(shortDay.chars, shortDay.practiceMs)).toBeNull();
  });
});
