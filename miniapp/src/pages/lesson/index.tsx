/**
 * 课程章节页:渲染共享核心 LEARN_CHAPTERS 的完整章节,覆盖全部
 * section 种类(text/list/practice/shape-explorer)。课程正文为规范
 * 中文;形码探索器数据来自本机规范分片聚合(数据不足时如实降级)。
 */

import { useMemo, useState } from "react";
import { View, Text } from "@tarojs/components";
import {
  LEARN_CHAPTERS,
  LEVEL_LABELS,
  MODE_LABELS,
  buildShapeKeyStats,
  translate,
} from "@xhup/trainer-core";
import type {
  LearnChapter,
  LearnSection,
  Language,
  ShapeKeyStat,
} from "@xhup/trainer-core";
import { dataset } from "../../lib/dataset";
import { useAppState } from "../../lib/store";
import { useI18n } from "../../lib/i18n";
import { ROUTES, navigateTo, pageParam } from "../../lib/navigation";

export default function Lesson() {
  const state = useAppState();
  const { t } = useI18n();
  const chapter = useMemo<LearnChapter | null>(() => {
    const id = pageParam("id");
    return LEARN_CHAPTERS.find((candidate) => candidate.id === id) ?? null;
  }, []);

  if (!chapter) {
    return (
      <View className="page">
        <View className="card">
          <Text className="title">{t("common.unknownError")}</Text>
          <Text className="subtitle">未找到该章节,请从学习中心重新进入。</Text>
        </View>
      </View>
    );
  }

  const chapterIndex = LEARN_CHAPTERS.findIndex((c) => c.id === chapter.id);

  return (
    <View className="page">
      <View className="card">
        <Text className="subtitle">{LEVEL_LABELS[chapter.level]}</Text>
        <Text className="title">{chapter.title}</Text>
        <Text className="body-text">{chapter.summary}</Text>
      </View>

      {chapter.sections.map((section, index) => (
        <SectionView key={index} section={section} language={state.settings.language} />
      ))}

      <View className="card">
        <Text className="subtitle">
          {translate(state.settings.language, "learn.chapterOf", {
            current: chapterIndex + 1,
            total: LEARN_CHAPTERS.length,
          })}
        </Text>
      </View>
    </View>
  );
}

/** 按顺序渲染单节内容。 */
function SectionView({
  section,
  language,
}: {
  section: LearnSection;
  language: Language;
}) {
  if (section.kind === "text") {
    return (
      <View className="card">
        {section.heading && <Text className="card-heading">{section.heading}</Text>}
        {section.paragraphs.map((paragraph, pIndex) => (
          <Text key={pIndex} className="body-text paragraph">
            {paragraph}
          </Text>
        ))}
      </View>
    );
  }
  if (section.kind === "list") {
    return (
      <View className="card">
        <Text className="card-heading">{section.heading}</Text>
        {section.items.map((item, iIndex) => (
          <Text key={iIndex} className="body-text paragraph">
            {section.ordered ? `${iIndex + 1}. ${item}` : `· ${item}`}
          </Text>
        ))}
      </View>
    );
  }
  if (section.kind === "practice") {
    return (
      <View className="card">
        <Text className="card-heading">{section.heading}</Text>
        <Text className="subtitle">{translate(language, MODE_LABELS[section.mode])}</Text>
        <View
          className="button"
          onClick={() => navigateTo(ROUTES.practiceSetup(section.mode))}
        >
          <Text>去练习</Text>
        </View>
      </View>
    );
  }
  // shape-explorer:形码探索器(数据驱动;分片不足时如实展示少量例字)。
  return <ShapeExplorer heading={section.heading} />;
}

/** 形码探索器:规范分片全码数据按首形/次形键聚合,点键查看例字。 */
function ShapeExplorer({ heading }: { heading: string }) {
  const [openKey, setOpenKey] = useState<string | null>(null);
  const stats = useMemo<ShapeKeyStat[]>(() => buildShapeKeyStats(dataset.entries), []);
  const active = stats.find((stat) => stat.key === openKey) ?? null;

  return (
    <View className="card">
      <Text className="card-heading">{heading}</Text>
      <Text className="subtitle">
        例字与计数全部来自本机训练数据(全码第 3、4 位);小程序分片只含高频子集。
      </Text>
      <View className="key-grid">
        {stats.map((stat) => (
          <View
            key={stat.key}
            className={`key-cell ${openKey === stat.key ? "key-cell-active" : ""}`}
            onClick={() => setOpenKey(stat.key)}
          >
            <Text className="key-cell-key">{stat.key.toUpperCase()}</Text>
            <Text className="key-cell-count">{stat.firstCount + stat.secondCount} 字</Text>
          </View>
        ))}
      </View>
      {active && (
        <View className="shape-samples">
          {active.firstSamples.length > 0 && (
            <Text className="body-text paragraph">
              首形 = {active.key.toUpperCase()}:
              {active.firstSamples.map((sample) => `${sample.char}(${sample.code})`).join(" ")}
            </Text>
          )}
          {active.secondSamples.length > 0 && (
            <Text className="body-text paragraph">
              次形 = {active.key.toUpperCase()}:
              {active.secondSamples.map((sample) => `${sample.char}(${sample.code})`).join(" ")}
            </Text>
          )}
        </View>
      )}
    </View>
  );
}
