//! 训练器规范数据集(JSON)的集成测试,以及 Rime 词典与训练器数据集的
//! 强一致性不变量:两者必须是同一份最终化静态单字条目集的投影。
//! 只使用公共序列化输出,不接触内部实现。

use std::collections::BTreeSet;

use xhup_generator::{generate_rime_char_dictionary, generate_trainer_dataset};

fn trainer_json() -> serde_json::Value {
    serde_json::from_str(&generate_trainer_dataset()).expect("训练器数据集应为合法 JSON")
}

#[test]
fn top_level_contract() {
    let doc = trainer_json();
    assert_eq!(doc["schemaVersion"], 1);
    assert_eq!(doc["packageVersion"], env!("CARGO_PKG_VERSION"));
    assert_eq!(doc["entries"].as_array().unwrap().len(), 26753);
    for key in ["schemaVersion", "packageVersion", "entries", "doublePinyin"] {
        assert!(doc.get(key).is_some(), "缺少顶层字段 {key}");
    }
    assert_eq!(doc.as_object().unwrap().len(), 4, "顶层字段集合固定");
}

#[test]
fn entries_are_well_formed_and_unique() {
    let doc = trainer_json();
    let entries = doc["entries"].as_array().unwrap();
    let mut pairs = BTreeSet::new();
    for entry in entries {
        let zi = entry["char"].as_str().unwrap();
        assert_eq!(zi.chars().count(), 1, "char 恰为一个字符");
        let code = entry["code"].as_str().unwrap();
        assert!(code.bytes().all(|b| b.is_ascii_lowercase()));
        assert!(matches!(code.len(), 2..=4));
        assert_eq!(entry["length"].as_u64().unwrap() as usize, code.len());
        assert!(entry["rimeWeight"].as_u64().unwrap() >= 1);
        entry["frequencyScore"]
            .as_u64()
            .expect("frequencyScore 应为 u64 兼容整数");
        let readings: Vec<&str> = entry["readings"]
            .as_array()
            .unwrap()
            .iter()
            .map(|r| r.as_str().unwrap())
            .collect();
        assert!(!readings.is_empty(), "每个条目至少一个贡献读音");
        for pair in readings.windows(2) {
            assert!(pair[0] < pair[1], "readings 唯一且字典序升序");
        }
        assert!(
            pairs.insert((zi, code)),
            "无重复 (char, code) 条目: {zi} {code}"
        );
    }
}

#[test]
fn double_pinyin_reference_sentinels() {
    let doc = trainer_json();
    let dp = &doc["doublePinyin"];
    assert_eq!(dp["initials"].as_array().unwrap().len(), 23);
    assert_eq!(dp["finals"].as_array().unwrap().len(), 33);
    assert_eq!(dp["zeroInitials"].as_array().unwrap().len(), 12);

    let initials = dp["initials"].as_array().unwrap();
    assert!(
        initials
            .iter()
            .any(|m| m["initial"] == "zh" && m["key"] == "v")
    );
    let finals = dp["finals"].as_array().unwrap();
    assert!(
        finals
            .iter()
            .any(|m| m["final"] == "ang" && m["key"] == "h")
    );
    assert!(finals.iter().any(|m| m["final"] == "ve" && m["key"] == "t"));
    let zero = dp["zeroInitials"].as_array().unwrap();
    assert!(
        zero.iter()
            .any(|m| m["syllable"] == "a" && m["code"] == "aa")
    );
    assert!(
        zero.iter()
            .any(|m| m["syllable"] == "ang" && m["code"] == "ah")
    );
}

/// 强一致性不变量:Rime 词典与训练器 JSON 描述完全相同的
/// `(汉字, 码, Rime 权重)` 集合——防止两条生成路径漂移。
#[test]
fn rime_dictionary_and_trainer_dataset_describe_identical_entries() {
    let dict = generate_rime_char_dictionary();
    let rime_set: BTreeSet<(String, String, u64)> = dict
        .lines()
        .filter(|line| !line.starts_with('#') && line.contains('\t'))
        .map(|line| {
            let mut fields = line.split('\t');
            let (Some(zi), Some(code), Some(weight), None) =
                (fields.next(), fields.next(), fields.next(), fields.next())
            else {
                panic!("词典数据行格式: {line:?}");
            };
            (
                zi.to_string(),
                code.to_string(),
                weight.parse::<u64>().unwrap(),
            )
        })
        .collect();
    assert_eq!(rime_set.len(), 26753);

    let doc = trainer_json();
    let trainer_set: BTreeSet<(String, String, u64)> = doc["entries"]
        .as_array()
        .unwrap()
        .iter()
        .map(|entry| {
            (
                entry["char"].as_str().unwrap().to_string(),
                entry["code"].as_str().unwrap().to_string(),
                entry["rimeWeight"].as_u64().unwrap(),
            )
        })
        .collect();
    assert_eq!(trainer_set.len(), 26753);

    assert_eq!(rime_set, trainer_set, "Rime 与训练器必须是同一最终化条目集");
}

#[test]
fn generation_is_byte_reproducible() {
    assert_eq!(
        generate_trainer_dataset().as_bytes(),
        generate_trainer_dataset().as_bytes()
    );
}
