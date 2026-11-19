# InfraWerf plugin views and the view message API v1

From plugin API v2, a `workspace_tab` contribution may carry a `view`: a web page the plugin ships. InfraWerf shows it as a tab in a server's workspace, in a view isolated from InfraWerf itself. The finished example is [`examples/host-dashboard`](../../examples/host-dashboard).

```json
{
  "id": "dashboard",
  "point": "workspace_tab",
  "title": "Host dashboard",
  "capabilities": ["read_saved_state"],
  "view": { "entry": "view/index.html" }
}
```

| Field | Required | Rule |
| --- | --- | --- |
| `entry` | yes | The page to open: a path inside the plugin's folder, written with `/`, ending in `.html`, with no `..` and no name starting with a dot. |

A wrong entry, or a `view` on another point, is refused with `invalid_view`.

## When a view appears

The tab appears in a server's workspace when the plugin is enabled, unchanged since review, its files intact, and the owner allowed on that server every capability the view's contribution lists. It opens only when the owner selects the tab.

## What a view can and cannot do

The page runs in its own webview with its own origin, `http://127.0.0.1:<port>`, served by InfraWerf from the plugin's folder under a secret path that changes every time the view opens.

- **It cannot call InfraWerf.** Its origin is not InfraWerf's, so the webview refuses every InfraWerf command, and every command also checks that its caller is InfraWerf's own page. InfraWerf's internals are removed from the page before it runs, and an attempt to use them is reported.
- **It stays on its own origin.** Every response carries a content security policy that allows scripts, styles, images, fonts and requests from the view's own origin only: no other site, no frames, no workers, no forms and no plugins. A link or redirect to anywhere else, including `blob:`, `data:` and `about:` documents, is refused, a new window is refused, and downloads are refused. Blocked attempts are reported.
- **No WebRTC.** WebRTC can send data to any address and is not covered by the content security policy, and WebView2 has no setting that turns it off. InfraWerf removes it from the view's page and from every frame the page creates, before any of the page's own code runs. This layer is a script, so InfraWerf's own tests check it in every kind of frame a page can make.
- **Nothing persists.** The view runs in a private profile: cookies and storage are gone when it closes.
- **It shows what was installed.** When the view opens, InfraWerf reads the plugin's files once, refusing links, and checks them against the hash recorded at install. It serves only those files, by their exact names, until the view closes.
- **It sees only what the owner allows.** The message API below checks each call against the owner's permissions at that moment and logs it like any plugin call.
- **It cannot run anything by itself.** It may ask; InfraWerf shows the owner the exact command in its own dialog, outside the view, and runs it only if the owner presses Run there.

The view closes when the owner closes the workspace or selects another tab, presses **Lock view**, or changes what plugins may do: disabling the plugin, changing its permissions, revoking trust, or turning all plugins off. Its waiting requests are then cancelled.

**The activity log.** Blocked attempts ("Its view tried something not allowed"), refused requests and refused reads are logged, at most 40 rows per opening of a view. Anything beyond that is counted, and one row with the count is added when the view closes, so a page cannot flood the log. Requests that never reached the view, such as one without the secret path, can come from any program on this PC: they are counted in a separate row and not charged to the plugin. Allowed reads are not logged one by one, because a page may check often.

**The owner's dialog.** When a view asks to run a command, InfraWerf hides the view and shows the plugin, the server and the exact command. **Run** stays disabled for a second, and again whenever InfraWerf's window regains focus, so a click aimed at the plugin's page cannot land on it; **Decline** has the focus. Requests from several views wait in turn.

## The message API, version 1

All calls are relative to the page: `../infrawerf/v1/<call>` from `view/index.html`, or build the address with `new URL("../infrawerf/v1/context", location.href)`. Requests must come from the view itself. Answers are JSON. A refusal has an HTTP error status and `{"error": "<code>", "message": "<sentence>"}`.

| Call | Needs | Answer |
| --- | --- | --- |
| `GET context` | nothing; `read_saved_state` for `server` | `api_version` (1), `plugin_id`, `contribution_id`, and `server` with `id`, `name`, `host`, or `null` without `read_saved_state`. |
| `GET observations` | `read_saved_state` on the server | This plugin's pack commands allowed on the server: `contribution_id`, `title`, `health_check`, `parameters` (each with `name`, `source` and the `choices` it may take, as `id` and `name`) and its latest `observation`, or `null`. |
| `GET inventory` | `read_saved_state` on the server | `{"inventory": ...}`: the server's saved inventory, or `null`. |
| `GET jobs` | `read_saved_state` on the server | This plugin's jobs on the server: `contribution_id`, `title`, `schedule` (`every_minutes`, `next_run_at`, `last_run_at`, `paused`) or `null`, and `latest`: the newest run's `started_at`, `finished_at`, `state`, `exit_code` and the job's own `result`, or `null`. Added in InfraWerf 0.1.13. |
| `POST actions` | the command allowed on the server | Body `{"contribution_id": "...", "parameters": {"name": "<workload id>"}}`. Asks the owner to run one of this plugin's pack commands. Answers `{"id": "...", "state": "waiting"}`. One request may wait per view. |
| `GET actions/<id>` | nothing | `state`: `waiting`, `running`, `declined`, `done`, `failed` or `cancelled`, and the `observation` once done. |

Error codes: `not_granted` (403), `unknown_workload` (403), `bad_request` (400), `bad_parameters` (400), `busy` (409), `unknown_action` (404), `unknown_call` (404), `unknown_server` (404), `closed` (410), `cross_origin` (403), `unavailable` (500).

### Versioning

The message API version is in every path (`v1`) and in `context`. Within version 1, calls and fields are only added, never changed or removed, and a page should ignore fields it does not know. A change that could break a page gets a new version beside the old one, which keeps working for as long as plugin API v2 is accepted.
