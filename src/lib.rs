//! Backlog API v2 の非公式 async クライアント。
//!
//! ```no_run
//! use backlog_api_rs::api::{AddIssueParams, GetIssuesParams};
//! use backlog_api_rs::BacklogClient;
//!
//! # async fn example() -> backlog_api_rs::Result<()> {
//! let client = BacklogClient::new("https://example.backlog.com", "API_KEY")?;
//!
//! // 自分の情報
//! let me = client.users().myself().await?;
//! println!("logged in as {}", me.name);
//!
//! // 課題の検索
//! let issues = client
//!     .issues()
//!     .list(GetIssuesParams::new().project_ids([12345]).count(50))
//!     .await?;
//!
//! // 課題の作成(projectId, summary, issueTypeId, priorityId)
//! let issue = client
//!     .issues()
//!     .add(AddIssueParams::new(12345, "新しい課題", 2, 3).description("詳細"))
//!     .await?;
//! # Ok(())
//! # }
//! ```

mod client;
mod download;
mod error;
mod origin_mark;
mod params;
mod partial;

pub mod api;
pub mod models;

pub use client::{BacklogClient, BacklogClientBuilder};
pub use download::{AttachmentDownload, SavedFile};
pub use error::{ApiError, Error, ErrorCode};
pub use origin_mark::OriginMark;
pub use partial::{PartialList, SkippedItem};

/// このクレートの標準Result型。
pub type Result<T> = std::result::Result<T, Error>;
