//! 规范频率数据(万象 / RIME-LMDG 提取子集)的来源与数据不变量测试(离线)。
//!
//! 锁定 pin 的来源身份(仓库/提交/路径/blob SHA/许可)、TSV 结构不变量与
//! 覆盖审计;不访问网络,不测试上游当前状态。

use std::collections::BTreeSet;
use std::path::Path;

use xhup_core::XhupHanzi;

const FREQUENCY_DIR: &str = "../../data/frequency";

fn tsv() -> String {
    std::fs::read_to_string(Path::new(FREQUENCY_DIR).join("wanxiang_reading_scores.tsv"))
        .expect("应能读取入库频率 TSV")
}

fn data_rows(text: &str) -> Vec<(char, &str, u64)> {
    text.lines()
        .filter(|line| !line.starts_with('#'))
        .map(|line| {
            let mut fields = line.split('\t');
            let (Some(zi), Some(reading), Some(score), None) =
                (fields.next(), fields.next(), fields.next(), fields.next())
            else {
                panic!("频率数据行应恰好为三列: {line:?}");
            };
            let mut chars = zi.chars();
            let zi = chars.next().expect("汉字字段非空");
            assert!(chars.next().is_none(), "汉字字段恰为一个字");
            (zi, reading, score.parse().expect("分数应为 u64"))
        })
        .collect()
}

#[test]
fn tsv_header_locks_pinned_source_identity() {
    let text = tsv();
    for line in [
        "# source_repo: amzxyz/rime-wanxiang",
        "# source_commit: 7ec998b28c9a5c57260d2ba24b264c1c1820e0ef",
        "# source_path: dicts/zi.dict.yaml",
        "# source_blob_sha: 9a69cb891f2e0c158313d14e0ea6c3925ca081ef",
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
    assert_eq!(rows.len(), 8544);
    assert!(
        text.contains(&format!("# rows: {}", rows.len())),
        "注释头行数应与实际数据行一致"
    );
}

#[test]
fn tsv_rows_are_strictly_ordered_unique_and_positive() {
    let text = tsv();
    let rows = data_rows(&text);
    let mut seen = BTreeSet::new();
    for (zi, reading, score) in &rows {
        assert!(*score > 0, "入库匹配行分数应为正");
        assert!(
            reading.bytes().all(|b| b.is_ascii_lowercase()),
            "读音应为小写 ASCII: {reading}"
        );
        assert!(seen.insert((*zi, *reading)), "无重复 (汉字, 读音) 行");
    }
    for pair in rows.windows(2) {
        assert!(
            (pair[0].0, pair[0].1) < (pair[1].0, pair[1].1),
            "行应按 (汉字, 读音) 严格升序"
        );
    }
}

#[test]
fn every_row_maps_to_canonical_relation() {
    for (zi, reading, _) in data_rows(&tsv()) {
        let hanzi = XhupHanzi::try_from(zi).expect("汉字应在规范清单内");
        assert!(
            hanzi.readings().iter().any(|r| r.as_str() == reading),
            "{zi} 的读音 {reading} 应为规范读音"
        );
    }
}

#[test]
fn reading_level_coverage_stays_at_audited_value() {
    let canonical: usize = XhupHanzi::all()
        .iter()
        .map(|hanzi| hanzi.readings().len())
        .sum();
    assert_eq!(canonical, 8580);
    let matched = data_rows(&tsv()).len();
    assert_eq!(matched, 8544, "覆盖审计锁定值(覆盖率 99.58%)");
    assert!(matched * 100 >= canonical * 98, "覆盖率应 >= 98%");
}

#[test]
fn provenance_documents_exist_and_reference_license() {
    let readme = std::fs::read_to_string(Path::new(FREQUENCY_DIR).join("README.md")).unwrap();
    for needle in [
        "amzxyz/rime-wanxiang",
        "7ec998b28c9a5c57260d2ba24b264c1c1820e0ef",
        "9a69cb891f2e0c158313d14e0ea6c3925ca081ef",
        "CC BY 4.0",
        "RIME-LMDG",
    ] {
        assert!(readme.contains(needle), "README 缺少 `{needle}`");
    }
    let license =
        std::fs::read_to_string(Path::new(FREQUENCY_DIR).join("LICENSE.wanxiang")).unwrap();
    assert!(license.contains("Attribution 4.0 International"));
}
