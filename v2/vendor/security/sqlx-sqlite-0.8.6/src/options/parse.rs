use std::borrow::Cow;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::sync::atomic::{AtomicUsize, Ordering};

use percent_encoding::{percent_decode_str, percent_encode, AsciiSet};
use url::Url;

use crate::error::Error;
use crate::SqliteConnectOptions;

// https://www.sqlite.org/uri.html

static IN_MEMORY_DB_SEQ: AtomicUsize = AtomicUsize::new(0);

impl SqliteConnectOptions {
    pub(crate) fn from_db_and_params(database: &str, params: Option<&str>) -> Result<Self, Error> {
        let mut options = Self::default();

        if database == ":memory:" {
            options.in_memory = true;
            options.shared_cache = true;
            let seqno = IN_MEMORY_DB_SEQ.fetch_add(1, Ordering::Relaxed);
            options.filename = Cow::Owned(PathBuf::from(format!("file:sqlx-in-memory-{seqno}")));
        } else {
            // % decode to allow for `?` or `#` in the filename
            options.filename = Cow::Owned(
                Path::new(
                    &*percent_decode_str(database)
                        .decode_utf8()
                        .map_err(Error::config)?,
                )
                .to_path_buf(),
            );
        }

        if let Some(params) = params {
            for (key, value) in url::form_urlencoded::parse(params.as_bytes()) {
                match &*key {
                    // The mode query parameter determines if the new database is opened read-only,
                    // read-write, read-write and created if it does not exist, or that the
                    // database is a pure in-memory database that never interacts with disk,
                    // respectively.
                    "mode" => {
                        match &*value {
                            "ro" => {
                                options.read_only = true;
                            }

                            // default
                            "rw" => {}

                            "rwc" => {
                                options.create_if_missing = true;
                            }

                            "memory" => {
                                options.in_memory = true;
                                options.shared_cache = true;
                            }

                            _ => {
                                return Err(Error::Configuration(
                                    format!("unknown value {value:?} for `mode`").into(),
                                ));
                            }
                        }
                    }

                    // The cache query parameter specifies the cache behaviour across multiple
                    // connections to the same database within the process. A shared cache is
                    // essential for persisting data across connections to an in-memory database.
                    "cache" => match &*value {
                        "private" => {
                            options.shared_cache = false;
                        }

                        "shared" => {
                            options.shared_cache = true;
                        }

                        _ => {
                            return Err(Error::Configuration(
                                format!("unknown value {value:?} for `cache`").into(),
                            ));
                        }
                    },

                    "immutable" => match &*value {
                        "true" | "1" => {
                            options.immutable = true;
                        }
                        "false" | "0" => {
                            options.immutable = false;
                        }
                        _ => {
                            return Err(Error::Configuration(
                                format!("unknown value {value:?} for `immutable`").into(),
                            ));
                        }
                    },

                    "vfs" => options.vfs = Some(Cow::Owned(value.into_owned())),

                    _ => {
                        return Err(Error::Configuration(
                            format!("unknown query parameter `{key}` while parsing connection URL")
                                .into(),
                        ));
                    }
                }
            }
        }

        Ok(options)
    }

    pub(crate) fn build_url(&self) -> Url {
        // https://url.spec.whatwg.org/#path-percent-encode-set
        static PATH_ENCODE_SET: AsciiSet = percent_encoding::CONTROLS
            .add(b' ')
            .add(b'"')
            .add(b'#')
            .add(b'<')
            .add(b'>')
            .add(b'?')
            .add(b'`')
            .add(b'{')
            .add(b'}');

        let filename_encoded = percent_encode(
            self.filename.as_os_str().as_encoded_bytes(),
            &PATH_ENCODE_SET,
        );

        let mut url = Url::parse(&format!("sqlite://{filename_encoded}"))
            .expect("BUG: generated un-parseable URL");

        let mode = match (self.in_memory, self.create_if_missing, self.read_only) {
            (true, _, _) => "memory",
            (false, true, _) => "rwc",
            (false, false, true) => "ro",
            (false, false, false) => "rw",
        };
        url.query_pairs_mut().append_pair("mode", mode);

        let cache = match self.shared_cache {
            true => "shared",
            false => "private",
        };
        url.query_pairs_mut().append_pair("cache", cache);

        if self.immutable {
            url.query_pairs_mut().append_pair("immutable", "true");
        }

        if let Some(vfs) = &self.vfs {
            url.query_pairs_mut().append_pair("vfs", vfs);
        }

        url
    }
}

impl FromStr for SqliteConnectOptions {
    type Err = Error;

    fn from_str(mut url: &str) -> Result<Self, Self::Err> {
        // remove scheme from the URL
        url = url
            .trim_start_matches("sqlite://")
            .trim_start_matches("sqlite:");

        let mut database_and_params = url.splitn(2, '?');

        let database = database_and_params.next().unwrap_or_default();
        let params = database_and_params.next();

        Self::from_db_and_params(database, params)
    }
}

#[test]
fn test_parse_in_memory() -> Result<(), Error> {
    let options: SqliteConnectOptions = "sqlite::memory:".parse()?;
    assert!(options.in_memory);
    assert!(options.shared_cache);

    let options: SqliteConnectOptions = "sqlite://?mode=memory".parse()?;
    assert!(options.in_memory);
    assert!(options.shared_cache);

    let options: SqliteConnectOptions = "sqlite://:memory:".parse()?;
    assert!(options.in_memory);
    assert!(options.shared_cache);

    let options: SqliteConnectOptions = "sqlite://?mode=memory&cache=private".parse()?;
    assert!(options.in_memory);
    assert!(!options.shared_cache);

    Ok(())
}

#[test]
fn test_parse_read_only() -> Result<(), Error> {
    let options: SqliteConnectOptions = "sqlite://a.db?mode=ro".parse()?;
    assert!(options.read_only);
    assert_eq!(&*options.filename.to_string_lossy(), "a.db");

    Ok(())
}

#[test]
fn test_parse_shared_in_memory() -> Result<(), Error> {
    let options: SqliteConnectOptions = "sqlite://a.db?cache=shared".parse()?;
    assert!(options.shared_cache);
    assert_eq!(&*options.filename.to_string_lossy(), "a.db");

    Ok(())
}

#[test]
fn it_returns_the_parsed_url() -> Result<(), Error> {
    let url = "sqlite://test.db?mode=rw&cache=shared";
    let options: SqliteConnectOptions = url.parse()?;

    let expected_url = Url::parse(url).unwrap();
    assert_eq!(options.build_url(), expected_url);

    Ok(())
}
