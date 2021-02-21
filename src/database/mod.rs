use crate::package_index::{Dependency, PackageIndex, PackageVersion};
use rusqlite::{named_params, params, Connection};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Prepare the database schema.
pub fn init(conn: &Connection) -> crate::Result<()> {
    conn.execute_batch(
        r#"
        BEGIN;
        CREATE TABLE IF NOT EXISTS crates
        (
            id   INTEGER PRIMARY KEY AUTOINCREMENT,
            name TEXT NOT NULL
        );
        CREATE UNIQUE INDEX IF NOT EXISTS idx_crates_name
            ON crates (name);
        CREATE TABLE IF NOT EXISTS crate_versions
        (
            id            INTEGER PRIMARY KEY AUTOINCREMENT,
            crate_id      INTEGER NOT NULL,
            vers          TEXT    NOT NULL,
            description   TEXT,
            yanked        INTEGER NOT NULL,
            metadata      TEXT    NOT NULL,
            created       TEXT    NOT NULL,
            modified      TEXT,
            FOREIGN KEY (crate_id)
                REFERENCES crates (id)
                ON DELETE CASCADE
                ON UPDATE NO ACTION
        );
        CREATE UNIQUE INDEX IF NOT EXISTS idx_crate_versions_crate_vers
            ON crate_versions (crate_id, vers);
        COMMIT;
    "#,
    )?;
    Ok(())
}

/// Estuary v0.1.1 and earlier didn't have a database to track crates published
/// to the index.
/// For users who started using the project prior to the database support being
/// added, this can be used to back-fill records based on the files in the
/// git index.
pub fn backfill_db(conn: &mut Connection, index: &PackageIndex) -> crate::Result<()> {
    let crates = index.list_crates()?;
    for crate_name in crates {
        let pkg_versions = index.get_package_versions(&crate_name)?;
        for pkg_vers in pkg_versions {
            // Need to convert from a `PackageVersion` into a `NewCrate` then
            // partition by success/failure.
            let new_crate = pkg_vers.into();

            match publish(conn, &new_crate) {
                Ok(_) => log::info!("Added {} v{}", &new_crate.name, &new_crate.vers),
                Err(e) => log::warn!(
                    "Failed to add {} v{}. Reason: `{}`",
                    &new_crate.name,
                    &new_crate.vers,
                    e
                ),
            }
        }
    }
    Ok(())
}

pub fn publish(conn: &mut Connection, new_crate: &NewCrate) -> crate::Result<()> {
    let tx = conn.transaction()?;
    let crate_id = if let Ok(1) = tx.execute(
        "INSERT INTO crates (name) VALUES (?)",
        params![new_crate.name],
    ) {
        tx.last_insert_rowid()
    } else {
        tx.query_row(
            "SELECT id FROM crates WHERE name=?",
            params![new_crate.name],
            |row| row.get(0),
        )?
    };

    tx.execute_named(
        r#"
        INSERT INTO crate_versions
        (
            crate_id,
            vers,
            description,
            yanked,
            metadata,
            created
        )
        VALUES (
            :crate_id,
            :vers,
            :description,
            :yanked,
            :metadata,
            datetime('now')
        )
        "#,
        named_params! {
        ":crate_id": crate_id,
        ":vers": new_crate.vers,
        ":description": new_crate.description,
        ":yanked": false,
        ":metadata": serde_json::to_string(&new_crate).unwrap(),
        },
    )?;

    tx.commit()?;
    Ok(())
}

pub fn _set_yanked(
    conn: &mut Connection,
    crate_name: &str,
    version: &semver::Version,
    yanked: bool,
) -> crate::Result<()> {
    let tx = conn.transaction()?;
    let crate_id: i64 = tx.query_row(
        "SELECT id FROM crates WHERE name=?",
        params![crate_name],
        |row| row.get(0),
    )?;

    tx.execute_named(
        r#"
        UPDATE crate_versions
        SET
            yanked = :yanked,
            modified = datetime('now')
        WHERE
            crate_id = :crate_id,
            vers = :vers
        "#,
        named_params! {
        ":crate_id": crate_id,
        ":vers": format!("{}", version),
        ":yanked": yanked,
        },
    )?;
    tx.commit()?;
    Ok(())
}

/// The JSON payload sent to the registry during publish.
///
/// There are some slight differences in the field names here and what we're
/// expected to write to the actual index files.
///
/// This type is "borrowed" from `cargo`'s `crates-io` crate.
/// <https://github.com/rust-lang/cargo/blob/master/crates/crates-io/lib.rs>
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct NewCrate {
    pub name: String,
    pub vers: String,
    pub deps: Vec<NewCrateDependency>,
    pub features: BTreeMap<String, Vec<String>>,
    pub authors: Vec<String>,
    pub description: Option<String>,
    pub documentation: Option<String>,
    pub homepage: Option<String>,
    pub readme: Option<String>,
    pub readme_file: Option<String>,
    pub keywords: Vec<String>,
    pub categories: Vec<String>,
    pub license: Option<String>,
    pub license_file: Option<String>,
    pub repository: Option<String>,
    pub badges: BTreeMap<String, BTreeMap<String, String>>,
    pub links: Option<String>,
}

/// This type is "borrowed" from `cargo`'s `crates-io` crate.
/// <https://github.com/rust-lang/cargo/blob/master/crates/crates-io/lib.rs>
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct NewCrateDependency {
    pub optional: bool,
    pub default_features: bool,
    pub name: String,
    pub features: Vec<String>,
    pub version_req: String,
    pub target: Option<String>,
    pub kind: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub registry: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub explicit_name_in_toml: Option<String>,
}

impl From<Dependency> for NewCrateDependency {
    fn from(dep: Dependency) -> Self {
        NewCrateDependency {
            optional: dep.optional,
            default_features: dep.default_features,
            name: dep.name,
            features: dep.features,
            version_req: dep.req,
            target: dep.target,
            kind: dep.kind.to_string(),
            registry: dep.registry,
            explicit_name_in_toml: dep.package,
        }
    }
}

impl From<PackageVersion> for NewCrate {
    fn from(pkg_vers: PackageVersion) -> Self {
        NewCrate {
            name: pkg_vers.name,
            vers: pkg_vers.vers.to_string(),
            deps: pkg_vers.deps.into_iter().map(Into::into).collect(),
            features: pkg_vers.features,
            // None of the other fields are stored in the index. RIP.
            authors: vec![],
            description: None,
            documentation: None,
            homepage: None,
            readme: None,
            readme_file: None,
            keywords: vec![],
            categories: vec![],
            license: None,
            license_file: None,
            repository: None,
            badges: Default::default(),
            links: None,
        }
    }
}
