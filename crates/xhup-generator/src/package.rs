//! 跨平台便携 Rime 源包的模板渲染与多产物生成。
//!
//! 主方案依赖 librime-lua；独立 Static 方案保留无 Lua 的兼容回退。
//! 人类维护模板与 Lua 源码，生成器验证并构建可移植模块、词典和来源清单。
//! 不发布平台专用 Lua 字节码或构建主机的 Rime 部署二进制。
//!
//! 方案把冻结精确编码层与低质量 Flow translator 隔离：前者直接精确查表，
//! 后者以完整 pinned 词汇证据 + 逐字两键音码原语完成开放组句和本地学习。
//! 固定层包括一级简码、生产字符 2/3/4 码、canonical v2 词语简码与 hot
//! 词语；同码次序由 merged_ranking 显式仲裁，Flow fallback 只追加候选。
//!
//! 模板是 `rime/templates/*.yaml.in` 源文件,唯一占位符为 `{{VERSION}}`
//! (渲染为 crate 的 package version)。在相同规范数据、相同生成器源码
//! (含 package version)与相同模板下,生成结果字节级一致。

use crate::lua_build::{
    BUILD_STRATEGY, COMPILER_REVISION, LuaBuildError, NOTICE_SOURCES, build_inputs,
    compile_runtime, sha256,
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
use serde::Serialize;

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

const RUNTIME_MANIFEST_FILENAME: &str = "xhup_flow.runtime-manifest.json";

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

    /// 产物内容；生成代码使用 UTF-8/LF，许可证保留规范来源的原始文本。
    pub fn contents(&self) -> &str {
        &self.contents
    }
}

fn render_template(
    template: &str,
    name: &str,
    schema_id: Option<&str>,
    index_id: Option<&str>,
) -> Result<String, LuaBuildError> {
    let mut rendered = String::with_capacity(template.len());
    let mut rest = template;
    let mut versions = 0;
    let mut schemas = 0;
    let mut indexes = 0;
    while let Some(start) = rest.find("{{") {
        rendered.push_str(&rest[..start]);
        rest = &rest[start + 2..];
        let end = rest
            .find("}}")
            .ok_or_else(|| LuaBuildError::new(name, "unclosed template placeholder"))?;
        match &rest[..end] {
            "VERSION" => {
                versions += 1;
                rendered.push_str(env!("CARGO_PKG_VERSION"));
            }
            "SCHEMA_ID" if schema_id.is_some() => {
                schemas += 1;
                rendered.push_str(schema_id.expect("matched schema placeholder"));
            }
            "NATIVE_EDGE_INDEX_ID" if index_id.is_some() => {
                indexes += 1;
                rendered.push_str(index_id.expect("matched index placeholder"));
            }
            unknown => {
                return Err(LuaBuildError::new(
                    name,
                    format!("unknown placeholder {unknown}"),
                ));
            }
        }
        rest = &rest[end + 2..];
    }
    rendered.push_str(rest);
    if versions != 1
        || schema_id.is_some() != (schemas > 0)
        || indexes != usize::from(index_id.is_some())
        || rendered.contains("}}")
    {
        return Err(LuaBuildError::new(
            name,
            "missing, repeated, or malformed template placeholders",
        ));
    }
    Ok(rendered)
}

#[derive(Serialize)]
struct InputIdentity {
    path: &'static str,
    sha256: &'static str,
    bytes: u64,
}

#[derive(Serialize)]
struct ArtifactIdentity {
    path: &'static str,
    sha256: String,
    bytes: usize,
    source_inputs: [&'static str; 1],
    dependencies: std::collections::BTreeSet<String>,
    module: Option<&'static str>,
    source_sha256: Option<String>,
}

#[derive(Serialize)]
struct CompilerIdentity {
    strategy: &'static str,
    full_moon_revision: &'static str,
    lua_syntax: &'static str,
    bytecode: bool,
    source_line_offset: u8,
    generator_version: &'static str,
    cargo_lock_sha256: &'static str,
}

#[derive(Serialize)]
struct RuntimeManifest {
    schema_version: u8,
    compiler: CompilerIdentity,
    inputs: Vec<InputIdentity>,
    artifacts: Vec<ArtifactIdentity>,
}

/// Build the complete portable package before exposing any output to a writer.
/// The manifest lists direct recipes and registered repository input identities;
/// it does not hash itself. Release provenance binds the manifest's final bytes.
pub fn generate_rime_artifacts() -> Result<Vec<RimeArtifact>, LuaBuildError> {
    let flow_dictionary = generate_rime_flow_dictionary();
    let learn_dictionary = generate_rime_learn_dictionary();
    let (native_edges, index_id) =
        crate::rime_flow::generate_native_edge_index([&flow_dictionary, &learn_dictionary])?;
    let schema = render_template(SCHEMA_TEMPLATE, RIME_SCHEMA_FILENAME, None, Some(&index_id))?;
    let static_schema = render_template(
        STATIC_SCHEMA_TEMPLATE,
        RIME_STATIC_SCHEMA_FILENAME,
        None,
        None,
    )?;
    let dictionary = render_template(DICTIONARY_TEMPLATE, RIME_DICTIONARY_FILENAME, None, None)?;
    let flow_wrapper = render_template(
        DICT_COMPILE_WRAPPER_TEMPLATE,
        RIME_FLOW_WRAPPER_SCHEMA_FILENAME,
        Some("xhup_flow_flow"),
        None,
    )?;
    let learn_wrapper = render_template(
        DICT_COMPILE_WRAPPER_TEMPLATE,
        RIME_LEARN_WRAPPER_SCHEMA_FILENAME,
        Some("xhup_flow_learn"),
        None,
    )?;
    let lua_modules = compile_runtime(&[&schema, &static_schema], &native_edges)?;
    let mut artifacts = vec![
        RimeArtifact {
            filename: "default.custom.yaml",
            contents: include_str!("../../../rime/package/default.custom.yaml").to_owned(),
        },
        RimeArtifact {
            filename: "xhup_flow.sources.tsv",
            contents: format!(
                "# distribution-policy=clean-v1\n# research-only sources are metadata only; their payloads are excluded\n{}",
                include_str!("../../../data/xhup/sources.tsv")
            ),
        },
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
            contents: dictionary,
        },
        RimeArtifact {
            filename: RIME_FIXED_FIRST_SHORTCUT_DICTIONARY_FILENAME,
            contents: generate_rime_fixed_first_shortcut_dictionary(),
        },
        RimeArtifact {
            filename: RIME_FLOW_DICTIONARY_FILENAME,
            contents: flow_dictionary,
        },
        RimeArtifact {
            filename: RIME_LEARN_DICTIONARY_FILENAME,
            contents: learn_dictionary,
        },
        RimeArtifact {
            filename: RIME_FLOW_WRAPPER_SCHEMA_FILENAME,
            contents: flow_wrapper,
        },
        RimeArtifact {
            filename: RIME_LEARN_WRAPPER_SCHEMA_FILENAME,
            contents: learn_wrapper,
        },
        RimeArtifact {
            filename: RIME_SCHEMA_FILENAME,
            contents: schema,
        },
        RimeArtifact {
            filename: RIME_STATIC_SCHEMA_FILENAME,
            contents: static_schema,
        },
    ];
    let mut identities: Vec<_> = artifacts
        .iter()
        .map(|artifact| ArtifactIdentity {
            path: artifact.filename,
            sha256: sha256(artifact.contents.as_bytes()),
            bytes: artifact.contents.len(),
            source_inputs: [match artifact.filename {
                "default.custom.yaml" => "rime/package/default.custom.yaml",
                "xhup_flow.sources.tsv" => "data/xhup/sources.tsv",
                RIME_SCHEMA_FILENAME => "rime/templates/xhup_flow.schema.yaml.in",
                RIME_STATIC_SCHEMA_FILENAME => "rime/templates/xhup_flow_static.schema.yaml.in",
                RIME_DICTIONARY_FILENAME => "rime/templates/xhup_flow.dict.yaml.in",
                RIME_SHORTCUT_DICTIONARY_FILENAME => "crates/xhup-generator/src/rime_shortcuts.rs",
                RIME_CHAR_DICTIONARY_FILENAME => "crates/xhup-generator/src/rime.rs",
                RIME_WORD_SHORTCUT_DICTIONARY_FILENAME => {
                    "crates/xhup-generator/src/rime_word_shortcuts.rs"
                }
                RIME_WORD_DICTIONARY_FILENAME => "crates/xhup-generator/src/rime_words.rs",
                RIME_FIXED_FIRST_SHORTCUT_DICTIONARY_FILENAME => {
                    "crates/xhup-generator/src/rime_fixed_first_shortcuts.rs"
                }
                RIME_FLOW_DICTIONARY_FILENAME | RIME_LEARN_DICTIONARY_FILENAME => {
                    "crates/xhup-generator/src/rime_flow.rs"
                }
                _ => "crates/xhup-generator/src/package.rs",
            }],
            dependencies: std::collections::BTreeSet::new(),
            module: None,
            source_sha256: None,
        })
        .collect();
    for notice in NOTICE_SOURCES {
        identities.push(ArtifactIdentity {
            path: notice.filename,
            sha256: sha256(notice.source.as_bytes()),
            bytes: notice.source.len(),
            source_inputs: [notice.source_path],
            dependencies: std::collections::BTreeSet::new(),
            module: None,
            source_sha256: None,
        });
        artifacts.push(RimeArtifact {
            filename: notice.filename,
            contents: notice.source.to_owned(),
        });
    }
    for module in lua_modules {
        identities.push(ArtifactIdentity {
            path: module.filename,
            sha256: sha256(module.compiled.contents.as_bytes()),
            bytes: module.compiled.contents.len(),
            source_inputs: [module.source_path],
            dependencies: module.compiled.dependencies,
            module: Some(module.module),
            source_sha256: Some(module.compiled.source_sha256),
        });
        artifacts.push(RimeArtifact {
            filename: module.filename,
            contents: module.compiled.contents,
        });
    }
    identities.sort_unstable_by_key(|identity| identity.path);
    let mut inputs: Vec<_> = build_inputs()
        .map(|(path, sha256, bytes)| InputIdentity {
            path,
            sha256,
            bytes,
        })
        .collect();
    inputs.sort_unstable_by_key(|identity| identity.path);
    let cargo_lock_sha256 = inputs
        .iter()
        .find(|input| input.path == "Cargo.lock")
        .ok_or_else(|| {
            LuaBuildError::new(RUNTIME_MANIFEST_FILENAME, "missing Cargo.lock identity")
        })?
        .sha256;
    let manifest = RuntimeManifest {
        schema_version: 1,
        compiler: CompilerIdentity {
            strategy: BUILD_STRATEGY,
            full_moon_revision: COMPILER_REVISION,
            lua_syntax: "5.1",
            bytecode: false,
            source_line_offset: 1,
            generator_version: env!("CARGO_PKG_VERSION"),
            cargo_lock_sha256,
        },
        inputs,
        artifacts: identities,
    };
    let mut contents = serde_json::to_string_pretty(&manifest)
        .map_err(|error| LuaBuildError::new(RUNTIME_MANIFEST_FILENAME, error.to_string()))?;
    contents.push('\n');
    artifacts.push(RimeArtifact {
        filename: RUNTIME_MANIFEST_FILENAME,
        contents,
    });
    Ok(artifacts)
}

#[cfg(test)]
mod tests {
    use super::render_template;

    #[test]
    fn incomplete_or_unknown_template_contracts_fail_closed() {
        for (source, schema) in [
            ("version: {{VERSOIN}}", None),
            ("version: {{VERSION", None),
            ("version: {{VERSION}} {{VERSION}}", None),
            ("version: 1", None),
            ("version: {{VERSION}} }}", None),
            ("version: {{VERSION}}\nschema: {{SCHEMA_ID}}", None),
            ("version: {{VERSION}}", Some("xhup_flow")),
        ] {
            assert!(render_template(source, "fixture.yaml", schema, None).is_err());
        }
    }
}
