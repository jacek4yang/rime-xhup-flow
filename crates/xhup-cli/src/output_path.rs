//! Keep generated packages outside the human-maintained Rime source tree.

use std::fs;
use std::io;
use std::path::{Component, Path, PathBuf};

use crate::CliError;

fn resolve_destination(path: &Path) -> io::Result<PathBuf> {
    let absolute = std::path::absolute(path)?;
    let mut ancestor = absolute.as_path();
    let mut missing = Vec::new();
    loop {
        match fs::canonicalize(ancestor) {
            Ok(mut resolved) => {
                for component in missing.into_iter().rev() {
                    match component {
                        Component::ParentDir => {
                            resolved.pop();
                        }
                        Component::CurDir => {}
                        Component::Normal(name) => resolved.push(name),
                        _ => {
                            return Err(io::Error::new(
                                io::ErrorKind::InvalidInput,
                                "invalid output root",
                            ));
                        }
                    }
                }
                return Ok(resolved);
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                let (Some(component), Some(parent)) =
                    (ancestor.components().next_back(), ancestor.parent())
                else {
                    return Err(error);
                };
                missing.push(component);
                ancestor = parent;
            }
            Err(error) => return Err(error),
        }
    }
}

pub(super) fn protect_source_tree(output: &Path, source_tree: &Path) -> Result<(), CliError> {
    let source = match fs::canonicalize(source_tree) {
        Ok(source) => Some(source),
        // A distributed CLI need not retain the checkout that built it.
        Err(error) if error.kind() == io::ErrorKind::NotFound => None,
        Err(source) => {
            return Err(CliError::OutputResolution {
                path: source_tree.to_owned(),
                source,
            });
        }
    };
    let resolved = resolve_destination(output).map_err(|source| CliError::OutputResolution {
        path: output.to_owned(),
        source,
    })?;
    if source
        .as_ref()
        .is_some_and(|source| resolved.starts_with(source))
        || resolved.ancestors().any(|directory| {
            directory
                .join("templates/xhup_flow.schema.yaml.in")
                .is_file()
        })
    {
        return Err(CliError::SourceOutput {
            path: output.to_owned(),
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn output_paths_cannot_overwrite_sources_or_create_intermediate_source_directories() {
        let root = std::env::temp_dir().join(format!("xhup-source-output-{}", std::process::id()));
        fs::create_dir(&root).unwrap();
        let source = root.join("rime");
        fs::create_dir_all(source.join("lua")).unwrap();
        fs::write(source.join("lua/module.lua"), "human-maintained source").unwrap();
        for output in [
            source.clone(),
            source.join("new/output"),
            root.join("missing/../rime"),
            source.join("new/../lua"),
        ] {
            assert!(matches!(
                protect_source_tree(&output, &source),
                Err(CliError::SourceOutput { .. })
            ));
        }
        assert!(!source.join("new").exists());
        assert!(!root.join("missing").exists());
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(&source, root.join("alias")).unwrap();
            assert!(matches!(
                protect_source_tree(&root.join("alias/new"), &source),
                Err(CliError::SourceOutput { .. })
            ));
            std::os::unix::fs::symlink(source.join("lua"), root.join("nested-alias")).unwrap();
            assert!(matches!(
                protect_source_tree(&root.join("nested-alias/../new"), &source),
                Err(CliError::SourceOutput { .. })
            ));
        }
        let foreign_source = root.join("other-checkout/rime");
        fs::create_dir_all(foreign_source.join("templates")).unwrap();
        fs::write(
            foreign_source.join("templates/xhup_flow.schema.yaml.in"),
            "source template",
        )
        .unwrap();
        assert!(matches!(
            protect_source_tree(&foreign_source.join("new"), &root.join("removed-checkout")),
            Err(CliError::SourceOutput { .. })
        ));
        let output = root.join("build/package");
        protect_source_tree(&output, &source).unwrap();
        crate::write_outputs(&output, &[("lua/module.lua", "compiled module")]).unwrap();
        assert_eq!(
            fs::read_to_string(output.join("lua/module.lua")).unwrap(),
            "compiled module"
        );
        assert_eq!(
            fs::read_to_string(source.join("lua/module.lua")).unwrap(),
            "human-maintained source"
        );
        fs::remove_dir_all(root).unwrap();
    }
}
