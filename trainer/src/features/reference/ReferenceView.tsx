import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import { OnScreenKeyboard, buildKeyLabels } from "@/components/OnScreenKeyboard";
import { useTrainerIndex } from "@/lib/trainer-context";

/**
 * 键位参考:键盘图 + 映射表 + 零声母 + 编码结构说明。
 * 所有数据来自生成的 trainer JSON,前端不维护任何映射副本。
 */
export function ReferenceView() {
  const index = useTrainerIndex();
  const { doublePinyin } = index.dataset;
  const keyLabels = buildKeyLabels(doublePinyin);
  const keys = [..."qwertyuiop", ..."asdfghjkl", ..."zxcvbnm"].sort();

  return (
    <div className="flex flex-col gap-4">
      <header>
        <h1 className="text-xl font-semibold">键位</h1>
        <p className="mt-1 text-sm text-muted-foreground">
          小鹤双拼的声母、韵母键位分布,数据来自规范码表。
        </p>
      </header>

      <Card>
        <CardHeader>
          <CardTitle>键盘视图</CardTitle>
        </CardHeader>
        <CardContent>
          <OnScreenKeyboard reference={doublePinyin} />
        </CardContent>
      </Card>

      <Card>
        <CardHeader>
          <CardTitle>映射表</CardTitle>
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
          <CardTitle>零声母</CardTitle>
          <CardDescription>
            没有声母的音节,按下面的规则补全两位音码。
          </CardDescription>
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
          <CardTitle>编码结构</CardTitle>
        </CardHeader>
        <CardContent>
          <ul className="flex flex-col gap-2 text-sm text-muted-foreground">
            <li>
              <span className="font-medium text-foreground">2 码(双拼)</span>
              :两位音码,声母 + 韵母各一键。
            </li>
            <li>
              <span className="font-medium text-foreground">3 码(音形)</span>
              :双拼音码 + 第一个形码。
            </li>
            <li>
              <span className="font-medium text-foreground">4 码(全码)</span>
              :双拼音码 + 两位形码,是最终的全码。
            </li>
          </ul>
        </CardContent>
      </Card>
    </div>
  );
}
