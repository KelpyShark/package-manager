/// KelpyShark Package Manager
///
/// Manages library installation, dependency resolution, and publishing.
///
/// Configuration file: `kelpy.toml`
/// Library format: `.kslib` directories under `libs/`
///
/// Capabilities:
///   - Parse kelpy.toml manifests
///   - Resolve dependencies (topological sort)
///   - Install packages (local file-based registry for now)
///   - Publish packages
///   - Update packages
///   - Lock file generation

pub mod manifest;
pub mod registry;
pub mod resolver;
pub mod installer;
pub mod publisher;
