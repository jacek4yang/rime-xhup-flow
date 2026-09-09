//! 跨平台便携 Rime 源包的模板渲染与多产物生成。
//!
//! 便携核心的输入行为只使用标准 librime 核心组件;Lua 简码提示
//! (`lua_filter@*xhup_flow.quick_hint`)是**可选增强**:librime-lua
//! 缺失时 librime 跳过该组件,方案保持完整静态功能(docs/lua-runtime.md
//! §2 降级语义)。同一份生成包面向 ibus-rime、fcitx5-rime、Weasel、
//! Squirrel、fcitx5-macos、fcitx5-android 等主流前端。
//!
//! 方案把冻结精确编码层与低质量 Flow translator 隔离：前者直接精确查表，
//! 后者以完整 pinned 词汇证据 + 逐字两键音码原语完成开放组句和本地学习。
//! 固定层包括一级简码、生产字符 2/3/4 码、canonical v2 词语简码与 hot
//! 词语；同码次序由 merged_ranking 显式仲裁，Flow fallback 只追加候选。
//!
//! 模板是 `rime/templates/*.yaml.in` 源文件,唯一占位符为 `{{VERSION}}`
//! (渲染为 crate 的 package version)。在相同规范数据、相同生成器源码
//! (含 package version)与相同模板下,生成结果字节级一致。

use crate::lua_hints::{
    LUA_QUICK_HINT_DATA_FILENAME, LUA_QUICK_HINT_FILENAME, generate_lua_quick_hints_data,
    lua_quick_hint_source,
};
use crate::rime::{RIME_CHAR_DICTIONARY_FILENAME, generate_rime_char_dictionary};
use crate::rime_fixed_first_shortcuts::{
    RIME_FIXED_FIRST_SHORTCUT_DICTIONARY_FILENAME, generate_rime_fixed_first_shortcut_dictionary,
};
use crate::rime_flow::{
    RIME_FLOW_DICTIONARY_FILENAME, RIME_LEARN_DICTIONARY_FILENAME, generate_rime_flow_dictionary,
    generate_rime_learn_dictionary,
};
use crate::rime_shortcuts::{RIME_SHORTCUT_DICTIONARY_FILENAME, generate_rime_shortcut_dictionary};
use crate::rime_word_shortcuts::{
    RIME_WORD_SHORTCUT_DICTIONARY_FILENAME, generate_rime_word_shortcut_dictionary,
};
use crate::rime_words::{RIME_WORD_DICTIONARY_FILENAME, generate_rime_word_dictionary};

/// 顶层词典产物文件名。
const RIME_DICTIONARY_FILENAME: &str = "xhup_flow.dict.yaml";

/// 方案产物文件名。
const RIME_SCHEMA_FILENAME: &str = "xhup_flow.schema.yaml";

/// 静态兼容方案产物文件名(Flow 引擎的静态回退,不重复静态词典)。
const RIME_STATIC_SCHEMA_FILENAME: &str = "xhup_flow_static.schema.yaml";

/// 辅助词典的「编译 wrapper」schema(见 generate_rime_artifacts 文档)。
const RIME_FLOW_WRAPPER_SCHEMA_FILENAME: &str = "xhup_flow_flow.schema.yaml";
const RIME_LEARN_WRAPPER_SCHEMA_FILENAME: &str = "xhup_flow_learn.schema.yaml";

/// 顶层词典模板。
const DICTIONARY_TEMPLATE: &str = include_str!("../../../rime/templates/xhup_flow.dict.yaml.in");

/// 方案模板。
const SCHEMA_TEMPLATE: &str = include_str!("../../../rime/templates/xhup_flow.schema.yaml.in");

/// 静态兼容方案模板。
const STATIC_SCHEMA_TEMPLATE: &str =
    include_str!("../../../rime/templates/xhup_flow_static.schema.yaml.in");

/// 编译 wrapper schema 模板:librime 部署只编译「默认 translator 命名
/// 空间」的词典;`table_translator@flow` / `@learn`
/// 引用的词典必须各自有一个同名 wrapper schema(经主方案的
/// `schema/dependencies` 官方机制参与部署),Weasel / rime_deployer
/// 才会为它们生成 table.bin。真机部署验收发现:缺失时 Flow 组句与
/// 本地学习会静默失效。wrapper 不进入任何
/// schema_list,不可被用户选择。
const DICT_COMPILE_WRAPPER_TEMPLATE: &str = r#"# Rime schema
# encoding: utf-8
---
schema:
  schema_id: "{{SCHEMA_ID}}"
  name: "{{SCHEMA_ID}}(词典编译 wrapper)"
  version: "{{VERSION}}"
  description: |
    仅供部署器编译 {{SCHEMA_ID}} 词典;不进入 schema_list,不可选择。
  dependencies: []

engine:
  translators:
    - table_translator

translator:
  dictionary: {{SCHEMA_ID}}
  enable_completion: false
  enable_sentence: false
  enable_user_dict: false
"#;

/// 模板中唯一的占位符。
const VERSION_PLACEHOLDER: &str = "{{VERSION}}";

/// 便携 Rime 源包中的一个生成产物。
///
/// 文件名与内容均由生成器拥有;调用方(如 CLI)只负责落盘,
/// 不了解任何 Rime 语义。
#[derive(Debug)]
pub struct RimeArtifact {
    filename: &'static str,
    contents: String,
}

impl RimeArtifact {
    /// 产物文件名(相对包根)。
    pub fn filename(&self) -> &'static str {
        self.filename
    }

    /// 产物内容(UTF-8、LF 换行、恰好一个末尾换行)。
    pub fn contents(&self) -> &str {
        &self.contents
    }
}

/// 渲染模板:把恰好一次的 `{{VERSION}}` 替换为 package version。
///
/// 模板是入库源文件,占位符缺失/多余或渲染后残留 `{{` 均属源码级错误,
/// 直接断言失败。
fn render_template(template: &str, name: &str) -> String {
    let count = template.matches(VERSION_PLACEHOLDER).count();
    assert_eq!(
        count, 1,
        "{name}: 模板应恰含一个 {VERSION_PLACEHOLDER} 占位符"
    );
    let rendered = template.replace(VERSION_PLACEHOLDER, env!("CARGO_PKG_VERSION"));
    assert!(!rendered.contains("{{"), "{name}: 渲染后存在未解析占位符");
    rendered
}

/// 生成完整的便携 Rime 源包产物集合。
///
/// 产物顺序固定且面向输入层级:一级简码词典(1 键)→ 单字全码词典
/// (2/3/4 码)→ PRIMARY 词语简码词典(2~5 键)→ FIXED_FIRST 词语简码
/// 词典(3~5 键)→ 固定层词语词典(4/6/8 键)→ 顶层词典(导入前五者)→
/// Flow 词典(组句专用,完整词汇关系 + 单字音码原语,无简码别名,由
/// table_translator@flow 加载,不被顶层词典导入)→ 两个辅助词典的编译
/// wrapper schema → Lua 简码提示模块与数据(librime-lua `*module` 组件,
/// 可选增强,缺失时主方案降级为纯静态行为)→ 主方案(Flow 引擎)→
/// 静态兼容方案(无 Flow translator 的回退)。同一规范数据、生成器
/// 源码与模板产生同一顺序、字节级一致的产物集合。
/// 渲染辅助词典的编译 wrapper schema。
fn render_dict_compile_wrapper(schema_id: &str) -> String {
    let version = env!("CARGO_PKG_VERSION");
    DICT_COMPILE_WRAPPER_TEMPLATE
        .replace("{{SCHEMA_ID}}", schema_id)
        .replace("{{VERSION}}", version)
}

pub fn generate_rime_artifacts() -> Vec<RimeArtifact> {
    vec![
        RimeArtifact {
            filename: RIME_SHORTCUT_DICTIONARY_FILENAME,
            contents: generate_rime_shortcut_dictionary(),
        },
        RimeArtifact {
            filename: RIME_CHAR_DICTIONARY_FILENAME,
            contents: generate_rime_char_dictionary(),
        },
        RimeArtifact {
            filename: RIME_WORD_SHORTCUT_DICTIONARY_FILENAME,
            contents: generate_rime_word_shortcut_dictionary(),
        },
        RimeArtifact {
            filename: RIME_WORD_DICTIONARY_FILENAME,
            contents: generate_rime_word_dictionary(),
        },
        RimeArtifact {
            filename: RIME_DICTIONARY_FILENAME,
            contents: render_template(DICTIONARY_TEMPLATE, RIME_DICTIONARY_FILENAME),
        },
        RimeArtifact {
            filename: RIME_FIXED_FIRST_SHORTCUT_DICTIONARY_FILENAME,
            contents: generate_rime_fixed_first_shortcut_dictionary(),
        },
        RimeArtifact {
            filename: RIME_FLOW_DICTIONARY_FILENAME,
            contents: generate_rime_flow_dictionary(),
        },
        RimeArtifact {
            filename: RIME_LEARN_DICTIONARY_FILENAME,
            contents: generate_rime_learn_dictionary(),
        },
        RimeArtifact {
            filename: RIME_FLOW_WRAPPER_SCHEMA_FILENAME,
            contents: render_dict_compile_wrapper("xhup_flow_flow"),
        },
        RimeArtifact {
            filename: RIME_LEARN_WRAPPER_SCHEMA_FILENAME,
            contents: render_dict_compile_wrapper("xhup_flow_learn"),
        },
        RimeArtifact {
            filename: LUA_QUICK_HINT_FILENAME,
            contents: lua_quick_hint_source().to_string(),
        },
        RimeArtifact {
            filename: LUA_QUICK_HINT_DATA_FILENAME,
            contents: generate_lua_quick_hints_data(),
        },
        RimeArtifact {
            filename: RIME_SCHEMA_FILENAME,
            contents: render_template(SCHEMA_TEMPLATE, RIME_SCHEMA_FILENAME),
        },
        RimeArtifact {
            filename: RIME_STATIC_SCHEMA_FILENAME,
            contents: render_template(STATIC_SCHEMA_TEMPLATE, RIME_STATIC_SCHEMA_FILENAME),
        },
    ]
}
