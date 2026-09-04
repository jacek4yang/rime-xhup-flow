/**
 * 学习中心视图测试:章节导航、内容渲染、练习入口与形码探索器。
 */

import { describe, expect, it, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import type { TrainerDataset } from "@/lib/trainer-data";
import { buildTrainerIndex } from "@/lib/trainer-index";
import { TrainerIndexProvider } from "@/lib/trainer-context";
import { LearnView } from "./LearnView";

const DATASET: TrainerDataset = {
  schemaVersion: 2,
  packageVersion: "0.1.0",
  entries: [
    { char: "阿", code: "aaed", length: 4, readings: ["a"], frequencyScore: 100, rimeWeight: 1 },
    { char: "低", code: "dped", length: 4, readings: ["di"], frequencyScore: 90, rimeWeight: 1 },
    { char: "行", code: "xk", length: 2, readings: ["xing"], frequencyScore: 80, rimeWeight: 1 },
  ],
  words: [],
  level1Shortcuts: [],
  wordShortcuts: [],
  fixedFirstShortcuts: [],
  twoKeyShortcuts: [],
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

  it("练习入口:点击后以对应模式开始练习", async () => {
    const user = userEvent.setup();
    const { onStartPractice } = renderLearn();
    // 进入双拼章节。
    await user.click(screen.getByRole("button", { name: /2\. 双拼/ }));
    const cta = await screen.findByRole("button", { name: /^双拼$/ });
    await user.click(cta);
    expect(onStartPractice).toHaveBeenCalledWith("double");
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
