import { useState } from "react";
import { beforeEach, expect, it } from "vitest";
import { fireEvent, render, screen } from "@testing-library/react";
import { emptyProgress, type TrainingItem } from "@xhup/trainer-core";
import { makeEntry, makeIndex } from "@xhup/trainer-core/testing";
import { TrainerIndexProvider } from "@/lib/trainer-context";
import { resetTrainerStore, useTrainerStore } from "@/stores/trainer-store";
import { PracticeView } from "../practice/PracticeView";
import { WeaknessCenter } from "./ReviewView";

const index = makeIndex({ entries: [makeEntry("去", "quts")] });
function Flow() {
  const [entries, setEntries] = useState<TrainingItem[] | null>(null);
  return <TrainerIndexProvider index={index}>
    {entries === null ? <WeaknessCenter onPracticeEntries={setEntries} /> :
      <PracticeView config={{ mode: "mixed", difficulty: "daily", targetLength: 1, entries }}
        onExit={() => setEntries(null)} onRestart={() => {}} onPracticeEntries={setEntries} onExitToToday={() => {}} />}
  </TrainerIndexProvider>;
}

beforeEach(() => { localStorage.clear(); resetTrainerStore(); });

it.each(["level1", "word", "sentence"] as const)("零错误的到期 %s 可从复习入口实际完成", (kind) => {
  const item = [...index.byId.values()].find((entry) => entry.kind === kind &&
    (kind !== "word" || entry.codeLength === 6))!;
  useTrainerStore.setState({ progress: { [item.id]: { ...emptyProgress(), attempts: 4, correct: 4, mastery: 100 } } });
  render(<Flow />);
  expect(screen.getByText(item.target)).toBeVisible();
  fireEvent.click(screen.getByRole("button", { name: /练这些/ }));
  const input = screen.getByLabelText("编码输入区");
  for (const key of item.primaryCode) fireEvent.keyDown(input, { key });
  fireEvent.keyDown(input, { key: " " });
  const progress = useTrainerStore.getState().progress[item.id]!;
  expect(progress.attempts).toBe(5);
  expect(progress.review).toMatchObject({ level: 1, lastResult: "recalled", routeUsed: "primary", usedAnswerHint: false });
  expect(progress.review!.dueAt).toBeGreaterThan(Date.now());
});
