/// Build the Progress line polyline points.
///
/// The Progress line (also called the "status line") is a zigzag that passes
/// through each task bar at the point corresponding to its `progress` value.
/// A task ahead of schedule causes the line to jag right; behind schedule jags left.
///
/// Returns a list of (x, y) pixel coordinates for an SVG `<polyline>`.
pub fn progress_line(
    tasks: &[(f64, f64, f64, f64, f64)], // (start_px, end_px, y_top, y_bottom, progress 0–1)
) -> Vec<(f64, f64)> {
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

#[cfg(test)]
mod tests {
    use super::*;

    // helper: single task spanning px 0–100, rows y 0–40
    fn one(progress: f64) -> Vec<(f64, f64)> {
        progress_line(&[(0.0, 100.0, 0.0, 40.0, progress)])
    }

    #[test]
    fn empty_input() {
        assert!(progress_line(&[]).is_empty());
    }

    #[test]
    fn single_task_not_started() {
        let pts = one(0.0);
        // x should be at start (0)
        assert_eq!(pts, vec![(0.0, 0.0), (0.0, 40.0)]);
    }

    #[test]
    fn single_task_complete() {
        let pts = one(1.0);
        assert_eq!(pts, vec![(100.0, 0.0), (100.0, 40.0)]);
    }

    #[test]
    fn single_task_halfway() {
        let pts = one(0.5);
        assert_eq!(pts, vec![(50.0, 0.0), (50.0, 40.0)]);
    }

    #[test]
    fn two_tasks_vertical_segment_connects_rows() {
        // task 0: 0–100px, y 0–40, 50% done → x=50
        // task 1: 0–100px, y 40–80, 75% done → x=75
        let pts = progress_line(&[
            (0.0, 100.0, 0.0, 40.0, 0.5),
            (0.0, 100.0, 40.0, 80.0, 0.75),
        ]);
        // task 0: (50, 0) → (50, 40)
        // task 1: vertical from prev x=50 to row top → (50, 40), then (75, 80)
        assert_eq!(pts[0], (50.0, 0.0));
        assert_eq!(pts[1], (50.0, 40.0));
        assert_eq!(pts[2], (50.0, 40.0)); // vertical start at prev x
        assert_eq!(pts[3], (75.0, 80.0));
    }
}
