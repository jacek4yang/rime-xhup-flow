import { useMemo, useState } from "react";
import { Search } from "lucide-react";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import { OnScreenKeyboard, buildKeyLabels } from "@/components/OnScreenKeyboard";
import { useI18n } from "@/lib/use-i18n";
import type { I18nKey } from "@/lib/i18n";
import { useTrainerIndex } from "@/lib/trainer-context";

/**
 * 键位参考:键盘图 + 映射表 + 零声母 + 编码结构说明。
 * 所有数据来自生成的 trainer JSON,前端不维护任何映射副本。
 */
export function ReferenceView() {
  const index = useTrainerIndex();
  const { t } = useI18n();
  const [query, setQuery] = useState("");
  const { doublePinyin } = index.dataset;
  const keyLabels = buildKeyLabels(doublePinyin);
  const keys = [..."qwertyuiop", ..."asdfghjkl", ..."zxcvbnm"].sort();

  return (
    <div className="flex flex-col gap-4">
      <header>
        <h1 className="text-xl font-semibold">{t("nav.reference")}</h1>
        <p className="mt-1 text-sm text-muted-foreground">
          {t("reference.subtitle")}
        </p>
      </header>

      <Card>
        <CardHeader>
          <CardTitle>{t("reference.keyboardView")}</CardTitle>
        </CardHeader>
        <CardContent>
          <OnScreenKeyboard reference={doublePinyin} />
        </CardContent>
      </Card>

      <Card>
        <CardHeader>
          <CardTitle>{t("reference.search")}</CardTitle>
          <CardDescription>{t("reference.searchHint")}</CardDescription>
        </CardHeader>
        <CardContent className="flex flex-col gap-3">
          <label className="flex items-center gap-2 rounded-md border border-border px-3 focus-within:outline-2 focus-within:outline-ring">
            <Search className="size-4 text-muted-foreground" aria-hidden />
            <input
              value={query}
              onChange={(event) => setQuery(event.target.value)}
              placeholder={t("reference.search")}
              aria-label={t("reference.search")}
              className="h-11 w-full bg-transparent text-sm outline-none"
            />
          </label>
          <SearchResults query={query} />
        </CardContent>
      </Card>

      <Card>
        <CardHeader>
          <CardTitle>{t("reference.mapping")}</CardTitle>
        </CardHeader>
        <CardContent>
          <div className="grid grid-cols-2 gap-2 sm:grid-cols-3 lg:grid-cols-4">
            {keys.map((key) => {
              const label = keyLabels.get(key);
              return (
                <div
                  key={key}
                  className="flex items-baseline gap-2 rounded-lg border border-border px-3 py-2"
                >
                  <span className="font-mono text-base font-semibold uppercase">
                    {key}
                  </span>
                  <span className="text-sm text-muted-foreground">
                    {[label?.initials.join(" "), label?.finals.join(" ")]
                      .filter(Boolean)
                      .join(" · ") || "—"}
                  </span>
                </div>
              );
            })}
          </div>
        </CardContent>
      </Card>

      <Card>
        <CardHeader>
          <CardTitle>{t("reference.zeroInitials")}</CardTitle>
          <CardDescription>{t("reference.zeroInitialsHint")}</CardDescription>
        </CardHeader>
        <CardContent>
          <div className="grid grid-cols-3 gap-2 sm:grid-cols-4 lg:grid-cols-6">
            {doublePinyin.zeroInitials.map(({ syllable, code }) => (
              <div
                key={syllable}
                className="flex items-baseline justify-between rounded-lg border border-border px-3 py-2"
              >
                <span className="font-mono text-sm">{syllable}</span>
                <span className="font-mono text-sm font-semibold text-primary">
                  {code}
                </span>
              </div>
            ))}
          </div>
        </CardContent>
      </Card>

      <Card>
        <CardHeader>
          <CardTitle>{t("reference.codeStructure")}</CardTitle>
        </CardHeader>
        <CardContent>
          <ul className="flex flex-col gap-2 text-sm text-muted-foreground">
            <li>
              <span className="font-medium text-foreground">
                {t("reference.code2")}
              </span>
              {t("reference.code2Desc")}
            </li>
            <li>
              <span className="font-medium text-foreground">
                {t("reference.code3")}
              </span>
              {t("reference.code3Desc")}
            </li>
            <li>
              <span className="font-medium text-foreground">
                {t("reference.code4")}
              </span>
              {t("reference.code4Desc")}
            </li>
          </ul>
        </CardContent>
      </Card>
    </div>
  );
}

/** 规范数据速查:汉字/词/码前缀匹配,最多 24 条(只读)。 */
function SearchResults({ query }: { query: string }) {
  const index = useTrainerIndex();
  const { t } = useI18n();
  const results = useMemo(() => {
    const q = query.trim();
    if (q.length === 0) return [];
    const matches: { target: string; code: string; kind: string }[] = [];
    for (const item of index.byId.values()) {
      if (
        item.target.includes(q) ||
        item.primaryCode.startsWith(q.toLowerCase())
      ) {
        matches.push({
          target: item.target,
          code: item.primaryCode,
          kind: item.kind,
        });
        if (matches.length >= 24) break;
      }
    }
    return matches;
  }, [index, query]);

  const KIND_LABELS: Record<string, I18nKey> = {
    char: "reference.kindChar",
    level1: "reference.kindLevel1",
    word: "reference.kindWord",
    shortcut: "reference.kindShortcut",
    sentence: "reference.kindSentence",
  };

  if (query.trim().length === 0) return null;
  if (results.length === 0) {
    return <p className="text-sm text-muted-foreground">{t("reference.noResult")}</p>;
  }
  return (
    <ul className="grid grid-cols-1 gap-2 sm:grid-cols-2">
      {results.map((result) => (
        <li
          key={`${result.kind}:${result.target}:${result.code}`}
          className="flex items-baseline gap-2 rounded-lg border border-border px-3 py-2"
        >
          <span className="text-lg font-medium">{result.target}</span>
          <span className="font-mono text-sm text-muted-foreground">
            {result.code}
          </span>
          <span className="ml-auto text-xs text-muted-foreground">
            {KIND_LABELS[result.kind] ? t(KIND_LABELS[result.kind]) : result.kind}
          </span>
        </li>
      ))}
    </ul>
  );
}
