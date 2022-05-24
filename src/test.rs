use std::{future::Future, sync::Arc};

use futures::{Stream, StreamExt};

use crate::{HttpError, HttpExecutor, Respond, Response};

const TEST_URL: &'static str = "127.0.0.1:44444";

fn start_test_server() -> Arc<tiny_http::Server> {
    let server = Arc::new(tiny_http::Server::http(TEST_URL).unwrap());

    {
        let server = server.clone();
        std::thread::spawn(move || {
            for request in server.incoming_requests() {
                let out = serde_json::to_vec(&serde_json::json!({
                    "url": request.url(),
                }))
                .unwrap();

                let res = tiny_http::Response::from_data(out).with_status_code(200);

                request.respond(res).unwrap();
            }
        });
    }
    server
}

pub async fn test_async_executor<E>(exec: E)
where
    E: HttpExecutor,
    E::ResponseBody: Respond + Send + 'static,
    <E::ResponseBody as Respond>::BytesOutput:
        Future<Output = Result<Vec<u8>, HttpError>> + Send + 'static,
    <E::ResponseBody as Respond>::Chunks:
        Stream<Item = Result<Vec<u8>, HttpError>> + Send + 'static,
    E::Output: Future<Output = Result<Response<E::ResponseBody>, HttpError>> + Send + 'static,
    E: Clone,
{
    let server = start_test_server();

    let client = crate::Client::new(exec);

    let url = format!("http://{TEST_URL}/");

    let res = client
        .get(&url)
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap();
    assert_eq!(res.uri().to_string(), url);

    let mut chunks = Box::pin(res.into_body().into_chunks());
    let mut all = Vec::new();
    while let Some(res) = chunks.next().await {
        all.extend(res.unwrap());
    }
    serde_json::from_slice::<serde_json::Value>(&all).unwrap();

    // FIXME: cookie tests

    server.unblock();
}
