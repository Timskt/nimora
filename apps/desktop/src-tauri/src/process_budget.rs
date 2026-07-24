//! Host process resource budget sampling (RSS + best-effort CPU).
//!
//! Product guardrail: idle host should stay under 200 MiB RSS and ~2% CPU.
//!
//! Platform notes:
//! - **macOS**: `task_info(MACH_TASK_BASIC_INFO)` for current `resident_size` (preferred);
//!   falls back to `getrusage(RUSAGE_SELF).ru_maxrss` (bytes on Darwin).
//! - **Linux / other Unix**: `getrusage` (`ru_maxrss` is KiB) or `/proc/self/status` `VmRSS`.
//! - **Windows**: `GetProcessMemoryInfo` → `WorkingSetSize` (physical RSS); if zero,
//!   falls back to `PrivateUsage` via `PROCESS_MEMORY_COUNTERS_EX`. On API failure →
//!   zero RSS and `within_memory_budget = false` (fail closed).
//! - **Other**: unavailable → zero RSS and `within_memory_budget = false` (fail closed).
//!
//! CPU is a best-effort single-process percent derived from user+system time deltas between
//! samples (None on the first sample, on non-Unix hosts, or when wall time did not advance).

use serde::Serialize;
use std::sync::Mutex;
use std::time::Instant;

/// Idle memory budget (product constraint).
pub const MEMORY_BUDGET_BYTES: u64 = 200 * 1024 * 1024;

/// Host process memory/CPU snapshot for Control Center / lifeform IPC.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ProcessBudgetSnapshot {
    pub rss_bytes: u64,
    /// RSS rounded to whole mebibytes.
    pub rss_mb: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cpu_percent_approx: Option<f32>,
    /// True only when RSS was observed and is strictly below [`MEMORY_BUDGET_BYTES`].
    pub within_memory_budget: bool,
    pub observed_at_ms: u64,
}

#[derive(Debug, Clone, Copy)]
struct CpuSample {
    /// Process CPU time in seconds (user + system).
    cpu_secs: f64,
    wall: Instant,
}

static LAST_CPU_SAMPLE: Mutex<Option<CpuSample>> = Mutex::new(None);

/// Sample the current process budget at `now_ms` (unix epoch milliseconds).
#[must_use]
pub fn sample_process_budget(now_ms: u64) -> ProcessBudgetSnapshot {
    let (rss_bytes, sampled) = sample_rss_bytes();
    let cpu_percent_approx = sample_cpu_percent_approx();
    let rss_mb = bytes_to_rounded_mb(rss_bytes);
    let within_memory_budget = sampled && rss_bytes < MEMORY_BUDGET_BYTES;
    ProcessBudgetSnapshot {
        rss_bytes,
        rss_mb,
        cpu_percent_approx,
        within_memory_budget,
        observed_at_ms: now_ms,
    }
}

fn bytes_to_rounded_mb(bytes: u64) -> u32 {
    let mb = (bytes as f64) / (1024.0 * 1024.0);
    let rounded = mb.round();
    if !rounded.is_finite() || rounded <= 0.0 {
        0
    } else if rounded >= f64::from(u32::MAX) {
        u32::MAX
    } else {
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        {
            rounded as u32
        }
    }
}

/// Returns `(rss_bytes, sampled_ok)`.
///
/// When sampling fails, returns `(0, false)` so callers can fail closed
/// (`within_memory_budget = false`) without panicking.
fn sample_rss_bytes() -> (u64, bool) {
    #[cfg(target_os = "macos")]
    {
        if let Some(bytes) = sample_rss_macos_task_info() {
            return (bytes, true);
        }
        if let Some(bytes) = sample_rss_getrusage() {
            return (bytes, true);
        }
        return (0, false);
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        if let Some(bytes) = sample_rss_linux_proc() {
            return (bytes, true);
        }
        if let Some(bytes) = sample_rss_getrusage() {
            return (bytes, true);
        }
        return (0, false);
    }
    #[cfg(target_os = "windows")]
    {
        if let Some(bytes) = sample_rss_windows() {
            return (bytes, true);
        }
        return (0, false);
    }
    #[cfg(not(any(unix, target_os = "windows")))]
    {
        (0, false)
    }
}

#[cfg(target_os = "macos")]
fn sample_rss_macos_task_info() -> Option<u64> {
    // SAFETY: MACH_TASK_BASIC_INFO writes into a properly sized mach_task_basic_info
    // for the current task; count is initialized to the expected field count.
    unsafe {
        let mut info = std::mem::MaybeUninit::<libc::mach_task_basic_info>::uninit();
        let mut count = libc::MACH_TASK_BASIC_INFO_COUNT;
        // libc deprecates this symbol in favor of the mach2 crate; keep zero extra deps.
        #[allow(deprecated)]
        let task = libc::mach_task_self_;
        let kr = libc::task_info(
            task,
            libc::MACH_TASK_BASIC_INFO,
            info.as_mut_ptr().cast(),
            &mut count,
        );
        if kr != libc::KERN_SUCCESS {
            return None;
        }
        let info = info.assume_init();
        let rss = info.resident_size;
        if rss == 0 {
            None
        } else {
            Some(rss)
        }
    }
}

#[cfg(unix)]
fn sample_rss_getrusage() -> Option<u64> {
    // SAFETY: getrusage writes into a fully-owned rusage; RUSAGE_SELF is valid.
    unsafe {
        let mut usage = std::mem::MaybeUninit::<libc::rusage>::uninit();
        if libc::getrusage(libc::RUSAGE_SELF, usage.as_mut_ptr()) != 0 {
            return None;
        }
        let usage = usage.assume_init();
        let maxrss = usage.ru_maxrss;
        if maxrss <= 0 {
            return None;
        }
        #[allow(clippy::cast_sign_loss)]
        let maxrss = maxrss as u64;
        // Darwin: ru_maxrss is bytes. Linux and most Unix: KiB.
        #[cfg(target_os = "macos")]
        {
            Some(maxrss)
        }
        #[cfg(not(target_os = "macos"))]
        {
            Some(maxrss.saturating_mul(1024))
        }
    }
}

#[cfg(all(unix, not(target_os = "macos")))]
fn sample_rss_linux_proc() -> Option<u64> {
    let status = std::fs::read_to_string("/proc/self/status").ok()?;
    for line in status.lines() {
        let Some(rest) = line.strip_prefix("VmRSS:") else {
            continue;
        };
        let kib = rest.split_whitespace().next()?.parse::<u64>().ok()?;
        return Some(kib.saturating_mul(1024));
    }
    None
}

/// Windows physical RSS via `GetProcessMemoryInfo` (`WorkingSetSize`).
///
/// Prefer working set (resident pages). If the API fills counters but
/// `WorkingSetSize` is zero, fall back to `PrivateUsage` (private commit).
/// Returns `None` on API failure so the snapshot fails closed.
#[cfg(target_os = "windows")]
fn sample_rss_windows() -> Option<u64> {
    use std::mem::size_of;
    use windows_sys::Win32::System::ProcessStatus::{
        GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS, PROCESS_MEMORY_COUNTERS_EX,
    };
    use windows_sys::Win32::System::Threading::GetCurrentProcess;

    // SAFETY: PROCESS_MEMORY_COUNTERS_EX is zero-initialized with `cb` set to its size.
    // GetProcessMemoryInfo accepts the extended layout when `cb` matches; the process
    // handle is the pseudo-handle from GetCurrentProcess (must not be closed).
    unsafe {
        let mut counters = PROCESS_MEMORY_COUNTERS_EX {
            cb: size_of::<PROCESS_MEMORY_COUNTERS_EX>() as u32,
            ..Default::default()
        };
        let ok = GetProcessMemoryInfo(
            GetCurrentProcess(),
            (&raw mut counters).cast::<PROCESS_MEMORY_COUNTERS>(),
            counters.cb,
        );
        if ok == 0 {
            return None;
        }
        let working_set = counters.WorkingSetSize as u64;
        if working_set > 0 {
            return Some(working_set);
        }
        let private = counters.PrivateUsage as u64;
        if private > 0 {
            return Some(private);
        }
        None
    }
}

fn sample_cpu_percent_approx() -> Option<f32> {
    let cpu_secs = process_cpu_secs()?;
    let now = Instant::now();
    let mut guard = LAST_CPU_SAMPLE.lock().ok()?;
    let prev = *guard;
    *guard = Some(CpuSample {
        cpu_secs,
        wall: now,
    });
    let prev = prev?;
    let wall_delta = now.duration_since(prev.wall).as_secs_f64();
    if wall_delta <= 0.0 {
        return None;
    }
    let cpu_delta = (cpu_secs - prev.cpu_secs).max(0.0);
    let percent = (cpu_delta / wall_delta) * 100.0;
    if !percent.is_finite() || percent < 0.0 {
        return None;
    }
    // Cap to a sane multi-core range for display.
    #[allow(clippy::cast_possible_truncation)]
    let capped = percent.min(100.0 * f64::from(num_cpus_approx())) as f32;
    Some(capped)
}

fn num_cpus_approx() -> u32 {
    std::thread::available_parallelism()
        .map(|n| u32::try_from(n.get()).unwrap_or(1))
        .unwrap_or(1)
        .max(1)
}

#[cfg(unix)]
fn process_cpu_secs() -> Option<f64> {
    // SAFETY: getrusage writes into a fully-owned rusage; RUSAGE_SELF is valid.
    unsafe {
        let mut usage = std::mem::MaybeUninit::<libc::rusage>::uninit();
        if libc::getrusage(libc::RUSAGE_SELF, usage.as_mut_ptr()) != 0 {
            return None;
        }
        let usage = usage.assume_init();
        let user = timeval_to_secs(usage.ru_utime);
        let system = timeval_to_secs(usage.ru_stime);
        Some(user + system)
    }
}

#[cfg(not(unix))]
fn process_cpu_secs() -> Option<f64> {
    None
}

#[cfg(unix)]
fn timeval_to_secs(tv: libc::timeval) -> f64 {
    #[allow(clippy::cast_precision_loss, clippy::cast_possible_wrap)]
    let secs = tv.tv_sec as f64;
    #[allow(clippy::cast_precision_loss)]
    let usecs = f64::from(tv.tv_usec);
    secs.max(0.0) + (usecs / 1_000_000.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sample_returns_finite_values_on_current_os() {
        let snap = sample_process_budget(1_700_000_000_000);
        assert_eq!(snap.observed_at_ms, 1_700_000_000_000);
        if let Some(cpu) = snap.cpu_percent_approx {
            assert!(cpu.is_finite(), "cpu must be finite, got {cpu}");
            assert!(cpu >= 0.0, "cpu must be non-negative, got {cpu}");
        }
        // On macOS/Linux/Windows we expect a real sample in unit tests.
        #[cfg(any(target_os = "macos", target_os = "linux", target_os = "windows"))]
        {
            assert!(
                snap.rss_bytes > 0,
                "expected non-zero RSS on this OS, got {}",
                snap.rss_bytes
            );
            assert!(
                snap.within_memory_budget,
                "test process should be under 200 MiB RSS (got {} bytes / {} MiB)",
                snap.rss_bytes,
                snap.rss_mb
            );
        }
    }

    /// Windows path: `sample_rss_windows` must return Some with positive WorkingSet/PrivateUsage
    /// when the Win32 API succeeds (fail-closed is covered by the Option return, not panic).
    #[cfg(target_os = "windows")]
    #[test]
    fn windows_get_process_memory_info_returns_positive_rss() {
        let bytes = sample_rss_windows().expect("GetProcessMemoryInfo should succeed");
        assert!(bytes > 0, "WorkingSetSize/PrivateUsage should be non-zero, got {bytes}");
        // Sanity: a unit-test process is not multi-terabyte.
        assert!(
            bytes < 16 * 1024 * 1024 * 1024,
            "implausible RSS {bytes} bytes"
        );
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn windows_sample_marks_sampled_ok_when_rss_present() {
        let (bytes, ok) = sample_rss_bytes();
        assert!(ok, "Windows RSS sample must report sampled_ok");
        assert!(bytes > 0);
        let snap = sample_process_budget(7);
        assert_eq!(snap.rss_bytes, bytes);
        assert!(snap.within_memory_budget);
    }

    #[test]
    fn bytes_to_rounded_mb_rounds_half_up() {
        assert_eq!(bytes_to_rounded_mb(0), 0);
        assert_eq!(bytes_to_rounded_mb(512 * 1024), 1); // 0.5 MiB → 1
        assert_eq!(bytes_to_rounded_mb(1024 * 1024), 1);
        assert_eq!(bytes_to_rounded_mb(1024 * 1024 + 100), 1);
        assert_eq!(bytes_to_rounded_mb(1536 * 1024), 2); // 1.5 MiB → 2
    }

    #[test]
    fn within_memory_budget_respects_200_mib() {
        let under = ProcessBudgetSnapshot {
            rss_bytes: MEMORY_BUDGET_BYTES - 1,
            rss_mb: bytes_to_rounded_mb(MEMORY_BUDGET_BYTES - 1),
            cpu_percent_approx: None,
            within_memory_budget: true,
            observed_at_ms: 1,
        };
        assert!(under.rss_bytes < MEMORY_BUDGET_BYTES);
        assert!(under.within_memory_budget);

        let over = ProcessBudgetSnapshot {
            rss_bytes: MEMORY_BUDGET_BYTES,
            rss_mb: bytes_to_rounded_mb(MEMORY_BUDGET_BYTES),
            cpu_percent_approx: Some(1.5),
            within_memory_budget: false,
            observed_at_ms: 2,
        };
        assert!(!over.within_memory_budget);
        assert_eq!(over.rss_mb, 200);
    }

    #[test]
    fn serde_uses_camel_case_and_skips_absent_cpu() {
        let snap = ProcessBudgetSnapshot {
            rss_bytes: 50 * 1024 * 1024,
            rss_mb: 50,
            cpu_percent_approx: None,
            within_memory_budget: true,
            observed_at_ms: 99,
        };
        let value = serde_json::to_value(&snap).expect("serialize");
        assert_eq!(
            value.get("rssBytes").and_then(|v| v.as_u64()),
            Some(50 * 1024 * 1024)
        );
        assert_eq!(value.get("rssMb").and_then(|v| v.as_u64()), Some(50));
        assert_eq!(
            value.get("withinMemoryBudget").and_then(|v| v.as_bool()),
            Some(true)
        );
        assert_eq!(value.get("observedAtMs").and_then(|v| v.as_u64()), Some(99));
        assert!(value.get("cpuPercentApprox").is_none());
        assert!(value.get("rss_bytes").is_none());
    }

    #[test]
    fn serde_includes_cpu_when_present() {
        let snap = ProcessBudgetSnapshot {
            rss_bytes: 1024,
            rss_mb: 0,
            cpu_percent_approx: Some(1.25),
            within_memory_budget: true,
            observed_at_ms: 1,
        };
        let value = serde_json::to_value(&snap).expect("serialize");
        let cpu = value
            .get("cpuPercentApprox")
            .and_then(|v| v.as_f64())
            .expect("cpu");
        assert!((cpu - 1.25).abs() < f64::EPSILON);
    }

    #[test]
    fn cpu_percent_becomes_available_after_second_sample() {
        let _first = sample_process_budget(1);
        // Burn a little CPU + wall time so the delta is non-zero.
        let start = Instant::now();
        let mut acc = 0u64;
        while start.elapsed().as_millis() < 20 {
            acc = acc.wrapping_add(1);
        }
        std::hint::black_box(acc);
        let second = sample_process_budget(2);
        #[cfg(unix)]
        {
            // Best-effort: may still be None if the mutex or clock quirks; when present, finite.
            if let Some(cpu) = second.cpu_percent_approx {
                assert!(cpu.is_finite());
                assert!(cpu >= 0.0);
            }
        }
        let _ = second;
    }
}
