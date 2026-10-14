use crate::models::{Category, IssueType, Project, Status, Version};
use crate::params::QueryPairs;
use crate::{BacklogClient, Result};

/// プロジェクト関連API。`client.projects()` で取得する。
#[derive(Debug, Clone, Copy)]
pub struct ProjectsApi<'a> {
    client: &'a BacklogClient,
}

impl<'a> ProjectsApi<'a> {
    pub(crate) fn new(client: &'a BacklogClient) -> Self {
        Self { client }
    }

    /// 参加しているプロジェクト一覧を取得する。
    pub async fn list(&self) -> Result<Vec<Project>> {
        self.client.get("/api/v2/projects", QueryPairs::new()).await
    }

    /// プロジェクトを取得する。`project_id_or_key` はID数値またはプロジェクトキー。
    pub async fn get(&self, project_id_or_key: &str) -> Result<Project> {
        self.client
            .get(
                &crate::client::api_path(&["projects", project_id_or_key]),
                QueryPairs::new(),
            )
            .await
    }

    /// プロジェクトの課題種別一覧を取得する。
    pub async fn issue_types(&self, project_id_or_key: &str) -> Result<Vec<IssueType>> {
        self.client
            .get(
                &crate::client::api_path(&["projects", project_id_or_key, "issueTypes"]),
                QueryPairs::new(),
            )
            .await
    }

    /// プロジェクトの状態一覧を取得する。
    pub async fn statuses(&self, project_id_or_key: &str) -> Result<Vec<Status>> {
        self.client
            .get(
                &crate::client::api_path(&["projects", project_id_or_key, "statuses"]),
                QueryPairs::new(),
            )
            .await
    }

    /// プロジェクトのカテゴリー一覧を取得する。
    pub async fn categories(&self, project_id_or_key: &str) -> Result<Vec<Category>> {
        self.client
            .get(
                &crate::client::api_path(&["projects", project_id_or_key, "categories"]),
                QueryPairs::new(),
            )
            .await
    }

    /// プロジェクトのバージョン(マイルストーン)一覧を取得する。
    pub async fn versions(&self, project_id_or_key: &str) -> Result<Vec<Version>> {
        self.client
            .get(
                &crate::client::api_path(&["projects", project_id_or_key, "versions"]),
                QueryPairs::new(),
            )
            .await
    }
}

#[cfg(test)]
mod tests {
    use crate::BacklogClient;
    use serde_json::json;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn project_json() -> serde_json::Value {
        json!({
            "id": 7, "projectKey": "TEST1", "name": "TEST1",
            "chartEnabled": true, "subtaskingEnabled": false,
            "projectLeaderCanEditProjectLeader": false,
            "useWiki": true, "useFileSharing": true, "useWikiTreeView": true,
            "useOriginalImageSizeAtWiki": false,
            "textFormattingRule": "markdown", "archived": false,
            "displayOrder": 0, "useDevAttributes": true
        })
    }

    #[tokio::test]
    async fn list_and_get_project() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/v2/projects"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!([project_json()])))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/api/v2/projects/TEST1"))
            .respond_with(ResponseTemplate::new(200).set_body_json(project_json()))
            .mount(&server)
            .await;

        let client = BacklogClient::new(&server.uri(), "KEY").unwrap();
        let projects = client.projects().list().await.unwrap();
        assert_eq!(projects.len(), 1);
        let project = client.projects().get("TEST1").await.unwrap();
        assert_eq!(project.id, 7);
    }

    #[tokio::test]
    async fn fetches_project_scoped_reference_data() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/v2/projects/TEST1/issueTypes"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!([
                { "id": 8, "projectId": 7, "name": "バグ", "color": "#990000", "displayOrder": 0 }
            ])))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/api/v2/projects/TEST1/statuses"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!([
                { "id": 1, "projectId": 7, "name": "未対応", "color": "#ed8077", "displayOrder": 1000 }
            ])))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/api/v2/projects/TEST1/categories"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!([
                { "id": 12, "name": "開発", "displayOrder": 0 }
            ])))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/api/v2/projects/TEST1/versions"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!([
                { "id": 3, "projectId": 7, "name": "v1", "description": null,
                  "startDate": null, "releaseDueDate": null, "archived": false, "displayOrder": 0 }
            ])))
            .mount(&server)
            .await;

        let client = BacklogClient::new(&server.uri(), "KEY").unwrap();
        assert_eq!(
            client.projects().issue_types("TEST1").await.unwrap()[0].name,
            "バグ"
        );
        assert_eq!(
            client.projects().statuses("TEST1").await.unwrap()[0].name,
            "未対応"
        );
        assert_eq!(
            client.projects().categories("TEST1").await.unwrap()[0].name,
            "開発"
        );
        assert_eq!(
            client.projects().versions("TEST1").await.unwrap()[0].name,
            "v1"
        );
    }
}
