/// KelpyShark Dependency Resolver
///
/// Resolves dependency graphs using topological sorting.
/// Detects circular dependencies.

use std::collections::{HashMap, HashSet, VecDeque};

use crate::registry::Registry;

/// A resolved dependency with name and version.
#[derive(Debug, Clone, PartialEq)]
pub struct ResolvedDep {
    pub name: String,
    pub version: String,
}

/// Resolve all transitive dependencies for a set of direct dependencies.
///
/// Returns dependencies in install order (leaves first, dependents later).
pub fn resolve(
    direct_deps: &HashMap<String, String>,
    registry: &Registry,
) -> Result<Vec<ResolvedDep>, String> {
    let mut all_deps: HashMap<String, String> = HashMap::new();
    let mut graph: HashMap<String, Vec<String>> = HashMap::new();
    let mut visited: HashSet<String> = HashSet::new();

    // BFS to collect all transitive dependencies
    let mut queue: VecDeque<(String, String)> = VecDeque::new();

    for (name, version) in direct_deps {
        queue.push_back((name.clone(), version.clone()));
    }

    while let Some((name, version)) = queue.pop_front() {
        if visited.contains(&name) {
            continue;
        }
        visited.insert(name.clone());
        all_deps.insert(name.clone(), version.clone());

        // Look up this package's transitive dependencies in the registry
        if registry.has_package(&name, &version) {
            if let Ok(manifest) = registry.get_manifest(&name, &version) {
                let child_deps: Vec<String> = manifest.dependencies.keys().cloned().collect();
                graph.insert(name.clone(), child_deps.clone());

                for (dep_name, dep_version) in &manifest.dependencies {
                    if !visited.contains(dep_name) {
                        queue.push_back((dep_name.clone(), dep_version.clone()));
                    }
                }
            } else {
                graph.insert(name.clone(), vec![]);
            }
        } else {
            // Package not in registry — we'll still track it
            graph.insert(name.clone(), vec![]);
        }
    }

    // Topological sort (Kahn's algorithm)
    topological_sort(&graph, &all_deps)
}

/// Topological sort using Kahn's algorithm. Returns Err on circular dependency.
fn topological_sort(
    graph: &HashMap<String, Vec<String>>,
    versions: &HashMap<String, String>,
) -> Result<Vec<ResolvedDep>, String> {
    let mut in_degree: HashMap<String, usize> = HashMap::new();
    let mut nodes: HashSet<String> = HashSet::new();

    // Initialize all nodes
    for (node, deps) in graph {
        nodes.insert(node.clone());
        in_degree.entry(node.clone()).or_insert(0);
        for dep in deps {
            nodes.insert(dep.clone());
            *in_degree.entry(dep.clone()).or_insert(0) += 0;
            *in_degree.entry(node.clone()).or_insert(0) += 1;
        }
    }

    // Wait — in Kahn's algorithm, in-degree should count incoming edges.
    // graph[A] = [B, C] means A depends on B and C.
    // So B and C have incoming edges FROM A... no, the direction matters.
    //
    // Let's define: graph[A] = [B, C] means A depends on B, C.
    // Edge direction: A → B (A needs B). For install order, B must come before A.
    // So we need reverse topological order: sources (no dependencies) first.

    // Recompute in-degree properly:
    // An edge A → B means A depends on B, i.e., B is a prerequisite of A.
    // In-degree of A = number of dependencies A has = graph[A].len()
    // BUT we want to output B before A, so we want "reverse dependency" edges.
    // Actually in Kahn's for install order:
    //   - Node with 0 in-degree (no deps) gets installed first.
    //   - in-degree[X] = graph[X].len()

    let mut in_deg: HashMap<String, usize> = HashMap::new();
    // Build reverse adjacency: rev_adj[B] = [A] means "A depends on B"
    let mut rev_adj: HashMap<String, Vec<String>> = HashMap::new();

    for node in &nodes {
        in_deg.entry(node.clone()).or_insert(0);
        rev_adj.entry(node.clone()).or_default();
    }
    for (node, deps) in graph {
        in_deg.insert(node.clone(), deps.len());
        for dep in deps {
            rev_adj.entry(dep.clone()).or_default().push(node.clone());
        }
    }

    let mut queue: VecDeque<String> = VecDeque::new();
    for (node, deg) in &in_deg {
        if *deg == 0 {
            queue.push_back(node.clone());
        }
    }

    let mut result = Vec::new();

    while let Some(node) = queue.pop_front() {
        if let Some(version) = versions.get(&node) {
            result.push(ResolvedDep {
                name: node.clone(),
                version: version.clone(),
            });
        }

        if let Some(dependents) = rev_adj.get(&node) {
            for dependent in dependents {
                if let Some(deg) = in_deg.get_mut(dependent) {
                    *deg -= 1;
                    if *deg == 0 {
                        queue.push_back(dependent.clone());
                    }
                }
            }
        }
    }

    if result.len() < nodes.len() {
        return Err("Circular dependency detected".to_string());
    }

    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn setup_registry_with_packages(tmp: &std::path::Path) -> Registry {
        let registry = Registry::new(tmp.join("registry"));

        // Create package "base" v1.0.0 (no deps)
        let base_dir = tmp.join("base_pkg");
        fs::create_dir_all(base_dir.join("src")).unwrap();
        fs::write(
            base_dir.join("kelpy.toml"),
            "[package]\nname = \"base\"\nversion = \"1.0.0\"\ndescription = \"\"\n\n[dependencies]\n",
        ).unwrap();
        fs::write(base_dir.join("src").join("lib.ks"), "print \"base\"").unwrap();
        registry.publish(&base_dir).unwrap();

        // Create package "mid" v1.0.0 (depends on base)
        let mid_dir = tmp.join("mid_pkg");
        fs::create_dir_all(mid_dir.join("src")).unwrap();
        fs::write(
            mid_dir.join("kelpy.toml"),
            "[package]\nname = \"mid\"\nversion = \"1.0.0\"\ndescription = \"\"\n\n[dependencies]\nbase = \"1.0.0\"\n",
        ).unwrap();
        fs::write(mid_dir.join("src").join("lib.ks"), "print \"mid\"").unwrap();
        registry.publish(&mid_dir).unwrap();

        registry
    }

    #[test]
    fn test_resolve_single_dep() {
        let tmp = tempfile::tempdir().unwrap();
        let registry = setup_registry_with_packages(tmp.path());

        let mut deps = HashMap::new();
        deps.insert("base".to_string(), "1.0.0".to_string());

        let resolved = resolve(&deps, &registry).unwrap();
        assert_eq!(resolved.len(), 1);
        assert_eq!(resolved[0].name, "base");
    }

    #[test]
    fn test_resolve_transitive_deps() {
        let tmp = tempfile::tempdir().unwrap();
        let registry = setup_registry_with_packages(tmp.path());

        let mut deps = HashMap::new();
        deps.insert("mid".to_string(), "1.0.0".to_string());

        let resolved = resolve(&deps, &registry).unwrap();
        assert_eq!(resolved.len(), 2);
        // base should come before mid
        let names: Vec<&str> = resolved.iter().map(|d| d.name.as_str()).collect();
        let base_idx = names.iter().position(|n| *n == "base").unwrap();
        let mid_idx = names.iter().position(|n| *n == "mid").unwrap();
        assert!(base_idx < mid_idx, "base should be installed before mid");
    }

    #[test]
    fn test_resolve_empty_deps() {
        let tmp = tempfile::tempdir().unwrap();
        let registry = Registry::new(tmp.path().join("registry"));
        registry.init().unwrap();

        let deps: HashMap<String, String> = HashMap::new();
        let resolved = resolve(&deps, &registry).unwrap();
        assert!(resolved.is_empty());
    }

    #[test]
    fn test_resolve_unknown_package() {
        let tmp = tempfile::tempdir().unwrap();
        let registry = Registry::new(tmp.path().join("registry"));
        registry.init().unwrap();

        let mut deps = HashMap::new();
        deps.insert("nonexistent".to_string(), "1.0.0".to_string());

        // Should still succeed — package just won't be in the registry
        let resolved = resolve(&deps, &registry).unwrap();
        assert_eq!(resolved.len(), 1);
        assert_eq!(resolved[0].name, "nonexistent");
    }
}
