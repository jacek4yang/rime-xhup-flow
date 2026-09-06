import { View, Text } from "@tarojs/components";
import Taro, { useDidShow } from "@tarojs/taro";
import { useState } from "react";
import { dataset, trainerIndex } from "../../lib/dataset";
import { loadProgress } from "../../lib/storage";

export default function Home() {
  const [progressCount, setProgressCount] = useState(0);

  useDidShow(() => {
    setProgressCount(Object.keys(loadProgress()).length);
  });

  const goLearn = () => Taro.navigateTo({ url: "/pages/learn/index" });
  const goPractice = () => Taro.navigateTo({ url: "/pages/practice/index" });

  return (
    <View className="page">
      <View className="card">
        <Text className="title">小鹤音形 · 今日</Text>
        <Text className="subtitle">
          规范数据已就绪:{dataset.entries.length} 个高频单字、
          {dataset.words.length} 个固定词、{dataset.level1Shortcuts.length} 个一级简码
        </Text>
        <Text className="status-ok">
          数据校验通过(schemaVersion {dataset.schemaVersion}),索引 {Object.values(trainerIndex.frequencySorted).reduce((sum, items) => sum + items.length, 0)} 条
        </Text>
        <Text className="subtitle">本机已积累 {progressCount} 条学习进度</Text>
      </View>

      <View className="card button" onClick={goLearn}>
        <Text>学习中心</Text>
      </View>
      <View className="card button button-secondary" onClick={goPractice}>
        <Text>开始练习(5 题)</Text>
      </View>
    </View>
  );
}
