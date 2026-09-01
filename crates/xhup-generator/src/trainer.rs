//! 训练器规范数据集的 JSON 投影。
//!
//! 训练器(键盘布局教学、2/3/4 码练习)不维护任何自己的双拼映射、汉字编码或
//! 频率表:本模块把 [`crate::char_codes`] 的最终化静态单字条目集与
//! `xhup-core` 的规范双拼布局投影为版本化 JSON 文档,与 Rime 词典共享同一份
//! 最终化数据(不存在第二份推导/排名实现)。
//!
//! 输出无时间戳、无随机数、无机器路径:相同规范数据与相同生成器源码(含
//! package version)下字节级一致。

use serde::Serialize;

use xhup_core::DoublePinyinLayout;

use crate::char_codes::finalized_char_code_entries;

/// 训练器数据集产物文件名(生成器拥有的产物标识,调用方不得自行命名)。
pub const TRAINER_DATA_FILENAME: &str = "xhup_flow_trainer.json";

/// 训练器 JSON 顶层文档(`schemaVersion` 是兼容性边界)。
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct TrainerDataset {
    schema_version: u32,
    package_version: &'static str,
    entries: Vec<TrainerEntry>,
    double_pinyin: DoublePinyinReference,
}

/// 一条训练条目:一个最终化的 `(汉字, 静态码)` 关系。
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct TrainerEntry {
    char: char,
    code: String,
    length: usize,
    readings: Vec<&'static str>,
    frequency_score: u64,
    rime_weight: u32,
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

/// 生成训练器规范数据集 JSON(UTF-8、恰好一个末尾换行、字节级可复现)。
pub fn generate_trainer_dataset() -> String {
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
            frequency_score: entry.frequency_score(),
            rime_weight: entry.rime_weight(),
        })
        .collect();

    let layout = DoublePinyinLayout::canonical();
    let dataset = TrainerDataset {
        schema_version: 1,
        package_version: env!("CARGO_PKG_VERSION"),
        entries,
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::char_codes::finalized_char_code_entries;

    fn parse() -> serde_json::Value {
        serde_json::from_str(&generate_trainer_dataset()).expect("应为合法 JSON")
    }

    #[test]
    fn top_level_contract() {
        let doc = parse();
        assert_eq!(doc["schemaVersion"], 1);
        assert_eq!(doc["packageVersion"], env!("CARGO_PKG_VERSION"));
        assert!(doc["entries"].is_array());
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
        assert_eq!(json_entries.len(), 26753);
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
                json["rimeWeight"].as_u64().unwrap(),
                entry.rime_weight() as u64
            );
            let readings: Vec<&str> = json["readings"]
                .as_array()
                .unwrap()
                .iter()
                .map(|r| r.as_str().unwrap())
                .collect();
            let expected: Vec<&str> = entry.readings().iter().map(|r| r.as_str()).collect();
            assert_eq!(readings, expected);
        }
    }

    #[test]
    fn double_pinyin_reference_matches_canonical_layout() {
        let doc = parse();
        let layout = DoublePinyinLayout::canonical();
        let dp = &doc["doublePinyin"];

        let initials = dp["initials"].as_array().unwrap();
        assert_eq!(initials.len(), layout.initials().len());
        for (json, mapping) in initials.iter().zip(layout.initials()) {
            assert_eq!(json["initial"].as_str().unwrap(), mapping.initial());
            assert_eq!(
                json["key"].as_str().unwrap(),
                mapping.key().as_char().to_string()
            );
        }

        let finals = dp["finals"].as_array().unwrap();
        assert_eq!(finals.len(), layout.finals().len());
        for (json, mapping) in finals.iter().zip(layout.finals()) {
            assert_eq!(json["final"].as_str().unwrap(), mapping.final_());
            assert_eq!(
                json["key"].as_str().unwrap(),
                mapping.key().as_char().to_string()
            );
        }

        let zero = dp["zeroInitials"].as_array().unwrap();
        assert_eq!(zero.len(), layout.zero_initials().len());
        for (json, mapping) in zero.iter().zip(layout.zero_initials()) {
            assert_eq!(json["syllable"].as_str().unwrap(), mapping.syllable());
            assert_eq!(json["code"].as_str().unwrap(), mapping.code().to_string());
        }
    }

    #[test]
    fn collapsed_relation_lists_all_contributing_readings() {
        let doc = parse();
        let lok = doc["entries"]
            .as_array()
            .unwrap()
            .iter()
            .find(|e| e["char"] == "咯" && e["code"] == "lok")
            .expect("咯 lok 应存在");
        let readings: Vec<&str> = lok["readings"]
            .as_array()
            .unwrap()
            .iter()
            .map(|r| r.as_str().unwrap())
            .collect();
        assert_eq!(readings, ["lo", "luo"]);
    }

    #[test]
    fn generation_is_byte_reproducible() {
        assert_eq!(
            generate_trainer_dataset().as_bytes(),
            generate_trainer_dataset().as_bytes()
        );
    }

    #[test]
    fn output_is_single_line_with_one_final_newline() {
        let json = generate_trainer_dataset();
        assert!(json.ends_with('\n') && !json.ends_with("\n\n"));
        assert!(!json.trim_end().contains('\n'), "紧凑单行 JSON");
        assert!(!json.starts_with('\u{feff}'), "无 BOM");
    }
}
