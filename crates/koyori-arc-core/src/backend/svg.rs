use crate::display_list::types::*;
use crate::display_list::DisplayList;

use super::traits::{BackendOutput, RenderBackend};

pub struct SvgBackend;

impl RenderBackend for SvgBackend {
    fn render(&self, list: &DisplayList) -> BackendOutput {
        BackendOutput::Svg(render_svg_from_list(list))
    }

    fn name(&self) -> &'static str {
        "svg"
    }
}

pub fn render_svg_from_list(list: &DisplayList) -> String {
    let chart_w = list.viewport.width;
    let chart_h = list.viewport.height;

    let mut svg = format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="{chart_w}" height="{chart_h}" viewBox="0 0 {chart_w} {chart_h}" role="img" aria-label="Gantt chart" font-family="sans-serif" font-size="12"><title>Gantt chart</title><desc>Task schedule with progress bars and dependency arrows</desc>"#
    );

    for layer in &list.layers {
        for prim in &layer.primitives {
            render_primitive(&mut svg, prim, &list.palette, chart_h);
        }
    }

    svg.push_str("</svg>");
    svg
}

fn render_primitive(svg: &mut String, prim: &Primitive, palette: &Palette, chart_h: f64) {
    match prim {
        Primitive::Rect(r) => match r.semantic {
            RectSemantic::HeaderBackground => {
                let fill = palette.resolve(r.fill);
                svg.push_str(&format!(
                    r#"<rect x="{x}" y="0" width="{w}" height="{h}" fill="{fill}"/>"#,
                    x = r.x,
                    w = r.width,
                    h = r.height,
                ));
            }
            RectSemantic::BarBackground => {
                let fill = palette.resolve(r.fill);
                svg.push_str(&format!(
                    r#"<rect class="bar-bg" x="{x}" y="{y}" width="{w}" height="{BAR_H}" rx="4" fill="{fill}"/>"#,
                    x = r.x,
                    y = r.y,
                    w = r.width,
                    BAR_H = r.height,
                ));
            }
            RectSemantic::LegendSwatch => {
                let fill = palette.resolve(r.fill);
                svg.push_str(&format!(
                    r#"<rect x="{x}" y="{y}" width="{sw}" height="{sw}" rx="2" fill="{fill}"/>"#,
                    x = r.x,
                    y = r.y,
                    sw = r.width,
                ));
            }
        },
        Primitive::RoundRect(r) => {
            let fill = palette.resolve(r.fill);
            svg.push_str(&format!(
                r#"<rect class="bar-progress bar-tier-{tier}" x="{x}" y="{y}" width="{w}" height="{BAR_H}" rx="4" fill="{fill}"/>"#,
                tier = r.tier.css_suffix(),
                x = r.x,
                y = r.y,
                w = r.width,
                BAR_H = r.height,
            ));
        }
        Primitive::Line(l) => {
            let stroke = palette.resolve(l.stroke);
            match l.semantic {
                LineSemantic::Grid => {
                    svg.push_str(&format!(
                        r#"<line x1="{x1}" y1="{HEADER_H}" x2="{x2}" y2="{chart_h}" stroke="{stroke}" stroke-width="1"/>"#,
                        x1 = l.x1,
                        x2 = l.x2,
                        HEADER_H = l.y1,
                        chart_h = chart_h,
                    ));
                }
                LineSemantic::TodayMarker => {
                    svg.push_str(&format!(
                        r#"<line x1="{x1}" y1="0" x2="{x2}" y2="{chart_h}" stroke="{stroke}" stroke-width="2" stroke-dasharray="4,3"/>"#,
                        x1 = l.x1,
                        x2 = l.x2,
                        chart_h = chart_h,
                    ));
                }
                LineSemantic::LegendProgressLine => {
                    svg.push_str(&format!(
                        r#"<line x1="{x1}" y1="{y1}" x2="{x2}" y2="{y2}" stroke="{stroke}" stroke-width="2" stroke-dasharray="6,3"/>"#,
                        x1 = l.x1,
                        y1 = l.y1,
                        x2 = l.x2,
                        y2 = l.y2,
                    ));
                }
            }
        }
        Primitive::Path(p) => {
            let stroke = palette.resolve(p.stroke);
            svg.push_str(&format!(
                r#"<path d="{d}" fill="none" stroke="{stroke}" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"/>"#,
                d = p.d,
            ));
        }
        Primitive::Polyline(p) => {
            let stroke = palette.resolve(p.stroke);
            let pts_str: String = p
                .points
                .iter()
                .map(|(x, y)| format!("{x},{y}"))
                .collect::<Vec<_>>()
                .join(" ");
            svg.push_str(&format!(
                r#"<polyline class="progress-status-line" points="{pts_str}" fill="none" stroke="{stroke}" stroke-width="2" stroke-dasharray="6,3"/>"#,
            ));
        }
        Primitive::Polygon(p) => {
            let fill = palette.resolve(p.fill);
            let stroke = palette.resolve(p.stroke);
            let points: String = p
                .points
                .iter()
                .map(|(x, y)| format!("{x},{y}"))
                .collect::<Vec<_>>()
                .join(" ");
            svg.push_str(&format!(
                r#"<polygon class="bar-milestone bar-tier-{tier}" points="{points}" fill="{fill}" stroke="{stroke}" stroke-width="1"/>"#,
                tier = p.tier.css_suffix(),
            ));
        }
        Primitive::Text(t) => render_text(svg, t, palette),
        Primitive::Group(g) => {
            let is_progress_legend = g.children.iter().any(|c| {
                matches!(
                    c,
                    Primitive::Line(l) if matches!(l.semantic, LineSemantic::LegendProgressLine)
                )
            });
            let is_tier_legend = g.children.iter().any(|c| {
                matches!(
                    c,
                    Primitive::Rect(r) if matches!(r.semantic, RectSemantic::LegendSwatch)
                )
            });

            if let Some(id) = &g.task_id {
                svg.push_str(&format!(
                    r#"<g data-task-id="{id}">"#,
                    id = escape_xml(id),
                ));
            } else if is_progress_legend {
                svg.push_str(r#"<g class="progress-line-legend" aria-hidden="true">"#);
            } else if is_tier_legend {
                svg.push_str(r#"<g class="bar-tier-legend" aria-hidden="true">"#);
            } else {
                svg.push_str("<g>");
            }
            if let Some(tooltip) = &g.tooltip {
                svg.push_str(&format!(
                    r#"<title>{tooltip}</title>"#,
                    tooltip = escape_xml(tooltip),
                ));
            }
            for child in &g.children {
                render_primitive(svg, child, palette, chart_h);
            }
            svg.push_str("</g>");
        }
    }
}

fn render_text(svg: &mut String, t: &TextPrim, palette: &Palette) {
    let content = escape_xml(&t.content);
    match t.semantic {
        TextSemantic::GridLabel => {
            let fill = palette.resolve(t.fill.unwrap());
            svg.push_str(&format!(
                r#"<text x="{x}" y="{y}" fill="{fill}" font-size="11">{content}</text>"#,
                x = t.x,
                y = t.y,
            ));
        }
        TextSemantic::ProgressPercent => {
            let fill = palette.resolve(t.fill.unwrap());
            if let Some(anchor) = t.anchor {
                let anchor_str = match anchor {
                    TextAnchor::Middle => "middle",
                    TextAnchor::Start => "start",
                    TextAnchor::End => "end",
                };
                svg.push_str(&format!(
                    r#"<text x="{x}" y="{y}" text-anchor="{anchor_str}" fill="{fill}" font-size="11" font-weight="600" dominant-baseline="middle">{content}</text>"#,
                    x = t.x,
                    y = t.y,
                ));
            } else {
                svg.push_str(&format!(
                    r#"<text x="{x}" y="{y}" fill="{fill}" font-size="11" font-weight="600" dominant-baseline="middle">{content}</text>"#,
                    x = t.x,
                    y = t.y,
                ));
            }
        }
        TextSemantic::RowLabel => {
            svg.push_str(&format!(
                r#"<text x="{x}" y="{y}" text-anchor="end" dominant-baseline="middle">{content}</text>"#,
                x = t.x,
                y = t.y,
            ));
        }
        TextSemantic::LegendProgress => {
            let fill = palette.resolve(t.fill.unwrap());
            svg.push_str(&format!(
                r#"<text x="{x}" y="{y}" fill="{fill}" font-size="10" dominant-baseline="middle">{content}</text>"#,
                x = t.x,
                y = t.y,
            ));
        }
        TextSemantic::LegendTier => {
            let fill = palette.resolve(t.fill.unwrap());
            svg.push_str(&format!(
                r#"<text x="{x}" y="{y}" fill="{fill}" font-size="9" dominant-baseline="middle">{content}</text>"#,
                x = t.x,
                y = t.y,
            ));
        }
    }
}

fn escape_xml(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

pub fn empty_svg() -> String {
    r#"<svg xmlns="http://www.w3.org/2000/svg" width="0" height="0" viewBox="0 0 0 0" role="img" aria-label="Empty Gantt chart" font-family="sans-serif" font-size="12"><title>Empty Gantt chart</title><desc>No tasks to display</desc></svg>"#.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escape_xml_quotes_in_attribute_context() {
        let malicious = "x\" onmouseover=\"alert(1)\"";
        let escaped = escape_xml(malicious);
        assert!(!escaped.contains('"'));
        assert!(escaped.contains("&quot;"));
        assert_eq!(
            escaped,
            "x&quot; onmouseover=&quot;alert(1)&quot;"
        );
    }

    #[test]
    fn data_task_id_attribute_is_safe() {
        let id = "x\" onmouseover=\"alert(1)\"";
        let fragment = format!(r#"<g data-task-id="{}">"#, escape_xml(id));
        assert!(fragment.contains("data-task-id=\"x&quot; onmouseover=&quot;alert(1)&quot;\""));
        assert!(!fragment.contains(r#"onmouseover="alert"#));
    }
}
