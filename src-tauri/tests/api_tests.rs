use httpmock::Method::{GET, POST};
use httpmock::MockServer;
use serde_json::json;

use cloudreve_sync_app::core::cloudreve::{
    finish_sign_in_with_2fa, password_sign_in, refresh_token, CloudreveClient, SignInResult,
};
use cloudreve_sync_app::core::config::ApiPaths;

#[tokio::test]
async fn list_files_calls_expected_endpoint() {
    let server = MockServer::start();
    let mock = server.mock(|when, then| {
        when.method(GET)
            .path("/api/v4/file")
            .query_param("uri", "cloudreve://my/Work");
        then.status(200)
            .header("content-type", "application/json")
            .body(r#"{"code":0,"data":{"files":[],"next_marker":null},"msg":""}"#);
    });

    let api_paths = ApiPaths::default();
    let client = CloudreveClient::new(server.url("/api/v4"), None, api_paths);
    let result = client.list_files("cloudreve://my/Work", Some(1)).await;
    assert!(result.is_ok());
    mock.assert();
}

#[tokio::test]
async fn list_all_files_handles_pagination() {
    let server = MockServer::start();
    let page1 = server.mock(|when, then| {
        when.method(GET)
            .path("/api/v4/file")
            .query_param("uri", "cloudreve://root/Work")
            .query_param("page", "1");
        then.status(200)
            .header("content-type", "application/json")
            .body(r#"{"code":0,"data":{"files":[{"type":0,"id":"f1","name":"a.txt","size":1,"updated_at":"2024-01-01T00:00:00Z","path":"cloudreve://root/Work/a.txt","metadata":{}}],"next_marker":"next"},"msg":""}"#);
    });
    let page2 = server.mock(|when, then| {
        when.method(GET)
            .path("/api/v4/file")
            .query_param("uri", "cloudreve://root/Work")
            .query_param("page", "2");
        then.status(200)
            .header("content-type", "application/json")
            .body(r#"{"code":0,"data":{"files":[{"type":0,"id":"f2","name":"b.txt","size":2,"updated_at":"2024-01-01T00:00:00Z","path":"cloudreve://root/Work/b.txt","metadata":{}}],"next_marker":null},"msg":""}"#);
    });

    let api_paths = ApiPaths::default();
    let client = CloudreveClient::new(server.url("/api/v4"), None, api_paths);
    let result = client.list_all_files("cloudreve://root/Work").await.expect("list");
    assert_eq!(result.len(), 2);
    assert_eq!(result[0].uri, "cloudreve://root/Work/a.txt");
    assert_eq!(result[1].uri, "cloudreve://root/Work/b.txt");
    page1.assert();
    page2.assert();
}

#[tokio::test]
async fn create_download_urls_posts_body() {
    let server = MockServer::start();
    let mock = server.mock(|when, then| {
        when.method(POST)
            .path("/api/v4/file/url")
            .json_body(json!({
                "uris": ["cloudreve://root/Work/a.txt"],
                "download": true
            }));
        then.status(200)
            .header("content-type", "application/json")
            .body(r#"{"code":0,"data":{"urls":[{"url":"https://example.com/a.txt","stream_saver_display_name":null}],"expires":"2024-01-01T00:00:00Z"},"msg":""}"#);
    });

    let api_paths = ApiPaths::default();
    let client = CloudreveClient::new(server.url("/api/v4"), None, api_paths);
    let response = client
        .create_download_urls(vec!["cloudreve://root/Work/a.txt".to_string()], true)
        .await
        .expect("download urls");
    assert_eq!(response.urls.len(), 1);
    assert_eq!(response.urls[0].url, "https://example.com/a.txt");
    mock.assert();
}

#[tokio::test]
async fn list_files_returns_error_on_nonzero_code() {
    let server = MockServer::start();
    let mock = server.mock(|when, then| {
        when.method(GET)
            .path("/api/v4/file")
            .query_param("uri", "cloudreve://my/Work");
        then.status(200)
            .header("content-type", "application/json")
            .body(r#"{"code":203,"data":"error-id","msg":""}"#);
    });

    let api_paths = ApiPaths::default();
    let client = CloudreveClient::new(server.url("/api/v4"), None, api_paths);
    let result = client.list_files("cloudreve://my/Work", Some(1)).await;
    assert!(result.is_err());
    let message = result.err().unwrap().to_string();
    assert!(message.contains("203"));
    mock.assert();
}

#[tokio::test]
async fn password_sign_in_returns_2fa_session() {
    let server = MockServer::start();
    let mock = server.mock(|when, then| {
        when.method(POST).path("/api/v4/session/token");
        then.status(200)
            .header("content-type", "application/json")
            .body(r#"{"code":203,"data":"session-123","msg":""}"#);
    });

    let result = password_sign_in(
        &server.url("/api/v4"),
        "user@example.com",
        "pass",
        None,
        None,
    )
    .await
    .expect("login");
    match result {
        SignInResult::TwoFaRequired(session_id) => assert_eq!(session_id, "session-123"),
        _ => panic!("expected two-fa required"),
    }
    mock.assert();
}

#[tokio::test]
async fn finish_sign_in_with_2fa_returns_tokens() {
    let server = MockServer::start();
    let mock = server.mock(|when, then| {
        when.method(POST)
            .path("/api/v4/session/token/2fa")
            .json_body(json!({
                "opt": "123456",
                "otp": "123456",
                "session_id": "session-abc"
            }));
        then.status(200)
            .header("content-type", "application/json")
            .body(r#"{"code":0,"data":{"token":{"access_token":"access","refresh_token":"refresh","access_expires":"2025-01-01T00:00:00Z","refresh_expires":"2025-02-01T00:00:00Z"}},"msg":""}"#);
    });

    let result = finish_sign_in_with_2fa(&server.url("/api/v4"), "123456", "session-abc")
        .await
        .expect("finish 2fa");
    assert_eq!(result.token.access_token, "access");
    mock.assert();
}

#[tokio::test]
async fn refresh_token_returns_new_pair() {
    let server = MockServer::start();
    let mock = server.mock(|when, then| {
        when.method(POST)
            .path("/api/v4/session/token/refresh")
            .json_body(json!({
                "refresh_token": "refresh-old"
            }));
        then.status(200)
            .header("content-type", "application/json")
            .body(r#"{"code":0,"data":{"access_token":"access-new","refresh_token":"refresh-new","access_expires":"2025-01-01T00:00:00Z","refresh_expires":"2025-02-01T00:00:00Z"},"msg":""}"#);
    });

    let result = refresh_token(&server.url("/api/v4"), "refresh-old")
        .await
        .expect("refresh");
    assert_eq!(result.access_token, "access-new");
    assert_eq!(result.refresh_token, "refresh-new");
    mock.assert();
}
