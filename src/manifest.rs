/// KelpyShark Package Manifest (`kelpy.toml`) parser.
///
/// Parses the project configuration file that defines package metadata and dependencies.
///
/// Format:
/// ```toml
/// [package]
/// name = "my_project"
/// version = "0.1.0"
/// description = "A cool project"
///
/// [dependencies]
/// http = "1.0.0"
/// json = "0.2.1"
/// ```

use std::collections::HashMap;
use std::path::Path;

/// A parsed kelpy.toml manifest.
#[derive(Debug, Clone, PartialEq)]
pub struct Manifest {
    pub package: PackageInfo,
    pub dependencies: HashMap<String, String>,
}

/// Package metadata from the [package] section.
#[derive(Debug, Clone, PartialEq)]
pub struct PackageInfo {
    pub name: String,
    pub version: String,
    pub description: String,
}

/// Parse a kelpy.toml file from disk.
pub fn load(path: &Path) -> Result<Manifest, String> {
    let content = std::fs::read_to_string(path)
        .map_err(|e| format!("Could not read '{}': {}", path.display(), e))?;
    parse(&content)
}

/// Parse kelpy.toml content from a string.
///
/// This is a simple TOML subset parser — it only handles
/// `[section]` headers and `key = "value"` pairs.
pub fn parse(content: &str) -> Result<Manifest, String> {
    let mut package_name = String::new();
    let mut package_version = String::new();
    let mut package_description = String::new();
    let mut dependencies: HashMap<String, String> = HashMap::new();

    let mut current_section = String::new();

    for (line_num, line) in content.lines().enumerate() {
        let line = line.trim();

        // Skip empty lines and comments
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        // Section header
        if line.starts_with('[') && line.ends_with(']') {
            current_section = line[1..line.len() - 1].trim().to_string();
            continue;
        }

        // Key-value pair
        if let Some(eq_pos) = line.find('=') {
            let key = line[..eq_pos].trim().to_string();
            let raw_value = line[eq_pos + 1..].trim();

            // Strip quotes
            let value = if raw_value.starts_with('"') && raw_value.ends_with('"') && raw_value.len() >= 2 {
                raw_value[1..raw_value.len() - 1].to_string()
            } else {
                raw_value.to_string()
            };

            match current_section.as_str() {
                "package" => match key.as_str() {
                    "name" => package_name = value,
                    "version" => package_version = value,
                    "description" => package_description = value,
                    _ => {} // Ignore unknown keys
                },
                "dependencies" => {
                    dependencies.insert(key, value);
                }
                _ => {} // Ignore unknown sections
            }
        } else {
            return Err(format!(
                "kelpy.toml line {}: expected key = value, got: {}",
                line_num + 1,
                line
            ));
        }
    }

    if package_name.is_empty() {
        return Err("kelpy.toml: missing [package] name".to_string());
    }

    Ok(Manifest {
        package: PackageInfo {
            name: package_name,
            version: if package_version.is_empty() {
                "0.1.0".to_string()
            } else {
                package_version
            },
            description: package_description,
        },
        dependencies,
    })
}

/// Serialize a Manifest back to kelpy.toml format.
pub fn serialize(manifest: &Manifest) -> String {
    let mut out = String::new();
    out.push_str("[package]\n");
    out.push_str(&format!("name = \"{}\"\n", manifest.package.name));
    out.push_str(&format!("version = \"{}\"\n", manifest.package.version));
    out.push_str(&format!(
        "description = \"{}\"\n",
        manifest.package.description
    ));
    out.push('\n');
    out.push_str("[dependencies]\n");
    for (name, version) in &manifest.dependencies {
        out.push_str(&format!("{} = \"{}\"\n", name, version));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_basic_manifest() {
        let content = r#"
[package]
name = "my_project"
version = "1.0.0"
description = "A KelpyShark project"

[dependencies]
http = "1.0.0"
json = "0.2.1"
"#;
        let manifest = parse(content).unwrap();
        assert_eq!(manifest.package.name, "my_project");
        assert_eq!(manifest.package.version, "1.0.0");
        assert_eq!(manifest.package.description, "A KelpyShark project");
        assert_eq!(manifest.dependencies.len(), 2);
        assert_eq!(manifest.dependencies.get("http"), Some(&"1.0.0".to_string()));
        assert_eq!(manifest.dependencies.get("json"), Some(&"0.2.1".to_string()));
    }

    #[test]
    fn test_parse_minimal_manifest() {
        let content = r#"
[package]
name = "hello"
"#;
        let manifest = parse(content).unwrap();
        assert_eq!(manifest.package.name, "hello");
        assert_eq!(manifest.package.version, "0.1.0"); // default
        assert!(manifest.dependencies.is_empty());
    }

    #[test]
    fn test_parse_missing_name() {
        let content = "[package]\nversion = \"1.0.0\"";
        assert!(parse(content).is_err());
    }

    #[test]
    fn test_parse_with_comments() {
        let content = r#"
# Project config
[package]
name = "test"
version = "0.1.0"
# This is a comment
description = ""

[dependencies]
# no deps yet
"#;
        let manifest = parse(content).unwrap();
        assert_eq!(manifest.package.name, "test");
    }

    #[test]
    fn test_serialize_roundtrip() {
        let mut deps = HashMap::new();
        deps.insert("http".to_string(), "1.0.0".to_string());

        let manifest = Manifest {
            package: PackageInfo {
                name: "test".to_string(),
                version: "1.0.0".to_string(),
                description: "A test".to_string(),
            },
            dependencies: deps,
        };

        let serialized = serialize(&manifest);
        let reparsed = parse(&serialized).unwrap();
        assert_eq!(reparsed.package.name, "test");
        assert_eq!(reparsed.package.version, "1.0.0");
        assert_eq!(reparsed.dependencies.get("http"), Some(&"1.0.0".to_string()));
    }
}
