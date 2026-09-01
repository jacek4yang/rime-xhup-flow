import { useState } from "react";
import { Trash2 } from "lucide-react";
import { Button } from "@/components/ui/button";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogTitle,
} from "@/components/ui/dialog";
import { OptionButton, OptionRow } from "@/components/OptionGroup";
import { Separator } from "@/components/ui/separator";
import { THEME_LABELS, type ThemePreference } from "@/lib/theme";
import { useTrainerIndex } from "@/lib/trainer-context";
import { useTrainerStore } from "@/stores/trainer-store";
import type { Difficulty } from "@/lib/trainer-index";
import {
  DIFFICULTY_LABELS,
  HINT_MODE_LABELS,
  SESSION_LENGTH_OPTIONS,
  type HintMode,
} from "@/features/practice/types";

const THEMES: ThemePreference[] = ["system", "light", "dark"];
const HINT_MODES: HintMode[] = ["always", "on-error", "hidden"];
const DIFFICULTIES: Difficulty[] = ["beginner", "daily", "full"];

export function SettingsView() {
  const index = useTrainerIndex();
  const theme = useTrainerStore((state) => state.theme);
  const hintMode = useTrainerStore((state) => state.hintMode);
  const difficulty = useTrainerStore((state) => state.difficulty);
  const sessionLength = useTrainerStore((state) => state.sessionLength);
  const setTheme = useTrainerStore((state) => state.setTheme);
  const setHintMode = useTrainerStore((state) => state.setHintMode);
  const setDifficulty = useTrainerStore((state) => state.setDifficulty);
  const setSessionLength = useTrainerStore((state) => state.setSessionLength);
  const resetProgress = useTrainerStore((state) => state.resetProgress);
  const [confirmOpen, setConfirmOpen] = useState(false);

  return (
    <div className="flex max-w-2xl flex-col gap-4">
      <header>
        <h1 className="text-xl font-semibold">设置</h1>
      </header>

      <Card>
        <CardHeader>
          <CardTitle>偏好</CardTitle>
        </CardHeader>
        <CardContent className="flex flex-col gap-4">
          <OptionRow label="主题">
            {THEMES.map((candidate) => (
              <OptionButton
                key={candidate}
                selected={theme === candidate}
                onClick={() => setTheme(candidate)}
              >
                {THEME_LABELS[candidate]}
              </OptionButton>
            ))}
          </OptionRow>
          <OptionRow label="提示方式">
            {HINT_MODES.map((candidate) => (
              <OptionButton
                key={candidate}
                selected={hintMode === candidate}
                onClick={() => setHintMode(candidate)}
              >
                {HINT_MODE_LABELS[candidate]}
              </OptionButton>
            ))}
          </OptionRow>
          <OptionRow label="默认难度">
            {DIFFICULTIES.map((candidate) => (
              <OptionButton
                key={candidate}
                selected={difficulty === candidate}
                onClick={() => setDifficulty(candidate)}
              >
                {DIFFICULTY_LABELS[candidate]}
              </OptionButton>
            ))}
          </OptionRow>
          <OptionRow label="默认题数">
            {SESSION_LENGTH_OPTIONS.map((candidate) => (
              <OptionButton
                key={candidate}
                selected={sessionLength === candidate}
                onClick={() => setSessionLength(candidate)}
              >
                {candidate === 0 ? "无限" : candidate}
              </OptionButton>
            ))}
          </OptionRow>
        </CardContent>
      </Card>

      <Card>
        <CardHeader>
          <CardTitle>训练数据</CardTitle>
          <CardDescription>
            码表与词频由 Rust 生成器统一产出,前端只读取,不维护副本。
          </CardDescription>
        </CardHeader>
        <CardContent className="flex flex-col gap-2 text-sm">
          <div className="flex justify-between">
            <span className="text-muted-foreground">数据版本</span>
            <span className="font-mono">{index.dataset.packageVersion}</span>
          </div>
          <div className="flex justify-between">
            <span className="text-muted-foreground">数据协议</span>
            <span className="font-mono">
              schemaVersion {index.dataset.schemaVersion}
            </span>
          </div>
          <div className="flex justify-between">
            <span className="text-muted-foreground">条目数</span>
            <span className="font-mono tabular-nums">
              {index.dataset.entries.length}
            </span>
          </div>
          <Separator className="my-2" />
          <p className="text-sm text-muted-foreground">
            练习记录仅保存在本机浏览器 / 应用本地存储中,不会上传。
          </p>
        </CardContent>
      </Card>

      <Card>
        <CardHeader>
          <CardTitle>学习进度</CardTitle>
          <CardDescription>
            清空全部练习进度与按日统计,并恢复练习偏好默认值;主题偏好保留。
          </CardDescription>
        </CardHeader>
        <CardContent>
          <Button variant="destructive" onClick={() => setConfirmOpen(true)}>
            <Trash2 aria-hidden />
            重置学习进度
          </Button>
        </CardContent>
      </Card>

      <Dialog open={confirmOpen} onOpenChange={setConfirmOpen}>
        <DialogContent>
          <DialogTitle>确认重置学习进度?</DialogTitle>
          <DialogDescription>
            将清空所有条目的掌握度、错题记录与按日统计,此操作无法撤销。
          </DialogDescription>
          <DialogFooter>
            <Button variant="outline" onClick={() => setConfirmOpen(false)}>
              取消
            </Button>
            <Button
              variant="destructive"
              onClick={() => {
                resetProgress();
                setConfirmOpen(false);
              }}
            >
              确认重置
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </div>
  );
}
