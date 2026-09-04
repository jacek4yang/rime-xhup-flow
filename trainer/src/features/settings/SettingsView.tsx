import { useRef, useState } from "react";
import { Download, Info, Trash2, Upload } from "lucide-react";
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
import { exportBackup, importBackup, BackupError } from "@/lib/backup";
import { LANGUAGE_LABELS, LANGUAGES } from "@/lib/i18n";
import { useI18n } from "@/lib/use-i18n";
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
const HINT_MODES: HintMode[] = ["always", "on-delay", "on-error", "hidden"];
const DIFFICULTIES: Difficulty[] = ["beginner", "daily", "full"];

export function SettingsView({ onRerunOnboarding }: { onRerunOnboarding?: () => void }) {
  const index = useTrainerIndex();
  const theme = useTrainerStore((state) => state.theme);
  const hintMode = useTrainerStore((state) => state.hintMode);
  const difficulty = useTrainerStore((state) => state.difficulty);
  const sessionLength = useTrainerStore((state) => state.sessionLength);
  const language = useTrainerStore((state) => state.language);
  const setLanguage = useTrainerStore((state) => state.setLanguage);
  const setTheme = useTrainerStore((state) => state.setTheme);
  const setHintMode = useTrainerStore((state) => state.setHintMode);
  const setDifficulty = useTrainerStore((state) => state.setDifficulty);
  const setSessionLength = useTrainerStore((state) => state.setSessionLength);
  const resetProgress = useTrainerStore((state) => state.resetProgress);
  const applyBackup = useTrainerStore((state) => state.applyBackup);
  const { t } = useI18n();
  const fileInputRef = useRef<HTMLInputElement>(null);
  const [confirmOpen, setConfirmOpen] = useState(false);
  const [notice, setNotice] = useState<string | null>(null);

  const handleExport = () => {
    const state = useTrainerStore.getState();
    const json = exportBackup(
      {
        language: state.language,
        theme: state.theme,
        hintMode: state.hintMode,
        difficulty: state.difficulty,
        sessionLength: state.sessionLength,
        lastMode: state.lastMode,
        progress: state.progress,
        daily: state.daily,
        keyErrors: state.keyErrors,
      },
      Date.now(),
    );
    const blob = new Blob([json], { type: "application/json" });
    const url = URL.createObjectURL(blob);
    const anchor = document.createElement("a");
    anchor.href = url;
    anchor.download = `xhup-flow-trainer-backup-${localDateStamp()}.json`;
    anchor.click();
    URL.revokeObjectURL(url);
  };

  const handleImportFile = async (file: File) => {
    try {
      const backup = importBackup(await file.text());
      applyBackup(backup);
      setNotice(t("settings.imported"));
    } catch (error) {
      const reason =
        error instanceof BackupError
          ? error.message
          : t("common.unknownError");
      setNotice(t("settings.importFailed", { reason }));
    }
  };

  return (
    <div className="flex max-w-2xl flex-col gap-4">
      <header>
        <h1 className="text-xl font-semibold">{t("nav.settings")}</h1>
      </header>

      <Card>
        <CardHeader>
          <CardTitle>{t("settings.language")} / {t("settings.theme")}</CardTitle>
        </CardHeader>
        <CardContent className="flex flex-col gap-4">
          <OptionRow label={t("settings.language")}>
            {LANGUAGES.map((candidate) => (
              <OptionButton
                key={candidate}
                selected={language === candidate}
                onClick={() => setLanguage(candidate)}
              >
                {LANGUAGE_LABELS[candidate]}
              </OptionButton>
            ))}
          </OptionRow>
          <OptionRow label={t("settings.theme")}>
            {THEMES.map((candidate) => (
              <OptionButton
                key={candidate}
                selected={theme === candidate}
                onClick={() => setTheme(candidate)}
              >
                {t(THEME_LABELS[candidate])}
              </OptionButton>
            ))}
          </OptionRow>
          <OptionRow label={t("practice.hint")}>
            {HINT_MODES.map((candidate) => (
              <OptionButton
                key={candidate}
                selected={hintMode === candidate}
                onClick={() => setHintMode(candidate)}
              >
                {t(HINT_MODE_LABELS[candidate])}
              </OptionButton>
            ))}
          </OptionRow>
          <OptionRow label={t("practice.difficulty")}>
            {DIFFICULTIES.map((candidate) => (
              <OptionButton
                key={candidate}
                selected={difficulty === candidate}
                onClick={() => setDifficulty(candidate)}
              >
                {t(DIFFICULTY_LABELS[candidate])}
              </OptionButton>
            ))}
          </OptionRow>
          <OptionRow label={t("practice.length")}>
            {SESSION_LENGTH_OPTIONS.map((candidate) => (
              <OptionButton
                key={candidate}
                selected={sessionLength === candidate}
                onClick={() => setSessionLength(candidate)}
              >
                {candidate === 0 ? t("practice.lengthUnlimited") : candidate}
              </OptionButton>
            ))}
          </OptionRow>
        </CardContent>
      </Card>

      <Card>
        <CardHeader>
          <CardTitle>{t("settings.data")}</CardTitle>
          <CardDescription>{t("settings.dataHint")}</CardDescription>
        </CardHeader>
        <CardContent className="flex flex-col gap-2 text-sm">
          <div className="flex justify-between">
            <span className="text-muted-foreground">{t("settings.dataVersion")}</span>
            <span className="font-mono">{index.dataset.packageVersion}</span>
          </div>
          <div className="flex justify-between">
            <span className="text-muted-foreground">{t("settings.dataSchema")}</span>
            <span className="font-mono">
              schemaVersion {index.dataset.schemaVersion}
            </span>
          </div>
          <div className="flex justify-between">
            <span className="text-muted-foreground">{t("settings.dataEntries")}</span>
            <span className="font-mono tabular-nums">
              {index.dataset.entries.length}
            </span>
          </div>
          <Separator className="my-2" />
          <div className="flex items-center justify-between gap-2">
            <span className="text-muted-foreground">{t("settings.onboarding")}</span>
            <Button
              variant="outline"
              className="min-h-11"
              onClick={onRerunOnboarding}
            >
              {t("settings.rerunOnboarding")}
            </Button>
          </div>
          <Separator className="my-2" />
          <p className="text-sm text-muted-foreground">
            {t("settings.privacyHint")}
          </p>
        </CardContent>
      </Card>

      <Card>
        <CardHeader>
          <CardTitle>{t("settings.backup")}</CardTitle>
          <CardDescription>{t("settings.backupHint")}</CardDescription>
        </CardHeader>
        <CardContent className="flex flex-col gap-2">
          <div className="flex flex-wrap gap-2">
            <Button variant="outline" onClick={handleExport}>
              <Download aria-hidden />
              {t("settings.export")}
            </Button>
            <Button
              variant="outline"
              onClick={() => fileInputRef.current?.click()}
            >
              <Upload aria-hidden />
              {t("settings.import")}
            </Button>
            <input
              ref={fileInputRef}
              type="file"
              accept="application/json,.json"
              className="hidden"
              onChange={(event) => {
                const file = event.target.files?.[0];
                if (file) void handleImportFile(file);
                event.target.value = "";
              }}
            />
          </div>
          {notice && <p className="text-sm text-muted-foreground">{notice}</p>}
        </CardContent>
      </Card>

      <Card>
        <CardHeader>
          <CardTitle>{t("settings.learning")}</CardTitle>
        </CardHeader>
        <CardContent className="flex items-start gap-2 text-sm text-muted-foreground">
          <Info className="mt-0.5 size-4 shrink-0" aria-hidden />
          <p>{t("settings.learningHint")}</p>
        </CardContent>
      </Card>

      <Card>
        <CardHeader>
          <CardTitle>{t("settings.reset")}</CardTitle>
          <CardDescription>
            {t("settings.resetHint")}
          </CardDescription>
        </CardHeader>
        <CardContent>
          <Button variant="destructive" onClick={() => setConfirmOpen(true)}>
            <Trash2 aria-hidden />
            {t("settings.reset")}
          </Button>
        </CardContent>
      </Card>

      <Dialog open={confirmOpen} onOpenChange={setConfirmOpen}>
        <DialogContent>
          <DialogTitle>{t("settings.reset")}</DialogTitle>
          <DialogDescription>{t("review.resetConfirm")}</DialogDescription>
          <DialogFooter>
            <Button variant="outline" onClick={() => setConfirmOpen(false)}>
              {t("common.cancel")}
            </Button>
            <Button
              variant="destructive"
              onClick={() => {
                resetProgress();
                setConfirmOpen(false);
              }}
            >
              {t("settings.reset")}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </div>
  );
}

/** 备份文件名用的本地日期戳(YYYYMMDD)。 */
function localDateStamp(): string {
  const now = new Date();
  const month = String(now.getMonth() + 1).padStart(2, "0");
  const day = String(now.getDate()).padStart(2, "0");
  return `${now.getFullYear()}${month}${day}`;
}
