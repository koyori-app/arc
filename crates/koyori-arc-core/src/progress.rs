/// Build the Progress line polyline points.
///
/// The Progress line (also called the "status line" or イナズマ線) is a zigzag that
/// passes through each task bar at the point corresponding to its `progress` value.
/// A task ahead of schedule causes the line to jag right; behind schedule jags left.
///
/// Each task contributes exactly one representative point at `(progress_x, y_mid)`.
/// Consecutive points are joined by diagonal straight segments — no horizontal jogs
/// or vertical bar-penetration (right-angle elbows).
///
/// When `today_x` is `Some`, endpoints are anchored on the today vertical line:
/// `(today_x, y_top)` of the first row → task midpoints → `(today_x, y_bottom)` of
/// the last row. Connections from/to anchors are diagonal when `progress_x ≠ today_x`.
///
/// When `today_x` is `None`, the legacy path connects task midpoints only
/// (see SPEC.md).
///
/// Returns a list of (x, y) pixel coordinates for an SVG `<polyline>`.
pub fn progress_line(
    tasks: &[(f64, f64, f64, f64, f64)], // (start_px, end_px, y_top, y_bottom, progress 0–1)
    today_x: Option<f64>,
) -> Vec<(f64, f64)> {
    if tasks.is_empty() {
        return Vec::new();
    }

    match today_x {
        Some(today_x) => progress_line_today_anchored(tasks, today_x),
        None => progress_line_legacy(tasks),
    }
}

fn progress_x(start: f64, end: f64, p: f64) -> f64 {
    start + (end - start) * p
}

fn y_mid(y_top: f64, y_bottom: f64) -> f64 {
    (y_top + y_bottom) / 2.0
}

fn task_midpoint(start: f64, end: f64, y_top: f64, y_bottom: f64, p: f64) -> (f64, f64) {
    (progress_x(start, end, p), y_mid(y_top, y_bottom))
}

fn progress_line_legacy(tasks: &[(f64, f64, f64, f64, f64)]) -> Vec<(f64, f64)> {
    tasks
        .iter()
        .map(|&(start, end, y_top, y_bottom, p)| task_midpoint(start, end, y_top, y_bottom, p))
        .collect()
}

fn progress_line_today_anchored(
    tasks: &[(f64, f64, f64, f64, f64)],
    today_x: f64,
) -> Vec<(f64, f64)> {
    let mut pts = Vec::with_capacity(tasks.len() + 2);
    let &(start0, end0, y_top0, y_bottom0, p0) = &tasks[0];
    let (mut x0, mid0) = task_midpoint(start0, end0, y_top0, y_bottom0, p0);
    if p0 == 0.0 {
        x0 = x0.min(today_x);
    }

    pts.push((today_x, y_top0));
    pts.push((x0, mid0));

    for &(start, end, y_top, y_bottom, p) in tasks.iter().skip(1) {
        let (mut x, y) = task_midpoint(start, end, y_top, y_bottom, p);
        if p == 0.0 {
            x = x.min(today_x);
        }
        pts.push((x, y));
    }

    let last_y = tasks.last().map(|t| t.3).unwrap_or(y_bottom0);
    pts.push((today_x, last_y));

    pts
}

/// True when segment a→b is horizontal or vertical (axis-aligned).
#[cfg(test)]
fn segment_is_axis_aligned(a: (f64, f64), b: (f64, f64)) -> bool {
    (a.0 - b.0).abs() < f64::EPSILON || (a.1 - b.1).abs() < f64::EPSILON
}

/// True when any interior segment between consecutive task midpoints is axis-aligned.
#[cfg(test)]
fn interior_segments_are_diagonal(pts: &[(f64, f64)]) -> bool {
    if pts.len() < 3 {
        return true;
    }
    // Skip first and last segments (today anchor legs may be vertical on today_x).
    pts.windows(2)
        .skip(1)
        .take(pts.len().saturating_sub(2))
        .all(|w| !segment_is_axis_aligned(w[0], w[1]))
}

/// True when any segment vertically spans a full task row (bar penetration).
#[cfg(test)]
fn has_vertical_bar_penetration(
    pts: &[(f64, f64)],
    tasks: &[(f64, f64, f64, f64, f64)],
) -> bool {
    for w in pts.windows(2) {
        let (a, b) = (w[0], w[1]);
        if (a.0 - b.0).abs() >= f64::EPSILON {
            continue;
        }
        let min_y = a.1.min(b.1);
        let max_y = a.1.max(b.1);
        for &(_, _, y_top, y_bottom, _) in tasks {
            if (min_y - y_top).abs() < f64::EPSILON && (max_y - y_bottom).abs() < f64::EPSILON {
                return true;
            }
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    fn one(progress: f64) -> Vec<(f64, f64)> {
        progress_line(&[(0.0, 100.0, 0.0, 40.0, progress)], None)
    }

    fn one_today(progress: f64, today_x: f64) -> Vec<(f64, f64)> {
        progress_line(&[(0.0, 100.0, 0.0, 40.0, progress)], Some(today_x))
    }

    #[test]
    fn empty_input() {
        assert!(progress_line(&[], None).is_empty());
        assert!(progress_line(&[], Some(50.0)).is_empty());
    }

    #[test]
    fn single_task_not_started_legacy() {
        let pts = one(0.0);
        assert_eq!(pts, vec![(0.0, 20.0)]);
    }

    #[test]
    fn single_task_complete_legacy() {
        let pts = one(1.0);
        assert_eq!(pts, vec![(100.0, 20.0)]);
    }

    #[test]
    fn single_task_halfway_legacy() {
        let pts = one(0.5);
        assert_eq!(pts, vec![(50.0, 20.0)]);
    }

    #[test]
    fn two_tasks_diagonal_segment_between_midpoints_legacy() {
        let tasks = [
            (0.0, 100.0, 0.0, 40.0, 0.5),
            (0.0, 100.0, 40.0, 80.0, 0.75),
        ];
        let pts = progress_line(&tasks, None);
        assert_eq!(pts.len(), 2);
        assert_eq!(pts[0], (50.0, 20.0));
        assert_eq!(pts[1], (75.0, 60.0));
        assert!(!segment_is_axis_aligned(pts[0], pts[1]));
        assert!(!has_vertical_bar_penetration(&pts, &tasks));
    }

    #[test]
    fn single_task_today_anchored_starts_and_ends_on_today() {
        let pts = one_today(0.5, 30.0);
        assert_eq!(pts.first(), Some(&(30.0, 0.0)));
        assert_eq!(pts.last(), Some(&(30.0, 40.0)));
        assert_eq!(pts[1], (50.0, 20.0));
    }

    #[test]
    fn single_task_today_anchored_at_progress_point_uses_midpoint() {
        let pts = one_today(0.3, 30.0);
        assert_eq!(pts, vec![(30.0, 0.0), (30.0, 20.0), (30.0, 40.0)]);
    }

    #[test]
    fn two_tasks_today_anchored_diagonal_through_midpoints() {
        let tasks = [
            (0.0, 100.0, 0.0, 40.0, 0.5),
            (0.0, 100.0, 40.0, 80.0, 0.75),
        ];
        let pts = progress_line(&tasks, Some(20.0));
        assert_eq!(pts.first(), Some(&(20.0, 0.0)));
        assert_eq!(pts.last(), Some(&(20.0, 80.0)));
        assert_eq!(pts[1], (50.0, 20.0));
        assert_eq!(pts[2], (75.0, 60.0));
        assert!(interior_segments_are_diagonal(&pts));
        assert!(!has_vertical_bar_penetration(&pts, &tasks));
    }

    #[test]
    fn progress_point_calculation_unchanged() {
        let legacy = progress_line(&[(10.0, 110.0, 0.0, 40.0, 0.25)], None);
        let anchored = progress_line(&[(10.0, 110.0, 0.0, 40.0, 0.25)], Some(5.0));
        let expected_x = 10.0 + (110.0 - 10.0) * 0.25;
        assert_eq!(legacy[0].0, expected_x);
        assert!(anchored.iter().any(|(x, _)| (*x - expected_x).abs() < f64::EPSILON));
    }

    #[test]
    fn legacy_multi_task_has_no_right_angle_elbows() {
        let tasks = [
            (0.0, 100.0, 0.0, 40.0, 0.2),
            (0.0, 100.0, 40.0, 80.0, 0.5),
            (0.0, 100.0, 80.0, 120.0, 0.9),
        ];
        let pts = progress_line(&tasks, None);
        assert_eq!(pts.len(), 3);
        for w in pts.windows(2) {
            assert!(!segment_is_axis_aligned(w[0], w[1]));
        }
        assert!(!has_vertical_bar_penetration(&pts, &tasks));
    }

    #[test]
    fn today_anchored_has_no_bar_penetration() {
        let tasks = [
            (0.0, 200.0, 10.0, 50.0, 0.4),
            (0.0, 200.0, 50.0, 90.0, 0.6),
            (0.0, 200.0, 90.0, 130.0, 0.8),
        ];
        let pts = progress_line(&tasks, Some(80.0));
        assert!(!has_vertical_bar_penetration(&pts, &tasks));
        assert!(interior_segments_are_diagonal(&pts));
    }

    #[test]
    fn today_anchored_future_not_started_clamps_to_today() {
        let today_x = 30.0;
        let tasks = [(50.0, 100.0, 0.0, 40.0, 0.0)];
        let pts = progress_line(&tasks, Some(today_x));
        assert_eq!(pts[1].0, today_x);
    }

    #[test]
    fn today_anchored_progress_positive_not_clamped_when_start_after_today() {
        let today_x = 30.0;
        let start = 50.0;
        let end = 100.0;
        let p = 0.05;
        let tasks = [(start, end, 0.0, 40.0, p)];
        let pts = progress_line(&tasks, Some(today_x));
        let expected_x = progress_x(start, end, p);
        assert_eq!(pts[1].0, expected_x);
        assert!(expected_x > today_x);
    }

    #[test]
    fn today_anchored_overdue_not_started_stays_at_start() {
        let today_x = 50.0;
        let start = 10.0;
        let tasks = [(start, 100.0, 0.0, 40.0, 0.0)];
        let pts = progress_line(&tasks, Some(today_x));
        assert_eq!(pts[1].0, start);
    }
}
