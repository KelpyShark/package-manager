/// KelpyShark Package Registry
///
/// A file-based local registry for storing and retrieving packages.
/// Packages are stored as `.kslib` directories in a central registry folder.
///
/// Registry layout:
/// ```text
/// ~/.kelpyshark/registry/
///   http/
///     1.0.0/
///       kelpy.toml
///       src/
///         lib.ks
///   json/
///     0.2.1/
///       kelpy.toml
///       src/
///         lib.ks
/// ```

use std::path::{Path, PathBuf};
use std::collections::HashMap;
use std::fs;

use crate::manifest::{self, Manifest};

/// The local package registry.
pub struct Registry {
    /// Root directory of the registry.
    pub root: PathBuf,
}

impl Registry {
    /// Create a registry rooted at the default location (~/.kelpyshark/registry).
    pub fn default() -> Self {
        let home = dirs_or_fallback();
        Registry {
            root: home.join(".kelpyshark").join("registry"),
        }
    }

    /// Create a registry at a custom path (useful for testing).
    pub fn new(root: PathBuf) -> Self {
        Registry { root }
    }

    /// Initialize the registry directory if it doesn't exist.
    pub fn init(&self) -> Result<(), String> {
        fs::create_dir_all(&self.root)
            .map_err(|e| format!("Could not create registry at '{}': {}", self.root.display(), e))
    }

    /// List all available packages and their versions.
    pub fn list_packages(&self) -> Result<HashMap<String, Vec<String>>, String> {
        let mut packages: HashMap<String, Vec<String>> = HashMap::new();

        if !self.root.exists() {
            return Ok(packages);
        }

        let entries = fs::read_dir(&self.root)
            .map_err(|e| format!("Could not read registry: {}", e))?;

        for entry in entries {
            let entry = entry.map_err(|e| format!("Registry read error: {}", e))?;
            if entry.file_type().map_or(false, |t| t.is_dir()) {
                let pkg_name = entry.file_name().to_string_lossy().to_string();
                let mut versions = Vec::new();

                let version_entries = fs::read_dir(entry.path())
                    .map_err(|e| format!("Could not read package '{}': {}", pkg_name, e))?;

                for ver_entry in version_entries {
                    let ver_entry = ver_entry.map_err(|e| format!("Error: {}", e))?;
                    if ver_entry.file_type().map_or(false, |t| t.is_dir()) {
                        versions.push(ver_entry.file_name().to_string_lossy().to_string());
                    }
                }

                versions.sort();
                packages.insert(pkg_name, versions);
            }
        }

        Ok(packages)
    }

    /// Check if a specific package version exists in the registry.
    pub fn has_package(&self, name: &str, version: &str) -> bool {
        self.package_path(name, version).exists()
    }

    /// Get the path to a specific package version.
    pub fn package_path(&self, name: &str, version: &str) -> PathBuf {
        self.root.join(name).join(version)
    }

    /// Publish a package to the registry from a project directory.
    pub fn publish(&self, project_dir: &Path) -> Result<String, String> {
        self.init()?;

        let toml_path = project_dir.join("kelpy.toml");
        let manifest = manifest::load(&toml_path)?;

        let dest = self.package_path(&manifest.package.name, &manifest.package.version);

        if dest.exists() {
            return Err(format!(
                "Package '{}@{}' already exists in registry",
                manifest.package.name, manifest.package.version
            ));
        }

        // Copy project directory to registry
        copy_dir_recursive(project_dir, &dest)?;

        Ok(format!(
            "Published {}@{} to registry",
            manifest.package.name, manifest.package.version
        ))
    }

    /// Get a package manifest from the registry.
    pub fn get_manifest(&self, name: &str, version: &str) -> Result<Manifest, String> {
        let pkg_path = self.package_path(name, version);
        let toml_path = pkg_path.join("kelpy.toml");
        manifest::load(&toml_path)
    }
}

/// Recursively copy a directory.
fn copy_dir_recursive(src: &Path, dest: &Path) -> Result<(), String> {
    fs::create_dir_all(dest)
        .map_err(|e| format!("Could not create '{}': {}", dest.display(), e))?;

    let entries = fs::read_dir(src)
        .map_err(|e| format!("Could not read '{}': {}", src.display(), e))?;

    for entry in entries {
        let entry = entry.map_err(|e| format!("Error: {}", e))?;
        let src_path = entry.path();
        let dest_path = dest.join(entry.file_name());

        if src_path.is_dir() {
            // Skip target/ and .git/ directories
            let name = entry.file_name().to_string_lossy().to_string();
            if name == "target" || name == ".git" {
                continue;
            }
            copy_dir_recursive(&src_path, &dest_path)?;
        } else {
            fs::copy(&src_path, &dest_path)
                .map_err(|e| format!("Could not copy '{}': {}", src_path.display(), e))?;
        }
    }

    Ok(())
}

/// Get home directory with fallback.
fn dirs_or_fallback() -> PathBuf {
    if let Ok(home) = std::env::var("HOME") {
        PathBuf::from(home)
    } else if let Ok(userprofile) = std::env::var("USERPROFILE") {
        PathBuf::from(userprofile)
    } else {
        PathBuf::from(".")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_registry_init() {
        let tmp = tempfile::tempdir().unwrap();
        let registry = Registry::new(tmp.path().join("registry"));
        registry.init().unwrap();
        assert!(registry.root.exists());
    }

    #[test]
    fn test_list_empty_registry() {
        let tmp = tempfile::tempdir().unwrap();
        let registry = Registry::new(tmp.path().join("registry"));
        registry.init().unwrap();
        let packages = registry.list_packages().unwrap();
        assert!(packages.is_empty());
    }

    #[test]
    fn test_publish_and_find() {
        let tmp = tempfile::tempdir().unwrap();
        let registry = Registry::new(tmp.path().join("registry"));

        // Create a fake project
        let proj_dir = tmp.path().join("my_lib");
        fs::create_dir_all(proj_dir.join("src")).unwrap();
        fs::write(
            proj_dir.join("kelpy.toml"),
            "[package]\nname = \"my_lib\"\nversion = \"1.0.0\"\ndescription = \"test\"\n\n[dependencies]\n",
        )
        .unwrap();
        fs::write(proj_dir.join("src").join("lib.ks"), "print \"hello from lib\"").unwrap();

        // Publish
        let result = registry.publish(&proj_dir);
        assert!(result.is_ok(), "Publish failed: {:?}", result);

        // Verify
        assert!(registry.has_package("my_lib", "1.0.0"));
        let packages = registry.list_packages().unwrap();
        assert_eq!(packages.len(), 1);
        assert!(packages.contains_key("my_lib"));
    }

    #[test]
    fn test_duplicate_publish_fails() {
        let tmp = tempfile::tempdir().unwrap();
        let registry = Registry::new(tmp.path().join("registry"));

        let proj_dir = tmp.path().join("my_lib2");
        fs::create_dir_all(proj_dir.join("src")).unwrap();
        fs::write(
            proj_dir.join("kelpy.toml"),
            "[package]\nname = \"my_lib2\"\nversion = \"1.0.0\"\ndescription = \"\"\n\n[dependencies]\n",
        )
        .unwrap();

        registry.publish(&proj_dir).unwrap();
        let result = registry.publish(&proj_dir);
        assert!(result.is_err());
    }

    #[test]
    fn test_get_manifest() {
        let tmp = tempfile::tempdir().unwrap();
        let registry = Registry::new(tmp.path().join("registry"));

        let proj_dir = tmp.path().join("gettest");
        fs::create_dir_all(proj_dir.join("src")).unwrap();
        fs::write(
            proj_dir.join("kelpy.toml"),
            "[package]\nname = \"gettest\"\nversion = \"2.0.0\"\ndescription = \"desc\"\n\n[dependencies]\n",
        )
        .unwrap();

        registry.publish(&proj_dir).unwrap();
        let manifest = registry.get_manifest("gettest", "2.0.0").unwrap();
        assert_eq!(manifest.package.name, "gettest");
        assert_eq!(manifest.package.version, "2.0.0");
    }
}
