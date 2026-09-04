/**
 * 学习中心章节内容:小鹤音形从入门到精通的结构化课程。
 *
 * 内容为规范中文(canonical;与键位/方案数据一致,不做英文翻译)。
 * 事实性内容全部以仓库规范数据为依据:
 * - 单字全码 = 声母 + 韵母 + 首形 + 次形(4 码);3 码 = 音 + 首形;
 * - 形键分布、双拼键位、简码层均可在训练数据/键位页中核对;
 * - 本章不引入数据之外的任何字根表或口诀,避免以讹传讹。
 */

import type { I18nKey } from "@/lib/i18n";
import type { PracticeMode } from "@/features/practice/types";

/** 一段章节内容(按顺序渲染)。 */
export type LearnSection =
  | { kind: "text"; heading?: string; paragraphs: readonly string[] }
  | { kind: "list"; heading: string; ordered?: boolean; items: readonly string[] }
  | { kind: "practice"; heading: string; mode: PracticeMode; label: I18nKey }
  | { kind: "shape-explorer"; heading: string };

export type LearnChapterLevel = "beginner" | "basic" | "intermediate" | "advanced" | "mastery";

export type LearnChapter = {
  id: string;
  level: LearnChapterLevel;
  /** 章节标题(规范中文)。 */
  title: string;
  /** 一句话摘要。 */
  summary: string;
  sections: readonly LearnSection[];
};

export const LEVEL_LABELS: Record<LearnChapterLevel, string> = {
  beginner: "入门",
  basic: "基础",
  intermediate: "进阶",
  advanced: "高级",
  mastery: "精通",
};

export const LEARN_CHAPTERS: readonly LearnChapter[] = [
  {
    id: "overview",
    level: "beginner",
    title: "小鹤音形是什么",
    summary: "音码定读音,形码定字形,四码以内选准一个字。",
    sections: [
      {
        kind: "text",
        paragraphs: [
          "小鹤音形(简称鹤形)是一套「音形结合」的汉字输入方案:用两个键打出读音(双拼),再用两个键区分同音字(形码)。全部常用单字最多四键必出,不需要翻页找字,也不需要死记整句词库。",
          "它由两层组成:音码层就是双拼——每个汉字的读音压缩成「声母一键 + 韵母一键」;形码层在音码之上,用一至两键按字形特征区分同音字。三码可出常用字(音 + 首形),四码覆盖全部规范单字(音 + 首形 + 次形)。",
          "本仓库在经典方案之上提供了两种使用方式:流畅模式(Flow)在冻结的静态编码之上支持连续组句与本地学习,越用越顺手;固定模式(Static)全程确定性固定编码,结果完全可预期,适合追求稳定的场景。两套方案可以随时在输入法的方案选单中切换。",
        ],
      },
      {
        kind: "list",
        heading: "从入门到精通的路径",
        ordered: true,
        items: [
          "入门:掌握双拼,做到「两键一个音」条件反射(第 2 章)。",
          "基础:理解形码规则,学会字形记忆方法(第 3、4 章)。",
          "进阶:打熟四码全码,记住一级简码(第 5 章)。",
          "高级:词语简码与整句组句,开始长句输入(第 6、7 章)。",
          "精通:全模式综合练习,用统计面板追踪 KPM 与准确率(第 8 章)。",
        ],
      },
      {
        kind: "text",
        paragraphs: [
          "本训练器为这条路径的每一站都准备了对应的练习模式:每个章节末尾都有「去练习」入口,读完全章立刻上手,学习数据保存在本机。",
        ],
      },
    ],
  },
  {
    id: "double",
    level: "beginner",
    title: "双拼:两键一个音",
    summary: "声母一键、韵母一键;零声母音节有固定规则。",
    sections: [
      {
        kind: "text",
        paragraphs: [
          "全拼里「zhuang」要敲六个字母,双拼把它压缩成两键:第一键是声母,第二键是韵母。每个韵母(含复韵母,如 uang、ian)都固定映射在一个字母键上,例如 zhuang = zh(z) + uang(l) 两键。映射表是方案的规范约定,在「键位」页可以随时查看完整对照。",
          "零声母音节(没有声母的音节,如 a、ai、an、ang、e、ou)不是无声可打:它们用固定的零声母键规则,同样是两键。例如「阿」读 a,全码前两键就是 aa。零声母对照同样收录在键位页,不用背整张表——先记住几个高频字(啊、恩、哦),其余在练习里自然会遇到。",
          "练习双拼时不要试图「背表」:正确的方法是拿高频字练——看到「装」想起 zl,看到「行」想起 xk。打错的字会自动进入错题中心,复习一轮就能覆盖日常 90% 的读音。",
        ],
      },
      {
        kind: "practice",
        heading: "双拼练习",
        mode: "double",
        label: "practice.modeDouble",
      },
      {
        kind: "text",
        paragraphs: [
          "达标参考:双拼模式连续 30 题、准确率 98% 以上、不看键盘,即可进入下一站。速度此时不必追求,KPM(每分钟键数)会在后面自然涨上来。",
        ],
      },
    ],
  },
  {
    id: "shape",
    level: "basic",
    title: "形码:两键定形",
    summary: "首形与次形两键,把同音字按字形特征区分开。",
    sections: [
      {
        kind: "text",
        paragraphs: [
          "双拼打完两键,同音字还有一长串(「zhi」下的字数以千计)。形码的任务就是在音码之上做区分:取字的第一个形键(首形)和最后一个形键(次形),各占一键。三码 = 音 + 首形,能打出当组里最有区分度的常用字;四码 = 音 + 首形 + 次形,规范单字全覆盖、无重码翻页。",
          "形键分布在整个字母键盘上,每个键代表一类字形特征;具体哪个部首/笔画组落在哪个键,以本仓库的规范数据为准——练习器中的全码数据(例如「阿」= aaed,首形 e、次形 d)就是权威来源,第 4 章的形码探索器可以把任意形键下的常见字直接列出来。",
          "一个重要的心态:形码不是五笔那种「拆字输入」。你始终先打读音,形码只是在同音字里「指认」你要的那个字。所以形码的学习负担远低于纯形码方案——常用 1500 字覆盖日常输入 95% 以上,把这一两千字的首形练熟,输入就已经很流畅了。",
        ],
      },
      {
        kind: "practice",
        heading: "音形(3 码)练习",
        mode: "sound-shape",
        label: "practice.modeSoundShape",
      },
    ],
  },
  {
    id: "shape-memory",
    level: "basic",
    title: "字形记忆方法",
    summary: "以字带根、分组归纳、错题回流——把形码练成下意识。",
    sections: [
      {
        kind: "text",
        paragraphs: [
          "形码记忆的核心不是背表,而是「以字带根」:记住一批高频字在什么形键上,遇到生字时按字形特征类推。下面的探索器把规范数据按形键聚合——点任意键,就能看到以它为首形/次形的高频字实例。每天选三个键,把例字打十遍,两周即可覆盖常用字。",
        ],
      },
      {
        kind: "shape-explorer",
        heading: "形码探索器(数据驱动)",
      },
      {
        kind: "list",
        heading: "五个经过验证的记忆策略",
        ordered: true,
        items: [
          "先音后形:先练双拼到条件反射,再加首形,最后加次形——一次只增加一个记忆变量。",
          "以字带根:用「啊-e」「地-d」这类你已经在打的高频字记住形键,而不是孤立背键位表。",
          "分组归纳:把同一形键下的例字放在一起观察(探索器就是为此设计的),字形特征的规律会自己浮现。",
          "错题回流:答错的字自动进入错题中心,当天清空错题,比刷新题有效得多。",
          "短时多次:每天 2-3 组、每组 20 题,优于一周一次长练——肌肉记忆靠的是频率而非时长。",
        ],
      },
      {
        kind: "practice",
        heading: "综合单字(2/3/4 码轮换)练习",
        mode: "mixed",
        label: "practice.modeMixed",
      },
    ],
  },
  {
    id: "fullcode",
    level: "intermediate",
    title: "全码与一级简码",
    summary: "四码必出、一键直达——确定性是这个方案的底座。",
    sections: [
      {
        kind: "text",
        paragraphs: [
          "全码(4 码)是整个体系的确定性地基:音 2 码 + 形 2 码,规范单字无翻页。本仓库的静态词典约 26753 条单字训练数据、超过 13 万个全码条目,全部经过冻结审计——你今天练的编码和明年输入法里的编码完全一致,肌肉记忆永不作废。",
          "一级简码是最高频的 25 个字,各占一个字母键,一键上屏(例如 q = 去)。它们是日常输入里性价比最高的记忆投资:占比极小的键位,承担了极大的输入频次。",
          "一级简码的备用合法码是该字的全码——忘了简码也不用心慌,四码照样出字。练习器把两条路线都接受,但简码路线才算完美,以此激励你用最短路径。",
        ],
      },
      {
        kind: "practice",
        heading: "全码(4 码)练习",
        mode: "full",
        label: "practice.modeFull",
      },
      {
        kind: "practice",
        heading: "一级简码练习",
        mode: "level1",
        label: "practice.modeLevel1",
      },
    ],
  },
  {
    id: "words",
    level: "advanced",
    title: "词语编码与多级简码",
    summary: "逐字双拼出全码,三个生产简码层把常用词压到两三键。",
    sections: [
      {
        kind: "text",
        paragraphs: [
          "词语的全码是逐字双拼的拼接:二字词 4 键(如「时间」= uijm),三字词 6 键,四字词 8 键。词库随方案一起安装、一起冻结,不需要联网更新。",
          "在词全码之上,本方案维护了三个生产简码层:两键零冲突层(两键无歧义直出)、ZERO_REGRESSION 层(保证不引起任何静态候选回归的前提下缩短)与 FIXED_FIRST 层(高稳健首字固定)。练习器对简码条目同样接受「简码 / 全码」双路线。",
          "建议的练习顺序:先打熟固定词全码建立「词感」,再练两键词简码,最后把三个简码层综合轮换——简码综合模式会自动在层间轮转。",
        ],
      },
      {
        kind: "practice",
        heading: "固定词全码练习",
        mode: "fixed-word",
        label: "practice.modeFixedWord",
      },
      {
        kind: "practice",
        heading: "简码综合(三层轮换)练习",
        mode: "mixed-shortcut",
        label: "practice.modeMixedShortcut",
      },
    ],
  },
  {
    id: "sentence",
    level: "advanced",
    title: "组句与本地学习",
    summary: "静态候选永远优先;动态学习只补静态之不足。",
    sections: [
      {
        kind: "text",
        paragraphs: [
          "流畅模式(Flow)的核心能力是连续组句:整句按字词拼接静态码,一次性输入;引擎按「静态 > 动态」的优先级出候选——所有冻结的静态编码(全码、简码、固定词)永远排在前面,动态学习产生的候选只补静态覆盖不到的地方。这意味着学习不会污染你已经练成的确定性编码。",
          "学习数据保存在本机的 xhup_flow_user.userdb:没有账号、没有遥测、没有云端同步。你可以在控制中心随时导出快照、换机恢复,或用类型化确认重置。",
          "组句练习的模式是「按词组打整句」:每个分段有自己的编码,分段提示会在练习里展示。建议从 4-6 字短句开始,追求「一次拼对」而不是回改。",
        ],
      },
      {
        kind: "practice",
        heading: "组句练习",
        mode: "sentence",
        label: "practice.modeSentence",
      },
    ],
  },
  {
    id: "mastery",
    level: "mastery",
    title: "精通:全模式与自我度量",
    summary: "用综合模式和统计面板,把正确率与速度一起推进。",
    sections: [
      {
        kind: "text",
        paragraphs: [
          "精通阶段的关键是把所有模式混起来练:全模式综合会跨池轮换(单字、简码、词、句),模拟真实输入的随机性。练习时打开实时统计——连对数反映稳定性,准确率反映编码质量,KPM 与 CPM 反映速度,「键/字」反映你是否真的用上了简码(熟练者的键/字会明显低于 3)。",
          "推荐的日常节奏:5 分钟综合热身 → 错题中心清空昨日错题 → 10 分钟专项(本周最弱的层)→ 5 分钟组句收尾。统计页的每日练习时长会帮你保持节奏。",
          "最后一条也是最重要的一条:输入法是用来「用」的,不是用来「练」的。当练习数据说你已经达标,就把日常输入全部切到小鹤音形——真实文本才是最终的训练场。遇到卡壳的字,回来在错题中心补上即可。",
        ],
      },
      {
        kind: "practice",
        heading: "全模式综合练习",
        mode: "mixed-all",
        label: "practice.modeMixedAll",
      },
    ],
  },
];

/** 校验章节结构的完整性(测试守卫;运行时数据为静态常量)。 */
export function validateChapters(chapters: readonly LearnChapter[]): string[] {
  const problems: string[] = [];
  const ids = new Set<string>();
  for (const chapter of chapters) {
    if (ids.has(chapter.id)) problems.push(`章节 id 重复: ${chapter.id}`);
    ids.add(chapter.id);
    if (chapter.sections.length === 0) problems.push(`章节无内容: ${chapter.id}`);
    let hasText = false;
    for (const section of chapter.sections) {
      if (section.kind === "text" && section.paragraphs.length > 0) hasText = true;
      if (section.kind === "list" && section.items.length === 0) {
        problems.push(`空列表: ${chapter.id}`);
      }
    }
    if (!hasText) problems.push(`章节缺少正文: ${chapter.id}`);
  }
  return problems;
}
