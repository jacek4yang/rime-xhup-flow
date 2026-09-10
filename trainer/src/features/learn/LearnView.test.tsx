/**
 * 学习中心视图测试:章节导航、内容渲染、练习入口与形码探索器。
 */

import { beforeEach, describe, expect, it, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import type { TrainerDataset } from "@xhup/trainer-core";
import { buildTrainerIndex, LEARN_CHAPTERS } from "@xhup/trainer-core";
import { TrainerIndexProvider } from "@/lib/trainer-context";
import { resetTrainerStore } from "@/stores/trainer-store";
import { LearnView } from "./LearnView";

beforeEach(() => {
  resetTrainerStore();
});

const DATASET: TrainerDataset = {
  schemaVersion: 4,
  packageVersion: "0.1.0",
  entries: [
    { char: "阿", code: "aaed", length: 4, readings: ["a"], frequencyScore: 100, rimeWeight: 1, scope: "core", codeSource: "canonical-reading-shape", sources: [], statuses: [] },
    { char: "低", code: "dped", length: 4, readings: ["di"], frequencyScore: 90, rimeWeight: 1, scope: "core", codeSource: "canonical-reading-shape", sources: [], statuses: [] },
    { char: "行", code: "xk", length: 2, readings: ["xing"], frequencyScore: 80, rimeWeight: 1, scope: "core", codeSource: "canonical-reading-shape", sources: [], statuses: [] },
  ],
  words: [],
  level1Shortcuts: [],
  primaryShortcuts: [],
  fixedFirstShortcuts: [],
  sentences: [],
  doublePinyin: { initials: [], finals: [], zeroInitials: [] },
};

function renderLearn(onStartPractice = vi.fn()) {
  const index = buildTrainerIndex(DATASET);
  render(
    <TrainerIndexProvider index={index}>
      <LearnView onStartPractice={onStartPractice} />
    </TrainerIndexProvider>,
  );
  return { onStartPractice };
}

describe("LearnView", () => {
  it("渲染章节目录与首章内容", async () => {
    renderLearn();
    expect(screen.getByText("小鹤音形是什么")).toBeInTheDocument();
    expect(screen.getByText(/音码层就是双拼/)).toBeInTheDocument();
    expect(screen.getByRole("navigation", { name: "学习" })).toBeInTheDocument();
  });

  it("章节间导航:上一章/下一章", async () => {
    const user = userEvent.setup();
    renderLearn();
    await user.click(screen.getByRole("button", { name: /下一章/ }));
    expect(screen.getByText("双拼:两键一个音")).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: /上一章/ }));
    expect(screen.getByText("小鹤音形是什么")).toBeInTheDocument();
  });

  it("练习入口:点击后以对应模式与章节开始练习", async () => {
    const user = userEvent.setup();
    const { onStartPractice } = renderLearn();
    // 进入双拼章节。
    await user.click(screen.getByRole("button", { name: /2\. 双拼/ }));
    const cta = await screen.findByRole("button", { name: /^双拼$/ });
    await user.click(cta);
    expect(onStartPractice).toHaveBeenCalledWith("double", "double");
  });

  it("章节建议状态徽标:打开过的章节显示「学习中」,其余「未开始」", async () => {
    const user = userEvent.setup();
    renderLearn();
    // 首章挂载即记录打开证据 → 学习中;其余章节未开始。
    expect(screen.getAllByText("学习中").length).toBeGreaterThan(0);
    expect(screen.getAllByText("未开始").length).toBe(LEARN_CHAPTERS.length - 1);
    // 建议性标签:切换章节后徽标随之渲染,但不拦截任何内容。
    await user.click(screen.getByRole("button", { name: /3\. 形码/ }));
    expect(screen.getByText("形码:两键定形")).toBeInTheDocument();
  });

  it("形码探索器:展示形键标签与高频例字", async () => {
    renderLearn();
    await userEvent.click(screen.getByRole("button", { name: /4\. 字形记忆方法/ }));
    expect(screen.getByText("形码探索器(数据驱动)")).toBeInTheDocument();
    // 数据里有 d 组(阿/低 的次形),点选后展示例字。
    await screen.findByRole("tab", { name: /D/ });
    expect(screen.getByText("阿")).toBeInTheDocument();
  });
});
