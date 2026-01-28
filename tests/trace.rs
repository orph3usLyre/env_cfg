//! Tests for the `trace` feature
//!
//! Note: The trace feature requires structs to implement Debug.
//! This test verifies that the derive macro works correctly with
//! the trace feature enabled.

#[cfg(test)]
#[cfg(feature = "trace")]
mod common;

#[cfg(feature = "trace")]
mod trace_tests {
    use super::*;
    use common::with_env_vars;
    use env_cfg::EnvConfig;

    /// Test that a struct with Debug derives correctly with trace feature
    #[derive(EnvConfig, Debug, PartialEq)]
    struct TracedConfig {
        api_key: String,
        port: u16,
        debug: Option<bool>,
    }

    #[test]
    fn should_work_with_trace_feature_and_debug() {
        let config = unsafe {
            with_env_vars(
                &[
                    ("TRACED_CONFIG_API_KEY", "secret"),
                    ("TRACED_CONFIG_PORT", "8080"),
                    ("TRACED_CONFIG_DEBUG", "true"),
                ],
                || TracedConfig::from_env().unwrap(),
            )
        };
        assert_eq!(
            config,
            TracedConfig {
                api_key: "secret".to_string(),
                port: 8080,
                debug: Some(true),
            }
        );
    }

    /// Test with a tracing subscriber to verify trace events are emitted
    #[test]
    fn should_emit_trace_event() {
        use std::sync::Arc;
        use std::sync::atomic::{AtomicBool, Ordering};

        // A simple subscriber that records whether a trace event occurred
        #[derive(Clone)]
        struct TestSubscriber {
            trace_called: Arc<AtomicBool>,
        }

        impl tracing::Subscriber for TestSubscriber {
            fn enabled(&self, _metadata: &tracing::Metadata<'_>) -> bool {
                true
            }

            fn new_span(&self, _span: &tracing::span::Attributes<'_>) -> tracing::span::Id {
                tracing::span::Id::from_u64(1)
            }

            fn record(&self, _span: &tracing::span::Id, _values: &tracing::span::Record<'_>) {}

            fn record_follows_from(&self, _span: &tracing::span::Id, _follows: &tracing::span::Id) {
            }

            fn event(&self, event: &tracing::Event<'_>) {
                // Check if this is a trace-level event from our macro
                if event.metadata().level() == &tracing::Level::TRACE {
                    self.trace_called.store(true, Ordering::SeqCst);
                }
            }

            fn enter(&self, _span: &tracing::span::Id) {}

            fn exit(&self, _span: &tracing::span::Id) {}
        }

        let trace_called = Arc::new(AtomicBool::new(false));
        let subscriber = TestSubscriber {
            trace_called: trace_called.clone(),
        };

        unsafe {
            with_env_vars(
                &[
                    ("TRACED_CONFIG_API_KEY", "test"),
                    ("TRACED_CONFIG_PORT", "3000"),
                ],
                || {
                    tracing::subscriber::with_default(subscriber.clone(), || {
                        let _config = TracedConfig::from_env().unwrap();
                    });
                },
            )
        };

        assert!(
            trace_called.load(Ordering::SeqCst),
            "Expected trace event to be emitted"
        );
    }
}
