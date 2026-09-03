//! 跨平台便携 Rime 源包的模板渲染与多产物生成。
//!
//! 便携核心只使用标准 librime 核心组件,不引用 Lua、OpenCC、
//! octagram、predict 等可选插件组件,也不包含任何前端私有配置;同一份
//! 生成包面向 ibus-rime、fcitx5-rime、Weasel、Squirrel、fcitx5-macos、
//! fcitx5-android 等主流前端。
//!
//! 当前方案是固定精确编码层:显式生成的编码直接精确查表,不做运行时
//! 拼写运算、候选补全枚举、组句与用户词学习。已提供一级简码(26 键,
//! 仅一键精确候选,不自动上屏)、固定层静态单字(2/3/4 码)与固定层
//! 静态高频词语(2~4 字词 4/6/8 键,逐字规范双拼两码按字序拼接;二字词
//! 仅收录不与规范四码单字全码冲突的 semantic entry)。更多简码、长短语
//! 与自适应连续输入将在后续数据层以独立层加入。
//!
//! 模板是 `rime/templates/*.yaml.in` 源文件,唯一占位符为 `{{VERSION}}`
//! (渲染为 crate 的 package version)。在相同规范数据、相同生成器源码
//! (含 package version)与相同模板下,生成结果字节级一致。

use crate::rime::{RIME_CHAR_DICTIONARY_FILENAME, generate_rime_char_dictionary};
use crate::rime_fixed_first_shortcuts::{
    RIME_FIXED_FIRST_SHORTCUT_DICTIONARY_FILENAME, generate_rime_fixed_first_shortcut_dictionary,
};
use crate::rime_flow::{
    RIME_FLOW_DICTIONARY_FILENAME, RIME_LEARN_DICTIONARY_FILENAME, generate_rime_flow_dictionary,
    generate_rime_learn_dictionary,
};
use crate::rime_shortcuts::{RIME_SHORTCUT_DICTIONARY_FILENAME, generate_rime_shortcut_dictionary};
use crate::rime_two_key_shortcuts::{
    RIME_TWO_KEY_SHORTCUT_DICTIONARY_FILENAME, generate_rime_two_key_shortcut_dictionary,
};
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

/// 顶层词典模板。
const DICTIONARY_TEMPLATE: &str = include_str!("../../../rime/templates/xhup_flow.dict.yaml.in");

/// 方案模板。
const SCHEMA_TEMPLATE: &str = include_str!("../../../rime/templates/xhup_flow.schema.yaml.in");

/// 静态兼容方案模板。
const STATIC_SCHEMA_TEMPLATE: &str =
    include_str!("../../../rime/templates/xhup_flow_static.schema.yaml.in");

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
/// (2/3/4 码)→ 词语简码词典(高稳健零冲突别名,3~7 键)→ 二码零冲突
/// 词语简码词典(2 键空码别名)→ 固定层词语词典(4/6/8 键)→ 顶层词典
/// (导入前五者)→ FIXED_FIRST 词语简码词典(高稳健重码别名,3/4/6 键,
/// 由方案中独立的第二 table_translator 加载,不被顶层词典导入)→ 方案
/// (使用前者)。同一规范数据、生成器源码与模板产生同一顺序、字节级一致
/// 的产物集合。
/// 生成完整的便携 Rime 源包产物集合。
///
/// 产物顺序固定且面向输入层级:一级简码词典(1 键)→ 单字全码词典
/// (2/3/4 码)→ 词语简码词典(高稳健零冲突别名,3~7 键)→ 二码零冲突
/// 词语简码词典(2 键空码别名)→ 固定层词语词典(4/6/8 键)→ 顶层词典
/// (导入前五者)→ FIXED_FIRST 词语简码词典(高稳健重码别名,3/4/6 键,
/// 由方案中独立的第二 table_translator 加载,不被顶层词典导入)→
/// Flow 词典(组句/学习专用,canonical 全码关系,无简码别名,由
/// table_translator@flow 加载,不被顶层词典导入)→ 主方案(Flow 引擎)
/// → 静态兼容方案(无 Flow translator 的回退)。同一规范数据、生成器
/// 源码与模板产生同一顺序、字节级一致的产物集合。
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
            filename: RIME_TWO_KEY_SHORTCUT_DICTIONARY_FILENAME,
            contents: generate_rime_two_key_shortcut_dictionary(),
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
            filename: RIME_SCHEMA_FILENAME,
            contents: render_template(SCHEMA_TEMPLATE, RIME_SCHEMA_FILENAME),
        },
        RimeArtifact {
            filename: RIME_STATIC_SCHEMA_FILENAME,
            contents: render_template(STATIC_SCHEMA_TEMPLATE, RIME_STATIC_SCHEMA_FILENAME),
        },
    ]
}
