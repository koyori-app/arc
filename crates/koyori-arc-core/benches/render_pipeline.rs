//! Layer 1 benchmark: native `render()` geometry + SVG string generation.

use chrono::NaiveDate;
use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use koyori_arc_core::bench_fixtures::{generate_fixture, DepDensity, TaskCount};
use koyori_arc_core::render;

fn parse_today(iso: &str) -> NaiveDate {
    NaiveDate::parse_from_str(iso, "%Y-%m-%d").unwrap()
}

fn bench_rust_render(c: &mut Criterion) {
    let mut group = c.benchmark_group("layer1_rust_render");
    group.sample_size(20);

    for count in TaskCount::ALL {
        for density in DepDensity::ALL {
            let fixture = generate_fixture(count, density);
            let today = parse_today(&fixture.today);
            let id = format!("{}/{}", count.label(), density);
            group.throughput(Throughput::Elements(count.get() as u64));
            group.bench_with_input(BenchmarkId::new("render", &id), &fixture, |b, fx| {
                b.iter(|| {
                    black_box(render(
                        black_box(&fx.tasks),
                        black_box(&fx.deps),
                        Some(today),
                    ))
                });
            });
        }
    }

    group.finish();
}

/// Layer 1b: `render_svg` on native target — isolates JSON parse + render (wasm boundary proxy).
fn bench_render_svg_native(c: &mut Criterion) {
    let mut group = c.benchmark_group("layer2_render_svg_native");
    group.sample_size(20);

    for count in TaskCount::ALL {
        for density in DepDensity::ALL {
            let fixture = generate_fixture(count, density);
            let tasks_json = serde_json::to_string(&fixture.tasks).unwrap();
            let deps_json = serde_json::to_string(&fixture.deps).unwrap();
            let today = fixture.today.clone();
            let id = format!("{}/{}", count.label(), density);
            group.throughput(Throughput::Elements(count.get() as u64));
            group.bench_with_input(
                BenchmarkId::new("render_svg", &id),
                &(tasks_json, deps_json, today),
                |b, (tj, dj, td)| {
                    b.iter(|| {
                        black_box(koyori_arc_core::render_svg(
                            black_box(tj.as_str()),
                            black_box(dj.as_str()),
                            Some(black_box(td.clone())),
                        ))
                    });
                },
            );
        }
    }

    group.finish();
}

criterion_group!(benches, bench_rust_render, bench_render_svg_native);
criterion_main!(benches);
