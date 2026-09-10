//! 训练器规范数据集的 JSON 投影(V4)。
//!
//! 训练器不维护任何自己的双拼映射、汉字编码、词码、简码策略或频率表:
//! 本模块把生成器的全部最终化 canonical 数据(单字、固定词、一级简码、
//! optimizer v2 两个生产简码层、组句 fixtures)与 `xhup-core` 的规范双拼布局投影为
//! 版本化 JSON 文档,与 Rime 词典共享同一份最终化数据(不存在第二份
//! 推导/排名实现)。
//!
//! V4 契约(见 `schemaVersion = 4`):
//! - `entries`:core + attested 生产字符，包含 scope、codeSource 与来源状态；
//! - `words`:固定词全码(按 Rime 权重降序取前 [`TRAINER_WORD_LIMIT`]
//!   条;训练无需全部 10 万词,截断保持数据集体积与加载校验可控);
//! - `level1Shortcuts` / `primaryShortcuts` / `fixedFirstShortcuts`:全部
//!   production 简码完整收录;PRIMARY 同时携带相对 rank 与 merged rank;
//! - `sentences`:组句练习 fixtures——语义组件列表(词),输入码由
//!   组件 canonical 全码机械拼接,不手写任何码串;组件无法全部解析的
//!   fixture 整条跳过(确定性)。
//!
//! 输出无时间戳、无随机数、无机器路径:相同规范数据与相同生成器源码(含
//! package version)下字节级一致。

use std::collections::HashMap;

use serde::Serialize;

use xhup_core::DoublePinyinLayout;

use crate::char_codes::finalized_char_code_entries;
use crate::fixed_first_shortcuts::canonical_fixed_first_shortcut_entries;
use crate::primary_shortcuts::canonical_primary_shortcut_entries;
use crate::shortcuts::canonical_level1_shortcuts;
use crate::word_codes::canonical_word_code_entries;

/// 训练器数据集产物文件名(生成器拥有的产物标识,调用方不得自行命名)。
pub const TRAINER_DATA_FILENAME: &str = "xhup_flow_trainer.json";

/// 数据集契约版本(V4:生产 InputHanzi provenance + canonical optimizer v2)。
pub const TRAINER_SCHEMA_VERSION: u32 = 4;

/// 固定词收录上限(按 Rime 权重降序截断;0 表示不截断)。
pub const TRAINER_WORD_LIMIT: usize = 20_000;

/// 训练器 JSON 顶层文档(`schemaVersion` 是兼容性边界)。
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct TrainerDataset {
    schema_version: u32,
    package_version: &'static str,
    /// 单字条目(2/3/4 码 canonical 关系;与 V1 兼容的既有字段)。
    entries: Vec<TrainerEntry>,
    /// 固定词全码(权重降序,截断于 [`TRAINER_WORD_LIMIT`])。
    words: Vec<TrainerWord>,
    /// 一级简码(26 条,完整)。
    level1_shortcuts: Vec<TrainerLevel1Shortcut>,
    /// optimizer v2 PRIMARY 词语简码(完整)。
    primary_shortcuts: Vec<TrainerPrimaryShortcut>,
    /// FIXED_FIRST 词语简码(完整)。
    fixed_first_shortcuts: Vec<TrainerShortcut>,
    /// 组句练习 fixtures(组件全码机械拼接)。
    sentences: Vec<TrainerSentence>,
    double_pinyin: DoublePinyinReference,
}

/// 一条单字训练条目:一个最终化的 `(汉字, 静态码)` 关系。
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct TrainerEntry {
    char: char,
    code: String,
    length: usize,
    readings: Vec<&'static str>,
    /// 规范默认读音(带调;来自 Unihan kMandarin 教育子集,见
    /// data/education/README.md)。教育元数据,缺失时前端回退无调展示。
    #[serde(skip_serializing_if = "Option::is_none")]
    tone_reading: Option<&'static str>,
    frequency_score: u64,
    rime_weight: u32,
    /// 字符成员层：规范 8105 core 或 attested 扩展。
    scope: &'static str,
    /// 当前编码关系的推导/证据类别。
    code_source: &'static str,
    /// 稳定来源标识与来源状态；core 机械推导关系可为空。
    sources: Vec<&'static str>,
    statuses: Vec<&'static str>,
}

/// 一条固定词训练条目(全码)。
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct TrainerWord {
    word: String,
    code: String,
    /// 全码键数(4/6/8)。
    length: usize,
    /// 汉字数。
    char_count: usize,
    rime_weight: u32,
}

/// 一条一级简码关系。
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct TrainerLevel1Shortcut {
    key: char,
    char: char,
}

/// 一条词语简码关系(shortcut 不替换 full_code,两者都保留可用)。
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct TrainerShortcut {
    word: String,
    full_code: String,
    shortcut_code: String,
    /// F/I 投影模式(如 `FI` / `II`)。
    mode: String,
}

/// 一条 PRIMARY 词语简码关系(相对与绝对候选位均来自 selected dump)。
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct TrainerPrimaryShortcut {
    word: String,
    full_code: String,
    shortcut_code: String,
    rank: usize,
    merged_rank: usize,
}

/// 一条组句练习 fixture:语义组件 + 机械拼接的全码。
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct TrainerSentence {
    /// 句子文本(组件依次拼接)。
    text: String,
    /// 输入码 = 组件 canonical 全码按字序拼接。
    code: String,
    /// 组件词列表(分段展示/提示用)。
    components: Vec<String>,
}

/// 规范小鹤双拼键盘布局参考(供前端渲染键位/映射,不在 TS 中复制)。
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct DoublePinyinReference {
    initials: Vec<InitialEntry>,
    finals: Vec<FinalEntry>,
    zero_initials: Vec<ZeroInitialEntry>,
}

#[derive(Serialize)]
struct InitialEntry {
    initial: &'static str,
    key: char,
}

#[derive(Serialize)]
struct FinalEntry {
    #[serde(rename = "final")]
    final_: &'static str,
    key: char,
}

#[derive(Serialize)]
struct ZeroInitialEntry {
    syllable: &'static str,
    code: String,
}

/// 组句 fixture 语义组件(词表;输入码机械派生,不手写)。
///
/// 每条 fixture 的全部组件都必须能在 canonical 固定词全码集中解析,
/// 否则整条跳过;因此这里只表达「想练什么句子」,不承担码的正确性。
const SENTENCE_FIXTURES: &[&[&str]] = &[
    // 2 词(4 字)
    &["我们", "时间"],
    &["经济", "发展"],
    &["文化", "教育"],
    &["社会", "工作"],
    // 4 词(8 字)
    &["我们", "学习", "文化", "教育"],
    &["工作", "时间", "社会", "经济"],
    &["科技", "发展", "生活", "方式"],
    &["教育", "工作", "学习", "方法"],
    // 8 词(16 字)
    &[
        "我们", "时间", "发展", "工作", "科技", "教育", "社会", "生活",
    ],
    &[
        "经济", "文化", "学习", "工作", "社会", "科技", "生活", "教育",
    ],
    &[
        "方法", "问题", "系统", "服务", "市场", "产品", "质量", "环境",
    ],
    // 10 词(20 字)
    &[
        "我们", "时间", "发展", "工作", "科技", "教育", "社会", "生活", "学习", "世界",
    ],
    &[
        "经济", "文化", "方法", "问题", "系统", "服务", "市场", "产品", "质量", "环境",
    ],
    &[
        "今天", "明天", "现在", "历史", "中国", "国家", "公司", "孩子", "朋友", "老师",
    ],
];

/// 规范带调读音教育子集(数据来源与许可见 data/education/README.md;
/// Unihan 17.0.0 kMandarin,8105 字全覆盖)。教育元数据,不属于编码契约。
const EDUCATIONAL_TONE_TSV: &str = include_str!("../../../data/education/pinyin_tone.tsv");

/// 字 → 规范默认带调读音(如 时 → shí)。
fn educational_tone_readings() -> std::collections::HashMap<char, &'static str> {
    EDUCATIONAL_TONE_TSV
        .lines()
        .filter_map(|line| {
            let mut parts = line.split('\t');
            let ch = parts.next()?.chars().next()?;
            let tone = parts.next()?;
            Some((ch, tone))
        })
        .collect()
}

/// 生成训练器规范数据集 JSON(UTF-8、恰好一个末尾换行、字节级可复现)。
pub fn generate_trainer_dataset() -> String {
    let tone_readings = educational_tone_readings();
    let entries = finalized_char_code_entries()
        .iter()
        .map(|entry| TrainerEntry {
            char: entry.hanzi().as_char(),
            code: entry.code().to_string(),
            length: entry.code().len(),
            readings: entry
                .readings()
                .iter()
                .map(|reading| reading.as_str())
                .collect(),
            tone_reading: tone_readings.get(&entry.hanzi().as_char()).copied(),
            frequency_score: entry.frequency_score(),
            rime_weight: entry.rime_weight(),
            scope: if entry.hanzi().is_core_standard() {
                "core"
            } else {
                "extended"
            },
            code_source: if entry.is_core_derived() {
                "canonical-reading-shape"
            } else if entry.is_official() {
                "official-attested"
            } else {
                "legacy-attested"
            },
            sources: entry.sources().to_vec(),
            statuses: entry.statuses().to_vec(),
        })
        .collect();

    // 固定词:权重降序 → 词升序,截断于 TRAINER_WORD_LIMIT。
    let mut words: Vec<_> = canonical_word_code_entries()
        .into_iter()
        .map(|entry| {
            (
                entry.word().to_string(),
                entry.code().to_string(),
                entry.weight(),
            )
        })
        .collect();
    words.sort_by(|a, b| b.2.cmp(&a.2).then(a.0.cmp(&b.0)));
    words.truncate(TRAINER_WORD_LIMIT);
    let words: Vec<TrainerWord> = words
        .into_iter()
        .map(|(word, code, weight)| TrainerWord {
            char_count: word.chars().count(),
            length: code.chars().count(),
            word,
            code,
            rime_weight: weight,
        })
        .collect();

    let level1_shortcuts = canonical_level1_shortcuts()
        .iter()
        .map(|entry| TrainerLevel1Shortcut {
            key: entry.key().as_char(),
            char: entry.hanzi().as_char(),
        })
        .collect();

    let primary_shortcuts = canonical_primary_shortcut_entries()
        .iter()
        .map(|entry| TrainerPrimaryShortcut {
            word: entry.word().to_string(),
            full_code: entry.full_code().to_string(),
            shortcut_code: entry.shortcut_code().to_string(),
            rank: entry.rank(),
            merged_rank: entry.merged_rank(),
        })
        .collect();
    let fixed_first_shortcuts = canonical_fixed_first_shortcut_entries()
        .iter()
        .map(|entry| TrainerShortcut {
            word: entry.word().to_string(),
            full_code: entry.full_code().to_string(),
            shortcut_code: entry.shortcut_code().to_string(),
            mode: entry.mode().to_string(),
        })
        .collect();
    let sentences = build_sentence_fixtures();

    let layout = DoublePinyinLayout::canonical();
    let dataset = TrainerDataset {
        schema_version: TRAINER_SCHEMA_VERSION,
        package_version: env!("CARGO_PKG_VERSION"),
        entries,
        words,
        level1_shortcuts,
        primary_shortcuts,
        fixed_first_shortcuts,
        sentences,
        double_pinyin: DoublePinyinReference {
            initials: layout
                .initials()
                .iter()
                .map(|mapping| InitialEntry {
                    initial: mapping.initial(),
                    key: mapping.key().as_char(),
                })
                .collect(),
            finals: layout
                .finals()
                .iter()
                .map(|mapping| FinalEntry {
                    final_: mapping.final_(),
                    key: mapping.key().as_char(),
                })
                .collect(),
            zero_initials: layout
                .zero_initials()
                .iter()
                .map(|mapping| ZeroInitialEntry {
                    syllable: mapping.syllable(),
                    code: mapping.code().to_string(),
                })
                .collect(),
        },
    };

    let mut json = serde_json::to_string(&dataset).expect("训练器数据集序列化不应失败(纯内存结构)");
    json.push('\n');
    json
}

/// 机械构建组句 fixtures:组件全码从 canonical 固定词解析并拼接;
/// 任一组件无法解析则整条跳过(确定性;不手写任何码串)。
fn build_sentence_fixtures() -> Vec<TrainerSentence> {
    let word_codes: HashMap<String, String> = canonical_word_code_entries()
        .into_iter()
        .map(|entry| (entry.word().to_string(), entry.code().to_string()))
        .collect();
    let mut fixtures = Vec::new();
    for components in SENTENCE_FIXTURES {
        let mut text = String::new();
        let mut code = String::new();
        let mut resolved = Vec::with_capacity(components.len());
        let mut complete = true;
        for word in *components {
            match word_codes.get(*word) {
                Some(word_code) => {
                    text.push_str(word);
                    code.push_str(word_code);
                    resolved.push((*word).to_string());
                }
                None => {
                    complete = false;
                    break;
                }
            }
        }
        if complete {
            fixtures.push(TrainerSentence {
                text,
                code,
                components: resolved,
            });
        }
    }
    fixtures
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::char_codes::finalized_char_code_entries;
    use crate::fixed_first_shortcuts::canonical_fixed_first_shortcut_entries;
    use crate::primary_shortcuts::canonical_primary_shortcut_entries;
    use crate::shortcuts::canonical_level1_shortcuts;
    use crate::word_codes::canonical_word_code_entries;

    fn parse() -> serde_json::Value {
        serde_json::from_str(&generate_trainer_dataset()).expect("应为合法 JSON")
    }

    #[test]
    fn top_level_contract() {
        let doc = parse();
        assert_eq!(doc["schemaVersion"], 4);
        assert_eq!(doc["packageVersion"], env!("CARGO_PKG_VERSION"));
        assert!(doc["entries"].is_array());
        assert!(doc["words"].is_array());
        assert!(doc["level1Shortcuts"].is_array());
        assert!(doc["primaryShortcuts"].is_array());
        assert!(doc["fixedFirstShortcuts"].is_array());
        assert!(doc["sentences"].is_array());
        assert!(doc["doublePinyin"].is_object());
        // 无时间戳等易变字段
        for key in ["generatedAt", "timestamp", "date", "version"] {
            assert!(doc.get(key).is_none(), "不应包含 {key}");
        }
    }

    #[test]
    fn entries_match_finalized_set_exactly() {
        let doc = parse();
        let json_entries = doc["entries"].as_array().unwrap();
        let finalized = finalized_char_code_entries();
        assert_eq!(json_entries.len(), 28_851);
        assert_eq!(json_entries.len(), finalized.len());
        for (json, entry) in json_entries.iter().zip(finalized) {
            assert_eq!(json["char"].as_str().unwrap(), entry.hanzi().to_string());
            assert_eq!(json["code"].as_str().unwrap(), entry.code().to_string());
            assert_eq!(json["length"].as_u64().unwrap(), entry.code().len() as u64);
            assert_eq!(
                json["frequencyScore"].as_u64().unwrap(),
                entry.frequency_score()
            );
            assert_eq!(
                json["scope"].as_str().unwrap(),
                if entry.hanzi().is_core_standard() {
                    "core"
                } else {
                    "extended"
                }
            );
        }
    }

    #[test]
    fn generation_is_byte_identical() {
        assert_eq!(
            generate_trainer_dataset(),
            generate_trainer_dataset(),
            "两次生成字节级一致"
        );
    }

    #[test]
    fn words_are_weight_capped_deterministic_subset() {
        let doc = parse();
        let words = doc["words"].as_array().unwrap();
        assert_eq!(words.len(), TRAINER_WORD_LIMIT, "固定词应按上限截断");
        // 权重降序;同权重按词升序。
        for pair in words.windows(2) {
            let (a, b) = (&pair[0], &pair[1]);
            let wa = a["rimeWeight"].as_u64().unwrap();
            let wb = b["rimeWeight"].as_u64().unwrap();
            assert!(wa >= wb, "词权重应非升序");
            if wa == wb {
                assert!(a["word"].as_str().unwrap() <= b["word"].as_str().unwrap());
            }
        }
        // 每条:码长 ∈ {4,6,8} 且等于字数×2(逐字双拼拼接)。
        for word in words {
            let code = word["code"].as_str().unwrap();
            let text = word["word"].as_str().unwrap();
            assert!(matches!(code.len(), 4 | 6 | 8), "词码长应 ∈ {{4,6,8}}");
            assert_eq!(
                word["charCount"].as_u64().unwrap(),
                text.chars().count() as u64
            );
            assert_eq!(word["length"].as_u64().unwrap(), code.len() as u64);
            assert_eq!(code.len(), text.chars().count() * 2);
        }
    }

    #[test]
    fn level1_shortcuts_complete() {
        let doc = parse();
        let level1 = doc["level1Shortcuts"].as_array().unwrap();
        assert_eq!(level1.len(), 26);
        assert_eq!(level1.len(), canonical_level1_shortcuts().len());
        for (json, entry) in level1.iter().zip(canonical_level1_shortcuts()) {
            assert_eq!(
                json["key"].as_str().unwrap(),
                entry.key().as_char().to_string()
            );
            assert_eq!(
                json["char"].as_str().unwrap(),
                entry.hanzi().as_char().to_string()
            );
        }
    }

    #[test]
    fn shortcut_layers_complete_and_shape_valid() {
        let doc = parse();
        let cases = [(
            "fixedFirstShortcuts",
            canonical_fixed_first_shortcut_entries().len(),
        )];
        for (key, expected) in cases {
            let rows = doc[key].as_array().unwrap();
            assert_eq!(rows.len(), expected, "{key} 应完整收录生产层");
            for row in rows {
                let full = row["fullCode"].as_str().unwrap();
                let shortcut = row["shortcutCode"].as_str().unwrap();
                assert!(
                    !shortcut.is_empty() && shortcut.len() < full.len(),
                    "{key}: 简码应短于全码"
                );
                assert!(!row["mode"].as_str().unwrap().is_empty());
            }
        }
        let primary = doc["primaryShortcuts"].as_array().unwrap();
        assert_eq!(primary.len(), canonical_primary_shortcut_entries().len());
        assert_eq!(primary.len(), 65_909);
        for row in primary {
            assert!(row["rank"].as_u64().unwrap() >= 1);
            assert!(row["mergedRank"].as_u64().unwrap() >= row["rank"].as_u64().unwrap());
        }
        assert_eq!(doc["fixedFirstShortcuts"].as_array().unwrap().len(), 2_933);
    }

    #[test]
    fn sentences_cover_all_practice_lengths() {
        let doc = parse();
        let sentences = doc["sentences"].as_array().unwrap();
        assert!(!sentences.is_empty(), "组句 fixtures 不应为空");
        let mut has = [false; 4]; // 2/4/8/10 词
        for sentence in sentences {
            let components = sentence["components"].as_array().unwrap();
            let text = sentence["text"].as_str().unwrap();
            let code = sentence["code"].as_str().unwrap();
            assert_eq!(components.len() * 2, text.chars().count());
            assert_eq!(code.len(), text.chars().count() * 2);
            match components.len() {
                2 => has[0] = true,
                4 => has[1] = true,
                8 => has[2] = true,
                10 => has[3] = true,
                _ => panic!("fixture 词数应 ∈ {{2,4,8,10}}"),
            }
        }
        assert!(has.iter().all(|present| *present), "四档长度都应有 fixture");
    }

    #[test]
    fn words_count_matches_canonical_source() {
        // 截断正确性:canonical 词全量 > 上限。
        assert!(canonical_word_code_entries().len() > TRAINER_WORD_LIMIT);
    }
}
