// The Chita SDK for AssemblyScript: what the host gives an extension (log, ask, the
// result) and what it needs from one (alloc, where a call's argument goes).

@external("chita", "log") declare function hostLog(ptr: usize, len: usize): void;
@external("chita", "ask") declare function hostAsk(ptr: usize, len: usize): i32;
@external("chita", "take") declare function hostTake(ptr: usize, cap: i32): i32;
@external("chita", "result") declare function hostResult(ptr: usize, len: usize): void;

function send(text: string, f: (ptr: usize, len: usize) => void): void {
  const bytes = String.UTF8.encode(text);
  f(changetype<usize>(bytes), bytes.byteLength);
}

/** Logs a line (Settings › Extensions shows the last ones). */
export function log(text: string): void { send(text, hostLog); }

/** Gives a call's result back to Chita. */
export function result(text: string): void { send(text, hostResult); }

/** Asks Chita for something, as JSON ({"ReadFile":{"path":…}}, {"ListFolder":{"path":…}},
 *  {"RunTool":{"tool":…,"args":[…]}}); the answer as JSON ({"File":…}, {"Folder":…},
 *  {"Tool":…} or {"Refused":{"reason":…}}). */
export function ask(json: string): string {
  const bytes = String.UTF8.encode(json);
  const len = hostAsk(changetype<usize>(bytes), bytes.byteLength);
  if (len < 0) return "{\"Refused\":{\"reason\":\"no answer\"}}";
  const out = new ArrayBuffer(len);
  hostTake(changetype<usize>(out), len);
  return String.UTF8.decode(out);
}

let argument: ArrayBuffer = new ArrayBuffer(0);

/** Where the host puts a call's argument. */
export function alloc(len: i32): usize {
  argument = new ArrayBuffer(len);
  return changetype<usize>(argument);
}

/** The argument of a call, as text. */
export function text(ptr: usize, len: usize): string {
  return String.UTF8.decodeUnsafe(ptr, len);
}

/** A string as a JSON string literal. */
export function quote(s: string): string {
  return "\"" + s.replaceAll("\\", "\\\\").replaceAll("\"", "\\\"").replaceAll("\n", "\\n") + "\"";
}

/** AssemblyScript's own abort (a failed assertion, an out-of-bounds access): logged, then a trap. */
export function abort(message: usize, file: usize, line: u32, column: u32): void {
  log("the extension aborted at line " + line.toString());
  unreachable();
}
