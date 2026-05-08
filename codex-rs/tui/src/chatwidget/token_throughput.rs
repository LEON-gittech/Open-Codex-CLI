use crate::token_usage::TokenUsage;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct TokenThroughputTiming {
    duration_ms: i64,
    interval: Option<TokenThroughputInterval>,
}

impl TokenThroughputTiming {
    pub(super) fn from_turn(
        started_at: Option<i64>,
        completed_at: Option<i64>,
        duration_ms: Option<i64>,
    ) -> Option<Self> {
        let duration_ms = duration_ms?;
        if duration_ms <= 0 {
            return None;
        }
        let interval = match (started_at, completed_at) {
            (_, Some(completed_at)) => {
                let end_ms = completed_at.saturating_mul(1000);
                Some(TokenThroughputInterval {
                    start_ms: end_ms.saturating_sub(duration_ms),
                    end_ms,
                })
            }
            (Some(started_at), None) => {
                let start_ms = started_at.saturating_mul(1000);
                Some(TokenThroughputInterval {
                    start_ms,
                    end_ms: start_ms.saturating_add(duration_ms),
                })
            }
            _ => None,
        };
        Some(Self {
            duration_ms,
            interval,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct TokenThroughputInterval {
    start_ms: i64,
    end_ms: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct TokenThroughputSample {
    usage: TokenUsage,
    timing: TokenThroughputTiming,
}

impl TokenThroughputSample {
    pub(super) fn new(usage: TokenUsage, timing: TokenThroughputTiming) -> Self {
        Self { usage, timing }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(super) struct TokenThroughput {
    input_tokens_per_second: f64,
    output_tokens_per_second: f64,
}

impl TokenThroughput {
    pub(super) fn from_samples(samples: &[TokenThroughputSample]) -> Option<Self> {
        if samples.is_empty() {
            return None;
        }

        let mut input_tokens = 0;
        let mut output_tokens = 0;
        let mut fallback_duration_ms = 0;
        let mut intervals = Vec::new();

        for sample in samples {
            input_tokens += sample.usage.input_tokens.max(0);
            output_tokens += sample.usage.output_tokens.max(0);
            if let Some(interval) = sample.timing.interval {
                intervals.push(interval);
            } else {
                fallback_duration_ms += sample.timing.duration_ms;
            }
        }

        let duration_ms = merged_interval_duration_ms(intervals) + fallback_duration_ms;
        if duration_ms <= 0 {
            return None;
        }

        let elapsed_seconds = duration_ms as f64 / 1000.0;
        Some(Self {
            input_tokens_per_second: input_tokens as f64 / elapsed_seconds,
            output_tokens_per_second: output_tokens as f64 / elapsed_seconds,
        })
    }

    pub(super) fn display(&self) -> String {
        format!(
            "in {:.1} / out {:.1} tok/s",
            self.input_tokens_per_second, self.output_tokens_per_second
        )
    }
}

pub(super) fn unavailable_display() -> String {
    "in -- / out -- tok/s".to_string()
}

fn merged_interval_duration_ms(mut intervals: Vec<TokenThroughputInterval>) -> i64 {
    if intervals.is_empty() {
        return 0;
    }

    intervals.sort_by_key(|interval| interval.start_ms);
    let mut merged: Vec<TokenThroughputInterval> = Vec::new();
    for interval in intervals {
        if interval.end_ms <= interval.start_ms {
            continue;
        }
        if let Some(last) = merged.last_mut()
            && interval.start_ms <= last.end_ms
        {
            last.end_ms = last.end_ms.max(interval.end_ms);
            continue;
        }
        merged.push(interval);
    }

    merged
        .into_iter()
        .map(|interval| interval.end_ms - interval.start_ms)
        .sum()
}
