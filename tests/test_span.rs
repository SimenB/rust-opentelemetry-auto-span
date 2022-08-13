use std::sync::{Arc, Mutex};

use opentelemetry::{global, trace::StatusCode, Key, Value};
use opentelemetry_auto_span::auto_span;
use tracing_test::{TestTracerProvider, TestTracerProviderInner};

const TRACE_NAME: &str = "test_test";

#[auto_span]
fn f(x: i32) -> Result<i32, &'static str> {
    if x < 0 {
        Err("x is negative")
    } else {
        Ok(x * x)
    }
}

#[auto_span]
fn g(x: i32) -> Result<i32, &'static str> {
    Ok(f(x)? + f(-x)?)
}

#[auto_span]
async fn test_sqlx() -> sqlx::Result<()> {
    use sqlx::Connection;
    let mut con = sqlx::sqlite::SqliteConnection::connect(":memory:").await?;
    let _ = sqlx::query("SELECT 1").fetch_one(&mut con).await?;
    Ok(())
}

#[actix_rt::test]
async fn main() {
    // setup
    let inner = Arc::new(Mutex::new(TestTracerProviderInner::new()));
    let provider = TestTracerProvider::new(inner.clone());
    let _ = global::set_tracer_provider(provider);

    // call test target functions
    let _ = g(12);
    assert!(test_sqlx().await.is_ok());

    // check result
    let spans = &inner.lock().unwrap().spans;
    let mut span_iter = spans.iter();
    // g から f を2回呼ぶ。g だけがエラーになる仕様
    assert_eq!(span_iter.next().unwrap().1.name, "fn:f");
    assert_eq!(span_iter.next().unwrap().1.name, "fn:f");
    {
        let data = &span_iter.next().unwrap().1;
        assert_eq!(data.name, "fn:g");
        assert_eq!(data.status_code, StatusCode::Error);
        assert_eq!(data.status_message.to_string(), "x is negative");
    }
    // test_sqlx
    {
        let data = &span_iter.next().unwrap().1;
        assert_eq!(data.name, "db"); // TODO: line 対応したら直す
        assert_eq!(
            data.attributes.get(&Key::new("sql")).unwrap(),
            &Value::from("SELECT 1"),
        );
    }
    {
        let data = &span_iter.next().unwrap().1;
        assert_eq!(data.name, "fn:test_sqlx");
    }
}