//! Order statistics over a handful of measurements (runs of a bench, frames of a run).

/// Median, `NaN` when there are no values.
#[must_use]
pub fn median(values: impl IntoIterator<Item = f64>) -> f64 {
    let mut values: Vec<f64> = values.into_iter().collect();
    if values.is_empty() {
        return f64::NAN;
    }
    values.sort_by(f64::total_cmp);
    let middle = values.len() / 2;
    if values.len() % 2 == 1 {
        values[middle]
    } else {
        f64::midpoint(values[middle - 1], values[middle])
    }
}

/// Largest minus smallest value: how far apart a handful of runs landed. `NaN` when there are
/// no values.
#[must_use]
pub fn spread(values: impl IntoIterator<Item = f64>) -> f64 {
    let (low, high) = values
        .into_iter()
        .fold((f64::INFINITY, f64::NEG_INFINITY), |(low, high), value| {
            (low.min(value), high.max(value))
        });
    if low > high { f64::NAN } else { high - low }
}

/// Nearest-rank percentile (`percent` in 0–100) of values sorted in ascending order, `NaN` when
/// there are none.
#[must_use]
pub fn percentile_of_sorted(sorted: &[f64], percent: f64) -> f64 {
    if sorted.is_empty() {
        return f64::NAN;
    }
    let rank = (percent / 100.0 * to_f64(sorted.len())).ceil();
    let index = to_index(rank).saturating_sub(1).min(sorted.len() - 1);
    sorted[index]
}

#[allow(clippy::cast_precision_loss)]
const fn to_f64(value: usize) -> f64 {
    value as f64
}

#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn to_index(rank: f64) -> usize {
    if rank.is_nan() || rank < 0.0 {
        0
    } else {
        rank as usize
    }
}
