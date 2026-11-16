//! One timing entry per operation, never request bodies, credentials or article text.
use std::{
    sync::atomic::{AtomicU64, Ordering},
    time::Instant,
};
static NEXT: AtomicU64 = AtomicU64::new(1);
// Verbose pipeline/timing logs are opt-in, including in development builds.
pub(crate) fn diagnostics_enabled() -> bool {
    static ENABLED: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ENABLED.get_or_init(|| std::env::var("ARABRIEF_DIAGNOSTICS").is_ok_and(|value| value == "1"))
}
macro_rules! diagnostic {
    ($($arg:tt)*) => {
        if crate::performance::diagnostics_enabled() {
            eprintln!($($arg)*);
        }
    };
}
pub(crate) use diagnostic;
pub(crate) struct Span {
    id: u64,
    operation: &'static str,
    started: Instant,
    status: &'static str,
}
impl Span {
    pub fn new(operation: &'static str) -> Self {
        Self {
            id: NEXT.fetch_add(1, Ordering::Relaxed),
            operation,
            started: Instant::now(),
            status: "error_or_cancelled",
        }
    }
    pub fn success(&mut self) {
        self.status = "ok";
    }
    pub fn cached(&mut self) {
        self.status = "cache_hit";
    }
}
impl Drop for Span {
    fn drop(&mut self) {
        crate::performance::diagnostic!(
            "[perf] id={} operation={} elapsed_ms={:.3} status={}",
            self.id,
            self.operation,
            self.started.elapsed().as_secs_f64() * 1000.0,
            self.status
        );
    }
}
pub(crate) fn measure<T, E>(
    operation: &'static str,
    action: impl FnOnce() -> Result<T, E>,
) -> Result<T, E> {
    let mut span = Span::new(operation);
    let result = action();
    if result.is_ok() {
        span.success();
    }
    result
}
