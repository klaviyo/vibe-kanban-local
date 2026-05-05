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
    issue::{CreateIssue, Issue, IssuePriority},
    issue_assignee::IssueAssignee,
    issue_comment::{CreateIssueComment, IssueComment},
    issue_comment_reaction::{CreateIssueCommentReaction, IssueCommentReaction},
    issue_follower::IssueFollower,
    issue_relationship::IssueRelationship,
    issue_tag::IssueTag,
    organization::Organization,
    organization_member::{CreateOrganizationMember, MemberRole, OrganizationMember},
    project::{CreateProject, MissingOrganizationId, ProjectRow},
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
    .unwrap()
    .data;
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
    let issue = make_issue(&pool, &fx, "TST-OF").await;

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
        .unwrap();
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
    let issue = make_issue(&pool, &fx, "TST-OT").await;

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
        .unwrap();
    }

    let listed: Vec<Uuid> = IssueTag::find_by_issue(&pool, issue.id)
        .await
        .unwrap()
        .into_iter()
        .map(|t| t.id)
        .collect();
    assert_eq!(listed, vec![id_c, id_b, id_a]);
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

// === End-to-end txid contract proof ===
//
// Proves the SMS2-784 acceptance criteria at the repository surface:
// - every committed mutation returns a non-zero, strictly-increasing `txid`
// - a rolled-back data write never advances the visible `mutation_log` sequence

#[tokio::test]
async fn mutations_return_sqlite_backed_txids_and_rollbacks_do_not_advance_visible() {
    let pool = make_pool().await;
    let fx = seed(&pool).await;

    // First committed create.
    let first = Issue::create(
        &pool,
        &CreateIssue {
            id: Uuid::new_v4(),
            issue_number: 1,
            simple_id: "TX-1".into(),
            creator_user_id: Some(fx.user.id),
            request: create_issue_request(fx.project.id, fx.status.id),
        },
    )
    .await
    .unwrap();
    assert!(first.txid > 0, "txid must be non-zero, got {}", first.txid);
    let visible_after_first: i64 = sqlx::query_scalar("SELECT MAX(id) FROM mutation_log")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(visible_after_first, first.txid);

    // Second committed create — strictly increasing.
    let second = Issue::create(
        &pool,
        &CreateIssue {
            id: Uuid::new_v4(),
            issue_number: 2,
            simple_id: "TX-2".into(),
            creator_user_id: Some(fx.user.id),
            request: create_issue_request(fx.project.id, fx.status.id),
        },
    )
    .await
    .unwrap();
    assert!(
        second.txid > first.txid,
        "second txid must exceed first: {} <= {}",
        second.txid,
        first.txid,
    );

    // Rolled-back create: FK violation aborts the inner transaction, so the
    // mutation_log row inserted alongside the failing data write is rolled
    // back too. Visible `mutation_log` MAX must NOT advance.
    let pre_rollback_visible: i64 = sqlx::query_scalar("SELECT MAX(id) FROM mutation_log")
        .fetch_one(&pool)
        .await
        .unwrap();
    let rolled_back = Issue::create(
        &pool,
        &CreateIssue {
            id: Uuid::new_v4(),
            issue_number: 3,
            simple_id: "TX-3".into(),
            creator_user_id: Some(fx.user.id),
            // Unknown status_id triggers an FK violation, rolling back the txn.
            request: create_issue_request(fx.project.id, Uuid::new_v4()),
        },
    )
    .await;
    assert!(rolled_back.is_err(), "expected FK violation");
    let post_rollback_visible: i64 = sqlx::query_scalar("SELECT MAX(id) FROM mutation_log")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(
        post_rollback_visible, pre_rollback_visible,
        "rolled-back data write must not advance the visible mutation_log sequence",
    );

    // Update on the second issue — strictly increasing past second.txid.
    let updated = Issue::update(
        &pool,
        second.data.id,
        &UpdateIssueRequest {
            status_id: None,
            title: Some("renamed".into()),
            description: None,
            priority: None,
            start_date: None,
            target_date: None,
            completed_at: None,
            sort_order: None,
            parent_issue_id: None,
            parent_issue_sort_order: None,
            extension_metadata: None,
        },
    )
    .await
    .unwrap();
    assert!(
        updated.txid > second.txid,
        "update txid must exceed prior committed: {} <= {}",
        updated.txid,
        second.txid,
    );

    // Delete on the second issue — strictly increasing past update.
    let deleted = Issue::delete(&pool, second.data.id).await.unwrap();
    assert!(
        deleted.txid > updated.txid,
        "delete txid must exceed prior committed: {} <= {}",
        deleted.txid,
        updated.txid,
    );

    // Visible MAX after all committed mutations equals the last committed txid.
    let final_visible: i64 = sqlx::query_scalar("SELECT MAX(id) FROM mutation_log")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(final_visible, deleted.txid);
}
