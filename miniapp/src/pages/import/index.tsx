/**
 * 备份导入页:选择文件 / 粘贴 → 校验预览 → 确认 → 应用。
 *
 * 导入机制(如实说明):
 * - 首选 wx.chooseMessageFile(Taro 4.2.1 类型已声明,weapp 支持):
 *   用户把桌面端导出的备份 JSON 经微信发送给自己(如「文件传输助手」),
 *   再从聊天记录选择该文件,经 FileSystemManager.readFile 读出文本。
 *   这是小程序侧唯一可靠的跨设备导入通道。
 * - 剪贴板不可跨设备:Taro.setClipboardData 仅本机剪贴板,手机与电脑
 *   剪贴板通常不同步;textarea 粘贴只是同机兜底(或经微信转发文本)。
 * - 应用语义与 app-store.applyBackup 一致:整体替换 progress / daily /
 *   keyErrors / confusions,偏好保留;确认前不触碰任何既有状态。
 */

import { useState } from "react";
import { Text, Textarea, View } from "@tarojs/components";
import Taro from "@tarojs/taro";
import { useI18n } from "../../lib/i18n";
import {
  parseBackupForImport,
  type ImportParseResult,
} from "../../lib/backup-import";
import { appStore } from "../../lib/store";

export default function ImportPage() {
  const { t } = useI18n();
  const [json, setJson] = useState("");
  const [result, setResult] = useState<ImportParseResult | null>(null);

  const failReasonText = (parse: ImportParseResult & { ok: false }): string =>
    parse.kind === "tooLarge"
      ? t("import.errorTooLarge")
      : t("import.errorInvalid", { reason: parse.reason });

  const parse = (text: string) => setResult(parseBackupForImport(text));

  const pickFile = () => {
    Taro.chooseMessageFile({
      count: 1,
      type: "file",
      extension: ["json"],
      success: (res) => {
        const file = res.tempFiles[0];
        if (!file) return;
        Taro.getFileSystemManager().readFile({
          filePath: file.path,
          encoding: "utf8",
          success: (read) => {
            const text = String(read.data);
            setJson(text);
            parse(text);
          },
          fail: (err) => {
            Taro.showToast({
              title: t("import.readFailed", { reason: err.errMsg ?? "" }),
              icon: "none",
            });
          },
        });
      },
    });
  };

  const apply = () => {
    if (!result || !result.ok) return;
    Taro.showModal({
      title: t("import.confirmTitle"),
      content: t("import.confirmBody"),
      confirmText: t("import.apply"),
      cancelText: t("common.cancel"),
      confirmColor: "#d93026",
      success: (res) => {
        if (!res.confirm) return;
        const { data } = result;
        // 仅在用户确认后整体替换(applyBackup 语义;偏好保留)。
        appStore.applyBackup({
          progress: data.progress,
          daily: data.daily,
          keyErrors: data.keyErrors,
          confusions: data.confusions,
        });
        Taro.showToast({ title: t("import.applied"), icon: "success" });
        setJson("");
        setResult(null);
      },
    });
  };

  return (
    <View className="page">
      <View className="card">
        <Text className="title">{t("import.title")}</Text>
        <Text className="subtitle">{t("settings.backupHint")}</Text>
      </View>

      <View className="card">
        <Text className="card-heading">{t("import.pickFile")}</Text>
        <Text className="subtitle">{t("import.fileHint")}</Text>
        <View className="button" onClick={pickFile}>
          <Text>{t("import.pickFile")}</Text>
        </View>
        <Text className="subtitle warning-soft">{t("import.clipboardNote")}</Text>

        <Text className="card-heading">{t("import.pasteLabel")}</Text>
        <Textarea
          className="textarea-input"
          value={json}
          placeholder={t("import.pastePlaceholder")}
          maxlength={-1}
          onInput={(event) => {
            setJson(event.detail.value);
            setResult(null);
          }}
        />
        <View
          className={`button ${json === "" ? "button-disabled" : ""}`}
          onClick={() => json !== "" && parse(json)}
        >
          <Text>{t("import.parse")}</Text>
        </View>
      </View>

      {result && !result.ok && (
        <View className="card">
          <Text className="warning-text">{failReasonText(result)}</Text>
        </View>
      )}

      {result && result.ok && (
        <View className="card">
          <Text className="card-heading">{t("import.title")}</Text>
          <Text className="body-text">
            {t("import.previewVersion")}: v{result.preview.version}
          </Text>
          <Text className="body-text">
            {t("import.previewProgress")}: {result.preview.progressCount}
          </Text>
          <Text className="body-text">
            {t("import.previewDays")}: {result.preview.dailyDays}
          </Text>
          <Text className="body-text">
            {t("import.previewRange")}:{" "}
            {result.preview.dateRange === null
              ? "—"
              : `${result.preview.dateRange[0]} ~ ${result.preview.dateRange[1]}`}
          </Text>
          <Text className="body-text">
            {t("import.previewKeyErrors")}: {result.preview.keyErrorKeys}
          </Text>
          <View className="button button-danger" onClick={apply}>
            <Text>{t("import.apply")}</Text>
          </View>
        </View>
      )}
    </View>
  );
}
