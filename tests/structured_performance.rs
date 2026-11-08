//! Opt-in native benchmarks. Run each test separately under /usr/bin/time -l to
//! measure peak RSS as well as timings, using ROWTATION_BENCH_ROWS for scaling.
//! cargo test --release --test structured_performance -- --ignored --nocapture
use nsh_app_rowtation::{
    ImportSource,
    formats::structured,
    session::{DocumentSession, ImportOptions},
};
use std::{fmt::Write, hint::black_box, time::Instant};

fn rows() -> usize {
    std::env::var("ROWTATION_BENCH_ROWS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(100_000)
}
fn timed<T>(label: &str, run: impl FnOnce() -> T) -> T {
    let start = Instant::now();
    let result = run();
    println!("{label}: {:.3} ms", start.elapsed().as_secs_f64() * 1000.0);
    result
}
#[test]
#[ignore = "scaling benchmark; run in release with --ignored --nocapture"]
fn heterogeneous_ndjson() {
    let n = rows();
    let mut input = String::new();
    for i in 0..n {
        writeln!(input, "{{\"field_{i}\":{i}}}").unwrap();
    }
    println!("{n} rows, {n} unique columns, {} input bytes", input.len());
    let source = ImportSource::from_text(input.clone(), "heterogeneous.ndjson".into());
    timed("review detection", || black_box(source.inspect().unwrap()));
    timed("structured review", || {
        black_box(source.inspect_format("ndjson".into()).unwrap())
    });
    drop(source);
    let doc = timed("parse", || {
        structured::parse(input.as_bytes(), "ndjson").unwrap()
    });
    let entries: usize = (0..n)
        .step_by(256)
        .map(|offset| {
            doc.window(offset, (n - offset).min(256))
                .unwrap()
                .iter()
                .map(|r| r.cells().iter().count())
                .sum::<usize>()
        })
        .sum();
    println!("stored cell entries: {entries}");
    assert_eq!(doc.column_count(), n);
    if std::env::var_os("ROWTATION_EXPECT_DENSE").is_none() {
        assert_eq!(entries, n);
    }
    assert_eq!(
        doc.cell(n as u32, n as u32).unwrap(),
        &nsh_app_rowtation::document::CellValue::Number((n - 1).to_string())
    );
}
#[test]
#[ignore = "numeric sorting benchmark; run in release with --ignored --nocapture"]
fn numeric_sort() {
    let n = rows();
    let mut input = String::new();
    for i in 0..n {
        // Deterministic permutation avoids measuring an already sorted fast path.
        let value = (i * 7919) % n;
        writeln!(
            input,
            "{{\"number\":1{value:08}12345678901234567890.000000000000000000001}}"
        )
        .unwrap();
    }
    println!("{n} exact decimal rows, {} input bytes", input.len());
    let mut options = ImportOptions::default();
    options.format = Some("ndjson".into());
    let mut session = timed("numeric import", || {
        DocumentSession::new(input.into_bytes(), options).unwrap()
    });
    timed("numeric ascending", || {
        session.sort(Some(1), false).unwrap()
    });
    timed("numeric descending", || {
        session.sort(Some(1), true).unwrap()
    });
    assert!(!session.metadata().dirty);
    assert_eq!(session.row_position(1).unwrap(), (n - 1) as u32);
}
#[test]
#[ignore = "CSV/TSV control benchmark; run in release with --ignored --nocapture"]
fn delimited_control() {
    let n = rows();
    for delimiter in [",", "\t"] {
        let mut input = format!("id{delimiter}value\n");
        for i in 0..n {
            writeln!(input, "{i:08}{delimiter}{:08}", (i * 7919) % n).unwrap();
        }
        let mut options = ImportOptions::default();
        options.delimiter = Some(delimiter.into());
        let mut session = timed(&format!("{delimiter:?} import"), || {
            DocumentSession::new(input.into_bytes(), options).unwrap()
        });
        timed(&format!("{delimiter:?} text sort"), || {
            session.sort(Some(2), false).unwrap()
        });
        assert_eq!(session.row_position(1).unwrap(), 0);
        assert!(!session.metadata().dirty);
    }
}
