/**
 * 设置页(tab「我的」):语言、提示方式、错误教学、键帽参考、触感
 * 反馈、进度备份导出与重置(带确认)。全部偏好写入应用状态并持久化;
 * 备份格式与桌面端一致(不含规范数据)。
 */

import { useState } from "react";
import { View, Text } from "@tarojs/components";
import Taro from "@tarojs/taro";
import {
  BACKUP_VERSION,
  ERROR_TEACHING_MODES,
  HINT_MODE_LABELS,
  KEY_REF_MODES,
  LANGUAGES,
  LANGUAGE_LABELS,
  exportBackup,
} from "@xhup/trainer-core";
import type {
  ErrorTeachingMode,
  HintMode,
  HapticsMode,
  KeyRefMode,
  Language,
} from "@xhup/trainer-core";
import { HAPTICS_MODES } from "@xhup/trainer-core";
import { appStore, actions, useAppState } from "../../lib/store";
import { useI18n } from "../../lib/i18n";
import { dataset } from "../../lib/dataset";
import { ROUTES, navigateTo } from "../../lib/navigation";

/** 提示方式全集(核心未导出枚举表;与 HintMode 一一对应)。 */
const HINT_MODES: readonly HintMode[] = ["always", "on-delay", "on-error", "hidden"];

/** 触感档位标签。 */
const HAPTICS_LABELS: Record<HapticsMode, string> = {
  off: "关",
  light: "轻",
  medium: "中",
};

/** 键帽参考标签。 */
const KEY_REF_LABELS: Record<KeyRefMode, string> = {
  contextual: "情境",
  none: "无",
  double: "双拼",
  shape: "形码",
  both: "双拼+形码",
};

/** 错误教学标签(核心 i18n 键)。 */
const ERROR_TEACHING_LABELS: Record<ErrorTeachingMode, Parameters<ReturnType<typeof useI18n>["t"]>[0]> = {
  quick: "practice.errorTeaching.quick",
  adaptive: "practice.errorTeaching.adaptive",
  detailed: "practice.errorTeaching.detailed",
};

export default function Settings() {
  const state = useAppState();
  const { t } = useI18n();
  const [exported, setExported] = useState(false);

  const update = actions.updateSettings;

  const exportProgress = () => {
    const current = appStore.getState();
    const json = exportBackup(
      {
        theme: "light",
        hintMode: current.settings.hintMode,
        difficulty: current.settings.difficulty,
        sessionLength: current.settings.sessionLength,
        lastMode: current.settings.lastMode,
        progress: current.progress,
        daily: current.daily,
        keyErrors: current.keyErrors,
      },
      Date.now(),
    );
    Taro.setClipboardData({
      data: json,
      success: () => setExported(true),
    });
  };

  const resetProgress = () => {
    Taro.showModal({
      title: t("settings.reset"),
      content: t("settings.resetHint"),
      confirmText: t("settings.reset"),
      cancelText: t("common.cancel"),
      confirmColor: "#d93026",
      success: (res) => {
        if (res.confirm) actions.resetProgress();
      },
    });
  };

  return (
    <View className="page">
      <View className="card">
        <Text className="title">{t("app.name")}</Text>
        <Text className="subtitle">{t("settings.privacyHint")}</Text>
      </View>

      <View className="card">
        <View
          className="action-row"
          hoverClass="action-row-hover"
          onClick={() => navigateTo(ROUTES.stats)}
        >
          <View className="action-main">
            <Text className="action-title">{t("nav.stats")}</Text>
            <Text className="subtitle">{t("stats.entryHint")}</Text>
          </View>
          <Text className="action-arrow">›</Text>
        </View>
      </View>

      <View className="card">
        <Text className="card-heading">{t("settings.language")}</Text>        <View className="option-row">
          {LANGUAGES.map((language) => (
            <View
              key={language}
              className={`option-chip ${
                state.settings.language === language ? "option-chip-active" : ""
              }`}
              onClick={() => update({ language: language as Language })}
            >
              <Text className="option-chip-text">{LANGUAGE_LABELS[language]}</Text>
            </View>
          ))}
        </View>

        <Text className="card-heading">{t("practice.hint")}</Text>
        <View className="option-row">
          {HINT_MODES.map((hintMode) => (
            <View
              key={hintMode}
              className={`option-chip ${
                state.settings.hintMode === hintMode ? "option-chip-active" : ""
              }`}
              onClick={() => update({ hintMode: hintMode as HintMode })}
            >
              <Text className="option-chip-text">{t(HINT_MODE_LABELS[hintMode])}</Text>
            </View>
          ))}
        </View>

        <Text className="card-heading">{t("practice.errorTeaching")}</Text>
        <View className="option-row">
          {ERROR_TEACHING_MODES.map((mode) => (
            <View
              key={mode}
              className={`option-chip ${
                state.settings.errorTeaching === mode ? "option-chip-active" : ""
              }`}
              onClick={() => update({ errorTeaching: mode })}
            >
              <Text className="option-chip-text">{t(ERROR_TEACHING_LABELS[mode])}</Text>
            </View>
          ))}
        </View>

        <Text className="card-heading">{t("practice.keyRefMode")}</Text>
        <View className="option-row">
          {KEY_REF_MODES.map((refMode) => (
            <View
              key={refMode}
              className={`option-chip ${
                state.settings.keyRefMode === refMode ? "option-chip-active" : ""
              }`}
              onClick={() => update({ keyRefMode: refMode })}
            >
              <Text className="option-chip-text">{KEY_REF_LABELS[refMode]}</Text>
            </View>
          ))}
        </View>

        <Text className="card-heading">{t("practice.haptics")}</Text>
        <View className="option-row">
          {HAPTICS_MODES.map((mode) => (
            <View
              key={mode}
              className={`option-chip ${
                state.settings.haptics === mode ? "option-chip-active" : ""
              }`}
              onClick={() => update({ haptics: mode })}
            >
              <Text className="option-chip-text">{HAPTICS_LABELS[mode]}</Text>
            </View>
          ))}
        </View>
      </View>

      <View className="card">
        <Text className="card-heading">{t("settings.backup")}</Text>
        <Text className="subtitle">{t("settings.backupHint")}</Text>
        <View className="button" onClick={exportProgress}>
          <Text>{t("settings.export")}</Text>
        </View>
        {exported && <Text className="status-ok">已复制到剪贴板(备份 v{BACKUP_VERSION})</Text>}

        <View
          className="button button-secondary"
          onClick={() => navigateTo(ROUTES.importBackup)}
        >
          <Text>{t("settings.import")}</Text>
        </View>

        <View className="button button-danger" onClick={resetProgress}>
          <Text>{t("settings.reset")}</Text>
        </View>
      </View>

      <View className="card">
        <Text className="card-heading">{t("settings.data")}</Text>
        <Text className="subtitle">{t("settings.dataHint")}</Text>
        <Text className="subtitle">
          {t("settings.dataVersion")}: {dataset.packageVersion} · schema {dataset.schemaVersion}
        </Text>
      </View>
    </View>
  );
}
