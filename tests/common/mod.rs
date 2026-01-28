use std::sync::{Mutex, OnceLock};

static TEST_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

fn get_test_lock() -> &'static Mutex<()> {
    TEST_LOCK.get_or_init(|| Mutex::new(()))
}

// NOTE: this is a helper function that runs a test with a given array of env vars to set and
// cleanup. This function MUST be used for all tests that run in parallel, even if they do not set
// ENV vars, since the ENV variables might be polluted by other tests running.
//
// WARNING: SAFETY: There can be no more than a SINGLE test running with this wrapper
// at a time. This is achieved by grabbing the static lock at the beginning of each test and
// dropping it at the end of it's execution. We catch any unwinds to avoid poisoning the lock
// (which would fail subsequetial tests)
// See https://doc.rust-lang.org/std/env/fn.set_var.html#safety for details
pub unsafe fn with_env_vars<U, F: FnOnce() -> U + std::panic::UnwindSafe>(
    vars: &[(&str, &str)],
    test: F,
) -> U {
    // acquire lock
    let _guard = get_test_lock().lock().unwrap();
    // set all vars
    for (key, value) in vars {
        unsafe { std::env::set_var(key, value) };
    }

    // run the test
    let result = std::panic::catch_unwind(test);

    // clean up
    for (key, _) in vars {
        unsafe { std::env::remove_var(key) };
    }
    // lock drops
    drop(_guard);
    match result {
        Ok(val) => val,
        Err(err) => std::panic::resume_unwind(err),
    }
}
