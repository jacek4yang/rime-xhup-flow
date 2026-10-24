/**
 * 学习中心(小程序):共享核心 LEARN_CHAPTERS 的 9 章清单,按 level
 * 分组展示。课程内容与桌面端同源,不复制不改编。
 *
 * 个人进度边界(诚实推导,不伪造):小程序没有桌面端的 lessonEvidence
 * 章节记账,章节「学习状态」在本机不可推导 → 不显示状态徽标;
 * 覆盖数(已练/关联条目)只依赖本机真实练习进度,可靠 → 展示。
 */

import { useMemo, useState } from "react";
import { View, Text } from "@tarojs/components";
import { useDidShow } from "@tarojs/taro";
import { LEARN_CHAPTERS, LEVEL_LABELS } from "@xhup/trainer-core";
import type { LearnChapterLevel } from "@xhup/trainer-core";
import { useTrainerData } from "../../lib/trainer-data";
import { useAppState } from "../../lib/store";
import { chapterCoverageById } from "../../lib/learn-coverage";
import { ROUTES, navigateTo } from "../../lib/navigation";
import { useI18n } from "../../lib/i18n";

/** level 展示顺序(与课程体系一致)。 */
const LEVEL_ORDER: readonly LearnChapterLevel[] = [
  "beginner",
  "basic",
  "intermediate",
  "advanced",
  "mastery",
];

function Learn() {
  const { t } = useI18n();
  const { trainerIndex } = useTrainerData();
  const state = useAppState();
  useRefreshOnShow();

  // 只取覆盖数(已练/关联);状态徽标因缺章节记账在本机不可推导,不展示。
  const coverageById = useMemo(
    () => chapterCoverageById(trainerIndex, state.progress, Date.now()),
    [trainerIndex, state.progress],
  );

  const groups = LEVEL_ORDER.map((level) => ({
    level,
    chapters: LEARN_CHAPTERS.filter((chapter) => chapter.level === level),
  })).filter((group) => group.chapters.length > 0);

  return (
    <View className="page">
      <View className="card">
        <Text className="title">{t("nav.learn")}</Text>
        <Text className="subtitle">共 {LEARN_CHAPTERS.length} 章 · 从入门到精通</Text>
      </View>

      {groups.map((group) => (
        <View className="card" key={group.level}>
          <Text className="card-heading">{LEVEL_LABELS[group.level]}</Text>
          {group.chapters.map((chapter) => {
            const coverage = coverageById.get(chapter.id);
            return (
              <View
                key={chapter.id}
                className="action-row"
                hoverClass="action-row-hover"
                onClick={() => navigateTo(ROUTES.lesson(chapter.id))}
              >
                <View className="action-main">
                  <Text className="action-title">{chapter.title}</Text>
                  <Text className="subtitle">{chapter.summary}</Text>
                  {coverage && coverage.total > 0 && coverage.practiced > 0 && (
                    <Text className="subtitle">
                      {t("learn.coverage", {
                        practiced: coverage.practiced,
                        total: coverage.total,
                      })}
                    </Text>
                  )}
                </View>
                <Text className="action-arrow">›</Text>
              </View>
            );
          })}
        </View>
      ))}
    </View>
  );
}

/** 页面重新显示(从练习返回/后台恢复)时触发一次重渲染刷新覆盖数。 */
function useRefreshOnShow(): void {
  const [, setTick] = useState(0);
  useDidShow(() => setTick((value) => value + 1));
}

export default Learn;
