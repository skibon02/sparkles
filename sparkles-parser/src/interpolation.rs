use std::collections::BTreeMap;

#[derive(Default)]
pub struct InterpolationPoints(BTreeMap<u64, u64>); // key: cpu timestamp or external timestamp, value: monotonic timestamp

impl InterpolationPoints {
    pub fn new() -> Self {
        Self(BTreeMap::new())
    }

    pub fn add_interpolation_point(&mut self, monotonic_tm: u64, cur_tm: u64) {
        self.0.insert(cur_tm, monotonic_tm);
    }
    pub fn get_avg_ticks_per_ns(&self) -> Option<f64> {
        let (&first_tm, first_monotonic) = self.0.iter().next()?;
        let (&last_tm, last_monotonic) = self.0.iter().next_back()?;

        let dur_ns = (*last_monotonic - *first_monotonic) as f64;
        let dur_ticks = (last_tm - first_tm) as f64;
        if dur_ns == 0.0 {
            return None;
        }
        Some(dur_ticks / dur_ns)
    }

    pub fn is_empty(&self) -> bool {
        self.0.len() < 2
    }

    /// Projects cpu timestamp or external timestamp to monotonic timestamps using received sync points
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
}
