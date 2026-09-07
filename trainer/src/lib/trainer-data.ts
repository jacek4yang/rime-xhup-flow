/**
 * 训练器规范数据(V2)的类型、运行时校验与加载。
 *
 * 唯一事实来源是 Rust 生成的 `xhup_flow_trainer.json`;前端不维护任何
 * 双拼映射、汉字编码、词码、简码策略或频率表。加载时完整校验一次,
 * 校验失败抛出带用户可读原因的 {@link TrainerDataError}。
 *
 * V2 相对 V1 新增:`words`(固定词全码,按权重截断)、
 * `level1Shortcuts`、三个生产简码层(`wordShortcuts` = ZERO_REGRESSION、
 * `fixedFirstShortcuts`、`twoKeyShortcuts`)与 `sentences`(组件语义
 * 列表,输入码由 Rust 从 canonical 全码机械拼接)。单字段 `entries`
 * (2/3/4 码单字)与 V1 兼容。
 */

import { translate, type I18nKey } from "@/lib/i18n";
import { useTrainerStore } from "@/stores/trainer-store";

/** 加载期错误文案语言:数据加载先于 React 上下文,直接读 store。 */
function t(key: I18nKey, params?: Record<string, string | number>): string {
  return translate(useTrainerStore.getState().language, key, params);
}

/** 一条单字训练条目:一个最终化的 `(汉字, 静态码)` 关系。 */
export type TrainerEntry = {
  char: string;
  code: string;
  length: 2 | 3 | 4;
  readings: string[];
  /** 规范默认读音(带调;教育元数据,缺失时回退无调展示)。 */
  toneReading?: string;
  frequencyScore: number;
  rimeWeight: number;
};

/** 一条固定词训练条目(全码,逐字双拼拼接)。 */
export type TrainerWord = {
  word: string;
  code: string;
  /** 全码键数(4/6/8)。 */
  length: number;
  /** 汉字数。 */
  charCount: number;
  rimeWeight: number;
};

/** 一条一级简码关系。 */
export type TrainerLevel1Shortcut = {
  key: string;
  char: string;
};

/** 生产简码层身份(与 analyzer 的 ShortcutPolicyId 对应)。 */
export type TrainerShortcutLayer =
  | "zero-regression"
  | "fixed-first"
  | "two-key-zero-regression";

/** 一条词语简码关系(shortcut 与 fullCode 都保留可用)。 */
export type TrainerShortcut = {
  word: string;
  fullCode: string;
  shortcutCode: string;
  /** F/I 投影模式(如 `FI` / `II`)。 */
  mode: string;
};

/** 一条组句练习 fixture(码由 Rust 机械拼接,前端不推导)。 */
export type TrainerSentence = {
  text: string;
  code: string;
  components: string[];
};

/** 规范小鹤双拼键盘布局参考。 */
export type DoublePinyinReference = {
  initials: { initial: string; key: string }[];
  finals: { final: string; key: string }[];
  zeroInitials: { syllable: string; code: string }[];
};

/** 校验后的 V2 数据集。 */
export type TrainerDataset = {
  schemaVersion: 2;
  packageVersion: string;
  entries: TrainerEntry[];
  words: TrainerWord[];
  level1Shortcuts: TrainerLevel1Shortcut[];
  wordShortcuts: TrainerShortcut[];
  fixedFirstShortcuts: TrainerShortcut[];
  twoKeyShortcuts: TrainerShortcut[];
  sentences: TrainerSentence[];
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

/** 固定词条目的稳定 ID。 */
export function wordId(word: Pick<TrainerWord, "word" | "code">): string {
  return `word:${word.word}:${word.code}`;
}

/** 简码条目的稳定 ID(以主练码 = 简码为准)。 */
export function shortcutId(shortcut: Pick<TrainerShortcut, "word" | "shortcutCode">): string {
  return `shortcut:${shortcut.word}:${shortcut.shortcutCode}`;
}

/** 组句条目的稳定 ID。 */
export function sentenceId(sentence: Pick<TrainerSentence, "text">): string {
  return `sentence:${sentence.text}`;
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

function isHanziString(value: unknown, at: string): string {
  if (typeof value !== "string" || !/^[\u4e00-\u9fff]+$/.test(value)) {
    fail(`${at} 应为汉字串`);
  }
  return value;
}

function validateEntry(value: unknown, index: number): TrainerEntry {
  const at = `第 ${index + 1} 条训练数据`;
  if (!isRecord(value)) fail(`${at} 结构无效`);

  const { char, code, length, readings, toneReading, frequencyScore, rimeWeight } = value;

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
    readings.some((reading) => reading.length < 1 || reading.length > 6)
  ) {
    fail(`${at} 的 readings 应为非空小写字母数组`);
  }
  if (
    typeof frequencyScore !== "number" ||
    !Number.isSafeInteger(frequencyScore) ||
    frequencyScore < 0
  ) {
    fail(`${at} 的 frequencyScore 应为非负安全整数`);
  }
  if (typeof rimeWeight !== "number" || !Number.isInteger(rimeWeight) || rimeWeight <= 0) {
    fail(`${at} 的 rimeWeight 应为正整数`);
  }
  if (new Set(readings).size !== readings.length) {
    fail(`${at} 的 readings 存在重复读音`);
  }
  if (
    toneReading !== undefined &&
    (typeof toneReading !== "string" || toneReading.length < 1 || toneReading.length > 7)
  ) {
    fail(`${at} 的 toneReading 应为 1-7 字符的带调拼音`);
  }
  return {
    char,
    code,
    length,
    readings,
    ...(toneReading === undefined ? {} : { toneReading }),
    frequencyScore,
    rimeWeight,
  };
}

function validateWord(value: unknown, index: number): TrainerWord {
  const at = `第 ${index + 1} 条词条数据`;
  if (!isRecord(value)) fail(`${at} 结构无效`);
  const { word, code, length, charCount, rimeWeight } = value;
  const text = isHanziString(word, `${at} 的 word`);
  const charCountValue = text.length;
  if (charCountValue < 2 || charCountValue > 4) {
    fail(`${at} 的 word 应为 2-4 字词`);
  }
  if (typeof code !== "string" || !/^[a-z]{4,8}$/.test(code)) {
    fail(`${at} 的 code 应为 4-8 位小写字母`);
  }
  if (code.length !== charCountValue * 2) {
    fail(`${at} 的 code 长度应等于字数 × 2`);
  }
  if (typeof length !== "number" || length !== code.length) {
    fail(`${at} 的 length 与 code 长度不一致`);
  }
  if (typeof charCount !== "number" || charCount !== charCountValue) {
    fail(`${at} 的 charCount 与 word 字数不一致`);
  }
  if (typeof rimeWeight !== "number" || !Number.isInteger(rimeWeight) || rimeWeight <= 0) {
    fail(`${at} 的 rimeWeight 应为正整数`);
  }
  return { word: text, code, length, charCount: charCountValue, rimeWeight };
}

function validateLevel1(value: unknown, index: number): TrainerLevel1Shortcut {
  const at = `第 ${index + 1} 条一级简码`;
  if (!isRecord(value)) fail(`${at} 结构无效`);
  const { key, char } = value;
  if (typeof key !== "string" || !/^[a-z]$/.test(key)) {
    fail(`${at} 的 key 应为单个小写字母`);
  }
  return { key, char: isHanziString(char, `${at} 的 char`) };
}

function validateShortcut(value: unknown, index: number): TrainerShortcut {
  const at = `第 ${index + 1} 条简码数据`;
  if (!isRecord(value)) fail(`${at} 结构无效`);
  const { word, fullCode, shortcutCode, mode } = value;
  const text = isHanziString(word, `${at} 的 word`);
  if (text.length < 2 || text.length > 4) {
    fail(`${at} 的 word 应为 2-4 字词`);
  }
  if (typeof fullCode !== "string" || !/^[a-z]{4,8}$/.test(fullCode)) {
    fail(`${at} 的 fullCode 应为 4-8 位小写字母`);
  }
  if (typeof shortcutCode !== "string" || !/^[a-z]{2,7}$/.test(shortcutCode)) {
    fail(`${at} 的 shortcutCode 应为 2-7 位小写字母`);
  }
  if (shortcutCode.length >= fullCode.length) {
    fail(`${at} 的 shortcutCode 应短于 fullCode`);
  }
  if (typeof mode !== "string" || !/^[FI]+$/.test(mode)) {
    fail(`${at} 的 mode 应为 F/I 投影串`);
  }
  return { word: text, fullCode, shortcutCode, mode };
}

function validateSentence(value: unknown, index: number): TrainerSentence {
  const at = `第 ${index + 1} 条组句数据`;
  if (!isRecord(value)) fail(`${at} 结构无效`);
  const { text, code, components } = value;
  const textValue = isHanziString(text, `${at} 的 text`);
  if (typeof code !== "string" || !/^[a-z]+$/.test(code)) {
    fail(`${at} 的 code 应为小写字母串`);
  }
  if (!Array.isArray(components) || components.length < 2) {
    fail(`${at} 的 components 应为至少 2 个词`);
  }
  const resolved = components.map((component, componentIndex) =>
    isHanziString(component, `${at} 的 components[${componentIndex}]`),
  );
  if (resolved.join("") !== textValue) {
    fail(`${at} 的 components 拼接应与 text 一致`);
  }
  if (code.length !== textValue.length * 2) {
    fail(`${at} 的 code 长度应等于字数 × 2`);
  }
  return { text: textValue, code, components: resolved };
}

function validateArray<T>(
  value: unknown,
  atLeast: number,
  label: string,
  validateItem: (item: unknown, index: number) => T,
): T[] {
  if (!Array.isArray(value) || value.length < atLeast) {
    fail(`${label} 应为至少 ${atLeast} 条的数组`);
  }
  return value.map(validateItem);
}

function ensureUnique(keys: string[], label: string): void {
  const seen = new Set<string>();
  for (const key of keys) {
    if (seen.has(key)) {
      fail(`${label} 存在重复条目:${key}`);
    }
    seen.add(key);
  }
}

function validateDoublePinyin(value: unknown): DoublePinyinReference {
  if (!isRecord(value)) fail("doublePinyin 结构无效");
  const { initials, finals, zeroInitials } = value;
  if (!Array.isArray(initials) || initials.length === 0) {
    fail("doublePinyin.initials 应为非空数组");
  }
  for (const mapping of initials) {
    if (
      !isRecord(mapping) ||
      typeof mapping.initial !== "string" ||
      typeof mapping.key !== "string" ||
      !/^[a-z]$/.test(mapping.key)
    ) {
      fail("doublePinyin.initials 条目结构无效");
    }
  }
  if (!Array.isArray(finals) || finals.length === 0) {
    fail("doublePinyin.finals 应为非空数组");
  }
  for (const mapping of finals) {
    if (
      !isRecord(mapping) ||
      typeof mapping.final !== "string" ||
      typeof mapping.key !== "string" ||
      !/^[a-z]$/.test(mapping.key)
    ) {
      fail("doublePinyin.finals 条目结构无效");
    }
  }
  if (!Array.isArray(zeroInitials) || zeroInitials.length === 0) {
    fail("doublePinyin.zeroInitials 应为非空数组");
  }
  for (const mapping of zeroInitials) {
    if (
      !isRecord(mapping) ||
      typeof mapping.syllable !== "string" ||
      typeof mapping.code !== "string" ||
      !/^[a-z]{2}$/.test(mapping.code)
    ) {
      fail("doublePinyin.zeroInitials 条目结构无效");
    }
  }
  return { initials, finals, zeroInitials } as DoublePinyinReference;
}

/**
 * 校验并返回 V2 数据集。
 *
 * 版本边界:schemaVersion 必须恰为 2——旧版本数据由 `pnpm build` 的
 * `generate:data` 重新生成,不做前端兼容解析。
 */
export function validateTrainerDataset(value: unknown): TrainerDataset {
  if (!isRecord(value)) fail("训练数据结构无效");
  if (value.schemaVersion !== 2) {
    fail(t("trainer.errorVersion", { actual: String(value.schemaVersion) }));
  }
  if (typeof value.packageVersion !== "string" || value.packageVersion === "") {
    fail("训练数据缺少 packageVersion");
  }
  const entries = validateArray(value.entries, 1, "entries", validateEntry);
  ensureUnique(entries.map((entry) => `${entry.char}:${entry.code}`), "entries");
  const words = validateArray(value.words, 1, "words", validateWord);
  ensureUnique(words.map((word) => `${word.word}:${word.code}`), "words");
  const level1Shortcuts = validateArray(
    value.level1Shortcuts,
    26,
    "level1Shortcuts",
    validateLevel1,
  );
  ensureUnique(level1Shortcuts.map((shortcut) => shortcut.key), "level1Shortcuts");
  const wordShortcuts = validateArray(value.wordShortcuts, 1, "wordShortcuts", validateShortcut);
  ensureUnique(
    wordShortcuts.map((shortcut) => `${shortcut.word}:${shortcut.shortcutCode}`),
    "wordShortcuts",
  );
  const fixedFirstShortcuts = validateArray(
    value.fixedFirstShortcuts,
    1,
    "fixedFirstShortcuts",
    validateShortcut,
  );
  ensureUnique(
    fixedFirstShortcuts.map((shortcut) => `${shortcut.word}:${shortcut.shortcutCode}`),
    "fixedFirstShortcuts",
  );
  const twoKeyShortcuts = validateArray(
    value.twoKeyShortcuts,
    1,
    "twoKeyShortcuts",
    validateShortcut,
  );
  ensureUnique(
    twoKeyShortcuts.map((shortcut) => `${shortcut.word}:${shortcut.shortcutCode}`),
    "twoKeyShortcuts",
  );
  const sentences = validateArray(value.sentences, 1, "sentences", validateSentence);
  ensureUnique(sentences.map((sentence) => sentence.text), "sentences");
  return {
    schemaVersion: 2,
    packageVersion: value.packageVersion,
    entries,
    words,
    level1Shortcuts,
    wordShortcuts,
    fixedFirstShortcuts,
    twoKeyShortcuts,
    sentences,
    doublePinyin: validateDoublePinyin(value.doublePinyin),
  };
}

/** 生成数据的默认加载地址(构建管线把数据集产物放到 public/generated)。 */
export const TRAINER_DATA_URL = `${import.meta.env.BASE_URL}generated/xhup_flow_trainer.json`;

/** 加载并校验训练数据集(fetch 失败或校验失败都会抛 {@link TrainerDataError})。 */
export async function loadTrainerDataset(
  url: string = TRAINER_DATA_URL,
): Promise<TrainerDataset> {
  let response: Response;
  try {
    response = await fetch(url);
  } catch (cause) {
    throw new TrainerDataError(t("trainer.errorLoad", { reason: String(cause) }));
  }
  if (!response.ok) {
    throw new TrainerDataError(
      t("trainer.errorLoad", { reason: `HTTP ${response.status}` }),
    );
  }
  let parsed: unknown;
  try {
    parsed = await response.json();
  } catch (cause) {
    throw new TrainerDataError(t("trainer.errorLoad", { reason: String(cause) }));
  }
  return validateTrainerDataset(parsed);
}
