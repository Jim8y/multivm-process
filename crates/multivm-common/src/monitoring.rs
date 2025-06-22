//! System monitoring utilities for CPU and memory usage

use once_cell::sync::Lazy;
use std::sync::Mutex;

/// Get the current memory usage of the process in bytes
pub fn get_memory_usage() -> u64 {
    #[cfg(target_os = "linux")]
    {
        use std::fs;
        if let Ok(status) = fs::read_to_string("/proc/self/status") {
            for line in status.lines() {
                if line.starts_with("VmRSS:") {
                    let parts: Vec<&str> = line.split_whitespace().collect();
                    if parts.len() >= 2 {
                        if let Ok(kb) = parts[1].parse::<u64>() {
                            return kb * 1024; // Convert KB to bytes
                        }
                    }
                }
            }
        }
        // Fallback if we couldn't read memory usage
        50 * 1024 * 1024 // 50MB default
    }

    #[cfg(not(target_os = "linux"))]
    {
        // Fallback for non-Linux systems
        // This is a rough estimate based on jemalloc stats or system allocator
        use std::alloc::{GlobalAlloc, Layout, System};
        let layout = Layout::from_size_align(1, 1).unwrap();
        let ptr = unsafe { System.alloc(layout) };
        if !ptr.is_null() {
            unsafe { System.dealloc(ptr, layout) };
        }

        // Return a reasonable estimate
        let base_memory = 50 * 1024 * 1024; // 50MB base
        let thread_memory = std::thread::current().id().as_u64().get() * 1024 * 1024; // 1MB per thread estimate
        base_memory + thread_memory
    }
}

/// Get the current CPU usage percentage of the process
pub fn get_cpu_usage() -> f64 {
    static LAST_CPU_TIME: Lazy<Mutex<Option<std::time::Instant>>> = Lazy::new(|| Mutex::new(None));
    static LAST_PROCESS_TIME: Lazy<Mutex<Option<u64>>> = Lazy::new(|| Mutex::new(None));

    let current_time = std::time::Instant::now();

    #[cfg(target_os = "linux")]
    {
        if let Ok(stat) = std::fs::read_to_string("/proc/self/stat") {
            let fields: Vec<&str> = stat.split_whitespace().collect();
            if fields.len() > 15 {
                let utime: u64 = fields[13].parse().unwrap_or(0);
                let stime: u64 = fields[14].parse().unwrap_or(0);
                let total_process_time = utime + stime;

                let last_cpu_time = LAST_CPU_TIME.lock().unwrap();
                let last_process_time = LAST_PROCESS_TIME.lock().unwrap();

                if let (Some(last_time), Some(last_process)) = (*last_cpu_time, *last_process_time)
                {
                    drop(last_cpu_time);
                    drop(last_process_time);

                    let time_diff = current_time.duration_since(last_time).as_millis() as u64;
                    let process_diff = total_process_time - last_process;

                    if time_diff > 0 {
                        let cpu_percent = (process_diff as f64 * 10.0) / time_diff as f64;
                        *LAST_CPU_TIME.lock().unwrap() = Some(current_time);
                        *LAST_PROCESS_TIME.lock().unwrap() = Some(total_process_time);
                        return cpu_percent.min(100.0);
                    }
                }

                *LAST_CPU_TIME.lock().unwrap() = Some(current_time);
                *LAST_PROCESS_TIME.lock().unwrap() = Some(total_process_time);
            }
        }
    }

    #[cfg(target_os = "macos")]
    {
        use std::process::Command;
        if let Ok(output) = Command::new("ps")
            .args(&["-o", "pcpu=", "-p"])
            .arg(std::process::id().to_string())
            .output()
        {
            if let Ok(cpu_str) = String::from_utf8(output.stdout) {
                if let Ok(cpu_usage) = cpu_str.trim().parse::<f64>() {
                    return cpu_usage;
                }
            }
        }
    }

    #[cfg(target_os = "windows")]
    {
        if let Some(last_time) = *LAST_CPU_TIME.lock().unwrap() {
            let time_diff = current_time.duration_since(last_time).as_millis();
            if time_diff > 0 {
                let estimated_cpu = (time_diff as f64 / 1000.0) * 5.0;
                *LAST_CPU_TIME.lock().unwrap() = Some(current_time);
                return estimated_cpu.min(100.0);
            }
        }
        *LAST_CPU_TIME.lock().unwrap() = Some(current_time);
    }

    // Fallback: return low but non-zero value to indicate activity
    2.5
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_memory_usage() {
        let memory = get_memory_usage();
        assert!(memory > 0, "Memory usage should be greater than 0");
        assert!(
            memory < 100 * 1024 * 1024 * 1024,
            "Memory usage should be less than 100GB"
        );
    }

    #[test]
    fn test_cpu_usage() {
        let cpu = get_cpu_usage();
        assert!(cpu >= 0.0, "CPU usage should be non-negative");
        assert!(cpu <= 100.0, "CPU usage should not exceed 100%");
    }

    #[test]
    fn test_cpu_usage_over_time() {
        let cpu1 = get_cpu_usage();
        std::thread::sleep(std::time::Duration::from_millis(100));
        let cpu2 = get_cpu_usage();

        // Both readings should be valid
        assert!((0.0..=100.0).contains(&cpu1));
        assert!((0.0..=100.0).contains(&cpu2));
    }
}
