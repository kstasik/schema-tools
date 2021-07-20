use crate::error::Error;
use std::{collections::HashMap, fs, path::PathBuf};
use walkdir::WalkDir;

#[derive(Debug, Default, Clone)]
pub struct Discovered {
    pub templates: HashMap<String, String>,
    pub files: HashMap<String, PathBuf>,
}

#[derive(Debug, Default)]
pub struct Discovery {
    registries: HashMap<String, Registry>,
}

impl Discovery {
    pub fn register(&mut self, name: String, registry: Registry) {
        self.registries.insert(name, registry);
    }

    pub fn resolve(&self, tpls: &[String]) -> Result<Discovered, Error> {
        let mut templates: HashMap<String, String> = HashMap::new();
        let mut files: HashMap<String, PathBuf> = HashMap::new();

        // -----------------------+
        // formats:               |
        // -----------------------+
        // registry::.            |
        // registry::/path/ .     |
        // /path/                 |
        // -----------------------+
        for template in tpls {
            let parts = template.split("::").into_iter().collect::<Vec<&str>>();
            let realpath = if let [registry, path] = parts[..] {
                let r = self
                    .registries
                    .get(registry)
                    .ok_or_else(|| Error::DiscoveryNoRegistry(registry.to_string()))?;

                let mut p = r.path.clone();
                p.push(path);
                p
            } else if let [path] = parts[..] {
                PathBuf::from(path)
            } else {
                return Err(Error::NotImplemented);
            };

            let prefix = realpath.to_string_lossy();
            for entry in WalkDir::new(realpath.clone())
                .follow_links(false)
                .into_iter()
                .filter_map(|e| e.ok())
                .filter(|d| d.file_type().is_file() || d.file_type().is_symlink())
            {
                let relative = entry
                    .path()
                    .strip_prefix(prefix.to_string())
                    .unwrap()
                    .to_string_lossy();

                if relative.starts_with(".git/") {
                    continue;
                }

                let path = if entry.path_is_symlink() {
                    fs::read_link(entry.path()).map_err(Error::DiscoverySymlinkError)?
                } else {
                    entry.clone().into_path()
                };

                if relative.ends_with(".j2") {
                    let content = fs::read_to_string(path).map_err(Error::DiscoveryReadFile)?;
                    templates.insert(relative.to_string(), content);
                } else {
                    // full path
                    files.insert(relative.to_string(), path);
                }
            }
        }

        Ok(Discovered { templates, files })
    }
}

#[derive(Debug)]
pub struct Registry {
    pub path: PathBuf,
}

pub enum GitCheckoutType {
    Rev(String),
    Branch(String),
    Tag(String),
}

impl Registry {
    pub fn get_file(&self, path: &str) -> Result<String, Error> {
        let mut filepath = self.path.clone();
        filepath.push(path);

        fs::read_to_string(filepath).map_err(Error::DiscoveryReadFile)
    }
}

pub fn discover_git(
    repository: &str,
    source: GitCheckoutType,
    clean: bool,
) -> Result<Registry, Error> {
    let mut directory = std::env::temp_dir();
    let mut refspecs: Vec<String> = vec![];

    let revparse = match source {
        GitCheckoutType::Tag(tag) => {
            refspecs.push(format!("refs/tags/{0}:refs/remotes/origin/tags/{0}", tag));

            format!("refs/remotes/origin/tags/{0}", tag)
        }
        GitCheckoutType::Rev(rev) => {
            refspecs.push(String::from("refs/heads/*:refs/remotes/origin/*"));
            refspecs.push(String::from("HEAD:refs/remotes/origin/HEAD"));

            rev
        }
        GitCheckoutType::Branch(branch) => {
            refspecs.push(format!("refs/heads/{0}:refs/remotes/origin/{0}", branch));

            format!("refs/remotes/origin/{0}", branch)
        }
    };

    let digest = md5::compute(&revparse);
    directory.push("schema-tools");
    directory.push(format!("{:x}", digest));

    if directory.exists() && clean {
        fs::remove_dir_all(directory.as_path()).map_err(Error::DiscoveryCleanRegistryError)?;
    } else if directory.exists() {
        log::debug!("already exists: {:?}", directory);
        return Ok(Registry { path: directory });
    }

    log::debug!("checking out: {:?}", directory);

    let repo = git2::Repository::init(directory.clone()).map_err(Error::GitDiscoveryError)?;
    let mut opts = git2::FetchOptions::new();

    repo.remote_anonymous(repository)
        .map_err(Error::GitDiscoveryError)?
        .fetch(&refspecs, Some(&mut opts), None)
        .map_err(Error::GitDiscoveryError)?;

    let obj = repo
        .revparse_single(&revparse)
        .map_err(Error::GitDiscoveryError)?;

    repo.checkout_tree(&obj, None)
        .map_err(Error::GitDiscoveryError)?;

    Ok(Registry { path: directory })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_discovery_git_inherit_templates() {
        let mut discovery = Discovery::default();

        let registry = discover_git(
            "git://github.com/kstasik/schema-tools.git",
            GitCheckoutType::Rev("a279f3b54bc7b03af83162fbf027eb781db1e046".to_string()),
            false,
        )
        .unwrap();
        discovery.register("testing".to_string(), registry);

        let result = discovery
            .resolve(&vec![
                "testing::.".to_string(),
                "resources/test/discovery/test2/".to_string(),
            ])
            .unwrap();

        assert_eq!(result.files.len(), 1);
        assert_eq!(result.files.contains_key("README.md"), true);

        let content = fs::read_to_string(result.files.get("README.md").unwrap()).unwrap();
        assert_eq!(content, "# Schema Tools\n".to_string());

        let template = result.templates.get("test.j2").unwrap();
        assert_eq!(template, "# just test");
    }

    #[test]
    fn test_discovery_git_inherit() {
        let mut discovery = Discovery::default();

        let registry = discover_git(
            "git://github.com/kstasik/schema-tools.git",
            GitCheckoutType::Rev("a279f3b54bc7b03af83162fbf027eb781db1e046".to_string()),
            false,
        )
        .unwrap();
        discovery.register("testing".to_string(), registry);

        let result = discovery
            .resolve(&vec![
                "testing::.".to_string(),
                "resources/test/discovery/test1/".to_string(),
            ])
            .unwrap();

        assert_eq!(result.files.len(), 1);
        assert_eq!(result.files.contains_key("README.md"), true);

        let content = fs::read_to_string(result.files.get("README.md").unwrap()).unwrap();
        assert_eq!(content, "# just a test case".to_string());
    }

    #[test]
    fn test_discovery_git() {
        let mut discovery = Discovery::default();

        let registry = discover_git(
            "git://github.com/kstasik/schema-tools.git",
            GitCheckoutType::Rev("a279f3b54bc7b03af83162fbf027eb781db1e046".to_string()),
            false,
        )
        .unwrap();
        discovery.register("testing".to_string(), registry);

        let result = discovery.resolve(&vec!["testing::.".to_string()]).unwrap();

        assert_eq!(result.files.len(), 1);
        assert_eq!(result.files.contains_key("README.md"), true);
    }

    #[test]
    fn test_discovery_file() {
        let discovery = Discovery::default();

        let result = discovery
            .resolve(&vec!["./resources/test/".to_string()])
            .unwrap();

        assert_eq!(
            result.files.contains_key("json-schemas/01-simple.json"),
            true
        );
    }

    #[test]
    fn test_discover_git_hash() {
        let registry = discover_git(
            "git://github.com/kstasik/schema-tools.git",
            GitCheckoutType::Rev("a279f3b54bc7b03af83162fbf027eb781db1e046".to_string()),
            false,
        )
        .unwrap();

        let data = registry.get_file("README.md").unwrap();
        let expected = "# Schema Tools\n";

        assert_eq!(data, expected);
    }

    #[test]
    fn test_discover_git_branch() {
        let registry = discover_git(
            "git://github.com/kstasik/schema-tools.git",
            GitCheckoutType::Branch("bugfix/title-conflict".to_string()),
            false,
        )
        .unwrap();

        let data = registry.get_file("Cargo.toml").unwrap();

        let expected = r#"[package]
name = "schematools"
version = "0.1.0"
authors = ["Kacper Stasik <kacper@stasik.eu>"]
edition = "2018"

[dependencies]
clap = "3.0.0-beta.2"
serde_json = { version = "1", features = ["preserve_order"] }
serde_yaml = "0.8"
url = "2"
lazy_static = "1.4.0"
regex = "1"
thiserror = "1.0"
log = "0.4"
env_logger = "0.8.2"
jsonschema = "0.4"
reqwest = { version = ">= 0.10", features = ["blocking"] }
tera = { version = "1", default-features = false }
serde = { version = "1.0", features = ["derive"] }
walkdir = "2"
json-patch = "*"

[dev-dependencies]
test-case = "1""#;

        assert_eq!(data, expected);
    }

    #[test]
    fn test_discover_git_tag() {
        let registry = discover_git(
            "git://github.com/kstasik/schema-tools.git",
            GitCheckoutType::Tag("v0.0.1".to_string()),
            false,
        )
        .unwrap();

        let data = registry.get_file("Cargo.toml").unwrap();

        let expected = r#"[package]
name = "schematools"
version = "0.1.0"
authors = ["Kacper Stasik <kacper@stasik.eu>"]
edition = "2018"

[dependencies]
clap = "3.0.0-beta.2"
serde_json = "1"
serde_yaml = "0.8"
url = "2"
lazy_static = "1.4.0"
regex = "1"
thiserror = "1.0"
log = "0.4"
env_logger = "0.8.2"
jsonschema = "0.4"
reqwest = { version = ">= 0.10", features = ["blocking"] }

[dev-dependencies]
test-case = "1""#;

        assert_eq!(data, expected);
    }

    #[test]
    fn test_discover_git_tag_return_already_existing_registry() {
        testing_logger::setup();

        discover_git(
            "git://github.com/kstasik/schema-tools.git",
            GitCheckoutType::Tag("v0.0.2".to_string()),
            true,
        )
        .unwrap();
        discover_git(
            "git://github.com/kstasik/schema-tools.git",
            GitCheckoutType::Tag("v0.0.2".to_string()),
            false,
        )
        .unwrap();

        testing_logger::validate(|captured_logs| {
            assert_eq!(captured_logs.len(), 2);
            assert!(captured_logs[0].body.contains("checking out:"));
            assert!(captured_logs[1].body.contains("already exists:"));
        });
    }

    #[test]
    fn test_discover_git_tag_clean_existing_registry() {
        testing_logger::setup();

        discover_git(
            "git://github.com/kstasik/schema-tools.git",
            GitCheckoutType::Tag("v0.0.3".to_string()),
            true,
        )
        .unwrap();
        discover_git(
            "git://github.com/kstasik/schema-tools.git",
            GitCheckoutType::Tag("v0.0.3".to_string()),
            true,
        )
        .unwrap();

        testing_logger::validate(|captured_logs| {
            assert_eq!(captured_logs.len(), 2);
            assert!(captured_logs[0].body.contains("checking out:"));
            assert!(captured_logs[1].body.contains("checking out:"));
        });
    }
}
