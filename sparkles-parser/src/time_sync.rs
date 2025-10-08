use std::collections::BTreeMap;

#[derive(Default)]
pub struct TimeSyncPoints(BTreeMap<u64, u64>); // key: cpu timestamp or external timestamp, value: monotonic timestamp

impl TimeSyncPoints {
    pub fn new() -> Self {
        Self(BTreeMap::new())
    }

    /// Push two timestamps from source and destination time domains, captured in the same time.
    /// `project_tm` can be used to convert time from source domain to destination domain.
    pub fn add_time_sync_point(&mut self, dst_tm: u64, src_tm: u64) {
        self.0.insert(src_tm, dst_tm);
    }
    /// Returns average frequency of the source time domain
    pub fn get_avg_ticks_per_ns(&self) -> Option<f64> {
        let (&first_tm, &first_monotonic) = self.0.iter().next()?;
        let (&last_tm, &last_monotonic) = self.0.iter().next_back()?;

        let dur_ns = (last_monotonic - first_monotonic) as f64;
        let dur_ticks = (last_tm - first_tm) as f64;
        if dur_ns == 0.0 {
            return None;
        }
        Some(dur_ticks / dur_ns)
    }

    pub fn is_empty(&self) -> bool {
        self.0.len() < 2
    }

    /// Convert timestamp from source domain to destination domain.
    /// If there are <2 sync points in total, returns None.
    /// If tm is not within known sync points, returns None.
    pub fn project_tm(&self, tm: u64) -> Option<u64> {
        let inter_points = &self.0;
        let closest_left = inter_points.range(..=tm).next_back();
        let closest_right = inter_points.range(tm..).next();

        let (Some((left_tm, left_ns)), Some((right_tm, right_ns))) = (closest_left, closest_right) else {
            return None;
        };

        let fraction = (tm - *left_tm) as f64 / (*right_tm - *left_tm) as f64;
        let projected = (*left_ns as f64) + fraction * ((*right_ns - *left_ns) as f64);

        Some(projected as u64)
    }

    /// Convert timestamp from source domain to destination domain.
    /// If there are <2 sync points in total, returns None.
    /// If tm is less than first stored sync point, returns None.
    /// Otherwise, predicts timestamp from two closest right time sync points
    pub fn project_tm_predict(&self, tm: u64) -> Option<u64> {
        let res = self.project_tm(tm);
        if res.is_some() {
            return res;
        }
        // try to use future 2 points
        let closest_left = self.0.range(..=tm).rev().take(2).collect::<Vec<_>>();
        if closest_left.len() < 2 {
            return None;
        }

        let (left_tm, left_ns) = closest_left[1];
        let (right_tm, right_ns) = closest_left[0];
        let fraction = (tm - *left_tm) as f64 / (*right_tm - *left_tm) as f64;
        let predicted = (*left_ns as f64) + fraction * ((*right_ns - *left_ns) as f64);
        Some(predicted as u64)
    }

    pub fn src_bounds(&self) -> Option<(u64, u64)> {
        if self.0.len() < 2 {
            return None;
        }
        let (&first_tm, _) = self.0.iter().next()?;
        let (&last_tm, _) = self.0.iter().next_back()?;
        Some((first_tm, last_tm))
    }
}

pub struct MonotonicTimeSyncPoints(TimeSyncPoints);
impl MonotonicTimeSyncPoints {
    pub fn new() -> Self {
        Self(TimeSyncPoints::new())
    }
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
    /// Returns average frequency of the source time domain
    pub fn get_avg_ticks_per_ns(&self) -> Option<f64> {
        self.0.get_avg_ticks_per_ns()
    }

    /// Push two timestamps from source and destination time domains, captured in the same time.
    /// `project_tm` can be used to convert time from source domain to destination domain.
    ///
    /// NOTE! each call to add_time_sync_point must have strictly increasing src_tm
    pub fn add_time_sync_point(&mut self, dst_tm: u64, src_tm: u64) {
        assert!(self.0.0.last_entry().is_none_or(|e| src_tm > *e.key() && dst_tm >= *e.get()));
        self.0.add_time_sync_point(dst_tm, src_tm);
    }

    fn project_tm_past(&self, tm: u64) -> Option<u64> {
        // Try two closest right points
        let closest_right = self.0.0.range(tm..).take(2).collect::<Vec<_>>();
        if closest_right.len() < 2 {
            return None;
        }
        let (left_tm, left_ns) = closest_right[0];
        let (right_tm, right_ns) = closest_right[1];
        let fraction = (*left_tm - tm) as f64 / (*right_tm - *left_tm) as f64;
        let past_projected = (*left_ns as f64) - fraction * ((*right_ns - *left_ns) as f64);
        Some(past_projected as u64)
    }

    /// Convert timestamp from source domain to destination domain.
    /// If timestamp is out of known range, attempt to use future 2 points
    ///
    /// If tm is higher than last stored sync point, returns None.
    /// If there are <2 sync points in total, returns None.
    pub fn project_tm(&self, tm: u64) -> Option<u64> {
        self.0.project_tm(tm).or_else(||{
            self.project_tm_past(tm)
        })
    }
    /// Convert timestamp from source domain to destination domain.
    /// If there are <2 sync points in total, returns None.
    /// Otherwise, predicts timestamp from two latest time sync points
    ///
    /// NOTE! predicted timestamps may be changed by calls to add_time_sync_point
    pub fn project_tm_predict(&self, tm: u64) -> Option<u64> {
        self.0.project_tm_predict(tm).or_else(|| {
            self.project_tm_past(tm)
        })
    }

    pub fn src_bounds(&self) -> Option<(u64, u64)> {
        self.0.src_bounds()
    }
}
