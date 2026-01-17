use httpmock::Method::GET;
use httpmock::MockServer;

use cloudreve_sync_app::core::cloudreve::CloudreveClient;
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
