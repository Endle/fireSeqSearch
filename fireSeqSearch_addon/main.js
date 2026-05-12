// MIT License
// Copyright (c) 2021-2024 Zhenbo Li

const fireSeqSearchDomId = "fireSeqSearchDom";


const fireSeqSearchScriptCSS = `
    #fireSeqSearchDom {
        margin: 1em 1em 1em 1em;
        color: var(--theme-col-txt-snippet); /* duckduck color*/
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
        margin: 0.5em 0;
    }
    .hideSummary {
        margin: 0 1em;
    }
    #fireSeqSearchDom ul {
        margin: 0;
        padding: 0.6em;
        border: 1px dotted  gray;
        list-style: none;
        line-height: 1.5em;
    }
    #fireSeqSearchDom ul li {
        font-size: 15px;
    }
    #fireSeqSearchDom ul li + li {
        margin-top: 0.4em;
    }
    #fireSeqSearchDom ul li a {
        text-decoration: underline;
        text-decoration-style: dotted;
        text-decoration-thickness: 1px;
        text-underline-offset: 2px;
    }
    #fireSeqSearchDom ul li::before {
        content: ' ';
        display: inline-block;
        margin-right: 0.4em;
        line-height: 1em;
        width: 1em;
        height: 1em;
        transform: translateY(3px);
        border-radius: 3px;
        background-image: url(data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAACAAAAAgCAMAAABEpIrGAAAAe1BMVEUAKzaFyMiKz88AJjIAKjaHy8sAHiuM0tIAGSgAGykAIC0AESIAFyYAIzAAHCoAFCQAEiBvq6wtVlw0XmQIMDtooqRAbnNdk5VPgYU4Y2lyr7B4trdHc3Z/wcEAHSctUFVimZoaP0hShIcoRksTLjQADBkAABNdjo8AAAzWDdSWAAABBklEQVQ4jc1S7XKDIBCEQ0DEL2oJVqOmiUl8/yfs6XTaePgA2T8w7N5xu3OMvSOESI5ejSnWM1Hqo/lUEa8a708WLyYAgG4zWv+lpQRXsdIDR2hLBGkn8RnazK4nB2+IIO9XQvZ5dsYf4FlHI4StcqiYGqeppvU4u+CogIvaXB56n53WrmBJYSprq5S6wB71ONZM5Cc/AAyuFXEUGF/aDLANg55b6hRRnjX/A6ZCExfTC4+KkBJB6uSrgOtv0iKHHc/hSr3ovUCGcs9bSQS0g7mQGSaSaTLvBNJFSRQ3+JfIfow3L5s7XJyVBR3uR5uZPG7PnsPQXedoJX4hzGNZlnt5VP7G+AHcFwwZX2F8QwAAAABJRU5ErkJggg==);
        background-repeat: no-repeat;
        background-size: 16px;
    }
    .fireSeqSearchHitSummary {
        font-size: 0.9em
    }
    .fireSeqSearchHitSummary::before {
        content: "\\00A0::\\00A0";
    }
    .fireSeqSearchHighlight {
        padding: 0 4px;
        color: black !important;
        background-color: gold;
        border-radius: 3px;
    }
    .fireSeqSearchAsk {
        margin: 0.4em 0;
    }
    .fireSeqSearchAskBtn {
        font-size: 0.9em;
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
        const record = JSON.parse(rawRecord);
        hits.push(record);
    }
    return hits;
}

// The addon auto-updates via AMO, but the user may run an older backend (or
// one with the LLM disabled). New backends advertise `version` + a
// `capabilities` list in /server_info; older ones have neither. Treat "no
// capabilities field" as "only the original /query path is guaranteed", so we
// never call an endpoint the backend doesn't have.
function detectBackendCapabilities(serverInfo) {
    const caps = Array.isArray(serverInfo && serverInfo.capabilities)
        ? serverInfo.capabilities : [];
    const advertised = caps.length > 0;
    return {
        version: (serverInfo && serverInfo.version) || "unknown",
        list: caps,
        // POST /ask — only ever present on a capabilities-aware backend.
        hasAsk: advertised && caps.includes("ask"),
        // The LLM/Summary buttons predate `capabilities` (addon 0.2.x), so on an
        // old backend keep the previous behaviour: show them unless `llm_enabled`
        // is explicitly false. On a new backend, gate on the advertised feature.
        hasLlmSummary: !(serverInfo && serverInfo.llm_enabled === false)
            && (!advertised || caps.includes("llm_summary")),
    };
}

function escapeHtml(s) {
    return String(s)
        .replaceAll("&", "&amp;").replaceAll("<", "&lt;").replaceAll(">", "&gt;")
        .replaceAll('"', "&quot;");
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

function createAskDom(serverInfo, question) {
    const wrap = createElementWithText("div", "");
    wrap.classList.add("fireSeqSearchAsk");

    const btn = createElementWithText("button", "Ask my notes");
    btn.classList.add("fireSeqSearchAskBtn");
    const answerBox = createElementWithText("div", "");
    answerBox.classList.add("fireSeqSearchAskAnswer");
    answerBox.style.display = "none";

    btn.onclick = function () {
        if (btn.disabled || !question) { return; }
        btn.disabled = true;
        btn.textContent = "Asking…";
        answerBox.style.display = "";
        answerBox.classList.remove("lowConfidence");
        answerBox.textContent = "";
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
            },
            onError: function (err) {
                consoleLogForDebug(err);
                answerBox.textContent = "(ask failed: " + (err && err.message) + ")";
            },
        }).then(function () {
            btn.disabled = false;
            btn.textContent = "Ask my notes";
        });
    };

    wrap.appendChild(btn);
    wrap.appendChild(answerBox);
    return wrap;
}

async function processLlmSummary(serverInfo, parsedSearchResult, fireDom) {

    const doneListApi = "http://127.0.0.1:3030/llm_done_list";
    let list = await fetch(doneListApi);
    list = await list.text();
    list = JSON.parse(list);

    const findByTitle = function(title) {
        const ul = fireDom.querySelector( ".fireSeqSearchHitList" );
        if (ul === null)    return null;
        for (const child of ul.children) {
            const liTitle = child.firstChild.text;
            if (title === liTitle) {
                return child;
            }
        }
        return null;
    };
    const setLlmResult = function (title, llmSummary) {
        const targetRow = findByTitle(title);
        if (targetRow === null) {
            consoleLogForDebug("Error! Can't find dom for ", title);
            return;
        }
        if (targetRow.querySelector( ".fireSeqSearchLlmSummary" ) != null) {
            consoleLogForDebug("Skip. We have the summary for ", title);
            return;
        }

        const summary = createElementWithText("span", "");
        summary.innerHTML = llmSummary;
        summary.classList.add('fireSeqSearchLlmSummary');
        targetRow.appendChild(summary);
    };
    for (const record of parsedSearchResult) {
        const title = record.title;
        if (!list.includes(title)) {
            consoleLogForDebug("Not ready, skip" + title);
            continue;
        }
        // TODO remove hard code port
        const llm_api = "http://127.0.0.1:3030/summarize/" + title;
        let sum = await fetch(llm_api);
        sum = await sum.text();
        setLlmResult(title, sum);
    }
}


function createFireSeqDom(serverInfo, parsedSearchResult, caps, searchParameter) {
    const count = parsedSearchResult.length;
    const div = document.createElement("div");
    div.setAttribute("id", fireSeqSearchDomId);

    const createTitleBarDom = function () {
        const titleBar = createElementWithText("div");
        titleBar.classList.add('fireSeqSearchTitleBar');
        const hitCount = `<span>We found <b>${count.toString()}</b> results in your logseq notebook</span>`;
        titleBar.insertAdjacentHTML("afterbegin",hitCount);

        function setSummaryState(cl, state) {
            let prop = 'none';
            if (state) { prop = ''; }
            for (const el of document.querySelectorAll(cl)) {
                el.style.display=prop;
            }
        }
        let btn = document.createElement("button");
        btn.classList.add("hideSummary");
        let text = document.createTextNode("Hide Summary");
        btn.appendChild(text);
        btn.onclick = function () {
            setSummaryState(".fireSeqSearchHitSummary", false);
            setSummaryState(".fireSeqSearchLlmSummary", false);
        };
        titleBar.appendChild(btn);

        btn = document.createElement("button");
        btn.classList.add("showSummary");
        text = document.createTextNode("Summary");
        btn.appendChild(text);
        btn.onclick = function () {
            setSummaryState(".fireSeqSearchHitSummary", true);
            setSummaryState(".fireSeqSearchLlmSummary", false);
        };
        titleBar.appendChild(btn);

        // The LLM-summary button only makes sense if the backend has the LLM
        // wired; an old or LLM-disabled backend just wouldn't answer.
        if (caps.hasLlmSummary) {
            btn = document.createElement("button");
            btn.classList.add("showLlm");
            text = document.createTextNode("LLM");
            btn.appendChild(text);
            btn.onclick = function () {
                setSummaryState(".fireSeqSearchHitSummary", false);
                setSummaryState(".fireSeqSearchLlmSummary", true);
                processLlmSummary(serverInfo, parsedSearchResult, div);
            };
            titleBar.appendChild(btn);
        }
        return titleBar;
    };
    const bar = createTitleBarDom();
    div.appendChild(bar);

    // POST /ask: only offered when the backend advertises it. Older backends
    // (no `capabilities` field) silently skip this — nothing breaks.
    if (caps.hasAsk && searchParameter) {
        div.appendChild(createAskDom(serverInfo, searchParameter));
    }
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

    function insertDivToWebpage(result) {
        let contextId = "rcnt";
        if (window.location.host.includes("duckduckgo.com")) {
            contextId = "web_content_wrapper";
        }
        if (window.location.host.includes("searx")) { // https://github.com/Endle/fireSeqSearch/issues/103
            contextId = "results";
        }
        if (window.location.host.includes("metager")) { // https://github.com/Endle/fireSeqSearch/issues/127
            contextId = "results";
        }
        document.getElementById(contextId).insertAdjacentElement("beforebegin", result);

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
    Promise.all([
        fetch("http://127.0.0.1:3030/server_info"),
        fetch("http://127.0.0.1:3030/query/" + searchParameter)
    ]).then(function (responses) {
        return Promise.all(responses.map(function (response) {return response.json();}));
    }).then(function (data) {
        mainProcess(data, searchParameter);
    }).then((_e) => {
        const highlightedItems = document.querySelectorAll('.fireSeqSearchHighlight');
        consoleLogForDebug(highlightedItems);
        highlightedItems.forEach((element) => {
            element.style.color = 'red';
        });
    }).catch(
        error => {consoleLogForDebug(error)}
    );


})();
