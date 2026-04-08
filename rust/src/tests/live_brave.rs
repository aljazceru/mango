/// Live integration test for Brave Search API.
/// Run with: BRAVE_API_KEY=<key> cargo test -p mango_core -- --ignored live_brave
use crate::agent::tools::dispatch_web_search;

#[test]
#[ignore]
fn live_brave_web_search() {
    let key = std::env::var("BRAVE_API_KEY")
        .expect("Set BRAVE_API_KEY env var to run this test");

    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .expect("tokio runtime");

    let args = r#"{"query": "weather in Trento Italy", "count": 3}"#;
    let result = dispatch_web_search(args, &runtime, &key);

    println!("--- Brave Search Result ---");
    println!("{}", result);
    println!("--- End ---");

    assert!(
        !result.starts_with("Error:"),
        "dispatch_web_search returned an error: {}",
        result
    );
    assert!(
        !result.contains("No results found"),
        "Brave search returned no results"
    );
}
