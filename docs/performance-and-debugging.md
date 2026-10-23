# Performance and debugging

Everything here runs through `make`, like the rest of the project. Run `make` to see every target.

| Command | Does |
| --- | --- |
| `make bench` | Run the benchmarks |
| `make bench-save` / `make bench-compare` | Save a baseline, then compare a change against it |
| `make profile` / `make profile-bench` | Profile the app or the benchmarks with samply |
| `make flamegraph` / `make flamegraph-bench` | Write a flamegraph SVG of the app or the benchmarks |
| `make run-debug` | Run with debug logs and backtraces |
| `make debugger` | Start the app under `rust-lldb` (macOS) or `rust-gdb` (Linux) |
| `make tools` | Install `cargo-flamegraph` and `samply` |

Variables, set on the command line (e.g. `make bench BENCH=find_all/email`):

| Variable | Default | Used by |
| --- | --- | --- |
| `BENCH` | all benchmarks | `bench*`, `profile-bench`, `flamegraph-bench`: a name filter such as `find_all`, `analyze/email/64KiB`, or `fancy-regex` |
| `BASELINE` | `main` | `bench-save`, `bench-compare` |
| `PROFILE_SECS` | `10` | `profile-bench`, `flamegraph-bench`: seconds to run each benchmark |
| `SAMPLY_FLAGS` | none | `profile`, `profile-bench`: e.g. `--save-only -o profile.json.gz` to save instead of opening a browser |
| `LOG` | `matcha=debug` | `run-debug`: the `RUST_LOG` filter |
| `DEBUGGER` | `rust-lldb` or `rust-gdb` | `debugger` |

## Benchmarks

`benches/search.rs` uses [criterion](https://docs.rs/criterion) to measure what Matcha runs on every keystroke, on generated log-like text that is the same on every run:

| Group | Measures |
| --- | --- |
| `compile` | `Regex::new` for each engine |
| `find_all` | Matching on 256 KiB with literal, capture-group, named-group, and alternation patterns on both engines, plus look-ahead and backreferences on `fancy-regex`. `match_limit` matches every word character, so it stops at `MATCH_LIMIT`: the worst case while a pattern is half typed |
| `replace_all` | Replacing with a `$2:$1` template on both engines |
| `explain` | The Explain panel's parser |
| `analyze` | One whole keystroke (compile, match, replace, explain) on 4 KiB, 64 KiB, and 1 MiB samples |

To check whether a change helps or hurts, save a baseline before it and compare after:

```sh
git switch main && make bench-save            # baseline "main"
git switch my-branch && make bench-compare    # prints the change for each benchmark
make bench-compare BENCH=replace_all          # or only some of them
```

Criterion writes its reports, with charts, to `target/criterion/report/index.html` (under `CARGO_TARGET_DIR` if set). Close other heavy programs while benchmarking; criterion warns when the noise is high.

## Profiling and flamegraphs

Both build with the `profiling` Cargo profile: release optimizations with line tables, so profiles show function names and source lines. Install the tools once with `make tools`.

- **samply** (`make profile`, `make profile-bench`) records a profile and opens it in the Firefox Profiler in your browser, with a call tree, flame graph, and timeline you can explore. Use it to find out where time goes.
- **cargo-flamegraph** (`make flamegraph`, `make flamegraph-bench`) writes a standalone SVG to `target/profiles/` (`app.svg` or `bench-<filter>.svg`). Open it in a browser; click a frame to zoom. Use it to share or attach to an issue.

For the app (`make profile`, `make flamegraph`), use Matcha to do the slow thing, then quit it with ⌘Q or Ctrl+Q; the profile is written when it exits. For benchmarks, filter to the case you care about, so the profile isn't a mix:

```sh
make flamegraph-bench BENCH=replace_all/email/fancy-regex PROFILE_SECS=5
```

Benchmarks run in criterion's `--profile-time` mode there: the code under test runs in a loop without the statistics, so it fills the profile.

Platform setup:

- **macOS:** samply works as is; run `samply setup` once if it asks to sign itself. cargo-flamegraph uses DTrace, which needs root, so `make flamegraph` runs it with `--root` and asks for your password.
- **Linux:** both use `perf` (`sudo apt install linux-perf` on Debian, `linux-tools-generic` on Ubuntu). If they can't record, allow it with `sudo sysctl kernel.perf_event_paranoid=1`. `make flamegraph` leaves `perf.data` in the repository; it is ignored by git and can be deleted.

### Example

The `replace_all` benchmarks showed `fancy-regex` replacing about 20 times slower than it matched. `make flamegraph-bench BENCH=replace_all/email/fancy-regex` put 94% of the time in `Captures::expand`, nearly all of it in `from_utf8`: fancy-regex re-checks the whole output string as UTF-8 each time it appends a match, so replacing got slower as the result grew. Expanding each match into a small scratch string first, then appending it, made it about 13 times faster; `make bench-compare BENCH=replace_all` showed −92%.

## Debugging

### Logs

`make run-debug` runs a debug build with `RUST_LOG=matcha=debug` and `RUST_BACKTRACE=1`. The log starts with the version and commit, then shows each keystroke and how long each search took:

```
INFO  matcha::startup] Matcha 0.1.2 (8152a57)
DEBUG matcha::app] search took 81.75µs
DEBUG matcha::startup] keystroke: ctrl-q
```

`LOG` sets the filter, e.g. `make run-debug LOG=matcha=trace,gpui=info`. Add your own with `log::debug!` while investigating.

Environment variables that help reproduce a problem:

- `MATCHA_LIBRARY=/tmp/lib.json` uses a separate library file, so your saved patterns stay untouched.
- `MATCHA_THEME=light` or `dark` forces a theme, for desktops that don't report one.

### Debugger

`make debugger` builds the debug binary and starts it under `rust-lldb` on macOS or `rust-gdb` on Linux (install `gdb` first). Set breakpoints with full paths, then run:

```
(gdb) break matcha::workbench::analyze
(gdb) run
```

`DEBUGGER` picks another one, e.g. `make debugger DEBUGGER=gdb`. For an IDE, point it at `target/debug/matcha` after `make build`.

### Panics and the UI

- A panic prints its backtrace under `make run-debug`. The code forbids `unwrap`, `expect`, and `panic!` outside tests and benches, so a panic usually comes from indexing or from a dependency.
- The UI-free logic lives in the library (`engine`, `explain`, `codegen`, `library`, `workbench`), so most bugs can be reproduced in a unit test without starting the app, and stepped through with `cargo test` under the debugger.
