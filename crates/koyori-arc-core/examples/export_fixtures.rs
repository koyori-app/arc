//! Export benchmark fixtures as JSON for Wasm/JS and browser DOM benches.

use koyori_arc_core::bench_fixtures::{
    fixture_id, generate_fixture, generate_fixture_n, micro_fixture_id, DepDensity, TaskCount,
    MICRO_BENCH_COUNTS,
};
use std::fs;
use std::path::PathBuf;

fn main() {
    let out_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("benches/fixtures");
    fs::create_dir_all(&out_dir).expect("create fixtures dir");

    for count in TaskCount::ALL {
        for density in DepDensity::ALL {
            let fixture = generate_fixture(count, density);
            let name = fixture_id(count, density);
            let path = out_dir.join(format!("{name}.json"));
            let json = serde_json::to_string_pretty(&fixture).expect("serialize fixture");
            fs::write(&path, json).expect("write fixture");
            eprintln!(
                "wrote {} ({} tasks, {} deps)",
                path.display(),
                fixture.tasks.len(),
                fixture.deps.len()
            );
        }
    }

    for &count in &MICRO_BENCH_COUNTS {
        for density in DepDensity::ALL {
            let fixture = generate_fixture_n(count, density);
            let name = micro_fixture_id(count, density);
            let path = out_dir.join(format!("{name}.json"));
            let json = serde_json::to_string_pretty(&fixture).expect("serialize fixture");
            fs::write(&path, json).expect("write fixture");
            eprintln!(
                "wrote {} ({} tasks, {} deps)",
                path.display(),
                fixture.tasks.len(),
                fixture.deps.len()
            );
        }
    }
}
