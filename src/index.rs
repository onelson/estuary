//! Index management.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

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
#[allow(dead_code)]
fn validate_package_name() -> Result<(), ()> {
    todo!()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

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
}
