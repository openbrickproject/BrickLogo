mod cli;
mod firmware;
mod repl;
mod script;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = cli::parse_cli_args();
    if let Some(fw) = args.firmware {
        return firmware::run(fw);
    }
    match args.script {
        Some(source) => script::run(source, args.net),
        None => repl::run(args.net),
    }
}
