pub mod instrument;

use std::collections::HashMap;
use std::time::{Duration, Instant};

#[derive(Debug, Clone)]
pub struct FunctionProfile {
    pub name: String,
    pub call_count: u64,
    pub total_time: Duration,
    pub self_time: Duration,
    pub avg_time: Duration,
    pub max_time: Duration,
    pub min_time: Duration,
}

pub struct Profiler {
    profiles: HashMap<String, ProfileAccumulator>,
    call_stack: Vec<(String, Instant)>,
    enabled: bool,
}

struct ProfileAccumulator {
    call_count: u64,
    total_time: Duration,
    self_time: Duration,
    max_time: Duration,
    min_time: Duration,
}

impl Profiler {
    pub fn new() -> Self {
        Self {
            profiles: HashMap::new(),
            call_stack: Vec::new(),
            enabled: false,
        }
    }

    pub fn enable(&mut self) {
        self.enabled = true;
    }

    pub fn disable(&mut self) {
        self.enabled = false;
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    pub fn enter(&mut self, func_name: &str) {
        if !self.enabled {
            return;
        }
        self.call_stack
            .push((func_name.to_string(), Instant::now()));
    }

    pub fn exit(&mut self, func_name: &str) {
        if !self.enabled {
            return;
        }
        if let Some((name, start)) = self.call_stack.pop() {
            assert_eq!(name, func_name);
            let elapsed = start.elapsed();
            let acc = self
                .profiles
                .entry(name)
                .or_insert_with(|| ProfileAccumulator {
                    call_count: 0,
                    total_time: Duration::ZERO,
                    self_time: Duration::ZERO,
                    max_time: Duration::ZERO,
                    min_time: Duration::MAX,
                });
            acc.call_count += 1;
            acc.total_time += elapsed;
            acc.self_time += elapsed;
            acc.max_time = acc.max_time.max(elapsed);
            acc.min_time = acc.min_time.min(elapsed);
        }
    }

    pub fn results(&self) -> Vec<FunctionProfile> {
        let mut profiles: Vec<FunctionProfile> = self
            .profiles
            .iter()
            .map(|(name, acc)| {
                let avg = if acc.call_count > 0 {
                    acc.total_time / acc.call_count as u32
                } else {
                    Duration::ZERO
                };
                FunctionProfile {
                    name: name.clone(),
                    call_count: acc.call_count,
                    total_time: acc.total_time,
                    self_time: acc.self_time,
                    avg_time: avg,
                    max_time: acc.max_time,
                    min_time: if acc.min_time == Duration::MAX {
                        Duration::ZERO
                    } else {
                        acc.min_time
                    },
                }
            })
            .collect();
        profiles.sort_by(|a, b| b.total_time.cmp(&a.total_time));
        profiles
    }

    pub fn report(&self) -> String {
        let results = self.results();
        let mut out = String::new();
        out.push_str(&format!(
            "{:<30} {:>8} {:>12} {:>12} {:>12}\n",
            "function", "calls", "total", "avg", "max"
        ));
        out.push_str(&"-".repeat(76));
        out.push('\n');
        for p in &results {
            out.push_str(&format!(
                "{:<30} {:>8} {:>12.3?} {:>12.3?} {:>12.3?}\n",
                p.name, p.call_count, p.total_time, p.avg_time, p.max_time
            ));
        }
        out
    }

    pub fn reset(&mut self) {
        self.profiles.clear();
        self.call_stack.clear();
    }
}

impl Default for Profiler {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;

    #[test]
    fn profiler_enter_exit_tracking() {
        let mut profiler = Profiler::new();
        profiler.enable();

        profiler.enter("foo");
        thread::sleep(Duration::from_millis(1));
        profiler.exit("foo");

        let results = profiler.results();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].name, "foo");
        assert_eq!(results[0].call_count, 1);
        assert!(results[0].total_time >= Duration::from_millis(1));
    }

    #[test]
    fn profiler_call_counting() {
        let mut profiler = Profiler::new();
        profiler.enable();

        for _ in 0..5 {
            profiler.enter("bar");
            profiler.exit("bar");
        }

        let results = profiler.results();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].call_count, 5);
    }

    #[test]
    fn profiler_report_formatting() {
        let mut profiler = Profiler::new();
        profiler.enable();

        profiler.enter("alpha");
        profiler.exit("alpha");
        profiler.enter("beta");
        profiler.exit("beta");

        let report = profiler.report();
        assert!(report.contains("function"));
        assert!(report.contains("calls"));
        assert!(report.contains("total"));
        assert!(report.contains("avg"));
        assert!(report.contains("max"));
        assert!(report.contains("alpha"));
        assert!(report.contains("beta"));
        assert!(report.contains("---"));
    }

    #[test]
    fn profiler_zero_calls() {
        let profiler = Profiler::new();
        let results = profiler.results();
        assert!(results.is_empty());

        let report = profiler.report();
        assert!(report.contains("function"));
        assert!(!report.contains("foo"));
    }

    #[test]
    fn profiler_disabled_no_tracking() {
        let mut profiler = Profiler::new();
        // Not enabled

        profiler.enter("baz");
        profiler.exit("baz");

        assert!(profiler.results().is_empty());
    }

    #[test]
    fn profiler_reset_clears_data() {
        let mut profiler = Profiler::new();
        profiler.enable();

        profiler.enter("func");
        profiler.exit("func");
        assert_eq!(profiler.results().len(), 1);

        profiler.reset();
        assert!(profiler.results().is_empty());
    }

    #[test]
    fn profiler_min_max_tracking() {
        let mut profiler = Profiler::new();
        profiler.enable();

        profiler.enter("timed");
        thread::sleep(Duration::from_millis(1));
        profiler.exit("timed");

        profiler.enter("timed");
        thread::sleep(Duration::from_millis(5));
        profiler.exit("timed");

        let results = profiler.results();
        assert_eq!(results[0].call_count, 2);
        assert!(results[0].max_time >= results[0].min_time);
    }

    #[test]
    fn profiler_results_sorted_by_total_time() {
        let mut profiler = Profiler::new();
        profiler.enable();

        profiler.enter("fast");
        profiler.exit("fast");

        profiler.enter("slow");
        thread::sleep(Duration::from_millis(5));
        profiler.exit("slow");

        let results = profiler.results();
        assert_eq!(results.len(), 2);
        assert!(results[0].total_time >= results[1].total_time);
    }
}
