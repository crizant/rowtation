//! Large-input checks and a local benchmark, ignored by default so CI stays fast.
//!
//! cargo test --release --test stress -- --ignored --nocapture
//!
//! ROWTATION_BENCH_MB sets the benchmark input size in MiB (default 64).
use nsh_app_rowtation::{
    ImportSource,
    document::{CellValue, MAX_WINDOW_ROWS},
    formats::delimited::{parse, serialize},
    session::{DocumentSession, ImportOptions, RowWindow},
};
use std::time::Instant;

#[test]
#[ignore = "allocates hundreds of megabytes; run with --ignored"]
fn huge_values_and_shapes_round_trip() {
    // Past every former cap at once: 32 MiB of output, over 1,000,000 stored
    // fields, and 256-row windows across 20,000 columns.
    let long = "雪".repeat(11 * 1024 * 1024 + 1);
    let input = format!("value\n{long}\n");
    let document = parse(input.as_bytes(), ",", true).unwrap();
    assert_eq!(document.cell(1, 1).unwrap(), &CellValue::Text(long));
    let output = serialize(&document, ",", true).unwrap();
    assert!(output.len() > 32 * 1024 * 1024);
    assert_eq!(output, input.into_bytes());

    let fields = format!("{}\n", vec!["x"; 513].join(",")).repeat(1_000_000 / 513 + 1);
    let document = parse(fields.as_bytes(), ",", false).unwrap();
    assert!(document.row_count() * document.column_count() > 1_000_000);

    let wide = format!("{}\n", vec!["x"; 20_000].join(",")).repeat(MAX_WINDOW_ROWS);
    let document = parse(wide.as_bytes(), ",", false).unwrap();
    assert_eq!(
        document.window(0, MAX_WINDOW_ROWS).unwrap().len(),
        MAX_WINDOW_ROWS
    );
}

fn time<T>(label: &str, run: impl FnOnce() -> T) -> T {
    let start = Instant::now();
    let result = run();
    println!(
        "{label:<44} {:>10.2} ms",
        start.elapsed().as_secs_f64() * 1000.0
    );
    result
}

fn report_search(
    label: &str,
    run: impl FnOnce() -> Result<
        nsh_app_rowtation::operations::SearchResult,
        nsh_app_rowtation::error::EngineError,
    >,
) {
    let start = Instant::now();
    match run() {
        Ok(result) => println!(
            "{label:<44} {:>10.2} ms ({} matches)",
            start.elapsed().as_secs_f64() * 1000.0,
            result.count
        ),
        Err(error) => println!(
            "{label:<44} {:>10.2} ms ({}: {})",
            start.elapsed().as_secs_f64() * 1000.0,
            error.code(),
            error.message()
        ),
    }
}

#[test]
#[ignore = "benchmark; run in release with --ignored --nocapture"]
fn benchmark_engine() {
    let mib: usize = std::env::var("ROWTATION_BENCH_MB")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(64);
    let rows_limit: Option<usize> = std::env::var("ROWTATION_BENCH_ROWS")
        .ok()
        .and_then(|value| value.parse().ok());
    let mut text = String::from("order_id,customer,country,status,amount,note\n");
    let (countries, statuses) = (["AU", "NZ", "JP", "US", "GB"], ["paid", "open", "void"]);
    let mut row = 0usize;
    while match rows_limit {
        Some(limit) => row < limit,
        None => text.len() < mib * 1024 * 1024,
    } {
        text.push_str(&format!(
            "{:08},customer-{},{},{},{}.{:02},\"note {}, with comma\"\n",
            row,
            row % 50_000,
            countries[row % 5],
            statuses[row % 3],
            row % 1000,
            row % 100,
            row % 7
        ));
        row += 1;
    }
    println!(
        "input: {} MiB, {row} rows, 6 columns",
        text.len() / (1024 * 1024)
    );

    let source = ImportSource::from_text(text, "orders.csv".into());
    time("inspect (both separators, whole input)", || {
        source.inspect().unwrap()
    });
    let mut session = time("parse into a document", || {
        DocumentSession::from_source(&source, ImportOptions::default()).unwrap()
    });
    drop(source);

    let middle = row as u32 / 2;
    let request = |preview: Option<u32>, columns: Option<(u32, u32)>| {
        let mut request = RowWindow::default();
        request.offset = middle;
        request.limit = 40;
        request.preview_bytes = preview;
        request.column_offset = columns.map(|c| c.0);
        request.column_limit = columns.map(|c| c.1);
        request
    };
    for (label, window) in [
        (
            "rows: 40 rows, all columns, whole values",
            request(None, None),
        ),
        (
            "rows: 40 rows, all columns, 512-byte previews",
            request(Some(512), None),
        ),
        (
            "rows: 40 rows, 2 columns, 512-byte previews",
            request(Some(512), Some((1, 2))),
        ),
    ] {
        let runs = 1000;
        let start = Instant::now();
        for _ in 0..runs {
            std::hint::black_box(session.rows(request_clone(&window)).unwrap());
        }
        println!(
            "{label:<44} {:>10.2} µs",
            start.elapsed().as_secs_f64() * 1e6 / runs as f64
        );
    }

    let columns = session.columns();
    let country = columns.iter().find(|c| c.name == "country").unwrap().id;
    let status = columns.iter().find(|c| c.name == "status").unwrap().id;
    let order_id = columns.iter().find(|c| c.name == "order_id").unwrap().id;
    time("sort country ascending", || {
        session.sort(Some(country), false).unwrap()
    });
    time("sort country descending", || {
        session.sort(Some(country), true).unwrap()
    });
    time("reset imported order", || {
        session.sort(None, false).unwrap()
    });
    // Search and sort run on the calling thread. These timings are not an import cap.
    let scoped = time("search order_id for 00000042", || {
        session
            .search(nsh_app_rowtation::operations::SearchOptions {
                term: "00000042".into(),
                column_id: Some(order_id),
                case_sensitive: Some(false),
                regex: Some(false),
            })
            .unwrap()
    });
    println!("scoped matches: {}", scoped.count);
    report_search("search status for paid", || {
        session.search(nsh_app_rowtation::operations::SearchOptions {
            term: "paid".into(),
            column_id: Some(status),
            case_sensitive: Some(false),
            regex: Some(false),
        })
    });
    report_search("search all columns for an absent token", || {
        session.search(nsh_app_rowtation::operations::SearchOptions {
            term: "rowtation-absent".into(),
            column_id: None,
            case_sensitive: Some(false),
            regex: Some(false),
        })
    });
    let first = columns[0].id;
    let (mut slices, mut longest, start) = (0, 0.0f64, Instant::now());
    loop {
        let slice = Instant::now();
        let progress = session.profile_step(first, 100_000).unwrap();
        longest = longest.max(slice.elapsed().as_secs_f64() * 1000.0);
        slices += 1;
        if progress.profile.is_some() {
            break;
        }
    }
    println!(
        "{:<44} {:>10.2} ms ({slices} slices of 100,000 rows, longest {longest:.2} ms)",
        format!("profile {} (sliced)", columns[0].name),
        start.elapsed().as_secs_f64() * 1000.0
    );
    for column in &columns[1..] {
        time(&format!("profile {} (whole column)", column.name), || {
            session.profile(column.id).unwrap()
        });
    }
}

fn request_clone(window: &RowWindow) -> RowWindow {
    let mut request = RowWindow::default();
    request.offset = window.offset;
    request.limit = window.limit;
    request.preview_bytes = window.preview_bytes;
    request.column_offset = window.column_offset;
    request.column_limit = window.column_limit;
    request
}
