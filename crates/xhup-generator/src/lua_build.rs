//! Portable Lua compilation. No renaming, bytecode, or runtime build dependency.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::ops::Range;

use full_moon::tokenizer::{Token, TokenType};
use full_moon::visitors::Visitor;
use sha2::{Digest, Sha256};

pub(crate) const COMPILER_REVISION: &str = "83b6f2957ed63132abcb270dd6f75e685273c02b";
pub(crate) const BUILD_STRATEGY: &str = "portable-lua-v1";

#[derive(Debug)]
pub struct LuaBuildError {
    module: String,
    message: String,
}

impl LuaBuildError {
    pub(crate) fn new(module: &str, message: impl Into<String>) -> Self {
        Self {
            module: module.to_owned(),
            message: message.into(),
        }
    }
}

impl fmt::Display for LuaBuildError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.module, self.message)
    }
}

impl std::error::Error for LuaBuildError {}

pub(crate) struct CompiledLua {
    pub contents: String,
    pub dependencies: BTreeSet<String>,
    pub source_sha256: String,
}

// Store source spans, not copies of every string in generated data tables.
#[derive(Default)]
struct SourceTokens {
    semantic: Vec<Range<usize>>,
    comments: Vec<Range<usize>>,
    module_literals: BTreeMap<usize, String>,
}

fn valid_module_name(name: &str) -> bool {
    name == "xhup_flow"
        || name.strip_prefix("xhup_flow.").is_some_and(|suffix| {
            suffix.split('.').all(|part| {
                !part.is_empty()
                    && part.bytes().all(|byte| {
                        byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_'
                    })
                    && !part.as_bytes()[0].is_ascii_digit()
            })
        })
}

impl Visitor for SourceTokens {
    // The pinned parser calls generic visit_token for both TokenReference and
    // its Token. These kind-specific hooks visit each lexical token exactly once.
    fn visit_identifier(&mut self, token: &Token) {
        self.record_semantic(token);
    }

    fn visit_number(&mut self, token: &Token) {
        self.record_semantic(token);
    }

    fn visit_symbol(&mut self, token: &Token) {
        self.record_semantic(token);
    }

    fn visit_string_literal(&mut self, token: &Token) {
        self.record_semantic(token);
    }

    fn visit_single_line_comment(&mut self, token: &Token) {
        self.comments
            .push(token.start_position().bytes()..token.end_position().bytes());
    }

    fn visit_multi_line_comment(&mut self, token: &Token) {
        self.comments
            .push(token.start_position().bytes()..token.end_position().bytes());
    }
}

impl SourceTokens {
    fn record_semantic(&mut self, token: &Token) {
        let span = token.start_position().bytes()..token.end_position().bytes();
        if let TokenType::StringLiteral { literal, .. } = token.token_type()
            && valid_module_name(literal.as_str())
        {
            self.module_literals.insert(span.start, literal.to_string());
        }
        self.semantic.push(span);
    }

    fn parse(name: &str, source: &str) -> Result<Self, LuaBuildError> {
        // parse(), unlike parse_fallible(), never admits recovered/phantom tokens.
        let ast = full_moon::parse(source).map_err(|errors| {
            LuaBuildError::new(
                name,
                errors
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join("; "),
            )
        })?;
        let mut tokens = Self::default();
        tokens.visit_ast(&ast);
        // AST traversal order is not an input to the output format.
        for spans in [&mut tokens.semantic, &mut tokens.comments] {
            if !spans.windows(2).all(|pair| pair[0].end <= pair[1].start) {
                spans.sort_unstable_by_key(|span| span.start);
            }
            if !spans.windows(2).all(|pair| pair[0].end <= pair[1].start) {
                return Err(LuaBuildError::new(name, "overlapping parser token spans"));
            }
        }
        Ok(tokens)
    }

    fn dependencies(&self, name: &str, source: &str) -> Result<BTreeSet<String>, LuaBuildError> {
        let text = |index: usize| self.semantic.get(index).map(|span| &source[span.clone()]);
        let mut dependencies = BTreeSet::new();
        for (index, span) in self.semantic.iter().enumerate() {
            let token = &source[span.clone()];
            if matches!(
                token,
                "_G" | "_ENV"
                    | "package"
                    | "getfenv"
                    | "setfenv"
                    | "load"
                    | "loadfile"
                    | "loadstring"
                    | "dofile"
            ) {
                return Err(LuaBuildError::new(
                    name,
                    format!("environment/loader access {token} prevents static module resolution"),
                ));
            }
            if token != "require" {
                continue;
            }
            let qualified = index > 0 && matches!(text(index - 1), Some("." | ":"));
            let literal_index = if !qualified
                && text(index + 3) == Some(")")
                && (text(index + 1) == Some("(")
                    || (index >= 2
                        && text(index - 2) == Some("pcall")
                        && text(index - 1) == Some("(")
                        && text(index + 1) == Some(",")))
            {
                Some(index + 2)
            } else if !qualified
                && self
                    .semantic
                    .get(index + 1)
                    .is_some_and(|next| self.module_literals.contains_key(&next.start))
            {
                Some(index + 1)
            } else {
                None
            };
            let dependency = literal_index
                .and_then(|position| self.semantic.get(position))
                .and_then(|literal| self.module_literals.get(&literal.start))
                .ok_or_else(|| LuaBuildError::new(name, format!(
                    "require at byte {} must use a literal registered xhup_flow module; dynamic/aliased loaders are forbidden",
                    span.start
                )))?;
            if !dependencies.contains(dependency) {
                dependencies.insert(dependency.clone());
            }
        }
        Ok(dependencies)
    }
}

pub(crate) fn sha256(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

pub(crate) fn compile_lua_module(name: &str, source: &str) -> Result<CompiledLua, LuaBuildError> {
    if !valid_module_name(name) {
        return Err(LuaBuildError::new(name, "invalid module namespace"));
    }
    if source.contains('\r') || source.starts_with('\u{feff}') {
        return Err(LuaBuildError::new(
            name,
            "Lua sources must be UTF-8 without BOM and use LF newlines",
        ));
    }
    let original = SourceTokens::parse(name, source)?;
    let dependencies = original.dependencies(name, source)?;
    let source_sha256 = sha256(source.as_bytes());
    let mut contents = String::with_capacity(source.len() + 128);
    contents.push_str(&format!(
        "-- xhup-flow {BUILD_STRATEGY} source-sha256:{source_sha256}\n"
    ));
    let mut cursor = 0;
    for span in &original.comments {
        contents.push_str(&source[cursor..span.start]);
        // A separator prevents token fusion; line breaks retain source line mapping (+1 header).
        contents.push(' ');
        for byte in source.as_bytes()[span.clone()]
            .iter()
            .filter(|byte| **byte == b'\n')
        {
            contents.push(char::from(*byte));
        }
        cursor = span.end;
    }
    contents.push_str(&source[cursor..]);
    contents.truncate(contents.trim_end().len());
    contents.push('\n');
    let compiled = SourceTokens::parse(name, &contents)?;
    if !original
        .semantic
        .iter()
        .map(|span| &source[span.clone()])
        .eq(compiled.semantic.iter().map(|span| &contents[span.clone()]))
    {
        return Err(LuaBuildError::new(
            name,
            "compilation changed non-trivia tokens",
        ));
    }
    Ok(CompiledLua {
        contents,
        dependencies,
        source_sha256,
    })
}

struct LuaSource {
    module: &'static str,
    filename: &'static str,
    source_path: &'static str,
    source: &'static str,
}

include!(concat!(env!("OUT_DIR"), "/runtime_sources.rs"));

pub(crate) struct RuntimeLuaModule {
    pub module: &'static str,
    pub filename: &'static str,
    pub source_path: &'static str,
    pub compiled: CompiledLua,
}

pub(crate) fn build_inputs() -> impl Iterator<Item = (&'static str, &'static str, u64)> {
    BUILD_INPUTS.iter().copied()
}

fn visit_module(
    name: &str,
    modules: &BTreeMap<&'static str, RuntimeLuaModule>,
    states: &mut BTreeMap<&'static str, u8>,
    ordered: &mut Vec<&'static str>,
) -> Result<(), LuaBuildError> {
    let (&name, module) = modules
        .get_key_value(name)
        .ok_or_else(|| LuaBuildError::new(name, "required Lua module is not registered"))?;
    match states.get(name) {
        Some(2) => return Ok(()),
        Some(1) => return Err(LuaBuildError::new(name, "cyclic Lua module dependency")),
        _ => {}
    }
    states.insert(name, 1);
    for dependency in &module.compiled.dependencies {
        visit_module(dependency, modules, states, ordered)?;
    }
    states.insert(name, 2);
    ordered.push(name);
    Ok(())
}

pub(crate) fn compile_runtime(schemas: &[&str]) -> Result<Vec<RuntimeLuaModule>, LuaBuildError> {
    let mut modules = BTreeMap::new();
    let mut insert = |module, filename, source_path, source: &str| {
        let std::collections::btree_map::Entry::Vacant(slot) = modules.entry(module) else {
            return Err(LuaBuildError::new(module, "duplicate Lua module"));
        };
        let compiled = compile_lua_module(module, source)
            .map_err(|error| LuaBuildError::new(source_path, error.to_string()))?;
        slot.insert(RuntimeLuaModule {
            module,
            filename,
            source_path,
            compiled,
        });
        Ok(())
    };
    for source in LUA_SOURCES {
        insert(
            source.module,
            source.filename,
            source.source_path,
            source.source,
        )?;
    }
    insert(
        "xhup_flow.data.quick_hints",
        crate::lua_hints::LUA_QUICK_HINT_DATA_FILENAME,
        "crates/xhup-generator/src/lua_hints.rs",
        &crate::lua_hints::generate_lua_quick_hints_data(),
    )?;
    insert(
        "xhup_flow.data.build_info",
        "lua/xhup_flow/data/build_info.lua",
        "Cargo.toml",
        &format!(
            "return {{ version = {:?}, compiler_revision = {COMPILER_REVISION:?}, strategy = {BUILD_STRATEGY:?} }}\n",
            env!("CARGO_PKG_VERSION")
        ),
    )?;
    resolve_modules(modules, schemas)
}

fn resolve_modules(
    mut modules: BTreeMap<&'static str, RuntimeLuaModule>,
    schemas: &[&str],
) -> Result<Vec<RuntimeLuaModule>, LuaBuildError> {
    let mut states = BTreeMap::new();
    let mut ordered = Vec::new();
    visit_module("xhup_flow", &modules, &mut states, &mut ordered)?;
    for schema in schemas {
        for (line_number, line) in schema.lines().enumerate() {
            let line = line.trim();
            if line.starts_with('#') {
                continue;
            }
            if let Some(component) = line.strip_prefix("- lua_") {
                let (kind, name) = component.split_once("@*").ok_or_else(|| {
                    LuaBuildError::new(
                        "schema",
                        format!("line {}: Lua component must use @*module", line_number + 1),
                    )
                })?;
                if !matches!(kind, "filter" | "translator" | "processor" | "segmentor")
                    || !valid_module_name(name)
                {
                    return Err(LuaBuildError::new(
                        "schema",
                        format!(
                            "line {}: unsupported Lua component declaration",
                            line_number + 1
                        ),
                    ));
                }
                visit_module(name, &modules, &mut states, &mut ordered)?;
            } else if [
                "lua_filter@",
                "lua_translator@",
                "lua_processor@",
                "lua_segmentor@",
            ]
            .iter()
            .any(|marker| line.contains(marker))
            {
                return Err(LuaBuildError::new(
                    "schema",
                    format!(
                        "line {}: Lua roots require unquoted block-list declarations",
                        line_number + 1
                    ),
                ));
            }
        }
    }
    let reachable = ordered.len();
    // Unused production registrations still must parse and have a valid graph.
    // Only the root closure is emitted; research modules are never embedded.
    for name in modules.keys() {
        visit_module(name, &modules, &mut states, &mut ordered)?;
    }
    ordered.truncate(reachable);
    Ok(ordered
        .into_iter()
        .map(|name| modules.remove(name).expect("validated module graph"))
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn modules(sources: &[(&'static str, &str)]) -> BTreeMap<&'static str, RuntimeLuaModule> {
        sources
            .iter()
            .map(|&(name, source)| {
                (
                    name,
                    RuntimeLuaModule {
                        module: name,
                        filename: name,
                        source_path: name,
                        compiled: compile_lua_module(name, source).unwrap(),
                    },
                )
            })
            .collect()
    }

    #[test]
    fn graph_orders_shared_dependencies_once_and_omits_unreachable_modules() {
        let result = resolve_modules(
            modules(&[
                (
                    "xhup_flow",
                    "local b = require('xhup_flow.b'); return require('xhup_flow.c')",
                ),
                ("xhup_flow.b", "return require('xhup_flow.c')"),
                ("xhup_flow.c", "return {}"),
                ("xhup_flow.unused", "return {}"),
                ("xhup_flow.filter", "return require('xhup_flow.c')"),
            ]),
            &["filters:\n  - lua_filter@*xhup_flow.filter\n"],
        )
        .unwrap();
        assert_eq!(
            result
                .iter()
                .map(|module| module.module)
                .collect::<Vec<_>>(),
            [
                "xhup_flow.c",
                "xhup_flow.b",
                "xhup_flow",
                "xhup_flow.filter"
            ]
        );
    }

    #[test]
    fn missing_dependencies_and_cycles_fail_even_in_unreachable_modules() {
        for extra in [
            vec![("xhup_flow.a", "return require('xhup_flow.missing')")],
            vec![
                ("xhup_flow.a", "return require('xhup_flow.b')"),
                ("xhup_flow.b", "return require('xhup_flow.a')"),
            ],
        ] {
            let mut sources = vec![("xhup_flow", "return {}")];
            sources.extend(extra);
            assert!(resolve_modules(modules(&sources), &[]).is_err());
        }
    }

    #[test]
    fn missing_or_ambiguous_schema_roots_are_build_errors() {
        for schema in [
            "- lua_filter@*xhup_flow.missing",
            "- lua_filter@xhup_flow",
            "filters: [lua_filter@*xhup_flow]",
            "- 'lua_filter@*xhup_flow'",
        ] {
            assert!(resolve_modules(modules(&[("xhup_flow", "return {}")]), &[schema]).is_err());
        }
    }

    #[test]
    fn comments_cannot_change_literals_operators_or_closures() {
        let source = "-- header\nlocal text = [=[-- not a comment\n中文]=]\nlocal n = 1--[[separator]]+2\nlocal function f(x) return function() return x .. text end end\nreturn { n = n, f = f, quoted = '--still literal' } -- trailing\n";
        let result = compile_lua_module("xhup_flow.test", source).unwrap();
        assert!(result.contents.contains("[=[-- not a comment\n中文]=]"));
        assert!(result.contents.contains("quoted = '--still literal'"));
        assert!(!result.contents.contains("separator"));
        assert!(result.dependencies.is_empty());
    }

    #[test]
    fn records_literal_dependencies_including_protected_and_long_string_loads() {
        let result = compile_lua_module("xhup_flow.test", "local a = require('xhup_flow.a')\nlocal ok, b = pcall(require, \"xhup_flow.b\")\nreturn require[=[xhup_flow.c]=]\n").unwrap();
        assert_eq!(
            result.dependencies,
            BTreeSet::from([
                "xhup_flow.a".to_owned(),
                "xhup_flow.b".to_owned(),
                "xhup_flow.c".to_owned(),
            ])
        );
    }

    #[test]
    fn rejects_incomplete_syntax_and_unprovable_loaders() {
        for source in [
            "local x =",
            "return require(name)",
            "return require('xhup_flow.' .. name)",
            "local load = require; return load(name)",
            "return pcall(require, name)",
            "return env.require('xhup_flow.a')",
            "return require('socket')",
            "return _G['require']('xhup_flow.a')",
            "return getfenv()['require']('xhup_flow.a')",
            "return loadfile('module.lua')()",
        ] {
            assert!(
                compile_lua_module("xhup_flow.test", source).is_err(),
                "accepted {source}"
            );
        }
    }

    #[test]
    fn comment_only_changes_invalidate_the_artifact() {
        let first = compile_lua_module("xhup_flow.test", "return {} -- first\n").unwrap();
        let second = compile_lua_module("xhup_flow.test", "return {} -- second\n").unwrap();
        assert_ne!(first.source_sha256, second.source_sha256);
        assert_ne!(first.contents, second.contents);
        assert_eq!(
            first.contents.split_once('\n').unwrap().1,
            second.contents.split_once('\n').unwrap().1
        );
    }
}
