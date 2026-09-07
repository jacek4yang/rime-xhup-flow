//! 入库语料派生统计(data/corpus/)的来源与结构不变量测试(离线)。
//!
//! 锁定 provenance 文档锚点(仓库/提交/许可/生成命令)与 TSV 结构;
//! 骨干口语词必须在会话域统计中可见(数据源偏差绊线)。

use std::path::Path;

use xhup_analyzer::corpus::CorpusStats;

const CORPUS_DIR: &str = "../../data/corpus";

fn conversation_stats() -> CorpusStats {
    let text = std::fs::read_to_string(Path::new(CORPUS_DIR).join("conversation_kdconv.tsv"))
        .expect("应能读取 KdConv 会话域派生统计");
    CorpusStats::from_tsv(&text).expect("派生统计 TSV 应可解析")
}

#[test]
fn conversation_tsv_is_structurally_valid() {
    let stats = conversation_stats();
    assert!(stats.sentences > 50_000, "KdConv 三域全量应产出数万句");
    assert!(stats.words.len() > 5_000, "应覆盖数千 canonical 词");
    for (word, entry) in &stats.words {
        assert!(
            entry.count > 0 && entry.sentence_count > 0,
            "{word} 计数应为正"
        );
        assert!(
            entry.sentence_count <= entry.count,
            "{word} 句子数不得超过总次数"
        );
        assert!(
            entry.sentence_count <= stats.sentences,
            "{word} 句子数不得超过总句数"
        );
    }
}

#[test]
fn conversational_backbone_words_are_visible() {
    // 骨干口语词在会话域统计中必须可见;缺席意味着数据源
    // 与「移动聊天」目标场景脱节(语料选择错误)或统计管线断裂。
    // 注意 KdConv 是任务导向对话(推荐/知识驱动),「我们」类主语词
    // 天然低频(实测 88 句)而「知道」类引导词高频(实测 4,973 句)——
    // 阈值锁定可见性而非频率直觉,频率校准是 optimizer v2 的职责。
    let stats = conversation_stats();
    for word in ["我们", "什么", "可以", "没有", "就是", "知道", "现在"] {
        let entry = stats
            .words
            .get(word)
            .unwrap_or_else(|| panic!("会话域统计应含骨干词 {word}"));
        assert!(
            entry.sentence_count >= 10,
            "{word} 的会话域句子覆盖应 ≥10,实际 {}",
            entry.sentence_count
        );
    }
    // 高强度哨兵:知识驱动对话的引导词必须大规模可见。
    assert!(stats.words["知道"].sentence_count >= 1_000);
    assert!(stats.words["什么"].sentence_count >= 500);
}

#[test]
fn provenance_documents_exist_and_pin_source() {
    let readme = std::fs::read_to_string(Path::new(CORPUS_DIR).join("README.md")).unwrap();
    for needle in [
        "thu-coai/KdConv",
        "653db76432de09a004ba708a68f8bbd5500e6bec",
        "Apache",
        "kdconv_to_sentences.py",
        "corpus-stats",
    ] {
        assert!(
            readme.contains(needle),
            "data/corpus/README.md 缺少 `{needle}`"
        );
    }
    let license = std::fs::read_to_string(Path::new(CORPUS_DIR).join("LICENSE.kdconv")).unwrap();
    assert!(license.contains("Apache License"));
}
