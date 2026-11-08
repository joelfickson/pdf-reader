use std::ops::Range;
use std::path::Path;

pub const SUMMARY_REQUEST: &str = "Summarize this document.";
const SUMMARY_INSTRUCTIONS: &str = "Summarize this document. Start with document_info and get_outline, then read the text with get_text in ranges until you reach the last page. Cite the page for every claim as (p. N). Say which pages you could not read.";
const TOOL_PREFIX: &str = "mcp__pdf-reader__";
const MAX_TOOL_ROWS: usize = 200;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Status {
    Pending,
    Running,
    Done,
    Failed,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PlanStep {
    pub text: String,
    pub status: Status,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ToolRow {
    pub id: String,
    pub title: String,
    pub detail: String,
    pub status: Status,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChoiceKind {
    AllowOnce,
    AllowAlways,
    RejectOnce,
    RejectAlways,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Choice {
    pub id: String,
    pub name: String,
    pub kind: ChoiceKind,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Update {
    Text(String),
    Plan(Vec<PlanStep>),
    ToolStarted(ToolRow),
    ToolChanged {
        id: String,
        title: Option<String>,
        detail: Option<String>,
        status: Option<Status>,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub enum Entry {
    User {
        text: String,
        context: String,
    },
    Plan(Vec<PlanStep>),
    Tools(Vec<ToolRow>),
    Answer(String),
    Permission {
        request: u64,
        title: String,
        choices: Vec<Choice>,
        chosen: Option<String>,
    },
    Notice(String),
}

#[derive(Default)]
pub struct Conversation {
    pub entries: Vec<Entry>,
}

impl Conversation {
    pub fn ask(&mut self, text: &str, context: &str) {
        self.entries.push(Entry::User {
            text: text.to_string(),
            context: context.to_string(),
        });
    }

    pub fn notice(&mut self, text: impl Into<String>) {
        self.entries.push(Entry::Notice(text.into()));
    }

    fn turn_start(&self) -> usize {
        self.entries
            .iter()
            .rposition(|entry| matches!(entry, Entry::User { .. }))
            .unwrap_or(0)
    }

    pub fn apply(&mut self, update: Update) {
        match update {
            Update::Text(text) => match self.entries.last_mut() {
                Some(Entry::Answer(answer)) => answer.push_str(&text),
                _ => self.entries.push(Entry::Answer(text)),
            },
            Update::Plan(steps) => {
                let start = self.turn_start();
                match self.entries[start..]
                    .iter_mut()
                    .find(|entry| matches!(entry, Entry::Plan(_)))
                {
                    Some(Entry::Plan(existing)) => *existing = steps,
                    _ => self.entries.push(Entry::Plan(steps)),
                }
            }
            Update::ToolStarted(row) => match self.entries.last_mut() {
                Some(Entry::Tools(rows)) if rows.len() < MAX_TOOL_ROWS => rows.push(row),
                _ => self.entries.push(Entry::Tools(vec![row])),
            },
            Update::ToolChanged {
                id,
                title,
                detail,
                status,
            } => {
                if let Some(row) = self
                    .entries
                    .iter_mut()
                    .rev()
                    .filter_map(|entry| match entry {
                        Entry::Tools(rows) => Some(rows),
                        _ => None,
                    })
                    .flat_map(|rows| rows.iter_mut().rev())
                    .find(|row| row.id == id)
                {
                    if let Some(title) = title {
                        row.title = title;
                    }
                    if let Some(detail) = detail {
                        row.detail = detail;
                    }
                    if let Some(status) = status {
                        row.status = status;
                    }
                }
            }
        }
    }

    pub fn permission(&mut self, request: u64, title: String, choices: Vec<Choice>) {
        self.entries.push(Entry::Permission {
            request,
            title,
            choices,
            chosen: None,
        });
    }

    pub fn choose(&mut self, request: u64, name: String) {
        for entry in &mut self.entries {
            if let Entry::Permission {
                request: pending,
                chosen,
                ..
            } = entry
                && *pending == request
            {
                *chosen = Some(name.clone());
            }
        }
    }

    pub fn last_answer(&self) -> Option<&str> {
        self.entries.iter().rev().find_map(|entry| match entry {
            Entry::Answer(text) => Some(text.as_str()),
            _ => None,
        })
    }

    pub fn finish_turn(&mut self) {
        let start = self.turn_start();
        for entry in &mut self.entries[start..] {
            match entry {
                Entry::Tools(rows) => {
                    for row in rows {
                        if matches!(row.status, Status::Pending | Status::Running) {
                            row.status = Status::Done;
                        }
                    }
                }
                Entry::Plan(steps) => {
                    for step in steps {
                        if step.status == Status::Running {
                            step.status = Status::Done;
                        }
                    }
                }
                _ => {}
            }
        }
    }
}

pub fn tool_label(title: &str) -> String {
    let name = title.strip_prefix(TOOL_PREFIX).unwrap_or(title);
    match name {
        "document_info" => "Read document info".into(),
        "get_outline" => "Read outline".into(),
        "get_text" => "Read text".into(),
        "search" => "Search".into(),
        other => other.to_string(),
    }
}

pub fn tool_detail(input: &serde_json::Value) -> Option<String> {
    if let Some(query) = input.get("query").and_then(|value| value.as_str()) {
        return Some(format!("\"{query}\""));
    }
    let from = input.get("from_page").and_then(|value| value.as_u64())?;
    Some(format!("From page {from}"))
}

pub fn file_uri(path: &Path) -> String {
    let text = path.to_string_lossy().replace('\\', "/");
    let mut encoded = String::from(if text.starts_with('/') {
        "file://"
    } else {
        "file:///"
    });
    for byte in text.bytes() {
        if byte.is_ascii_alphanumeric() || b"/-._~:".contains(&byte) {
            encoded.push(byte as char);
        } else {
            encoded.push_str(&format!("%{byte:02X}"));
        }
    }
    encoded
}

pub fn summary_prompt(path: &Path) -> String {
    format!(
        "{SUMMARY_INSTRUCTIONS}\n\nThe document is the PDF at {}. Read it with the pdf-reader tools.",
        path.display()
    )
}

pub fn question_prompt(question: &str, path: &Path, page: usize) -> String {
    format!(
        "{question}\n\nThis question is about the PDF at {}, which is open on page {page}. Read it with the pdf-reader tools and cite pages as (p. N).",
        path.display()
    )
}

pub fn page_references(text: &str) -> Vec<(Range<usize>, usize)> {
    let lower = text.to_ascii_lowercase();
    let bytes = lower.as_bytes();
    let mut found = Vec::new();
    let mut index = 0;
    while index < bytes.len() {
        let rest = &lower[index..];
        let prefix = ["pages ", "page ", "pp. ", "pp.", "p. ", "p."]
            .into_iter()
            .find(|prefix| rest.starts_with(prefix));
        let at_word = index == 0 || !bytes[index - 1].is_ascii_alphanumeric();
        if let (Some(prefix), true) = (prefix, at_word) {
            let digits_start = index + prefix.len();
            let digits_end = lower[digits_start..]
                .find(|character: char| !character.is_ascii_digit())
                .map_or(lower.len(), |offset| digits_start + offset);
            if digits_end > digits_start
                && let Ok(page) = lower[digits_start..digits_end].parse::<usize>()
                && page > 0
            {
                found.push((index..digits_end, page));
                index = digits_end;
                continue;
            }
        }
        index += rest.chars().next().map_or(1, char::len_utf8);
    }
    found
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Block {
    Heading(String),
    Bullet(String),
    Paragraph(String),
}

fn plain(text: &str) -> String {
    text.replace("**", "").replace('`', "").trim().to_string()
}

pub fn blocks(markdown: &str) -> Vec<Block> {
    let mut blocks = Vec::new();
    let mut paragraph: Vec<&str> = Vec::new();
    let flush = |paragraph: &mut Vec<&str>, blocks: &mut Vec<Block>| {
        if !paragraph.is_empty() {
            blocks.push(Block::Paragraph(plain(&paragraph.join(" "))));
            paragraph.clear();
        }
    };
    for line in markdown.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            flush(&mut paragraph, &mut blocks);
        } else if let Some(heading) = trimmed.strip_prefix('#') {
            flush(&mut paragraph, &mut blocks);
            blocks.push(Block::Heading(plain(heading.trim_start_matches('#'))));
        } else if let Some(item) = trimmed
            .strip_prefix("- ")
            .or_else(|| trimmed.strip_prefix("* "))
            .or_else(|| {
                let digits = trimmed.find(|c: char| !c.is_ascii_digit())?;
                (digits > 0)
                    .then(|| trimmed[digits..].strip_prefix(". "))
                    .flatten()
            })
        {
            flush(&mut paragraph, &mut blocks);
            blocks.push(Block::Bullet(plain(item)));
        } else {
            paragraph.push(trimmed);
        }
    }
    flush(&mut paragraph, &mut blocks);
    blocks
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tool(id: &str) -> ToolRow {
        ToolRow {
            id: id.into(),
            title: "Read text".into(),
            detail: String::new(),
            status: Status::Pending,
        }
    }

    #[test]
    fn streams_answers_groups_tools_and_replaces_the_turn_plan() {
        let mut conversation = Conversation::default();
        conversation.ask(SUMMARY_REQUEST, "book.pdf, 9 pages");
        let step = |text: &str, status| PlanStep {
            text: text.into(),
            status,
        };
        conversation.apply(Update::Plan(vec![step("Outline", Status::Running)]));
        conversation.apply(Update::ToolStarted(tool("a")));
        conversation.apply(Update::ToolStarted(tool("b")));
        conversation.apply(Update::Plan(vec![step("Outline", Status::Done)]));
        conversation.apply(Update::ToolChanged {
            id: "a".into(),
            title: None,
            detail: Some("From page 1".into()),
            status: Some(Status::Done),
        });
        conversation.apply(Update::Text("A short ".into()));
        conversation.apply(Update::Text("book (p. 1).".into()));
        assert_eq!(conversation.entries.len(), 4);
        assert_eq!(
            conversation.entries[1],
            Entry::Plan(vec![step("Outline", Status::Done)])
        );
        let Entry::Tools(rows) = &conversation.entries[2] else {
            panic!("tools expected")
        };
        assert_eq!(
            (rows[0].status, rows[0].detail.as_str()),
            (Status::Done, "From page 1")
        );
        assert_eq!(rows[1].status, Status::Pending);
        assert_eq!(conversation.last_answer(), Some("A short book (p. 1)."));

        conversation.finish_turn();
        let Entry::Tools(rows) = &conversation.entries[2] else {
            panic!("tools expected")
        };
        assert_eq!(rows[1].status, Status::Done);

        conversation.ask("And page 4?", "book.pdf, page 4");
        conversation.apply(Update::Plan(vec![step("Read page 4", Status::Running)]));
        assert_eq!(conversation.entries.len(), 6);
    }

    #[test]
    fn permission_choices_are_recorded() {
        let mut conversation = Conversation::default();
        conversation.permission(
            7,
            "Highlight 2 passages".into(),
            vec![Choice {
                id: "allow".into(),
                name: "Allow once".into(),
                kind: ChoiceKind::AllowOnce,
            }],
        );
        conversation.choose(7, "Allow once".into());
        assert!(matches!(
            &conversation.entries[0],
            Entry::Permission { chosen: Some(name), .. } if name == "Allow once"
        ));
    }

    #[test]
    fn finds_page_references_in_common_forms() {
        let text =
            "Rent rises (p. 3), see pp.4 and page 12; Pages 7 to 9. Not a map. or p.0 or page x.";
        let found: Vec<_> = page_references(text)
            .into_iter()
            .map(|(range, page)| (&text[range], page))
            .collect();
        assert_eq!(
            found,
            [("p. 3", 3), ("pp.4", 4), ("page 12", 12), ("Pages 7", 7)]
        );
        assert!(page_references("step 2 of the map.5").is_empty());
        assert_eq!(page_references("Café p. 2")[0].1, 2);
    }

    #[test]
    fn renders_a_small_markdown_subset() {
        let text = "## Summary\nA **ten-year** lease\nfor `floor 3`.\n\n- Term: 10 years (p. 2)\n* Rent\n2. Review\n\nGaps remain.";
        assert_eq!(
            blocks(text),
            [
                Block::Heading("Summary".into()),
                Block::Paragraph("A ten-year lease for floor 3.".into()),
                Block::Bullet("Term: 10 years (p. 2)".into()),
                Block::Bullet("Rent".into()),
                Block::Bullet("Review".into()),
                Block::Paragraph("Gaps remain.".into()),
            ]
        );
    }

    #[test]
    fn labels_tools_and_builds_prompts() {
        assert_eq!(tool_label("mcp__pdf-reader__get_outline"), "Read outline");
        assert_eq!(tool_label("ToolSearch"), "ToolSearch");
        assert_eq!(
            tool_detail(&serde_json::json!({"path": "a.pdf", "from_page": 6})),
            Some("From page 6".into())
        );
        assert_eq!(
            tool_detail(&serde_json::json!({"query": "rent"})),
            Some("\"rent\"".into())
        );
        assert_eq!(
            file_uri(Path::new("/Users/me/My Lease (v2).pdf")),
            "file:///Users/me/My%20Lease%20%28v2%29.pdf"
        );
        assert_eq!(
            file_uri(Path::new("C:\\Docs\\a b.pdf")),
            "file:///C:/Docs/a%20b.pdf"
        );
        assert!(summary_prompt(Path::new("/tmp/a.pdf")).contains("PDF at /tmp/a.pdf"));
        assert!(question_prompt("Why?", Path::new("/tmp/a.pdf"), 4).starts_with("Why?\n\n"));
    }
}
