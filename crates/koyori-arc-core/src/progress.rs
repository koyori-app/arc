/// Build the Progress line polyline points.
///
/// The Progress line (also called the "status line" or イナズマ線) is a zigzag that
/// passes through each task bar at the point corresponding to its `progress` value.
/// A task ahead of schedule causes the line to jag right; behind schedule jags left.
///
/// When `today_x` is `Some`, endpoints are anchored on the today vertical line:
/// top row's `y_top` → each task's progress point → bottom row's `y_bottom`.
///
/// When `today_x` is `None`, the legacy path connects all task progress points
/// without today anchoring (see SPEC.md).
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

fn progress_line_legacy(tasks: &[(f64, f64, f64, f64, f64)]) -> Vec<(f64, f64)> {
    let mut pts = Vec::with_capacity(tasks.len() * 2);
    for (i, &(start, end, y_top, y_bottom, p)) in tasks.iter().enumerate() {
        let x = start + (end - start) * p;
        if i == 0 {
            pts.push((x, y_top));
        } else {
            let prev_x = pts.last().map(|p| p.0).unwrap_or(x);
            pts.push((prev_x, y_top));
        }
        pts.push((x, y_bottom));
    }
    pts
}

fn progress_line_today_anchored(
    tasks: &[(f64, f64, f64, f64, f64)],
    today_x: f64,
) -> Vec<(f64, f64)> {
    let mut pts = Vec::with_capacity(tasks.len() * 2 + 2);
    let &(start0, end0, y_top0, y_bottom0, p0) = &tasks[0];
    let x0 = start0 + (end0 - start0) * p0;

    pts.push((today_x, y_top0));
    if (today_x - x0).abs() > f64::EPSILON {
        pts.push((x0, y_top0));
    }
    pts.push((x0, y_bottom0));

    for &(start, end, y_top, y_bottom, p) in tasks.iter().skip(1) {
        let x = start + (end - start) * p;
        let prev_x = pts.last().map(|p| p.0).unwrap_or(today_x);
        if (prev_x - x).abs() > f64::EPSILON || pts.last().map(|p| p.1) != Some(y_top) {
            if pts.last().map(|p| p.1) != Some(y_top) {
                pts.push((prev_x, y_top));
            }
            if (prev_x - x).abs() > f64::EPSILON {
                pts.push((x, y_top));
            }
        }
        pts.push((x, y_bottom));
    }

    let last_y = tasks.last().map(|t| t.3).unwrap_or(y_bottom0);
    let last_x = pts.last().map(|p| p.0).unwrap_or(today_x);
    if (last_x - today_x).abs() > f64::EPSILON {
        pts.push((today_x, last_y));
    } else if pts.last().map(|p| p.1) != Some(last_y) {
        pts.push((today_x, last_y));
    }

    pts
}

#[cfg(test)]
mod tests {
    use super::*;

    // helper: single task spanning px 0–100, rows y 0–40
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
        assert_eq!(pts, vec![(0.0, 0.0), (0.0, 40.0)]);
    }

    #[test]
    fn single_task_complete_legacy() {
        let pts = one(1.0);
        assert_eq!(pts, vec![(100.0, 0.0), (100.0, 40.0)]);
    }

    #[test]
    fn single_task_halfway_legacy() {
        let pts = one(0.5);
        assert_eq!(pts, vec![(50.0, 0.0), (50.0, 40.0)]);
    }

    #[test]
    fn two_tasks_vertical_segment_connects_rows_legacy() {
        let pts = progress_line(
            &[
                (0.0, 100.0, 0.0, 40.0, 0.5),
                (0.0, 100.0, 40.0, 80.0, 0.75),
            ],
            None,
        );
        assert_eq!(pts[0], (50.0, 0.0));
        assert_eq!(pts[1], (50.0, 40.0));
        assert_eq!(pts[2], (50.0, 40.0));
        assert_eq!(pts[3], (75.0, 80.0));
    }

    #[test]
    fn single_task_today_anchored_starts_and_ends_on_today() {
        let pts = one_today(0.5, 30.0);
        assert_eq!(pts.first(), Some(&(30.0, 0.0)));
        assert_eq!(pts.last(), Some(&(30.0, 40.0)));
        assert!(pts.contains(&(50.0, 0.0)));
        assert!(pts.contains(&(50.0, 40.0)));
    }

    #[test]
    fn single_task_today_anchored_at_progress_point_skips_redundant_top() {
        // progress at x=30 equals today_x — no extra horizontal at row top
        let pts = one_today(0.3, 30.0);
        assert_eq!(pts, vec![(30.0, 0.0), (30.0, 40.0)]);
    }

    #[test]
    fn two_tasks_today_anchored_zigzag_through_progress_points() {
        let pts = progress_line(
            &[
                (0.0, 100.0, 0.0, 40.0, 0.5),
                (0.0, 100.0, 40.0, 80.0, 0.75),
            ],
            Some(20.0),
        );
        assert_eq!(pts.first(), Some(&(20.0, 0.0)));
        assert_eq!(pts.last(), Some(&(20.0, 80.0)));
        assert!(pts.contains(&(50.0, 0.0)));
        assert!(pts.contains(&(50.0, 40.0)));
        assert!(pts.contains(&(75.0, 80.0)));
    }

    #[test]
    fn progress_point_calculation_unchanged() {
        let legacy = progress_line(&[(10.0, 110.0, 0.0, 40.0, 0.25)], None);
        let anchored = progress_line(&[(10.0, 110.0, 0.0, 40.0, 0.25)], Some(5.0));
        let expected_x = 10.0 + (110.0 - 10.0) * 0.25;
        assert_eq!(legacy[0].0, expected_x);
        assert!(anchored.iter().any(|(x, _)| (*x - expected_x).abs() < f64::EPSILON));
    }
}
