//! Integration-style storage tests for the issue domain row modules.
//!
//! Exercises happy-path CRUD for every issue-domain entity and FK-violation
//! negative paths where the migration's foreign key constraints define one.

#![cfg(test)]

use std::str::FromStr;

use api_types::{
    issue::{CreateIssueRequest, IssuePriority as WireIssuePriority, UpdateIssueRequest},
    issue_assignee::CreateIssueAssigneeRequest,
    issue_comment::{CreateIssueCommentRequest, UpdateIssueCommentRequest},
    issue_comment_reaction::{
        CreateIssueCommentReactionRequest, UpdateIssueCommentReactionRequest,
    },
    issue_follower::CreateIssueFollowerRequest,
    issue_relationship::{CreateIssueRelationshipRequest, IssueRelationshipType as WireRelType},
    issue_tag::CreateIssueTagRequest,
    organizations::{CreateOrganizationRequest, UpdateOrganizationRequest},
    project_status::{CreateProjectStatusRequest, UpdateProjectStatusRequest},
    tag::{CreateTagRequest, UpdateTagRequest},
    workspace_issue_link::CreateWorkspaceIssueLinkRequest,
};
use serde_json::json;
use sqlx::{
    SqlitePool,
    sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions},
};
use uuid::Uuid;

use super::{
    issue::{CreateIssue, Issue, IssuePriority},
    issue_assignee::IssueAssignee,
    issue_comment::{CreateIssueComment, IssueComment},
    issue_comment_reaction::{CreateIssueCommentReaction, IssueCommentReaction},
    issue_follower::IssueFollower,
    issue_relationship::IssueRelationship,
    issue_tag::IssueTag,
    organization::Organization,
    organization_member::{CreateOrganizationMember, MemberRole, OrganizationMember},
    project::{CreateProject, ProjectRow},
    project_status::ProjectStatus,
    project_tag::ProjectTag,
    user::{CreateUser, UpdateUser, User},
    workspace::{CreateWorkspace, Workspace},
    workspace_issue_link::WorkspaceIssueLink,
};

async fn make_pool() -> SqlitePool {
    let opts = SqliteConnectOptions::from_str("sqlite::memory:")
        .unwrap()
        .create_if_missing(true)
        .journal_mode(SqliteJournalMode::Delete)
        .foreign_keys(true);
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(opts)
        .await
        .unwrap();
    sqlx::migrate!("./migrations").run(&pool).await.unwrap();
    pool
}

struct Fixtures {
    organization: Organization,
    user: User,
    project: ProjectRow,
    status: ProjectStatus,
    tag: ProjectTag,
    workspace: Workspace,
}

async fn seed(pool: &SqlitePool) -> Fixtures {
    let organization = Organization::create(
        pool,
        Uuid::new_v4(),
        &CreateOrganizationRequest {
            name: "Test Org".into(),
            slug: format!("test-{}", Uuid::new_v4().simple()),
        },
    )
    .await
    .unwrap();

    let user = User::create(
        pool,
        &CreateUser {
            id: Uuid::new_v4(),
            email: format!("{}@example.com", Uuid::new_v4().simple()),
            first_name: Some("Ada".into()),
            last_name: Some("Lovelace".into()),
            username: Some("ada".into()),
        },
    )
    .await
    .unwrap();

    let project = ProjectRow::create(
        pool,
        &CreateProject {
            id: Uuid::new_v4(),
            organization_id: organization.id,
            name: "Test Project".into(),
            color: "#fff".into(),
        },
    )
    .await
    .unwrap();

    let status = ProjectStatus::create(
        pool,
        Uuid::new_v4(),
        &CreateProjectStatusRequest {
            id: None,
            project_id: project.id,
            name: "Backlog".into(),
            color: "#000".into(),
            sort_order: 0,
            hidden: false,
        },
    )
    .await
    .unwrap();

    let tag = ProjectTag::create(
        pool,
        Uuid::new_v4(),
        &CreateTagRequest {
            id: None,
            project_id: project.id,
            name: "feature".into(),
            color: "#abc".into(),
        },
    )
    .await
    .unwrap();

    let workspace = Workspace::create(
        pool,
        &CreateWorkspace {
            branch: "main".into(),
            name: Some("ws".into()),
        },
        Uuid::new_v4(),
    )
    .await
    .unwrap();

    Fixtures {
        organization,
        user,
        project,
        status,
        tag,
        workspace,
    }
}

fn create_issue_request(project_id: Uuid, status_id: Uuid) -> CreateIssueRequest {
    CreateIssueRequest {
        id: None,
        project_id,
        status_id,
        title: "Hello".into(),
        description: Some("body".into()),
        priority: Some(WireIssuePriority::Medium),
        start_date: None,
        target_date: None,
        completed_at: None,
        sort_order: 0.0,
        parent_issue_id: None,
        parent_issue_sort_order: None,
        extension_metadata: json!({}),
    }
}

async fn make_issue(pool: &SqlitePool, fx: &Fixtures, simple_id: &str) -> Issue {
    Issue::create(
        pool,
        &CreateIssue {
            id: Uuid::new_v4(),
            issue_number: 1,
            simple_id: simple_id.into(),
            creator_user_id: Some(fx.user.id),
            request: create_issue_request(fx.project.id, fx.status.id),
        },
    )
    .await
    .unwrap()
}

#[tokio::test]
async fn organization_crud_round_trip() {
    let pool = make_pool().await;
    let org = Organization::create(
        &pool,
        Uuid::new_v4(),
        &CreateOrganizationRequest {
            name: "Acme".into(),
            slug: "acme".into(),
        },
    )
    .await
    .unwrap();

    let fetched = Organization::find_by_id(&pool, org.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(fetched.slug, "acme");
    let by_slug = Organization::find_by_slug(&pool, "acme").await.unwrap();
    assert!(by_slug.is_some());

    let updated = Organization::update(
        &pool,
        org.id,
        &UpdateOrganizationRequest {
            name: "Acme Inc".into(),
        },
    )
    .await
    .unwrap();
    assert_eq!(updated.name, "Acme Inc");

    let all = Organization::find_all(&pool).await.unwrap();
    assert_eq!(all.len(), 1);

    assert_eq!(Organization::delete(&pool, org.id).await.unwrap(), 1);
    assert!(
        Organization::find_by_id(&pool, org.id)
            .await
            .unwrap()
            .is_none()
    );
}

#[tokio::test]
async fn user_crud_round_trip() {
    let pool = make_pool().await;
    let user = User::create(
        &pool,
        &CreateUser {
            id: Uuid::new_v4(),
            email: "ada@example.com".into(),
            first_name: Some("Ada".into()),
            last_name: None,
            username: None,
        },
    )
    .await
    .unwrap();

    assert_eq!(
        User::find_by_email(&pool, "ada@example.com")
            .await
            .unwrap()
            .unwrap()
            .id,
        user.id,
    );

    let updated = User::update(
        &pool,
        user.id,
        &UpdateUser {
            first_name: Some(Some("Augusta".into())),
            last_name: Some(Some("King".into())),
            username: Some(None),
        },
    )
    .await
    .unwrap();
    assert_eq!(updated.first_name.as_deref(), Some("Augusta"));
    assert_eq!(updated.last_name.as_deref(), Some("King"));
    assert!(updated.username.is_none());

    assert_eq!(User::delete(&pool, user.id).await.unwrap(), 1);
}

#[tokio::test]
async fn organization_member_crud_round_trip() {
    let pool = make_pool().await;
    let fx = seed(&pool).await;

    let member = OrganizationMember::create(
        &pool,
        &CreateOrganizationMember {
            organization_id: fx.organization.id,
            user_id: fx.user.id,
            role: MemberRole::Admin,
        },
    )
    .await
    .unwrap();
    assert!(matches!(member.role, MemberRole::Admin));

    let demoted =
        OrganizationMember::update_role(&pool, fx.organization.id, fx.user.id, MemberRole::Member)
            .await
            .unwrap();
    assert!(matches!(demoted.role, MemberRole::Member));

    assert_eq!(
        OrganizationMember::find_by_organization(&pool, fx.organization.id)
            .await
            .unwrap()
            .len(),
        1
    );
    assert_eq!(
        OrganizationMember::find_by_user(&pool, fx.user.id)
            .await
            .unwrap()
            .len(),
        1
    );

    assert_eq!(
        OrganizationMember::delete(&pool, fx.organization.id, fx.user.id)
            .await
            .unwrap(),
        1
    );
}

#[tokio::test]
async fn organization_member_rejects_unknown_org() {
    let pool = make_pool().await;
    let fx = seed(&pool).await;

    let result = OrganizationMember::create(
        &pool,
        &CreateOrganizationMember {
            organization_id: Uuid::new_v4(), // does not exist
            user_id: fx.user.id,
            role: MemberRole::Member,
        },
    )
    .await;
    assert!(result.is_err(), "expected FK violation, got {:?}", result);
}

#[tokio::test]
async fn project_status_and_tag_crud() {
    let pool = make_pool().await;
    let fx = seed(&pool).await;

    let updated = ProjectStatus::update(
        &pool,
        fx.status.id,
        &UpdateProjectStatusRequest {
            name: Some("In Progress".into()),
            color: None,
            sort_order: Some(2),
            hidden: Some(true),
        },
    )
    .await
    .unwrap();
    assert_eq!(updated.name, "In Progress");
    assert_eq!(updated.sort_order, 2);
    assert!(updated.hidden);

    let by_project = ProjectStatus::find_by_project(&pool, fx.project.id)
        .await
        .unwrap();
    assert_eq!(by_project.len(), 1);

    let tag_updated = ProjectTag::update(
        &pool,
        fx.tag.id,
        &UpdateTagRequest {
            name: Some("bug".into()),
            color: None,
        },
    )
    .await
    .unwrap();
    assert_eq!(tag_updated.name, "bug");
    assert_eq!(
        ProjectTag::find_by_project(&pool, fx.project.id)
            .await
            .unwrap()
            .len(),
        1
    );
}

#[tokio::test]
async fn issue_crud_round_trip_with_patch_shape() {
    let pool = make_pool().await;
    let fx = seed(&pool).await;
    let issue = make_issue(&pool, &fx, "TST-1").await;

    let fetched = Issue::find_by_id(&pool, issue.id).await.unwrap().unwrap();
    assert_eq!(fetched.title, "Hello");
    assert!(matches!(fetched.priority, Some(IssuePriority::Medium)));

    // Update: skip-some-fields, set-others, null-out priority.
    let updated = Issue::update(
        &pool,
        issue.id,
        &UpdateIssueRequest {
            status_id: None,
            title: Some("Hello world".into()),
            description: Some(None),
            priority: Some(None),
            start_date: None,
            target_date: None,
            completed_at: None,
            sort_order: Some(10.0),
            parent_issue_id: None,
            parent_issue_sort_order: None,
            extension_metadata: Some(json!({"k": 1})),
        },
    )
    .await
    .unwrap();
    assert_eq!(updated.title, "Hello world");
    assert!(updated.description.is_none());
    assert!(updated.priority.is_none());
    assert_eq!(updated.sort_order, 10.0);
    assert_eq!(updated.extension_metadata.0, json!({"k": 1}));

    // simple_id lookup
    let by_simple = Issue::find_by_simple_id(&pool, fx.project.id, "TST-1")
        .await
        .unwrap();
    assert!(by_simple.is_some());

    assert_eq!(
        Issue::find_by_project(&pool, fx.project.id)
            .await
            .unwrap()
            .len(),
        1
    );
    assert_eq!(Issue::delete(&pool, issue.id).await.unwrap(), 1);
}

#[tokio::test]
async fn issue_rejects_unknown_status_fk() {
    let pool = make_pool().await;
    let fx = seed(&pool).await;
    let result = Issue::create(
        &pool,
        &CreateIssue {
            id: Uuid::new_v4(),
            issue_number: 1,
            simple_id: "TST-99".into(),
            creator_user_id: None,
            request: create_issue_request(fx.project.id, Uuid::new_v4()),
        },
    )
    .await;
    assert!(result.is_err(), "expected FK violation, got {:?}", result);
}

#[tokio::test]
async fn issue_assignee_follower_tag_round_trip() {
    let pool = make_pool().await;
    let fx = seed(&pool).await;
    let issue = make_issue(&pool, &fx, "TST-2").await;

    let assignee = IssueAssignee::create(
        &pool,
        Uuid::new_v4(),
        &CreateIssueAssigneeRequest {
            id: None,
            issue_id: issue.id,
            user_id: fx.user.id,
        },
    )
    .await
    .unwrap();
    assert_eq!(
        IssueAssignee::find_by_issue(&pool, issue.id)
            .await
            .unwrap()
            .len(),
        1
    );
    assert_eq!(
        IssueAssignee::delete_by_issue_and_user(&pool, issue.id, fx.user.id)
            .await
            .unwrap(),
        1
    );
    assert!(
        IssueAssignee::find_by_id(&pool, assignee.id)
            .await
            .unwrap()
            .is_none()
    );

    let follower = IssueFollower::create(
        &pool,
        Uuid::new_v4(),
        &CreateIssueFollowerRequest {
            id: None,
            issue_id: issue.id,
            user_id: fx.user.id,
        },
    )
    .await
    .unwrap();
    assert_eq!(IssueFollower::delete(&pool, follower.id).await.unwrap(), 1);

    let issue_tag = IssueTag::create(
        &pool,
        Uuid::new_v4(),
        &CreateIssueTagRequest {
            id: None,
            issue_id: issue.id,
            tag_id: fx.tag.id,
        },
    )
    .await
    .unwrap();
    assert_eq!(
        IssueTag::find_by_issue(&pool, issue.id)
            .await
            .unwrap()
            .len(),
        1
    );
    assert_eq!(IssueTag::delete(&pool, issue_tag.id).await.unwrap(), 1);
}

#[tokio::test]
async fn issue_assignee_rejects_unknown_issue() {
    let pool = make_pool().await;
    let fx = seed(&pool).await;
    let result = IssueAssignee::create(
        &pool,
        Uuid::new_v4(),
        &CreateIssueAssigneeRequest {
            id: None,
            issue_id: Uuid::new_v4(), // unknown
            user_id: fx.user.id,
        },
    )
    .await;
    assert!(result.is_err());
}

#[tokio::test]
async fn issue_relationship_round_trip() {
    let pool = make_pool().await;
    let fx = seed(&pool).await;
    let a = make_issue(&pool, &fx, "TST-A").await;
    let b = make_issue(&pool, &fx, "TST-B").await;

    let rel = IssueRelationship::create(
        &pool,
        Uuid::new_v4(),
        &CreateIssueRelationshipRequest {
            id: None,
            issue_id: a.id,
            related_issue_id: b.id,
            relationship_type: WireRelType::Blocking,
        },
    )
    .await
    .unwrap();
    let fetched = IssueRelationship::find_by_id(&pool, rel.id).await.unwrap();
    assert!(fetched.is_some());
    assert_eq!(
        IssueRelationship::find_by_issue(&pool, a.id)
            .await
            .unwrap()
            .len(),
        1
    );
    assert_eq!(IssueRelationship::delete(&pool, rel.id).await.unwrap(), 1);
}

#[tokio::test]
async fn issue_relationship_rejects_self_link() {
    let pool = make_pool().await;
    let fx = seed(&pool).await;
    let a = make_issue(&pool, &fx, "TST-S").await;
    let result = IssueRelationship::create(
        &pool,
        Uuid::new_v4(),
        &CreateIssueRelationshipRequest {
            id: None,
            issue_id: a.id,
            related_issue_id: a.id,
            relationship_type: WireRelType::Related,
        },
    )
    .await;
    assert!(
        result.is_err(),
        "expected CHECK violation, got {:?}",
        result
    );
}

#[tokio::test]
async fn issue_comment_and_reaction_round_trip() {
    let pool = make_pool().await;
    let fx = seed(&pool).await;
    let issue = make_issue(&pool, &fx, "TST-C").await;

    let comment = IssueComment::create(
        &pool,
        &CreateIssueComment {
            id: Uuid::new_v4(),
            author_id: Some(fx.user.id),
            request: CreateIssueCommentRequest {
                id: None,
                issue_id: issue.id,
                message: "hi".into(),
                parent_id: None,
            },
        },
    )
    .await
    .unwrap();

    let updated = IssueComment::update(
        &pool,
        comment.id,
        &UpdateIssueCommentRequest {
            message: Some("hi there".into()),
            parent_id: None,
        },
    )
    .await
    .unwrap();
    assert_eq!(updated.message, "hi there");

    let reaction = IssueCommentReaction::create(
        &pool,
        &CreateIssueCommentReaction {
            id: Uuid::new_v4(),
            user_id: fx.user.id,
            request: CreateIssueCommentReactionRequest {
                id: None,
                comment_id: comment.id,
                emoji: "👍".into(),
            },
        },
    )
    .await
    .unwrap();
    let updated = IssueCommentReaction::update(
        &pool,
        reaction.id,
        &UpdateIssueCommentReactionRequest {
            emoji: Some("🎉".into()),
        },
    )
    .await
    .unwrap();
    assert_eq!(updated.emoji, "🎉");
    assert_eq!(
        IssueCommentReaction::find_by_comment(&pool, comment.id)
            .await
            .unwrap()
            .len(),
        1
    );
    assert_eq!(
        IssueCommentReaction::delete(&pool, reaction.id)
            .await
            .unwrap(),
        1
    );
    assert_eq!(IssueComment::delete(&pool, comment.id).await.unwrap(), 1);
}

#[tokio::test]
async fn workspace_issue_link_round_trip() {
    let pool = make_pool().await;
    let fx = seed(&pool).await;
    let issue = make_issue(&pool, &fx, "TST-W").await;

    let link = WorkspaceIssueLink::create(
        &pool,
        Uuid::new_v4(),
        &CreateWorkspaceIssueLinkRequest {
            id: None,
            workspace_id: fx.workspace.id,
            issue_id: issue.id,
            project_id: fx.project.id,
        },
    )
    .await
    .unwrap();
    assert_eq!(
        WorkspaceIssueLink::find_by_issue(&pool, issue.id)
            .await
            .unwrap()
            .len(),
        1
    );
    assert_eq!(
        WorkspaceIssueLink::find_by_workspace(&pool, fx.workspace.id)
            .await
            .unwrap()
            .len(),
        1
    );
    assert_eq!(WorkspaceIssueLink::delete(&pool, link.id).await.unwrap(), 1);
}

#[tokio::test]
async fn workspace_issue_link_rejects_unknown_issue() {
    let pool = make_pool().await;
    let fx = seed(&pool).await;
    let result = WorkspaceIssueLink::create(
        &pool,
        Uuid::new_v4(),
        &CreateWorkspaceIssueLinkRequest {
            id: None,
            workspace_id: fx.workspace.id,
            issue_id: Uuid::new_v4(), // unknown
            project_id: fx.project.id,
        },
    )
    .await;
    assert!(result.is_err());
}
