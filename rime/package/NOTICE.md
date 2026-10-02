# XHUP Flow 来源、改动与许可

本包不是小鹤官方产品。完整版本固定值、来源 URL、用途和许可分类随
`xhup_flow.sources.tsv` 分发；项目源码与构建记录位于
https://github.com/jacek4yang/rime-xhup-flow 。

下方文件路径指便携 Rime ZIP；Trainer 安装包将相同字节的许可按来源放在
`licenses/<来源>/` 子目录，保留上游文件名，避免 Windows MSI 同名文件冲突。

- 项目代码、方案配置和兼容字符编码来源：XHUP Flow 与
  https://github.com/boomker/rime-fast-xhup ，LGPL-3.0。
  项目进行了规范化、编码/权重编译、简码选择、组句和 Lua 行为调整，
  不是未经修改的上游词典。许可证见 `licenses/project-LGPL-3.0.txt`，
  其引用的 GPL v3 全文见 `licenses/GPL-3.0.txt`
  （https://www.gnu.org/licenses/gpl-3.0.html）。
- 万象单字频率、hot/extended 词语与派生简码：
  https://github.com/amzxyz/rime-wanxiang ，语义上游
  https://github.com/amzxyz/RIME-LMDG 。CC BY 4.0，
  https://creativecommons.org/licenses/by/4.0/ ，全文见
  `licenses/wanxiang-CC-BY-4.0.txt`。
  经过读音规范化、过滤、分层、码表投影及简码选择；不因编译改授 LGPL。
  单字来源提交 `7ec998b28c9a5c57260d2ba24b264c1c1820e0ef`；
  词语来源提交 `4618d67a978ff4f41b165c10b35558d38e333ab1`。
- 规范读音来自 https://github.com/mozillazg/pinyin-data ，
  提交 `923b108dc5d45dee061324c011b478fb649f8b73`，
  Copyright (c) 2016 mozillazg，MIT；全文与版权声明见
  `licenses/pinyin-data-MIT.txt`。项目合并规范读音并转换声调/ü 等表示。
- 简码选择中的离线会话统计来源：
  https://github.com/thu-coai/KdConv
  (`653db76432de09a004ba708a68f8bbd5500e6bec`) 与
  https://github.com/zake7749/Gossiping-Chinese-Corpus
  (`65b7e3630a560223a2b4d702d78d120d5ff1e8dd`)。
  聚合统计影响离线选择，不是运行时联网服务；原始对话不打入本包。
  保留 Apache-2.0 文本于 `licenses/kdconv-Apache-2.0.txt` 与
  `licenses/ptt-Apache-2.0.txt`。
- 小鹤官网 https://flypy.cc/ 仅作为少量编码事实核验入口。
  不分发官网整库或图像，不把品牌、数据库或未声明许可的材料改授项目许可。
- clean-v1 不包含没有再分发授权的搜狗词库、分类器衍生词条及原始抓取。
  来源登记中的 research-only 行只是排除说明，不是授权或词库内容。
  历史仓库记录不构成本包的再分发依据。

保留本说明、来源登记和许可文本。以上明确数据来源和项目改动，
不是对所有第三方权利的法律保证；二进制应用还另有依赖许可说明。
