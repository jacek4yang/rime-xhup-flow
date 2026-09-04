# 教育元数据(pinyin_tone.tsv)

带调普通话读音子集,供训练器的错误教学卡片展示「读音 + 声调」。
本目录数据是**教育元数据**,不属于 XHUP 编码契约;缺失时展示层必须
回退为无调读音,不得猜测声调。

## 来源与许可

| 项 | 值 |
| --- | --- |
| 来源 | Unicode Character Database,`Unihan_Readings.txt` 的 `kMandarin` 字段 |
| 版本 | Unicode 17.0.0(文件头 `Date: 2025-07-24`) |
| 下载 | https://www.unicode.org/Public/UCD/latest/ucd/Unihan.zip |
| 下载 SHA256 | `f7a48b2b545acfaa77b2d607ae28747404ce02baefee16396c5d2d7a8ef34b5e` |
| 许可 | Unicode License Agreement - Data Files and Software (v3);允许再分发 |
| 导入 | `node data/education/import_unihan_tone.mjs <Unihan_Readings.txt> data/hanzi/readings.tsv data/education/pinyin_tone.tsv` |

## 子集规则(确定性)

- 字符集 = 仓库规范读音表 `data/hanzi/readings.tsv` 的全部字符(8105 字)。
- 每字取 `kMandarin` 第一个读音(Unicode 的 zh-Hans 默认读音,含调号)。
- 输出按规范读音表顺序排列;TSV 两列:`字\t带调拼音`。
- 覆盖率 100%(2026-09-05 导入,Unihan 17.0.0)。

## 已知限制

- 多音字只含规范默认读音;教学卡片展示时不得声称「该字只有这个读音」。
- 声调与 XHUP 编码无关:编码不含调,本子集仅用于讲解读音。
