// Nested EnvConfig tests
use env_cfg::{EnvConfig, EnvConfigError};

mod common;

#[derive(Debug, EnvConfig, PartialEq)]
#[env_cfg(no_prefix)]
struct DatabaseConfig {
    host: String, // -> HOST
    port: u16,    // -> PORT
    #[env_cfg(default = "myapp")]
    database: String, // -> DATABASE
}

#[derive(Debug, EnvConfig, PartialEq)]
#[env_cfg(no_prefix)]
struct RedisConfig {
    #[env_cfg(env = "REDIS_URL")]
    url: String, // -> REDIS_URL
    #[env_cfg(env = "REDIS_TIMEOUT", default = "5")]
    timeout: u64, // -> REDIS_TIMEOUT
}

#[derive(Debug, EnvConfig, PartialEq)]
#[env_cfg(no_prefix)]
struct AppConfig {
    #[env_cfg(nested)]
    database: DatabaseConfig,

    #[env_cfg(default = "info")]
    log_level: String, // -> LOG_LEVEL
}

#[derive(Debug, EnvConfig, PartialEq)]
#[env_cfg(no_prefix)]
struct MultiNestedConfig {
    #[env_cfg(nested)]
    database: DatabaseConfig,

    #[env_cfg(nested)]
    redis: RedisConfig,

    app_name: String, // -> APP_NAME
}

#[test]
fn should_parse_nested_config() {
    const ENV_VARS: &[(&str, &str)] = &[
        ("HOST", "localhost"),
        ("PORT", "5432"),
        ("DATABASE", "testdb"),
        ("LOG_LEVEL", "debug"),
    ];

    let config = unsafe { common::with_env_vars(ENV_VARS, || AppConfig::from_env().unwrap()) };

    assert_eq!(config.database.host, "localhost");
    assert_eq!(config.database.port, 5432);
    assert_eq!(config.database.database, "testdb");
    assert_eq!(config.log_level, "debug");
}

#[test]
fn should_parse_nested_config_with_defaults() {
    const ENV_VARS: &[(&str, &str)] = &[
        ("HOST", "localhost"),
        ("PORT", "5432"),
        // Don't set DATABASE or LOG_LEVEL to test defaults
    ];

    let config = unsafe { common::with_env_vars(ENV_VARS, || AppConfig::from_env().unwrap()) };

    assert_eq!(config.database.host, "localhost");
    assert_eq!(config.database.port, 5432);
    assert_eq!(config.database.database, "myapp"); // default
    assert_eq!(config.log_level, "info"); // default
}

#[test]
fn should_parse_multiple_nested_configs() {
    const ENV_VARS: &[(&str, &str)] = &[
        ("HOST", "db.example.com"),
        ("PORT", "5432"),
        ("DATABASE", "prod"),
        ("REDIS_URL", "redis://localhost:6379"),
        ("REDIS_TIMEOUT", "10"),
        ("APP_NAME", "my-app"),
    ];

    let config =
        unsafe { common::with_env_vars(ENV_VARS, || MultiNestedConfig::from_env().unwrap()) };

    assert_eq!(config.database.host, "db.example.com");
    assert_eq!(config.database.port, 5432);
    assert_eq!(config.database.database, "prod");
    assert_eq!(config.redis.url, "redis://localhost:6379");
    assert_eq!(config.redis.timeout, 10);
    assert_eq!(config.app_name, "my-app");
}

#[test]
fn should_propagate_nested_config_errors() {
    const ENV_VARS: &[(&str, &str)] = &[
        ("HOST", "localhost"),
        ("PORT", "not_a_number"), // Invalid port
        ("LOG_LEVEL", "debug"),
    ];

    let result = unsafe { common::with_env_vars(ENV_VARS, AppConfig::from_env) };

    if let Err(EnvConfigError::Nested(struct_name, inner)) = result {
        assert_eq!(struct_name, "DatabaseConfig");
        assert!(matches!(*inner, EnvConfigError::Parse(_, _)));
    } else {
        panic!("Expected Nested error wrapping a Parse error, got: {:?}", result);
    }
}

#[test]
fn should_fail_when_nested_required_vars_missing() {
    const ENV_VARS: &[(&str, &str)] = &[
        // Don't set HOST - this should cause the nested config to fail
        ("PORT", "5432"),
        ("LOG_LEVEL", "debug"),
    ];

    let result = unsafe { common::with_env_vars(ENV_VARS, AppConfig::from_env) };

    if let Err(EnvConfigError::Nested(struct_name, inner)) = result {
        assert_eq!(struct_name, "DatabaseConfig");
        assert!(matches!(*inner, EnvConfigError::Missing(_)));
    } else {
        panic!("Expected Nested error wrapping a Missing error, got: {:?}", result);
    }
}

#[test]
fn should_parse_multiple_nested_with_defaults() {
    const ENV_VARS: &[(&str, &str)] = &[
        ("HOST", "localhost"),
        ("PORT", "5432"),
        ("REDIS_URL", "redis://localhost:6379"),
        // Don't set DATABASE, REDIS_TIMEOUT to test defaults
        ("APP_NAME", "test-app"),
    ];

    let config =
        unsafe { common::with_env_vars(ENV_VARS, || MultiNestedConfig::from_env().unwrap()) };

    assert_eq!(config.database.host, "localhost");
    assert_eq!(config.database.port, 5432);
    assert_eq!(config.database.database, "myapp"); // default
    assert_eq!(config.redis.url, "redis://localhost:6379");
    assert_eq!(config.redis.timeout, 5); // default
    assert_eq!(config.app_name, "test-app");
}

// Test validation: nested cannot be combined with other attributes
#[test]
fn test_nested_with_parse_with_should_not_compile() {
    // This test exists to document that the following should NOT compile:
    //
    // #[derive(EnvConfig)]
    // struct InvalidConfig {
    //     #[env_cfg(nested, parse_with = "some_parser")]
    //     field: SomeType,
    // }
    //
    // The macro should panic with: "Cannot use 'nested' with 'default' or 'parse_with' attributes"
}

#[test]
fn test_nested_with_default_should_not_compile() {
    // This test exists to document that the following should NOT compile:
    //
    // #[derive(EnvConfig)]
    // struct InvalidConfig {
    //     #[env_cfg(nested, default = "something")]
    //     field: SomeType,
    // }
    //
    // The macro should panic with: "Cannot use 'nested' with 'default' or 'parse_with' attributes"
}
