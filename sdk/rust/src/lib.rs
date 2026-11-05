//! What a Chita extension is written with.
//!
//! An extension is a WebAssembly module (`wasm32-unknown-unknown`, a `cdylib`) that Chita
//! runs in its confined host. It can't open files, use the network or run programs: it asks
//! Chita, which answers from what the user granted ([`ask`], [`read_file`], [`list_folder`],
//! [`run_tool`]). Chita calls into it for what its manifest contributes: a command
//! (`command`, with a [`CommandArgument`], giving back a [`CommandResult`]), a column
//! (`column`, [`ColumnArgument`] → one text per path) and a panel (`panel`,
//! [`PanelArgument`] → [`Panel`]). The [`extension!`] macro writes those entry points and the
//! memory function the host needs, from plain Rust functions.
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
    }

    /// Chita's answer.
    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
    pub enum Answer {
        File {
            contents: Vec<u8>,
        },
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

    /// Nothing to show.
    pub fn nothing() -> Self {
        Self::default()
    }
}

/// One line of text asked of the user.
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
/// Any of the three may be left out when the manifest doesn't contribute it. The functions
/// need names other than `command`, `column` and `panel`: those are the exports.
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
