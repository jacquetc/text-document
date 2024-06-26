use clap::{Parser, Subcommand};
use text_document::text_document_reader::TextDocumentReader;
use text_document::text_document_writer::TextDocumentWriter;
use text_document::TextDocument;

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
        #[arg(value_name = "FILE")]
        input_positional: String,

        #[arg(short, long, value_name = "FORMAT")]
        from: Option<String>,

        #[arg(short, long, value_name = "FORMAT")]
        to: String,

        #[arg(short, long, value_name = "FILE")]
        output: Option<String>,
    },
}

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Commands::Convert {
            input_positional,
            from,
            to,
            output,
        } => {
            let from = determine_input_formats(&input_positional, from);
            verify_output_format(&to);

            // add current directory to the input file if it's a relative path
            let input_positional = if input_positional.starts_with('/') {
                input_positional
            } else {
                format!(
                    "{}/{}",
                    std::env::current_dir().unwrap().to_str().unwrap(),
                    input_positional
                )
            };

            let mut text_document = TextDocument::new();

            //convert from input file to text_document
            match from.as_ref() {
                "plain-text" => {
                    let result = TextDocumentReader::new(&mut text_document)
                        .read_plain_text_file(&input_positional);
                    if let Err(e) = result {
                        eprintln!("Error: {}", e);
                        std::process::exit(exitcode::DATAERR);
                    }
                }
                "markdown" => {
                    let result = TextDocumentReader::new(&mut text_document)
                        .read_plain_text_file(&input_positional);
                    if let Err(e) = result {
                        eprintln!("Error: {}", e);
                        std::process::exit(exitcode::DATAERR);
                    }
                }
                _ => {
                    eprintln!("Error: Unknown input format '{}', please use either 'markdown' or 'plain-text'.", from);
                    std::process::exit(exitcode::USAGE)
                }
            }

            //convert from text_document to output file
            if let Some(output) = output {
                println!("Converting {} from {} to {}", input_positional, from, to);

                match to.as_ref() {
                    "plain-text" => {
                        let result =
                            TextDocumentWriter::new(&text_document).write_plain_text_file(&output);
                        if let Err(e) = result {
                            eprintln!("Error: {}", e);
                            std::process::exit(exitcode::DATAERR);
                        }
                    }
                    "markdown" => {
                        let result =
                            TextDocumentWriter::new(&text_document).write_plain_text_file(&output);
                        if let Err(e) = result {
                            eprintln!("Error: {}", e);
                            std::process::exit(exitcode::DATAERR);
                        }
                    }
                    _ => {
                        eprintln!("Error: Unknown output format '{}', please use either 'markdown' or 'plain-text'.", to);
                        std::process::exit(exitcode::USAGE)
                    }
                }

                println!("Write to {}", output);
            } else {
                println!("Output to stdout");
            }
            std::process::exit(exitcode::OK);
        }
    }
}

fn determine_input_formats(input: &str, from: Option<String>) -> String {
    match from {
        Some(from) => from,
        None => {
            let extension = input.split('.').last().unwrap();
            match extension {
                "md" => "markdown".to_string(),
                "txt" => "plain-text".to_string(),
                _ => {
                    eprintln!("Error: Couldn't determine input format from file extension, please specify it manually using the --from flag.");
                    eprintln!("Known extensions are .md, .txt.");
                    eprintln!("Known formats are 'markdown' and 'plain-text'.");
                    std::process::exit(exitcode::USAGE)
                }
            }
        }
    }
}

fn verify_output_format(to: &str) {
    match to {
        "markdown" | "plain-text" => (),
        _ => {
            eprintln!(
                "Error: Unknown output format '{}', please use either 'markdown' or 'plain-text'.",
                to
            );
            std::process::exit(exitcode::USAGE)
        }
    }
}
