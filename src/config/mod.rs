use thiserror::Error;

/// Config holds application configuration.
#[derive(Debug, Clone)]
pub struct Config {
    pub aws_bin: String,
    pub profile: Option<String>,
    pub region: String,
}

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("aws CLI binary {bin:?} not found in PATH")]
    InvalidBin { bin: String },
    #[error("{0}")]
    Other(String),
}

/// Determines the AWS CLI binary and resolves profile/region.
pub fn resolve(profile: Option<&str>, region: Option<&str>) -> Result<Config, ConfigError> {
    let aws_bin = std::env::var("AWS_CLI_BIN").unwrap_or_else(|_| "aws".to_string());

    let main_bin = aws_bin.split_whitespace().next().unwrap_or(&aws_bin);

    if which::which(main_bin).is_err() {
        return Err(ConfigError::InvalidBin { bin: aws_bin });
    }

    let profile = profile
        .map(|s| s.to_string())
        .or_else(|| std::env::var("AWS_PROFILE").ok());

    let region = region
        .map(|s| s.to_string())
        .or_else(|| std::env::var("AWS_REGION").ok())
        .or_else(|| std::env::var("AWS_DEFAULT_REGION").ok())
        .unwrap_or_else(|| "us-east-1".to_string());

    Ok(Config {
        aws_bin,
        profile,
        region,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;

    #[test]
    #[serial]
    fn resolve_no_profile() {
        std::env::remove_var("AWS_PROFILE");
        std::env::remove_var("AWS_REGION");
        let cfg = resolve(None, None).unwrap();
        assert_eq!(cfg.profile, None);
        assert_eq!(cfg.region, "us-east-1");
    }

    #[test]
    #[serial]
    fn resolve_explicit_profile() {
        let cfg = resolve(Some("staging"), Some("eu-west-1")).unwrap();
        assert_eq!(cfg.profile, Some("staging".to_string()));
        assert_eq!(cfg.region, "eu-west-1");
    }

    #[test]
    #[serial]
    fn resolve_env_profile() {
        std::env::set_var("AWS_PROFILE", "from-env");
        std::env::set_var("AWS_REGION", "ap-southeast-1");
        let cfg = resolve(None, None).unwrap();
        std::env::remove_var("AWS_PROFILE");
        std::env::remove_var("AWS_REGION");
        assert_eq!(cfg.profile, Some("from-env".to_string()));
        assert_eq!(cfg.region, "ap-southeast-1");
    }

    #[test]
    #[serial]
    fn resolve_explicit_overrides_env() {
        std::env::set_var("AWS_PROFILE", "from-env");
        let cfg = resolve(Some("explicit"), None).unwrap();
        std::env::remove_var("AWS_PROFILE");
        assert_eq!(cfg.profile, Some("explicit".to_string()));
    }

    #[test]
    #[serial]
    fn resolve_invalid_bin() {
        std::env::set_var("AWS_CLI_BIN", "nonexistent-aws-xyz-12345");
        let result = resolve(None, None);
        std::env::remove_var("AWS_CLI_BIN");
        match result.unwrap_err() {
            ConfigError::InvalidBin { bin } => {
                assert_eq!(bin, "nonexistent-aws-xyz-12345");
            }
            e => panic!("expected InvalidBin, got {e:?}"),
        }
    }
}
