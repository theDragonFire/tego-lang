#[allow(unused_macros)]
macro_rules! basic_test {
    ( $name:ident $( $actual:expr => $expected:expr );+) => {
        #[allow(clippy::eq_op)]
        #[test]
        fn $name() {
            $( assert_eq!($expected, $actual); )+
        }
    };
}

use std::path::PathBuf;
use structopt::StructOpt;

mod codefile;
mod repl;

fn main() {
    let cli = Cli::from_args();

    match cli {
        Cli::Repl => repl::run().unwrap_or(()),
        Cli::Run { file_loc } => codefile::run(file_loc).unwrap_or(()),
    }
}

#[derive(StructOpt)]
enum Cli {
    Repl,
    Run {
        #[structopt(name = "file-path", parse(from_os_str))]
        file_loc: PathBuf,
    },
}
