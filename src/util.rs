use std::env::VarError;
use std::env;

pub const HOST: &str = "HOST";
pub const PORT: &str = "PORT";

pub const VAR_TOPIC: &str = "TOPIC";
pub const VAR_TEAM_ID: &str = "TEAM_ID";
pub const VAR_TOKEN_KEY_PATH: &str = "TOKEN_KEY_PATH";
pub const VAR_AUTH_KEY_ID: &str = "AUTH_KEY_ID";
pub const VAR_APNS_HOST_NAME: &str = "APNS_HOST_NAME";

pub fn check_environment_vars() -> Result<(), VarError> {
    env::var(VAR_TOPIC)?;
    env::var(VAR_TEAM_ID)?;
    env::var(VAR_TOKEN_KEY_PATH)?;
    env::var(VAR_AUTH_KEY_ID)?;
    env::var(VAR_APNS_HOST_NAME)?;
    Ok(())
}
