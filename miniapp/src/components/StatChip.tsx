/**
 * 实况/小结统计芯片:标签 + 数值的紧凑展示。
 */

import { View, Text } from "@tarojs/components";

export function StatChip({
  label,
  value,
}: {
  label: string;
  value: string | number;
}) {
  return (
    <View className="stat-chip">
      <Text className="stat-value">{value}</Text>
      <Text className="stat-label">{label}</Text>
    </View>
  );
}
