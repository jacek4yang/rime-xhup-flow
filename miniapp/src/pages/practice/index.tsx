/**
 * 练习首页:全部 12 种练习模式的分组选择。模式可用性来自本机规范
 * 分片的池内容:池为空的模式禁用并标注「该模式数据未加载」,
 * 绝不进入空会话。
 */

import { View, Text } from "@tarojs/components";
import {
  MODE_DESCRIPTIONS,
  MODE_LABELS,
} from "@xhup/trainer-core";
import type { PracticeMode } from "@xhup/trainer-core";
import { trainerIndex } from "../../lib/dataset";
import { MODE_GROUPS, computeModeAvailability } from "../../lib/mode-availability";
import { useAppState, actions } from "../../lib/store";
import { useI18n } from "../../lib/i18n";
import { ROUTES, navigateTo } from "../../lib/navigation";

export default function Practice() {
  const state = useAppState();
  const { t } = useI18n();
  const availability = computeModeAvailability(trainerIndex);

  const pick = (mode: PracticeMode) => {
    actions.updateSettings({ lastMode: mode });
    navigateTo(ROUTES.practiceSetup(mode));
  };

  return (
    <View className="page">
      <View className="card">
        <Text className="title">{t("practice.chooseMode")}</Text>
        <Text className="subtitle">{t("practice.setupHint")}</Text>
      </View>

      {MODE_GROUPS.map((group) => (
        <View className="card" key={group.labelKey}>
          <Text className="card-heading">{t(group.labelKey)}</Text>
          {group.modes.map((mode) => {
            const available = availability[mode];
            return (
              <View
                key={mode}
                className={`mode-row ${available ? "" : "mode-row-disabled"} ${
                  state.settings.lastMode === mode ? "mode-row-active" : ""
                }`}
                hoverClass={available ? "action-row-hover" : "none"}
                onClick={() => available && pick(mode)}
              >
                <View className="action-main">
                  <Text className="action-title">{t(MODE_LABELS[mode])}</Text>
                  <Text className="subtitle">
                    {available
                      ? t(MODE_DESCRIPTIONS[mode])
                      : "该模式数据未加载"}
                  </Text>
                </View>
                {available && <Text className="action-arrow">›</Text>}
              </View>
            );
          })}
        </View>
      ))}
    </View>
  );
}
