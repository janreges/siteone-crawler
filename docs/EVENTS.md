# Event stream (`--events-file`)

With `--events-file=<file>` the crawler writes a machine-readable account of the run: one JSON
object per line ([NDJSON](https://github.com/ndjson/ndjson-spec)). It is meant for hosts that drive
the crawler — the desktop GUI, CI tooling, scripts — so they never have to parse the human-readable
output, whose wording and table layout may change between versions.

The stream is opt-in: without `--events-file` nothing here runs.

## Transport

- **`--events-file=<file>`** — the file is created (or truncated) before the crawl starts. If it
  cannot be opened, the crawler exits with code `101` before crawling: a host that asked for events
  relies on them. Each line is written and **flushed immediately**, so a host can tail the file
  while the run goes on. Writing is best-effort afterwards: if a write fails (e.g. a full disk), the
  crawler says so once on stderr, stops the stream and carries on with the run.
- **`--control-stdin`** — the crawler reads commands from stdin. A line `stop` (any case) winds the
  crawl down exactly like Ctrl+C: what was crawled is still analysed and reported, and
  `runFinished` says `"outcome":"cancelled"` with `"interrupted":true`. End of input means the same,
  so a crawl cannot outlive the process that started it (do not combine with `< /dev/null`).
- Options are validated before the stream is opened: a configuration error (exit code `101`)
  produces no events at all. Otherwise the first line is `runStarted` and the last one is
  `runFinished`.
- `--hide-progress-bar` hides the per-request AI lines on stderr, not the events.

## Compatibility

- `runStarted.protocol` is the protocol version, currently **`1`**. It is bumped only when the
  shape of an existing event changes incompatibly.
- New event types, new fields and new values (artifact kinds, issue labels, AI task keys) are
  added without a bump. **Ignore event types and fields you do not know.**
- Field names are camelCase. A value that is not known is **left out**, never written as `null`.
- Rates (tokens per second) are numbers with one decimal; everything else is an integer, a boolean
  or a string.

## Events

| `type` | When |
|---|---|
| [`runStarted`](#runstarted) | first line of every run |
| [`phase`](#phase) | a phase of the run starts or ends |
| [`url`](#url) | a URL was crawled |
| [`aiRequest`](#airequest) | an LLM response (or AI cache hit) arrived |
| [`aiProgress`](#aiprogress) | an AI task started, finished a unit of work, or ended |
| [`aiUsage`](#aiusage) | once, after the last AI request of the run |
| [`artifact`](#artifact) | a file or URL the run produced |
| [`issue`](#issue) | a step failed without failing the run |
| [`runFinished`](#runfinished) | last line of every run |

The examples below are real lines written by the crawler in a run over a two-page site with
`--ai-actions=seo --ai-report=ia`, against a local OpenAI-compatible mock that served a response
captured from vLLM (and answered the first request with HTTP 429).

### runStarted

| Field | Type | Meaning |
|---|---|---|
| `protocol` | int | Protocol version (see [Compatibility](#compatibility)). |
| `version` | string | Crawler version, as `--version` prints it. |
| `executedAt` | string | Start time, RFC 3339 with the local offset. |
| `url` | string | The `--url` of the run. |
| `workingDir` | string | The crawler's working directory; relative paths in `artifact` events resolve against it. |

```json
{"type":"runStarted","protocol":1,"version":"2.6.1.20260914-gui-review.3","executedAt":"2026-09-25T19:06:20.727600099+02:00","url":"http://127.0.0.1:8888/","workingDir":"/tmp/evdoc"}
```

### phase

| Field | Type | Meaning |
|---|---|---|
| `name` | string | `crawl`, `ai` or `analysis`. |
| `state` | string | `started` or `finished` (`failed` is reserved; this version does not emit it). |
| `ms` | int? | Duration, on the `finished` of `crawl`. |
| `detail` | string? | Reserved for a human-readable note. |

The phases follow each other: `crawl`, then `ai` (only when `--ai-actions`, `--ai-elaborate` or
`--ai-profile` is used), then `analysis` (analyzers, the AI executive summary of the `summary`
action, exporters).

```json
{"type":"phase","name":"crawl","state":"finished","ms":113}
```

### url

| Field | Type | Meaning |
|---|---|---|
| `url` | string | The crawled URL. |
| `status` | int | HTTP status; negative for a failure without one: `-1` connection error, `-2` timeout, `-4` send error. |
| `contentType` | string | `HTML`, `JS`, `CSS`, `Image`, `Audio`, `Video`, `Font`, `Document`, `JSON`, `XML`, `Redirect` or `Other`. |
| `timeMs` | int | Response time. |
| `size` | int | Body size in bytes (decoded). |
| `cached` | bool | Served from the HTTP cache. |
| `done` | int | URLs crawled so far. |
| `total` | int | URLs known so far (grows as links are discovered). |

```json
{"type":"url","url":"http://127.0.0.1:8888/about.html","status":200,"contentType":"HTML","timeMs":1,"size":172,"cached":false,"done":2,"total":2}
```

### aiRequest

One event per LLM **HTTP attempt** (a retried request gives one event per attempt) and one per
**AI cache hit**, written as the response arrives. It is the structured twin of the per-request line
on stderr:

```
  AI ✓ #3 SEO 2/2 · /about.html · 17 in · 37 out (33 reasoning) · 0.4 s · 92 tok/s
```

| Field | Type | Meaning |
|---|---|---|
| `seq` | int | 1, 2, 3, … in the order the responses arrived. Events are written in this order. |
| `task` | string? | Key of the AI task the request belongs to (see [AI tasks](#ai-tasks)). |
| `label` | string? | Human-readable name of that task. |
| `done` | int? | Units of the task finished when the response arrived (the request's own unit is not counted yet, so the console shows `done + 1`). |
| `total` | int? | Units of the task. |
| `category` | string | The accounting category of the summary's AI token lines, e.g. `SEO analysis`, `AI report (extract)`, `AI profile (synthesis)`. |
| `subject` | string? | What the request is about: a page path (`/about.html`), an area, a section, a chapter heading, a selection round or the host. |
| `provider` | string | `openai`, `anthropic`, `gemini` or `openai-compatible`. |
| `model` | string | The configured model. |
| `attempt` | int | Transport attempt, 1-based. |
| `maxAttempts` | int | Transport attempts allowed for one request. |
| `outcome` | string | `ok`; `retry` (failed, retried: HTTP 429/5xx or a connection error); `error` (failed and not retried); `cacheHit` (answered from `--ai-cache-dir`, no HTTP request). |
| `status` | int? | HTTP status of the response. Absent for cache hits and transport errors. |
| `error` | string? | Why the attempt failed; at most 200 characters, without credentials (a URL it quotes has no userinfo). |
| `inputTokens` | int? | Prompt tokens the provider processed (Anthropic: incl. cache creation and cache reads). |
| `outputTokens` | int? | Generated tokens, **including** reasoning/thinking tokens. |
| `reasoningTokens` | int? | The part of `outputTokens` that was reasoning, when the provider reports it. |
| `cachedInputTokens` | int? | The part of `inputTokens` served from the provider's prompt cache. |
| `reasoningChars` | int? | Characters of reasoning text in the response (reasoning present even when not counted). The text itself is never logged. |
| `ms` | int? | Time of this attempt from sending the request to reading the whole body. Rate-limit waits and retry backoff are not included. Absent for cache hits. |
| `outputTokensPerSecond` | number? | `outputTokens` / `ms`. |
| `totalTokensPerSecond` | number? | (`inputTokens` + `outputTokens`) / `ms`. |
| `finishReason` | string? | The provider's finish reason, e.g. `stop`, `length`, `end_turn`. |

Token counts come from whatever usage block the response carries (OpenAI and OpenAI-compatible
runtimes, Anthropic, Gemini, Ollama, llama.cpp). A response with an unknown or missing usage block
still produces its event — only the token fields are left out. A failed attempt carries no tokens.

```json
{"type":"aiRequest","seq":3,"task":"seo","label":"SEO","done":1,"total":2,"category":"SEO analysis","subject":"/about.html","provider":"openai-compatible","model":"qwen3.8","attempt":1,"maxAttempts":3,"outcome":"ok","status":200,"inputTokens":17,"outputTokens":37,"reasoningTokens":33,"cachedInputTokens":0,"reasoningChars":148,"ms":401,"outputTokensPerSecond":92.3,"totalTokensPerSecond":134.7,"finishReason":"stop"}
```

A retried attempt and a cache hit (from a second run with the same `--ai-cache-dir`):

```json
{"type":"aiRequest","seq":1,"task":"seo","label":"SEO","done":0,"total":2,"category":"SEO analysis","subject":"/","provider":"openai-compatible","model":"qwen3.8","attempt":1,"maxAttempts":3,"outcome":"retry","status":429,"error":"HTTP 429","ms":402}
{"type":"aiRequest","seq":1,"task":"seo","label":"SEO","done":0,"total":2,"category":"SEO analysis","subject":"/","provider":"openai-compatible","model":"qwen3.8","attempt":1,"maxAttempts":3,"outcome":"cacheHit","inputTokens":17,"outputTokens":37,"reasoningTokens":33,"cachedInputTokens":0,"finishReason":"stop"}
```

### aiProgress

| Field | Type | Meaning |
|---|---|---|
| `task` | string | Stable key of the AI task (see [AI tasks](#ai-tasks)). |
| `label` | string | Human-readable name. |
| `done` | int | Units finished — successfully or not. |
| `total` | int | Units of the task. |
| `state` | string | `started` (`done` is 0), `progress` (one more unit done) or `finished`. |

A task that runs to its end reaches `done == total` before `finished`: a failed unit (a page whose
request or answer failed) counts as done too. A task that ends early is `finished` with
`done < total` (e.g. the executive summary stops before its synthesis when every area evaluation
failed). Several tasks may be in progress at the same time.

```json
{"type":"aiProgress","task":"seo","label":"SEO","done":0,"total":2,"state":"started"}
{"type":"aiProgress","task":"seo","label":"SEO","done":1,"total":2,"state":"progress"}
{"type":"aiProgress","task":"seo","label":"SEO","done":2,"total":2,"state":"finished"}
```

#### AI tasks

| Task key | Label | Unit (`subject` of its requests) |
|---|---|---|
| `seo` | SEO | page (page path) |
| `typos` | Typos | page (page path) |
| `custom` | Custom check | page (page path) |
| `llms-txt` / `llms-full` | llms.txt / llms-full.txt | page (page path); one task for both files, keyed `llms-txt` when that action is on |
| `summary` | Executive summary | area evaluation (`security`, `accessibility`, `seo`, `performance`, `infrastructure`), then the `synthesis` |
| `report:<preset>` | Report '&lt;preset&gt;' | page (page path); `<preset>` is `ia`, `quality`, `topics`, `compliance` or `extract` |
| `elaborate:select` | Elaborate: select | selection round (`round 1`, `round 2`; a round 2 that is not needed counts as done) |
| `elaborate:gapfill` | Elaborate: gap-fill | navigation page fetched (plain HTTP, no AI requests) |
| `elaborate:extract` | Elaborate: extract | page (page path) |
| `elaborate:synthesize` | Elaborate: synthesis | prose section (section id), or one call (`all sections`) |
| `elaborate:correct` | Elaborate: correction | one call (`prose`) |
| `profile:summary` | Profile: summary | one call (host) |
| `profile:classify` | Profile: classify | one call (host) |
| `profile:describe` | Profile: describe | page (page path) |
| `profile:localize` | Profile: headings | one call (report language) |
| `profile:chapters` | Profile: chapters | chapter (its heading) — the chapter's page selection, synthesis and correction |
| `profile:executive` | Profile: executive summary | one call (host) |
| `profile:correct` | Profile: correction | one call (`executive summary`) |

Stages that do not run (dry run, forced profile type, a small site that needs no selection,
English headings) start no task.

### aiUsage

The AI totals of the run, once, after the last AI request — after the executive summary, which runs
in the `analysis` phase. Emitted whenever the run uses AI, also when no request was made.

| Field | Type | Meaning |
|---|---|---|
| `provider`, `model` | string | As configured. |
| `calls` | int | Responses with a success (2xx) status, plus cache hits. |
| `cacheHits` | int | Of them answered from the AI cache. Cache hits add no tokens: they were paid for in an earlier run. |
| `httpAttempts` | int | HTTP attempts, successful or not (the `aiRequest` events that are not cache hits). |
| `retries` | int | Attempts that repeated an earlier one: transport retries and re-asks after an unusable answer. |
| `inputTokens`, `outputTokens`, `reasoningTokens`, `cachedInputTokens` | int | Sums over the 2xx responses (the same fields of their `aiRequest` events). |
| `callsWithoutUsage` | int | 2xx responses that reported no token usage. |
| `networkMs` | int | Time spent on AI calls, including rate-limit waits and retry backoff. |

```json
{"type":"aiUsage","provider":"openai-compatible","model":"qwen3.8","calls":4,"cacheHits":0,"httpAttempts":5,"retries":1,"inputTokens":68,"outputTokens":148,"reasoningTokens":132,"cachedInputTokens":0,"callsWithoutUsage":0,"networkMs":3011}
```

### artifact

| Field | Type | Meaning |
|---|---|---|
| `kind` | string | What it is (table below). |
| `label` | string | Human-readable name. |
| `path` | string | File or directory path (for `online` a URL). AI files are announced with absolute paths; others as the exporter wrote them (resolve a relative one against `workingDir`). |

| `kind` | `label` |
|---|---|
| `html` | HTML report |
| `json` | JSON data |
| `text` | Text output |
| `online` | Online HTML report |
| `offline` | Website clone |
| `markdown` | Markdown export |
| `markdown-single` | Single-file Markdown |
| `sitemap-xml` | XML sitemap |
| `sitemap-txt` | Text sitemap |
| `screenshots` | Screenshots |
| `animation` | Animation |
| `llms` | llms.txt |
| `llms-full` | llms-full.txt |
| `ai-report-json`, `ai-report-html` | AI report (JSON), AI report (HTML) |
| `ai-elaborate-md`, `ai-elaborate-json`, `ai-elaborate-html` | Brand profile (Markdown), (JSON), (HTML) |
| `ai-profile-md`, `ai-profile-json`, `ai-profile-html` | AI profile (Markdown), (JSON), (HTML) |

Every AI output file gets its own event as soon as it is written. The AI report, brand elaborate and
AI profile files are written as a set that is rolled back when one of them fails, so their events
follow once the whole set exists.

```json
{"type":"artifact","kind":"ai-report-json","label":"AI report (JSON)","path":"/tmp/evdoc/out/ai-report.ia.127-0-0-1.2026-09-25-19-06-25-763-2411710.json"}
```

### issue

| Field | Type | Meaning |
|---|---|---|
| `kind` | string | `upload`, `mail`, `offline`, `markdown`, `ai` (`other` for anything else). |
| `label` | string | What failed. |
| `detail` | string | The message, as in the summary or on stderr. |

Labels of kind `ai`: `AI phase skipped` (no API key, an unreadable key, no page to analyse),
`AI report skipped` (an invalid report configuration), `AI custom check skipped` (no prompt),
`llms.txt export failed`, `AI executive summary failed`, `Brand elaborate failed`,
`AI profile failed`, `AI report export failed`, `Brand elaborate export failed`,
`AI profile export failed`. A single page whose AI request failed is not an issue: it is an
`aiRequest` with `"outcome":"error"`, still counted in `aiProgress`.

```json
{"type":"issue","kind":"ai","label":"AI phase skipped","detail":"AI phase skipped: AI is enabled but no API key resolved for provider 'anthropic'. Set ANTHROPIC_API_KEY or use --ai-api-key-file."}
```

### runFinished

| Field | Type | Meaning |
|---|---|---|
| `outcome` | string | `success`, `cancelled` (stopped by Ctrl+C or `stop`), `qualityGate` (the `--ci` gate failed) or `failed`. |
| `exitCode` | int | The process exit code (see the README). |
| `ms` | int | Duration of the run. |
| `interrupted` | bool | Whether the crawl was cut short by Ctrl+C or `stop`. |
| `error` | string? | The error of a failed run. |

```json
{"type":"runFinished","outcome":"success","exitCode":0,"ms":3375,"interrupted":false}
```
