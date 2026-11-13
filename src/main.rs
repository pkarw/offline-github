mod format;
mod offline_api;
mod process;
mod repository;
mod search;
mod server;
mod skills;
mod store;
mod sync;
use anyhow::{Context, Result};
use clap::{Args, Parser, Subcommand};
use format::Format;
use serde_json::json;
use std::path::PathBuf;
use store::Store;

#[derive(Parser)]
#[command(
    name = "ogh",
    version,
    about = "GitHub in airplane mode. Cached reads, explicit sync, never write upstream."
)]
struct Cli {
    #[arg(long, env = "OGH_DATA_DIR", global = true)]
    data_dir: Option<PathBuf>,
    #[arg(short = 'R', long, env = "OGH_REPO", global = true)]
    repo: Option<String>,
    #[command(subcommand)]
    command: Commands,
}
#[derive(Subcommand)]
enum Commands {
    Repo {
        #[command(subcommand)]
        command: RepoCommand,
    },
    Sync {
        #[arg(long)]
        reviews: bool,
        #[arg(long)]
        full: bool,
    },
    Issue {
        #[command(subcommand)]
        command: ItemCommand,
    },
    Pr {
        #[command(subcommand)]
        command: PrCommand,
    },
    /// Read supported REST resources from disk; never forwards to GitHub
    Api(ApiArgs),
    Label {
        #[command(subcommand)]
        command: LabelCommand,
    },
    Run {
        #[command(subcommand)]
        command: RunCommand,
    },
    Search {
        #[command(subcommand)]
        command: SearchCommand,
    },
    /// Prepare per-PR review/CI context online, or inspect offline readiness
    Skills {
        #[command(subcommand)]
        command: SkillsCommand,
    },
    Serve {
        #[arg(long, default_value_t = 4173)]
        port: u16,
        #[arg(long, default_value = "web/dist")]
        web_dir: PathBuf,
    },
}
#[derive(Subcommand)]
enum RepoCommand {
    Add {
        repository: String,
    },
    List,
    View {
        repository: Option<String>,
        #[command(flatten)]
        format: Format,
    },
}
#[derive(Args)]
struct ListArgs {
    #[arg(long,default_value="open",value_parser=["open","closed","merged","all"])]
    state: String,
    #[arg(long, default_value = "")]
    search: String,
    #[arg(long)]
    author: Option<String>,
    #[arg(long)]
    assignee: Option<String>,
    #[arg(long, short = 'l', value_delimiter = ',')]
    label: Vec<String>,
    #[arg(long, short = 'B')]
    base: Option<String>,
    #[arg(long, short = 'H')]
    head: Option<String>,
    #[arg(long)]
    draft: bool,
    #[arg(short = 'L', long, default_value_t = 30)]
    limit: usize,
    #[command(flatten)]
    format: Format,
}
#[derive(Args)]
struct ViewArgs {
    number: String,
    #[arg(long)]
    comments: bool,
    #[command(flatten)]
    format: Format,
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
    Diff {
        number: String,
        #[arg(long)]
        name_only: bool,
    },
    Checkout {
        number: String,
        #[arg(long)]
        dir: Option<PathBuf>,
        #[arg(long,default_value="no",value_parser=["no"])]
        recurse_submodules: String,
    },
    Checks {
        number: String,
        #[arg(long)]
        required: bool,
        #[command(flatten)]
        format: Format,
    },
}
#[derive(Args)]
struct ApiArgs {
    endpoint: String,
    #[arg(short = 'X', long, default_value = "GET")]
    method: String,
    #[arg(long)]
    paginate: bool,
    #[arg(long, requires = "paginate")]
    slurp: bool,
    #[arg(short = 'q', long)]
    jq: Option<String>,
}
#[derive(Subcommand)]
enum LabelCommand {
    List {
        #[arg(short = 'L', long, default_value_t = 200)]
        limit: usize,
        #[command(flatten)]
        format: Format,
    },
}
#[derive(Subcommand)]
enum RunCommand {
    List {
        #[arg(long)]
        branch: Option<String>,
        #[arg(short = 'L', long, default_value_t = 20)]
        limit: usize,
        #[command(flatten)]
        format: Format,
    },
    View {
        id: u64,
        #[arg(long)]
        log_failed: bool,
        #[command(flatten)]
        format: Format,
    },
}
#[derive(Subcommand)]
enum SearchCommand {
    Issues {
        query: String,
        #[arg(short = 'L', long, default_value_t = 30)]
        limit: usize,
        #[command(flatten)]
        format: Format,
    },
    Prs {
        query: String,
        #[arg(short = 'L', long, default_value_t = 30)]
        limit: usize,
        #[command(flatten)]
        format: Format,
    },
}
#[derive(Subcommand)]
enum SkillsCommand {
    /// Explicit online GET/read snapshot: PR metadata/reviews, labels, checks, runs and optional failed logs
    Prepare {
        #[arg(long)]
        pr: u64,
        #[arg(long)]
        logs: bool,
    },
    /// Report cache coverage and timestamps; does not authenticate to GitHub
    Status {
        #[arg(long)]
        pr: Option<u64>,
    },
    /// Print the read-only tracker-provider descriptor for agents
    Provider,
}
fn parse_number(raw: &str, repo: &str, kind: &str) -> Result<u64> {
    if let Ok(n) = raw.trim_start_matches('#').parse::<u64>() {
        anyhow::ensure!(n > 0, "Item numbers must be positive");
        return Ok(n);
    }
    let prefix = format!("https://github.com/{repo}/{kind}/");
    let rest = raw
        .strip_prefix(&prefix)
        .context("Use a number or an item URL in the selected repository")?;
    let value = rest
        .trim_end_matches('/')
        .parse::<u64>()
        .context("Invalid item URL")?;
    anyhow::ensure!(value > 0, "Item numbers must be positive");
    Ok(value)
}
fn resolve_repo(store: &Store, explicit: Option<&str>) -> Result<String> {
    if let Some(repo) = explicit {
        store::validate_repo(repo)?;
        return Ok(repo.into());
    }
    if let Ok(repo) = std::env::var("GH_REPO") {
        store::validate_repo(&repo)?;
        return Ok(repo);
    }
    if let Ok(url) = process::run("git", &["remote", "get-url", "origin"], 10) {
        let url = url.trim().trim_end_matches('/').trim_end_matches(".git");
        for prefix in [
            "https://github.com/",
            "ssh://git@github.com/",
            "git@github.com:",
        ] {
            if let Some(repo) = url.strip_prefix(prefix) {
                store::validate_repo(repo)?;
                return Ok(repo.into());
            }
        }
    }
    let rows = store.repos()?;
    if rows.len() == 1 {
        return Ok(rows[0]["full_name"].as_str().unwrap().into());
    }
    anyhow::bail!("Select a cached repository with -R owner/repo or OGH_REPO (multiple/no cached repositories and no GitHub origin)")
}
fn list(store: &Store, repo: &str, args: ListArgs, kind: &str) -> Result<()> {
    format::require_fields(&args.format)?;
    let mut query = args.search;
    for (key, value) in [
        ("author", args.author),
        ("assignee", args.assignee),
        ("base", args.base),
        ("head", args.head),
    ] {
        if let Some(value) = value {
            query.push_str(&format!(" {key}:\"{}\"", value.replace('"', "\\\"")));
        }
    }
    for label in args.label {
        query.push_str(&format!(" label:\"{}\"", label.replace('"', "\\\"")));
    }
    if args.draft {
        query.push_str(" draft:true");
    }
    let search = search::Search::parse(&query)?;
    let s = store.load(repo)?;
    let mut rows = Vec::new();
    for (n, item) in &s.items {
        let pr = item.get("pull_request").is_some();
        let merged = !item["merged_at"].is_null();
        let state = match args.state.as_str() {
            "all" => true,
            "merged" => pr && merged,
            "closed" => item["state"] == "closed" && (!pr || !merged),
            _ => item["state"] == "open",
        };
        if pr == (kind == "pr") && state && search.matches(item, s.details.get(n), repo)? {
            rows.push(item.clone());
        }
    }
    rows.sort_by(|a, b| b["updated_at"].as_str().cmp(&a["updated_at"].as_str()));
    rows.truncate(args.limit);
    if args.format.json.is_some() || args.format.jq.is_some() {
        if args.format.json.as_ref().is_some_and(|s| s.is_empty()) {
            return format::emit(&json!(rows), &args.format);
        }
        let normalized = rows
            .iter()
            .map(|v| format::item(v, s.details.get(&v["number"].as_u64().unwrap()), repo))
            .collect::<Result<Vec<_>>>()?;
        return format::emit(&json!(normalized), &args.format);
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
fn view(store: &Store, repo: &str, args: ViewArgs, kind: &str) -> Result<()> {
    let n = parse_number(
        &args.number,
        repo,
        if kind == "pr" { "pull" } else { "issues" },
    )?;
    let d = repository::detail(store, repo, n)?;
    anyhow::ensure!(
        d["item"].get("pull_request").is_some() == (kind == "pr"),
        "Wrong item type"
    );
    if args.format.json.is_some() || args.format.jq.is_some() {
        let value = if args.format.json.as_ref().is_some_and(|s| s.is_empty()) {
            d.clone()
        } else {
            format::item(&d["item"], Some(&d), repo)?
        };
        return format::emit(&value, &args.format);
    }
    println!(
        "#{} {}\n{} · @{}\n\n{}",
        n,
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
    let selected = cli.repo.as_deref();
    match cli.command {
        Commands::Repo { command } => match command {
            RepoCommand::Add { repository } => {
                format::emit(&store.add(&repository)?, &Format::default())?
            }
            RepoCommand::List => format::emit(&json!(store.repos()?), &Format::default())?,
            RepoCommand::View { repository, format } => {
                let repo = resolve_repo(&store, repository.as_deref().or(selected))?;
                let s = store.load(&repo)?;
                let value = json!({"nameWithOwner":repo,"name":repo.split('/').nth(1),"description":s.description,"url":format!("https://github.com/{repo}"),"defaultBranchRef":{"name":s.default_branch},"visibility":if s.metadata["private"]==true{"PRIVATE"}else if s.metadata["private"]==false{"PUBLIC"}else{"UNKNOWN"}});
                format::emit(&value, &format)?;
            }
        },
        Commands::Sync { reviews, full } => {
            let repos = if let Some(r) = selected {
                vec![r.into()]
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
                format::emit(
                    &sync::sync(&store, &r, reviews, full).with_context(|| {
                        format!("Sync of {r} failed; last completed watermark retained")
                    })?,
                    &Format::default(),
                )?;
            }
        }
        Commands::Issue { command } => {
            let repo = resolve_repo(&store, selected)?;
            match command {
                ItemCommand::List(a) => list(&store, &repo, a, "issue")?,
                ItemCommand::View(a) => view(&store, &repo, a, "issue")?,
            }
        }
        Commands::Pr { command } => {
            let repo = resolve_repo(&store, selected)?;
            match command {
                PrCommand::List(a) => list(&store, &repo, a, "pr")?,
                PrCommand::View(a) => view(&store, &repo, a, "pr")?,
                PrCommand::Diff { number, name_only } => {
                    let n = parse_number(&number, &repo, "pull")?;
                    if name_only {
                        for f in repository::detail(&store, &repo, n)?["files"]
                            .as_array()
                            .context("Files missing")?
                        {
                            println!("{}", f["filename"].as_str().unwrap_or(""));
                        }
                    } else {
                        print!("{}", repository::diff(&store, &repo, n)?);
                    }
                }
                PrCommand::Checkout { number, dir, .. } => {
                    let n = parse_number(&number, &repo, "pull")?;
                    let dir = dir.unwrap_or_else(|| {
                        PathBuf::from(format!("{}-pr-{n}", repo.replace('/', "-")))
                    });
                    format::emit(
                        &repository::checkout(&store, &repo, n, &dir)?,
                        &Format::default(),
                    )?;
                }
                PrCommand::Checks {
                    number,
                    required,
                    format,
                } => {
                    let n = parse_number(&number, &repo, "pull")?;
                    let checks = skills::checks(&store, &repo, n, required)?;
                    format::emit(&checks, &format)?;
                }
            }
        }
        Commands::Api(args) => {
            anyhow::ensure!(
                args.method.eq_ignore_ascii_case("GET"),
                "ogh is read-only: API mutations are unavailable offline; no request was sent"
            );
            let value = offline_api::read(&store, selected, &args.endpoint, args.paginate)?;
            format::emit(
                &if args.slurp { json!([value]) } else { value },
                &Format {
                    json: None,
                    jq: args.jq,
                },
            )?;
        }
        Commands::Label { command } => {
            let repo = resolve_repo(&store, selected)?;
            match command {
                LabelCommand::List { limit, format } => {
                    let mut value = offline_api::read(
                        &store,
                        Some(&repo),
                        &format!("repos/{repo}/labels"),
                        true,
                    )?;
                    value
                        .as_array_mut()
                        .context("Labels unavailable")?
                        .truncate(limit);
                    format::emit(&value, &format)?;
                }
            }
        }
        Commands::Run { command } => {
            let repo = resolve_repo(&store, selected)?;
            match command {
                RunCommand::List {
                    branch,
                    limit,
                    format,
                } => {
                    let runs = skills::runs(&store, &repo, branch.as_deref(), limit)?;
                    format::emit(&runs, &format)?;
                }
                RunCommand::View {
                    id,
                    log_failed,
                    format,
                } => {
                    if log_failed {
                        print!("{}", skills::logs(&store, &repo, id)?);
                    } else {
                        format::emit(&skills::run_view(&store, &repo, id)?, &format)?;
                    }
                }
            }
        }
        Commands::Search { command } => {
            let repo = resolve_repo(&store, selected)?;
            let (query, limit, fmt, kind) = match command {
                SearchCommand::Issues {
                    query,
                    limit,
                    format,
                } => (query, limit, format, "issue"),
                SearchCommand::Prs {
                    query,
                    limit,
                    format,
                } => (query, limit, format, "pr"),
            };
            let args = ListArgs {
                state: "all".into(),
                search: query,
                author: None,
                assignee: None,
                label: vec![],
                base: None,
                head: None,
                draft: false,
                limit,
                format: fmt,
            };
            list(&store, &repo, args, kind)?;
        }
        Commands::Skills { command } => match command {
            SkillsCommand::Provider => {
                print!("{}", include_str!("../docs/trackers/offline-github.md"))
            }
            SkillsCommand::Prepare { pr, logs } => {
                let repo = resolve_repo(&store, selected)?;
                format::emit(
                    &skills::prepare(&store, &repo, pr, logs)?,
                    &Format::default(),
                )?;
            }
            SkillsCommand::Status { pr } => {
                let repo = resolve_repo(&store, selected)?;
                format::emit(&skills::status(&store, &repo, pr)?, &Format::default())?;
            }
        },
        Commands::Serve { port, web_dir } => server::serve(store, port, web_dir).await?,
    }
    Ok(())
}
