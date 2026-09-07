//! 规范高频词语数据(万象 / RIME-LMDG 提取子集)的来源与数据不变量测试(离线)。
//!
//! 锁定 pin 的来源身份(仓库/提交/路径/blob SHA/许可)、TSV 结构不变量、二字词
//! 与单字全码的碰撞共存语义与覆盖审计;不访问网络,不测试上游当前状态。

use std::collections::BTreeSet;
use std::path::Path;

use xhup_core::XhupHanzi;
use xhup_generator::canonical_char_entries;

const WORDS_DIR: &str = "../../data/words";

fn tsv() -> String {
    std::fs::read_to_string(Path::new(WORDS_DIR).join("wanxiang_base_words.tsv"))
        .expect("应能读取入库词语 TSV")
}

/// 数据行 `(词, 规范读音序列, 分数)`。
fn data_rows(text: &str) -> Vec<(&str, Vec<&str>, u64)> {
    text.lines()
        .filter(|line| !line.starts_with('#'))
        .map(|line| {
            let mut fields = line.split('\t');
            let (Some(word), Some(readings), Some(score), None) =
                (fields.next(), fields.next(), fields.next(), fields.next())
            else {
                panic!("词语数据行应恰好为三列: {line:?}");
            };
            (
                word,
                readings.split(' ').collect(),
                score.parse().expect("分数应为 u64"),
            )
        })
        .collect()
}

/// 用 xhup-core 公共 API 独立推导一行的 XHUP 词码(逐字双拼两键按字序拼接)。
fn derive_code(word: &str, readings: &[&str]) -> String {
    assert_eq!(word.chars().count(), readings.len(), "读音数应等于字数");
    let mut code = String::new();
    for (zi, spelling) in word.chars().zip(readings) {
        let hanzi = XhupHanzi::try_from(zi).expect("汉字应在规范清单内");
        let reading = hanzi
            .readings()
            .iter()
            .copied()
            .find(|r| r.as_str() == *spelling)
            .expect("读音应为该字规范读音");
        let syllable = reading
            .to_input_syllable()
            .expect("读音应可编码为 XHUP 输入音节");
        code.push_str(&syllable.to_double_pinyin_code().to_string());
    }
    code
}

/// 规范单字全码集(二字词 collision 比对的基准)。
fn canonical_fullcodes() -> BTreeSet<String> {
    canonical_char_entries()
        .iter()
        .map(|entry| entry.code().to_string())
        .collect()
}

#[test]
fn tsv_header_locks_pinned_source_identity() {
    let text = tsv();
    for line in [
        "# source_repo: amzxyz/rime-wanxiang",
        "# source_commit: 4618d67a978ff4f41b165c10b35558d38e333ab1",
        "# source_path: dicts/jichu.dict.yaml",
        "# source_blob_sha: a0f66e2fc6130f3f1c9b2e5109644c8b893477b0",
        "# source_license: CC-BY-4.0",
        "# semantic_source: amzxyz/RIME-LMDG",
    ] {
        assert!(text.contains(line), "TSV 注释头缺少 `{line}`");
    }
    assert!(!text.contains('\r'), "LF only");
}

#[test]
fn tsv_row_count_matches_header_and_audit() {
    let text = tsv();
    let rows = data_rows(&text);
    assert_eq!(rows.len(), 100_000);
    assert!(
        text.contains(&format!("# rows: {}", rows.len())),
        "注释头行数应与实际数据行一致"
    );
    for (len, expected) in [(2, 50_000), (3, 30_000), (4, 20_000)] {
        assert_eq!(
            rows.iter()
                .filter(|(word, _, _)| word.chars().count() == len)
                .count(),
            expected,
            "{len} 字 semantic entries"
        );
    }
}

#[test]
fn tsv_rows_are_strictly_ordered_unique_and_positive() {
    let text = tsv();
    let rows = data_rows(&text);
    for (word, readings, score) in &rows {
        assert!(*score > 0, "入库行分数应为正");
        let word_len = word.chars().count();
        assert!((2..=4).contains(&word_len), "词长应为 2~4: {word}");
        assert_eq!(readings.len(), word_len, "读音数应等于字数: {word}");
        for reading in readings {
            assert!(
                reading.bytes().all(|b| b.is_ascii_lowercase()),
                "读音应为小写 ASCII: {reading}"
            );
        }
    }
    for pair in rows.windows(2) {
        let a = (pair[0].0.chars().count(), pair[0].0, &pair[0].1);
        let b = (pair[1].0.chars().count(), pair[1].0, &pair[1].1);
        assert!(a < b, "行应按 (词长, 词, 读音序列) 严格升序(无重复)");
    }
}

#[test]
fn every_row_maps_to_canonical_relations() {
    for (word, readings, _) in data_rows(&tsv()) {
        for (zi, spelling) in word.chars().zip(&readings) {
            let hanzi = XhupHanzi::try_from(zi).expect("汉字应在规范清单内");
            let reading = hanzi
                .readings()
                .iter()
                .find(|r| r.as_str() == *spelling)
                .unwrap_or_else(|| panic!("{zi} 的读音 {spelling} 应为规范读音"));
            assert!(
                reading.to_input_syllable().is_some(),
                "{zi} 的读音 {spelling} 应可编码为 XHUP 输入音节"
            );
        }
    }
}

#[test]
fn every_word_surface_is_unique() {
    // 上游 pinned 源每个词形恰好一个读音序列(见 README 审计),入库保持该性质。
    let text = tsv();
    let rows = data_rows(&text);
    let mut seen = BTreeSet::new();
    for (word, _, _) in &rows {
        assert!(seen.insert(*word), "词形应唯一: {word}");
    }
}

#[test]
fn sentinel_rows_match_real_data() {
    let text = tsv();
    let rows = data_rows(&text);
    let find = |word: &str| -> (&Vec<&str>, u64) {
        rows.iter()
            .find(|(w, _, _)| *w == word)
            .map(|(_, readings, score)| (readings, *score))
            .unwrap_or_else(|| panic!("哨兵词 {word} 应存在"))
    };
    assert_eq!(find("我们"), (&vec!["wo", "men"], 1_488_731));
    assert_eq!(find("中国"), (&vec!["zhong", "guo"], 438_875));
    assert_eq!(find("可以"), (&vec!["ke", "yi"], 1_454_147));
    assert_eq!(find("时间"), (&vec!["shi", "jian"], 533_426));
    assert_eq!(find("输入法"), (&vec!["shu", "ru", "fa"], 3_175));
    assert_eq!(find("图书馆"), (&vec!["tu", "shu", "guan"], 34_848));
    assert_eq!(find("社会主义"), (&vec!["she", "hui", "zhu", "yi"], 6_977));
    assert_eq!(
        find("众所周知"),
        (&vec!["zhong", "suo", "zhou", "zhi"], 22_206)
    );
}

#[test]
fn two_char_entries_may_coexist_with_canonical_fullcodes() {
    // 共存语义:二字 semantic entry 的 4 键码允许命中规范全码集——词与字
    // 在同码上共存,碰撞只影响候选排序(merged_ranking),绝不删除词。
    let fullcodes = canonical_fullcodes();
    assert_eq!(fullcodes.len(), 8416, "规范全码 distinct 数(回归锚点)");
    let text = tsv();
    let rows = data_rows(&text);
    let collided = rows
        .iter()
        .filter(|(word, readings, _)| {
            word.chars().count() == 2 && fullcodes.contains(&derive_code(word, readings))
        })
        .count();
    assert!(collided > 0, "什么/但是 等碰撞 entry 必须入库");
}

#[test]
fn collided_two_char_entry_is_retained() {
    // 回归:历史缺陷样本「但是 dan shi」——推导码 djui 是「蛋」的规范全码。
    // 共存修复后该 semantic entry 必须入库(碰撞不再导致词汇删除)。
    let fullcodes = canonical_fullcodes();
    assert!(fullcodes.contains("djui"), "djui 应属于规范全码集");
    let text = tsv();
    let rows = data_rows(&text);
    assert!(
        rows.iter().any(|(word, readings, _)| {
            *word == "但是" && derive_code(word, readings) == "djui"
        }),
        "「但是」必须入库并与「蛋」的全码 djui 共存"
    );
    assert!(
        rows.iter().any(|(word, readings, _)| {
            *word == "什么" && derive_code(word, readings) == "ufme"
        }),
        "「什么」必须入库并与「𬳽」的全码 ufme 共存"
    );
}

#[test]
fn everyday_word_sentinels_are_present() {
    // 日常高频词哨兵门禁:这些词是中文日常输入的骨干词汇,任何一个缺席
    // 都意味着词汇层被破坏(历史缺陷:什么/但是 曾因码碰撞被整体删除)。
    // 完整的 top-1000 语料级覆盖门禁随语料统计管线落地;此处锁定骨干哨兵。
    let text = tsv();
    let words: BTreeSet<&str> = data_rows(&text).iter().map(|(w, _, _)| *w).collect();
    for sentinel in [
        "什么",
        "为什么",
        "怎么",
        "这个",
        "那个",
        "可以",
        "没有",
        "不是",
        "就是",
        "知道",
        "现在",
        "还是",
        "已经",
        "如果",
        "因为",
        "所以",
        "我们",
        "你们",
        "他们",
        "但是",
        "比如",
        "选择",
        "数据",
        "中国",
        "时间",
        "工作",
        "学习",
        "问题",
        "朋友",
        "手机",
    ] {
        assert!(
            words.contains(sentinel),
            "骨干日常词 {sentinel} 必须在词库中"
        );
    }
}

#[test]
fn provenance_documents_exist_and_reference_license() {
    let readme = std::fs::read_to_string(Path::new(WORDS_DIR).join("README.md")).unwrap();
    for needle in [
        "amzxyz/rime-wanxiang",
        "4618d67a978ff4f41b165c10b35558d38e333ab1",
        "a0f66e2fc6130f3f1c9b2e5109644c8b893477b0",
        "dicts/jichu.dict.yaml",
        "CC BY 4.0",
        "RIME-LMDG",
    ] {
        assert!(readme.contains(needle), "README 缺少 `{needle}`");
    }
    let license = std::fs::read_to_string(Path::new(WORDS_DIR).join("LICENSE.wanxiang")).unwrap();
    assert!(license.contains("Attribution 4.0 International"));
}
