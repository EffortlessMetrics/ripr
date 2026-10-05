use super::{dedupe_key, existing_comments, is_code_span, publish_requests};
use serde_json::json;

#[test]
fn dedupe_key_is_the_shortest_key_that_closes_on_its_line() {
    assert_eq!(dedupe_key("x <!-- ripr:dedupe=a:b -->"), Some("a:b"));
    assert_eq!(
        dedupe_key("<!-- ripr:dedupe=a b presentation=compact-v1 -->"),
        Some("a b")
    );
    // The first close wins, as `.*?` does.
    assert_eq!(dedupe_key("<!-- ripr:dedupe=a -->b -->"), Some("a"));
    // An empty presentation tag is part of the key, not a tag.
    assert_eq!(
        dedupe_key("<!-- ripr:dedupe=k presentation= -->"),
        Some("k presentation=")
    );
    // The key cannot cross a line; a later marker can still match.
    assert_eq!(dedupe_key("<!-- ripr:dedupe=a\nb -->"), None);
    assert_eq!(
        dedupe_key("<!-- ripr:dedupe=a\n<!-- ripr:dedupe=b -->"),
        Some("b")
    );
}

#[test]
fn code_span_lines_need_matching_fences_around_content() {
    assert!(is_code_span("`ripr agent verify`"));
    assert!(is_code_span("``a`b``"));
    assert!(is_code_span("`a``b`"));
    assert!(!is_code_span("``"));
    assert!(!is_code_span("``a`"));
    assert!(!is_code_span("`a``"));
    assert!(!is_code_span("plain"));
}

#[test]
fn only_marked_comments_from_the_workflow_bot_are_existing_cards() {
    let bot = json!({"login": "github-actions[bot]", "type": "Bot"});
    let pages = json!([[
        {"id": 1, "user": bot, "path": "a.rs", "line": 3, "position": 1,
         "body": "lead\n\n<details><summary>Full RIPR repair card</summary>\n\ncard\n\n</details>\n\n<!-- ripr:dedupe=k1 presentation=compact-v1 -->"},
        {"id": 2, "user": bot, "path": "b.rs", "original_line": 9,
         "body": "old\n<!-- ripr:dedupe=k2 -->"},
        {"id": 3, "user": {"login": "mallory", "type": "User"},
         "body": "<!-- ripr:dedupe=k3 -->"},
        {"id": 4, "user": bot, "body": "no marker"},
        "not an object"
    ]]);
    let existing = existing_comments(&pages);
    assert_eq!(
        existing["comments"],
        json!([
            {"comment_id": 1, "dedupe_key": "k1", "path": "a.rs", "line": 3,
             "side": "RIGHT", "body": "card", "outdated": false},
            {"comment_id": 2, "dedupe_key": "k2", "path": "b.rs", "line": 9,
             "side": "RIGHT", "body": "__ripr_legacy_presentation__", "outdated": true}
        ])
    );
}

#[test]
fn an_unsafe_plan_publishes_nothing_and_says_why() {
    let plan = json!({
        "summary": {"safe_to_publish": false},
        "blocked": [{"blocked_reason": "no_token", "message": "line\r\nbreak"}]
    });
    let requests = publish_requests(&plan, "7", "abc");
    assert!(requests.requests.is_empty());
    assert_eq!(
        requests.notes,
        vec![
            "RIPR inline comments were not published because the publish plan is not safe."
                .to_string(),
            "- no_token: line  break".to_string(),
        ]
    );
}

#[test]
fn updates_come_before_one_review_that_creates_the_new_cards() {
    let plan = json!({
        "summary": {"safe_to_publish": true, "publishable": 2, "summary_only": 1},
        "operations": [
            {"operation": "update", "safe_to_publish": true, "existing_comment_id": 41,
             "dedupe_key": "k1", "body": "### ripr gap: G\nRepair:\nR\nVerify:\n`v`"},
            {"operation": "create", "safe_to_publish": true, "dedupe_key": "k2",
             "placement": {"path": "src/a.rs", "line": 5}, "body": "card"},
            {"operation": "create", "safe_to_publish": false, "dedupe_key": "k3", "body": "x"},
            {"operation": "keep", "safe_to_publish": true, "dedupe_key": "k4\nx", "body": "y"}
        ]
    });
    let requests = publish_requests(&plan, "7", "abc");
    let calls: Vec<_> = requests
        .requests
        .iter()
        .map(|request| (request.method, request.endpoint.as_str()))
        .collect();
    assert_eq!(
        calls,
        vec![("PATCH", "pulls/comments/41"), ("POST", "pulls/7/reviews")]
    );
    assert_eq!(
        requests.requests[0].payload["body"],
        "**ripr: G** — R\n\nVerify: `v`\n\n<details><summary>Full RIPR repair card</summary>\n\n### ripr gap: G\nRepair:\nR\nVerify:\n`v`\n\n</details>\n\n<!-- ripr:dedupe=k1 presentation=compact-v1 -->"
    );
    let review = &requests.requests[1].payload;
    assert_eq!(review["commit_id"], "abc");
    assert_eq!(review["event"], "COMMENT");
    assert_eq!(review["comments"][0]["path"], "src/a.rs");
    assert_eq!(review["comments"][0]["side"], "RIGHT");
    assert_eq!(review["comments"].as_array().map(Vec::len), Some(1));
    assert_eq!(
        requests.requests[1].message,
        "Created one RIPR review with 1 inline comment(s)."
    );
    assert_eq!(
        requests.notes,
        vec!["RIPR inline comment already current: k4 x".to_string()]
    );
}

#[test]
fn an_update_only_rerun_posts_a_summary_review_only_when_something_is_left_over() {
    let update = json!({"operation": "update", "safe_to_publish": true,
        "existing_comment_id": 9, "dedupe_key": "k", "body": "card"});
    let plan = |summary: serde_json::Value| json!({"summary": summary, "operations": [update]});
    let calls = |plan: &serde_json::Value| {
        publish_requests(plan, "7", "abc")
            .requests
            .iter()
            .map(|request| (request.method, request.endpoint.clone()))
            .collect::<Vec<_>>()
    };

    // Nothing beyond the updated card: no review, so a rerun adds no noise.
    let quiet = plan(json!({"safe_to_publish": true, "publishable": 1}));
    assert_eq!(
        calls(&quiet),
        vec![("PATCH", "pulls/comments/9".to_string())]
    );

    // Leftover recommendations or suppressions still get one summary review.
    for summary in [
        json!({"safe_to_publish": true, "publishable": 1, "summary_only": 2}),
        json!({"safe_to_publish": true, "publishable": 1, "suppressed": 1}),
    ] {
        let plan = plan(summary);
        let requests = publish_requests(&plan, "7", "abc");
        assert_eq!(
            calls(&plan),
            vec![
                ("PATCH", "pulls/comments/9".to_string()),
                ("POST", "pulls/7/reviews".to_string())
            ]
        );
        let review = &requests.requests[1];
        assert_eq!(
            review.message,
            "Created one RIPR review summary after 1 inline comment update(s)."
        );
        assert!(
            review.payload.get("comments").is_none(),
            "{:?}",
            review.payload
        );
    }
    let suppressed = plan(json!({"safe_to_publish": true, "publishable": 1, "suppressed": 1}));
    let body = publish_requests(&suppressed, "7", "abc").requests[1].payload["body"].clone();
    assert_eq!(
        body,
        "RIPR surfaced 1 line-placed recommendation.\n\n1 suppressed recommendation remain visible there with reasons.\n\nAdvisory static evidence only; gate authority remains separate."
    );
}

#[test]
fn a_workflow_bot_login_from_a_user_account_is_not_an_existing_card() {
    let pages = json!([[
        {"id": 1, "user": {"login": "github-actions[bot]", "type": "User"},
         "path": "a.rs", "line": 1, "body": "<!-- ripr:dedupe=k1 -->"},
        // A compact marker whose card cannot be read back.
        {"id": 2, "user": {"login": "github-actions[bot]", "type": "Bot"},
         "path": "a.rs", "line": 2, "body": "lead <!-- ripr:dedupe=k2 presentation=compact-v1 -->"}
    ]]);
    let existing = existing_comments(&pages);
    assert_eq!(
        existing["comments"],
        json!([{"comment_id": 2, "dedupe_key": "k2", "path": "a.rs", "line": 2,
                "side": "RIGHT", "body": "__ripr_compact_presentation_unreadable__",
                "outdated": false}])
    );
}

#[test]
fn the_card_runs_to_the_last_details_close_and_mixed_plans_publish_only_safe_creates() {
    // The retired `(?<card>.*)` with flag `m` was greedy: a card that holds
    // its own `</details>` keeps it.
    let bot = json!({"login": "github-actions[bot]", "type": "Bot"});
    let pages = json!([[{"id": 1, "user": bot, "path": "a.rs", "line": 1,
        "body": "lead\n\n<details><summary>Full RIPR repair card</summary>\n\nouter\n\n</details>\n\ninner\n\n</details>\n\n<!-- ripr:dedupe=k presentation=compact-v1 -->"}]]);
    assert_eq!(
        existing_comments(&pages)["comments"][0]["body"],
        "outer\n\n</details>\n\ninner"
    );

    let plan = json!({
        "summary": {"safe_to_publish": true, "publishable": 2},
        "operations": [
            {"operation": "update", "safe_to_publish": true, "dedupe_key": "no-id", "body": "x"},
            {"operation": "create", "safe_to_publish": false, "dedupe_key": "held",
             "placement": {"path": "a.rs", "line": 1}, "body": "x"},
            {"operation": "create", "safe_to_publish": true, "dedupe_key": "sent",
             "placement": {"path": "a.rs", "line": 2}, "body": "x"}
        ]
    });
    let requests = publish_requests(&plan, "7", "abc");
    assert_eq!(requests.requests.len(), 1);
    let comments = &requests.requests[0].payload["comments"];
    assert_eq!(comments.as_array().map(Vec::len), Some(1));
    assert_eq!(comments[0]["line"], 2);
    assert_eq!(
        requests.notes,
        vec![
            "Skipped a RIPR inline comment update without a numeric comment id: no-id".to_string()
        ]
    );
}

#[test]
fn a_skipped_update_does_not_post_a_summary_review() {
    let plan = json!({
        "summary": {"safe_to_publish": true, "publishable": 1, "summary_only": 1},
        "operations": [{"operation": "update", "safe_to_publish": true, "dedupe_key": "no-id", "body": "x"}]
    });
    let requests = publish_requests(&plan, "7", "abc");
    assert!(requests.requests.is_empty());
    assert_eq!(requests.notes.len(), 1);
}

#[test]
fn a_keep_only_or_empty_plan_sends_nothing() {
    let keep_only = json!({
        "summary": {"safe_to_publish": true, "publishable": 1},
        "operations": [
            {"operation": "keep", "safe_to_publish": true, "dedupe_key": "kept", "body": "x"},
            {"operation": "keep", "safe_to_publish": false, "dedupe_key": "held", "body": "x"}
        ]
    });
    let requests = publish_requests(&keep_only, "7", "abc");
    assert!(requests.requests.is_empty());
    assert_eq!(
        requests.notes,
        vec!["RIPR inline comment already current: kept".to_string()]
    );

    for empty in [
        json!({"summary": {"safe_to_publish": true}, "operations": []}),
        json!({"summary": {"safe_to_publish": true}}),
    ] {
        let requests = publish_requests(&empty, "7", "abc");
        assert!(requests.requests.is_empty(), "{empty}");
        assert!(requests.notes.is_empty(), "{empty}");
    }
}

#[test]
fn only_cap_and_size_skips_count_as_additional_recommendations() {
    let plan = json!({
        "summary": {"safe_to_publish": true, "publishable": 1},
        "operations": [{"operation": "create", "safe_to_publish": true, "dedupe_key": "k",
            "placement": {"path": "a.rs", "line": 1}, "body": "x"}],
        "skipped": [
            {"skip_reason": "inline_comment_cap_reached"},
            {"skip_reason": "comment_body_too_large"},
            {"skip_reason": "outside_diff_hunk"},
            {}
        ]
    });
    let requests = publish_requests(&plan, "7", "abc");
    assert_eq!(requests.requests.len(), 1);
    let body = requests.requests[0].payload["body"]
        .as_str()
        .unwrap_or_default();
    assert!(
        body.contains("\n\n2 additional recommendations remain in the generated"),
        "{body}"
    );
}

#[test]
fn update_messages_and_notes_fold_line_breaks_in_the_key() {
    let plan = json!({
        "summary": {"safe_to_publish": true, "publishable": 2},
        "operations": [
            {"operation": "update", "safe_to_publish": true, "existing_comment_id": 9,
             "dedupe_key": "a\r\n::error::x", "body": "card"},
            {"operation": "update", "safe_to_publish": true,
             "dedupe_key": "b\n::warning::y", "body": "card"}
        ]
    });
    let requests = publish_requests(&plan, "7", "abc");
    assert_eq!(requests.requests.len(), 1);
    assert_eq!(
        requests.requests[0].message,
        "Updated RIPR inline comment: a  ::error::x"
    );
    assert_eq!(
        requests.notes,
        vec![
            "Skipped a RIPR inline comment update without a numeric comment id: b ::warning::y"
                .to_string()
        ]
    );
}
