// MIT License
// Copyright (c) 2021-2024 Zhenbo Li

const fireSeqSearchDomId = "fireSeqSearchDom";


const fireSeqSearchScriptCSS = `
    #fireSeqSearchDom {
        margin: 1em 0.5em;
        padding: 0.8em 1em 1em;
        border: 1px solid #dadce0;
        border-radius: 8px;
        background-color: #f8f9fa;
        box-shadow: 0 1px 2px rgba(60, 64, 67, 0.06);
        color: #1f1f1f;
        font-family: arial, sans-serif;
        font-size: 14px;
    }
    #fireSeqSearchDom.experimentalLayout {
        position: fixed;
        top: 140px;
        right: 12px;
        width: 200px;
        background-color: hsla(200, 40%, 96%, .8);
        font-size: 12px;
        border-radius: 6px;
        z-index: 99999;
    }
    .fireSeqSearchTitleBar {
        display: flex;
        align-items: center;
        gap: 0.75em;
        flex-wrap: wrap;
        margin: 0 0 0.6em 0;
    }
    .fireSeqSearchHitCount {
        font-size: 13px;
        color: #70757a;
    }
    .fireSeqSearchHitCount b {
        color: #1f1f1f;
        font-weight: 500;
    }
    .fireSeqSearchViewToggle {
        display: inline-flex;
        border: 1px solid #dadce0;
        border-radius: 999px;
        overflow: hidden;
        background: #fff;
    }
    .fireSeqSearchViewToggle button {
        border: 0;
        background: transparent;
        padding: 4px 12px;
        font-size: 13px;
        font-family: arial, sans-serif;
        color: #4d5156;
        cursor: pointer;
        line-height: 1.4;
    }
    .fireSeqSearchViewToggle button + button {
        border-left: 1px solid #dadce0;
    }
    .fireSeqSearchViewToggle button.active {
        background: #0b57d0;
        color: #fff;
    }
    .fireSeqSearchTitleBarSpacer {
        flex: 1;
    }
    #fireSeqSearchDom ul {
        margin: 0;
        padding: 0.2em 0 0;
        list-style: none;
        line-height: 1.58;
    }
    #fireSeqSearchDom ul li {
        font-size: 14px;
    }
    #fireSeqSearchDom ul li + li {
        margin-top: 0.4em;
    }
    #fireSeqSearchDom ul li a {
        color: #1f1f1f;
        text-decoration: none;
    }
    #fireSeqSearchDom ul li a:hover {
        text-decoration: underline;
        text-decoration-thickness: 1px;
        text-underline-offset: 2px;
    }
    .fireSeqSearchHitSummary {
        color: #4d5156;
        margin-left: 0.4em;
    }
    .fireSeqSearchHitSummary::before {
        content: "\\2014\\00A0";
        color: #bdc1c6;
        margin-right: 0.2em;
    }
    .fireSeqSearchHighlight {
        padding: 0 4px;
        color: black !important;
        background-color: gold;
        border-radius: 3px;
    }
    .fireSeqSearchAskBox {
        display: inline-flex;
        align-items: stretch;
        border: 1px solid #dadce0;
        border-radius: 999px;
        overflow: hidden;
        background: #fff;
    }
    .fireSeqSearchAskInput {
        border: 0;
        outline: 0;
        padding: 4px 12px;
        font-size: 13px;
        font-family: arial, sans-serif;
        color: #1f1f1f;
        background: transparent;
        min-width: 220px;
    }
    .fireSeqSearchAskBtn {
        border: 0;
        background: #0b57d0;
        color: #fff;
        padding: 5px 14px;
        font-size: 13px;
        font-family: arial, sans-serif;
        cursor: pointer;
    }
    .fireSeqSearchAskBtn:hover:not(:disabled) {
        background: #0842a0;
    }
    .fireSeqSearchAskBtn:disabled {
        background: #9aa0a6;
        cursor: default;
    }
    .fireSeqSearchAskAnswer {
        margin: 0.5em 0;
        padding: 0.6em 0.8em;
        border-left: 3px solid #6aa3c4;
        background-color: hsla(200, 40%, 96%, .6);
        font-size: 0.95em;
        line-height: 1.5em;
        white-space: pre-wrap;
    }
    .fireSeqSearchAskAnswer.lowConfidence {
        border-left-color: #c4a86a;
        background-color: hsla(40, 40%, 96%, .6);
    }
    .fireSeqSearchAskNote {
        display: block;
        margin-bottom: 0.3em;
        font-size: 0.85em;
        color: gray;
    }
    .fireSeqSearchCiteBadge {
        margin-left: 0.3em;
        font-size: 0.8em;
        color: #0b57d0;
        font-weight: 500;
    }
    .fireSeqSearchAskExtras {
        margin: 0.3em 0 0.6em 0;
        font-size: 0.85em;
        color: #4d5156;
    }
    .fireSeqSearchAskExtras a {
        color: #0b57d0;
        text-decoration: none;
        margin-right: 0.6em;
    }
    .fireSeqSearchAskExtras a:hover {
        text-decoration: underline;
    }
    `;

function consoleLogForDebug(message) {
    console.log(message); //skipcq: JS-0002
}


function addGlobalStyle(css) {
    const head = document.getElementsByTagName("head")[0];
    if (!head) { return; }
    const style = document.createElement("style");
    style.id = "fireSeqSearchScriptCSS";
    // style.type = "text/css";
    style.innerHTML = css;
    head.appendChild(style);
}


function createElementWithText(type, text) {
    const element = document.createElement(type);
    element.textContent = text;
    return element;
}


function createHrefToLogseq(record, serverInfo) {
    const name = serverInfo.notebook_name;

    const title = record.title;
    const prettyTitle = title.replaceAll("%2F", "/");

    const target = record.logseq_uri || `logseq://graph/${name}?page=${title}`;

    const logseqPageLink = document.createElement('a');
    const text = document.createTextNode(prettyTitle);
    logseqPageLink.appendChild(text);
    logseqPageLink.title = prettyTitle;
    logseqPageLink.href = target;
    consoleLogForDebug(logseqPageLink);
    return logseqPageLink;
}


function checkUserOptions() {
    return Promise.all([
        /*global browser */
        browser.storage.sync.get("debugStr"),
        browser.storage.sync.get("ExperimentalLayout"),
        browser.storage.sync.get("ShowHighlight"),
        browser.storage.sync.get("ShowScore")
    ]).then(function(res) {
        consoleLogForDebug(res);

        const options = {
            debugStr: res[0].debugStr,
            ExperimentalLayout: res[1].ExperimentalLayout,
            ShowHighlight: res[2].ShowHighlight,
            ShowScore: res[3].ShowScore
        }
        return options;
    });
}


function parseRawList(rawSearchResult) {
    const hits = [];
    for (const rawRecord of rawSearchResult) {
        const record = (typeof rawRecord === "string") ? JSON.parse(rawRecord) : rawRecord;
        hits.push(record);
    }
    return hits;
}

// The /query response shape — and the whole LLM-first contract this addon
// renders — is what shipped from this backend version on. A backend older than
// this (including any pre-`version` backend, which omits the field) speaks a
// contract we can't render, so we show an update notice instead of mis-parsing
// its response. Bump this in lockstep with any breaking /query|/server_info
// change.
const MIN_BACKEND_VERSION = "0.2.2";

function parseSemver(v) {
    const m = /^(\d+)\.(\d+)\.(\d+)/.exec(String(v == null ? "" : v));
    return m ? [Number(m[1]), Number(m[2]), Number(m[3])] : null;
}

// a < b ? Unparseable/absent `a` counts as "older than anything".
function semverLt(a, b) {
    const pa = parseSemver(a), pb = parseSemver(b);
    if (!pa) { return true; }
    if (!pb) { return false; }
    for (let i = 0; i < 3; i++) {
        if (pa[i] !== pb[i]) { return pa[i] < pb[i]; }
    }
    return false;
}

function backendTooOld(serverInfo) {
    return semverLt(serverInfo && serverInfo.version, MIN_BACKEND_VERSION);
}

function createOutdatedBackendDom(gotVersion) {
    const div = document.createElement("div");
    div.setAttribute("id", fireSeqSearchDomId);
    const bar = createElementWithText("div", "");
    bar.classList.add("fireSeqSearchTitleBar");
    bar.innerHTML = "<span>fireSeqSearch: your local backend (<b>" + escapeHtml(gotVersion)
        + "</b>) is older than this extension needs (≥ <b>" + MIN_BACKEND_VERSION
        + "</b>). Please update <code>fire_seq_search_server</code>.</span>";
    div.appendChild(bar);
    return div;
}

// Where in the search-results page our DOM gets inserted. Lifted to module
// scope so the outdated-backend notice can reuse it without going through the
// full result-rendering path.
function insertDivToWebpage(result) {
    let contextId = "rcnt";
    if (window.location.host.includes("duckduckgo.com")) { // https://github.com/Endle/fireSeqSearch/issues/103
        contextId = "web_content_wrapper";
    }
    if (window.location.host.includes("searx")) {
        contextId = "results";
    }
    if (window.location.host.includes("metager")) { // https://github.com/Endle/fireSeqSearch/issues/127
        contextId = "results";
    }
    const anchor = document.getElementById(contextId);
    if (anchor === null) {
        consoleLogForDebug("fireSeqSearch: couldn't find insertion anchor #" + contextId);
        return;
    }
    anchor.insertAdjacentElement("beforebegin", result);
}

// The addon auto-updates via AMO, but the user may run an older backend.
// New backends advertise `version` + a `capabilities` list in /server_info;
// older ones have neither. Treat "no capabilities field" as "only the original
// /query path is guaranteed", so we never call an endpoint the backend doesn't
// have.
function detectBackendCapabilities(serverInfo) {
    const caps = Array.isArray(serverInfo && serverInfo.capabilities)
        ? serverInfo.capabilities : [];
    const advertised = caps.length > 0;
    return {
        version: (serverInfo && serverInfo.version) || "unknown",
        list: caps,
        // POST /ask — only ever present on a capabilities-aware backend.
        hasAsk: advertised && caps.includes("ask"),
    };
}

function escapeHtml(s) {
    return String(s)
        .replaceAll("&", "&amp;").replaceAll("<", "&lt;").replaceAll(">", "&gt;")
        .replaceAll('"', "&quot;");
}

// Find a hit-list row whose link title (raw note title) matches `title`.
// Source titles from /ask and row titles from /query can differ in %2F
// encoding, so normalize before comparing.
function findRowByTitle(root, title) {
    const list = root && root.querySelector(".fireSeqSearchHitList");
    if (!list) { return null; }
    const norm = String(title || "").replaceAll("%2F", "/");
    for (const li of list.children) {
        const a = li.querySelector("a");
        if (!a) { continue; }
        if (a.textContent === norm || a.getAttribute("title") === norm) { return li; }
    }
    return null;
}

function clearCitationDecorations(root) {
    if (!root) { return; }
    for (const el of root.querySelectorAll(".fireSeqSearchCiteBadge")) { el.remove(); }
    for (const el of root.querySelectorAll(".fireSeqSearchAskExtras")) { el.remove(); }
}

// Tag matching rows in the snippet list with a `[N]` badge for each cited
// source. Sources not present in the list are listed under `answerBox` as
// "Other cited: [N] Title …" links so the user can still reach them.
// `answerText` is the final answer prose; sources whose `[N]` marker does
// not appear in it were retrieved but ignored by the LLM, and are skipped
// so the badge always means "the answer used this".
function decorateRowsWithCitations(root, sources, serverInfo, answerBox, answerText) {
    clearCitationDecorations(root);
    if (!sources || sources.length === 0) { return; }
    const cited = new Set();
    const re = /\[(\d+)\]/g;
    let m;
    while ((m = re.exec(answerText || "")) !== null) { cited.add(parseInt(m[1], 10)); }
    const missing = [];
    sources.forEach(function (src, i) {
        const n = i + 1;
        if (!cited.has(n)) { return; }
        const row = findRowByTitle(root, src.title);
        if (row) {
            const badge = document.createElement("span");
            badge.classList.add("fireSeqSearchCiteBadge");
            badge.textContent = "[" + n + "]";
            const link = row.querySelector("a");
            if (link) { link.insertAdjacentElement("afterend", badge); }
        } else {
            missing.push({ n: n, src: src });
        }
    });
    if (missing.length > 0) {
        const extras = document.createElement("div");
        extras.classList.add("fireSeqSearchAskExtras");
        extras.appendChild(document.createTextNode("Other cited: "));
        missing.forEach(function (item) {
            const a = document.createElement("a");
            const uri = item.src.logseq_uri
                || ("logseq://graph/" + serverInfo.notebook_name + "?page=" + item.src.title);
            a.href = uri;
            a.textContent = "[" + item.n + "] " + String(item.src.title || "").replaceAll("%2F", "/");
            extras.appendChild(a);
        });
        answerBox.insertAdjacentElement("afterend", extras);
    }
}

// Turn `[N]` citation markers in the answer into links to the Nth /ask source.
function linkifyCitations(escapedAnswer, sources, serverInfo) {
    return escapedAnswer.replace(/\[(\d+)\]/g, function (whole, n) {
        const src = sources[parseInt(n, 10) - 1];
        if (!src) { return whole; }
        const uri = src.logseq_uri
            || `logseq://graph/${serverInfo.notebook_name}?page=${src.title}`;
        const title = escapeHtml((src.title || "").replaceAll("%2F", "/"));
        return `<a href="${escapeHtml(uri)}" title="${title}">[${n}]</a>`;
    });
}

// Consume the SSE-over-POST stream from /ask. Resolves when the stream ends;
// reports failures via onError rather than rejecting, so callers don't have to
// double-handle. Uses fetch + a ReadableStream reader because EventSource is
// GET-only and /ask is a POST.
async function streamAsk(question, handlers) {
    const { onMeta, onDelta, onDone, onError } = handlers;
    let resp;
    try {
        resp = await fetch("http://127.0.0.1:3030/ask", {
            method: "POST",
            headers: { "Content-Type": "application/json" },
            body: JSON.stringify({ question }),
        });
    } catch (e) { onError(e); return; }
    if (!resp.ok || !resp.body) { onError(new Error("HTTP " + resp.status)); return; }

    const dispatch = function (name, dataStr) {
        let data;
        try { data = JSON.parse(dataStr); } catch (e) { data = dataStr; }
        if (name === "meta") { onMeta(data); }
        else if (name === "delta") { onDelta(data); }
        else if (name === "done") { onDone(data); }
        else if (name === "error") { onError(new Error((data && data.message) || "ask error")); }
    };

    const reader = resp.body.getReader();
    const decoder = new TextDecoder();
    let buf = "";
    for (;;) {
        let chunk;
        try { chunk = await reader.read(); } catch (e) { onError(e); return; }
        if (chunk.done) { break; }
        buf += decoder.decode(chunk.value, { stream: true });
        let sep;
        while ((sep = buf.indexOf("\n\n")) >= 0) {
            const rawEvent = buf.slice(0, sep);
            buf = buf.slice(sep + 2);
            let name = "message";
            const dataLines = [];
            for (const line of rawEvent.split("\n")) {
                if (line.startsWith("event:")) { name = line.slice(6).trim(); }
                else if (line.startsWith("data:")) { dataLines.push(line.slice(5).replace(/^ /, "")); }
            }
            if (dataLines.length > 0) { dispatch(name, dataLines.join("\n")); }
        }
    }
}

function createAskControls(serverInfo, defaultQuestion) {
    const box = document.createElement("div");
    box.classList.add("fireSeqSearchAskBox");
    const input = document.createElement("input");
    input.type = "text";
    input.classList.add("fireSeqSearchAskInput");
    input.placeholder = "Ask your notes…";
    input.value = defaultQuestion || "";
    const btn = createElementWithText("button", "Ask");
    btn.classList.add("fireSeqSearchAskBtn");
    box.appendChild(input);
    box.appendChild(btn);

    const answerBox = createElementWithText("div", "");
    answerBox.classList.add("fireSeqSearchAskAnswer");
    answerBox.style.display = "none";

    function submit() {
        const question = input.value.trim();
        if (btn.disabled || !question) { return; }
        btn.disabled = true;
        btn.textContent = "Asking…";
        answerBox.style.display = "";
        answerBox.classList.remove("lowConfidence");
        answerBox.textContent = "";
        clearCitationDecorations(document.getElementById(fireSeqSearchDomId));
        let sources = [];
        let answerText = "";
        let lowConfidence = false;
        streamAsk(question, {
            onMeta: function (meta) {
                sources = (meta && meta.sources) || [];
                if (meta && meta.confidence === "low") {
                    lowConfidence = true;
                    answerBox.classList.add("lowConfidence");
                }
            },
            onDelta: function (d) {
                answerText += (d && d.text) || "";
                answerBox.textContent = answerText;
            },
            onDone: function (done) {
                if (done && done.confidence === "low") {
                    lowConfidence = true;
                    answerBox.classList.add("lowConfidence");
                }
                const note = lowConfidence
                    ? '<span class="fireSeqSearchAskNote">Weak match — these notes may only be loosely related:</span>'
                    : "";
                answerBox.innerHTML = note + linkifyCitations(escapeHtml(answerText), sources, serverInfo);
                decorateRowsWithCitations(
                    document.getElementById(fireSeqSearchDomId),
                    sources,
                    serverInfo,
                    answerBox,
                    answerText,
                );
            },
            onError: function (err) {
                consoleLogForDebug(err);
                answerBox.textContent = "(ask failed: " + (err && err.message) + ")";
            },
        }).then(function () {
            btn.disabled = false;
            btn.textContent = "Ask";
        });
    }

    btn.onclick = submit;
    input.addEventListener("keydown", function (e) {
        if (e.key === "Enter") { e.preventDefault(); submit(); }
    });

    return { box, answerBox };
}

function createFireSeqDom(serverInfo, parsedSearchResult, caps, searchParameter) {
    const count = parsedSearchResult.length;
    const div = document.createElement("div");
    div.setAttribute("id", fireSeqSearchDomId);

    // POST /ask: only offered when the backend advertises it. Older backends
    // (no `capabilities` field) silently skip this — nothing breaks.
    const ask = (caps.hasAsk && searchParameter)
        ? createAskControls(serverInfo, searchParameter)
        : null;

    const titleBar = document.createElement("div");
    titleBar.classList.add('fireSeqSearchTitleBar');

    const hitCount = document.createElement("span");
    hitCount.classList.add("fireSeqSearchHitCount");
    hitCount.innerHTML = `<b>${count.toString()}</b> notes`;
    hitCount.title = `from ${serverInfo.notebook_name}`;
    titleBar.appendChild(hitCount);

    const toggle = document.createElement("div");
    toggle.classList.add("fireSeqSearchViewToggle");

    function setSummaryState(cl, state) {
        const prop = state ? '' : 'none';
        for (const el of document.querySelectorAll(cl)) {
            el.style.display = prop;
        }
    }
    function makeSegment(label, onActivate) {
        const seg = document.createElement("button");
        seg.textContent = label;
        seg.onclick = function () {
            for (const sib of toggle.children) { sib.classList.remove("active"); }
            seg.classList.add("active");
            onActivate();
        };
        return seg;
    }
    const segHide = makeSegment("Hide", function () {
        setSummaryState(".fireSeqSearchHitSummary", false);
    });
    const segSummary = makeSegment("Summary", function () {
        setSummaryState(".fireSeqSearchHitSummary", true);
    });
    segSummary.classList.add("active"); // default render shows summaries
    toggle.appendChild(segHide);
    toggle.appendChild(segSummary);
    titleBar.appendChild(toggle);

    const spacer = document.createElement("div");
    spacer.classList.add("fireSeqSearchTitleBarSpacer");
    titleBar.appendChild(spacer);

    if (ask) { titleBar.appendChild(ask.box); }

    div.appendChild(titleBar);
    if (ask) { div.appendChild(ask.answerBox); }
    return div;
}

async function appendResultToSearchResult(serverInfo, parsedSearchResult, dom) {
    const firefoxExtensionUserOption = await checkUserOptions();
    consoleLogForDebug('Loaded user option: ' + JSON.stringify(firefoxExtensionUserOption));

    function buildListItems(parsedSearchResult) {
        const hitList = document.createElement("ul");
        hitList.classList.add('fireSeqSearchHitList');
        for (const record of parsedSearchResult) {
            const li =  createElementWithText("li", "");
            li.classList.add('fireSeqSearchHitListItem');
            if (firefoxExtensionUserOption.ShowScore) {
                const score = createElementWithText("span", String(record.score));
                li.appendChild(score);
            }
            const href = createHrefToLogseq(record, serverInfo);
            li.appendChild(href);

            const summary = createElementWithText("span", "");
            summary.innerHTML = record.summary;
            summary.classList.add('fireSeqSearchHitSummary');
            li.appendChild(summary);

            hitList.appendChild(li);
        }
        return hitList;
    }
    const hitList = buildListItems(parsedSearchResult);
    dom.appendChild(hitList);

    if (firefoxExtensionUserOption.ExperimentalLayout) {
        // Inspired by https://twitter.com/rockucn
        // https://greasyfork.org/en/scripts/446492-%E6%90%9C%E7%B4%A2%E5%BC%95%E6%93%8E%E5%88%87%E6%8D%A2%E5%99%A8-search-engine-switcher/code

        dom.classList.add("experimentalLayout");
    }

    insertDivToWebpage(dom);
}

async function mainProcess(fetchResultArray, searchParameter) {
    consoleLogForDebug("main process");

    const serverInfo = fetchResultArray[0];
    const rawSearchResult = fetchResultArray[1];
    consoleLogForDebug(serverInfo);
    const caps = detectBackendCapabilities(serverInfo);
    consoleLogForDebug("Backend version " + caps.version + ", capabilities: " + JSON.stringify(caps.list));
    const parsedSearchResult = parseRawList(rawSearchResult);

    const fireDom = createFireSeqDom(serverInfo, parsedSearchResult, caps, searchParameter);

    appendResultToSearchResult(serverInfo, parsedSearchResult, fireDom);

}


function getSearchParameterFromCurrentPage() {
    let searchParam = "";

    function getSearchParameterOfSearx() {
        const inputBox = document.getElementById("q");
        return inputBox.value;
    }
    function getSearchParameterOfMetager() {
        const urlParams = new URLSearchParams(window.location.search);
        return urlParams.get('eingabe');
    }

    if (window.location.toString().includes("searx")) {
        searchParam = getSearchParameterOfSearx();
    } else if (window.location.toString().includes("metager")) {
        searchParam = getSearchParameterOfMetager();
    } else {
        // https://stackoverflow.com/a/901144/1166518
        const urlParams = new URLSearchParams(window.location.search);
        searchParam = urlParams.get('q');
    }

    consoleLogForDebug(`Got search param: ${searchParam}`);
    return searchParam;
}



(function() {
    const searchParameter = getSearchParameterFromCurrentPage();

    addGlobalStyle(fireSeqSearchScriptCSS);

    //https://gomakethings.com/waiting-for-multiple-all-api-responses-to-complete-with-the-vanilla-js-promise.all-method/
    // Both requests fire in parallel, but we parse /server_info first: if the
    // backend is too old we must NOT try to parse its /query body (an old
    // backend's response shape would just throw or render garbage).
    Promise.all([
        fetch("http://127.0.0.1:3030/server_info"),
        fetch("http://127.0.0.1:3030/query/" + searchParameter)
    ]).then(async function (responses) {
        const serverInfo = await responses[0].json();
        if (backendTooOld(serverInfo)) {
            const got = (serverInfo && serverInfo.version) || "pre-" + MIN_BACKEND_VERSION;
            consoleLogForDebug("fireSeqSearch: backend " + got + " is older than required " + MIN_BACKEND_VERSION + "; showing update notice");
            insertDivToWebpage(createOutdatedBackendDom(got));
            return;
        }
        const rawSearchResult = await responses[1].json();
        await mainProcess([serverInfo, rawSearchResult], searchParameter);
        const highlightedItems = document.querySelectorAll('.fireSeqSearchHighlight');
        consoleLogForDebug(highlightedItems);
        highlightedItems.forEach((element) => {
            element.style.color = 'red';
        });
    }).catch(
        error => {consoleLogForDebug(error)}
    );


})();
