//! Focused native search benchmarks, with no elapsed-time assertions.
//! cargo test --release --test search_performance -- --ignored --nocapture --test-threads=1
use nsh_app_rowtation::{
    operations::SearchOptions,
    session::{DocumentSession, ImportOptions},
};
use std::{fmt::Write, time::Instant};

fn measure(s: &mut DocumentSession, label: &str, term: &str, column: Option<u32>, expected: u32) {
    let start = Instant::now();
    let result = s.search(SearchOptions {
        term: term.into(),
        column_id: column,
        ..Default::default()
    });
    let elapsed = start.elapsed().as_secs_f64() * 1000.0;
    match result {
        Ok(result) => {
            println!("{label}: {elapsed:.3} ms ({} matches)", result.count);
            assert_eq!(result.count, expected);
        }
        Err(error) if std::env::var_os("ROWTATION_SEARCH_BASELINE").is_some() => {
            // The previous PR revision falsely exhausts its payload budget on
            // this tiny sparse dataset. Record that failure for comparison.
            println!("{label}: {elapsed:.3} ms ({})", error.code());
            assert_eq!(error.code(), "SEARCH_LIMIT");
        }
        Err(error) => panic!("{label}: {error}"),
    }
}
#[test]
#[ignore = "10k heterogeneous NDJSON search benchmark; run in release"]
fn heterogeneous_ndjson_search() {
    let mut input = String::new();
    for i in 0..10_000 {
        writeln!(input, "{{\"field_{i}\":\"needle\"}}").unwrap();
    }
    let mut options = ImportOptions::default();
    options.format = Some("ndjson".into());
    let mut s = DocumentSession::new(input.into_bytes(), options).unwrap();
    assert_eq!(
        (s.metadata().row_count, s.metadata().column_count),
        (10_000, 10_000)
    );
    for _ in 0..5 {
        measure(&mut s, "sparse all absent", "absent", None, 0);
        measure(&mut s, "sparse all matches", "needle", None, 10_000);
        measure(&mut s, "sparse one column", "needle", Some(5001), 1);
    }
}
#[test]
#[ignore = "CSV/TSV search control benchmark; run in release"]
fn delimited_search_control() {
    let rows = 100_000;
    for delimiter in [",", "\t"] {
        let mut input = format!("id{delimiter}value\n");
        for i in 0..rows {
            writeln!(input, "{i:08}{delimiter}value-{:08}", (i * 7919) % rows).unwrap();
        }
        let mut options = ImportOptions::default();
        options.delimiter = Some(delimiter.into());
        let mut s = DocumentSession::new(input.into_bytes(), options).unwrap();
        for _ in 0..5 {
            measure(
                &mut s,
                &format!("{delimiter:?} all absent"),
                "absent",
                None,
                0,
            );
            measure(
                &mut s,
                &format!("{delimiter:?} all matches"),
                "value",
                None,
                rows,
            );
            measure(
                &mut s,
                &format!("{delimiter:?} one column"),
                "value",
                Some(2),
                rows,
            );
        }
    }
}

#[test]
#[ignore = "40 MiB sliced search benchmark; run in release"]
fn large_time_sliced_search() {
    let input = format!("value\n{}", format!("{}\n", "x".repeat(80)).repeat(500_000));
    let mut s = DocumentSession::new(input.into_bytes(), ImportOptions::default()).unwrap();
    for _ in 0..5 {
        measure(&mut s, "40 MB all absent synchronous", "absent", None, 0);
        let began = Instant::now();
        s.start_search(SearchOptions {
            term: "absent".into(),
            ..Default::default()
        })
        .unwrap();
        let mut slices = 0;
        let mut longest = 0.0_f64;
        let mut scanning = 0.0;
        loop {
            let start = Instant::now();
            let progress = s.search_step().unwrap();
            let elapsed = start.elapsed().as_secs_f64() * 1000.0;
            longest = longest.max(elapsed);
            scanning += elapsed;
            slices += 1;
            if let Some(result) = progress.result {
                assert_eq!(result.count, 0);
                break;
            }
        }
        println!(
            "40 MB all absent sliced: {:.3} ms total, {scanning:.3} ms scanning, {slices} slices, longest {longest:.3} ms",
            began.elapsed().as_secs_f64() * 1000.0
        );
    }
}
