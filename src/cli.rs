//! Some of the fields on `Opt` require careful handling currently managed
//! through getters. In order to restrict direct access to those
//! getter-accessed fields, we tuck it away in this module.
use std::path::PathBuf;
use structopt::StructOpt;

/// Something about the macros used by `structopt` mean the return from
/// `from_args()` is <unknown> in code editors without a type ascription or some
/// other
/// hint. This function provides such a hint.
pub fn parse_args() -> Opt {
    Opt::from_args()
}

#[derive(StructOpt)]
pub struct Opt {
    #[structopt(
        long,
        env = "ESTUARY_BASE_URL",
        help = "The public url for the service."
    )]
    base_url: String,

    #[structopt(
        long,
        parse(from_os_str),
        env = "ESTUARY_INDEX_DIR",
        help = "A directory to store the package index git repo."
    )]
    pub index_dir: PathBuf,

    #[structopt(
        long,
        parse(from_os_str),
        env = "ESTUARY_CRATE_DIR",
        help = "A directory to store `.crate` files."
    )]
    pub crate_dir: PathBuf,

    #[structopt(
        long,
        env = "ESTUARY_DOWNLOAD_URL",
        help = "The url template cargo will use when downloading crates from the registry. \
        Defaults to `<base_url>/api/v1/crates/{crate}/{version}/download`."
    )]
    download_url: Option<String>,

    #[structopt(long, default_value = "0.0.0.0", env = "ESTUARY_HTTP_HOST")]
    pub http_host: String,

    #[structopt(long, default_value = "7878", env = "ESTUARY_HTTP_PORT")]
    pub http_port: u16,

    #[structopt(
        long,
        parse(from_os_str),
        env = "ESTUARY_GIT_BIN",
        default_value = "git",
        help = "Path to `git`."
    )]
    pub git_bin: PathBuf,

    #[structopt(long, env = "ESTUARY_PUBLISH_KEY")]
    pub publish_key: Option<String>
}

impl Opt {
    /// Public getter for the `base_url` field.
    ///
    /// Mainly this just ensures there are no trailing slashes in there.
    pub fn base_url(&self) -> &str {
        self.base_url.trim_end_matches('/')
    }

    /// Returns the value of the `download_url` field verbatim when set.
    ///
    /// When left `download_url` is left unset, the path to the download handler
    /// is built based on the value of the base url.
    pub fn download_url(&self) -> String {
        self.download_url.clone().unwrap_or_else(|| {
            format!(
                "{}/api/v1/crates/{{crate}}/{{version}}/download",
                self.base_url()
            )
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_base_url_trims_trailing_slashes() {
        let opt = Opt {
            // weird
            base_url: "http://example.com/////".to_string(),
            index_dir: Default::default(),
            crate_dir: Default::default(),
            download_url: None,
            http_host: "".to_string(),
            http_port: 0,
            git_bin: Default::default(),
            publish_key: Default::default()
        };

        assert_eq!("http://example.com", opt.base_url());
    }

    #[test]
    fn test_download_url_default() {
        let opt = Opt {
            base_url: "http://example.com".to_string(),
            index_dir: Default::default(),
            crate_dir: Default::default(),
            download_url: None,
            http_host: "".to_string(),
            http_port: 0,
            git_bin: Default::default(),
            publish_key: Default::default()
        };

        assert_eq!(
            "http://example.com/api/v1/crates/{crate}/{version}/download",
            opt.download_url()
        );
    }
}
