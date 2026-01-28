//!
//! ```rust
//! use env_cfg::EnvConfig;
//! use std::time::Duration;
//!
//! #[derive(Debug, EnvConfig)]
//! struct AppConfig {
//!     // By default, we search for ENV variables using STRUCT_NAME_FIELD_NAME in SCREAMING_SNAKE_CASE.
//!     // `database_url` will be loaded from `APP_CONFIG_DATABASE_URL`
//!     url: String, // -> APP_CONFIG_URL (required)
//!
//!     // if a default value is provided, that value is used as a fallback
//!     #[env_cfg(default = "8080")]
//!     port: u16, // -> APP_CONFIG_PORT (with default)
//!
//!     timeout: Option<u64>, // -> APP_CONFIG_TIMEOUT (optional)
//!
//!     // custom ENV variable keys can be provided with `env = "CUSTOM_NAME"`
//!     #[env_cfg(env = "DEBUG_MODE")]
//!     debug: bool, // -> DEBUG_MODE (custom name)
//!
//!     // fields marked with `skip` will always use the `Default` impl for the type
//!     #[env_cfg(skip)]
//!     internal_state: Option<String>, // Skipped - uses Default::default()
//!
//!     // fields marked with `parse_with = "my_fn_name"` will use the provided function to parse the env variable.
//!     // These functions must have the signature `fn(String) -> T`
//!     #[env_cfg(parse_with = "parse_point")]
//!     position: Point, // -> APP_CONFIG_POSITION (with custom parser)
//!
//!     // fields marked with `parse_with = "my_fn_name"` can also be optional
//!     #[env_cfg(parse_with = "parse_timeout_with_default")]
//!     timeout_duration: Option<Duration>, // -> APP_CONFIG_TIMEOUT_DURATION (with custom parser that provides defaults)
//!
//!     #[env_cfg(nested)]
//!     db_config: DatabaseConfig,
//!
//!     #[env_cfg(nested)]
//!     redis_config: RedisConfig,
//! }
//!
//! #[derive(Debug, EnvConfig)]
//! // Use no_prefix to disable the struct name prefix
//! #[env_cfg(no_prefix)]
//! struct DatabaseConfig {
//!     postgres_url: String, // -> POSTGRES_URL
//!     #[env_cfg(env = "DB_NAME", default = "mydb")]
//!     database: String, // -> DB_NAME (with default)
//! }
//!
//! #[derive(Debug, EnvConfig)]
//! // Use custom prefix instead of struct name
//! #[env_cfg(prefix = "REDIS")]
//! struct RedisConfig {
//!     url: String, // -> REDIS_URL
//!     #[env_cfg(default = "5")]
//!     cache_timeout: u64, // -> REDIS_CACHE_TIMEOUT (with default)
//! }
//!
//! #[derive(Debug)]
//! struct Point {
//!     x: f64,
//!     y: f64,
//! }
//!
//! fn parse_point(s: String) -> Point {
//!     let (x, y) = s.split_once(',').expect("Invalid format");
//!     Point {
//!         x: x.trim().parse().expect("Invalid x coordinate"),
//!         y: y.trim().parse().expect("Invalid y coordinate"),
//!     }
//! }
//!
//! fn parse_timeout_with_default(s: String) -> Duration {
//!     Duration::from_secs(s.parse::<u64>().unwrap_or(100))
//! }
//!
//! fn main() -> Result<(), env_cfg::EnvConfigError> {
//!     // Set some environment variables for demonstration
//!     //
//!     // # Safety
//!     // This example cannot run in parallel with other programs that set/remove ENV variables
//!     unsafe {
//!         std::env::set_var("APP_CONFIG_URL", "0.0.0.0:8080");
//!         std::env::set_var("APP_CONFIG_TIMEOUT", "42");
//!         std::env::set_var("DEBUG_MODE", "true");
//!         std::env::set_var("APP_CONFIG_POSITION", "42.43, 893.2123");
//!         std::env::set_var("APP_CONFIG_TIMEOUT_DURATION", "243");
//!         std::env::set_var("POSTGRES_URL", "postgres://postgres:postgres@0.0.0.0:5432");
//!         std::env::set_var("REDIS_URL", "redis://localhost:6379");
//!     }
//!     let config = AppConfig::from_env()?;
//!     println!("AppConfig: {config:#?}");
//!
//!     Ok(())
//! }
//! ```

use std::str::FromStr;

// Re-export the derive macro
pub use env_cfg_derive::EnvConfig;

/// Trait for loading configuration from environment variables.
///
/// This trait provides an interface for loading configuration from environment variables.
///
/// # Derive Macro Example
///
/// ```rust
/// use env_cfg::*;
///
/// #[derive(Debug, EnvConfig)]
/// struct AppConfig {
///     database_url: String,                          // -> APP_CONFIG_DATABASE_URL (required)
///     timeout: Option<u64>,                          // -> APP_CONFIG_TIMEOUT (optional)
///     #[env_cfg(env = "DEBUG_MODE")]
///     debug: bool,                                   // -> DEBUG_MODE (custom name)
///     #[env_cfg(skip)]
///     internal_state: String,                        // Skipped - uses Default::default()
/// }
///
/// // Use no_prefix to disable the struct name prefix
/// #[derive(Debug, EnvConfig)]
/// #[env_cfg(no_prefix)]
/// struct SimpleConfig {
///     database_url: String,                          // -> DATABASE_URL (no prefix)
/// }
///
/// // Use custom prefix instead of struct name
/// #[derive(Debug, EnvConfig)]
/// #[env_cfg(prefix = "APP")]
/// struct CustomConfig {
///     database_url: String,                          // -> APP_DATABASE_URL (custom prefix)
/// }
/// ```
///
/// # Derive Macro Attributes
///
/// **Struct-level attributes:**
/// - **`#[env_cfg(no_prefix)]`**: Don't use struct name as prefix for field names
/// - **`#[env_cfg(prefix = "PREFIX")]`**: Use custom prefix instead of struct name
///
/// **Field-level attributes:**
/// - **No attribute**: Field name is prefixed with struct name and converted to UPPER_SNAKE_CASE
/// - **`#[env_cfg(env = "VAR_NAME")]`**: Use custom environment variable name
/// - **`#[env_cfg(default = "value")]`**: Provide default value if env var not set
/// - **`#[env_cfg(skip)]`**: Skip this field (must implement `Default`)
/// - **`#[env_cfg(parse_with = "function_name")]`**: Use custom parser function (takes `String`, returns `T`)
/// - **`#[env_cfg(nested)]`**: Treat field as nested EnvConfig struct (calls `T::from_env()`)
pub trait EnvConfig: Sized {
    /// Error type returned by `from_env()`.
    type Error;

    /// Load configuration from environment variables.
    fn from_env() -> Result<Self, Self::Error>;
}

/// Error type for environment configuration loading.
#[derive(Debug, thiserror::Error)]
pub enum EnvConfigError {
    /// Environment variable is missing.
    #[error("Missing environment variable: `{0}`")]
    Missing(String),
    /// Failed to parse environment variable value.
    #[error("Failed to parse environment variable: '{0}': {1}")]
    Parse(String, String),
    /// Error from a nested EnvConfig struct.
    #[error("Nested config `{0}` failed: {1}")]
    Nested(String, Box<EnvConfigError>),
}

/// Convert a `std::env::VarError` to an `EnvConfigError`.
fn var_error_to_env_config_error(name: &str, err: std::env::VarError) -> EnvConfigError {
    match err {
        std::env::VarError::NotPresent => EnvConfigError::Missing(name.to_string()),
        std::env::VarError::NotUnicode(_) => {
            EnvConfigError::Parse(name.to_string(), "Invalid Unicode".to_string())
        }
    }
}

// Helper functions for implementing the trait
/// Load a required environment variable and parse it to the target type.
/// Fails if the variable is not set or cannot be parsed.
#[must_use = "this returns a Result that should be handled"]
pub fn env_var<T>(name: &str) -> Result<T, EnvConfigError>
where
    T: FromStr,
    T::Err: std::fmt::Display,
{
    let value = std::env::var(name).map_err(|e| var_error_to_env_config_error(name, e))?;
    value
        .parse::<T>()
        .map_err(|e| EnvConfigError::Parse(name.to_string(), e.to_string()))
}

/// Load an optional environment variable and parse it to the target type.
/// Returns `None` if the variable is not set.
#[must_use = "this returns a Result that should be handled"]
pub fn env_var_optional<T>(name: &str) -> Result<Option<T>, EnvConfigError>
where
    T: FromStr,
    T::Err: std::fmt::Display,
{
    match std::env::var(name) {
        Ok(value) => value
            .parse::<T>()
            .map(Some)
            .map_err(|e| EnvConfigError::Parse(name.to_string(), e.to_string())),
        Err(std::env::VarError::NotPresent) => Ok(None),
        Err(e) => Err(var_error_to_env_config_error(name, e)),
    }
}

/// Load an environment variable with a string default that gets parsed if env var not present.
///
/// This is used by the derive macro for `#[env_cfg(default = "value")]` attributes.
#[must_use = "this returns a Result that should be handled"]
pub fn env_var_or_parse<T>(name: &str, default: &str) -> Result<T, EnvConfigError>
where
    T: FromStr,
    T::Err: std::fmt::Display,
{
    match std::env::var(name) {
        Ok(value) => value
            .parse::<T>()
            .map_err(|e| EnvConfigError::Parse(name.to_string(), e.to_string())),
        Err(std::env::VarError::NotPresent) => default
            .parse::<T>()
            .map_err(|e| EnvConfigError::Parse(format!("default for {}", name), e.to_string())),
        Err(e) => Err(var_error_to_env_config_error(name, e)),
    }
}

/// Load a required environment variable and parse it using a custom parser function.
/// The parser function should take a String and return the target type T.
/// Any panics or errors from the parser function will bubble up naturally.
#[must_use = "this returns a Result that should be handled"]
pub fn env_var_with_parser<T, F>(name: &str, parser: F) -> Result<T, EnvConfigError>
where
    F: FnOnce(String) -> T,
{
    let value = std::env::var(name).map_err(|e| var_error_to_env_config_error(name, e))?;
    Ok(parser(value))
}

/// Load an optional environment variable and parse it using a custom parser function.
/// Returns None if the variable is not set.
/// The parser function should take a String and return the target type T.
/// Any panics or errors from the parser function will bubble up naturally.
#[must_use = "this returns a Result that should be handled"]
pub fn env_var_optional_with_parser<T, F>(
    name: &str,
    parser: F,
) -> Result<Option<T>, EnvConfigError>
where
    F: FnOnce(String) -> T,
{
    match std::env::var(name) {
        Ok(value) => Ok(Some(parser(value))),
        Err(std::env::VarError::NotPresent) => Ok(None),
        Err(e) => Err(var_error_to_env_config_error(name, e)),
    }
}
