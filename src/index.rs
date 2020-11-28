//! Index management.
//!
//! The registry's index is comprised of a config file and one file per crate.
//! Each crate file contains a json object per line per published version.
//!
//! This "registry index" should not be confused with the "git index" which is
//! also a thing in here since all changes to the registry index must be
//! committed a the git repo which is also managed by this module..

use anyhow::{anyhow, Result};
use git2::{Oid, Repository, Signature};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::OpenOptions;
use std::io::{BufReader, Read, Write};
use std::path::{Path, PathBuf};

/// The config data for the registry.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct Config {
    dl: String,
    api: String,
}

/// These records appear, one per line per version, in each crate file.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct PackageVersion {
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
pub struct Dependency {
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
pub enum DependencyKind {
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

pub struct PackageIndex {
    repo: Repository,
}

impl PackageIndex {
    /// Initialize a fresh (registry) index.
    ///
    /// Given an empty directory, this will create a new git repo containing a
    /// `config.json`.
    ///
    /// If the directory is non-empty *and has a git repo in it*, the assumption
    /// is there's already a valid index at that path.
    /// An attempt to update the config (if necessary) using the supplied values
    /// will be made.
    pub fn init<P>(path: P, config: &Config) -> Result<Self>
    where
        P: AsRef<Path>,
    {
        let path = path.as_ref();
        let pkg_index = Self {
            repo: get_or_create_repo(path)?,
        };
        let current_config: Option<Config> = pkg_index.read_config().ok();

        if Some(config) != current_config.as_ref() {
            // XXX: might need to think about reverting if something fails part way
            // through the operation.
            pkg_index.write_config(config)?;
            pkg_index.add_and_commit_file("config.json", "update registry config")?;
        }
        Ok(pkg_index)
    }

    /// Add a file, then commit it to the git repo.
    ///
    /// Roughly equivalent to:
    ///
    /// ```text
    /// git add <path> && git commit -m <msg>
    /// ```
    fn add_and_commit_file<P>(&self, path: P, msg: &str) -> Result<()>
    where
        P: AsRef<Path>,
    {
        let head = self.repo.head()?;
        let parent = head.peel_to_commit()?;
        let mut index = self.repo.index()?;
        index.add_path(path.as_ref())?;
        index.write()?;
        let tree_id = index.write_tree()?;
        let tree = self.repo.find_tree(tree_id)?;
        let sig = get_sig()?;
        self.repo
            .commit(Some("HEAD"), &sig, &sig, msg, &tree, &[&parent])?;
        Ok(())
    }

    /// Read and parse the config file from the registry root directory.
    fn read_config(&self) -> Result<Config> {
        let fh = std::fs::File::open(self.repo.workdir().unwrap().join("config.json"))?;
        Ok(serde_json::from_reader(fh)?)
    }

    /// Write the config to the registry root directory.
    fn write_config(&self, config: &Config) -> Result<()> {
        log::debug!("Writing registry config file.");
        let mut fh = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(self.repo.workdir().unwrap().join("config.json"))?;
        fh.write_all(&serde_json::to_vec(config)?)?;
        fh.flush()?;
        fh.sync_all()?;
        Ok(())
    }

    /// Update (or create) a package file in the index.
    ///
    /// When publishing a new package, a package file is created in the index.
    ///
    /// With the package file created, versions are added as json objects, one per
    /// line (per version).
    ///
    /// If the version already exists in the package file, this function will
    /// return an `Err`.
    pub fn publish(&self, pkg: &PackageVersion) -> Result<()> {
        let root = self.repo.workdir().unwrap();
        let dir = get_package_file_dir(&pkg.name)?;
        std::fs::create_dir_all(root.join(&dir))?;
        let pkg_file = dir.join(&pkg.name);

        // "touch" the file to make sure it's available for reading.
        OpenOptions::new()
            .create(true)
            .write(true)
            .open(root.join(&pkg_file))?;

        // Read the file to see if the version we're publishing is already present.
        // Bail if it is.
        {
            let contents = self.read_package_file(&pkg.name)?;
            for line in contents.lines() {
                let PackageVersion { vers, .. } = serde_json::from_str(line)?;
                if vers == pkg.vers {
                    return Err(anyhow!(
                        "Failed to publish `{} v{}`. Already exists in index.",
                        pkg.name,
                        pkg.vers
                    ));
                }
            }
        }

        // Write the version to the file.
        {
            let mut fh = OpenOptions::new()
                .create(false)
                .append(true)
                .open(root.join(&pkg_file))?;
            writeln!(fh, "{}", serde_json::to_string(pkg)?)?;
            fh.flush()?;
            fh.sync_all()?
        }

        self.add_and_commit_file(
            pkg_file,
            &format!("publish crate: `{} v{}`", pkg.name, pkg.vers),
        )?;
        Ok(())
    }

    /// Get the contents of a package file.
    fn read_package_file(&self, name: &str) -> Result<String> {
        let root = self.repo.workdir().unwrap();
        let dir = get_package_file_dir(name)?;
        std::fs::create_dir_all(root.join(&dir))?;
        let pkg_file = dir.join(name);
        let mut fh = BufReader::new(
            OpenOptions::new()
                .create(false)
                .read(true)
                .open(root.join(&pkg_file))?,
        );

        let mut buf = String::new();
        fh.read_to_string(&mut buf)?;
        Ok(buf)
    }

    /// Truncate and rewrite a package file.
    fn rewrite_package_file(&self, name: &str, pkg_versions: &[PackageVersion]) -> Result<()> {
        let root = self.repo.workdir().unwrap();
        let dir = get_package_file_dir(name)?;
        std::fs::create_dir_all(root.join(&dir))?;
        let pkg_file = dir.join(name);

        let mut fh = OpenOptions::new()
            .create(false)
            .write(true)
            .truncate(true)
            .open(root.join(&pkg_file))?;

        for x in pkg_versions {
            writeln!(fh, "{}", serde_json::to_string(x)?)?;
        }

        fh.flush()?;
        fh.sync_all()?;
        Ok(())
    }

    /// Updates the `yanked` field of a given package version.
    pub fn set_yanked(&self, name: &str, version: &str, yanked: bool) -> Result<()> {
        // This is the most naive impl I can think of for this, but it should get
        // things rolling.
        // Read the whole package file, json parse all lines, modify the struct that
        // matches our target, then write all the lines back to the file (truncating
        // the file).
        // A better version of this would modify the specific line in the file, I
        // guess.

        let mut pkg_versions = self
            .read_package_file(name)?
            .lines()
            .map(serde_json::from_str)
            .map(|r| r.map_err(Into::into))
            .collect::<Result<Vec<PackageVersion>>>()?;

        for pkg in &mut pkg_versions {
            if pkg.vers == version {
                if pkg.yanked == yanked {
                    // Nothing to do if the values are the same.
                    return Ok(());
                }
                pkg.yanked = yanked;
                break;
            }
        }

        self.rewrite_package_file(name, &pkg_versions)?;

        let dir = get_package_file_dir(name)?;

        let verb = if yanked { "yank" } else { "unyank" };

        self.add_and_commit_file(
            dir.join(name),
            &format!("{} crate: `{} v{}`", verb, name, version),
        )?;

        Ok(())
    }

    fn get_repo_log(&self) -> Result<Vec<(Oid, Option<String>)>> {
        Ok(self
            .repo
            .reflog("HEAD")?
            .iter()
            .map(|entry| (entry.id_new(), entry.message().map(String::from)))
            .collect())
    }
}

/// Generate the directory name for a package file in the index.
///
/// The index repository contains one file for each package, where the filename
/// is the name of the package in lowercase. Each version of the package has a
/// separate line in the file. The files are organized in a tier of directories:
///
/// - Packages with 1 character names are placed in a directory named `1`.
/// - Packages with 2 character names are placed in a directory named `2`.
/// - Packages with 3 character names are placed in the directory
///   `3/{first-character}` where `{first-character}` is the first character of
///   the package name.
/// - All other packages are stored in directories named
///   `{first-two}/{second-two}` where the top directory is the first two
///   characters of the package name, and the next subdirectory is the third and
///   fourth characters of the package name. For example, `cargo` would be
///   stored in a file named `ca/rg/cargo`.
fn get_package_file_dir(name: &str) -> Result<PathBuf> {
    let name = name.trim().to_lowercase();
    match name.len() {
        0 => Err(anyhow!("Empty string is not a valid package name.")),
        1 => Ok(PathBuf::from("1/")),
        2 => Ok(PathBuf::from("2/")),
        3 => {
            let mut pb = PathBuf::from("3/");
            pb.push(name.chars().next().unwrap().to_string());
            Ok(pb)
        }
        _ => {
            let mut pb = PathBuf::new();
            let chars = name.chars().take(4).map(String::from).collect::<Vec<_>>();
            pb.push(chars[0..2].join(""));
            pb.push(chars[2..4].join(""));
            Ok(pb)
        }
    }
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

        let idx = PackageIndex::init(
            &root,
            &Config {
                dl: String::from("http://localhost/dl"),
                api: String::from("http://localhost/api"),
            },
        )
        .unwrap();
        let entries = idx.get_repo_log().unwrap();

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

        let _idx = PackageIndex::init(
            &root,
            &Config {
                dl: String::from("http://localhost/dl"),
                api: String::from("http://localhost/api"),
            },
        )
        .unwrap();

        let idx2 = PackageIndex::init(
            &root,
            &Config {
                dl: String::from("http://example.com/dl"),
                api: String::from("http://example.com/api"),
            },
        )
        .unwrap();

        let entries = idx2.get_repo_log().unwrap();

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

        let _idx = PackageIndex::init(&root, &config).unwrap();
        let idx2 = PackageIndex::init(&root, &config).unwrap();
        let entries = idx2.get_repo_log().unwrap();

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

    #[test]
    fn test_get_empty_package_dir_is_err() {
        assert!(get_package_file_dir("").is_err());
        assert!(get_package_file_dir("  ").is_err());
        assert!(get_package_file_dir("    ").is_err());
    }

    #[test]
    fn test_get_1_char_package_dir() {
        assert_eq!(PathBuf::from("1/"), get_package_file_dir("a").unwrap());
        assert_eq!(PathBuf::from("1/"), get_package_file_dir("b").unwrap());
        assert_eq!(PathBuf::from("1/"), get_package_file_dir("c").unwrap());
    }

    #[test]
    fn test_get_2_char_package_dir() {
        assert_eq!(PathBuf::from("2/"), get_package_file_dir("aa").unwrap());
        assert_eq!(PathBuf::from("2/"), get_package_file_dir("ba").unwrap());
        assert_eq!(PathBuf::from("2/"), get_package_file_dir("ca").unwrap());
    }

    #[test]
    fn test_get_3_char_package_dir() {
        assert_eq!(PathBuf::from("3/a"), get_package_file_dir("aaa").unwrap());
        assert_eq!(PathBuf::from("3/b"), get_package_file_dir("baa").unwrap());
        assert_eq!(PathBuf::from("3/c"), get_package_file_dir("caa").unwrap());
    }

    #[test]
    fn test_get_4_or_more_char_package_dir() {
        assert_eq!(
            PathBuf::from("aa/aa"),
            get_package_file_dir("aaaa").unwrap()
        );
        assert_eq!(
            PathBuf::from("ba/ab"),
            get_package_file_dir("baab").unwrap()
        );
        assert_eq!(
            PathBuf::from("ab/cd"),
            get_package_file_dir("abcd").unwrap()
        );
        assert_eq!(
            PathBuf::from("de/ad"),
            get_package_file_dir("deadbeef").unwrap()
        );
    }

    #[test]
    fn test_get_upper_cased_package_dir() {
        assert_eq!(
            PathBuf::from("aa/aa"),
            get_package_file_dir("aAAa").unwrap()
        );
    }

    #[test]
    fn test_publish_create_happy() {
        let pkg = PackageVersion {
            name: "foo".to_string(),
            vers: "0.1.0".to_string(),
            deps: vec![],
            cksum: "".to_string(),
            features: Default::default(),
            yanked: false,
            links: None,
        };

        let root = TempDir::new("test_publish_create_happy").unwrap();

        let config = Config {
            dl: String::from("http://localhost/dl"),
            api: String::from("http://localhost/api"),
        };

        let idx = PackageIndex::init(&root, &config).unwrap();

        idx.publish(&pkg).unwrap();

        let entries = idx.get_repo_log().unwrap();

        // There should be one commit for the empty repo, and one for the config
        // updates, and one for the publish.
        assert_eq!(entries.len(), 3);
        assert_eq!(
            entries
                .into_iter()
                .filter_map(|(_, msg)| msg)
                .filter(|msg| msg.contains("publish crate: `foo v0.1.0`"))
                .count(),
            1
        );
    }

    #[test]
    fn test_publish_same_vers_twice_is_err() {
        let pkg = PackageVersion {
            name: "foo".to_string(),
            vers: "0.1.0".to_string(),
            deps: vec![],
            cksum: "".to_string(),
            features: Default::default(),
            yanked: false,
            links: None,
        };

        let root = TempDir::new("test_publish_same_vers_twice_is_err").unwrap();

        let config = Config {
            dl: String::from("http://localhost/dl"),
            api: String::from("http://localhost/api"),
        };

        let idx = PackageIndex::init(&root, &config).unwrap();

        idx.publish(&pkg).unwrap();
        assert!(idx.publish(&pkg).is_err());

        let entries = idx.get_repo_log().unwrap();

        // There should be one commit for the empty repo, and one for the config
        // updates, and one for the 1st publish, but nothing for the 2nd.
        assert_eq!(entries.len(), 3);
        assert_eq!(
            entries
                .into_iter()
                .filter_map(|(_, msg)| msg)
                .filter(|msg| msg.contains("publish crate: `foo v0.1.0`"))
                .count(),
            1
        );
    }
    #[test]
    fn test_yank() {
        let pkg = PackageVersion {
            name: "foo".to_string(),
            vers: "0.1.0".to_string(),
            deps: vec![],
            cksum: "".to_string(),
            features: Default::default(),
            yanked: false,
            links: None,
        };

        let root = TempDir::new("test_yank").unwrap();

        let config = Config {
            dl: String::from("http://localhost/dl"),
            api: String::from("http://localhost/api"),
        };

        let idx = PackageIndex::init(&root, &config).unwrap();

        idx.publish(&pkg).unwrap();
        idx.set_yanked(&pkg.name, &pkg.vers, true).unwrap();

        let entries = idx.get_repo_log().unwrap();

        // There should be one commit for the empty repo, and one for the config
        // updates, and one for the publish, and one for the yank.
        assert_eq!(entries.len(), 4);
        assert_eq!(
            entries
                .into_iter()
                .filter_map(|(_, msg)| msg)
                .filter(|msg| msg.contains("commit: yank crate: `foo v0.1.0`"))
                .count(),
            1
        );
    }

    #[test]
    fn test_unyank() {
        let pkg = PackageVersion {
            name: "foo".to_string(),
            vers: "0.1.0".to_string(),
            deps: vec![],
            cksum: "".to_string(),
            features: Default::default(),
            yanked: false,
            links: None,
        };

        let root = TempDir::new("test_unyank").unwrap();

        let config = Config {
            dl: String::from("http://localhost/dl"),
            api: String::from("http://localhost/api"),
        };

        let idx = PackageIndex::init(&root, &config).unwrap();

        idx.publish(&pkg).unwrap();

        idx.set_yanked(&pkg.name, &pkg.vers, true).unwrap();
        idx.set_yanked(&pkg.name, &pkg.vers, false).unwrap();

        let entries = idx.get_repo_log().unwrap();

        // There should be one commit for the empty repo, and one for the config
        // updates, and one for the publish, one for the yank, and finally one
        // for the unyank.
        assert_eq!(entries.len(), 5);
        assert_eq!(
            entries
                .iter()
                .filter_map(|(_, msg)| msg.as_ref())
                .filter(|msg| msg.contains("commit: yank crate: `foo v0.1.0`"))
                .count(),
            1
        );
        assert_eq!(
            entries
                .iter()
                .filter_map(|(_, msg)| msg.as_ref())
                .filter(|msg| msg.contains("commit: unyank crate: `foo v0.1.0`"))
                .count(),
            1
        );
    }

    #[test]
    fn test_double_yank() {
        let pkg = PackageVersion {
            name: "foo".to_string(),
            vers: "0.1.0".to_string(),
            deps: vec![],
            cksum: "".to_string(),
            features: Default::default(),
            yanked: false,
            links: None,
        };

        let root = TempDir::new("test_double_yank").unwrap();

        let config = Config {
            dl: String::from("http://localhost/dl"),
            api: String::from("http://localhost/api"),
        };

        let idx = PackageIndex::init(&root, &config).unwrap();

        idx.publish(&pkg).unwrap();

        idx.set_yanked(&pkg.name, &pkg.vers, true).unwrap();
        idx.set_yanked(&pkg.name, &pkg.vers, true).unwrap();

        let entries = idx.get_repo_log().unwrap();
        assert_eq!(entries.len(), 4);
        assert_eq!(
            entries
                .iter()
                .filter_map(|(_, msg)| msg.as_ref())
                .filter(|msg| msg.contains("commit: yank crate: `foo v0.1.0`"))
                .count(),
            1
        );
    }

    #[test]
    fn test_double_unyank() {
        let pkg = PackageVersion {
            name: "foo".to_string(),
            vers: "0.1.0".to_string(),
            deps: vec![],
            cksum: "".to_string(),
            features: Default::default(),
            yanked: false,
            links: None,
        };

        let root = TempDir::new("test_double_yank").unwrap();

        let config = Config {
            dl: String::from("http://localhost/dl"),
            api: String::from("http://localhost/api"),
        };

        let idx = PackageIndex::init(&root, &config).unwrap();

        idx.publish(&pkg).unwrap();

        idx.set_yanked(&pkg.name, &pkg.vers, false).unwrap();
        idx.set_yanked(&pkg.name, &pkg.vers, false).unwrap();

        let entries = idx.get_repo_log().unwrap();

        assert_eq!(entries.len(), 3);
        assert_eq!(
            entries
                .iter()
                .filter_map(|(_, msg)| msg.as_ref())
                .filter(|msg| msg.contains("commit: unyank crate: `foo v0.1.0`"))
                .count(),
            // Since packages start as unyanked, two more unyanks shouldn't do anything.
            0
        );
    }
}
