/**
 * 教学键盘(Taro 版):QWERTY 错位三行 + 退格,固定 rpx 几何。
 *
 * 质量对齐桌面端 OnScreenKeyboard(PR #39):
 * - 固定键宽/键高与字号,内容变化不漂移;
 * - 大触摸目标(键高 96rpx)、大主字母、清晰按下态(hover-class);
 * - 情境二级标签:双拼声母(左上)/韵母(左下)、形码代表字(右上/右下),
 *   全部来自规范数据;无数据的标签留空,绝不编造;
 * - 「下一期待键」高亮由调用方按提示策略门控后传入,组件不自行判断。
 */

import { View, Text } from "@tarojs/components";
import { KEY_ROWS, compactFinals, compactInitials } from "../lib/keyboard-labels";
import type { KeyLabels } from "../lib/keyboard-labels";

/** 键帽参考内容模式(情境模式由调用方按练习模式解析后再传入)。 */
export type KeyboardRefMode = "none" | "double" | "shape" | "both";

/** 形码参考:键 → {首形代表字, 次形代表字}(调用方从规范数据聚合)。 */
export type ShapeKeyRef = Map<string, { first: string; second: string }>;

export function TeachingKeyboard({
  labels,
  shapeRef,
  refMode = "none",
  nextKey = null,
  wrongKey = null,
  onKeyPress,
  onKeyInfo,
  showBackspace = false,
  onBackspace,
}: {
  labels: Map<string, KeyLabels>;
  shapeRef?: ShapeKeyRef;
  refMode?: KeyboardRefMode;
  nextKey?: string | null;
  wrongKey?: string | null;
  onKeyPress?: (key: string) => void;
  /** 参考模式(键位页):点按 = 查看键详情。 */
  onKeyInfo?: (key: string) => void;
  showBackspace?: boolean;
  onBackspace?: () => void;
}) {
  const showDouble = refMode === "double" || refMode === "both";
  const showShape = refMode === "shape" || refMode === "both";
  const press = onKeyPress ?? onKeyInfo;

  return (
    <View className="kb">
      {KEY_ROWS.map((row, rowIndex) => (
        <View className={`kb-row kb-row-${rowIndex}`} key={rowIndex}>
          {row.map((key) => {
            const label = labels.get(key);
            const shape = shapeRef?.get(key);
            const topLeft = showDouble ? compactInitials(label?.initials ?? []) : "";
            const bottomLeft = showDouble ? compactFinals(label?.finals ?? []) : "";
            const topRight = showShape ? (shape?.first ?? "") : "";
            const bottomRight = showShape ? (shape?.second ?? "") : "";
            const stateClass = nextKey === key
              ? "kb-key-next"
              : wrongKey === key
                ? "kb-key-wrong"
                : "";
            return (
              <View
                key={key}
                className={`kb-key ${stateClass}`}
                hoverClass="kb-key-pressed"
                hoverStartTime={0}
                hoverStayTime={80}
                onClick={() => press?.(key)}
              >
                {topLeft !== "" && <Text className="kb-ref kb-ref-tl">{topLeft}</Text>}
                {topRight !== "" && <Text className="kb-ref kb-ref-tr">{topRight}</Text>}
                <Text className="kb-letter">{key.toUpperCase()}</Text>
                {bottomLeft !== "" && <Text className="kb-ref kb-ref-bl">{bottomLeft}</Text>}
                {bottomRight !== "" && <Text className="kb-ref kb-ref-br">{bottomRight}</Text>}
              </View>
            );
          })}
        </View>
      ))}
      {showBackspace && (
        <View className="kb-row">
          <View
            className="kb-key kb-key-backspace"
            hoverClass="kb-key-pressed"
            hoverStartTime={0}
            hoverStayTime={80}
            onClick={() => onBackspace?.()}
          >
            <Text className="kb-backspace-label">⌫ 退格</Text>
          </View>
        </View>
      )}
    </View>
  );
}
