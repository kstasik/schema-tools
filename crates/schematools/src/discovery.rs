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
            let parts = template.split("::").collect::<Vec<&str>>();
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

                if relative.starts_with(".git") {
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
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }

    pub fn get_file(&self, path: &str) -> Result<String, Error> {
        let mut filepath = self.path.clone();
        filepath.push(path);

        fs::read_to_string(filepath).map_err(Error::DiscoveryReadFile)
    }
}

#[cfg(feature = "git")]
pub fn discover_git(
    repository: &str,
    source: GitCheckoutType,
    no_cache: bool,
) -> Result<Registry, Error> {
    // On Rust 1.89+ `std::fs::File::lock` is stable and takes precedence over
    // the trait method via inherent-method resolution, so this import is only
    // needed to support older toolchains.
    #[allow(unused_imports)]
    use fs4::FileExt;

    let mut directory = std::env::temp_dir();

    let revparse = match &source {
        GitCheckoutType::Tag(tag) => format!("refs/tags/{tag}"),
        GitCheckoutType::Rev(rev) => rev.clone(),
        GitCheckoutType::Branch(branch) => format!("refs/heads/{branch}"),
    };

    use md5::{Digest, Md5};
    let digest = Md5::digest(&revparse);
    let digest_bytes: &[u8] = digest.as_ref();
    let digest_string = digest_bytes
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect::<String>();
    directory.push("schema-tools");
    fs::create_dir_all(&directory).map_err(Error::DiscoveryCacheRegistryError)?; // create

    let lock = std::fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(directory.join(format!("{digest_string}.lock")))
        .map_err(Error::DiscoveryLockError)?;

    lock.lock().map_err(Error::DiscoveryLockError)?;
    directory.push(digest_string);

    if directory.exists() && no_cache {
        fs::remove_dir_all(directory.as_path()).map_err(Error::DiscoveryCleanRegistryError)?;
    } else if directory.exists() {
        log::debug!("already exists: {:?}", directory);
        return Ok(Registry::new(directory));
    }

    if let Some(parent) = directory.parent() {
        fs::create_dir_all(parent).map_err(Error::DiscoveryCacheRegistryError)?;
    }

    log::debug!("checking out: {:?}", directory);

    // For branches/tags we can hint gix to fetch a specific ref; for arbitrary
    // revisions we fetch the default set and resolve the object locally.
    let ref_name: Option<&str> = match &source {
        GitCheckoutType::Branch(b) => Some(b.as_str()),
        GitCheckoutType::Tag(t) => Some(t.as_str()),
        GitCheckoutType::Rev(_) => None,
    };

    let mut prepare = gix::prepare_clone(repository, &directory)
        .map_err(|e| Error::GitDiscoveryError(e.to_string()))?;
    if let Some(name) = ref_name {
        prepare = prepare
            .with_ref_name(Some(name))
            .map_err(|e| Error::GitDiscoveryError(e.to_string()))?;
    }

    let interrupt = std::sync::atomic::AtomicBool::new(false);
    let (repo, _outcome) = prepare
        .fetch_only(gix::progress::Discard, &interrupt)
        .map_err(|e| Error::GitDiscoveryError(e.to_string()))?;

    let target = match &source {
        GitCheckoutType::Rev(rev) => repo
            .rev_parse_single(rev.as_str())
            .map_err(|e| Error::GitDiscoveryError(e.to_string()))?,
        GitCheckoutType::Branch(branch) => repo
            .rev_parse_single(format!("refs/remotes/origin/{branch}").as_str())
            .map_err(|e| Error::GitDiscoveryError(e.to_string()))?,
        GitCheckoutType::Tag(tag) => repo
            .rev_parse_single(format!("refs/tags/{tag}").as_str())
            .map_err(|e| Error::GitDiscoveryError(e.to_string()))?,
    }
    .detach();

    checkout_commit(&repo, target).map_err(Error::GitDiscoveryError)?;

    Ok(Registry::new(directory))
}

#[cfg(feature = "git")]
fn checkout_commit(repo: &gix::Repository, target: gix::ObjectId) -> Result<(), String> {
    use gix::refs::transaction::{Change, LogChange, PreviousValue, RefEdit, RefLog};
    use gix::refs::Target;

    let workdir = repo
        .workdir()
        .ok_or_else(|| "repository has no working tree".to_string())?;

    let tree_id = repo
        .find_object(target)
        .map_err(|e| e.to_string())?
        .peel_to_kind(gix::object::Kind::Tree)
        .map_err(|e| e.to_string())?
        .id();

    let validate = gix::validate::path::component::Options::default();
    let state = gix::index::State::from_tree(&tree_id, &repo.objects, validate)
        .map_err(|e| e.to_string())?;
    let mut index = gix::index::File::from_state(state, repo.index_path());

    // Detach HEAD at the requested commit.
    let edit = RefEdit {
        change: Change::Update {
            log: LogChange {
                mode: RefLog::AndReference,
                force_create_reflog: false,
                message: format!("checkout {target}").into(),
            },
            expected: PreviousValue::Any,
            new: Target::Object(target),
        },
        name: "HEAD".try_into().expect("HEAD is a valid ref name"),
        deref: true,
    };
    repo.edit_reference(edit).map_err(|e| e.to_string())?;

    let opts = gix::worktree::state::checkout::Options {
        overwrite_existing: true,
        destination_is_initially_empty: true,
        ..Default::default()
    };
    let interrupt = std::sync::atomic::AtomicBool::new(false);
    gix::worktree::state::checkout(
        &mut index,
        workdir.to_path_buf(),
        repo.objects.clone(),
        &gix::progress::Discard,
        &gix::progress::Discard,
        &interrupt,
        opts,
    )
    .map_err(|e| e.to_string())?;

    index.write(Default::default()).map_err(|e| e.to_string())?;

    Ok(())
}

#[cfg(test)]
#[cfg(feature = "git")]
mod tests {
    use super::*;
    use serial_test::serial;

    #[test]
    #[serial]
    fn test_discovery_git_inherit_templates() {
        let mut discovery = Discovery::default();

        let registry = discover_git(
            "https://github.com/kstasik/schema-tools.git",
            GitCheckoutType::Rev("a279f3b54bc7b03af83162fbf027eb781db1e046".to_string()),
            false,
        )
        .unwrap();
        discovery.register("testing".to_string(), registry);

        let result = discovery
            .resolve(&[
                "testing::.".to_string(),
                "resources/test/discovery/test2/".to_string(),
            ])
            .unwrap();

        assert_eq!(result.files.len(), 1);
        assert!(result.files.contains_key("README.md"));

        let content = fs::read_to_string(result.files.get("README.md").unwrap()).unwrap();
        assert_eq!(content.replace('\r', ""), "# Schema Tools\n".to_string());

        let template = result.templates.get("test.j2").unwrap();
        assert_eq!(template, "# just test");
    }

    #[test]
    #[serial]
    fn test_discovery_git_inherit() {
        let mut discovery = Discovery::default();

        let registry = discover_git(
            "https://github.com/kstasik/schema-tools.git",
            GitCheckoutType::Rev("a279f3b54bc7b03af83162fbf027eb781db1e046".to_string()),
            false,
        )
        .unwrap();
        discovery.register("testing".to_string(), registry);

        let result = discovery
            .resolve(&[
                "testing::.".to_string(),
                "resources/test/discovery/test1/".to_string(),
            ])
            .unwrap();

        assert_eq!(result.files.len(), 1);
        assert!(result.files.contains_key("README.md"));

        let content = fs::read_to_string(result.files.get("README.md").unwrap()).unwrap();
        assert_eq!(content, "# just a test case".to_string());
    }

    #[test]
    #[serial]
    fn test_discovery_git() {
        let mut discovery = Discovery::default();

        let registry = discover_git(
            "https://github.com/kstasik/schema-tools.git",
            GitCheckoutType::Rev("a279f3b54bc7b03af83162fbf027eb781db1e046".to_string()),
            false,
        )
        .unwrap();
        discovery.register("testing".to_string(), registry);

        let result = discovery.resolve(&["testing::.".to_string()]).unwrap();

        assert_eq!(result.files.len(), 1);
        assert!(result.files.contains_key("README.md"));
    }

    #[test]
    fn test_discovery_file() {
        let discovery = Discovery::default();

        let result = discovery
            .resolve(&["./resources/test/".to_string()])
            .unwrap();

        let mut path = PathBuf::from("json-schemas");
        path.push("01-simple.json");

        let expected = path.to_str().unwrap();

        assert!(
            result.files.contains_key(expected),
            "cant find {} in {:?}",
            expected,
            result.files
        );
    }

    #[test]
    #[serial]
    fn test_discover_git_hash() {
        let registry = discover_git(
            "https://github.com/kstasik/schema-tools.git",
            GitCheckoutType::Rev("a279f3b54bc7b03af83162fbf027eb781db1e046".to_string()),
            false,
        )
        .unwrap();

        let data = registry.get_file("README.md").unwrap();
        let expected = "# Schema Tools\n";

        assert_eq!(data.replace('\r', ""), expected);
    }

    #[test]
    fn test_discover_git_branch() {
        let registry = discover_git(
            "https://github.com/kstasik/schema-tools.git",
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

        assert_eq!(data.replace('\r', ""), expected);
    }

    #[test]
    fn test_discover_git_tag() {
        let registry = discover_git(
            "https://github.com/kstasik/schema-tools.git",
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

        assert_eq!(data.replace('\r', ""), expected);
    }

    #[test]
    fn test_discover_git_tag_return_already_existing_registry() {
        testing_logger::setup();

        discover_git(
            "https://github.com/kstasik/schema-tools.git",
            GitCheckoutType::Tag("v0.0.2".to_string()),
            true,
        )
        .unwrap();
        discover_git(
            "https://github.com/kstasik/schema-tools.git",
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
            "https://github.com/kstasik/schema-tools.git",
            GitCheckoutType::Tag("v0.0.3".to_string()),
            true,
        )
        .unwrap();
        discover_git(
            "https://github.com/kstasik/schema-tools.git",
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
