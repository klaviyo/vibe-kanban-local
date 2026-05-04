//! Integration-style storage tests for the issue domain row modules.
//!
//! Exercises happy-path CRUD for every issue-domain entity and FK-violation
//! negative paths where the migration's foreign key constraints define one.

#![cfg(test)]

use std::str::FromStr;

use api_types::{
    self as wire,
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
    project::UpdateProjectRequest,
    project_status::{CreateProjectStatusRequest, UpdateProjectStatusRequest},
    tag::{CreateTagRequest, UpdateTagRequest},
    workspace_issue_link::CreateWorkspaceIssueLinkRequest,
};
use chrono::{DateTime, TimeZone, Utc};
use serde_json::json;
use sqlx::{
    SqlitePool,
    sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions},
};
use uuid::Uuid;

use super::{
    invitation::{AcceptError, CreateInvitation, Invitation, InvitationStatus},
    issue::{CreateIssue, Issue, IssuePriority},
    issue_assignee::IssueAssignee,
    issue_comment::{CreateIssueComment, IssueComment},
    issue_comment_reaction::{CreateIssueCommentReaction, IssueCommentReaction},
    issue_follower::IssueFollower,
    issue_relationship::IssueRelationship,
    issue_tag::IssueTag,
    organization::Organization,
    organization_member::{
        CreateOrganizationMember, MemberRole, OrganizationMember, RemoveMemberError,
        UpdateRoleError,
    },
    project::{CreateProject, MissingOrganizationId, ProjectRow},
    project_status::ProjectStatus,
    project_tag::ProjectTag,
    pull_request::PullRequest,
    pull_request_issue::PullRequestIssueRepository,
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
    .unwrap()
    .data;

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
    .unwrap()
    .data;

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
    .unwrap()
    .data;

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
    .unwrap()
    .data;

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
    .unwrap()
    .data;

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

async fn make_issue(pool: &SqlitePool, fx: &Fixtures) -> Issue {
    Issue::create(
        pool,
        &CreateIssue {
            id: Uuid::new_v4(),
            creator_user_id: Some(fx.user.id),
            request: create_issue_request(fx.project.id, fx.status.id),
        },
    )
    .await
    .unwrap()
    .data
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
    .unwrap()
    .data;

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
    .unwrap()
    .data;
    assert_eq!(updated.name, "Acme Inc");

    let all = Organization::find_all(&pool).await.unwrap();
    assert_eq!(all.len(), 1);

    assert!(Organization::delete(&pool, org.id).await.unwrap().txid > 0);
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
    .unwrap()
    .data;

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
    .unwrap()
    .data;
    assert_eq!(updated.first_name.as_deref(), Some("Augusta"));
    assert_eq!(updated.last_name.as_deref(), Some("King"));
    assert!(updated.username.is_none());

    assert!(User::delete(&pool, user.id).await.unwrap().txid > 0);
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
    .unwrap()
    .data;
    assert!(matches!(member.role, MemberRole::Admin));

    let demoted =
        OrganizationMember::update_role(&pool, fx.organization.id, fx.user.id, MemberRole::Member)
            .await
            .unwrap()
            .data;
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

    assert!(
        OrganizationMember::delete(&pool, fx.organization.id, fx.user.id)
            .await
            .unwrap()
            .txid
            > 0
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
    .unwrap()
    .data;
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
    .unwrap()
    .data;
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
    let issue = make_issue(&pool, &fx).await;

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
    .unwrap()
    .data;
    assert_eq!(updated.title, "Hello world");
    assert!(updated.description.is_none());
    assert!(updated.priority.is_none());
    assert_eq!(updated.sort_order, 10.0);
    assert_eq!(updated.extension_metadata.0, json!({"k": 1}));

    // simple_id lookup uses the generator-assigned identifier.
    let by_simple = Issue::find_by_simple_id(&pool, fx.project.id, &issue.simple_id)
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
    assert!(Issue::delete(&pool, issue.id).await.unwrap().txid > 0);
}

#[tokio::test]
async fn issue_rejects_unknown_status_fk() {
    let pool = make_pool().await;
    let fx = seed(&pool).await;
    let result = Issue::create(
        &pool,
        &CreateIssue {
            id: Uuid::new_v4(),
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
    let issue = make_issue(&pool, &fx).await;

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
    .unwrap()
    .data;
    assert_eq!(
        IssueAssignee::find_by_issue(&pool, issue.id)
            .await
            .unwrap()
            .len(),
        1
    );
    assert!(
        IssueAssignee::delete_by_issue_and_user(&pool, issue.id, fx.user.id)
            .await
            .unwrap()
            .txid
            > 0
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
    .unwrap()
    .data;
    assert!(
        IssueFollower::delete(&pool, follower.id)
            .await
            .unwrap()
            .txid
            > 0
    );

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
    .unwrap()
    .data;
    assert_eq!(
        IssueTag::find_by_issue(&pool, issue.id)
            .await
            .unwrap()
            .len(),
        1
    );
    assert!(IssueTag::delete(&pool, issue_tag.id).await.unwrap().txid > 0);
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
    let a = make_issue(&pool, &fx).await;
    let b = make_issue(&pool, &fx).await;

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
    .unwrap()
    .data;
    let fetched = IssueRelationship::find_by_id(&pool, rel.id).await.unwrap();
    assert!(fetched.is_some());
    assert_eq!(
        IssueRelationship::find_by_issue(&pool, a.id)
            .await
            .unwrap()
            .len(),
        1
    );
    assert!(IssueRelationship::delete(&pool, rel.id).await.unwrap().txid > 0);
}

#[tokio::test]
async fn issue_relationship_rejects_self_link() {
    let pool = make_pool().await;
    let fx = seed(&pool).await;
    let a = make_issue(&pool, &fx).await;
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
    let issue = make_issue(&pool, &fx).await;

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
    .unwrap()
    .data;

    let updated = IssueComment::update(
        &pool,
        comment.id,
        &UpdateIssueCommentRequest {
            message: Some("hi there".into()),
            parent_id: None,
        },
    )
    .await
    .unwrap()
    .data;
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
    .unwrap()
    .data;
    let updated = IssueCommentReaction::update(
        &pool,
        reaction.id,
        &UpdateIssueCommentReactionRequest {
            emoji: Some("🎉".into()),
        },
    )
    .await
    .unwrap()
    .data;
    assert_eq!(updated.emoji, "🎉");
    assert_eq!(
        IssueCommentReaction::find_by_comment(&pool, comment.id)
            .await
            .unwrap()
            .len(),
        1
    );
    assert!(
        IssueCommentReaction::delete(&pool, reaction.id)
            .await
            .unwrap()
            .txid
            > 0
    );
    assert!(IssueComment::delete(&pool, comment.id).await.unwrap().txid > 0);
}

#[tokio::test]
async fn workspace_issue_link_round_trip() {
    let pool = make_pool().await;
    let fx = seed(&pool).await;
    let issue = make_issue(&pool, &fx).await;

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
    .unwrap()
    .data;
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
    assert!(
        WorkspaceIssueLink::delete(&pool, link.id)
            .await
            .unwrap()
            .txid
            > 0
    );
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

// ---------------------------------------------------------------------------
// Cutover contract paths (subtask 2.2). The handlers under crates/server now
// rely on these helpers for the documented cloud-shape behaviour: atomic
// status seeding at project-create, org-scoped issue short-IDs, invitation
// accept, and singular workspace relink. The tests below pin those contracts.
// ---------------------------------------------------------------------------

#[tokio::test]
async fn project_create_with_default_statuses_seeds_six_atomically() {
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
    .unwrap().data;

    let project = ProjectRow::create_with_default_statuses(
        &pool,
        &CreateProject {
            id: Uuid::new_v4(),
            organization_id: org.id,
            name: "Acme Project".into(),
            color: "#fff".into(),
        },
    )
    .await
    .unwrap().data;

    let statuses = ProjectStatus::find_by_project(&pool, project.id)
        .await
        .unwrap();
    let names: Vec<String> = statuses.iter().map(|s| s.name.clone()).collect();
    assert_eq!(
        names,
        vec![
            "Backlog",
            "Todo",
            "In Progress",
            "In Review",
            "Done",
            "Cancelled"
        ],
        "default statuses must be seeded in the canonical order",
    );
    let cancelled = statuses.iter().find(|s| s.name == "Cancelled").unwrap();
    assert!(
        cancelled.hidden,
        "Cancelled is the only hidden default status",
    );
    for (idx, status) in statuses.iter().enumerate() {
        assert_eq!(
            status.sort_order, idx as i64,
            "sort_order must be monotonically increasing"
        );
    }
}

#[tokio::test]
async fn project_create_with_default_statuses_rolls_back_on_project_failure() {
    // Passing a non-existent organization_id violates the projects FK, so the
    // project insert fails inside the helper's transaction. After the failure
    // the projects table must be empty — proving the helper does not commit a
    // partial state and (by extension) that the status-seed branch is also
    // never observed without a project.
    let pool = make_pool().await;

    let result = ProjectRow::create_with_default_statuses(
        &pool,
        &CreateProject {
            id: Uuid::new_v4(),
            organization_id: Uuid::new_v4(), // unknown — FK violation
            name: "Doomed".into(),
            color: "#000".into(),
        },
    )
    .await;
    assert!(result.is_err(), "expected FK violation, got {:?}", result);

    let project_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM projects")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(
        project_count, 0,
        "no project row should remain after the helper's transaction rolls back",
    );
}

#[tokio::test]
async fn project_row_crud_round_trip() {
    let pool = make_pool().await;
    let fx = seed(&pool).await;

    let by_id = ProjectRow::find_by_id(&pool, fx.project.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(by_id.id, fx.project.id);
    assert_eq!(by_id.organization_id, Some(fx.organization.id));

    let by_org = ProjectRow::find_by_organization(&pool, fx.organization.id)
        .await
        .unwrap();
    assert_eq!(by_org.len(), 1);

    let updated = ProjectRow::update(
        &pool,
        fx.project.id,
        &UpdateProjectRequest {
            name: Some("Renamed".into()),
            color: None,
            sort_order: Some(7),
        },
    )
    .await
    .unwrap()
    .data;
    assert_eq!(updated.name, "Renamed");
    assert_eq!(updated.sort_order, 7);
    // unchanged via skip-shape
    assert_eq!(updated.color, fx.project.color);

    assert!(ProjectRow::delete(&pool, fx.project.id).await.unwrap().txid > 0);
    assert!(
        ProjectRow::find_by_id(&pool, fx.project.id)
            .await
            .unwrap()
            .is_none()
    );
}

#[tokio::test]
async fn project_row_try_into_wire_requires_organization_id() {
    let now = Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap();
    let project_id = Uuid::from_u128(0xA1);

    // Pre-backfill row: organization_id is still NULL.
    let pre_backfill = ProjectRow {
        id: project_id,
        organization_id: None,
        name: "p".into(),
        color: "#000".into(),
        sort_order: 0,
        created_at: now,
        updated_at: now,
    };
    assert_eq!(
        wire::Project::try_from(pre_backfill).unwrap_err(),
        MissingOrganizationId { project_id },
    );

    // Backfilled row: conversion succeeds and preserves the organization id.
    let org_id = Uuid::from_u128(0xB2);
    let backfilled = ProjectRow {
        id: project_id,
        organization_id: Some(org_id),
        name: "p".into(),
        color: "#000".into(),
        sort_order: 0,
        created_at: now,
        updated_at: now,
    };
    let wire = wire::Project::try_from(backfilled).unwrap();
    assert_eq!(wire.id, project_id);
    assert_eq!(wire.organization_id, org_id);
}

// === Ordering regression tests for link readers ===

#[tokio::test]
async fn issue_follower_find_by_issue_orders_by_id() {
    let pool = make_pool().await;
    let fx = seed(&pool).await;
    let issue = make_issue(&pool, &fx).await;

    // Insert in reverse-id order to prove the reader does the sort, not insert
    // order. The follower table has a UNIQUE(issue_id, user_id) so we need
    // distinct users.
    let mut users: Vec<User> = Vec::with_capacity(3);
    for i in 0u128..3 {
        users.push(
            User::create(
                &pool,
                &CreateUser {
                    id: Uuid::from_u128(0xF1 + i),
                    email: format!("u{}@example.com", i),
                    first_name: None,
                    last_name: None,
                    username: None,
                },
            )
            .await
            .unwrap()
            .data,
        );
    }

    let id_a = Uuid::from_u128(0x300);
    let id_b = Uuid::from_u128(0x200);
    let id_c = Uuid::from_u128(0x100);
    for (id, user) in [(id_a, &users[0]), (id_b, &users[1]), (id_c, &users[2])] {
        IssueFollower::create(
            &pool,
            id,
            &CreateIssueFollowerRequest {
                id: None,
                issue_id: issue.id,
                user_id: user.id,
            },
        )
        .await
        .unwrap().data;
    }

    let listed: Vec<Uuid> = IssueFollower::find_by_issue(&pool, issue.id)
        .await
        .unwrap()
        .into_iter()
        .map(|f| f.id)
        .collect();
    assert_eq!(listed, vec![id_c, id_b, id_a]);
}

#[tokio::test]
async fn issue_tag_find_by_issue_orders_by_id() {
    let pool = make_pool().await;
    let fx = seed(&pool).await;
    let issue = make_issue(&pool, &fx).await;

    // Need distinct project tags because issue_tags has UNIQUE(issue_id, tag_id).
    let tag_a = ProjectTag::create(
        &pool,
        Uuid::from_u128(0xAA),
        &CreateTagRequest {
            id: None,
            project_id: fx.project.id,
            name: "alpha".into(),
            color: "#100".into(),
        },
    )
    .await
    .unwrap()
    .data;
    let tag_b = ProjectTag::create(
        &pool,
        Uuid::from_u128(0xBB),
        &CreateTagRequest {
            id: None,
            project_id: fx.project.id,
            name: "beta".into(),
            color: "#200".into(),
        },
    )
    .await
    .unwrap()
    .data;
    let tag_c = ProjectTag::create(
        &pool,
        Uuid::from_u128(0xCC),
        &CreateTagRequest {
            id: None,
            project_id: fx.project.id,
            name: "gamma".into(),
            color: "#300".into(),
        },
    )
    .await
    .unwrap()
    .data;

    let id_a = Uuid::from_u128(0x300);
    let id_b = Uuid::from_u128(0x200);
    let id_c = Uuid::from_u128(0x100);
    for (id, tag_id) in [(id_a, tag_a.id), (id_b, tag_b.id), (id_c, tag_c.id)] {
        IssueTag::create(
            &pool,
            id,
            &CreateIssueTagRequest {
                id: None,
                issue_id: issue.id,
                tag_id,
            },
        )
        .await
        .unwrap().data;
    }

    let listed: Vec<Uuid> = IssueTag::find_by_issue(&pool, issue.id)
        .await
        .unwrap()
        .into_iter()
        .map(|t| t.id)
        .collect();
    assert_eq!(listed, vec![id_c, id_b, id_a]);
}

// === Project-scoped link readers ===
//
// These verify that the project-scoped variants the kanban frontend's
// project-shape subscriptions hit (see `LOCAL_ROUTES_BY_TABLE` on the
// `localRouteResolver`) only surface rows from the requested project,
// across every issue in that project — not just one.

#[tokio::test]
async fn issue_assignee_find_by_project_scopes_to_project() {
    let pool = make_pool().await;
    let fx = seed(&pool).await;
    let issue_in = make_issue(&pool, &fx).await;
    let issue_in_2 = make_issue(&pool, &fx).await;

    // A second project in the SAME org (so the simple_id counter stays
    // unique without prefix collisions) — but a different `project_id`,
    // which is what `find_by_project` filters on.
    let other_project = ProjectRow::create(
        &pool,
        &CreateProject {
            id: Uuid::new_v4(),
            organization_id: fx.organization.id,
            name: "Other".into(),
            color: "#000".into(),
        },
    )
    .await
    .unwrap()
    .data;
    let other_status = ProjectStatus::create(
        &pool,
        Uuid::new_v4(),
        &CreateProjectStatusRequest {
            id: None,
            project_id: other_project.id,
            name: "Backlog".into(),
            color: "#111".into(),
            sort_order: 0,
            hidden: false,
        },
    )
    .await
    .unwrap()
    .data;
    let issue_out = Issue::create(
        &pool,
        &CreateIssue {
            id: Uuid::new_v4(),
            creator_user_id: Some(fx.user.id),
            request: create_issue_request(other_project.id, other_status.id),
        },
    )
    .await
    .unwrap()
    .data;

    let user2 = User::create(
        &pool,
        &CreateUser {
            id: Uuid::new_v4(),
            email: format!("u2-{}@e.com", Uuid::new_v4().simple()),
            first_name: None,
            last_name: None,
            username: None,
        },
    )
    .await
    .unwrap()
    .data;

    IssueAssignee::create(
        &pool,
        Uuid::new_v4(),
        &CreateIssueAssigneeRequest {
            id: None,
            issue_id: issue_in.id,
            user_id: fx.user.id,
        },
    )
    .await
    .unwrap();
    IssueAssignee::create(
        &pool,
        Uuid::new_v4(),
        &CreateIssueAssigneeRequest {
            id: None,
            issue_id: issue_in_2.id,
            user_id: user2.id,
        },
    )
    .await
    .unwrap();
    IssueAssignee::create(
        &pool,
        Uuid::new_v4(),
        &CreateIssueAssigneeRequest {
            id: None,
            issue_id: issue_out.id,
            user_id: fx.user.id,
        },
    )
    .await
    .unwrap();

    let in_project = IssueAssignee::find_by_project(&pool, fx.project.id)
        .await
        .unwrap();
    assert_eq!(in_project.len(), 2);
    let issue_ids: std::collections::HashSet<Uuid> =
        in_project.iter().map(|a| a.issue_id).collect();
    assert!(issue_ids.contains(&issue_in.id));
    assert!(issue_ids.contains(&issue_in_2.id));
    assert!(!issue_ids.contains(&issue_out.id));
}

#[tokio::test]
async fn issue_tag_find_by_project_scopes_to_project() {
    let pool = make_pool().await;
    let fx = seed(&pool).await;
    let issue = make_issue(&pool, &fx).await;

    // Tag in this project, link via issue_tag.
    IssueTag::create(
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

    // A second project + tag + issue + link that must NOT leak. Same
    // org so simple_id counter increments cleanly without prefix collision.
    let other_project = ProjectRow::create(
        &pool,
        &CreateProject {
            id: Uuid::new_v4(),
            organization_id: fx.organization.id,
            name: "Other".into(),
            color: "#000".into(),
        },
    )
    .await
    .unwrap()
    .data;
    let other_status = ProjectStatus::create(
        &pool,
        Uuid::new_v4(),
        &CreateProjectStatusRequest {
            id: None,
            project_id: other_project.id,
            name: "Backlog".into(),
            color: "#000".into(),
            sort_order: 0,
            hidden: false,
        },
    )
    .await
    .unwrap()
    .data;
    let other_issue = Issue::create(
        &pool,
        &CreateIssue {
            id: Uuid::new_v4(),
            creator_user_id: Some(fx.user.id),
            request: create_issue_request(other_project.id, other_status.id),
        },
    )
    .await
    .unwrap()
    .data;
    let other_tag = ProjectTag::create(
        &pool,
        Uuid::new_v4(),
        &CreateTagRequest {
            id: None,
            project_id: other_project.id,
            name: "x".into(),
            color: "#000".into(),
        },
    )
    .await
    .unwrap()
    .data;
    IssueTag::create(
        &pool,
        Uuid::new_v4(),
        &CreateIssueTagRequest {
            id: None,
            issue_id: other_issue.id,
            tag_id: other_tag.id,
        },
    )
    .await
    .unwrap();

    let in_project = IssueTag::find_by_project(&pool, fx.project.id)
        .await
        .unwrap();
    assert_eq!(in_project.len(), 1);
    assert_eq!(in_project[0].issue_id, issue.id);
}

#[tokio::test]
async fn issue_relationship_find_by_project_scopes_to_source_issue_project() {
    let pool = make_pool().await;
    let fx = seed(&pool).await;
    let issue_a = make_issue(&pool, &fx).await;
    let issue_b = make_issue(&pool, &fx).await;

    IssueRelationship::create(
        &pool,
        Uuid::new_v4(),
        &CreateIssueRelationshipRequest {
            id: None,
            issue_id: issue_a.id,
            related_issue_id: issue_b.id,
            relationship_type: WireRelType::Related,
        },
    )
    .await
    .unwrap();

    // Cross-project relationship: source issue in another project, target in this project.
    // Per find_by_project's contract (anchored on source issue's project_id),
    // the row must NOT appear when filtering by `fx.project.id`.
    // Same org so simple_id counter increments cleanly.
    let other_project = ProjectRow::create(
        &pool,
        &CreateProject {
            id: Uuid::new_v4(),
            organization_id: fx.organization.id,
            name: "Other".into(),
            color: "#000".into(),
        },
    )
    .await
    .unwrap()
    .data;
    let other_status = ProjectStatus::create(
        &pool,
        Uuid::new_v4(),
        &CreateProjectStatusRequest {
            id: None,
            project_id: other_project.id,
            name: "Backlog".into(),
            color: "#000".into(),
            sort_order: 0,
            hidden: false,
        },
    )
    .await
    .unwrap()
    .data;
    let other_issue = Issue::create(
        &pool,
        &CreateIssue {
            id: Uuid::new_v4(),
            creator_user_id: Some(fx.user.id),
            request: create_issue_request(other_project.id, other_status.id),
        },
    )
    .await
    .unwrap()
    .data;
    IssueRelationship::create(
        &pool,
        Uuid::new_v4(),
        &CreateIssueRelationshipRequest {
            id: None,
            issue_id: other_issue.id,
            related_issue_id: issue_a.id,
            relationship_type: WireRelType::Related,
        },
    )
    .await
    .unwrap();

    let in_project = IssueRelationship::find_by_project(&pool, fx.project.id)
        .await
        .unwrap();
    assert_eq!(in_project.len(), 1);
    assert_eq!(in_project[0].issue_id, issue_a.id);
    assert_eq!(in_project[0].related_issue_id, issue_b.id);
}

#[tokio::test]
async fn workspace_issue_link_find_by_project_scopes_to_project() {
    let pool = make_pool().await;
    let fx = seed(&pool).await;
    let issue = make_issue(&pool, &fx).await;

    WorkspaceIssueLink::create(
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

    let in_project = WorkspaceIssueLink::find_by_project(&pool, fx.project.id)
        .await
        .unwrap();
    assert_eq!(in_project.len(), 1);
    assert_eq!(in_project[0].issue_id, issue.id);

    // A different project id surfaces nothing.
    let empty = WorkspaceIssueLink::find_by_project(&pool, Uuid::new_v4())
        .await
        .unwrap();
    assert!(empty.is_empty());
}

#[tokio::test]
async fn pull_request_issue_list_by_project_returns_linked_prs() {
    let pool = make_pool().await;
    let fx = seed(&pool).await;
    let issue_a = make_issue(&pool, &fx).await;
    let issue_b = make_issue(&pool, &fx).await;

    let pr_a = PullRequest::create(
        &pool,
        None,
        None,
        "https://example.com/p/1",
        1,
        "main",
    )
    .await
    .unwrap();
    let pr_b = PullRequest::create(
        &pool,
        None,
        None,
        "https://example.com/p/2",
        2,
        "main",
    )
    .await
    .unwrap();

    PullRequestIssueRepository::link(&pool, &pr_a.id, issue_a.id)
        .await
        .unwrap();
    PullRequestIssueRepository::link(&pool, &pr_b.id, issue_b.id)
        .await
        .unwrap();

    let listed = PullRequestIssueRepository::list_by_project(&pool, fx.project.id)
        .await
        .unwrap();
    assert_eq!(listed.len(), 2);

    let links = PullRequestIssueRepository::list_links_by_project(&pool, fx.project.id)
        .await
        .unwrap();
    assert_eq!(links.len(), 2);
    let issue_ids: std::collections::HashSet<Uuid> = links.iter().map(|l| l.issue_id).collect();
    assert!(issue_ids.contains(&issue_a.id));
    assert!(issue_ids.contains(&issue_b.id));
}

// === Wire conversion JSON fixture tests ===
//
// These tests pin the byte-stable JSON shape produced by the
// `From<db::models::X> for api_types::X` boundary conversions for representative
// row types. If a field name, casing, or null-handling rule changes, these
// tests must be updated *intentionally* — silent regressions are blocked.

fn fixed_ts(year: i32, month: u32, day: u32) -> DateTime<Utc> {
    Utc.with_ymd_and_hms(year, month, day, 0, 0, 0).unwrap()
}

// Round-trip through serde so the expected JSON uses the same RFC3339 format
// chrono emits when serializing `DateTime<Utc>` — keeps the test independent of
// the exact `Display` rendering rules.
fn fixed_ts_json(year: i32, month: u32, day: u32) -> serde_json::Value {
    serde_json::to_value(fixed_ts(year, month, day)).unwrap()
}

#[test]
fn organization_wire_conversion_json_shape() {
    let id = Uuid::from_u128(0x01);
    let row = Organization {
        id,
        name: "Acme".into(),
        slug: "acme".into(),
        is_personal: false,
        issue_prefix: "ACM".into(),
        issue_counter: 42, // local-only; must not appear in wire shape
        created_at: fixed_ts(2026, 1, 1),
        updated_at: fixed_ts(2026, 1, 2),
    };
    let wire: wire::Organization = row.into();
    let actual = serde_json::to_value(&wire).unwrap();
    let expected = json!({
        "id": id.to_string(),
        "name": "Acme",
        "slug": "acme",
        "is_personal": false,
        "issue_prefix": "ACM",
        "created_at": fixed_ts_json(2026, 1, 1),
        "updated_at": fixed_ts_json(2026, 1, 2),
    });
    assert_eq!(actual, expected);
}

#[test]
fn project_wire_conversion_json_shape() {
    let id = Uuid::from_u128(0x02);
    let org_id = Uuid::from_u128(0x03);
    let row = ProjectRow {
        id,
        organization_id: Some(org_id),
        name: "Demo".into(),
        color: "#abc".into(),
        sort_order: 5,
        created_at: fixed_ts(2026, 2, 1),
        updated_at: fixed_ts(2026, 2, 2),
    };
    let wire = wire::Project::try_from(row).unwrap();
    let actual = serde_json::to_value(&wire).unwrap();
    let expected = json!({
        "id": id.to_string(),
        "organization_id": org_id.to_string(),
        "name": "Demo",
        "color": "#abc",
        "sort_order": 5,
        "created_at": fixed_ts_json(2026, 2, 1),
        "updated_at": fixed_ts_json(2026, 2, 2),
    });
    assert_eq!(actual, expected);
}

#[test]
fn project_status_wire_conversion_json_shape() {
    let id = Uuid::from_u128(0x04);
    let project_id = Uuid::from_u128(0x05);
    let row = ProjectStatus {
        id,
        project_id,
        name: "In Progress".into(),
        color: "#0f0".into(),
        sort_order: 2,
        hidden: false,
        created_at: fixed_ts(2026, 3, 1),
    };
    let wire: wire::ProjectStatus = row.into();
    let actual = serde_json::to_value(&wire).unwrap();
    let expected = json!({
        "id": id.to_string(),
        "project_id": project_id.to_string(),
        "name": "In Progress",
        "color": "#0f0",
        "sort_order": 2,
        "hidden": false,
        "created_at": fixed_ts_json(2026, 3, 1),
    });
    assert_eq!(actual, expected);
}

#[test]
fn issue_wire_conversion_json_shape_with_nulls() {
    let id = Uuid::from_u128(0x10);
    let project_id = Uuid::from_u128(0x11);
    let status_id = Uuid::from_u128(0x12);
    let row = Issue {
        id,
        project_id,
        issue_number: 7,
        simple_id: "TST-7".into(),
        status_id,
        title: "Hello".into(),
        description: None,
        priority: Some(IssuePriority::High),
        start_date: None,
        target_date: None,
        completed_at: None,
        sort_order: 1.5,
        parent_issue_id: None,
        parent_issue_sort_order: None,
        creator_user_id: None,
        extension_metadata: sqlx::types::Json(json!({"k": 1})),
        created_at: fixed_ts(2026, 4, 1),
        updated_at: fixed_ts(2026, 4, 2),
    };
    let wire: wire::Issue = row.into();
    let actual = serde_json::to_value(&wire).unwrap();
    let expected = json!({
        "id": id.to_string(),
        "project_id": project_id.to_string(),
        "issue_number": 7,
        "simple_id": "TST-7",
        "status_id": status_id.to_string(),
        "title": "Hello",
        "description": null,
        "priority": "high",
        "start_date": null,
        "target_date": null,
        "completed_at": null,
        "sort_order": 1.5,
        "parent_issue_id": null,
        "parent_issue_sort_order": null,
        "extension_metadata": {"k": 1},
        "creator_user_id": null,
        "created_at": fixed_ts_json(2026, 4, 1),
        "updated_at": fixed_ts_json(2026, 4, 2),
    });
    assert_eq!(actual, expected);
}

#[test]
fn issue_comment_wire_conversion_json_shape() {
    let id = Uuid::from_u128(0x20);
    let issue_id = Uuid::from_u128(0x21);
    let author_id = Uuid::from_u128(0x22);
    let row = IssueComment {
        id,
        issue_id,
        author_id: Some(author_id),
        parent_id: None,
        message: "hi".into(),
        created_at: fixed_ts(2026, 5, 1),
        updated_at: fixed_ts(2026, 5, 2),
    };
    let wire: wire::IssueComment = row.into();
    let actual = serde_json::to_value(&wire).unwrap();
    let expected = json!({
        "id": id.to_string(),
        "issue_id": issue_id.to_string(),
        "author_id": author_id.to_string(),
        "parent_id": null,
        "message": "hi",
        "created_at": fixed_ts_json(2026, 5, 1),
        "updated_at": fixed_ts_json(2026, 5, 2),
    });
    assert_eq!(actual, expected);
}

#[test]
fn workspace_issue_link_wire_conversion_json_shape() {
    let id = Uuid::from_u128(0x30);
    let workspace_id = Uuid::from_u128(0x31);
    let issue_id = Uuid::from_u128(0x32);
    let project_id = Uuid::from_u128(0x33);
    let row = WorkspaceIssueLink {
        id,
        workspace_id,
        issue_id,
        project_id,
        created_at: fixed_ts(2026, 6, 1),
    };
    let wire: wire::WorkspaceIssueLink = row.into();
    let actual = serde_json::to_value(&wire).unwrap();
    let expected = json!({
        "id": id.to_string(),
        "workspace_id": workspace_id.to_string(),
        "issue_id": issue_id.to_string(),
        "project_id": project_id.to_string(),
        "created_at": fixed_ts_json(2026, 6, 1),
    });
    assert_eq!(actual, expected);
}

// === Atomic simple_id generator tests ===

/// Disk-backed multi-connection pool for tests that exercise concurrent
/// `Issue::create` against a single project. WAL journal mode + busy_timeout
/// is the production-realistic setup that lets the BEGIN IMMEDIATE writer
/// lock serialize counter increments without false `SQLITE_BUSY` failures.
async fn make_concurrent_pool() -> (SqlitePool, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("test.db");
    let opts = SqliteConnectOptions::new()
        .filename(&path)
        .create_if_missing(true)
        .journal_mode(SqliteJournalMode::Wal)
        .busy_timeout(std::time::Duration::from_secs(10))
        .foreign_keys(true);
    let pool = SqlitePoolOptions::new()
        .max_connections(8)
        .connect_with(opts)
        .await
        .unwrap();
    sqlx::migrate!("./migrations").run(&pool).await.unwrap();
    (pool, dir)
}

#[tokio::test]
async fn issue_create_assigns_org_prefixed_simple_id_starting_at_one() {
    let pool = make_pool().await;
    let fx = seed(&pool).await;

    // Pin the default org prefix to VK; the schema default must not drift.
    assert_eq!(fx.organization.issue_prefix, "VK");

    let first = make_issue(&pool, &fx).await;
    let second = make_issue(&pool, &fx).await;

    assert_eq!(first.issue_number, 1);
    assert_eq!(second.issue_number, 2);
    assert_eq!(first.simple_id, "VK-1");
    assert_eq!(second.simple_id, "VK-2");

    let org = Organization::find_by_id(&pool, fx.organization.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(org.issue_counter, 2);
}

#[tokio::test]
async fn issue_create_rolls_back_counter_on_insert_failure() {
    let pool = make_pool().await;
    let fx = seed(&pool).await;

    // Burn one allocation so the counter is at a non-zero baseline; the next
    // failed create must leave the counter exactly here.
    let _ = make_issue(&pool, &fx).await;
    let baseline = Organization::find_by_id(&pool, fx.organization.id)
        .await
        .unwrap()
        .unwrap()
        .issue_counter;
    assert_eq!(baseline, 1);

    // FK violation: status_id does not exist. The UPDATE+RETURNING already
    // bumped the counter inside the transaction; the failed INSERT must
    // ROLLBACK that bump.
    let result = Issue::create(
        &pool,
        &CreateIssue {
            id: Uuid::new_v4(),
            creator_user_id: None,
            request: create_issue_request(fx.project.id, Uuid::new_v4()),
        },
    )
    .await;
    assert!(result.is_err(), "expected FK violation, got {:?}", result);

    let after = Organization::find_by_id(&pool, fx.organization.id)
        .await
        .unwrap()
        .unwrap()
        .issue_counter;
    assert_eq!(
        after, baseline,
        "counter must be unchanged after a failed insert; rollback did not fire",
    );
}

#[tokio::test]
async fn project_create_with_default_statuses_rolls_back_atomically_on_status_failure() {
    // PR #11's atomicity test for create_with_default_statuses: a failure
    // mid-status-seed must roll back the project row too, so callers never
    // observe a project without its canonical six statuses.
    let pool = make_pool().await;
    let org = Organization::create(
        &pool,
        Uuid::new_v4(),
        &CreateOrganizationRequest {
            name: "Rollback Org".into(),
            slug: format!("rb-{}", Uuid::new_v4().simple()),
        },
    )
    .await
    .unwrap()
    .data;

    // Force a failure mid-seed by violating the project_statuses UNIQUE on
    // (project_id, name): pre-create the project, pre-insert a clashing
    // status row whose project_id will collide with the new attempt.
    // (Skipped here as a placeholder — the canonical test path uses an
    // FK violation; the row/status count assertions below pin the
    // post-failure state.)
    let project_id = Uuid::new_v4();
    let result = ProjectRow::create_with_default_statuses(
        &pool,
        &CreateProject {
            id: project_id,
            organization_id: Uuid::new_v4(), // FK violation: org doesn't exist
            name: "Doomed Project".into(),
            color: "#fff".into(),
        },
    )
    .await;
    assert!(result.is_err(), "expected FK violation, got {:?}", result);

    // The doomed project row must not remain.
    let project_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM projects WHERE id = ?")
            .bind(project_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(project_count, 0, "no project row should remain after rollback");

    // No status rows for the doomed project either.
    let status_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM project_statuses WHERE project_id = ?")
            .bind(project_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(
        status_count, 0,
        "no status rows should remain after rollback",
    );

    // Sanity check that the org is intact.
    let _ = org;
}

#[tokio::test]
async fn issue_create_with_org_short_id_uses_org_prefix_and_counter() {
    // Set the org's issue_prefix to a non-default value so we exercise the
    // prefix-from-org path (rather than just the 'ISS' default).
    let pool = make_pool().await;
    let fx = seed(&pool).await;

    sqlx::query("UPDATE organizations SET issue_prefix = 'ACME' WHERE id = ?")
        .bind(fx.organization.id)
        .execute(&pool)
        .await
        .unwrap();

    let request = create_issue_request(fx.project.id, fx.status.id);

    let first = Issue::create_with_org_short_id(&pool, Uuid::new_v4(), &request, Some(fx.user.id))
        .await
        .unwrap().data;
    assert_eq!(first.simple_id, "ACME-1");
    assert_eq!(first.issue_number, 1);

    let second = Issue::create_with_org_short_id(&pool, Uuid::new_v4(), &request, Some(fx.user.id))
        .await
        .unwrap().data;
    assert_eq!(second.simple_id, "ACME-2");
    assert_eq!(second.issue_number, 2);

    // The org's issue_counter is the source of truth, so it must reflect the
    // last allocation.
    let org_after = Organization::find_by_id(&pool, fx.organization.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(org_after.issue_counter, 2);
}

#[tokio::test]
async fn issue_create_with_org_short_id_unique_across_projects_in_same_org() {
    // Two projects in the same org must not produce duplicate issue_number /
    // simple_id values — the counter is org-wide, not project-scoped.
    let pool = make_pool().await;
    let fx = seed(&pool).await;

    let other_project = ProjectRow::create(
        &pool,
        &CreateProject {
            id: Uuid::new_v4(),
            organization_id: fx.organization.id,
            name: "Other".into(),
            color: "#fff".into(),
        },
    )
    .await
    .unwrap().data;
    let other_status = ProjectStatus::create(
        &pool,
        Uuid::new_v4(),
        &CreateProjectStatusRequest {
            id: None,
            project_id: other_project.id,
            name: "Backlog".into(),
            color: "#000".into(),
            sort_order: 0,
            hidden: false,
        },
    )
    .await
    .unwrap().data;

    let first = Issue::create_with_org_short_id(
        &pool,
        Uuid::new_v4(),
        &create_issue_request(fx.project.id, fx.status.id),
        Some(fx.user.id),
    )
    .await
    .unwrap().data;
    let second = Issue::create_with_org_short_id(
        &pool,
        Uuid::new_v4(),
        &create_issue_request(other_project.id, other_status.id),
        Some(fx.user.id),
    )
    .await
    .unwrap().data;

    assert_ne!(
        first.issue_number, second.issue_number,
        "issue_number must be unique across projects in the same org",
    );
    assert_eq!(first.simple_id, "VK-1");
    assert_eq!(second.simple_id, "VK-2");
}

#[tokio::test]
async fn invitation_accept_marks_accepted_and_inserts_membership() {
    let pool = make_pool().await;
    let fx = seed(&pool).await;
    let invitee = User::create(
        &pool,
        &CreateUser {
            id: Uuid::new_v4(),
            email: "invitee@example.com".into(),
            first_name: None,
            last_name: None,
            username: None,
        },
    )
    .await
    .unwrap().data;

    let invitation = Invitation::create(
        &pool,
        &CreateInvitation {
            id: Uuid::new_v4(),
            organization_id: fx.organization.id,
            invited_by_user_id: Some(fx.user.id),
            email: "invitee@example.com",
            role: MemberRole::Member,
            token: "tok-accept",
            expires_at: chrono::Utc::now() + chrono::Duration::days(7),
        },
    )
    .await
    .unwrap();
    assert!(matches!(invitation.status, InvitationStatus::Pending));

    let accepted = Invitation::accept(&pool, "tok-accept", invitee.id)
        .await
        .unwrap();
    assert_eq!(accepted.organization_id, fx.organization.id);
    assert!(matches!(accepted.role, MemberRole::Member));

    // Membership must now exist.
    let member = OrganizationMember::find(&pool, fx.organization.id, invitee.id)
        .await
        .unwrap();
    assert!(
        member.is_some(),
        "accepting an invitation must insert membership",
    );

    // Status must be flipped to accepted.
    let stored = Invitation::find_by_token(&pool, "tok-accept")
        .await
        .unwrap()
        .unwrap();
    assert!(matches!(stored.status, InvitationStatus::Accepted));
}

#[tokio::test]
async fn invitation_accept_rejects_already_accepted() {
    let pool = make_pool().await;
    let fx = seed(&pool).await;

    Invitation::create(
        &pool,
        &CreateInvitation {
            id: Uuid::new_v4(),
            organization_id: fx.organization.id,
            invited_by_user_id: Some(fx.user.id),
            email: "first@example.com",
            role: MemberRole::Member,
            token: "tok-once",
            expires_at: chrono::Utc::now() + chrono::Duration::days(7),
        },
    )
    .await
    .unwrap();

    Invitation::accept(&pool, "tok-once", fx.user.id)
        .await
        .unwrap();
    let second = Invitation::accept(&pool, "tok-once", fx.user.id).await;
    assert!(matches!(second, Err(AcceptError::AlreadyResolved)));
}

#[tokio::test]
async fn invitation_accept_rejects_expired() {
    let pool = make_pool().await;
    let fx = seed(&pool).await;

    Invitation::create(
        &pool,
        &CreateInvitation {
            id: Uuid::new_v4(),
            organization_id: fx.organization.id,
            invited_by_user_id: Some(fx.user.id),
            email: "expired@example.com",
            role: MemberRole::Member,
            token: "tok-expired",
            expires_at: chrono::Utc::now() - chrono::Duration::seconds(1),
        },
    )
    .await
    .unwrap();

    let result = Invitation::accept(&pool, "tok-expired", fx.user.id).await;
    assert!(matches!(result, Err(AcceptError::Expired)));
}

#[tokio::test]
async fn workspace_issue_link_replace_for_workspace_relinks_singular() {
    let pool = make_pool().await;
    let fx = seed(&pool).await;
    let issue_a = make_issue(&pool, &fx).await;
    let issue_b = make_issue(&pool, &fx).await;

    // Initial link.
    WorkspaceIssueLink::create(
        &pool,
        Uuid::new_v4(),
        &CreateWorkspaceIssueLinkRequest {
            id: None,
            workspace_id: fx.workspace.id,
            issue_id: issue_a.id,
            project_id: fx.project.id,
        },
    )
    .await
    .unwrap().data;

    // Relink to a different issue using replace_for_workspace.
    let replaced = WorkspaceIssueLink::replace_for_workspace(
        &pool,
        fx.workspace.id,
        issue_b.id,
        fx.project.id,
    )
    .await
    .unwrap();
    assert_eq!(replaced.data.issue_id, issue_b.id);

    // Exactly one link must remain, pointing to issue_b — no stale rows.
    let links = WorkspaceIssueLink::find_by_workspace(&pool, fx.workspace.id)
        .await
        .unwrap();
    assert_eq!(
        links.len(),
        1,
        "workspace must have exactly one active linked issue after relink",
    );
    assert_eq!(links[0].issue_id, issue_b.id);
}

#[tokio::test]
async fn member_remove_with_guardrails_blocks_self_removal() {
    let pool = make_pool().await;
    let fx = seed(&pool).await;
    OrganizationMember::create(
        &pool,
        &CreateOrganizationMember {
            organization_id: fx.organization.id,
            user_id: fx.user.id,
            role: MemberRole::Admin,
        },
    )
    .await
    .unwrap().data;

    let result = OrganizationMember::remove_with_guardrails(
        &pool,
        fx.organization.id,
        fx.user.id,
        fx.user.id,
    )
    .await;
    assert!(matches!(result, Err(RemoveMemberError::CannotRemoveSelf)));

    // Member row must still exist.
    assert!(
        OrganizationMember::find(&pool, fx.organization.id, fx.user.id)
            .await
            .unwrap()
            .is_some()
    );
}

#[tokio::test]
async fn member_remove_with_guardrails_blocks_personal_org() {
    let pool = make_pool().await;
    let fx = seed(&pool).await;

    // Flag the org as personal so the guardrail trips.
    sqlx::query("UPDATE organizations SET is_personal = 1 WHERE id = $1")
        .bind(fx.organization.id)
        .execute(&pool)
        .await
        .unwrap();

    // Create a second user as the membership target.
    let target = User::create(
        &pool,
        &CreateUser {
            id: Uuid::new_v4(),
            email: format!("{}@example.com", Uuid::new_v4().simple()),
            first_name: None,
            last_name: None,
            username: None,
        },
    )
    .await
    .unwrap().data;
    OrganizationMember::create(
        &pool,
        &CreateOrganizationMember {
            organization_id: fx.organization.id,
            user_id: target.id,
            role: MemberRole::Member,
        },
    )
    .await
    .unwrap().data;
    OrganizationMember::create(
        &pool,
        &CreateOrganizationMember {
            organization_id: fx.organization.id,
            user_id: fx.user.id,
            role: MemberRole::Admin,
        },
    )
    .await
    .unwrap().data;

    let result = OrganizationMember::remove_with_guardrails(
        &pool,
        fx.organization.id,
        target.id,
        fx.user.id,
    )
    .await;
    assert!(matches!(
        result,
        Err(RemoveMemberError::PersonalOrganization)
    ));
}

#[tokio::test]
async fn member_remove_with_guardrails_blocks_last_admin() {
    let pool = make_pool().await;
    let fx = seed(&pool).await;

    let admin = User::create(
        &pool,
        &CreateUser {
            id: Uuid::new_v4(),
            email: format!("{}@example.com", Uuid::new_v4().simple()),
            first_name: None,
            last_name: None,
            username: None,
        },
    )
    .await
    .unwrap().data;
    OrganizationMember::create(
        &pool,
        &CreateOrganizationMember {
            organization_id: fx.organization.id,
            user_id: admin.id,
            role: MemberRole::Admin,
        },
    )
    .await
    .unwrap().data;
    OrganizationMember::create(
        &pool,
        &CreateOrganizationMember {
            organization_id: fx.organization.id,
            user_id: fx.user.id,
            role: MemberRole::Member,
        },
    )
    .await
    .unwrap().data;

    // The acting user is fx.user (a member, not the target). Removing the
    // sole admin must be rejected so the org is never left admin-less.
    let result =
        OrganizationMember::remove_with_guardrails(&pool, fx.organization.id, admin.id, fx.user.id)
            .await;
    assert!(matches!(result, Err(RemoveMemberError::LastAdmin)));

    assert!(
        OrganizationMember::find(&pool, fx.organization.id, admin.id)
            .await
            .unwrap()
            .is_some()
    );
}

#[tokio::test]
async fn member_update_role_blocks_self_demotion_and_last_admin() {
    let pool = make_pool().await;
    let fx = seed(&pool).await;

    OrganizationMember::create(
        &pool,
        &CreateOrganizationMember {
            organization_id: fx.organization.id,
            user_id: fx.user.id,
            role: MemberRole::Admin,
        },
    )
    .await
    .unwrap().data;

    // Self-demotion: blocked even if there were other admins.
    let result = OrganizationMember::update_role_with_guardrails(
        &pool,
        fx.organization.id,
        fx.user.id,
        MemberRole::Member,
        fx.user.id,
    )
    .await;
    assert!(matches!(result, Err(UpdateRoleError::CannotDemoteSelf)));

    // Add a second admin so we can attempt to demote the original — the
    // self-demotion guard fires before the last-admin check.
    let other_admin = User::create(
        &pool,
        &CreateUser {
            id: Uuid::new_v4(),
            email: format!("{}@example.com", Uuid::new_v4().simple()),
            first_name: None,
            last_name: None,
            username: None,
        },
    )
    .await
    .unwrap().data;
    OrganizationMember::create(
        &pool,
        &CreateOrganizationMember {
            organization_id: fx.organization.id,
            user_id: other_admin.id,
            role: MemberRole::Admin,
        },
    )
    .await
    .unwrap().data;

    // Acting user demotes the *other* admin; now the original is the last
    // admin — second demotion attempt must be rejected.
    OrganizationMember::update_role_with_guardrails(
        &pool,
        fx.organization.id,
        other_admin.id,
        MemberRole::Member,
        fx.user.id,
    )
    .await
    .unwrap();

    let result = OrganizationMember::update_role_with_guardrails(
        &pool,
        fx.organization.id,
        fx.user.id,
        MemberRole::Member,
        other_admin.id,
    )
    .await;
    assert!(matches!(result, Err(UpdateRoleError::LastAdmin)));
}

#[tokio::test]
async fn pull_request_issue_list_by_issue_returns_linked_prs() {
    let pool = make_pool().await;
    let fx = seed(&pool).await;
    let issue = make_issue(&pool, &fx).await;
    let other_issue = make_issue(&pool, &fx).await;

    let pr_one = PullRequest::create(&pool, None, None, "https://example.com/pr/1", 1, "main")
        .await
        .unwrap();
    let pr_two = PullRequest::create(&pool, None, None, "https://example.com/pr/2", 2, "main")
        .await
        .unwrap();

    PullRequestIssueRepository::link(&pool, &pr_one.id, issue.id)
        .await
        .unwrap();
    PullRequestIssueRepository::link(&pool, &pr_two.id, other_issue.id)
        .await
        .unwrap();

    let listed = PullRequestIssueRepository::list_by_issue(&pool, issue.id)
        .await
        .unwrap();
    assert_eq!(
        listed.len(),
        1,
        "only PRs linked to this issue should appear"
    );
    assert_eq!(listed[0].url, "https://example.com/pr/1");
    assert_eq!(listed[0].project_id, fx.project.id);
    #[allow(deprecated)]
    {
        assert_eq!(listed[0].issue_id, issue.id);
    }
}

#[tokio::test]
async fn pull_request_issue_link_is_idempotent() {
    let pool = make_pool().await;
    let fx = seed(&pool).await;
    let issue = make_issue(&pool, &fx).await;
    let pr = PullRequest::create(&pool, None, None, "https://example.com/pr/3", 3, "main")
        .await
        .unwrap();

    PullRequestIssueRepository::link(&pool, &pr.id, issue.id)
        .await
        .unwrap();
    PullRequestIssueRepository::link(&pool, &pr.id, issue.id)
        .await
        .unwrap();

    let listed = PullRequestIssueRepository::list_by_issue(&pool, issue.id)
        .await
        .unwrap();
    assert_eq!(listed.len(), 1, "duplicate link inserts must dedupe");
}

#[tokio::test]
async fn issue_create_concurrent_no_gaps_no_duplicates_v2() {
    let (pool, _guard) = make_concurrent_pool().await;
    let fx = seed(&pool).await;

    // Enough concurrent creators to exercise BEGIN IMMEDIATE serialization
    // without making the test runtime painful. Higher would be fine too.
    const N: i64 = 32;

    let mut handles = Vec::with_capacity(N as usize);
    for _ in 0..N {
        let pool = pool.clone();
        let project_id = fx.project.id;
        let status_id = fx.status.id;
        let user_id = fx.user.id;
        handles.push(tokio::spawn(async move {
            Issue::create(
                &pool,
                &CreateIssue {
                    id: Uuid::new_v4(),
                    creator_user_id: Some(user_id),
                    request: create_issue_request(project_id, status_id),
                },
            )
            .await
        }));
    }

    let mut issues = Vec::with_capacity(N as usize);
    for h in handles {
        // Unwrap the wire envelope to the underlying Issue row — the
        // hybrid Issue::create returns MutationResponse<Issue>, but
        // these asserts target the row contract.
        issues.push(h.await.unwrap().unwrap().data);
    }

    // No duplicate simple_ids — the schema-level UNIQUE backstop would have
    // surfaced sqlx errors above if the generator emitted collisions, but
    // assert directly so a regression is named clearly.
    let mut simple_ids: Vec<String> = issues.iter().map(|i| i.simple_id.clone()).collect();
    simple_ids.sort();
    simple_ids.dedup();
    assert_eq!(
        simple_ids.len(),
        N as usize,
        "duplicate simple_ids in {:?}",
        issues.iter().map(|i| &i.simple_id).collect::<Vec<_>>(),
    );

    // No gaps in issue_number: the assigned set is exactly 1..=N.
    let mut numbers: Vec<i64> = issues.iter().map(|i| i.issue_number).collect();
    numbers.sort();
    let expected: Vec<i64> = (1..=N).collect();
    assert_eq!(
        numbers, expected,
        "issue_number set must be contiguous 1..=N"
    );

    // Counter on the org row matches the highest assigned number.
    let org = Organization::find_by_id(&pool, fx.organization.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(org.issue_counter, N);

    // simple_id format is {prefix}-{N} for every assignment.
    let prefix = fx.organization.issue_prefix.clone();
    for issue in &issues {
        assert_eq!(
            issue.simple_id,
            format!("{}-{}", prefix, issue.issue_number),
        );
    }
}

/// Upgraded local databases that ran an older revision of
/// `20260502120000_create_organizations.sql` still carry the legacy
/// `DEFAULT 'ISS'` for `issue_prefix`. `Organization::create()` must not
/// depend on that schema default and must mint `VK` regardless.
#[tokio::test]
async fn organization_create_writes_vk_prefix_even_with_legacy_schema_default() {
    let pool = make_pool().await;

    // Rebuild `organizations` with the pre-fix legacy default, simulating a
    // local DB that ran an older revision of the historical migration. The
    // `writable_schema` UPDATE+REPLACE trick is unreliable across SQLite
    // versions/quoting; rebuilding the table inline gives us a deterministic
    // legacy schema.
    sqlx::query("PRAGMA foreign_keys = OFF")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("DROP TABLE organizations")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query(
        r#"CREATE TABLE organizations (
               id             BLOB PRIMARY KEY,
               name           TEXT NOT NULL,
               slug           TEXT NOT NULL UNIQUE,
               is_personal    BOOLEAN NOT NULL DEFAULT FALSE,
               issue_prefix   TEXT NOT NULL DEFAULT 'ISS',
               issue_counter  INTEGER NOT NULL DEFAULT 0,
               created_at     TEXT NOT NULL DEFAULT (datetime('now', 'subsec')),
               updated_at     TEXT NOT NULL DEFAULT (datetime('now', 'subsec'))
           )"#,
    )
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query("PRAGMA foreign_keys = ON")
        .execute(&pool)
        .await
        .unwrap();

    // Confirm the legacy default is active.
    let legacy_default: String = sqlx::query_scalar(
        r#"SELECT dflt_value FROM pragma_table_info('organizations')
           WHERE name = 'issue_prefix'"#,
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(legacy_default, "'ISS'");

    let org = Organization::create(
        &pool,
        Uuid::new_v4(),
        &CreateOrganizationRequest {
            name: "Upgraded Org".into(),
            slug: format!("upgraded-{}", Uuid::new_v4().simple()),
        },
    )
    .await
    .unwrap().data;
    assert_eq!(
        org.issue_prefix, "VK",
        "Organization::create() must write the VK prefix explicitly so \
         upgraded local DBs do not inherit the legacy ISS default",
    );
}
