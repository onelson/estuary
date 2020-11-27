//! Index management.
//!
//! The registry's index is comprised of a config file and one file per crate.
//! Each crate file contains a json object per line per published version.
//!
//! This "registry index" should not be confused with the "git index" which is
//! also a thing in here since all changes to the registry index must be
//! committed a the git repo which is also managed by this module..

use anyhow::Result;
use git2::{Oid, Repository, Signature};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::Write;
use std::path::Path;

/// The config.json for the registry.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
struct Config {
    dl: String,
    api: String,
}

/// These records appear, one per line per version, in each crate file.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
struct PackageVersion {
    /// The name of the package.
    ///
    /// This must only contain alphanumeric, `-`, or `_` characters.
    name: String,
    /// The version of the package this row is describing.
    ///
    /// This must be a valid version number according to the Semantic
    /// Versioning 2.0.0 spec at https://semver.org/.
    vers: String,
    /// Array of direct dependencies of the package.
    deps: Vec<Dependency>,
    /// A SHA256 checksum of the `.crate` file.
    cksum: String,
    /// Set of features defined for the package.
    ///
    /// Each feature maps to an array of features or dependencies it enables.
    features: HashMap<String, Vec<String>>,
    /// Boolean of whether or not this version has been yanked.
    yanked: bool,
    /// The `links` string value from the package's manifest, or null if not
    /// specified. This field is optional and defaults to null.
    links: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
struct Dependency {
    /// Name of the dependency.
    ///
    /// If the dependency is renamed from the original package name,
    /// this is the new name. The original package name is stored in
    /// the `package` field.
    name: String,
    /// The semver requirement for this dependency.
    ///
    /// This must be a valid version requirement defined at
    /// https://github.com/steveklabnik/semver#requirements.
    req: String,
    /// Array of features (as strings) enabled for this dependency.
    features: Vec<String>,
    /// Boolean of whether or not this is an optional dependency.
    optional: bool,
    /// Boolean of whether or not default features are enabled.
    default_features: bool,
    /// The target platform for the dependency.
    /// null if not a target dependency.
    /// Otherwise, a string such as "cfg(windows)".
    target: Option<String>,
    /// The dependency kind.
    ///
    /// "dev", "build", or "normal".
    ///
    /// Note: this is a required field, but a small number of entries
    /// exist in the crates.io index with either a missing or null
    /// `kind` field due to implementation bugs.
    // FIXME: think about providing a 2nd `PartialDependency` struct that
    //  omits this field instead of weakening the requirement here.
    //  Mainly I want to ensure *new crates* published here are valid per the
    //  *current requirements*.
    kind: DependencyKind,
    /// The URL of the index of the registry where this dependency is
    /// from as a string. If not specified or null, it is assumed the
    /// dependency is in the current registry.
    registry: Option<String>,
    /// If the dependency is renamed, this is a string of the actual
    /// package name. If not specified or null, this dependency is not
    /// renamed.
    package: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "lowercase")]
enum DependencyKind {
    Build,
    Dev,
    Normal,
}

/// The cargo docs recommend restrictions to apply to package names on ingest:
///
/// - Only allows ASCII characters.
/// - Only alphanumeric, -, and _ characters.
/// - First character must be alphabetic.
/// - Case-insensitive collision detection.
/// - Prevent differences of - vs _.
/// - Under a specific length (max 64).
/// - Rejects reserved names, such as Windows special filenames like "nul".
fn validate_package_name() -> Result<()> {
    todo!()
}

/// Get a git signature for "the system".
fn get_sig() -> Result<Signature<'static>> {
    Ok(Signature::now("estuary", "admin@localhost")?)
}

fn get_or_create_repo<P>(root: P) -> Result<Repository>
where
    P: AsRef<Path>,
{
    let sig = get_sig()?;
    let root = root.as_ref();
    let is_empty = std::fs::read_dir(root)?.next().is_none();
    if is_empty {
        log::debug!("Creating a fresh index.");
        let repo = Repository::init(root)?;
        {
            let tree_id = {
                let mut index = repo.index()?;
                index.write_tree()?
            };
            let tree = repo.find_tree(tree_id)?;
            repo.commit(Some("HEAD"), &sig, &sig, "init empty repo", &tree, &[])?;
        }
        Ok(repo)
    } else {
        log::debug!("Using preexisting index.");
        Ok(Repository::open(root)?)
    }
}

/// Initialize a fresh (registry) index.
///
/// Given an empty directory, this will create a new git repo containing a
/// `config.json`.
///
/// If the directory is non-empty *and has a git repo in it*, the assumption is
/// there's already a valid index at that path.
/// An attempt to update the config (if necessary) using the supplied values
/// will be made.
fn init<P>(root: P, config: &Config) -> Result<()>
where
    P: AsRef<Path>,
{
    let root = root.as_ref();
    let repo = get_or_create_repo(root)?;
    let current_config: Option<Config> = read_config_from_disk(root).ok();

    if Some(config) != current_config.as_ref() {
        write_config_to_disk(root, config)?;

        let head = repo.head()?;
        let parent = head.peel_to_commit()?;

        let mut index = repo.index()?;

        index.add_path(Path::new("config.json"))?;
        index.write()?;

        let tree_id = index.write_tree()?;
        let tree = repo.find_tree(tree_id)?;
        let sig = get_sig()?;
        repo.commit(
            Some("HEAD"),
            &sig,
            &sig,
            "update registry config",
            &tree,
            &[&parent],
        )?;
    }

    Ok(())
}

/// Read and parse the config file from the registry root directory.
fn read_config_from_disk<P>(root: P) -> Result<Config>
where
    P: AsRef<Path>,
{
    let fh = std::fs::File::open(root.as_ref().join("config.json"))?;
    Ok(serde_json::from_reader(fh)?)
}

/// Write the config to the registry root directory.
fn write_config_to_disk<P>(root: P, config: &Config) -> Result<()>
where
    P: AsRef<Path>,
{
    log::debug!("Writing registry config file.");
    let mut fh = std::fs::OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .append(false)
        .open(root.as_ref().join("config.json"))?;
    fh.write_all(&serde_json::to_vec(config)?)?;
    fh.flush()?;
    fh.sync_all()?;
    Ok(())
}

fn get_repo_log<P>(root: P) -> Result<Vec<(Oid, Option<String>)>>
where
    P: AsRef<Path>,
{
    let repo = get_or_create_repo(root.as_ref())?;
    Ok(repo
        .reflog("HEAD")?
        .iter()
        .map(|entry| (entry.id_new(), entry.message().map(String::from)))
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use tempdir::TempDir;

    /// Parse the sample object from the cargo docs to verify our structs
    /// capture all the keys they're supposed to.
    #[test]
    fn test_pkg_codec_happy() {
        let sample = json!({
            "name": "foo",
            "vers": "0.1.0",
            "deps": [
                {
                    "name": "rand",
                    "req": "^0.6",
                    "features": ["i128_support"],
                    "optional": false,
                    "default_features": true,
                    "target": null,
                    "kind": "normal",
                    "registry": null,
                    "package": null,
                }
            ],
            "cksum": "d867001db0e2b6e0496f9fac96930e2d42233ecd3ca0413e0753d4c7695d289c",
            "features": {
                "extras": ["rand/simd_support"]
            },
            "yanked": false,
            "links": null
        });

        let pkg: PackageVersion = serde_json::from_value(sample).unwrap();

        assert_eq!("foo", pkg.name);
        assert_eq!("0.1.0", pkg.vers);
        assert_eq!("rand", pkg.deps[0].name);
        assert_eq!("^0.6", pkg.deps[0].req);
        assert_eq!(vec!["i128_support"], pkg.deps[0].features);
        assert_eq!(false, pkg.deps[0].optional);
        assert_eq!(true, pkg.deps[0].default_features);
        assert_eq!(None, pkg.deps[0].target);
        assert_eq!(DependencyKind::Normal, pkg.deps[0].kind);
        assert_eq!(None, pkg.deps[0].registry);
        assert_eq!(None, pkg.deps[0].package);
        assert_eq!(
            "d867001db0e2b6e0496f9fac96930e2d42233ecd3ca0413e0753d4c7695d289c",
            pkg.cksum
        );
        assert_eq!(
            &vec!["rand/simd_support"],
            pkg.features.get("extras").unwrap()
        );
        assert_eq!(false, pkg.yanked);
        assert_eq!(None, pkg.links);
    }

    #[test]
    fn test_init_empty_dir() {
        let root = TempDir::new("test_empty").unwrap();

        init(
            &root,
            &Config {
                dl: String::from("http://localhost/dl"),
                api: String::from("http://localhost/api"),
            },
        )
        .unwrap();
        let entries = get_repo_log(&root).unwrap();

        // There should be one commit for the empty repo, and one for the config
        // update.
        assert_eq!(entries.len(), 2);

        assert_eq!(
            entries
                .into_iter()
                .filter_map(|(_, msg)| msg)
                .filter(|msg| msg.contains("update registry config"))
                .count(),
            1
        );
    }

    #[test]
    fn test_config_change_updates_repo() {
        let root = TempDir::new("test_config_change_updates").unwrap();

        init(
            &root,
            &Config {
                dl: String::from("http://localhost/dl"),
                api: String::from("http://localhost/api"),
            },
        )
        .unwrap();

        init(
            &root,
            &Config {
                dl: String::from("http://example.com/dl"),
                api: String::from("http://example.com/api"),
            },
        )
        .unwrap();

        let entries = get_repo_log(&root).unwrap();

        // There should be one commit for the empty repo, and two for the config
        // updates.
        assert_eq!(entries.len(), 3);

        assert_eq!(
            entries
                .into_iter()
                .filter_map(|(_, msg)| msg)
                .filter(|msg| msg.contains("update registry config"))
                .count(),
            2
        );
    }

    #[test]
    fn test_unchanged_config_does_not_update_repo() {
        let root = TempDir::new("test_unchanged_config").unwrap();

        let config = Config {
            dl: String::from("http://localhost/dl"),
            api: String::from("http://localhost/api"),
        };

        init(&root, &config).unwrap();
        init(&root, &config).unwrap();
        let entries = get_repo_log(&root).unwrap();

        // There should be one commit for the empty repo, and one (only one) for
        // the config update.
        assert_eq!(entries.len(), 2);

        assert_eq!(
            entries
                .into_iter()
                .filter_map(|(_, msg)| msg)
                .filter(|msg| msg.contains("update registry config"))
                .count(),
            1
        );
    }
}
