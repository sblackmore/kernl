use std::collections::HashMap;
use std::time::{Duration, Instant};

pub struct RateLimiter {
    requests: HashMap<String, Vec<Instant>>,
    window: Duration,
    max_requests: usize,
}

impl RateLimiter {
    pub fn new(window_secs: u64, max_requests: usize) -> Self {
        Self {
            requests: HashMap::new(),
            window: Duration::from_secs(window_secs),
            max_requests,
        }
    }

    pub fn check(&mut self, ip: &str) -> bool {
        let now = Instant::now();
        let reqs = self.requests.entry(ip.to_string()).or_default();
        reqs.retain(|t| now.duration_since(*t) < self.window);
        if reqs.len() >= self.max_requests {
            return false;
        }
        reqs.push(now);
        true
    }

    pub fn remaining(&self, ip: &str) -> usize {
        let now = Instant::now();
        let count = self
            .requests
            .get(ip)
            .map(|reqs| {
                reqs.iter()
                    .filter(|t| now.duration_since(**t) < self.window)
                    .count()
            })
            .unwrap_or(0);
        self.max_requests.saturating_sub(count)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allows_under_limit() {
        let mut rl = RateLimiter::new(60, 5);
        assert!(rl.check("192.168.1.1"));
        assert!(rl.check("192.168.1.1"));
        assert!(rl.check("192.168.1.1"));
    }

    #[test]
    fn blocks_over_limit() {
        let mut rl = RateLimiter::new(60, 3);
        assert!(rl.check("10.0.0.1"));
        assert!(rl.check("10.0.0.1"));
        assert!(rl.check("10.0.0.1"));
        assert!(!rl.check("10.0.0.1"));
    }

    #[test]
    fn different_ips_tracked_separately() {
        let mut rl = RateLimiter::new(60, 2);
        assert!(rl.check("10.0.0.1"));
        assert!(rl.check("10.0.0.1"));
        assert!(!rl.check("10.0.0.1"));
        assert!(rl.check("10.0.0.2"));
    }

    #[test]
    fn remaining_count_accurate() {
        let mut rl = RateLimiter::new(60, 5);
        assert_eq!(rl.remaining("10.0.0.1"), 5);
        rl.check("10.0.0.1");
        assert_eq!(rl.remaining("10.0.0.1"), 4);
        rl.check("10.0.0.1");
        assert_eq!(rl.remaining("10.0.0.1"), 3);
    }

    #[test]
    fn unknown_ip_has_full_remaining() {
        let rl = RateLimiter::new(60, 10);
        assert_eq!(rl.remaining("unknown"), 10);
    }

    #[test]
    fn zero_limit_blocks_everything() {
        let mut rl = RateLimiter::new(60, 0);
        assert!(!rl.check("10.0.0.1"));
        assert_eq!(rl.remaining("10.0.0.1"), 0);
    }
}
