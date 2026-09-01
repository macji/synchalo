use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HlcTimestamp {
    pub physical_ms: i64,
    pub logical: u32,
}

#[derive(Debug, Clone, Default)]
pub struct HlcClock {
    last: HlcTimestamp,
}

impl HlcClock {
    pub fn from_timestamp(last: HlcTimestamp) -> Self {
        Self { last }
    }

    pub fn last(&self) -> HlcTimestamp {
        self.last
    }

    pub fn tick(&mut self, now_ms: i64) -> HlcTimestamp {
        if now_ms > self.last.physical_ms {
            self.last = HlcTimestamp {
                physical_ms: now_ms,
                logical: 0,
            };
        } else {
            self.last.logical = self.last.logical.saturating_add(1);
        }
        self.last
    }

    pub fn merge(&mut self, remote: HlcTimestamp, now_ms: i64) -> HlcTimestamp {
        let local = self.last;
        let physical_ms = now_ms.max(local.physical_ms).max(remote.physical_ms);
        let logical = if physical_ms == local.physical_ms && physical_ms == remote.physical_ms {
            local.logical.max(remote.logical).saturating_add(1)
        } else if physical_ms == local.physical_ms {
            local.logical.saturating_add(1)
        } else if physical_ms == remote.physical_ms {
            remote.logical.saturating_add(1)
        } else {
            0
        };

        self.last = HlcTimestamp {
            physical_ms,
            logical,
        };
        self.last
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tick_is_monotonic_when_wall_clock_moves_backwards() {
        let mut clock = HlcClock::default();
        let first = clock.tick(100);
        let second = clock.tick(90);

        assert!(second > first);
        assert_eq!(second.physical_ms, 100);
        assert_eq!(second.logical, 1);
    }

    #[test]
    fn merge_orders_future_local_events_after_remote() {
        let mut clock = HlcClock::from_timestamp(HlcTimestamp {
            physical_ms: 100,
            logical: 2,
        });
        let merged = clock.merge(
            HlcTimestamp {
                physical_ms: 100,
                logical: 7,
            },
            95,
        );

        assert_eq!(merged.physical_ms, 100);
        assert_eq!(merged.logical, 8);
        assert!(clock.tick(96) > merged);
    }
}
