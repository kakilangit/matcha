//! Benchmarks for what Matcha runs on every keystroke: compiling, matching,
//! replacing, explaining, and the whole workbench analysis.
//!
//! Run with `make bench`; `make bench BENCH=find_all/email` runs a subset.
#![expect(
    clippy::expect_used,
    reason = "a benchmark with broken setup should stop loudly, like a test"
)]

use std::fmt::Write;
use std::hint::black_box;

use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use matcha::engine::{EngineKind, Flags, MATCH_LIMIT, Regex};
use matcha::explain;
use matcha::library::Library;
use matcha::workbench::{self, Texts, Workbench};

/// Sample size for the matching benchmarks: large enough to measure, small
/// enough that the common patterns stay under [`MATCH_LIMIT`].
const SAMPLE_BYTES: usize = 256 * 1024;

/// Patterns that both engines support, named for the report.
const PATTERNS: [(&str, &str); 4] = [
    ("literal", "error"),
    ("email", r"(\w+)@(\w+)\.com"),
    ("date", r"(?<y>\d{4})-(?<m>\d{2})-(?<d>\d{2})"),
    ("alternation", "GET|POST|PUT|DELETE"),
];

/// Patterns that only `fancy-regex` supports.
const FANCY_PATTERNS: [(&str, &str); 2] = [
    ("lookahead", r"\w+(?=@)"),
    ("backreference", r"\b(\w+) \1\b"),
];

/// A pattern long enough to give the explainer some work.
const EXPLAIN_PATTERN: &str =
    r"^(?<user>[\w.+-]+)@(?<host>[\w-]+(?:\.[\w-]+)*)\.(?:com|org|net)\s*(?:#.*)?$";

fn compile(pattern: &str, engine: EngineKind) -> Regex {
    Regex::new(pattern, Flags::default(), engine).expect("benchmark pattern compiles")
}

/// Log-like text of about `bytes` bytes, the same on every run.
fn sample(bytes: usize) -> String {
    const WORDS: [&str; 8] = [
        "user", "request", "error", "the", "cache", "timeout", "ok", "retry",
    ];
    const METHODS: [&str; 4] = ["GET", "POST", "PUT", "DELETE"];
    let mut text = String::with_capacity(bytes + 128);
    let mut seed: u32 = 0x2545_f491;
    let mut next = move || {
        // xorshift32: cheap and deterministic.
        seed ^= seed << 13;
        seed ^= seed >> 17;
        seed ^= seed << 5;
        seed as usize
    };
    let mut line = 0;
    while text.len() < bytes {
        line += 1;
        let word = WORDS[next() % WORDS.len()];
        writeln!(
            text,
            "2026-{:02}-{:02} {} /api/{word}/{line} {word} {word} ana{}@example.com took {}ms",
            next() % 12 + 1,
            next() % 28 + 1,
            METHODS[next() % METHODS.len()],
            next() % 100,
            next() % 1000,
        )
        .expect("writing to a String can't fail");
    }
    text
}

fn bench_compile(c: &mut Criterion) {
    let mut group = c.benchmark_group("compile");
    for engine in EngineKind::ALL {
        group.bench_function(BenchmarkId::new("date", engine.label()), |b| {
            b.iter(|| Regex::new(black_box(PATTERNS[2].1), Flags::default(), engine));
        });
    }
    group.finish();
}

fn bench_find_all(c: &mut Criterion) {
    let text = sample(SAMPLE_BYTES);
    let mut group = c.benchmark_group("find_all");
    group.throughput(Throughput::Bytes(text.len() as u64));
    let cases = PATTERNS
        .iter()
        .flat_map(|&case| EngineKind::ALL.map(|engine| (case, engine)))
        .chain(FANCY_PATTERNS.iter().map(|&case| (case, EngineKind::Fancy)));
    for ((name, pattern), engine) in cases {
        let re = compile(pattern, engine);
        let found = re.find_all(&text).expect("benchmark search succeeds");
        if found.truncated {
            eprintln!(
                "note: find_all/{name} hits MATCH_LIMIT; it measures the first {MATCH_LIMIT} matches"
            );
        }
        group.bench_function(BenchmarkId::new(name, engine.label()), |b| {
            b.iter(|| re.find_all(black_box(&text)));
        });
    }
    // Every word character matches, so this stops at MATCH_LIMIT: the worst case
    // the UI sees while a pattern is half typed.
    for engine in EngineKind::ALL {
        let re = compile(r"\w", engine);
        group.bench_function(BenchmarkId::new("match_limit", engine.label()), |b| {
            b.iter(|| re.find_all(black_box(&text)));
        });
    }
    group.finish();
}

fn bench_replace_all(c: &mut Criterion) {
    let text = sample(SAMPLE_BYTES);
    let mut group = c.benchmark_group("replace_all");
    group.throughput(Throughput::Bytes(text.len() as u64));
    for engine in EngineKind::ALL {
        let re = compile(PATTERNS[1].1, engine);
        group.bench_function(BenchmarkId::new("email", engine.label()), |b| {
            b.iter(|| re.replace_all(black_box(&text), "$2:$1"));
        });
    }
    group.finish();
}

fn bench_explain(c: &mut Criterion) {
    c.bench_function("explain/email", |b| {
        b.iter(|| explain::explain(black_box(EXPLAIN_PATTERN), Flags::default()));
    });
}

/// Everything one keystroke triggers, at sample sizes from a snippet to a large log.
fn bench_analyze(c: &mut Criterion) {
    let mut group = c.benchmark_group("analyze");
    for bytes in [4 * 1024, 64 * 1024, 1024 * 1024] {
        let text = sample(bytes);
        let bench = Workbench::new(Library::load(None));
        let request = bench.request(Texts {
            pattern: PATTERNS[1].1,
            sample: &text,
            replacement: "$2:$1",
        });
        group.throughput(Throughput::Bytes(text.len() as u64));
        group.bench_function(
            BenchmarkId::new("email", format!("{}KiB", bytes / 1024)),
            |b| {
                b.iter(|| workbench::analyze(black_box(&request)));
            },
        );
    }
    group.finish();
}

criterion_group!(
    benches,
    bench_compile,
    bench_find_all,
    bench_replace_all,
    bench_explain,
    bench_analyze
);
criterion_main!(benches);
