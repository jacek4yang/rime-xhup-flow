import { useState } from "react";
import { beforeEach, expect, it } from "vitest";
import { fireEvent, render, screen } from "@testing-library/react";
import {
  applyQuestionProgress,
  emptyProgress,
  type RecallEvidence,
  type TrainingItem,
} from "@xhup/trainer-core";
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

const answer: RecallEvidence = {
  outcome: "perfect", routeUsed: "primary", independentRecall: true,
  usedAnswerHint: false, wrongKeyEvents: 0, questionMs: 1000, chars: 1, now: 0,
};

it("复习行解释未升级原因与下次到期时间", () => {
  const now = Date.now();
  const item = [...index.byId.values()].find((entry) => entry.kind === "level1")!;
  const first = applyQuestionProgress(emptyProgress(), { ...answer, now });
  // 早失败一次(等级归零)→ 到期重试成功(等级 1,明天到期)→ 行内展示
  // 「提前重复不升级」的原因与「X后到期」倒计时。
  const lapsed = applyQuestionProgress(first, { ...answer, usedAnswerHint: true, now: now + 1000 });
  const recovered = applyQuestionProgress(lapsed, { ...answer, now: now + 11 * 60_000 });
  useTrainerStore.setState({ progress: { [item.id]: recovered } });
  render(<TrainerIndexProvider index={index}><WeaknessCenter onPracticeEntries={() => {}} /></TrainerIndexProvider>);
  expect(screen.getAllByText(item.target).length).toBeGreaterThan(0);
  expect(screen.getByText(/提前重复不升级/)).toBeVisible();
  expect(screen.getByText(/后到期/)).toBeVisible();
});

it("失败条目明确说明等级已重置与重试等待", () => {
  const now = Date.now();
  const item = [...index.byId.values()].find((entry) => entry.kind === "level1")!;
  const first = applyQuestionProgress(emptyProgress(), { ...answer, now });
  const lapsed = applyQuestionProgress(first, { ...answer, usedAnswerHint: true, now: now + 1000 });
  useTrainerStore.setState({ progress: { [item.id]: lapsed } });
  render(<TrainerIndexProvider index={index}><WeaknessCenter onPracticeEntries={() => {}} /></TrainerIndexProvider>);
  expect(screen.getByText(/等级已重置/)).toBeVisible();
  expect(screen.getByText(/后到期/)).toBeVisible();
});
