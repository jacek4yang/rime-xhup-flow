use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fs;
use std::io::Read;
use std::path::{Component, Path, PathBuf};

use serde::Deserialize;
use sha2::{Digest, Sha256};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Registry {
    production: Vec<String>,
    excluded: BTreeMap<String, String>,
}

fn files(directory: &Path, output: &mut BTreeSet<PathBuf>) -> Result<(), Box<dyn Error>> {
    println!("cargo:rerun-if-changed={}", directory.display());
    for entry in fs::read_dir(directory)? {
        let entry = entry?;
        if entry.file_name() == "__pycache__"
            || entry.path().extension().is_some_and(|ext| ext == "pyc")
        {
            continue;
        }
        let kind = entry.file_type()?;
        if kind.is_symlink() {
            return Err(format!(
                "build input cannot be a symlink: {}",
                entry.path().display()
            )
            .into());
        }
        if kind.is_dir() {
            files(&entry.path(), output)?;
        } else if kind.is_file() {
            output.insert(entry.path());
        } else {
            return Err(format!("unsupported build input: {}", entry.path().display()).into());
        }
    }
    Ok(())
}

fn main() -> Result<(), Box<dyn Error>> {
    let crate_dir = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR")?);
    let root = crate_dir.parent().unwrap().parent().unwrap();
    let lua_dir = root.join("rime/lua");
    let registry_file = lua_dir.join("modules.json");
    let registry: Registry = serde_json::from_slice(&fs::read(&registry_file)?)?;
    let mut registered = BTreeSet::new();
    let mut names = BTreeSet::new();
    let mut source = String::from("const LUA_SOURCES: &[LuaSource] = &[\n");
    for (relative, production) in registry
        .production
        .iter()
        .map(|path| (path, true))
        .chain(registry.excluded.keys().map(|path| (path, false)))
    {
        let path = Path::new(relative);
        if !relative.starts_with("xhup_flow/")
            || path.extension().and_then(|ext| ext.to_str()) != Some("lua")
            || !path
                .components()
                .all(|part| matches!(part, Component::Normal(_)))
            || relative.contains('\\')
        {
            return Err(format!("invalid registered Lua path: {relative}").into());
        }
        if !registered.insert(lua_dir.join(path)) {
            return Err(format!("duplicate Lua registration: {relative}").into());
        }
        let stem = relative.strip_suffix(".lua").unwrap();
        let module = stem.strip_suffix("/init").unwrap_or(stem).replace('/', ".");
        if !names.insert(module.clone()) {
            return Err(format!("duplicate Lua namespace: {module}").into());
        }
        if production {
            source.push_str(&format!(
                "LuaSource {{ module: {module:?}, filename: {:?}, source_path: {:?}, source: include_str!({:?}) }},\n",
                format!("lua/{relative}"), format!("rime/lua/{relative}"), lua_dir.join(path).to_str().ok_or("non-UTF-8 Lua path")?
            ));
        } else if registry.excluded[relative].trim().is_empty() {
            return Err(format!("excluded Lua module needs an explicit reason: {relative}").into());
        }
    }
    source.push_str("];\n");
    let mut lua_files = BTreeSet::new();
    files(&lua_dir, &mut lua_files)?;
    lua_files.retain(|path| path.extension().is_some_and(|ext| ext == "lua"));
    if lua_files != registered {
        return Err(format!(
            "Lua registry mismatch; unregistered: {:?}; missing: {:?}",
            lua_files.difference(&registered).collect::<Vec<_>>(),
            registered.difference(&lua_files).collect::<Vec<_>>()
        )
        .into());
    }

    // Bind production data and source metadata, not evaluation outputs or
    // redistribution-not-authorized research payloads.
    let mut inputs = BTreeSet::new();
    for directory in [
        "crates/xhup-core/src",
        "crates/xhup-generator/src",
        "crates/xhup-cli/src",
        "crates/xhup-analyzer/src",
        "crates/xhup-decoder/src",
        "vendor/full_moon/src",
        "rime/lua",
        "rime/templates",
        "rime/package",
        "data/double_pinyin",
        "data/education",
        "data/frequency",
        "data/hanzi",
        "data/pinyin",
        "data/shape",
        "data/shortcuts",
        "data/xhup/rules",
    ] {
        files(&root.join(directory), &mut inputs)?;
    }
    for relative in [
        ".gitattributes",
        "Cargo.toml",
        "Cargo.lock",
        "rust-toolchain.toml",
        "crates/xhup-core/Cargo.toml",
        "crates/xhup-generator/Cargo.toml",
        "crates/xhup-cli/Cargo.toml",
        "crates/xhup-analyzer/Cargo.toml",
        "crates/xhup-decoder/Cargo.toml",
        "crates/xhup-generator/build.rs",
        "vendor/full_moon/Cargo.toml",
        "vendor/full_moon/Cargo.toml.orig",
        "vendor/full_moon/README.md",
        "vendor/full_moon/LICENSE.md",
        "vendor/full_moon/XHUP-UPSTREAM.json",
        "data/xhup/sources.tsv",
        "data/xhup/attested_char_codes.tsv",
        "data/xhup/official_char_code_oracle.tsv",
        "data/xhup/README.md",
        "data/corpus/conversation_kdconv.tsv",
        "data/corpus/LICENSE.kdconv",
        "data/words/wanxiang_base_words.tsv",
        "data/words/wanxiang_extended_words.tsv",
        "data/words/LICENSE.wanxiang",
        "data/words/README.md",
    ] {
        inputs.insert(root.join(relative));
    }
    source.push_str("const BUILD_INPUTS: &[(&str, &str, u64)] = &[\n");
    let mut buffer = [0_u8; 65536];
    for path in inputs {
        println!("cargo:rerun-if-changed={}", path.display());
        let mut file = fs::File::open(&path)?;
        let mut hash = Sha256::new();
        let mut size = 0_u64;
        loop {
            let count = file.read(&mut buffer)?;
            if count == 0 {
                break;
            }
            hash.update(&buffer[..count]);
            size += count as u64;
        }
        let digest = format!("{:x}", hash.finalize());
        let relative = path
            .strip_prefix(root)?
            .to_str()
            .ok_or("non-UTF-8 build input")?
            .replace('\\', "/");
        source.push_str(&format!("({relative:?}, {digest:?}, {size}),\n"));
    }
    source.push_str("];\n");
    fs::write(
        PathBuf::from(std::env::var("OUT_DIR")?).join("runtime_sources.rs"),
        source,
    )?;
    Ok(())
}
