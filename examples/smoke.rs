//! 実際のBacklogスペースに対する読み取り専用の動作確認。
//!
//! 実行方法:
//!
//! ```sh
//! BACKLOG_BASE_URL=https://<space>.backlog.com BACKLOG_API_KEY=<key> \
//!     cargo run --example smoke
//! ```

use backlog_api_rs::BacklogClient;
use backlog_api_rs::api::GetIssuesParams;

fn env(name: &str) -> String {
    std::env::var(name).unwrap_or_else(|_| {
        eprintln!("環境変数 {name} を設定してください");
        std::process::exit(1);
    })
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = BacklogClient::new(&env("BACKLOG_BASE_URL"), env("BACKLOG_API_KEY"))?;

    let me = client.users().myself().await?;
    println!("logged in as: {} (id={})", me.name, me.id);

    let priorities = client.space().priorities().await?;
    let names: Vec<&str> = priorities.iter().map(|p| p.name.as_str()).collect();
    println!("priorities: {names:?}");

    let projects = client.projects().list().await?;
    println!("projects: {}", projects.len());
    for project in projects.iter().take(3) {
        println!("  [{}] {}", project.project_key, project.name);
    }

    if let Some(project) = projects.first() {
        let issues = client
            .issues()
            .list(GetIssuesParams::new().project_ids([project.id]).count(5))
            .await?;
        println!("recent issues in {}:", project.project_key);
        for issue in &issues {
            println!(
                "  {} {} [{}]",
                issue.issue_key, issue.summary, issue.status.name
            );
        }
    }

    Ok(())
}
