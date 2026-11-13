mod process;
mod repository;
mod server;
mod store;
mod sync;
use anyhow::{Context, Result};
use clap::{Args, Parser, Subcommand};
use serde_json::{json, Value};
use std::path::PathBuf;
use store::Store;

#[derive(Parser)]
#[command(
    name = "ogh",
    version,
    about = "GitHub in airplane mode. Read locally, sync explicitly, never write upstream."
)]
struct Cli {
    #[arg(long, env = "OGH_DATA_DIR", global = true)]
    data_dir: Option<PathBuf>,
    #[command(subcommand)]
    command: Commands,
}
#[derive(Subcommand)]
enum Commands {
    /// Register or list local projects
    Repo {
        #[command(subcommand)]
        command: RepoCommand,
    },
    /// Pull GitHub changes and full Git history into the offline cache
    Sync {
        #[arg(short = 'R', long)]
        repo: Option<String>,
        #[arg(
            long,
            help = "Also download review summaries (one API request per updated PR)"
        )]
        reviews: bool,
        #[arg(
            long,
            help = "Rebuild metadata/conversations from GitHub to remove deleted records"
        )]
        full: bool,
    },
    /// Browse cached issues, gh-style
    Issue {
        #[command(subcommand)]
        command: ItemCommand,
    },
    /// Browse and check out cached pull requests, gh-style
    Pr {
        #[command(subcommand)]
        command: PrCommand,
    },
    /// Serve the offline web application on localhost
    Serve {
        #[arg(long, default_value_t = 4173)]
        port: u16,
        #[arg(long, default_value = "web/dist")]
        web_dir: PathBuf,
    },
}
#[derive(Subcommand)]
enum RepoCommand {
    Add { repository: String },
    List,
}
#[derive(Args)]
struct ListArgs {
    #[arg(short = 'R', long)]
    repo: String,
    #[arg(long,default_value="open",value_parser=["open","closed","all"])]
    state: String,
    #[arg(long, default_value = "")]
    search: String,
    #[arg(long)]
    json: bool,
    #[arg(short = 'L', long, default_value_t = 30)]
    limit: usize,
}
#[derive(Args)]
struct ViewArgs {
    number: u64,
    #[arg(short = 'R', long)]
    repo: String,
    #[arg(long)]
    json: bool,
}
#[derive(Subcommand)]
enum ItemCommand {
    List(ListArgs),
    View(ViewArgs),
}
#[derive(Subcommand)]
enum PrCommand {
    List(ListArgs),
    View(ViewArgs),
    Diff(ViewArgs),
    Checkout {
        number: u64,
        #[arg(short = 'R', long)]
        repo: String,
        #[arg(long)]
        dir: Option<PathBuf>,
    },
}
fn print_json(value: &Value) -> Result<()> {
    println!("{}", serde_json::to_string_pretty(value)?);
    Ok(())
}
fn list(store: &Store, args: ListArgs, kind: &str) -> Result<()> {
    let mut rows = store.items(&args.repo, kind, &args.state, &args.search)?;
    rows.truncate(args.limit);
    if args.json {
        return print_json(&json!(rows));
    }
    for v in rows {
        println!(
            "#{}\t{}\t{}",
            v["number"],
            v["state"].as_str().unwrap_or(""),
            v["title"].as_str().unwrap_or("")
        );
    }
    Ok(())
}
fn view(store: &Store, args: ViewArgs, kind: &str) -> Result<()> {
    let d = repository::detail(store, &args.repo, args.number)?;
    anyhow::ensure!(
        d["item"].get("pull_request").is_some() == (kind == "pr"),
        "Wrong item type"
    );
    if args.json {
        return print_json(&d);
    }
    println!(
        "#{} {}\n{} · @{}\n\n{}",
        args.number,
        d["item"]["title"].as_str().unwrap_or(""),
        d["item"]["state"].as_str().unwrap_or(""),
        d["item"]["user"]["login"].as_str().unwrap_or(""),
        d["item"]["body"].as_str().unwrap_or("")
    );
    for key in ["comments", "reviews", "review_comments"] {
        for c in d[key].as_array().unwrap() {
            println!(
                "\n--- @{} ({key}) ---\n{}",
                c["user"]["login"].as_str().unwrap_or(""),
                c["body"].as_str().unwrap_or("")
            );
        }
    }
    Ok(())
}
#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    let root = cli.data_dir.unwrap_or_else(|| {
        dirs::data_local_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("offline-github")
    });
    let store = Store::new(store::absolute(&root)?)?;
    match cli.command {
        Commands::Repo { command } => match command {
            RepoCommand::Add { repository } => print_json(&store.add(&repository)?)?,
            RepoCommand::List => print_json(&json!(store.repos()?))?,
        },
        Commands::Sync {
            repo,
            reviews,
            full,
        } => {
            let repos = if let Some(r) = repo {
                vec![r]
            } else {
                store
                    .repos()?
                    .iter()
                    .filter_map(|v| v["full_name"].as_str().map(str::to_owned))
                    .collect()
            };
            anyhow::ensure!(
                !repos.is_empty(),
                "No repositories registered; use ogh repo add owner/repo"
            );
            for r in repos {
                print_json(&sync::sync(&store, &r, reviews, full).with_context(|| {
                    format!(
                        "Sync of {r} failed; last completed watermark is retained, retry safely"
                    )
                })?)?;
            }
        }
        Commands::Issue { command } => match command {
            ItemCommand::List(a) => list(&store, a, "issue")?,
            ItemCommand::View(a) => view(&store, a, "issue")?,
        },
        Commands::Pr { command } => match command {
            PrCommand::List(a) => list(&store, a, "pr")?,
            PrCommand::View(a) => view(&store, a, "pr")?,
            PrCommand::Diff(a) => print!("{}", repository::diff(&store, &a.repo, a.number)?),
            PrCommand::Checkout { number, repo, dir } => {
                let dir = dir.unwrap_or_else(|| {
                    PathBuf::from(format!("{}-pr-{number}", repo.replace('/', "-")))
                });
                print_json(&repository::checkout(&store, &repo, number, &dir)?)?;
            }
        },
        Commands::Serve { port, web_dir } => server::serve(store, port, web_dir).await?,
    }
    Ok(())
}
