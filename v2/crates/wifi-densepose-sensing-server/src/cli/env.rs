//! Environment variable parsing helpers for CLI flags.

/// Value parser for boolean flags that can also be set from the environment.
///
/// clap's default `bool` parser accepts only `true`/`false`, so
/// `RUVIEW_UDP_INSECURE_LAN=1` used to exit 2 with "invalid value '1'"
/// (#2091). This accepts the usual spellings, case-insensitive:
/// `1/true/yes/on` and `0/false/no/off`. Anything else is still an error, so
/// a typo cannot silently flip a security flag. The bare `--flag` form is
/// unchanged: clap's `SetTrue` action feeds this parser `"true"`.
pub fn parse_env_bool(value: &str) -> Result<bool, String> {
    match value.trim().to_ascii_lowercase().as_str() {
        "1" | "true" | "yes" | "on" => Ok(true),
        "0" | "false" | "no" | "off" => Ok(false),
        _ => Err(format!(
            "expected one of 1/0, true/false, yes/no, on/off (got '{value}')"
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    #[test]
    fn parse_env_bool_accepts_common_spellings() {
        for v in ["1", "true", "TRUE", "True", "yes", "YES", "on", "On", " 1 "] {
            assert_eq!(parse_env_bool(v), Ok(true), "{v:?}");
        }
        for v in ["0", "false", "FALSE", "no", "No", "off", "OFF"] {
            assert_eq!(parse_env_bool(v), Ok(false), "{v:?}");
        }
    }

    #[test]
    fn parse_env_bool_rejects_other_values() {
        for v in ["", "2", "y", "n", "enable", "truee", "-1"] {
            assert!(parse_env_bool(v).is_err(), "{v:?} must be rejected");
        }
    }

    #[test]
    fn env_bool_flag_reads_env_spellings() {
        #[derive(Parser)]
        struct W {
            #[arg(long, env = "RUVIEW_TEST_ENV_BOOL_A", value_parser = parse_env_bool)]
            a: bool,
            #[arg(long, env = "RUVIEW_TEST_ENV_BOOL_B", value_parser = parse_env_bool)]
            b: bool,
            #[arg(long, env = "RUVIEW_TEST_ENV_BOOL_C", value_parser = parse_env_bool)]
            c: bool,
            #[arg(long, env = "RUVIEW_TEST_ENV_BOOL_D", value_parser = parse_env_bool)]
            d: bool,
        }
        std::env::set_var("RUVIEW_TEST_ENV_BOOL_A", "1");
        std::env::set_var("RUVIEW_TEST_ENV_BOOL_B", "yes");
        std::env::set_var("RUVIEW_TEST_ENV_BOOL_C", "0");
        std::env::remove_var("RUVIEW_TEST_ENV_BOOL_D");
        let w = W::parse_from(["t"]);
        assert!(w.a && w.b && !w.c && !w.d);

        // The command-line flag wins over an env value of `0`.
        let w = W::parse_from(["t", "--c"]);
        assert!(w.c);

        std::env::set_var("RUVIEW_TEST_ENV_BOOL_D", "maybe");
        assert!(W::try_parse_from(["t"]).is_err());

        for k in ["A", "B", "C", "D"] {
            std::env::remove_var(format!("RUVIEW_TEST_ENV_BOOL_{k}"));
        }
    }
}

