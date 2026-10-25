use std::collections::BTreeMap;
use std::path::PathBuf;

use deck_core::tools::{self, ToolId, ToolRequest};
use deck_core::Category;

fn main() {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    let id = ToolId::from_id(&args[0]).expect("tool");
    let category = Category::of(&PathBuf::from(&args[1])).expect("category");
    let sources = args[1..]
        .iter()
        .filter(|arg| !arg.starts_with("--"))
        .map(PathBuf::from)
        .collect::<Vec<_>>();
    let mut values = BTreeMap::new();
    for arg in &args[1..] {
        if let Some(pair) = arg.strip_prefix("--") {
            let (key, value) = pair.split_once('=').unwrap_or((pair, ""));
            values.insert(key.to_owned(), value.to_owned());
        }
    }
    println!(
        "tool={} category={category:?} fields={:?}",
        id.id(),
        tools::fields(id, category)
    );
    println!(
        "availability={:?}",
        tools::availability(id, category, &sources)
    );
    let outcome = tools::execute(&ToolRequest {
        id: Some(id),
        sources,
        values,
    });
    println!("created={:?}", outcome.created);
    println!("failures={:?}", outcome.failures);
    println!("message={:?}", outcome.message);
}
