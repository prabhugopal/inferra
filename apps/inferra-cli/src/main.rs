use inferra_app::InferenceService;
use inferra_mistral::MistralRsBackend;
use clap::{Parser, Subcommand};
use inferra_core::{ChatMessage, GenerationEvent, GenerationOptions, GenerationRequest, Usage};
use std::io::Write;
use std::{error::Error, path::PathBuf, sync::Arc, time::Instant};

#[derive(Debug, Parser)]
#[command(name = "inferra", about = "Build, inspect, and understand LLM inference in Rust")]
struct Cli {
    /// Directory containing the local GGUF file.
    #[arg(long)]
    model_dir: PathBuf,
    /// GGUF file name within `model_dir`.
    #[arg(long)]
    gguf_file: String,
    /// Label reported as the model id (does not affect loading).
    #[arg(long, default_value = "local-gguf")]
    model: String,
    /// Hugging Face repo id to source tokenizer/chat-template metadata from,
    /// for GGUF files that do not embed their own.
    #[arg(long)]
    tok_model_id: Option<String>,
    /// Inline Jinja template or path to a chat-template JSON/Jinja file.
    #[arg(long)]
    chat_template: Option<String>,
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    Info,
    InspectGguf { path: PathBuf },
    Generate {
        prompt: String,
        #[arg(short = 'n', long, default_value_t = 128)]
        max_tokens: u32,
        #[arg(long, default_value_t = 0.0)]
        temperature: f32,
        #[arg(long, default_value_t = 0.95)]
        top_p: f32,
        #[arg(long, default_value_t = 42)]
        seed: u64,
        /// Stream tokens as they arrive instead of waiting for the full response.
        #[arg(long)]
        stream: bool,
        /// Skip requesting per-token log-probabilities (saves a per-token
        /// softmax over the vocabulary — useful for comparing decode speed).
        #[arg(long)]
        no_logprobs: bool,
    },
}

fn print_usage(usage: &Usage) {
    if let (Some(prompt_tokens), Some(prompt_tps), Some(prompt_time)) =
        (usage.prompt_tokens, usage.prompt_tokens_per_sec, usage.prompt_time_sec)
    {
        eprintln!("prefill: {prompt_tokens} tokens in {prompt_time:.3}s ({prompt_tps:.2} tok/s)");
    }
    if let (Some(completion_tokens), Some(completion_tps), Some(completion_time)) = (
        usage.completion_tokens,
        usage.completion_tokens_per_sec,
        usage.completion_time_sec,
    ) {
        eprintln!("decode: {completion_tokens} tokens in {completion_time:.3}s ({completion_tps:.2} tok/s)");
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let cli = Cli::parse();

    match cli.command {
        Command::Info => {
            println!("backend: mistral.rs (embedded)");
            println!("model_dir: {}", cli.model_dir.display());
            println!("gguf_file: {}", cli.gguf_file);
            println!("model label: {}", cli.model);
        }
        Command::InspectGguf { path } => {
            let header = inferra_gguf::read_header(&path)?;
            println!("path: {}", path.display());
            println!("GGUF version: {}", header.version);
            println!("tensor count: {}", header.tensor_count);
            println!("metadata entries: {}", header.metadata_kv_count);
        }
        Command::Generate { prompt, max_tokens, temperature, top_p, seed, stream, no_logprobs } => {
            let backend = MistralRsBackend::load(
                cli.model_dir.to_string_lossy().into_owned(),
                cli.gguf_file,
                cli.model,
                cli.tok_model_id,
                cli.chat_template,
            ).await?;
            let service = InferenceService::new(Arc::new(backend));
            let request = GenerationRequest {
                messages: vec![ChatMessage::user(prompt)],
                options: GenerationOptions {
                    max_new_tokens: max_tokens,
                    temperature,
                    top_p,
                    seed: Some(seed),
                    want_logprobs: !no_logprobs,
                },
            };

            if stream {
                let started = Instant::now();
                let mut time_to_first_token = None;
                let mut rx = service.generate_stream(request).await?;
                while let Some(event) = rx.recv().await {
                    match event {
                        GenerationEvent::Token { delta, logprob } => {
                            if time_to_first_token.is_none() {
                                time_to_first_token = Some(started.elapsed());
                            }
                            print!("{delta}");
                            std::io::stdout().flush().ok();
                            if let Some(logprob) = logprob {
                                let probability = logprob.logprob.exp() * 100.0;
                                eprint!(" [{:.1}%]", probability);
                            }
                        }
                        GenerationEvent::Done { result } => {
                            println!();
                            if let Some(ttft) = time_to_first_token {
                                eprintln!("time to first token: {:.3}s", ttft.as_secs_f64());
                            }
                            eprintln!("elapsed: {:.3}s", result.elapsed.as_secs_f64());
                            print_usage(&result.usage);
                        }
                        GenerationEvent::Error { message } => eprintln!("error: {message}"),
                    }
                }
            } else {
                let result = service.generate(request).await?;
                println!("{}", result.text);
                eprintln!("elapsed: {:.3}s", result.elapsed.as_secs_f64());
                print_usage(&result.usage);
            }
        }
    }
    Ok(())
}
