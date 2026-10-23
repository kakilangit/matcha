use std::path::Path;
use std::process::Command;

fn main() {
    let commit = git(&["rev-parse", "--short", "HEAD"]).unwrap_or_else(|| "unknown".to_owned());
    println!("cargo:rustc-env=GIT_COMMIT={commit}");

    // Rebuild when HEAD moves: a checkout rewrites HEAD, a commit rewrites the
    // branch's ref, and `git gc` moves that ref into packed-refs. Git reports the
    // paths, so worktrees, where `.git` is a file, work too.
    let (Some(git_dir), Some(common_dir)) = (
        git(&["rev-parse", "--git-dir"]),
        git(&["rev-parse", "--git-common-dir"]),
    ) else {
        println!("cargo:rerun-if-changed=build.rs");
        return;
    };
    let mut watched = vec![
        Path::new(&git_dir).join("HEAD"),
        Path::new(&common_dir).join("packed-refs"),
    ];
    if let Some(branch) = git(&["symbolic-ref", "-q", "HEAD"]) {
        watched.push(Path::new(&common_dir).join(branch));
    }
    // A missing path would make Cargo rerun this script on every build; a ref
    // that later appears or disappears changes one of the others anyway.
    for path in watched.iter().filter(|path| path.exists()) {
        println!("cargo:rerun-if-changed={}", path.display());
    }
}

/// Runs git with `args` and returns its trimmed output, or `None` if it fails.
fn git(args: &[&str]) -> Option<String> {
    let output = Command::new("git").args(args).output().ok()?;
    output
        .status
        .success()
        .then(|| String::from_utf8_lossy(&output.stdout).trim().to_owned())
}
