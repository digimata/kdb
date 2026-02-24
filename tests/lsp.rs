use serde_json::{json, Value};
use std::fs;
use std::io::{self, BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::{mpsc, Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};
use tempfile::{tempdir, TempDir};
use tower_lsp::lsp_types::Url;

// ---------------------------------------------------------------------
// ## Index
//
// fn write_file()                                                   L45
// fn bin()                                                          L53
// struct VaultFixture                                               L57
//   fn VaultFixture::new()                                          L66
// struct LspSession                                                L102
//   fn LspSession::start()                                         L111
//   fn LspSession::initialize_with_capabilities()                  L146
//   fn LspSession::initialize()                                    L169
//   fn LspSession::send()                                          L173
//   fn LspSession::wait_for_id()                                   L180
//   fn LspSession::wait_for()                                      L187
//   fn LspSession::shutdown()                                      L225
//   fn LspSession::stderr_snapshot()                               L256
//   fn LspSession::drop()                                          L262
// fn read_stdout_loop()                                            L270
// fn read_message()                                                L285
// fn diagnostics_for_uri()                                         L324
// fn initialize_advertises_expected_capabilities()                 L334
// fn initialize_registers_markdown_watcher_when_supported()        L357
// fn symbols_definition_completion_and_hover_work()                L406
// fn diagnostics_publish_on_open_change_and_close()                L535
// fn watched_file_events_refresh_cached_index_and_diagnostics()    L604
// fn goto_definition_resolves_wikilink_targets()                   L691
// fn completion_uses_unsaved_document_buffer_state()               L714
// fn completion_includes_unsaved_open_file_from_cached_index()     L760
// fn heading_completion_reverts_to_disk_after_target_close()       L804
// fn hover_on_nonexistent_target_returns_none()                    L891
// fn diagnostics_include_missing_heading_anchor_errors()           L926
// ---------------------------------------------------------------------

fn write_file(root: &Path, rel_path: &str, content: &str) {
    let path = root.join(rel_path);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("create parent dirs");
    }
    fs::write(path, content).expect("write fixture file");
}

fn bin() -> &'static str {
    env!("CARGO_BIN_EXE_kdb")
}

struct VaultFixture {
    _temp: TempDir,
    root: PathBuf,
    a_uri: Url,
    b_uri: Url,
    scratch_uri: Url,
}

impl VaultFixture {
    fn new() -> Self {
        let temp = tempdir().expect("tempdir");
        let root_path = temp.path().to_path_buf();

        write_file(
            &root_path,
            ".kdb/config.toml",
            "[project]\nname = \"fixture\"\n",
        );
        write_file(
            &root_path,
            "a.md",
            "# A\n\n## Details\n\nSee [B](b.md#target)\nSee [[b#target]]\n",
        );
        write_file(
            &root_path,
            "b.md",
            "# B\n\n## Target\nHello from target section.\n",
        );

        let root = root_path.canonicalize().expect("canonicalize root");

        let a_uri = Url::from_file_path(root.join("a.md")).expect("a uri");
        let b_uri = Url::from_file_path(root.join("b.md")).expect("b uri");
        let scratch_uri = Url::from_file_path(root.join("scratch.md")).expect("scratch uri");

        Self {
            _temp: temp,
            root,
            a_uri,
            b_uri,
            scratch_uri,
        }
    }
}

struct LspSession {
    child: Child,
    stdin: ChildStdin,
    rx: mpsc::Receiver<Value>,
    buffered: Vec<Value>,
    stderr: Arc<Mutex<String>>,
}

impl LspSession {
    fn start(root: &Path) -> Self {
        let mut child = Command::new(bin())
            .arg("lsp")
            .arg(root)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("spawn kdb lsp");

        let stdin = child.stdin.take().expect("child stdin");
        let stdout = child.stdout.take().expect("child stdout");
        let stderr = child.stderr.take().expect("child stderr");

        let (tx, rx) = mpsc::channel();
        thread::spawn(move || read_stdout_loop(stdout, tx));

        let stderr_buf = Arc::new(Mutex::new(String::new()));
        let stderr_clone = Arc::clone(&stderr_buf);
        thread::spawn(move || {
            let mut reader = BufReader::new(stderr);
            let mut out = String::new();
            let _ = reader.read_to_string(&mut out);
            *stderr_clone.lock().expect("stderr lock") = out;
        });

        Self {
            child,
            stdin,
            rx,
            buffered: Vec::new(),
            stderr: stderr_buf,
        }
    }

    fn initialize_with_capabilities(&mut self, root: &Path, capabilities: Value) -> Value {
        let root_uri = Url::from_file_path(root).expect("root uri");
        self.send(json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize",
            "params": {
                "processId": Value::Null,
                "rootUri": root_uri,
                "capabilities": capabilities
            }
        }));

        let response = self.wait_for_id(1, Duration::from_secs(5));
        self.send(json!({
            "jsonrpc": "2.0",
            "method": "initialized",
            "params": {}
        }));

        response
    }

    fn initialize(&mut self, root: &Path) -> Value {
        self.initialize_with_capabilities(root, json!({}))
    }

    fn send(&mut self, message: Value) {
        let body = message.to_string();
        write!(self.stdin, "Content-Length: {}\r\n\r\n{}", body.len(), body)
            .expect("write message");
        self.stdin.flush().expect("flush stdin");
    }

    fn wait_for_id(&mut self, id: i64, timeout: Duration) -> Value {
        self.wait_for(timeout, |msg| {
            msg.get("id").and_then(Value::as_i64) == Some(id)
                || msg.get("id").and_then(Value::as_u64) == Some(id as u64)
        })
    }

    fn wait_for<F>(&mut self, timeout: Duration, mut predicate: F) -> Value
    where
        F: FnMut(&Value) -> bool,
    {
        if let Some(index) = self.buffered.iter().position(&mut predicate) {
            return self.buffered.remove(index);
        }

        let deadline = Instant::now() + timeout;
        loop {
            let now = Instant::now();
            if now >= deadline {
                panic!(
                    "timed out waiting for LSP message; stderr: {}",
                    self.stderr_snapshot()
                );
            }

            let remaining = deadline.saturating_duration_since(now);
            let next_timeout = remaining.min(Duration::from_millis(200));
            match self.rx.recv_timeout(next_timeout) {
                Ok(message) => {
                    if predicate(&message) {
                        return message;
                    }
                    self.buffered.push(message);
                }
                Err(mpsc::RecvTimeoutError::Timeout) => continue,
                Err(mpsc::RecvTimeoutError::Disconnected) => {
                    panic!(
                        "LSP output channel disconnected; stderr: {}",
                        self.stderr_snapshot()
                    )
                }
            }
        }
    }

    fn shutdown(&mut self) {
        self.send(json!({
            "jsonrpc": "2.0",
            "id": 999,
            "method": "shutdown",
            "params": Value::Null
        }));
        let _ = self.wait_for_id(999, Duration::from_secs(5));

        self.send(json!({
            "jsonrpc": "2.0",
            "method": "exit",
            "params": {}
        }));

        let deadline = Instant::now() + Duration::from_secs(3);
        loop {
            match self.child.try_wait() {
                Ok(Some(_)) => break,
                Ok(None) if Instant::now() < deadline => {
                    thread::sleep(Duration::from_millis(50));
                }
                _ => {
                    let _ = self.child.kill();
                    let _ = self.child.wait();
                    break;
                }
            }
        }
    }

    fn stderr_snapshot(&self) -> String {
        self.stderr.lock().expect("stderr lock").trim().to_string()
    }
}

impl Drop for LspSession {
    fn drop(&mut self) {
        if let Ok(None) = self.child.try_wait() {
            let _ = self.child.kill();
            let _ = self.child.wait();
        }
    }
}

fn read_stdout_loop(stdout: ChildStdout, tx: mpsc::Sender<Value>) {
    let mut reader = BufReader::new(stdout);
    loop {
        match read_message(&mut reader) {
            Ok(Some(message)) => {
                if tx.send(message).is_err() {
                    break;
                }
            }
            Ok(None) => break,
            Err(_) => break,
        }
    }
}

fn read_message(reader: &mut BufReader<ChildStdout>) -> io::Result<Option<Value>> {
    let mut content_length = None;

    loop {
        let mut line = String::new();
        let read = reader.read_line(&mut line)?;
        if read == 0 {
            return Ok(None);
        }

        let header = line.trim_end_matches(['\r', '\n']);
        if header.is_empty() {
            break;
        }

        if let Some(raw_len) = header.strip_prefix("Content-Length:") {
            let parsed = raw_len.trim().parse::<usize>().map_err(|error| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("invalid length: {error}"),
                )
            })?;
            content_length = Some(parsed);
        }
    }

    let Some(length) = content_length else {
        return Ok(None);
    };

    let mut body = vec![0_u8; length];
    reader.read_exact(&mut body)?;
    let message = serde_json::from_slice::<Value>(&body).map_err(|error| {
        io::Error::new(io::ErrorKind::InvalidData, format!("invalid json: {error}"))
    })?;

    Ok(Some(message))
}

fn diagnostics_for_uri(message: &Value, uri: &Url) -> bool {
    message.get("method").and_then(Value::as_str) == Some("textDocument/publishDiagnostics")
        && message
            .get("params")
            .and_then(|params| params.get("uri"))
            .and_then(Value::as_str)
            == Some(uri.as_str())
}

#[test]
fn initialize_advertises_expected_capabilities() {
    let fixture = VaultFixture::new();
    let mut session = LspSession::start(&fixture.root);
    let init = session.initialize(&fixture.root);

    let caps = &init["result"]["capabilities"];
    assert_eq!(caps["documentSymbolProvider"], json!(true));
    assert_eq!(caps["definitionProvider"], json!(true));
    assert_eq!(caps["hoverProvider"], json!(true));
    assert_eq!(caps["textDocumentSync"]["openClose"], json!(true));
    assert_eq!(caps["textDocumentSync"]["change"], json!(1));

    let triggers = caps["completionProvider"]["triggerCharacters"]
        .as_array()
        .expect("trigger characters array");
    assert!(triggers.contains(&json!("[")));
    assert!(triggers.contains(&json!("(")));
    assert!(triggers.contains(&json!("#")));

    session.shutdown();
}

#[test]
fn initialize_registers_markdown_watcher_when_supported() {
    let fixture = VaultFixture::new();
    let mut session = LspSession::start(&fixture.root);
    let _ = session.initialize_with_capabilities(
        &fixture.root,
        json!({
            "workspace": {
                "didChangeWatchedFiles": {
                    "dynamicRegistration": true
                }
            }
        }),
    );

    let register_request = session.wait_for(Duration::from_secs(5), |message| {
        message.get("method").and_then(Value::as_str) == Some("client/registerCapability")
    });

    let registrations = register_request["params"]["registrations"]
        .as_array()
        .expect("registrations array");
    assert_eq!(registrations.len(), 1);
    assert_eq!(
        registrations[0]["method"],
        json!("workspace/didChangeWatchedFiles")
    );
    assert_eq!(
        registrations[0]["registerOptions"]["watchers"][0]["globPattern"],
        json!("**/*.md")
    );
    assert_eq!(
        registrations[0]["registerOptions"]["watchers"][0]["kind"],
        json!(7)
    );

    let register_id = register_request
        .get("id")
        .cloned()
        .expect("register capability id");
    session.send(json!({
        "jsonrpc": "2.0",
        "id": register_id,
        "result": Value::Null
    }));

    session.shutdown();
}

#[test]
fn symbols_definition_completion_and_hover_work() {
    let fixture = VaultFixture::new();
    let mut session = LspSession::start(&fixture.root);
    session.initialize(&fixture.root);

    session.send(json!({
        "jsonrpc": "2.0",
        "id": 2,
        "method": "textDocument/documentSymbol",
        "params": {
            "textDocument": { "uri": fixture.a_uri }
        }
    }));
    let symbols_response = session.wait_for_id(2, Duration::from_secs(5));
    let symbols = symbols_response["result"]
        .as_array()
        .expect("symbols array");
    assert_eq!(symbols[0]["name"], json!("A"));
    assert_eq!(symbols[0]["children"][0]["name"], json!("Details"));

    session.send(json!({
        "jsonrpc": "2.0",
        "id": 3,
        "method": "textDocument/definition",
        "params": {
            "textDocument": { "uri": fixture.a_uri },
            "position": { "line": 4, "character": 10 }
        }
    }));
    let definition_response = session.wait_for_id(3, Duration::from_secs(5));
    assert_eq!(
        definition_response["result"]["uri"],
        json!(fixture.b_uri.as_str())
    );
    assert_eq!(
        definition_response["result"]["range"]["start"]["line"],
        json!(2)
    );

    session.send(json!({
        "jsonrpc": "2.0",
        "id": 4,
        "method": "textDocument/hover",
        "params": {
            "textDocument": { "uri": fixture.a_uri },
            "position": { "line": 4, "character": 10 }
        }
    }));
    let hover_response = session.wait_for_id(4, Duration::from_secs(5));
    let hover_value = hover_response["result"]["contents"]["value"]
        .as_str()
        .expect("hover markdown value");
    assert!(hover_value.contains("b.md#target"));
    assert!(hover_value.contains("## Target"));

    session.send(json!({
        "jsonrpc": "2.0",
        "method": "textDocument/didOpen",
        "params": {
            "textDocument": {
                "uri": fixture.scratch_uri,
                "languageId": "markdown",
                "version": 1,
                "text": "# Scratch\n\nSee [[\n"
            }
        }
    }));
    let _ = session.wait_for(Duration::from_secs(5), |message| {
        diagnostics_for_uri(message, &fixture.scratch_uri)
    });

    session.send(json!({
        "jsonrpc": "2.0",
        "id": 5,
        "method": "textDocument/completion",
        "params": {
            "textDocument": { "uri": fixture.scratch_uri },
            "position": { "line": 2, "character": 6 }
        }
    }));
    let file_completion = session.wait_for_id(5, Duration::from_secs(5));
    let labels = file_completion["result"]
        .as_array()
        .expect("completion array")
        .iter()
        .filter_map(|item| item.get("label").and_then(Value::as_str))
        .collect::<Vec<_>>();
    assert!(labels.contains(&"b"));

    session.send(json!({
        "jsonrpc": "2.0",
        "method": "textDocument/didChange",
        "params": {
            "textDocument": { "uri": fixture.scratch_uri, "version": 2 },
            "contentChanges": [
                { "text": "# Scratch\n\nSee [[b#\n" }
            ]
        }
    }));

    session.send(json!({
        "jsonrpc": "2.0",
        "id": 6,
        "method": "textDocument/completion",
        "params": {
            "textDocument": { "uri": fixture.scratch_uri },
            "position": { "line": 2, "character": 8 }
        }
    }));
    let heading_completion = session.wait_for_id(6, Duration::from_secs(5));
    let heading_items = heading_completion["result"]
        .as_array()
        .expect("heading completion array");
    let heading_labels = heading_items
        .iter()
        .filter_map(|item| item.get("label").and_then(Value::as_str))
        .collect::<Vec<_>>();
    assert!(heading_labels.contains(&"Target"));

    let target_heading = heading_items
        .iter()
        .find(|item| item.get("label").and_then(Value::as_str) == Some("Target"))
        .expect("target heading completion");
    assert_eq!(target_heading["insertText"], json!("target"));

    session.shutdown();
}

#[test]
fn diagnostics_publish_on_open_change_and_close() {
    let fixture = VaultFixture::new();
    let mut session = LspSession::start(&fixture.root);
    session.initialize(&fixture.root);

    session.send(json!({
        "jsonrpc": "2.0",
        "method": "textDocument/didOpen",
        "params": {
            "textDocument": {
                "uri": fixture.scratch_uri,
                "languageId": "markdown",
                "version": 1,
                "text": "# Scratch\n\n[bad](missing.md)\n"
            }
        }
    }));

    let open_diag = session.wait_for(Duration::from_secs(5), |message| {
        diagnostics_for_uri(message, &fixture.scratch_uri)
    });
    let open_diags = open_diag["params"]["diagnostics"]
        .as_array()
        .expect("diagnostics array on open");
    assert!(!open_diags.is_empty());
    assert!(open_diags[0]["message"]
        .as_str()
        .expect("diagnostic message")
        .contains("target file not found"));

    session.send(json!({
        "jsonrpc": "2.0",
        "method": "textDocument/didChange",
        "params": {
            "textDocument": { "uri": fixture.scratch_uri, "version": 2 },
            "contentChanges": [
                { "text": "# Scratch\n\n[ok](b.md#target)\n" }
            ]
        }
    }));

    let change_diag = session.wait_for(Duration::from_secs(5), |message| {
        diagnostics_for_uri(message, &fixture.scratch_uri)
    });
    let change_diags = change_diag["params"]["diagnostics"]
        .as_array()
        .expect("diagnostics array on change");
    assert!(change_diags.is_empty());

    session.send(json!({
        "jsonrpc": "2.0",
        "method": "textDocument/didClose",
        "params": {
            "textDocument": { "uri": fixture.scratch_uri }
        }
    }));

    let close_diag = session.wait_for(Duration::from_secs(5), |message| {
        diagnostics_for_uri(message, &fixture.scratch_uri)
    });
    let close_diags = close_diag["params"]["diagnostics"]
        .as_array()
        .expect("diagnostics array on close");
    assert!(close_diags.is_empty());

    session.shutdown();
}

#[test]
fn watched_file_events_refresh_cached_index_and_diagnostics() {
    let fixture = VaultFixture::new();
    let mut session = LspSession::start(&fixture.root);
    session.initialize(&fixture.root);

    let a_text = fs::read_to_string(fixture.root.join("a.md")).expect("read a.md");
    session.send(json!({
        "jsonrpc": "2.0",
        "method": "textDocument/didOpen",
        "params": {
            "textDocument": {
                "uri": fixture.a_uri,
                "languageId": "markdown",
                "version": 1,
                "text": a_text
            }
        }
    }));

    let initial_diag = session.wait_for(Duration::from_secs(5), |message| {
        diagnostics_for_uri(message, &fixture.a_uri)
    });
    let initial_diags = initial_diag["params"]["diagnostics"]
        .as_array()
        .expect("diagnostics array after open");
    assert!(initial_diags.is_empty());

    let moved_dir = fixture.root.join("archived");
    fs::create_dir_all(&moved_dir).expect("create archived directory");
    let moved_b_path = moved_dir.join("b.md");
    fs::rename(fixture.root.join("b.md"), &moved_b_path).expect("move b.md");
    let moved_b_uri = Url::from_file_path(&moved_b_path).expect("moved b.md uri");

    session.send(json!({
        "jsonrpc": "2.0",
        "method": "workspace/didChangeWatchedFiles",
        "params": {
            "changes": [
                {
                    "uri": fixture.b_uri,
                    "type": 3
                },
                {
                    "uri": moved_b_uri,
                    "type": 1
                }
            ]
        }
    }));

    let stale_diag = session.wait_for(Duration::from_secs(5), |message| {
        diagnostics_for_uri(message, &fixture.a_uri)
    });
    let stale_diags = stale_diag["params"]["diagnostics"]
        .as_array()
        .expect("diagnostics array after move");
    assert!(!stale_diags.is_empty());
    assert!(stale_diags[0]["message"]
        .as_str()
        .expect("stale diagnostic message")
        .contains("target file not found: b.md"));

    session.send(json!({
        "jsonrpc": "2.0",
        "method": "textDocument/didChange",
        "params": {
            "textDocument": { "uri": fixture.a_uri, "version": 2 },
            "contentChanges": [
                {
                    "text": "# A\n\n## Details\n\nSee [B](archived/b.md#target)\nSee [[archived/b#target]]\n"
                }
            ]
        }
    }));

    let fixed_diag = session.wait_for(Duration::from_secs(5), |message| {
        diagnostics_for_uri(message, &fixture.a_uri)
    });
    let fixed_diags = fixed_diag["params"]["diagnostics"]
        .as_array()
        .expect("diagnostics array after link update");
    assert!(fixed_diags.is_empty());

    session.shutdown();
}

#[test]
fn goto_definition_resolves_wikilink_targets() {
    let fixture = VaultFixture::new();
    let mut session = LspSession::start(&fixture.root);
    session.initialize(&fixture.root);

    session.send(json!({
        "jsonrpc": "2.0",
        "id": 7,
        "method": "textDocument/definition",
        "params": {
            "textDocument": { "uri": fixture.a_uri },
            "position": { "line": 5, "character": 8 }
        }
    }));

    let response = session.wait_for_id(7, Duration::from_secs(5));
    assert_eq!(response["result"]["uri"], json!(fixture.b_uri.as_str()));
    assert_eq!(response["result"]["range"]["start"]["line"], json!(2));

    session.shutdown();
}

#[test]
fn completion_uses_unsaved_document_buffer_state() {
    let fixture = VaultFixture::new();
    let mut session = LspSession::start(&fixture.root);
    session.initialize(&fixture.root);

    session.send(json!({
        "jsonrpc": "2.0",
        "method": "textDocument/didOpen",
        "params": {
            "textDocument": {
                "uri": fixture.scratch_uri,
                "languageId": "markdown",
                "version": 1,
                "text": "# Scratch\n\n[[b#\n"
            }
        }
    }));

    session.send(json!({
        "jsonrpc": "2.0",
        "id": 8,
        "method": "textDocument/completion",
        "params": {
            "textDocument": { "uri": fixture.scratch_uri },
            "position": { "line": 2, "character": 4 }
        }
    }));

    let response = session.wait_for_id(8, Duration::from_secs(5));
    let heading_items = response["result"].as_array().expect("completion array");
    let labels = heading_items
        .iter()
        .filter_map(|item| item.get("label").and_then(Value::as_str))
        .collect::<Vec<_>>();
    assert!(labels.contains(&"Target"));

    let target_heading = heading_items
        .iter()
        .find(|item| item.get("label").and_then(Value::as_str) == Some("Target"))
        .expect("target heading completion");
    assert_eq!(target_heading["insertText"], json!("target"));

    session.shutdown();
}

#[test]
fn completion_includes_unsaved_open_file_from_cached_index() {
    let fixture = VaultFixture::new();
    let mut session = LspSession::start(&fixture.root);
    session.initialize(&fixture.root);

    session.send(json!({
        "jsonrpc": "2.0",
        "method": "textDocument/didOpen",
        "params": {
            "textDocument": {
                "uri": fixture.scratch_uri,
                "languageId": "markdown",
                "version": 1,
                "text": "# Scratch\n\n[[\n"
            }
        }
    }));
    let _ = session.wait_for(Duration::from_secs(5), |message| {
        diagnostics_for_uri(message, &fixture.scratch_uri)
    });

    session.send(json!({
        "jsonrpc": "2.0",
        "id": 10,
        "method": "textDocument/completion",
        "params": {
            "textDocument": { "uri": fixture.scratch_uri },
            "position": { "line": 2, "character": 2 }
        }
    }));

    let response = session.wait_for_id(10, Duration::from_secs(5));
    let labels = response["result"]
        .as_array()
        .expect("completion array")
        .iter()
        .filter_map(|item| item.get("label").and_then(Value::as_str))
        .collect::<Vec<_>>();
    assert!(labels.contains(&"scratch"));

    session.shutdown();
}

#[test]
fn heading_completion_reverts_to_disk_after_target_close() {
    let fixture = VaultFixture::new();
    let mut session = LspSession::start(&fixture.root);
    session.initialize(&fixture.root);

    session.send(json!({
        "jsonrpc": "2.0",
        "method": "textDocument/didOpen",
        "params": {
            "textDocument": {
                "uri": fixture.b_uri,
                "languageId": "markdown",
                "version": 1,
                "text": "# B\n\n## Renamed\n"
            }
        }
    }));
    let _ = session.wait_for(Duration::from_secs(5), |message| {
        diagnostics_for_uri(message, &fixture.b_uri)
    });

    session.send(json!({
        "jsonrpc": "2.0",
        "method": "textDocument/didOpen",
        "params": {
            "textDocument": {
                "uri": fixture.scratch_uri,
                "languageId": "markdown",
                "version": 1,
                "text": "# Scratch\n\n[[b#\n"
            }
        }
    }));

    session.send(json!({
        "jsonrpc": "2.0",
        "id": 11,
        "method": "textDocument/completion",
        "params": {
            "textDocument": { "uri": fixture.scratch_uri },
            "position": { "line": 2, "character": 4 }
        }
    }));

    let open_completion = session.wait_for_id(11, Duration::from_secs(5));
    let open_labels = open_completion["result"]
        .as_array()
        .expect("completion array")
        .iter()
        .filter_map(|item| item.get("label").and_then(Value::as_str))
        .collect::<Vec<_>>();
    assert!(open_labels.contains(&"Renamed"));

    session.send(json!({
        "jsonrpc": "2.0",
        "method": "textDocument/didClose",
        "params": {
            "textDocument": { "uri": fixture.b_uri }
        }
    }));
    let _ = session.wait_for(Duration::from_secs(5), |message| {
        diagnostics_for_uri(message, &fixture.b_uri)
    });

    session.send(json!({
        "jsonrpc": "2.0",
        "id": 12,
        "method": "textDocument/completion",
        "params": {
            "textDocument": { "uri": fixture.scratch_uri },
            "position": { "line": 2, "character": 4 }
        }
    }));

    let closed_completion = session.wait_for_id(12, Duration::from_secs(5));
    let closed_labels = closed_completion["result"]
        .as_array()
        .expect("completion array")
        .iter()
        .filter_map(|item| item.get("label").and_then(Value::as_str))
        .collect::<Vec<_>>();
    assert!(closed_labels.contains(&"Target"));

    session.shutdown();
}

#[test]
fn hover_on_nonexistent_target_returns_none() {
    let fixture = VaultFixture::new();
    let mut session = LspSession::start(&fixture.root);
    session.initialize(&fixture.root);

    session.send(json!({
        "jsonrpc": "2.0",
        "method": "textDocument/didOpen",
        "params": {
            "textDocument": {
                "uri": fixture.scratch_uri,
                "languageId": "markdown",
                "version": 1,
                "text": "# Scratch\n\n[bad](missing.md)\n"
            }
        }
    }));

    session.send(json!({
        "jsonrpc": "2.0",
        "id": 9,
        "method": "textDocument/hover",
        "params": {
            "textDocument": { "uri": fixture.scratch_uri },
            "position": { "line": 2, "character": 8 }
        }
    }));

    let response = session.wait_for_id(9, Duration::from_secs(5));
    assert_eq!(response["result"], Value::Null);

    session.shutdown();
}

#[test]
fn diagnostics_include_missing_heading_anchor_errors() {
    let fixture = VaultFixture::new();
    let mut session = LspSession::start(&fixture.root);
    session.initialize(&fixture.root);

    session.send(json!({
        "jsonrpc": "2.0",
        "method": "textDocument/didOpen",
        "params": {
            "textDocument": {
                "uri": fixture.scratch_uri,
                "languageId": "markdown",
                "version": 1,
                "text": "# Scratch\n\n[bad](b.md#missing-heading)\n"
            }
        }
    }));

    let diag = session.wait_for(Duration::from_secs(5), |message| {
        diagnostics_for_uri(message, &fixture.scratch_uri)
    });
    let diagnostics = diag["params"]["diagnostics"]
        .as_array()
        .expect("diagnostics array");
    assert!(!diagnostics.is_empty());
    assert!(diagnostics[0]["message"]
        .as_str()
        .expect("diagnostic message")
        .contains("target heading not found"));

    session.shutdown();
}
