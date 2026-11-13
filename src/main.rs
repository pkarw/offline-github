mod app;
mod format;
mod launcher;
mod offline_api;
mod outbox;
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
    about = "GitHub in airplane mode. Work offline; queue durable changes and sync explicitly."
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
    /// Durable outgoing intentions, conflict resolution and recovery reports
    Outbox {
        #[command(subcommand)]
        command: OutboxCommand,
    },
    /// Create local branches/worktrees or queue a publication
    Branch {
        #[command(subcommand)]
        command: BranchCommand,
    },
    /// Commit already-staged local files (or tracked edits with --all)
    Commit {
        #[arg(long)]
        dir: PathBuf,
        #[arg(short = 'm', long)]
        message: String,
        #[arg(short = 'a', long)]
        all: bool,
    },
    /// Install local UI assets or show application paths
    App {
        #[command(subcommand)]
        command: AppCommand,
    },
    Serve {
        #[arg(long, default_value_t = 4173)]
        port: u16,
        /// Do not open the default browser (useful for headless servers)
        #[arg(long)]
        no_browser: bool,
        /// Write the actual bound URL for launchers
        #[arg(long)]
        url_file: Option<PathBuf>,
        #[arg(long, env = "OGH_WEB_DIR")]
        web_dir: Option<PathBuf>,
    },
}
#[derive(Subcommand)]
enum AppCommand {
    Install {
        #[arg(long, default_value = "web/dist")]
        web_dir: PathBuf,
    },
    Paths,
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
    Create(CreateIssue),
    Comment(CommentArgs),
    Edit(EditArgs),
}
#[derive(Subcommand)]
enum PrCommand {
    Comment(CommentArgs),
    Edit(EditArgs),
    Create(CreatePr),
    Review {
        number: String,
        #[arg(long, conflicts_with_all = ["request_changes","comment"])]
        approve: bool,
        #[arg(long, conflicts_with = "comment")]
        request_changes: bool,
        #[arg(long)]
        comment: bool,
        #[command(flatten)]
        body: BodyArgs,
    },

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
    Create {
        name: String,
        #[arg(long, default_value = "ededed")]
        color: String,
        #[arg(long, default_value = "")]
        description: String,
    },
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
        #[arg(long,default_value="all",value_parser=["open","closed","merged","all"])]
        state: String,
        #[arg(short = 'L', long, default_value_t = 30)]
        limit: usize,
        #[command(flatten)]
        format: Format,
    },
    Prs {
        query: String,
        #[arg(long,default_value="all",value_parser=["open","closed","merged","all"])]
        state: String,
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

#[derive(Subcommand)]
enum OutboxCommand {
    List,
    Report {
        #[arg(long,default_value="json",value_parser=["json","md"])]
        format: String,
    },
    Enqueue {
        #[arg(long)]
        kind: String,
        #[arg(long)]
        target: Option<String>,
        #[arg(long)]
        payload: String,
        #[arg(long, value_delimiter = ',')]
        depends_on: Vec<String>,
        #[arg(long)]
        client_id: Option<String>,
    },
    Resolve {
        id: String,
        #[arg(long,value_parser=["retry","use_local","keep_remote"])]
        action: String,
    },
    Sync {
        #[arg(long)]
        dry_run: bool,
        #[arg(long)]
        yes: bool,
        #[arg(long)]
        plan_token: Option<String>,
    },
}
#[derive(Subcommand)]
enum BranchCommand {
    Create {
        name: String,
        #[arg(long, default_value = "HEAD")]
        from: String,
        #[arg(long)]
        dir: PathBuf,
    },
    List,
    Publish {
        name: String,
    },
}
#[derive(Args)]
struct BodyArgs {
    #[arg(long, conflicts_with = "body_file")]
    body: Option<String>,
    #[arg(long)]
    body_file: Option<PathBuf>,
}
#[derive(Args)]
struct CommentArgs {
    number: String,
    #[command(flatten)]
    body: BodyArgs,
}
#[derive(Args)]
struct CreateIssue {
    #[arg(long)]
    title: String,
    #[command(flatten)]
    body: BodyArgs,
    #[arg(long, short = 'l', value_delimiter = ',')]
    label: Vec<String>,
}
#[derive(Args)]
struct CreatePr {
    #[arg(long)]
    title: String,
    #[arg(long)]
    head: String,
    #[arg(long)]
    base: String,
    #[arg(long)]
    draft: bool,
    #[command(flatten)]
    body: BodyArgs,
}
#[derive(Args)]
struct EditArgs {
    number: String,
    #[arg(long)]
    title: Option<String>,
    #[arg(long,value_parser=["open","closed"])]
    state: Option<String>,
    #[command(flatten)]
    body: BodyArgs,
    #[arg(long, value_delimiter = ',')]
    add_label: Vec<String>,
    #[arg(long, value_delimiter = ',')]
    remove_label: Vec<String>,
}
fn read_body(args: BodyArgs) -> Result<String> {
    if let Some(body) = args.body {
        return Ok(body);
    }
    if let Some(path) = args.body_file {
        if path.as_os_str() == "-" {
            use std::io::Read;
            let mut body = String::new();
            std::io::stdin()
                .take(10_000_001)
                .read_to_string(&mut body)?;
            anyhow::ensure!(body.len() <= 10_000_000, "Body exceeds 10 MB");
            return Ok(body);
        }
        return Ok(std::fs::read_to_string(path)?);
    }
    Ok(String::new())
}
fn queue(
    store: &Store,
    repo: &str,
    kind: &str,
    target: Option<String>,
    payload: serde_json::Value,
    depends_on: Vec<String>,
) -> Result<()> {
    let event = outbox::enqueue(
        store,
        outbox::Intent {
            repo: repo.into(),
            kind: kind.into(),
            target,
            payload,
            depends_on,
            client_id: None,
        },
    )?;
    format::emit(&serde_json::to_value(event)?, &Format::default())
}
fn queue_comment(store: &Store, repo: &str, args: CommentArgs) -> Result<()> {
    let target = if args.number.starts_with("local:") {
        args.number
    } else {
        parse_number(
            &args.number,
            repo,
            if args.number.contains("/pull/") {
                "pull"
            } else {
                "issues"
            },
        )?
        .to_string()
    };
    queue(
        store,
        repo,
        "comment",
        Some(target),
        json!({"body":read_body(args.body)?}),
        vec![],
    )
}
fn queue_edit(store: &Store, repo: &str, args: EditArgs, kind: &str) -> Result<()> {
    let target = if args.number.starts_with("local:") {
        args.number
    } else {
        parse_number(
            &args.number,
            repo,
            if kind == "pr" { "pull" } else { "issues" },
        )?
        .to_string()
    };
    let mut fields = serde_json::Map::new();
    if let Some(title) = args.title {
        fields.insert("title".into(), json!(title));
    }
    if let Some(state) = args.state {
        fields.insert("state".into(), json!(state));
    }
    if args.body.body.is_some() || args.body.body_file.is_some() {
        fields.insert("body".into(), json!(read_body(args.body)?));
    }
    anyhow::ensure!(
        !fields.is_empty() || !args.add_label.is_empty() || !args.remove_label.is_empty(),
        "No edit fields or label changes supplied"
    );
    if !fields.is_empty() {
        queue(
            store,
            repo,
            "edit",
            Some(target.clone()),
            json!({"item_kind":kind,"fields":fields}),
            vec![],
        )?;
    }
    if !args.add_label.is_empty() || !args.remove_label.is_empty() {
        queue(
            store,
            repo,
            "labels",
            Some(target),
            json!({"add":args.add_label,"remove":args.remove_label}),
            vec![],
        )?;
    }
    Ok(())
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
    anyhow::ensure!(
        s.last_sync.is_some(),
        "Incoming cache is not complete; run ogh sync -R {repo} before searching"
    );
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
                anyhow::ensure!(
                    s.last_sync.is_some(),
                    "Repository metadata is not synced; run ogh sync -R {repo} while online"
                );
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
                ItemCommand::Create(a) => {
                    let body = read_body(a.body)?;
                    queue(
                        &store,
                        &repo,
                        "issue_create",
                        None,
                        json!({"title":a.title,"body":body,"labels":a.label}),
                        vec![],
                    )?;
                }
                ItemCommand::Comment(a) => queue_comment(&store, &repo, a)?,
                ItemCommand::Edit(a) => queue_edit(&store, &repo, a, "issue")?,
            }
        }
        Commands::Pr { command } => {
            let repo = resolve_repo(&store, selected)?;
            match command {
                PrCommand::List(a) => list(&store, &repo, a, "pr")?,
                PrCommand::View(a) => view(&store, &repo, a, "pr")?,
                PrCommand::Comment(a) => queue_comment(&store, &repo, a)?,
                PrCommand::Edit(a) => queue_edit(&store, &repo, a, "pr")?,
                PrCommand::Create(a) => {
                    let body = read_body(a.body)?;
                    queue(
                        &store,
                        &repo,
                        "pr_create",
                        None,
                        json!({"title":a.title,"body":body,"head":a.head,"base":a.base,"draft":a.draft}),
                        vec![],
                    )?;
                }
                PrCommand::Review {
                    number,
                    approve,
                    request_changes,
                    body,
                    ..
                } => {
                    let n = parse_number(&number, &repo, "pull")?;
                    let item = store.detail(&repo, n)?;
                    let sha = item["item"]["head"]["sha"]
                        .as_str()
                        .context("Cached PR head missing")?;
                    queue(
                        &store,
                        &repo,
                        "review",
                        Some(n.to_string()),
                        json!({"body":read_body(body)?,"event":if approve{"APPROVE"}else if request_changes{"REQUEST_CHANGES"}else{"COMMENT"},"commit_id":sha}),
                        vec![],
                    )?;
                }
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
                    let rows = checks.as_array().context("Invalid checks snapshot")?;
                    anyhow::ensure!(
                        !rows.is_empty(),
                        "No checks captured for this head; an empty cache is not a passing gate"
                    );
                    let failed = rows.iter().any(|v| {
                        matches!(
                            v["state"].as_str(),
                            Some(
                                "FAILURE"
                                    | "ERROR"
                                    | "CANCELLED"
                                    | "TIMED_OUT"
                                    | "ACTION_REQUIRED"
                                    | "STARTUP_FAILURE"
                                    | "STALE"
                            )
                        )
                    });
                    anyhow::ensure!(
                        !failed,
                        "Cached checks contain failures; not a live merge gate"
                    );
                    if rows.iter().any(|v| {
                        !matches!(v["state"].as_str(), Some("SUCCESS" | "NEUTRAL" | "SKIPPED"))
                    }) {
                        std::process::exit(8);
                    }
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
                LabelCommand::Create {
                    name,
                    color,
                    description,
                } => queue(
                    &store,
                    &repo,
                    "label_create",
                    None,
                    json!({"name":name,"color":color.trim_start_matches('#'),"description":description}),
                    vec![],
                )?,
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
            let (query, state, limit, fmt, kind) = match command {
                SearchCommand::Issues {
                    query,
                    state,
                    limit,
                    format,
                } => (query, state, limit, format, "issue"),
                SearchCommand::Prs {
                    query,
                    state,
                    limit,
                    format,
                } => (query, state, limit, format, "pr"),
            };
            let args = ListArgs {
                state,
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
        Commands::Outbox { command } => match command {
            OutboxCommand::List => {
                format::emit(&outbox::report(&store, selected, None)?, &Format::default())?
            }
            OutboxCommand::Report { format } => {
                let report = outbox::report(&store, selected, None)?;
                if format == "md" {
                    print!("{}", outbox::markdown(&report)?);
                } else {
                    format::emit(&report, &Format::default())?;
                }
            }
            OutboxCommand::Enqueue {
                kind,
                target,
                payload,
                depends_on,
                client_id,
            } => {
                let repo = resolve_repo(&store, selected)?;
                let payload = serde_json::from_str(&payload).context("--payload must be JSON")?;
                format::emit(
                    &serde_json::to_value(outbox::enqueue(
                        &store,
                        outbox::Intent {
                            repo,
                            kind,
                            target,
                            payload,
                            depends_on,
                            client_id,
                        },
                    )?)?,
                    &Format::default(),
                )?;
            }
            OutboxCommand::Resolve { id, action } => format::emit(
                &serde_json::to_value(outbox::resolve(&store, &id, &action)?)?,
                &Format::default(),
            )?,
            OutboxCommand::Sync {
                dry_run,
                yes,
                plan_token,
            } => {
                anyhow::ensure!(dry_run||yes,"Outgoing sync can publish changes. Review ogh outbox report first, then use --yes; --dry-run never sends anything.");
                let result = outbox::sync(&store, selected, dry_run, true, plan_token.as_deref())?;
                format::emit(&result, &Format::default())?;
                if !dry_run {
                    let unresolved = result["events"].as_array().is_some_and(|rows| {
                        rows.iter()
                            .any(|v| !matches!(v["status"].as_str(), Some("applied" | "cancelled")))
                    }) || result["incoming_errors"]
                        .as_array()
                        .is_some_and(|rows| !rows.is_empty());
                    anyhow::ensure!(!unresolved,"Sync has unresolved outgoing changes or incoming errors; confirmed results are retained. Inspect ogh outbox report");
                }
            }
        },
        Commands::Branch { command } => {
            let repo = resolve_repo(&store, selected)?;
            match command {
                BranchCommand::Create { name, from, dir } => {
                    process::run("git", &["check-ref-format", "--branch", &name], 10)?;
                    let sha = process::git(
                        &store.mirror(&repo)?,
                        &[
                            "rev-parse",
                            "--verify",
                            "--end-of-options",
                            &format!("{from}^{{commit}}"),
                        ],
                    )?;
                    let dir = store::absolute(&dir)?;
                    anyhow::ensure!(!dir.exists(), "Worktree destination already exists");
                    process::git(
                        &store.mirror(&repo)?,
                        &[
                            "worktree",
                            "add",
                            "-b",
                            &name,
                            dir.to_str().context("Invalid worktree path")?,
                            sha.trim(),
                        ],
                    )?;
                    format::emit(
                        &json!({"branch":name,"path":dir,"sha":sha.trim(),"published":false}),
                        &Format::default(),
                    )?;
                }
                BranchCommand::List => print!(
                    "{}",
                    process::git(
                        &store.mirror(&repo)?,
                        &[
                            "for-each-ref",
                            "--format=%(refname:short) %(objectname)",
                            "refs/heads/"
                        ]
                    )?
                ),
                BranchCommand::Publish { name } => {
                    let sha = process::git(
                        &store.mirror(&repo)?,
                        &[
                            "rev-parse",
                            "--verify",
                            "--end-of-options",
                            &format!("refs/heads/{name}"),
                        ],
                    )?;
                    queue(
                        &store,
                        &repo,
                        "branch_push",
                        None,
                        json!({"branch":name,"sha":sha.trim()}),
                        vec![],
                    )?;
                }
            }
        }
        Commands::Commit { dir, message, all } => {
            let repo = resolve_repo(&store, selected)?;
            let dir = store::absolute(&dir)?;
            let common = process::run(
                "git",
                &[
                    "-C",
                    dir.to_str().context("Invalid worktree path")?,
                    "rev-parse",
                    "--path-format=absolute",
                    "--git-common-dir",
                ],
                10,
            )?;
            anyhow::ensure!(
                std::fs::canonicalize(common.trim())?
                    == std::fs::canonicalize(store.mirror(&repo)?)?,
                "--dir must be a worktree of the selected cached repository"
            );
            let mut args = vec!["-C", dir.to_str().unwrap(), "commit", "-m", &message];
            if all {
                args.push("--all");
            }
            print!("{}", process::run("git", &args, 120)?);
        }
        Commands::App { command } => match command {
            AppCommand::Install { web_dir } => {
                format::emit(&app::install(&store, &web_dir)?, &Format::default())?
            }
            AppCommand::Paths => format::emit(&app::paths(&store), &Format::default())?,
        },
        Commands::Serve {
            port,
            web_dir,
            no_browser,
            url_file,
        } => {
            let web_dir = app::web_dir(&store, web_dir);
            server::serve(store, port, web_dir, no_browser, url_file).await?;
        }
    }
    Ok(())
}
