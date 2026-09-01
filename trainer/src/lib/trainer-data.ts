/**
 * 训练器规范数据(V1)的类型、运行时校验与加载。
 *
 * 唯一事实来源是 Rust 生成的 `xhup_flow_trainer.json`;前端不维护任何
 * 双拼映射、汉字编码、拼音转换或频率表。加载时完整校验一次,
 * 校验失败抛出带用户可读原因的 {@link TrainerDataError}。
 */

/** 一条训练条目:一个最终化的 `(汉字, 静态码)` 关系。 */
export type TrainerEntry = {
  char: string;
  code: string;
  length: 2 | 3 | 4;
  readings: string[];
  frequencyScore: number;
  rimeWeight: number;
};

/** 规范小鹤双拼键盘布局参考。 */
export type DoublePinyinReference = {
  initials: { initial: string; key: string }[];
  finals: { final: string; key: string }[];
  zeroInitials: { syllable: string; code: string }[];
};

/** 校验后的 V1 数据集。 */
export type TrainerDataset = {
  schemaVersion: 1;
  packageVersion: string;
  entries: TrainerEntry[];
  doublePinyin: DoublePinyinReference;
};

/** 训练数据契约错误:信息面向用户,不暴露内部细节。 */
export class TrainerDataError extends Error {
  constructor(message: string) {
    super(message);
    this.name = "TrainerDataError";
  }
}

/** 训练条目的稳定 ID:进度、错题、调度共用这一个函数。 */
export function itemId(entry: Pick<TrainerEntry, "char" | "code">): string {
  return `${entry.char}:${entry.code}`;
}

function fail(reason: string): never {
  throw new TrainerDataError(reason);
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function isLowerAlpha(value: unknown): value is string {
  return typeof value === "string" && /^[a-z]+$/.test(value);
}

function validateEntry(value: unknown, index: number): TrainerEntry {
  const at = `第 ${index + 1} 条训练数据`;
  if (!isRecord(value)) fail(`${at} 结构无效`);

  const { char, code, length, readings, frequencyScore, rimeWeight } = value;

  if (typeof char !== "string" || [...char].length !== 1) {
    fail(`${at} 的 char 应恰好为一个字符`);
  }
  if (typeof code !== "string" || !/^[a-z]{2,4}$/.test(code)) {
    fail(`${at} 的 code 应为 2-4 位小写字母`);
  }
  if (length !== 2 && length !== 3 && length !== 4) {
    fail(`${at} 的 length 应为 2/3/4`);
  }
  if (code.length !== length) {
    fail(`${at} 的 length 与 code 长度不一致`);
  }
  if (
    !Array.isArray(readings) ||
    readings.length === 0 ||
    !readings.every(isLowerAlpha) ||
    new Set(readings).size !== readings.length
  ) {
    fail(`${at} 的 readings 应为非空且不重复的规范读音`);
  }
  // Rust 端是 u64:必须显式拒绝超出 JS 安全整数的数据,不允许静默舍入。
  if (
    typeof frequencyScore !== "number" ||
    !Number.isSafeInteger(frequencyScore) ||
    frequencyScore < 0
  ) {
    fail(`${at} 的 frequencyScore 应为非负安全整数`);
  }
  if (
    typeof rimeWeight !== "number" ||
    !Number.isSafeInteger(rimeWeight) ||
    rimeWeight < 1
  ) {
    fail(`${at} 的 rimeWeight 应为正整数`);
  }

  return { char, code, length, readings, frequencyScore, rimeWeight };
}

function validateKeyMappings<T extends "initial" | "final">(
  rows: unknown[],
  field: T,
  label: string,
): ({ [K in T]: string } & { key: string })[] {
  return rows.map((row, index) => {
    const at = `双拼参考 ${label} 第 ${index + 1} 行`;
    if (!isRecord(row)) fail(`${at} 结构无效`);
    const name = row[field];
    if (!isLowerAlpha(name)) fail(`${at} 名称无效`);
    if (typeof row.key !== "string" || !/^[a-z]$/.test(row.key)) {
      fail(`${at} 键位无效`);
    }
    return { [field]: name, key: row.key } as { [K in T]: string } & {
      key: string;
    };
  });
}

function validateZeroInitials(
  rows: unknown[],
  label: string,
): { syllable: string; code: string }[] {
  return rows.map((row, index) => {
    const at = `双拼参考 ${label} 第 ${index + 1} 行`;
    if (!isRecord(row)) fail(`${at} 结构无效`);
    if (!isLowerAlpha(row.syllable)) fail(`${at} 音节无效`);
    if (typeof row.code !== "string" || !/^[a-z]{2}$/.test(row.code)) {
      fail(`${at} 编码无效`);
    }
    return { syllable: row.syllable, code: row.code };
  });
}

/** 完整校验 V1 数据集;失败抛出 {@link TrainerDataError}。 */
export function validateTrainerDataset(raw: unknown): TrainerDataset {
  if (!isRecord(raw)) fail("训练数据不是有效的 JSON 对象");
  if (raw.schemaVersion !== 1) {
    fail(
      `不支持的训练数据版本: ${String(raw.schemaVersion)}(需要 schemaVersion 1)`,
    );
  }
  if (
    typeof raw.packageVersion !== "string" ||
    raw.packageVersion.length === 0
  ) {
    fail("训练数据缺少 packageVersion");
  }
  if (!Array.isArray(raw.entries)) fail("训练数据缺少 entries 数组");

  const entries = raw.entries.map(validateEntry);

  const seen = new Set<string>();
  for (const entry of entries) {
    const id = itemId(entry);
    if (seen.has(id)) fail(`训练数据存在重复条目: ${id}`);
    seen.add(id);
  }

  if (!isRecord(raw.doublePinyin)) fail("训练数据缺少 doublePinyin 参考");
  const { initials, finals, zeroInitials } = raw.doublePinyin;
  if (!Array.isArray(initials) || initials.length !== 23) {
    fail("双拼参考 initials 应为 23 条");
  }
  if (!Array.isArray(finals) || finals.length !== 33) {
    fail("双拼参考 finals 应为 33 条");
  }
  if (!Array.isArray(zeroInitials) || zeroInitials.length !== 12) {
    fail("双拼参考 zeroInitials 应为 12 条");
  }

  return {
    schemaVersion: 1,
    packageVersion: raw.packageVersion,
    entries,
    doublePinyin: {
      initials: validateKeyMappings(initials, "initial", "initials"),
      finals: validateKeyMappings(finals, "final", "finals"),
      zeroInitials: validateZeroInitials(zeroInitials, "zeroInitials"),
    },
  };
}

/** 生成的规范数据在本地的静态路径(跟随 Vite base,兼容 Tauri WebView)。 */
export const TRAINER_DATA_URL = `${import.meta.env.BASE_URL}generated/xhup_flow_trainer.json`;

/** 加载并校验训练数据集;网络/解析/契约失败均抛出可读错误。 */
export async function loadTrainerDataset(): Promise<TrainerDataset> {
  let response: Response;
  try {
    response = await fetch(TRAINER_DATA_URL, { cache: "no-cache" });
  } catch {
    throw new TrainerDataError("无法读取训练数据文件");
  }
  if (!response.ok) {
    throw new TrainerDataError(`训练数据请求失败(HTTP ${response.status})`);
  }
  let raw: unknown;
  try {
    raw = await response.json();
  } catch {
    throw new TrainerDataError("训练数据不是合法的 JSON");
  }
  return validateTrainerDataset(raw);
}
