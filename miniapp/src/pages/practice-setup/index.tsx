/**
 * 练习设置页:题数、难度、提示方式、键帽参考。选择即写入应用偏好
 * (与桌面端 setup 语义一致),「开始练习」进入会话页。
 */

import { View, Text } from "@tarojs/components";
import {
  DEFAULT_DIFFICULTY,
  DIFFICULTY_LABELS,
  HINT_MODE_LABELS,
  MODE_LABELS,
  SESSION_LENGTH_OPTIONS,
  KEY_REF_MODES,
} from "@xhup/trainer-core";
import type {
  Difficulty,
  HintMode,
  KeyRefMode,
  PracticeMode,
} from "@xhup/trainer-core";
import { trainerIndex } from "../../lib/dataset";
import { isModeAvailable } from "../../lib/mode-availability";
import { actions, useAppState } from "../../lib/store";
import { useI18n } from "../../lib/i18n";
import { ROUTES, navigateToOrRedirect, pageParam } from "../../lib/navigation";

/** 提示方式全集(核心未导出枚举表;与 HintMode 一一对应)。 */
const HINT_MODES: readonly HintMode[] = ["always", "on-delay", "on-error", "hidden"];

/** 键帽参考的展示标签(核心无 i18n 键的用核心原词)。 */
const KEY_REF_LABELS: Record<KeyRefMode, string> = {
  contextual: "情境",
  none: "无",
  double: "双拼",
  shape: "形码",
  both: "双拼+形码",
};

export default function PracticeSetup() {
  const state = useAppState();
  const { t } = useI18n();

  // 模式来自路由参数;非法或缺省回退当前偏好模式。
  const modeParam = pageParam("mode");
  const mode: PracticeMode = isPracticeMode(modeParam)
    ? modeParam
    : state.settings.lastMode;
  const available = isModeAvailable(trainerIndex, mode);

  const lengthLabel = (length: number) =>
    length === 0 ? t("practice.lengthUnlimited") : String(length);

  const start = () => {
    actions.updateSettings({ lastMode: mode });
    navigateToOrRedirect(ROUTES.session(mode));
  };

  return (
    <View className="page">
      <View className="card">
        <Text className="title">{t(MODE_LABELS[mode])}</Text>
        {!available && (
          <Text className="subtitle warning-text">该模式数据未加载,请返回选择其他模式。</Text>
        )}
      </View>

      <View className="card">
        <Text className="card-heading">{t("practice.length")}</Text>
        <View className="option-row">
          {SESSION_LENGTH_OPTIONS.map((length) => (
            <OptionChip
              key={length}
              label={lengthLabel(length)}
              active={state.settings.sessionLength === length}
              onSelect={() => actions.updateSettings({ sessionLength: length })}
            />
          ))}
        </View>

        <Text className="card-heading">{t("practice.difficulty")}</Text>
        <View className="option-row">
          {(Object.keys(DIFFICULTY_LABELS) as Difficulty[]).map((difficulty) => (
            <OptionChip
              key={difficulty}
              label={t(DIFFICULTY_LABELS[difficulty])}
              active={(state.settings.difficulty ?? DEFAULT_DIFFICULTY) === difficulty}
              onSelect={() => actions.updateSettings({ difficulty })}
            />
          ))}
        </View>

        <Text className="card-heading">{t("practice.hint")}</Text>
        <View className="option-row">
          {HINT_MODES.map((hintMode) => (
            <OptionChip
              key={hintMode}
              label={t(HINT_MODE_LABELS[hintMode])}
              active={state.settings.hintMode === hintMode}
              onSelect={() => actions.updateSettings({ hintMode })}
            />
          ))}
        </View>

        <Text className="card-heading">{t("practice.keyRefMode")}</Text>
        <View className="option-row">
          {KEY_REF_MODES.map((refMode) => (
            <OptionChip
              key={refMode}
              label={KEY_REF_LABELS[refMode]}
              active={state.settings.keyRefMode === refMode}
              onSelect={() => actions.updateSettings({ keyRefMode: refMode })}
            />
          ))}
        </View>
      </View>

      <View
        className={`button ${available ? "" : "button-disabled"}`}
        onClick={() => available && start()}
      >
        <Text>{t("practice.start")}</Text>
      </View>
    </View>
  );
}

/** 单个设置项胶囊。 */
function OptionChip({
  label,
  active,
  onSelect,
}: {
  label: string;
  active: boolean;
  onSelect: () => void;
}) {
  return (
    <View
      className={`option-chip ${active ? "option-chip-active" : ""}`}
      onClick={onSelect}
    >
      <Text className="option-chip-text">{label}</Text>
    </View>
  );
}

function isPracticeMode(value: string | null): value is PracticeMode {
  return (
    value === "double" ||
    value === "sound-shape" ||
    value === "full" ||
    value === "mixed" ||
    value === "level1" ||
    value === "two-key-word" ||
    value === "zero-regression" ||
    value === "fixed-first" ||
    value === "fixed-word" ||
    value === "mixed-shortcut" ||
    value === "sentence" ||
    value === "mixed-all"
  );
}
