//! What a Chita extension is written with.
//!
//! An extension is a WebAssembly module (`wasm32-unknown-unknown`, a `cdylib`) that Chita
//! runs in its confined host. It can't open files, use the network or run programs: it asks
//! Chita, which answers from what the user granted ([`ask`], [`read_file`], [`list_folder`],
//! [`run_tool`]). Chita calls into it for what its manifest contributes: a command
//! (`command`, with a [`CommandArgument`], giving back a [`CommandResult`]), a column
//! (`column`, [`ColumnArgument`] → one text per path), a panel (`panel`,
//! [`PanelArgument`] → [`Panel`]), a context it owns (`context`, [`ContextArgument`] →
//! [`Context`]: the header's line and actions for a kind of folder, a git repository) and a
//! screen of it (`screen`, [`ScreenArgument`] → [`Screen`]: sections of rows with actions,
//! drawn by Chita in place of the file list). The [`extension!`] macro writes those entry
//! points and the memory function the host needs, from plain Rust functions.
//!
//! Natively (tests), nothing is imported: [`testing::answer_with`] installs what answers the
//! asks, and [`testing::logged`] / [`testing::taken`] show what the extension logged and gave
//! back.

use serde::{Deserialize, Serialize};

/// The requests and answers between an extension and Chita (JSON, externally tagged).
pub mod protocol {
    use serde::{Deserialize, Serialize};

    /// What an extension asks Chita for.
    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
    pub enum Ask {
        /// A file's contents: a path relative to the viewed folder, or absolute.
        ReadFile { path: String },
        /// A folder's entries.
        ListFolder { path: String },
        /// A tool from the manifest, run in the viewed folder.
        RunTool { tool: String, args: Vec<String> },
        /// Writes a file whole, where the manifest's grant says `write`.
        WriteFile { path: String, contents: Vec<u8> },
        /// Clones a repository into the viewed folder from an address the user types into
        /// Chita's own prompt. Needs the `git` tool.
        CloneRepository {},
    }

    /// Chita's answer.
    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
    pub enum Answer {
        File {
            contents: Vec<u8>,
        },
        Written {},
        Folder {
            entries: Vec<Entry>,
        },
        Tool {
            status: i32,
            stdout: String,
            stderr: String,
        },
        Refused {
            reason: String,
        },
    }

    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
    pub struct Entry {
        pub name: String,
        pub is_folder: bool,
        pub size: u64,
        /// Seconds since 1970.
        pub modified: i64,
    }
}

pub use protocol::{Answer, Ask, Entry};

/// What a tool printed, and how it ended.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolOutput {
    /// The exit status (`-1` when stopped for taking too long).
    pub status: i32,
    pub stdout: String,
    pub stderr: String,
}

impl ToolOutput {
    pub fn succeeded(&self) -> bool {
        self.status == 0
    }
}

// MARK: What Chita calls with

/// What a command gets.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommandArgument {
    /// The command's id from the manifest.
    pub command: String,
    /// The folder being viewed, when it's one on this Mac.
    #[serde(default)]
    pub folder: Option<String>,
    /// The selected items (or what was right-clicked), as full paths.
    #[serde(default)]
    pub selection: Vec<String>,
    /// What the user typed, when the command asked for it with a [`Prompt`] the time before.
    #[serde(default)]
    pub input: Option<String>,
    /// The screen the command came from, when it did (an action of a row, a section or the
    /// toolbar), and the rows it applies to (their ids).
    #[serde(default)]
    pub screen: Option<String>,
    #[serde(default)]
    pub rows: Vec<String>,
}

/// What a command gives back: a short message (shown as an alert), output (shown in full),
/// or a prompt (Chita asks the user, then calls the command again with their `input`).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommandResult {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prompt: Option<Prompt>,
    /// A unified diff, shown coloured (the Inspector, for a screen's selection).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub diff: Option<String>,
    /// A title for the output or the diff.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
}

impl CommandResult {
    pub fn message(text: impl Into<String>) -> Self {
        CommandResult {
            message: Some(text.into()),
            ..Default::default()
        }
    }

    pub fn output(text: impl Into<String>) -> Self {
        CommandResult {
            output: Some(text.into()),
            ..Default::default()
        }
    }

    pub fn prompt(prompt: Prompt) -> Self {
        CommandResult {
            prompt: Some(prompt),
            ..Default::default()
        }
    }

    /// A diff to show (the Inspector).
    pub fn diff(title: impl Into<String>, text: impl Into<String>) -> Self {
        CommandResult {
            diff: Some(text.into()),
            title: Some(title.into()),
            ..Default::default()
        }
    }

    /// Nothing to show.
    pub fn nothing() -> Self {
        Self::default()
    }
}

/// Text asked of the user: one line, or a text (`multiline`) with something to start from.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Prompt {
    pub title: String,
    #[serde(default)]
    pub message: String,
    #[serde(default)]
    pub placeholder: String,
    /// The button, "OK" when empty.
    #[serde(default)]
    pub action: String,
    /// A text editor instead of one line (a file's contents).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub multiline: bool,
    /// What the field holds to begin with.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub initial: String,
    /// The command the answer goes to (as its `input`); the same command when empty.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub next: String,
    /// Texts the editor offers to insert (templates), by title.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub inserts: Vec<Insert>,
}

/// A text the editor can insert at the end, chosen by its title.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Insert {
    pub title: String,
    pub text: String,
}

impl Prompt {
    pub fn line(
        title: impl Into<String>,
        message: impl Into<String>,
        action: impl Into<String>,
    ) -> Prompt {
        Prompt {
            title: title.into(),
            message: message.into(),
            action: action.into(),
            ..Default::default()
        }
    }

    pub fn placeholder(mut self, text: impl Into<String>) -> Self {
        self.placeholder = text.into();
        self
    }

    /// A text editor, starting from `initial`.
    pub fn editor(mut self, initial: impl Into<String>) -> Self {
        self.multiline = true;
        self.initial = initial.into();
        self
    }

    /// The answer goes to another command.
    pub fn next(mut self, command: impl Into<String>) -> Self {
        self.next = command.into();
        self
    }

    /// A text the editor offers to insert.
    pub fn insert(mut self, title: impl Into<String>, text: impl Into<String>) -> Self {
        self.inserts.push(Insert {
            title: title.into(),
            text: text.into(),
        });
        self
    }
}

/// A column's cell with more than text: a symbol in a colour before it, or dimmed (an
/// item that doesn't count, like one git ignores). `column` may answer these instead of
/// plain texts (the `cells` entry point).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ColumnCell {
    pub text: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub symbol: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub color: String,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub dim: bool,
}

impl ColumnCell {
    pub fn text(text: impl Into<String>) -> ColumnCell {
        ColumnCell {
            text: text.into(),
            ..Default::default()
        }
    }

    pub fn symbol(mut self, name: impl Into<String>, color: impl Into<String>) -> Self {
        self.symbol = name.into();
        self.color = color.into();
        self
    }

    pub fn dim(mut self) -> Self {
        self.dim = true;
        self
    }
}

// MARK: Contexts and screens

/// What `context` gets: which context, where the user is.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContextArgument {
    pub context: String,
    #[serde(default)]
    pub folder: Option<String>,
    #[serde(default)]
    pub selection: Vec<String>,
}

/// What the header shows for a context: whether the folder is one at all, a title line (the
/// branch), a subtitle (the state), the actions, and badges on the screens (a count).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Context {
    pub active: bool,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub subtitle: String,
    #[serde(default)]
    pub actions: Vec<Action>,
    /// Screen id → text on its segment ("3").
    #[serde(default, skip_serializing_if = "std::collections::BTreeMap::is_empty")]
    pub badges: std::collections::BTreeMap<String, String>,
}

impl Context {
    /// Not this kind of folder after all.
    pub fn inactive() -> Context {
        Context::default()
    }

    pub fn active(title: impl Into<String>) -> Context {
        Context {
            active: true,
            title: title.into(),
            ..Default::default()
        }
    }

    pub fn subtitle(mut self, text: impl Into<String>) -> Self {
        self.subtitle = text.into();
        self
    }

    pub fn action(mut self, action: Action) -> Self {
        self.actions.push(action);
        self
    }

    pub fn badge(mut self, screen: impl Into<String>, text: impl Into<String>) -> Self {
        self.badges.insert(screen.into(), text.into());
        self
    }
}

/// A button: in the header, a screen's toolbar, a section, or a row. Pressed, Chita calls
/// `command` with the action's `id` (and, from a screen, the screen and the rows).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Action {
    pub id: String,
    pub title: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub symbol: String,
    /// "default", "prominent" (the one to press) or "destructive" (red; asks first when
    /// `confirm` says what).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub style: String,
    /// The question asked before, for a destructive action.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub confirm: String,
}

impl Action {
    pub fn new(id: impl Into<String>, title: impl Into<String>) -> Action {
        Action {
            id: id.into(),
            title: title.into(),
            ..Default::default()
        }
    }

    pub fn symbol(mut self, name: impl Into<String>) -> Self {
        self.symbol = name.into();
        self
    }

    pub fn prominent(mut self) -> Self {
        self.style = "prominent".into();
        self
    }

    pub fn destructive(mut self, confirm: impl Into<String>) -> Self {
        self.style = "destructive".into();
        self.confirm = confirm.into();
        self
    }
}

/// What `screen` gets: which screen, where the user is, what's selected in the list.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScreenArgument {
    pub screen: String,
    #[serde(default)]
    pub folder: Option<String>,
    #[serde(default)]
    pub selection: Vec<String>,
}

/// A screen: sections of rows, drawn by Chita in place of the file list, with a toolbar, a
/// search over the rows (Chita's), a selection, and the Inspector showing what `inspector`
/// (a command id) gives back for the selected rows (a diff, output).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Screen {
    #[serde(default)]
    pub toolbar: Vec<Action>,
    #[serde(default)]
    pub sections: Vec<Section>,
    /// Shown when there are no rows at all.
    #[serde(default)]
    pub empty: String,
    /// "single" (the default), "multiple" or "none".
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub selection: String,
    /// The command Chita calls for the selected rows, whose result the Inspector shows.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub inspector: Option<String>,
}

impl Screen {
    pub fn new() -> Screen {
        Screen::default()
    }

    pub fn toolbar(mut self, action: Action) -> Self {
        self.toolbar.push(action);
        self
    }

    pub fn section(mut self, section: Section) -> Self {
        self.sections.push(section);
        self
    }

    pub fn empty(mut self, text: impl Into<String>) -> Self {
        self.empty = text.into();
        self
    }

    pub fn multiple(mut self) -> Self {
        self.selection = "multiple".into();
        self
    }

    pub fn inspector(mut self, command: impl Into<String>) -> Self {
        self.inspector = Some(command.into());
        self
    }

    pub fn rows(&self) -> usize {
        self.sections.iter().map(|s| s.rows.len()).sum()
    }
}

/// A section of a screen: a title, rows, and actions on the section as a whole.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Section {
    pub id: String,
    pub title: String,
    #[serde(default)]
    pub rows: Vec<ScreenRow>,
    #[serde(default)]
    pub actions: Vec<Action>,
}

impl Section {
    pub fn new(id: impl Into<String>, title: impl Into<String>) -> Section {
        Section {
            id: id.into(),
            title: title.into(),
            ..Default::default()
        }
    }

    pub fn row(mut self, row: ScreenRow) -> Self {
        self.rows.push(row);
        self
    }

    pub fn action(mut self, action: Action) -> Self {
        self.actions.push(action);
        self
    }
}

/// A row: a title, a subtitle, an icon (an SF Symbol in a colour, or the file at `path`), a
/// badge, its actions (in its menu and on hover), and the `primary` one (double-click,
/// Return).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScreenRow {
    pub id: String,
    pub title: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub subtitle: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub symbol: String,
    /// "red", "orange", "yellow", "green", "blue", "purple", "gray" or "accent".
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub color: String,
    /// A file: Chita shows its icon, and Reveal in the row's menu.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub path: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub badge: String,
    #[serde(default)]
    pub actions: Vec<Action>,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub primary: String,
    /// A folder on this Mac: Chita offers Open (here, or in a new tab), and opens it on a
    /// double click when there's no primary action.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub folder: String,
    /// Its place in a graph of rows (a commit history): drawn before the icon.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub graph: Option<GraphCell>,
}

/// A row's cell of a graph: lanes are columns, numbered from 0. The row's own dot sits in
/// `lane`; `above` are the lines from the row above (lane there → lane here, where a line
/// may bend into this row's lane), `below` the lines to the row below.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct GraphCell {
    pub lane: u32,
    #[serde(default)]
    pub above: Vec<(u32, u32)>,
    #[serde(default)]
    pub below: Vec<(u32, u32)>,
}

impl ScreenRow {
    pub fn new(id: impl Into<String>, title: impl Into<String>) -> ScreenRow {
        ScreenRow {
            id: id.into(),
            title: title.into(),
            ..Default::default()
        }
    }

    /// A row for a file (its icon, Reveal), with its full `path`.
    pub fn file(path: impl Into<String>, title: impl Into<String>) -> ScreenRow {
        let path = path.into();
        ScreenRow {
            id: path.clone(),
            title: title.into(),
            path,
            ..Default::default()
        }
    }

    pub fn subtitle(mut self, text: impl Into<String>) -> Self {
        self.subtitle = text.into();
        self
    }

    pub fn symbol(mut self, name: impl Into<String>, color: impl Into<String>) -> Self {
        self.symbol = name.into();
        self.color = color.into();
        self
    }

    pub fn badge(mut self, text: impl Into<String>) -> Self {
        self.badge = text.into();
        self
    }

    pub fn action(mut self, action: Action) -> Self {
        self.actions.push(action);
        self
    }

    /// The action for a double click or Return.
    pub fn primary(mut self, id: impl Into<String>) -> Self {
        self.primary = id.into();
        self
    }

    /// A folder on this Mac the row stands for.
    pub fn folder(mut self, path: impl Into<String>) -> Self {
        self.folder = path.into();
        self
    }

    pub fn graph(mut self, cell: GraphCell) -> Self {
        self.graph = Some(cell);
        self
    }
}

/// What a column gets: the column's id, the folder, and the paths to give a text for.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ColumnArgument {
    pub column: String,
    pub folder: String,
    pub paths: Vec<String>,
}

/// What a panel gets.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PanelArgument {
    pub panel: String,
    #[serde(default)]
    pub folder: Option<String>,
    #[serde(default)]
    pub selection: Vec<String>,
}

/// What a panel shows: a title (the manifest's when empty), rows, text under them. With no
/// rows and no text, Chita shows nothing.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Panel {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(default)]
    pub rows: Vec<Row>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Row {
    pub label: String,
    pub value: String,
}

impl Panel {
    pub fn row(mut self, label: impl Into<String>, value: impl Into<String>) -> Self {
        self.rows.push(Row {
            label: label.into(),
            value: value.into(),
        });
        self
    }

    pub fn text(mut self, text: impl Into<String>) -> Self {
        self.text = Some(text.into());
        self
    }
}

// MARK: Talking to Chita

/// Logs a line (Settings › Extensions shows the last ones).
pub fn log(text: &str) {
    host::log(text);
}

/// Asks Chita for something; what it answered.
pub fn ask(ask: &Ask) -> Answer {
    let json = serde_json::to_vec(ask).unwrap_or_default();
    let answer = host::ask(&json);
    serde_json::from_slice(&answer).unwrap_or_else(|e| Answer::Refused {
        reason: format!("not an answer the extension understands: {e}"),
    })
}

/// A file's contents, or why not.
pub fn read_file(path: &str) -> Result<Vec<u8>, String> {
    match ask(&Ask::ReadFile {
        path: path.to_owned(),
    }) {
        Answer::File { contents } => Ok(contents),
        Answer::Refused { reason } => Err(reason),
        other => Err(format!("unexpected answer: {other:?}")),
    }
}

/// A folder's entries, or why not.
pub fn list_folder(path: &str) -> Result<Vec<Entry>, String> {
    match ask(&Ask::ListFolder {
        path: path.to_owned(),
    }) {
        Answer::Folder { entries } => Ok(entries),
        Answer::Refused { reason } => Err(reason),
        other => Err(format!("unexpected answer: {other:?}")),
    }
}

/// Writes a file whole (the manifest's grant must say `write`), or says why not.
pub fn write_file(path: &str, contents: &[u8]) -> Result<(), String> {
    match ask(&Ask::WriteFile {
        path: path.to_owned(),
        contents: contents.to_vec(),
    }) {
        Answer::Written {} => Ok(()),
        Answer::Refused { reason } => Err(reason),
        other => Err(format!("unexpected answer: {other:?}")),
    }
}

/// Clones a repository into the viewed folder: Chita asks the user for the address and
/// runs git. What git said, or why not (`"cancelled"` when the user didn't answer).
pub fn clone_repository() -> Result<ToolOutput, String> {
    match ask(&Ask::CloneRepository {}) {
        Answer::Tool {
            status,
            stdout,
            stderr,
        } => Ok(ToolOutput {
            status,
            stdout,
            stderr,
        }),
        Answer::Refused { reason } => Err(reason),
        other => Err(format!("unexpected answer: {other:?}")),
    }
}

/// Runs a tool from the manifest in the viewed folder, or says why not.
pub fn run_tool(tool: &str, args: &[&str]) -> Result<ToolOutput, String> {
    match ask(&Ask::RunTool {
        tool: tool.to_owned(),
        args: args.iter().map(|a| (*a).to_owned()).collect(),
    }) {
        Answer::Tool {
            status,
            stdout,
            stderr,
        } => Ok(ToolOutput {
            status,
            stdout,
            stderr,
        }),
        Answer::Refused { reason } => Err(reason),
        other => Err(format!("unexpected answer: {other:?}")),
    }
}

/// Gives a call's result back to Chita (the entry points do this for you).
pub fn result(text: &str) {
    host::result(text);
}

pub fn result_json<T: Serialize>(value: &T) {
    if let Ok(text) = serde_json::to_string(value) {
        host::result(&text);
    }
}

/// Reads the JSON argument of an entry point.
pub fn argument<T: for<'de> Deserialize<'de> + Default>(json: &[u8]) -> T {
    serde_json::from_slice(json).unwrap_or_else(|e| {
        log(&format!("the argument couldn't be read: {e}"));
        T::default()
    })
}

// MARK: The ABI

/// The host's imports, and natively a stand-in for tests.
#[doc(hidden)]
pub mod host {
    #[cfg(target_arch = "wasm32")]
    mod imports {
        #[link(wasm_import_module = "chita")]
        unsafe extern "C" {
            pub fn log(ptr: *const u8, len: usize);
            pub fn ask(ptr: *const u8, len: usize) -> i32;
            pub fn take(out_ptr: *mut u8, out_cap: i32) -> i32;
            pub fn result(ptr: *const u8, len: usize);
        }
    }

    #[cfg(target_arch = "wasm32")]
    pub fn log(text: &str) {
        // SAFETY: the pointer and length describe `text`, which lives through the call.
        unsafe { imports::log(text.as_ptr(), text.len()) }
    }

    #[cfg(target_arch = "wasm32")]
    pub fn ask(json: &[u8]) -> Vec<u8> {
        // SAFETY: the pointer and length describe `json`, which lives through the call.
        let length = unsafe { imports::ask(json.as_ptr(), json.len()) };
        let Ok(length) = usize::try_from(length) else {
            return b"{\"Refused\":{\"reason\":\"no answer\"}}".to_vec();
        };
        let mut answer = vec![0u8; length];
        // SAFETY: `answer` has `length` bytes for the host to fill.
        let taken = unsafe { imports::take(answer.as_mut_ptr(), length as i32) };
        if usize::try_from(taken) != Ok(length) {
            return b"{\"Refused\":{\"reason\":\"the answer couldn't be taken\"}}".to_vec();
        }
        answer
    }

    #[cfg(target_arch = "wasm32")]
    pub fn result(text: &str) {
        // SAFETY: the pointer and length describe `text`, which lives through the call.
        unsafe { imports::result(text.as_ptr(), text.len()) }
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub fn log(text: &str) {
        super::testing::STATE.with(|s| s.borrow_mut().logged.push(text.to_owned()));
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub fn ask(json: &[u8]) -> Vec<u8> {
        let ask: super::Ask = match serde_json::from_slice(json) {
            Ok(ask) => ask,
            Err(e) => {
                return serde_json::to_vec(&super::Answer::Refused {
                    reason: e.to_string(),
                })
                .unwrap_or_default();
            }
        };
        let answer = super::testing::STATE.with(|s| {
            let mut state = s.borrow_mut();
            match state.answerer.as_mut() {
                Some(f) => f(&ask),
                None => super::Answer::Refused {
                    reason: "no answerer installed (testing::answer_with)".into(),
                },
            }
        });
        serde_json::to_vec(&answer).unwrap_or_default()
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub fn result(text: &str) {
        super::testing::STATE.with(|s| s.borrow_mut().taken = Some(text.to_owned()));
    }
}

/// Natively: what stands in for Chita.
#[cfg(not(target_arch = "wasm32"))]
pub mod testing {
    use super::{Answer, Ask};
    use std::cell::RefCell;

    type Answerer = Box<dyn FnMut(&Ask) -> Answer>;

    #[derive(Default)]
    pub(crate) struct State {
        pub(crate) answerer: Option<Answerer>,
        pub(crate) logged: Vec<String>,
        pub(crate) taken: Option<String>,
    }

    thread_local! {
        pub(crate) static STATE: RefCell<State> = RefCell::new(State::default());
    }

    /// Installs what answers the extension's asks on this thread.
    pub fn answer_with(f: impl FnMut(&Ask) -> Answer + 'static) {
        STATE.with(|s| s.borrow_mut().answerer = Some(Box::new(f)));
    }

    /// What the extension logged so far on this thread.
    pub fn logged() -> Vec<String> {
        STATE.with(|s| s.borrow().logged.clone())
    }

    /// The last result given back on this thread, taking it.
    pub fn taken() -> Option<String> {
        STATE.with(|s| s.borrow_mut().taken.take())
    }
}

/// The argument buffer the host writes into: one, reused (an argument is read at the start
/// of the call it's for).
#[doc(hidden)]
pub mod memory {
    use std::cell::RefCell;

    thread_local! {
        static ARGUMENT: RefCell<Vec<u8>> = const { RefCell::new(Vec::new()) };
    }

    /// Room for `len` bytes; where the host should write them.
    pub fn alloc(len: usize) -> *mut u8 {
        ARGUMENT.with(|a| {
            let mut a = a.borrow_mut();
            a.clear();
            a.resize(len, 0);
            a.as_mut_ptr()
        })
    }

    /// The bytes the host wrote at `ptr`.
    ///
    /// # Safety
    /// `ptr` and `len` must describe memory the host wrote, from `alloc`.
    pub unsafe fn argument<'a>(ptr: *const u8, len: usize) -> &'a [u8] {
        if ptr.is_null() || len == 0 {
            return &[];
        }
        // SAFETY: the caller's contract: `len` initialised bytes at `ptr`, untouched for the
        // call.
        unsafe { std::slice::from_raw_parts(ptr, len) }
    }

    /// Logs a panic instead of losing it (the host sees only a trap).
    pub fn install_panic_hook() {
        use std::sync::Once;
        static ONCE: Once = Once::new();
        ONCE.call_once(|| {
            std::panic::set_hook(Box::new(|info| {
                super::log(&format!("the extension panicked: {info}"));
            }));
        });
    }
}

/// The entry points Chita calls, from plain functions:
///
/// ```ignore
/// chita_ext_sdk::extension! {
///     command: my_command,   // fn(CommandArgument) -> CommandResult
///     column: my_column,     // fn(ColumnArgument) -> Vec<String>
///     panel: my_panel,       // fn(PanelArgument) -> Panel
/// }
/// ```
///
/// With a context: `context: my_context` (fn(ContextArgument) -> Context) and
/// `screen: my_screen` (fn(ScreenArgument) -> Screen). Any may be left out when the
/// manifest doesn't contribute it. The functions need names other than the exports
/// (`command`, `column`, `panel`, `context`, `screen`).
#[macro_export]
macro_rules! extension {
    ($($kind:ident : $f:path),* $(,)?) => {
        /// Where the host puts a call's argument.
        #[unsafe(no_mangle)]
        pub extern "C" fn alloc(len: i32) -> *mut u8 {
            $crate::memory::alloc(usize::try_from(len).unwrap_or(0))
        }
        $($crate::extension!(@one $kind $f);)*
    };
    (@one command $f:path) => {
        // The ABI: the host passes what it wrote through `alloc`.
        #[allow(clippy::not_unsafe_ptr_arg_deref)]
        #[unsafe(no_mangle)]
        pub extern "C" fn command(ptr: *const u8, len: usize) {
            $crate::memory::install_panic_hook();
            // SAFETY: the host wrote `len` bytes at `ptr`, which came from `alloc`.
            let json = unsafe { $crate::memory::argument(ptr, len) };
            let argument: $crate::CommandArgument = $crate::argument(json);
            let result: $crate::CommandResult = $f(argument);
            $crate::result_json(&result);
        }
    };
    (@one column $f:path) => {
        // The ABI: the host passes what it wrote through `alloc`.
        #[allow(clippy::not_unsafe_ptr_arg_deref)]
        #[unsafe(no_mangle)]
        pub extern "C" fn column(ptr: *const u8, len: usize) {
            $crate::memory::install_panic_hook();
            // SAFETY: the host wrote `len` bytes at `ptr`, which came from `alloc`.
            let json = unsafe { $crate::memory::argument(ptr, len) };
            let argument: $crate::ColumnArgument = $crate::argument(json);
            let texts: Vec<String> = $f(argument);
            $crate::result_json(&texts);
        }
    };
    (@one cells $f:path) => {
        // The ABI: the host passes what it wrote through `alloc`. A column answered with
        // cells (a symbol, a colour, dimmed) instead of plain texts.
        #[allow(clippy::not_unsafe_ptr_arg_deref)]
        #[unsafe(no_mangle)]
        pub extern "C" fn column(ptr: *const u8, len: usize) {
            $crate::memory::install_panic_hook();
            // SAFETY: the host wrote `len` bytes at `ptr`, which came from `alloc`.
            let json = unsafe { $crate::memory::argument(ptr, len) };
            let argument: $crate::ColumnArgument = $crate::argument(json);
            let cells: Vec<$crate::ColumnCell> = $f(argument);
            $crate::result_json(&cells);
        }
    };
    (@one panel $f:path) => {
        // The ABI: the host passes what it wrote through `alloc`.
        #[allow(clippy::not_unsafe_ptr_arg_deref)]
        #[unsafe(no_mangle)]
        pub extern "C" fn panel(ptr: *const u8, len: usize) {
            $crate::memory::install_panic_hook();
            // SAFETY: the host wrote `len` bytes at `ptr`, which came from `alloc`.
            let json = unsafe { $crate::memory::argument(ptr, len) };
            let argument: $crate::PanelArgument = $crate::argument(json);
            let panel: $crate::Panel = $f(argument);
            $crate::result_json(&panel);
        }
    };
    (@one context $f:path) => {
        // The ABI: the host passes what it wrote through `alloc`.
        #[allow(clippy::not_unsafe_ptr_arg_deref)]
        #[unsafe(no_mangle)]
        pub extern "C" fn context(ptr: *const u8, len: usize) {
            $crate::memory::install_panic_hook();
            // SAFETY: the host wrote `len` bytes at `ptr`, which came from `alloc`.
            let json = unsafe { $crate::memory::argument(ptr, len) };
            let argument: $crate::ContextArgument = $crate::argument(json);
            let context: $crate::Context = $f(argument);
            $crate::result_json(&context);
        }
    };
    (@one screen $f:path) => {
        // The ABI: the host passes what it wrote through `alloc`.
        #[allow(clippy::not_unsafe_ptr_arg_deref)]
        #[unsafe(no_mangle)]
        pub extern "C" fn screen(ptr: *const u8, len: usize) {
            $crate::memory::install_panic_hook();
            // SAFETY: the host wrote `len` bytes at `ptr`, which came from `alloc`.
            let json = unsafe { $crate::memory::argument(ptr, len) };
            let argument: $crate::ScreenArgument = $crate::argument(json);
            let screen: $crate::Screen = $f(argument);
            $crate::result_json(&screen);
        }
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn asks_go_to_the_answerer_and_results_are_kept() {
        testing::answer_with(|ask| match ask {
            Ask::ReadFile { path } if path == "a" => Answer::File {
                contents: b"hi".to_vec(),
            },
            Ask::RunTool { tool, args } => Answer::Tool {
                status: 0,
                stdout: format!("{tool} {}", args.join(" ")),
                stderr: String::new(),
            },
            _ => Answer::Refused {
                reason: "no".into(),
            },
        });
        assert_eq!(read_file("a"), Ok(b"hi".to_vec()));
        assert_eq!(read_file("b"), Err("no".into()));
        assert_eq!(list_folder("x"), Err("no".into()));
        assert_eq!(
            run_tool("git", &["status"]).map(|o| o.stdout),
            Ok("git status".into())
        );
        log("one");
        assert_eq!(testing::logged(), vec!["one"]);
        result_json(&CommandResult::message("done"));
        assert_eq!(testing::taken().as_deref(), Some(r#"{"message":"done"}"#));
        assert_eq!(testing::taken(), None);
    }

    #[test]
    fn the_argument_buffer_is_reused() {
        let p = memory::alloc(3);
        // SAFETY: three bytes were just allocated at `p`.
        unsafe {
            p.copy_from(b"abc".as_ptr(), 3);
            assert_eq!(memory::argument(p, 3), b"abc");
        }
        let q = memory::alloc(5);
        // SAFETY: `q` has five bytes.
        unsafe {
            assert_eq!(memory::argument(q, 5), &[0; 5]);
            assert!(memory::argument(std::ptr::null(), 5).is_empty());
        }
        let a: CommandArgument = argument(br#"{"command":"c","selection":["/x"]}"#);
        assert_eq!(a.command, "c");
        assert_eq!(a.selection, vec!["/x"]);
        let bad: CommandArgument = argument(b"nope");
        assert_eq!(bad, CommandArgument::default());
    }
}
