use axum::Router;

use crate::DeploymentImpl;

mod cutover_stubs;
mod issue_assignees;
mod issue_relationships;
mod issue_tags;
mod issues;
mod project_statuses;
mod projects;
pub mod pull_requests;
mod tags;
mod workspaces;

pub fn router() -> Router<DeploymentImpl> {
    Router::new()
        .merge(issue_assignees::router())
        .merge(issue_relationships::router())
        .merge(issue_tags::router())
        .merge(issues::router())
        .merge(projects::router())
        .merge(project_statuses::router())
        .merge(pull_requests::router())
        .merge(tags::router())
        .merge(workspaces::router())
        // Cutover read-side stubs for the 6 entity shapes the kanban frontend
        // resolves through `localRouteResolver` but whose full CRUD routes
        // have not yet been wired (CRITICAL #6b — Round 2 follow-up).
        .merge(cutover_stubs::router())
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
