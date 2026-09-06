/**
 * 学习中心(小程序):共享核心 LEARN_CHAPTERS 的 9 章清单,按 level
 * 分组展示。课程内容与桌面端同源,不复制不改编。
 */

import { View, Text } from "@tarojs/components";
import { LEVEL_LABELS, LEARN_CHAPTERS } from "@xhup/trainer-core";
import type { LearnChapterLevel } from "@xhup/trainer-core";
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

export default function Learn() {
  const { t } = useI18n();
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
          {group.chapters.map((chapter) => (
            <View
              key={chapter.id}
              className="action-row"
              hoverClass="action-row-hover"
              onClick={() => navigateTo(ROUTES.lesson(chapter.id))}
            >
              <View className="action-main">
                <Text className="action-title">{chapter.title}</Text>
                <Text className="subtitle">{chapter.summary}</Text>
              </View>
              <Text className="action-arrow">›</Text>
            </View>
          ))}
        </View>
      ))}
    </View>
  );
}
