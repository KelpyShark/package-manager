/// KelpyShark Package Publisher
///
/// Publishes a local KelpyShark package to the file-based registry.
///
/// The package directory must contain a valid `kelpy.toml`.
/// It is copied into `~/.kelpyshark/registry/<name>/<version>/`.

use std::fs;
use std::path::Path;

use crate::manifest;
use crate::registry::Registry;

/// Publish the package in `project_dir` to the registry.
///
/// Returns a success message string, or an error description.
pub fn publish(project_dir: &Path, registry: &Registry) -> Result<String, String> {
    let toml_path = project_dir.join("kelpy.toml");
    if !toml_path.exists() {
        return Err(format!(
            "No kelpy.toml found in '{}'",
            project_dir.display()
        ));
    }

    let manifest = manifest::load(&toml_path)?;
    let name = &manifest.package.name;
    let version = &manifest.package.version;

    // Destination in registry
    let dest = registry.package_path(name, version);

    if dest.exists() {
        return Err(format!(
            "Package '{}@{}' is already published. \
             Bump the version in kelpy.toml to publish a new release.",
            name, version
        ));
    }

    fs::create_dir_all(&dest)
        .map_err(|e| format!("Could not create registry directory: {}", e))?;

    copy_dir_recursive(project_dir, &dest)?;

    Ok(format!(
        "Published {}@{} to local registry at '{}'",
        name,
        version,
        dest.display()
    ))
}

/// Recursively copy a directory (skipping target/ and libs/ to keep registry clean).
fn copy_dir_recursive(src: &Path, dest: &Path) -> Result<(), String> {
    fs::create_dir_all(dest)
        .map_err(|e| format!("Could not create '{}': {}", dest.display(), e))?;

    let entries = fs::read_dir(src)
        .map_err(|e| format!("Could not read '{}': {}", src.display(), e))?;

    for entry in entries {
        let entry = entry.map_err(|e| format!("Error: {}", e))?;
        let name = entry.file_name();
        let name_str = name.to_string_lossy();

        // Skip build artifacts and installed libs
        if matches!(name_str.as_ref(), "target" | "libs" | ".git") {
            continue;
        }

        let src_path = entry.path();
        let dest_path = dest.join(&name);

        if src_path.is_dir() {
            copy_dir_recursive(&src_path, &dest_path)?;
        } else {
            fs::copy(&src_path, &dest_path)
                .map_err(|e| format!("Could not copy '{}': {}", src_path.display(), e))?;
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::registry::Registry;

    #[test]
    fn test_publish_basic() {
        let tmp = tempfile::tempdir().unwrap();
        let registry = Registry::new(tmp.path().join("registry"));
        registry.init().unwrap();

        // Create a fake package
        let pkg_dir = tmp.path().join("my_lib");
        fs::create_dir_all(pkg_dir.join("src")).unwrap();
        fs::write(
            pkg_dir.join("kelpy.toml"),
            "[package]\nname = \"my_lib\"\nversion = \"1.0.0\"\ndescription = \"\"\n\n[dependencies]\n",
        ).unwrap();
        fs::write(pkg_dir.join("src").join("lib.ks"), "# lib").unwrap();

        let result = publish(&pkg_dir, &registry);
        assert!(result.is_ok(), "Publish failed: {:?}", result);
        assert!(result.unwrap().contains("Published my_lib@1.0.0"));

        // Verify it's in the registry
        let dest = registry.package_path("my_lib", "1.0.0");
        assert!(dest.exists());
        assert!(dest.join("kelpy.toml").exists());
    }

    #[test]
    fn test_publish_duplicate_fails() {
        let tmp = tempfile::tempdir().unwrap();
        let registry = Registry::new(tmp.path().join("registry"));
        registry.init().unwrap();

        let pkg_dir = tmp.path().join("lib");
        fs::create_dir_all(pkg_dir.join("src")).unwrap();
        fs::write(
            pkg_dir.join("kelpy.toml"),
            "[package]\nname = \"lib\"\nversion = \"1.0.0\"\ndescription = \"\"\n\n[dependencies]\n",
        ).unwrap();

        publish(&pkg_dir, &registry).unwrap();

        // Try to publish again
        let result = publish(&pkg_dir, &registry);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("already published"));
    }

    #[test]
    fn test_publish_no_toml() {
        let tmp = tempfile::tempdir().unwrap();
        let registry = Registry::new(tmp.path().join("registry"));

        let pkg_dir = tmp.path().join("empty");
        fs::create_dir_all(&pkg_dir).unwrap();

        let result = publish(&pkg_dir, &registry);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("kelpy.toml"));
    }
}
