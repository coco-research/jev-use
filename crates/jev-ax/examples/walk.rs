//! Print one observation of an app as a single JSON object, in the reference's shape.
//!
//! This is the port's side of `scripts/parity --live <app> --port-cmd ...`, which is rule R8's
//! acceptance test: it runs the Python reference and this port on the same app, back to back,
//! and diffs the two tables key by key. The shape here is therefore not a display choice - it
//! is the interface the harness reads:
//!
//! ```text
//! {"app":..., "title":..., "fingerprint":..., "nodes_seen":N, "hidden":N, "truncated":bool,
//!  "actions":[{"id":"n1","node":"1","kind":"click","label":...,"role":"AXButton",
//!              "rect":[x,y,w,h], ...}]}
//! ```
//!
//! `elements_seen` on the reference's side is its own counter, which increments twice per node
//! (`ax.py:272-273`), so the harness expects exactly twice this port's `nodes_seen`. That is
//! why the field is named differently here rather than quietly matching.
//!
//! The extra keys on an element - `value`, `typeable`, `current_value`, `options`, `selected`,
//! `checked` - are the reference's, emitted the way it emits them: only when they carry
//! something (`ax.py:329-350`). JSON is used rather than a hand-rolled string because labels
//! on a real screen contain quotes, backslashes and newlines; a real Finder label is
//! `Tags…`, and a Terminal label is whatever the user typed.
//!
//! ```text
//! cargo run --quiet --example walk -p jev-ax -- --app Finder --json
//! ```
//!
//! Exit codes: 0 observed, 1 the walk failed (the reason is on stderr), 2 bad arguments or
//! the wrong platform. `--json` is currently the only format: a human-readable table is T7's
//! CLI, which will share this observation.

#[cfg(not(target_os = "macos"))]
fn main() {
    eprintln!("this example needs macOS: accessibility is not a thing elsewhere");
    std::process::exit(2);
}

#[cfg(target_os = "macos")]
fn main() {
    std::process::exit(mac::run());
}

#[cfg(target_os = "macos")]
mod mac {
    use jev_ax::walk::observe;
    use jev_ax::{Element, ElementTable, Kind, Limits};
    use serde_json::{json, Map, Value};

    pub fn run() -> i32 {
        let mut app: Option<String> = None;
        let mut args = std::env::args().skip(1);
        while let Some(arg) = args.next() {
            match arg.as_str() {
                // Accepted, and the only format there is. Naming it keeps the documented
                // invocation working and leaves room for `--table` later.
                "--json" => {}
                "--app" => match args.next() {
                    Some(name) => app = Some(name),
                    None => {
                        eprintln!("--app needs an app name");
                        return 2;
                    }
                },
                other => {
                    eprintln!("unknown argument {other:?}; expected: --app <name> --json");
                    return 2;
                }
            }
        }

        // The defaults are the reference's tunables (rule R6), so the harness compares the
        // same walk the Python side performs.
        match observe(app.as_deref(), Limits::default()) {
            Ok(table) => match serde_json::to_string(&table_json(&table)) {
                Ok(line) => {
                    println!("{line}");
                    0
                }
                Err(e) => {
                    // Cannot happen for a table of strings and numbers, and still not worth
                    // panicking over: rule R10 says say what went wrong.
                    eprintln!("could not render the table as JSON: {e}");
                    1
                }
            },
            Err(e) => {
                eprintln!("walk failed: {e}");
                1
            }
        }
    }

    /// The whole observation, key for key.
    fn table_json(table: &ElementTable) -> Value {
        let actions: Vec<Value> = table.elements.iter().map(element_json).collect();
        json!({
            "app": table.app,
            "title": table.title,
            "fingerprint": table.fingerprint(),
            "nodes_seen": table.nodes_seen,
            "hidden": table.hidden,
            "truncated": table.truncated,
            "actions": actions,
        })
    }

    /// One addressable element, in the reference's key order.
    ///
    /// The optional keys are inserted only when they have something to say. `options` is the
    /// exception: a `select` reports it even when empty, because the reference does
    /// (`ax.py:343`) and "this popup has no reachable items" is a real answer.
    fn element_json(element: &Element) -> Value {
        let mut object = Map::new();
        object.insert("id".to_string(), json!(format!("n{}", element.node)));
        object.insert("node".to_string(), json!(element.node.to_string()));
        object.insert("kind".to_string(), json!(kind_name(element.kind)));
        object.insert("label".to_string(), json!(element.label));
        object.insert("role".to_string(), json!(element.role));
        object.insert(
            "rect".to_string(),
            match element.rect {
                Some(r) => json!([r.x, r.y, r.w, r.h]),
                // The reference writes `null` here rather than omitting the key, because its
                // table reuses the rect when executing an index.
                None => Value::Null,
            },
        );
        if let Some(value) = &element.value {
            object.insert("value".to_string(), json!(value));
        }
        if element.typeable() {
            object.insert("typeable".to_string(), json!(true));
        }
        if element.kind == Kind::Select {
            object.insert("options".to_string(), json!(element.options));
            if let Some(current) = &element.current_value {
                object.insert("current_value".to_string(), json!(current));
            }
        }
        if let Some(selected) = element.selected {
            object.insert("selected".to_string(), json!(selected));
        }
        if let Some(checked) = element.checked {
            object.insert("checked".to_string(), json!(checked));
        }
        Value::Object(object)
    }

    /// The kind as the decision layer spells it: `click`, `fill`, `select`.
    fn kind_name(kind: Kind) -> &'static str {
        match kind {
            Kind::Click => "click",
            Kind::Fill => "fill",
            Kind::Select => "select",
        }
    }
}
