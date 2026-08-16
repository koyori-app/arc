use crate::display_list::constants::DEFAULT_FONT_SIZE_PX;
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
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="{chart_w}" height="{chart_h}" viewBox="0 0 {chart_w} {chart_h}" role="img" aria-label="Gantt chart" font-family="sans-serif" font-size="{DEFAULT_FONT_SIZE_PX}"><title>Gantt chart</title><desc>Task schedule with progress bars and dependency arrows</desc>"#
    );

    for layer in &list.layers {
        for prim in &layer.primitives {
            render_primitive(&mut svg, prim, &list.palette);
        }
    }

    svg.push_str("</svg>");
    svg
}

/// Every geometry attribute below is read off the primitive. The display list
/// is the only place that decides a coordinate, a radius, a stroke width or a
/// font size; restating one here would give the value a second home that no
/// golden fixture can see, because this file is what writes the golden.
fn render_primitive(svg: &mut String, prim: &Primitive, palette: &Palette) {
    match prim {
        Primitive::Rect(r) => {
            let class = match r.semantic {
                RectSemantic::BarBackground => Some("bar-bg"),
                RectSemantic::HeaderBackground | RectSemantic::LegendSwatch => None,
            };
            push_rect(
                svg,
                class,
                (r.x, r.y, r.width, r.height),
                r.rx,
                palette.resolve(r.fill),
            );
        }
        Primitive::RoundRect(r) => {
            let class = format!("bar-progress bar-tier-{}", r.tier.css_suffix());
            push_rect(
                svg,
                Some(&class),
                (r.x, r.y, r.width, r.height),
                Some(r.rx),
                palette.resolve(r.fill),
            );
        }
        Primitive::Line(l) => {
            svg.push_str(&format!(
                r#"<line x1="{x1}" y1="{y1}" x2="{x2}" y2="{y2}" stroke="{stroke}" stroke-width="{width}""#,
                x1 = l.x1,
                y1 = l.y1,
                x2 = l.x2,
                y2 = l.y2,
                stroke = palette.resolve(l.stroke),
                width = l.stroke_width,
            ));
            push_dash(svg, l.stroke_dash.as_deref());
            svg.push_str("/>");
        }
        Primitive::Path(p) => {
            svg.push_str(&format!(
                r#"<path d="{d}" fill="none" stroke="{stroke}" stroke-width="{width}" stroke-linecap="round" stroke-linejoin="round"/>"#,
                d = p.d,
                stroke = palette.resolve(p.stroke),
                width = p.stroke_width,
            ));
        }
        Primitive::Polyline(p) => {
            svg.push_str(&format!(
                r#"<polyline class="progress-status-line" points="{points}" fill="none" stroke="{stroke}" stroke-width="{width}""#,
                points = join_points(&p.points),
                stroke = palette.resolve(p.stroke),
                width = p.stroke_width,
            ));
            push_dash(svg, p.stroke_dash.as_deref());
            svg.push_str("/>");
        }
        Primitive::Polygon(p) => {
            svg.push_str(&format!(
                r#"<polygon class="bar-milestone bar-tier-{tier}" points="{points}" fill="{fill}" stroke="{stroke}" stroke-width="{width}"/>"#,
                tier = p.tier.css_suffix(),
                points = join_points(&p.points),
                fill = palette.resolve(p.fill),
                stroke = palette.resolve(p.stroke),
                width = p.stroke_width,
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
                render_primitive(svg, child, palette);
            }
            svg.push_str("</g>");
        }
    }
}

/// `geom` is `(x, y, width, height)`, taken from the primitive as-is.
fn push_rect(
    svg: &mut String,
    class: Option<&str>,
    geom: (f64, f64, f64, f64),
    rx: Option<f64>,
    fill: &str,
) {
    let (x, y, width, height) = geom;
    svg.push_str("<rect");
    if let Some(class) = class {
        svg.push_str(&format!(r#" class="{class}""#));
    }
    svg.push_str(&format!(
        r#" x="{x}" y="{y}" width="{width}" height="{height}""#
    ));
    if let Some(rx) = rx {
        svg.push_str(&format!(r#" rx="{rx}""#));
    }
    svg.push_str(&format!(r#" fill="{fill}"/>"#));
}

fn push_dash(svg: &mut String, dash: Option<&str>) {
    if let Some(dash) = dash {
        svg.push_str(&format!(r#" stroke-dasharray="{dash}""#));
    }
}

fn join_points(points: &[(f64, f64)]) -> String {
    points
        .iter()
        .map(|(x, y)| format!("{x},{y}"))
        .collect::<Vec<_>>()
        .join(" ")
}

/// One writer for every `TextSemantic`. Which attributes a label gets follows
/// from the fields the display list filled in — a `None` font size means
/// "inherit the root", not "look up 11 in a table over here".
fn render_text(svg: &mut String, t: &TextPrim, palette: &Palette) {
    svg.push_str(&format!(r#"<text x="{x}" y="{y}""#, x = t.x, y = t.y));
    if let Some(anchor) = t.anchor {
        let anchor = match anchor {
            TextAnchor::Middle => "middle",
            TextAnchor::Start => "start",
            TextAnchor::End => "end",
        };
        svg.push_str(&format!(r#" text-anchor="{anchor}""#));
    }
    if let Some(fill) = t.fill {
        svg.push_str(&format!(r#" fill="{}""#, palette.resolve(fill)));
    }
    if let Some(size) = t.font_size {
        svg.push_str(&format!(r#" font-size="{size}""#));
    }
    if let Some(weight) = t.font_weight {
        svg.push_str(&format!(r#" font-weight="{weight}""#));
    }
    if matches!(t.baseline, TextBaseline::Middle) {
        svg.push_str(r#" dominant-baseline="middle""#);
    }
    svg.push_str(&format!(
        ">{content}</text>",
        content = escape_xml(&t.content),
    ));
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
