# What a manifest can say about where things show

`extension.json` decides where your commands and columns appear. Every field here is
optional, and older versions of Chita ignore the ones they don't know.

## Commands

```json
{ "id": "stage", "title": "Stage", "description": "Adds the selected items to the next commit.",
  "symbol": "plus.circle", "when": "selection", "place": "context", "menu": "items", "group": "Staging" }
```

| Field | Values | Meaning |
|---|---|---|
| `when` | `always` (default), `selection`, `file`, `folder` | What must be true: items selected, exactly one file selected, or a folder on this Mac. |
| `place` | `any` (default), `context`, `outside` | Everywhere, only in a folder one of your contexts owns, or only outside such folders. |
| `menu` | `any` (default), `items`, `folder` | The menu opened on items, or the one opened on the folder (which also means the Extensions menu's enabled commands). |
| `group` | a label | Commands with the same label sit together under it, with a line between groups. Use title case. |
| `symbol` | an SF Symbol | Shown beside the command in menus. |
| `description` | one line | The command's tooltip, and its line in the store. |

One or two commands that apply sit directly in the context menu, with your extension's name
under them. More go into a submenu named after your extension, grouped as you say.

## Columns

```json
{ "id": "state", "title": "Git", "place": "context", "shown": true, "badge": true }
```

| Field | Values | Meaning |
|---|---|---|
| `place` | `any` (default), `context`, `outside` | Where the column shows. A column about a kind of folder should say `context`, or it stands empty everywhere else. Chita doesn't ask you for values where it doesn't show. |
| `shown` | `false` (default), `true` | On from the start where it shows. Otherwise the user turns it on. |
| `badge` | `false` (default), `true` | Its marks also badge the file icons from the start. A cell needs a `symbol` to make a mark. |

The user can always turn a column and its badges on or off in your extension's row in
Settings › Extensions.

## Contexts

A context may add `"folders_first": true`: such a folder lists its folders first, unless the
user chose otherwise for it.

## See it in use

`extensions/it.urial.chita.git/extension.json` uses all of these fields.
