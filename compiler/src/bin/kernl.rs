//! Script-friendly CLI: runs `.knl` programs without `--run`; pipes stdin into `main`'s `str` when stdin is not a TTY.
//! Use `kernlc` for full compiler flags (or `kernl … --compile …`).

fn main() {
    kernlc::cli::run(true);
}
