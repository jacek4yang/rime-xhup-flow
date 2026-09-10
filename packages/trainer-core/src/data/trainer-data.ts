/**
 * 训练器规范数据(V4)的类型、运行时校验与加载。
 *
 * 唯一事实来源是 Rust 生成的 `xhup_flow_trainer.json`;前端不维护任何
 * 双拼映射、汉字编码、词码、简码策略或频率表。加载时完整校验一次,
 * 校验失败抛出带用户可读原因的 {@link TrainerDataError}。
 *
 * V4 在 optimizer v2 简码契约上加入生产 InputHanzi 的 core/extended
 * scope 与 attested provenance；旧 ZERO_REGRESSION / 独立二码层不再是 production。
 */

import { translate, type I18nKey, type Language } from "../i18n";

/** 数据加载/校验期的用户可读文案语言(错误先于任何 UI 上下文出现)。 */
export type TrainerDataTextOptions = {
  /** 缺省为中文。 */
  language?: Language;
};

function t(language: Language, key: I18nKey, params?: Record<string, string | number>): string {
  return translate(language, key, params);
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
  /** 字符属于规范 8105 core，还是有来源的扩展输入层。 */
  scope: "core" | "extended";
  /** 编码来自规范读音/形码推导，或官方/历史 attested 事实。 */
  codeSource: "canonical-reading-shape" | "official-attested" | "legacy-attested";
  sources: string[];
  statuses: string[];
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
  | "primary"
  | "fixed-first"
  | "primary-two-key";

/** 一条词语简码关系(shortcut 与 fullCode 都保留可用)。 */
export type TrainerShortcut = {
  word: string;
  fullCode: string;
  shortcutCode: string;
};

/** optimizer v2 PRIMARY 简码,含子集相对位次与实际静态菜单位次。 */
export type TrainerPrimaryShortcut = TrainerShortcut & {
  rank: number;
  mergedRank: number;
};

/** FIXED_FIRST 简码,格式可由单调 F/I 模式机械投影。 */
export type TrainerFixedFirstShortcut = TrainerShortcut & {
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

/** 校验后的 V4 数据集。 */
export type TrainerDataset = {
  schemaVersion: 4;
  packageVersion: string;
  entries: TrainerEntry[];
  words: TrainerWord[];
  level1Shortcuts: TrainerLevel1Shortcut[];
  primaryShortcuts: TrainerPrimaryShortcut[];
  fixedFirstShortcuts: TrainerFixedFirstShortcut[];
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
  if (typeof value !== "string" || !/^\p{Script=Han}+$/u.test(value)) {
    fail(`${at} 应为汉字串`);
  }
  return value;
}

function validateEntry(value: unknown, index: number): TrainerEntry {
  const at = `第 ${index + 1} 条训练数据`;
  if (!isRecord(value)) fail(`${at} 结构无效`);

  const {
    char,
    code,
    length,
    readings,
    toneReading,
    frequencyScore,
    rimeWeight,
    scope,
    codeSource,
    sources,
    statuses,
  } = value;

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
    !readings.every(isLowerAlpha) ||
    readings.some((reading) => reading.length < 1 || reading.length > 6)
  ) {
    fail(`${at} 的 readings 应为小写字母数组`);
  }
  if (scope !== "core" && scope !== "extended") {
    fail(`${at} 的 scope 应为 core/extended`);
  }
  if (
    codeSource !== "canonical-reading-shape" &&
    codeSource !== "official-attested" &&
    codeSource !== "legacy-attested"
  ) {
    fail(`${at} 的 codeSource 无效`);
  }
  if (codeSource === "canonical-reading-shape" && readings.length === 0) {
    fail(`${at} 的规范推导编码必须携带读音`);
  }
  const evidenceArray = (field: unknown, label: string): string[] => {
    if (!Array.isArray(field) || !field.every((item) => typeof item === "string")) {
      fail(`${at} 的 ${label} 应为字符串数组`);
    }
    return field as string[];
  };
  const sourceValues = evidenceArray(sources, "sources");
  const statusValues = evidenceArray(statuses, "statuses");
  if (codeSource !== "canonical-reading-shape" && sourceValues.length === 0) {
    fail(`${at} 的 attested 编码必须携带来源`);
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
    scope,
    codeSource,
    sources: sourceValues,
    statuses: statusValues,
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

function validateShortcutBase(value: unknown, index: number): TrainerShortcut {
  const at = `第 ${index + 1} 条简码数据`;
  if (!isRecord(value)) fail(`${at} 结构无效`);
  const { word, fullCode, shortcutCode } = value;
  const text = isHanziString(word, `${at} 的 word`);
  if (text.length < 2 || text.length > 4) {
    fail(`${at} 的 word 应为 2-4 字词`);
  }
  if (typeof fullCode !== "string" || !/^[a-z]{4,8}$/.test(fullCode)) {
    fail(`${at} 的 fullCode 应为 4-8 位小写字母`);
  }
  if (typeof shortcutCode !== "string" || !/^[a-z]{2,5}$/.test(shortcutCode)) {
    fail(`${at} 的 shortcutCode 应为 2-5 位小写字母`);
  }
  if (shortcutCode.length >= fullCode.length) {
    fail(`${at} 的 shortcutCode 应短于 fullCode`);
  }
  return { word: text, fullCode, shortcutCode };
}

function validatePrimaryShortcut(value: unknown, index: number): TrainerPrimaryShortcut {
  const base = validateShortcutBase(value, index);
  if (!isRecord(value)) fail(`第 ${index + 1} 条 PRIMARY 简码结构无效`);
  const { rank, mergedRank } = value;
  if (!Number.isSafeInteger(rank) || (rank as number) < 1) {
    fail(`第 ${index + 1} 条 PRIMARY 简码 rank 应为正整数`);
  }
  if (!Number.isSafeInteger(mergedRank) || (mergedRank as number) < (rank as number)) {
    fail(`第 ${index + 1} 条 PRIMARY 简码 mergedRank 应为不小于 rank 的正整数`);
  }
  return { ...base, rank: rank as number, mergedRank: mergedRank as number };
}

function validateFixedFirstShortcut(
  value: unknown,
  index: number,
): TrainerFixedFirstShortcut {
  const at = `第 ${index + 1} 条简码数据`;
  const base = validateShortcutBase(value, index);
  if (!isRecord(value)) fail(`第 ${index + 1} 条 FIXED_FIRST 简码结构无效`);
  const { mode } = value;
  if (typeof mode !== "string" || !/^[FI]+$/.test(mode)) {
    fail(`${at} 的 mode 应为 F/I 投影串`);
  }
  return { ...base, mode };
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
 * 校验并返回 V4 数据集。
 *
 * 版本边界:schemaVersion 必须恰为 4——旧版本数据由 `pnpm build` 的
 * `generate:data` 重新生成,不做前端兼容解析。
 */
export function validateTrainerDataset(
  value: unknown,
  options: TrainerDataTextOptions = {},
): TrainerDataset {
  const language = options.language ?? "zh";
  if (!isRecord(value)) fail("训练数据结构无效");
  if (value.schemaVersion !== 4) {
    fail(t(language, "trainer.errorVersion", { actual: String(value.schemaVersion) }));
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
  const primaryShortcuts = validateArray(
    value.primaryShortcuts,
    1,
    "primaryShortcuts",
    validatePrimaryShortcut,
  );
  ensureUnique(
    primaryShortcuts.map((shortcut) => `${shortcut.word}:${shortcut.shortcutCode}`),
    "primaryShortcuts",
  );
  const fixedFirstShortcuts = validateArray(
    value.fixedFirstShortcuts,
    1,
    "fixedFirstShortcuts",
    validateFixedFirstShortcut,
  );
  ensureUnique(
    fixedFirstShortcuts.map((shortcut) => `${shortcut.word}:${shortcut.shortcutCode}`),
    "fixedFirstShortcuts",
  );
  const sentences = validateArray(value.sentences, 1, "sentences", validateSentence);
  ensureUnique(sentences.map((sentence) => sentence.text), "sentences");
  return {
    schemaVersion: 4,
    packageVersion: value.packageVersion,
    entries,
    words,
    level1Shortcuts,
    primaryShortcuts,
    fixedFirstShortcuts,
    sentences,
    doublePinyin: validateDoublePinyin(value.doublePinyin),
  };
}

/**
 * 宿主注入的最小 fetch 契约(结构化;核心不引用 DOM 类型)。
 * Web/Tauri 直接传全局 fetch;小程序宿主可包装 `Taro.request`。
 */
export type TrainerDataFetch = (
  url: string,
) => Promise<{ ok: boolean; status: number; json(): Promise<unknown> }>;

/**
 * 加载训练数据集的选项。核心不假设任何宿主平台:URL 与 fetch 实现
 * 由调用方注入(Web/Tauri 用 fetch,小程序宿主可注入自己的加载器)。
 */
export type LoadTrainerDatasetOptions = TrainerDataTextOptions & {
  /** 数据集 JSON 地址(必填;各平台自行解析自己的生成数据位置)。 */
  url: string;
  /** 缺省使用全局 fetch(仅当宿主存在全局 fetch 时省略)。 */
  fetchImpl?: TrainerDataFetch;
};

/** 加载并校验训练数据集(fetch 失败或校验失败都会抛 {@link TrainerDataError})。 */
export async function loadTrainerDataset(
  options: LoadTrainerDatasetOptions,
): Promise<TrainerDataset> {
  const language = options.language ?? "zh";
  // 宿主未注入时回退到运行时全局 fetch(存在则结构兼容;类型上保持无 DOM)。
  const globalFetch = (globalThis as { fetch?: TrainerDataFetch }).fetch;
  const fetchImpl: TrainerDataFetch = options.fetchImpl ?? globalFetch!;
  let response: Awaited<ReturnType<TrainerDataFetch>>;
  try {
    response = await fetchImpl(options.url);
  } catch (cause) {
    throw new TrainerDataError(t(language, "trainer.errorLoad", { reason: String(cause) }));
  }
  if (!response.ok) {
    throw new TrainerDataError(
      t(language, "trainer.errorLoad", { reason: `HTTP ${response.status}` }),
    );
  }
  let parsed: unknown;
  try {
    parsed = await response.json();
  } catch (cause) {
    throw new TrainerDataError(t(language, "trainer.errorLoad", { reason: String(cause) }));
  }
  return validateTrainerDataset(parsed, { language });
}
