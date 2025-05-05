#![allow(clippy::unit_arg)]

extern crate test; // needed
use super::*;

#[bench]
pub fn bench_eval(bencher: &mut test::Bencher) {
    bencher.iter(|| {
        let mut state = State {
            shell_env: Vec::new(),
            focus: Focus::Str(String::new()),
            working_dir: std::env::current_dir()
                .unwrap_or(std::env::home_dir().unwrap_or(PathBuf::from("/"))),
            aliases: Vec::new(),
            raw_term: None,
        };
        state.shell_env.push(ShellVar {
            name: "PROMPT1".to_string(),
            value: "\x1b[32m$u@$h\x1b[39m \x1b[34m$P\x1b[39m> ".to_string(),
        });
        state.shell_env.push(ShellVar {
            name: "PROMPT2".to_string(),
            value: "> ".to_string(),
        });
        core::hint::black_box(eval("", &mut state));
        core::hint::black_box(eval("()", &mut state));
        core::hint::black_box(eval("echo", &mut state));
    });
}
