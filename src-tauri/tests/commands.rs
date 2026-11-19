//! Every command the frontend invokes must be registered and get exactly the argument names
//! the backend reads. Tauri only checks this at runtime: a misspelt required argument fails
//! when the user clicks, a misspelt optional one silently arrives as `None`.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use syn::{Expr, FnArg, Item, Lit, Meta, Pat, Type};

/// Parameters Tauri fills in itself instead of taking them from the call's arguments.
const INJECTED: &[&str] = &["State", "AppHandle", "Window", "WebviewWindow", "Webview"];

#[derive(Debug)]
struct Param {
    name: String,
    optional: bool,
}

#[derive(Debug)]
struct Call {
    command: String,
    keys: Vec<String>,
    at: String,
}

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap().to_path_buf()
}

fn files(dir: &Path, ext: &[&str], out: &mut Vec<PathBuf>) {
    for entry in std::fs::read_dir(dir).unwrap().flatten() {
        let path = entry.path();
        if path.is_dir() {
            files(&path, ext, out);
        } else if path.extension().is_some_and(|e| ext.iter().any(|x| e == *x)) {
            out.push(path);
        }
    }
}

fn camel(snake: &str) -> String {
    let mut out = String::new();
    let mut upper = false;
    for c in snake.trim_start_matches("r#").chars() {
        if c == '_' {
            upper = true;
        } else if upper {
            out.extend(c.to_uppercase());
            upper = false;
        } else {
            out.push(c);
        }
    }
    out
}

fn last_segment(ty: &Type) -> Option<String> {
    match ty {
        Type::Path(p) => p.path.segments.last().map(|s| s.ident.to_string()),
        Type::Reference(r) => last_segment(&r.elem),
        _ => None,
    }
}

/// Whether the command keeps snake_case argument names (`#[tauri::command(rename_all = "snake_case")]`).
fn snake_case_args(meta: &Meta) -> bool {
    let Meta::List(list) = meta else { return false };
    let mut snake = false;
    list.parse_nested_meta(|m| {
        if m.path.is_ident("rename_all") {
            let Lit::Str(s) = m.value()?.parse::<Lit>()? else { panic!("rename_all takes a string") };
            match s.value().as_str() {
                "snake_case" => snake = true,
                "camelCase" => {}
                other => panic!("rename_all = {other:?} is not handled here"),
            }
        } else if m.input.peek(syn::Token![=]) {
            let _: Expr = m.value()?.parse()?;
        }
        Ok(())
    })
    .unwrap();
    snake
}

fn is_command(meta: &Meta) -> bool {
    let segs: Vec<String> = meta.path().segments.iter().map(|s| s.ident.to_string()).collect();
    segs == ["tauri", "command"]
}

/// Every `#[tauri::command]` in the app crate with the argument names the frontend must use.
fn backend_commands() -> BTreeMap<String, Vec<Param>> {
    let mut paths = Vec::new();
    files(&root().join("src-tauri/src"), &["rs"], &mut paths);
    let mut out = BTreeMap::new();
    for path in paths {
        let file = syn::parse_file(&std::fs::read_to_string(&path).unwrap())
            .unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        for item in file.items {
            let Item::Fn(f) = item else { continue };
            let Some(attr) = f.attrs.iter().find(|a| is_command(&a.meta)) else { continue };
            let snake = snake_case_args(&attr.meta);
            let params = f
                .sig
                .inputs
                .iter()
                .filter_map(|arg| {
                    let FnArg::Typed(t) = arg else { return None };
                    let Pat::Ident(name) = &*t.pat else { panic!("{}: unnamed argument", f.sig.ident) };
                    let ty = last_segment(&t.ty);
                    if ty.as_deref().is_some_and(|ty| INJECTED.contains(&ty)) {
                        return None;
                    }
                    let name = name.ident.to_string();
                    Some(Param {
                        name: if snake { name } else { camel(&name) },
                        optional: ty.as_deref() == Some("Option"),
                    })
                })
                .collect();
            let previous = out.insert(f.sig.ident.to_string(), params);
            assert!(previous.is_none(), "command {} is defined twice", f.sig.ident);
        }
    }
    out
}

/// Command names in `generate_handler![…]`, by their last path segment.
fn registered() -> BTreeSet<String> {
    let lib = std::fs::read_to_string(root().join("src-tauri/src/lib.rs")).unwrap();
    let start = lib.find("generate_handler![").expect("generate_handler! in lib.rs") + "generate_handler![".len();
    let len = lib[start..].find(']').unwrap();
    lib[start..start + len]
        .split(',')
        .map(|p| p.trim().rsplit("::").next().unwrap().to_string())
        .filter(|p| !p.is_empty())
        .collect()
}

fn is_ident(c: u8) -> bool {
    c.is_ascii_alphanumeric() || c == b'_' || c == b'$'
}

fn skip_ws(s: &[u8], mut i: usize) -> usize {
    while i < s.len() && s[i].is_ascii_whitespace() {
        i += 1;
    }
    i
}

/// Index after the bracket that closes the one at `open`, skipping strings.
fn matching(s: &[u8], open: usize) -> usize {
    let mut depth = 0;
    let mut i = open;
    while i < s.len() {
        match s[i] {
            b'"' | b'\'' | b'`' => {
                let q = s[i];
                i += 1;
                while s[i] != q {
                    i += if s[i] == b'\\' { 2 } else { 1 };
                }
            }
            b'(' | b'[' | b'{' => depth += 1,
            b')' | b']' | b'}' => {
                depth -= 1;
                if depth == 0 {
                    return i + 1;
                }
            }
            _ => {}
        }
        i += 1;
    }
    panic!("unbalanced bracket");
}

/// Top-level keys of the object literal whose `{` is at `open`.
fn object_keys(s: &[u8], open: usize, at: &str) -> Vec<String> {
    let end = matching(s, open) - 1;
    let mut keys = Vec::new();
    let mut part = open + 1;
    let mut i = open + 1;
    while i <= end {
        if i == end || s[i] == b',' {
            let text = std::str::from_utf8(&s[part..i]).unwrap().trim();
            if !text.is_empty() {
                assert!(!text.starts_with("..."), "{at}: spread arguments cannot be checked");
                let key = text.split(':').next().unwrap().trim().trim_matches(['"', '\'']);
                keys.push(key.to_string());
            }
            part = i + 1;
            i += 1;
        } else if matches!(s[i], b'(' | b'[' | b'{' | b'"' | b'\'' | b'`') {
            i = if matches!(s[i], b'"' | b'\'' | b'`') { string_end(s, i) } else { matching(s, i) };
        } else {
            i += 1;
        }
    }
    keys
}

fn string_end(s: &[u8], open: usize) -> usize {
    let mut i = open + 1;
    while s[i] != s[open] {
        i += if s[i] == b'\\' { 2 } else { 1 };
    }
    i + 1
}

/// `invoke("cmd", { … })` and `call("cmd", { … }, fallback)` with a literal command name.
fn frontend_calls() -> Vec<Call> {
    let mut paths = Vec::new();
    files(&root().join("src"), &["ts", "svelte"], &mut paths);
    let mut calls = Vec::new();
    for path in paths {
        let text = std::fs::read_to_string(&path).unwrap();
        let s = text.as_bytes();
        for word in ["invoke", "call"] {
            for (pos, _) in text.match_indices(word) {
                if pos > 0 && (is_ident(s[pos - 1]) || s[pos - 1] == b'.') {
                    continue;
                }
                let mut i = pos + word.len();
                if i < s.len() && is_ident(s[i]) {
                    continue;
                }
                if s.get(i) == Some(&b'<') {
                    let mut depth = 0;
                    while i < s.len() {
                        match s[i] {
                            b'<' => depth += 1,
                            b'>' if s[i - 1] != b'=' => {
                                depth -= 1;
                                if depth == 0 {
                                    break;
                                }
                            }
                            _ => {}
                        }
                        i += 1;
                    }
                    i += 1;
                }
                if s.get(i) != Some(&b'(') {
                    continue;
                }
                i = skip_ws(s, i + 1);
                if !matches!(s.get(i), Some(b'"' | b'\'')) {
                    continue;
                }
                let end = string_end(s, i);
                let command = std::str::from_utf8(&s[i + 1..end - 1]).unwrap().to_string();
                if command.contains(':') {
                    continue; // a plugin's command
                }
                let line = text[..pos].matches('\n').count() + 1;
                let at = format!("{}:{line}", path.strip_prefix(root()).unwrap().display());
                i = skip_ws(s, end);
                let keys = match s.get(i) {
                    Some(b')') => Vec::new(),
                    Some(b',') => {
                        i = skip_ws(s, i + 1);
                        match s.get(i) {
                            Some(b'{') => object_keys(s, i, &at),
                            Some(b')') => Vec::new(),
                            _ => panic!("{at}: arguments of {command} must be an object literal"),
                        }
                    }
                    _ => panic!("{at}: cannot read the call of {command}"),
                };
                calls.push(Call { command, keys, at });
            }
        }
    }
    calls
}

fn check(calls: &[Call], backend: &BTreeMap<String, Vec<Param>>, registered: &BTreeSet<String>) -> Vec<String> {
    let mut problems = Vec::new();
    for call in calls {
        let Some(params) = backend.get(&call.command) else {
            problems.push(format!("{}: no command {}", call.at, call.command));
            continue;
        };
        if !registered.contains(&call.command) {
            problems.push(format!("{}: {} is not in generate_handler!", call.at, call.command));
        }
        for key in &call.keys {
            if !params.iter().any(|p| &p.name == key) {
                let expected: Vec<&str> = params.iter().map(|p| p.name.as_str()).collect();
                problems.push(format!("{}: {} takes no argument {key} (it takes {expected:?})", call.at, call.command));
            }
        }
        for p in params.iter().filter(|p| !p.optional) {
            if !call.keys.contains(&p.name) {
                problems.push(format!("{}: {} is missing argument {}", call.at, call.command, p.name));
            }
        }
    }
    problems
}

#[test]
fn frontend_calls_match_the_registered_commands() {
    let backend = backend_commands();
    let registered = registered();
    let calls = frontend_calls();
    for name in &registered {
        assert!(backend.contains_key(name), "generate_handler! lists {name}, but no #[tauri::command] has that name");
    }
    // Guards the scanner itself: it must keep finding the calls in api.ts.
    assert!(calls.iter().any(|c| c.command == "pushfold_chart"), "the scanner misses calls");
    assert!(calls.len() >= 60, "only {} calls found", calls.len());
    let problems = check(&calls, &backend, &registered);
    assert!(problems.is_empty(), "frontend and commands disagree:\n{}", problems.join("\n"));
}

#[test]
fn a_misspelt_argument_is_reported() {
    let backend = BTreeMap::from([(
        "pushfold_chart".to_string(),
        vec![
            Param { name: "ante".into(), optional: false },
            Param { name: "stackBb".into(), optional: false },
            Param { name: "seed".into(), optional: true },
        ],
    )]);
    let registered = BTreeSet::from(["pushfold_chart".to_string()]);
    let call = |keys: &[&str]| Call {
        command: "pushfold_chart".into(),
        keys: keys.iter().map(|k| k.to_string()).collect(),
        at: "api.ts:1".into(),
    };
    assert!(check(&[call(&["ante", "stackBb"])], &backend, &registered).is_empty());
    assert_eq!(check(&[call(&["ante", "stack_bb"])], &backend, &registered).len(), 2);
    assert_eq!(check(&[call(&["ante", "stackBb", "sead"])], &backend, &registered).len(), 1);
}

#[test]
fn object_keys_skip_nested_values() {
    let src = br#"{ spot, settings: { a: 1, b: [2, 3] }, "run": f(x, y), }"#;
    assert_eq!(object_keys(src, 0, "t"), ["spot", "settings", "run"]);
}

/// Calls that refuse or empty a command while PokerStars runs (`src-tauri/src/lib.rs`).
const GATES: &[&str] = &["blocking", "unlocked", "locked"];

/// Commands that answer while PokerStars runs. Each of them returns nothing that rates a play
/// against a model, an equilibrium or a solver, or recommends one: PokerStars allows own
/// stats, HUD values and the hand history only. A new command that answers while the client
/// runs fails this test until it is listed here or goes through a gate.
const OPEN_WHILE_PLAYING: &[&str] = &[
    // HUD and tables.
    "layout_for",
    "set_hit_regions",
    "save_seat",
    "reset_layout",
    "client_windows",
    "poker_client_running",
    "poker_client_name",
    // Own results, stats and hand history.
    "reports",
    "positions",
    "tournaments",
    "shown_hands",
    "player_note",
    "notes_sources",
    "hand",
    "range_groups",
    // Settings and switches.
    "get_settings",
    "save_settings",
    "export_profile",
    "import_profile",
    "reset_profile",
    "import_status",
    "import_hand_files",
    "detected_dirs",
    "analysis_set_auto",
    "library_set_mode",
    "library_set_power",
    "coach_status",
    "coach_set_key",
    "coach_test_key",
    "coach_delete_key",
    "coach_models",
    "coach_set_model",
    "coach_set_effort",
    "cloud_settings",
    "cloud_save_settings",
    "cloud_ack_notice",
    "cloud_malformed",
    "cloud_dismiss_malformed",
    // Stopping and clearing.
    "preflop_cancel",
    "solver_page",
    "solver_cancel",
    "solver_clear",
    "play_pool_cancel",
    "coach_cancel",
    "jobs_list",
    "jobs_cancel",
    "jobs_clear",
    "play_away",
];

/// Every function of the app crate with the functions it calls. Methods are left out: the gates
/// are functions, and `db.hands()` is not the command `hands`.
fn calls_by_function() -> BTreeMap<String, BTreeSet<String>> {
    use syn::visit::Visit;

    #[derive(Default)]
    struct Calls(BTreeSet<String>);
    impl<'a> Visit<'a> for Calls {
        fn visit_expr_call(&mut self, c: &'a syn::ExprCall) {
            if let Expr::Path(p) = &*c.func
                && let Some(last) = p.path.segments.last()
            {
                self.0.insert(last.ident.to_string());
            }
            syn::visit::visit_expr_call(self, c);
        }
    }

    let mut paths = Vec::new();
    files(&root().join("src-tauri/src"), &["rs"], &mut paths);
    let mut out: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for path in paths {
        let file = syn::parse_file(&std::fs::read_to_string(&path).unwrap()).unwrap();
        for item in file.items {
            if let Item::Fn(f) = item {
                let mut calls = Calls::default();
                calls.visit_block(&f.block);
                // Same names in several modules merge: a gate in either counts for both.
                out.entry(f.sig.ident.to_string()).or_default().extend(calls.0);
            }
        }
    }
    out
}

/// Functions that pass a gate, directly or through functions they call.
fn gated(calls: &BTreeMap<String, BTreeSet<String>>) -> BTreeSet<String> {
    let mut gated: BTreeSet<String> = GATES.iter().map(|g| g.to_string()).collect();
    loop {
        let more: Vec<String> = calls
            .iter()
            .filter(|(name, called)| !gated.contains(*name) && called.iter().any(|c| gated.contains(c)))
            .map(|(name, _)| name.clone())
            .collect();
        if more.is_empty() {
            return gated;
        }
        gated.extend(more);
    }
}

#[test]
fn only_listed_commands_answer_while_pokerstars_runs() {
    let calls = calls_by_function();
    let gated = gated(&calls);
    assert!(gated.contains("analysis_summary") && gated.contains("tournament_report"), "the scan misses gates");
    let registered = registered();
    let open: BTreeSet<&str> = registered.iter().filter(|c| !gated.contains(*c)).map(|c| c.as_str()).collect();
    let listed: BTreeSet<&str> = OPEN_WHILE_PLAYING.iter().copied().collect();
    let unlisted: Vec<_> = open.difference(&listed).collect();
    assert!(
        unlisted.is_empty(),
        "these commands answer while PokerStars runs: gate them with unlocked()/blocking(), or, if they give no advice, list them in OPEN_WHILE_PLAYING: {unlisted:?}"
    );
    let stale: Vec<_> = listed.difference(&open).collect();
    assert!(stale.is_empty(), "OPEN_WHILE_PLAYING lists commands that are gated or gone: {stale:?}");
}
