/******************************************************************************
 * Copyright ContinuousC. Licensed under the "Elastic License 2.0".           *
 ******************************************************************************/

use thiserror::Error;
use tokio::test;

use rpc::{rpc, RequestHandler};

#[derive(Debug)]
pub struct TestSession;

#[rpc(service(extra_args = "session: TestSession"), stub)]
trait TestService {
    async fn test_method(&self, arg1: String) -> String;
}

struct TestImpl;

impl TestService for TestImpl {
    type Error = Error;
    async fn test_method(
        &self,
        _session: TestSession,
        arg1: String,
    ) -> Result<String, Error> {
        Ok(arg1)
    }
}

#[derive(Error, Debug)]
enum Error {}

#[test]
async fn derive_service() {
    let service = TestHandler::new(TestImpl);
    let result: Result<String, String> = serde_json::from_value(
        service
            .handle(
                serde_json::to_value(TestRequest::TestMethod {
                    arg1: "12345".to_string(),
                })
                .unwrap(),
                (TestSession,),
            )
            .await
            .unwrap(),
    )
    .unwrap();
    assert!(result.as_deref() == Ok("12345"))
}
