// ==UserScript==
// @name         fireSeqSearchScript
// @namespace    https://github.com/Endle/fireSeqSearch
// @version      0.0.18
// @description  Everytime you use the search engine, FireSeqSearch searches your personal logseq notes.
// @author       Zhenbo Li
// @match        https://www.google.com/search*
// @match        https://duckduckgo.com/?q=*
// @icon         https://www.google.com/s2/favicons?sz=64&domain=tampermonkey.net
// @grant GM_xmlhttpRequest
// ==/UserScript==

// MIT License
// Copyright (c) 2021-2022 Zhenbo Li

/*global GM*/

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
    `;

function consoleLogForDebug(message) {
    console.log(message); //skipcq: JS-0002
    // Comment it in master branch, to make deepSource happy
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

    const target = `logseq://graph/${name}?page=${title}`;
    const logseqPageLink = document.createElement('a');
    const text = document.createTextNode(prettyTitle);
    logseqPageLink.appendChild(text);
    logseqPageLink.title = prettyTitle;
    logseqPageLink.href = target;
    consoleLogForDebug(logseqPageLink);
    return logseqPageLink;
}


function checkUserOptions() {
    const options = {
        debugStr: "tampermonkey",
        ExperimentalLayout: false,
        ShowHighlight: true,
        ShowScore: false
    }
    consoleLogForDebug(options);
    return options;

}


async function appendResultToSearchResult(fetchResultArray, container) {
    const serverInfo = fetchResultArray[0];
    const rawSearchResult = fetchResultArray[1];
    const firefoxExtensionUserOption = await checkUserOptions();


    consoleLogForDebug(firefoxExtensionUserOption);

    function createTitleBarDom(count) {
        const titleBar = createElementWithText("div");
        titleBar.classList.add('fireSeqSearchTitleBar');
        const hitCount = `<span>We found <b>${count.toString()}</b> results in your logseq notebook</span>`;
        titleBar.insertAdjacentHTML("afterbegin",hitCount);
        const btn = document.createElement("button");
        btn.classList.add("hideSummary");
        const text = document.createTextNode("Hide Summary (Tmp)");
        btn.appendChild(text);
        btn.onclick = function () {
            // alert("Button is clicked");
            for (const el of document.querySelectorAll('.fireSeqSearchHitSummary')) {
                // el.style.visibility = 'hidden';
                el.remove();
            }
        };
        titleBar.appendChild(btn);
        return titleBar;
    }



    function createFireSeqDom() {

        const div = document.createElement("div");
        // div.appendChild(createElementWithText("p", "fireSeqSearch launched!"));
        div.setAttribute("id", fireSeqSearchDomId);


        return div;
    }

    const dom = createFireSeqDom();
    dom.appendChild(createTitleBarDom(rawSearchResult.length));
    consoleLogForDebug(dom);

    const hitList = document.createElement("ul");

    consoleLogForDebug(rawSearchResult);
    for (const rawRecord of rawSearchResult) {
        // const e = document.createTextNode(record);
        consoleLogForDebug(rawRecord);
        const record = JSON.parse(rawRecord);
        consoleLogForDebug(typeof record);

        const li =  createElementWithText("li", "");


        if (firefoxExtensionUserOption.ShowScore) {
            const score = createElementWithText("span", String(record.score));
            li.appendChild(score);
        }
        const href = createHrefToLogseq(record, serverInfo);
        li.appendChild(href);
        li.append(' ')
        if (firefoxExtensionUserOption.ShowHighlight) {
            const summary = createElementWithText("span", "");
            summary.innerHTML = record.summary;
            summary.classList.add('fireSeqSearchHitSummary');
            li.appendChild(summary);
        }
        // let e = wrapRawRecordIntoElement(record, serverInfo);

        // e.style.
        hitList.appendChild(li);
        // consoleLogForDebug("Added an element to the list");
    }
    dom.appendChild(hitList);

    if (firefoxExtensionUserOption.ExperimentalLayout) {
        // Inspired by https://twitter.com/rockucn
        // https://greasyfork.org/en/scripts/446492-%E6%90%9C%E7%B4%A2%E5%BC%95%E6%93%8E%E5%88%87%E6%8D%A2%E5%99%A8-search-engine-switcher/code

        dom.classList.add("experimentalLayout");
    }
    let contextId = "rcnt";
    if (window.location.href.includes("duckduckgo.com")) {
        contextId = "web_content_wrapper";
    }
    document.getElementById(contextId).insertAdjacentElement("beforebegin", dom);

}

function getSearchParameterFromCurrentPage() {
    let searchParam = "";

    function getSearchParameterOfSearx() {
        const inputBox = document.getElementById("q");
        return inputBox.value;
    }

    if (window.location.toString().includes("searx")) {
        searchParam = getSearchParameterOfSearx();
    } else {
        // https://stackoverflow.com/a/901144/1166518
        const urlParams = new URLSearchParams(window.location.search);
        // consoleLogForDebug(urlParams);
        searchParam = urlParams.get('q');
    }

    consoleLogForDebug(`Got search param: ${searchParam}`);
    return searchParam;
}

function waitForContainer() {
    return new Promise((resolve, reject) => {
        const interval = setInterval(() => {
            const container = document.querySelector("#search") // google
            || document.querySelector("#links") // duckduckgo

            if (container) {
                resolve(container)
                clearInterval(interval);
            }
        }, 200)
    });
}


(function() {
    const searchParameter = getSearchParameterFromCurrentPage();

    consoleLogForDebug(searchParameter);
    addGlobalStyle(fireSeqSearchScriptCSS);

    GM.xmlHttpRequest({
        method: "GET",
        url: "http://127.0.0.1:3030/server_info",
        onload(infoResponse) {
            const server_info = JSON.parse(infoResponse.responseText);
            consoleLogForDebug(server_info);
            GM.xmlHttpRequest({
                method: "GET",
                url: `http://127.0.0.1:3030/query/${searchParameter}`,
                onload(queryResponse) {
                    const hit = JSON.parse(queryResponse.responseText);
                    // consoleLogForDebug(hit);
                    consoleLogForDebug(typeof hit);

                    appendResultToSearchResult([server_info, hit])
                        .then((_e) => {
                            const highlightedItems = document.querySelectorAll('.fireSeqSearchHighlight');
                            consoleLogForDebug(highlightedItems);
                        })
                        .catch(error => {
                            consoleLogForDebug(error);
                        });

                }
            });
        }
    });

    /*
        //https://gomakethings.com/waiting-for-multiple-all-api-responses-to-complete-with-the-vanilla-js-promise.all-method/
        Promise.all([
            fetch("http://127.0.0.1:3030/server_info"),
            fetch(`http://127.0.0.1:3030/query/${searchParameter}`)
        ]).then(function (responses) {
            return Promise.all(responses.map(function (response) {return response.json();}));
        }).then(function (data) {
            consoleLogForDebug(data);
            return appendResultToSearchResult(data);
        }).then((_e) => {
            const highlightedItems = document.querySelectorAll('.fireSeqSearchHighlight');
            consoleLogForDebug(highlightedItems);
            highlightedItems.forEach((element) => {
                element.style.color = 'red';
            });
        }).catch(function (error) {
            consoleLogForDebug(error);
        });



     */



})();
