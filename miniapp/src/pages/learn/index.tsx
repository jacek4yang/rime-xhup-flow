/**
 * 学习中心(小程序):渲染共享核心课程体系的入门章节。
 * 课程内容与桌面端同源(LEARN_CHAPTERS),不复制不改编。
 */

import { View, Text } from "@tarojs/components";
import Taro from "@tarojs/taro";
import { LEARN_CHAPTERS } from "@xhup/trainer-core";

const CHAPTER = LEARN_CHAPTERS[0];

export default function Learn() {
  return (
    <View className="page">
      <View className="card">
        <Text className="title">{CHAPTER.title}</Text>
        {CHAPTER.sections.map((section, index) => (
          <View key={index} style={{ marginTop: "16rpx" }}>
            {section.kind === "text" && (
              <View>
                <Text className="subtitle">{section.heading}</Text>
                {section.paragraphs.map((paragraph, pIndex) => (
                  <Text key={pIndex} className="body-text" style={{ display: "block" }}>
                    {paragraph}
                  </Text>
                ))}
              </View>
            )}
            {section.kind === "list" && (
              <View>
                <Text className="subtitle">{section.heading}</Text>
                {section.items.map((item, iIndex) => (
                  <Text key={iIndex} className="body-text" style={{ display: "block" }}>
                    · {item}
                  </Text>
                ))}
              </View>
            )}
          </View>
        ))}
      </View>

      <View
        className="card button"
        onClick={() => Taro.navigateTo({ url: "/pages/practice/index" })}
      >
        <Text>去练习</Text>
      </View>
    </View>
  );
}
