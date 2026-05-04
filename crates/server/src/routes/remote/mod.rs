use axum::Router;

use crate::DeploymentImpl;

mod issue_assignees;
mod issue_comment_reactions;
mod issue_comments;
mod issue_followers;
mod issue_relationships;
mod issue_tags;
mod issues;
mod notifications;
mod organization_member_metadata;
mod project_statuses;
mod projects;
pub mod pull_requests;
mod tags;
mod users;
mod workspaces;

pub fn router() -> Router<DeploymentImpl> {
    Router::new()
        .merge(issue_assignees::router())
        .merge(issue_comment_reactions::router())
        .merge(issue_comments::router())
        .merge(issue_followers::router())
        .merge(issue_relationships::router())
        .merge(issue_tags::router())
        .merge(issues::router())
        .merge(notifications::router())
        .merge(organization_member_metadata::router())
        .merge(projects::router())
        .merge(project_statuses::router())
        .merge(pull_requests::router())
        .merge(tags::router())
        .merge(users::router())
        .merge(workspaces::router())
}

#[cfg(test)]
mod tests {
    use api_types::DeleteResponse;
    use serde_json::json;
    use utils::response::ApiResponse;

    #[test]
    fn cloud_proxy_delete_envelope_preserves_txid_on_the_wire() {
        let envelope: ApiResponse<DeleteResponse> =
            ApiResponse::success(DeleteResponse { txid: 4242 });

        let body = serde_json::to_value(&envelope).expect("serialize envelope");

        assert_eq!(
            body,
            json!({
                "success": true,
                "data": { "txid": 4242 },
                "error_data": null,
                "message": null,
            }),
            "cloud-proxy delete routes must surface DeleteResponse.txid in the wire envelope; \
             returning ApiResponse<()> would silently drop it"
        );

        let round_trip: ApiResponse<DeleteResponse> =
            serde_json::from_value(body).expect("deserialize envelope");
        let data = round_trip.into_data().expect("envelope carries data");
        assert_eq!(data.txid, 4242);
    }
}
