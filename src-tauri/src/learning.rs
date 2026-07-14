use chrono::{NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{SqlitePool, FromRow};
use tauri::State;

// ── Structs ──

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct HourlyFocus {
    pub hour: i32,
    pub total_seconds: i64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct EfficiencyPattern {
    pub hourly_focus: Vec<HourlyFocus>,
    pub peak_start_hour: Option<i32>,
    pub peak_end_hour: Option<i32>,
    pub avg_daily_focus_seconds: f64,
    pub total_focus_sessions: i32,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CalibrationSummary {
    pub overall_ratio: f64, // weighted avg of all ratios
    pub sample_count: i32,
    pub suggestion: String, // e.g. "建议将预估时长乘以 1.3"
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PeakHoursSuggestion {
    pub peak_hours: Vec<PeakRange>,
    pub insight: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PeakRange {
    pub start_hour: i32, // 0-23
    pub end_hour: i32,
    pub avg_focus_seconds: i64,
}

// ── Helpers ──

fn iso_date(d: NaiveDate) -> String {
    d.format("%Y-%m-%d").to_string()
}

/// Parse "YYYY-MM-DD" to NaiveDate. Returns error on invalid format.
#[allow(dead_code)]
fn parse_date(s: &str) -> Result<NaiveDate, String> {
    NaiveDate::parse_from_str(s, "%Y-%m-%d")
        .map_err(|e| format!("Invalid date format '{}', expected YYYY-MM-DD: {}", s, e))
}

// ── Tauri Commands ──

/// Analyze focus_sessions data to identify user's peak productivity hours.
#[tauri::command]
pub async fn get_efficiency_pattern(
    days: Option<i32>, // lookback window in days, default 14
    state: State<'_, SqlitePool>,
) -> Result<EfficiencyPattern, String> {
    let pool = state.inner();
    get_efficiency_pattern_inner(days.unwrap_or(14), pool).await
}

/// Find peak hours: find the contiguous block of 2-4 hours with max total focus.
fn find_peak_hours(hourly: &[HourlyFocus]) -> Option<PeakRange> {
    if hourly.is_empty() {
        return None;
    }
    // Build a 24-hour array
    let mut arr = [0i64; 24];
    for h in hourly {
        arr[h.hour as usize] = h.total_seconds;
    }

    let window_sizes = [2, 3, 4];
    let mut best_start: i32 = 0;
    let mut best_sum = -1i64;
    let mut best_size: i32 = 2;

    for &w in &window_sizes {
        for start in 0..24 {
            let mut sum = 0i64;
            for i in 0..w {
                sum += arr[(start + i) % 24];
            }
            if sum > best_sum {
                best_sum = sum;
                best_start = start as i32;
                best_size = w as i32;
            }
        }
    }

    if best_sum <= 0 {
        return None;
    }

    Some(PeakRange {
        start_hour: best_start,
        end_hour: (best_start + best_size - 1) % 24,
        avg_focus_seconds: best_sum / best_size as i64,
    })
}

/// Compare estimated_duration_min vs actual duration for completed tasks with focus sessions.
/// Calculate calibration coefficient and store in settings table.
#[tauri::command]
pub async fn calibrate_estimate(
    state: State<'_, SqlitePool>,
) -> Result<CalibrationSummary, String> {
    let pool = state.inner();

    // Find tasks with: estimated_duration_min > 0, status='done', and have at least one completed focus session
    #[derive(FromRow)]
    #[allow(dead_code)]
    struct CalibRow {
        task_id: String,
        estimated_min: i32,
        actual_seconds: i64,
    }

    let rows: Vec<CalibRow> = sqlx::query_as(
        "SELECT t.id as task_id, t.estimated_duration_min as estimated_min, \
                COALESCE(SUM( \
                    CAST((julianday(f.end_time) - julianday(f.start_time)) * 86400 AS INTEGER) \
                ), 0) as actual_seconds \
         FROM tasks t \
         LEFT JOIN focus_sessions f ON f.task_id = t.id AND f.end_time IS NOT NULL \
         WHERE t.status = 'done' AND t.estimated_duration_min > 0 \
         GROUP BY t.id \
         HAVING actual_seconds > 0 \
         ORDER BY t.created_at DESC \
         LIMIT 50",
    )
    .fetch_all(pool)
    .await
    .map_err(|e| format!("DB error: {}", e))?;

    if rows.is_empty() {
        return Ok(CalibrationSummary {
            overall_ratio: 1.0,
            sample_count: 0,
            suggestion: "暂无足够数据，请先完成更多任务并记录专注时间".to_string(),
        });
    }

    // Compute ratio for each, then harmonic mean (or weighted avg)
    let mut weighted_ratio_sum = 0.0;
    let mut weight_sum = 0.0;
    for row in &rows {
        let actual_min = (row.actual_seconds as f64 / 60.0).round() as i32;
        if row.estimated_min <= 0 {
            continue;
        }
        let ratio = actual_min as f64 / row.estimated_min as f64;
        // Weight by actual_min (longer tasks = more reliable)
        let weight = actual_min as f64;
        weighted_ratio_sum += ratio * weight;
        weight_sum += weight;
    }

    let overall_ratio = if weight_sum > 0.0 {
        (weighted_ratio_sum / weight_sum * 100.0).round() / 100.0
    } else {
        1.0
    };

    // Store in settings table
    let ratio_str = overall_ratio.to_string();
    sqlx::query(
        "INSERT INTO settings (key, value) VALUES ('calibration_ratio', ?) \
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
    )
    .bind(&ratio_str)
    .execute(pool)
    .await
    .map_err(|e| format!("DB error: {}", e))?;

    let suggestion = if overall_ratio > 1.3 {
        format!(
            "你的实际耗时平均是预估的 {:.1} 倍，建议将预估时长乘以 {:.1}",
            overall_ratio,
            (overall_ratio * 10.0).round() / 10.0
        )
    } else if overall_ratio < 0.8 {
        format!(
            "你的实际耗时平均是预估的 {:.1} 倍，建议适当缩短预估时长",
            overall_ratio
        )
    } else {
        "你的预估时长较为准确，保持即可".to_string()
    };

    Ok(CalibrationSummary {
        overall_ratio,
        sample_count: rows.len() as i32,
        suggestion,
    })
}

/// Internal helper: get efficiency pattern without going through Tauri command.
async fn get_efficiency_pattern_inner(
    days: i32,
    pool: &SqlitePool,
) -> Result<EfficiencyPattern, String> {
    let days = days.max(1);
    let since = iso_date(Utc::now().date_naive() - chrono::Duration::days(days as i64));

    // Hourly aggregation: extract hour from start_time
    let rows: Vec<(i32, i64)> = sqlx::query_as(
        "SELECT CAST(strftime('%H', start_time) AS INTEGER) as hr, \
                CAST((julianday(end_time) - julianday(start_time)) * 86400 AS INTEGER) as secs \
         FROM focus_sessions \
         WHERE date(start_time) >= ? AND end_time IS NOT NULL \
         ORDER BY hr ASC",
    )
    .bind(&since)
    .fetch_all(pool)
    .await
    .map_err(|e| format!("DB error: {}", e))?;

    // Aggregate by hour
    use std::collections::HashMap;
    let mut hour_map: HashMap<i32, i64> = HashMap::new();
    for (hr, secs) in rows {
        *hour_map.entry(hr).or_insert(0) += secs;
    }

    let mut hourly_focus: Vec<HourlyFocus> = hour_map
        .into_iter()
        .map(|(hour, total_seconds)| HourlyFocus { hour, total_seconds })
        .collect();
    hourly_focus.sort_by_key(|h| h.hour);

    // Find peak: top 2-3 contiguous hours with most focus seconds
    let peak = find_peak_hours(&hourly_focus);

    // Avg daily focus
    let total_focus: i64 = hourly_focus.iter().map(|h| h.total_seconds).sum();
    let avg_daily = total_focus as f64 / days as f64;

    let total_sessions: i32 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM focus_sessions \
         WHERE date(start_time) >= ? AND end_time IS NOT NULL",
    )
    .bind(&since)
    .fetch_one(pool)
    .await
    .map_err(|e| format!("DB error: {}", e))?;

    Ok(EfficiencyPattern {
        hourly_focus,
        peak_start_hour: peak.as_ref().map(|p| p.start_hour),
        peak_end_hour: peak.as_ref().map(|p| p.end_hour),
        avg_daily_focus_seconds: avg_daily,
        total_focus_sessions: total_sessions,
    })
}

/// Aggregate hourly focus seconds from the last `days` and return peak ranges
/// using the shared 2-4 hour window algorithm. This is the single source of
/// truth for "peak hours" consumed by both the learning module and the focus
/// module (for the start-focus linkage notification).
pub async fn compute_peak_ranges(pool: &SqlitePool, days: i32) -> Vec<PeakRange> {
    let days = days.max(1);
    let since = iso_date(Utc::now().date_naive() - chrono::Duration::days(days as i64));
    let rows: Vec<(i32, i64)> = sqlx::query_as(
        "SELECT CAST(strftime('%H', start_time) AS INTEGER) as hr, \
                CAST((julianday(end_time) - julianday(start_time)) * 86400 AS INTEGER) as secs \
         FROM focus_sessions \
         WHERE date(start_time) >= ? AND end_time IS NOT NULL \
         ORDER BY hr ASC",
    )
    .bind(&since)
    .fetch_all(pool)
    .await
    .unwrap_or_default();

    use std::collections::HashMap;
    let mut hour_map: HashMap<i32, i64> = HashMap::new();
    for (hr, secs) in rows {
        *hour_map.entry(hr).or_insert(0) += secs;
    }
    let hourly: Vec<HourlyFocus> = hour_map
        .into_iter()
        .map(|(hour, total_seconds)| HourlyFocus { hour, total_seconds })
        .collect();

    if let Some(peak) = find_peak_hours(&hourly) {
        vec![peak]
    } else {
        Vec::new()
    }
}

/// Cache peak hours into settings for cross-module linkage (focus module reads it).
pub async fn cache_peak_hours(pool: &SqlitePool) {
    let peaks = compute_peak_ranges(pool, 14).await;
    let json = serde_json::to_string(&peaks).unwrap_or_default();
    let _ = sqlx::query(
        "INSERT INTO settings (key, value) VALUES ('peak_hours_data', ?) \
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
    )
    .bind(&json)
    .execute(pool)
    .await;

    let _ = sqlx::query(
        "INSERT INTO settings (key, value) VALUES ('peak_hours_updated_at', ?) \
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
    )
    .bind(Utc::now().format("%Y-%m-%dT%H:%M:%S").to_string())
    .execute(pool)
    .await;
}

/// Return peak hours suggestion for the user.
#[tauri::command]
pub async fn get_peak_hours(
    state: State<'_, SqlitePool>,
) -> Result<PeakHoursSuggestion, String> {
    let pool = state.inner();

    // Get efficiency pattern for last 14 days
    let pattern_result = get_efficiency_pattern_inner(14, pool).await?;
    let hourly = &pattern_result.hourly_focus;

    // Also read calibration_ratio from settings
    let calibration_ratio: Option<String> = sqlx::query_scalar(
        "SELECT value FROM settings WHERE key = 'calibration_ratio' LIMIT 1",
    )
    .fetch_optional(pool)
    .await
    .map_err(|e| format!("DB error: {}", e))?;

    let ratio: f64 = calibration_ratio
        .and_then(|s| s.parse::<f64>().ok())
        .unwrap_or(1.0);

    // Build peak_hours list
    let mut peaks: Vec<PeakRange> = Vec::new();
    if let (Some(start), Some(end)) = (pattern_result.peak_start_hour, pattern_result.peak_end_hour)
    {
        peaks.push(PeakRange {
            start_hour: start,
            end_hour: end,
            avg_focus_seconds: pattern_result.avg_daily_focus_seconds as i64,
        });
    } else {
        // Fallback: find top 2 hours individually
        let mut sorted = hourly.clone();
        sorted.sort_by(|a, b| b.total_seconds.cmp(&a.total_seconds));
        for h in sorted.iter().take(2) {
            peaks.push(PeakRange {
                start_hour: h.hour,
                end_hour: h.hour,
                avg_focus_seconds: h.total_seconds,
            });
        }
    }

    let insight = if peaks.is_empty() {
        "暂无足够专注数据，请先开始使用专注模式".to_string()
    } else {
        let parts: Vec<String> = peaks
            .iter()
            .map(|p| {
                if p.start_hour == p.end_hour {
                    format!(
                        "{}:00-{:02}:00",
                        p.start_hour,
                        (p.start_hour + 1) % 24
                    )
                } else {
                    format!(
                        "{}:00-{:02}:00",
                        p.start_hour,
                        (p.end_hour + 1) % 24
                    )
                }
            })
            .collect();
        format!(
            "你的高效时段为 {}。校准系数：{:.1}（实际/预估）",
            parts.join("、"),
            ratio
        )
    };

    Ok(PeakHoursSuggestion { peak_hours: peaks, insight })
}

/// Get calibration ratio from settings (used by frontend to adjust estimates)
#[tauri::command]
pub async fn get_calibration_ratio(
    state: State<'_, SqlitePool>,
) -> Result<Option<f64>, String> {
    let pool = state.inner();
    let value: Option<String> = sqlx::query_scalar(
        "SELECT value FROM settings WHERE key = 'calibration_ratio' LIMIT 1",
    )
    .fetch_optional(pool)
    .await
    .map_err(|e| format!("DB error: {}", e))?;

    Ok(value.and_then(|s| s.parse::<f64>().ok()))
}

// ── Unit Tests ──

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_find_peak_hours_empty() {
        let result = find_peak_hours(&[]);
        assert!(result.is_none());
    }

    #[test]
    fn test_find_peak_hours_single_block() {
        let hourly = vec![
            HourlyFocus { hour: 9, total_seconds: 3600 },
            HourlyFocus { hour: 10, total_seconds: 7200 },
            HourlyFocus { hour: 11, total_seconds: 5400 },
            HourlyFocus { hour: 14, total_seconds: 1800 },
        ];
        let peak = find_peak_hours(&hourly).unwrap();
        assert_eq!(peak.start_hour, 9);
        // 9-11 is 3 hours, sum = 3600+7200+5400 = 16200
        assert_eq!(peak.end_hour, 11);
    }

    #[test]
    fn test_find_peak_hours_all_zero() {
        let mut hourly: Vec<HourlyFocus> = Vec::new();
        for h in 0..24 {
            hourly.push(HourlyFocus { hour: h, total_seconds: 0 });
        }
        let result = find_peak_hours(&hourly);
        assert!(result.is_none());
    }

    #[test]
    fn test_find_peak_hours_wraps_midnight() {
        let hourly = vec![
            HourlyFocus { hour: 23, total_seconds: 10000 },
            HourlyFocus { hour: 0, total_seconds: 8000 },
            HourlyFocus { hour: 1, total_seconds: 6000 },
            HourlyFocus { hour: 12, total_seconds: 1000 },
        ];
        let peak = find_peak_hours(&hourly).unwrap();
        // Best window should be 23-1 (wrapped) sum = 10000+8000+6000 = 24000
        assert_eq!(peak.start_hour, 23);
    }
}
