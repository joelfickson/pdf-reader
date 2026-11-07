use serde_json::{Value, json};
use std::{
    io::{BufRead, BufReader, Write},
    path::PathBuf,
    process::{Child, ChildStdin, Command, Stdio},
    sync::mpsc,
    thread,
    time::Duration,
};

const RESPONSE_TIMEOUT: Duration = Duration::from_secs(20);

struct Session {
    child: Child,
    stdin: Option<ChildStdin>,
    lines: mpsc::Receiver<String>,
    next_id: u64,
}

impl Session {
    fn start() -> Self {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let mut child = Command::new(env!("CARGO_BIN_EXE_pdf-reader"))
            .arg("mcp")
            .env("PDFIUM_LIBRARY_PATH", root.join("vendor/pdfium/lib"))
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .expect("start pdf-reader mcp");
        let stdin = child.stdin.take().unwrap();
        let stdout = child.stdout.take().unwrap();
        let (sender, lines) = mpsc::channel();
        thread::spawn(move || {
            for line in BufReader::new(stdout).lines() {
                let Ok(line) = line else { break };
                if sender.send(line).is_err() {
                    break;
                }
            }
        });
        Self {
            child,
            stdin: Some(stdin),
            lines,
            next_id: 1,
        }
    }

    fn send(&mut self, message: Value) {
        let stdin = self.stdin.as_mut().expect("stdin is open");
        writeln!(stdin, "{message}").unwrap();
        stdin.flush().unwrap();
    }

    fn request(&mut self, method: &str, params: Value) -> Value {
        let id = self.next_id;
        self.next_id += 1;
        self.send(json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params}));
        loop {
            let line = self
                .lines
                .recv_timeout(RESPONSE_TIMEOUT)
                .expect("server responded in time");
            let message: Value = serde_json::from_str(&line).expect("stdout carries only JSON-RPC");
            if message["id"] == json!(id) {
                return message;
            }
        }
    }

    fn call(&mut self, tool: &str, arguments: Value) -> Value {
        let response = self.request("tools/call", json!({"name": tool, "arguments": arguments}));
        response["result"].clone()
    }
}

impl Session {
    fn close(&mut self) -> std::process::ExitStatus {
        drop(self.stdin.take());
        let deadline = std::time::Instant::now() + RESPONSE_TIMEOUT;
        loop {
            if let Some(status) = self.child.try_wait().unwrap() {
                return status;
            }
            assert!(
                std::time::Instant::now() < deadline,
                "server did not exit after stdin closed"
            );
            thread::sleep(Duration::from_millis(20));
        }
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn fixture(name: &str) -> String {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
        .to_string_lossy()
        .into_owned()
}

#[test]
#[ignore = "requires PDFium installed by scripts/setup_pdfium.py"]
fn mcp_server_serves_read_only_pdf_tools_over_stdio() {
    let mut session = Session::start();
    let initialized = session.request(
        "initialize",
        json!({
            "protocolVersion": "2025-06-18",
            "capabilities": {},
            "clientInfo": {"name": "pdf-reader-test", "version": "1"}
        }),
    );
    assert_eq!(initialized["result"]["serverInfo"]["name"], "agentic-pdf");
    assert!(initialized["result"]["capabilities"]["tools"].is_object());
    session.send(json!({"jsonrpc": "2.0", "method": "notifications/initialized"}));

    let listed = session.request("tools/list", json!({}));
    let mut names: Vec<String> = listed["result"]["tools"]
        .as_array()
        .unwrap()
        .iter()
        .map(|tool| tool["name"].as_str().unwrap().to_string())
        .collect();
    names.sort();
    assert_eq!(
        names,
        ["document_info", "get_outline", "get_text", "search"]
    );
    for tool in listed["result"]["tools"].as_array().unwrap() {
        assert_eq!(
            tool["annotations"]["readOnlyHint"], true,
            "{}",
            tool["name"]
        );
    }

    let info = session.call("document_info", json!({"path": fixture("selection.pdf")}));
    assert_eq!(info["structuredContent"]["page_count"], 3);
    assert_eq!(info["structuredContent"]["pages_without_text"], json!([3]));

    let outline = session.call("get_outline", json!({"path": fixture("outline.pdf")}));
    let entries = outline["structuredContent"]["entries"].as_array().unwrap();
    assert_eq!(entries.len(), 6);
    assert_eq!(entries[1]["title"], "Begin with the page");
    assert_eq!(entries[1]["level"], 2);
    assert_eq!(entries[5]["page"], 3);

    let found = session.call(
        "search",
        json!({"path": fixture("reading.pdf"), "query": "step aside"}),
    );
    assert_eq!(found["structuredContent"]["matches"][0]["page"], 1);
    assert_eq!(
        found["structuredContent"]["matches"][0]["matched"],
        "step aside"
    );

    let mut page = json!(1);
    let mut pages_read = Vec::new();
    while !page.is_null() {
        let range = session.call(
            "get_text",
            json!({"path": fixture("reading.pdf"), "from_page": page, "max_chars": 400}),
        );
        assert_ne!(range["isError"], true, "{range}");
        for entry in range["structuredContent"]["pages"].as_array().unwrap() {
            pages_read.push(entry["page"].as_u64().unwrap());
        }
        page = range["structuredContent"]["next_page"].clone();
    }
    assert_eq!(pages_read, [1, 2, 3]);

    let rejected = session.call("get_text", json!({"path": "/etc/hosts"}));
    assert_eq!(rejected["isError"], true);
    assert!(
        rejected["content"][0]["text"]
            .as_str()
            .unwrap()
            .contains("not a .pdf")
    );

    let empty_query = session.call(
        "search",
        json!({"path": fixture("reading.pdf"), "query": "  "}),
    );
    assert_eq!(empty_query["isError"], true);

    let status = session.close();
    assert!(status.success(), "server exited with {status}");
}
