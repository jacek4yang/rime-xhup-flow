//! #148 §2:文档不得与机器可读验收清单漂移。
//!
//! `docs/platform-acceptance.md` 的逐平台验收表格必须与
//! `release/acceptance-v2.0.0.json` 一致:同一批检查键、同样的状态值。
//! 修改清单而忘记同步文档(或反之)会让本测试失败,CI 门禁
//! (`validate-acceptance`)与文档共同构成 GA 发布依据。

use std::fs;
use std::path::Path;
use xhup_cli::acceptance::{self, CHECK_KEYS, CheckState};

const MANIFEST_PATH: &str = "release/acceptance-v2.0.0.json";
const DOC_PATH: &str = "docs/platform-acceptance.md";

fn repo_root() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("workspace root")
}

#[test]
fn acceptance_manifest_exists_and_parses() {
    let path = repo_root().join(MANIFEST_PATH);
    let json = fs::read_to_string(&path).expect("验收清单存在");
    let manifest = acceptance::parse_manifest(&json).expect("清单是合法 JSON");
    // 结构合法性:平台齐全、检查键齐全、工件引用一致。
    let violations = acceptance::check_rc(&manifest);
    assert!(violations.is_empty(), "清单结构/门禁违例: {violations:#?}");
}

#[test]
fn acceptance_doc_table_matches_manifest() {
    let manifest = {
        let json = fs::read_to_string(repo_root().join(MANIFEST_PATH)).expect("清单存在");
        acceptance::parse_manifest(&json).expect("清单合法")
    };
    let doc = fs::read_to_string(repo_root().join(DOC_PATH)).expect("验收文档存在");

    // 文档表格:每个检查键必须出现在某一行;行内的状态列数必须与平台数一致,
    // 且每个状态值与清单相同(按平台列序 windows/linux/macos/android)。
    let platform_order = ["Windows", "Linux", "macOS", "Android"];
    let table_rows: Vec<&str> = doc
        .lines()
        .filter(|line| line.starts_with("| ") && !line.contains("----"))
        .collect();

    // 第一行是表头,校验平台顺序未变。
    let header = table_rows.first().copied().unwrap_or("");
    for (idx, platform) in platform_order.iter().enumerate() {
        let column = header
            .split('|')
            .nth(idx + 2)
            .unwrap_or_else(|| panic!("表头缺少平台 {platform} 列"));
        assert!(
            column.contains(platform),
            "表头列 {idx} 应为 {platform},实际: {column}"
        );
    }

    // 键 → 行 的映射由文档的中文标签顺序固定;这里反过来校验:
    // 每个清单检查键的状态值必须出现在文档对应行。
    // 文档行按 CHECK_KEYS 顺序排列(第 1 行数据 = CHECK_KEYS[0],依此类推)。
    let data_rows = &table_rows[1..];
    assert_eq!(
        data_rows.len(),
        CHECK_KEYS.len(),
        "验收文档表格行数({})与检查键数({})不一致 —— 清单与文档漂移",
        data_rows.len(),
        CHECK_KEYS.len()
    );

    for (row_idx, key) in CHECK_KEYS.iter().enumerate() {
        let row = data_rows[row_idx];
        for (col_idx, platform) in platform_order.iter().enumerate() {
            let entry = manifest
                .platforms
                .iter()
                .find(|p| {
                    let doc_platform = match *platform {
                        "Windows" => "windows",
                        "Linux" => "linux",
                        "macOS" => "macos",
                        _ => "android",
                    };
                    p.platform == doc_platform
                })
                .unwrap_or_else(|| panic!("清单缺少平台 {platform}"));
            let expected = entry
                .checks
                .get(*key)
                .copied()
                .unwrap_or_else(|| panic!("清单平台 {platform} 缺少检查键 {key}"));
            let cell = row
                .split('|')
                .nth(col_idx + 2)
                .unwrap_or_else(|| panic!("文档行 {row_idx} 缺少列 {col_idx}"))
                .trim();
            let expected_str = expected.as_str();
            let cell_matches = match expected {
                CheckState::NotApplicable => cell.starts_with("N/A"),
                _ => cell.starts_with(expected_str),
            };
            assert!(
                cell_matches,
                "文档与清单漂移:{platform}/{key} 清单={expected_str},文档单元格=`{cell}`"
            );
        }
    }
}

#[test]
fn acceptance_doc_declares_machine_source() {
    let doc = fs::read_to_string(repo_root().join(DOC_PATH)).expect("验收文档存在");
    assert!(
        doc.contains("release/acceptance-v2.0.0.json"),
        "验收文档必须声明机器可读事实来源"
    );
}
