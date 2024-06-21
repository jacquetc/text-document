use clap::{Parser, Subcommand};

#[derive(Parser, Debug)]
#[command(name = "text-document", about, version, author)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    Convert {
        /// Input file as a positional argument
        #[arg(value_name = "FILE", required = false)]
        input_positional: Option<String>,

        /// Input file via --input flag
        #[arg(long, required = false, value_name = "FILE")]
        input_flag: Option<String>,

        #[arg(long, required = true, value_name = "FORMAT")]
        to: String,

        #[arg(long, required = true, value_name = "FILE")]
        output: String,
    },
}

fn main() {
    let cli = Cli::parse();

    match &cli.command {
        Commands::Convert {
            input_positional,
            input_flag,
            to,
            output,
        } => {
            // Check that one and only one input method is provided
            let input_file = match (input_positional, input_flag) {
                (Some(positional), None) => positional,
                (None, Some(flag)) => flag,
                (None, None) => panic!("An input file must be provided."),
                (Some(_), Some(_)) => panic!("Please specify an input file either positionally or with --input, but not both."),
            };

            println!("Converting from ({}) to {} ({}).", input_file, to, output);
            // Implement your conversion logic here
        }
    }
}
