# 外部文本原生评估协议（结果测量前固定）

本评估补充原生行为回归，不代替真实用户验收，不宣称“完美输入法”。
输入源为 Universal Dependencies Chinese-GSDSimp r2.16 的 test 文件，
固定提交、SHA256、作者、许可边界见 `tests/quality/ud-source.json`。
这是外部维基文本，不是中文会话的代表性随机样本；尚未证明与所有上游
词频/训练文本无重复，不能称严格训练隔离的 held-out 质量估计。

## 固定选择与输入

只使用源文件已有 Translit，不根据当前词典、候选名次或预期输出猜读音。
从 Rust 规范音节编码器导出 406 个音节双拼码；按汉字数约束解析拼音，
多解/缺失/不支持的 token 必须排除并计数，绝不选择有利于当前系统的一解。
保留所有连续合格汉字 token 串，长度 2–64 字；标点、混合文字、不支持的
token 切断串。每个原始句子的所有串保留 parent sentence 标识。

源文件留在本地缓存，不随生产包分发。UD 标注 CC BY-SA 4.0，
底层维基文本另有原作者归属/许可；本仓库只提交协议、工具、摘要和源哈希，
不将下载到的公开文本宣称为自行创作或无版权。

## 原生运行与消融

使用 source-byte gate 校验生成包中的 8 个 Lua 模块及 2 个方案；
隔离目录重新部署，验证编译后所有 native user dict 开关关闭；
关闭上下文调序、会话记忆、提示，不读取真实用户目录。

两组固定比较：planner 为生产边界规划；native-only 只通过测试包装器移除
规划器的 lookup memory，保留同一原生 provider、词典、过滤链及基础候选流。
包装器仅存在测试目录，不能进入生产包。实际编译后的 provider 必须与报告
模式相符。不得按这组测试结果调参后继续将它描述为未见过的测试集。

每键记录原始输入、preedit、前16项、目标在前256中的名次及耗时。
结束后记录 top1/top3/top256；未出现是质量失败样本，不得删除或改成通过。
对可达目标实际选中，验证精确提交、输入耗尽、无重复提交。
硬行为检查覆盖每键不丢码/不提前提交、退格恢复、重新输入恢复、
增加一个待续键后撤回的状态恢复。待续目标可达率另行报告，不预设歧义输入
一定选中某个文本。质量统计与硬行为错误分开。

摘要必须包含所有排除计数、长度/专名分层、按原始句子聚类的固定 seed
bootstrap 区间、两组配对差异、源码/包/轨迹哈希和实际 librime/Lua 版本。
耗时包含真实菜单生成，但前256探测单独统计；并发全量审计期间的测量
必须标注机器负载，不能作为无负载生产性能保证。

## 已知训练文本重叠审计

另行按固定提交与 SHA256 读取 KDConv 全部九个 train/dev/test 文件和 PTT 原始文本，
使用仓库既有抽取脚本；PTT 的 OpenCC 固定为 0.1.7。检测前统一 NFKC 并保留字母数字，
逐例报告与完整训练句子等值、以及共享连续 8 字跨度（保守重叠提示，不等于语义抄袭）。
短于 8 字的案例仍记录精确等值，不凭空计为通过近重复检查。所有原始案例继续留在
主质量分母，不因观察到名次或重叠而剔除；另报没有这些已知重叠的子集。
这不能证明万象等词频源未见过原文，也不能将“无已知重叠”改称绝对无污染。

## 复现

```sh
cargo run --locked -p xhup-core --example syllable_codes > syllables.tsv
python3 tests/quality/prepare_ud_corpus.py SOURCE.conllu syllables.tsv fixture.tsv metadata.json
XHUP_AUDIT_ONLY_REPLAY=1 XHUP_REPLAY_CORPUS="$PWD/fixture.tsv" \
  XHUP_CORPUS_MODE=planner tests/librime/run-flow-audit.sh GENERATED > planner.jsonl
# 同样命令切换 XHUP_CORPUS_MODE=native-only，使用另一干净部署。
```

状态：协议已固定，完整原生测量尚未完成。不得将启动任务记作通过。
