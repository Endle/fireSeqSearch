// ==UserScript==
// @name         fireSeqSearchScript
// @namespace    https://github.com/Endle/fireSeqSearch
// @version      0.0.16
// @description  Everytime you use the search engine, FireSeqSearch searches your personal logseq notes.
// @author       Zhenbo Li
// @match        https://www.google.com/search*
// @match        https://duckduckgo.com/?q=*
// @icon         https://www.google.com/s2/favicons?sz=64&domain=tampermonkey.net
// @grant GM_xmlhttpRequest
// ==/UserScript==

// MIT License
// Copyright (c) 2021-2022 Zhenbo Li

const fireSeqSearchDomId = "fireSeqSearchDom";

function consoleLogForDebug(s) {
    console.log(s);
    // Comment it in master branch, to make deepSource happy
}

function createElementWithText(type, text) {
    let x = document.createElement(type);
    x.textContent = text;
    return x;
}

function createHrefToLogseq(record, serverInfo) {
    const name = serverInfo.notebook_name;

    const title = record.title;
    const prettyTitle = title.replaceAll("%2F", "/");
    const target = "logseq://graph/" + name + "?page=" + title;
    let a = document.createElement('a');
    a.style.textDecoration = 'underline';
    let text = document.createTextNode(prettyTitle);
    a.appendChild(text);
    a.title = prettyTitle;
    a.href = target;
    consoleLogForDebug(a);
    return a;
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
        let titleBar = createElementWithText("span");
        titleBar.classList.add('fireSeqSearchTitleBar');
        let hitCount = createElementWithText("span",
            "We found " + count.toString() + " results in your logseq notebook");

        titleBar.appendChild(hitCount);

        let btn = document.createElement("button");
        let text = document.createTextNode("Hide Summary (Tmp)");
        btn.appendChild(text);
        btn.onclick = function () {
            // alert("Button is clicked");
            for (let el of document.querySelectorAll('.fireSeqSearchHitSummary')) {
                // el.style.visibility = 'hidden';
                el.remove();
            }
        };
        titleBar.appendChild(btn);
        return titleBar;
    }



    function createFireSeqDom() {
        let div = document.createElement("div");
        const p = createElementWithText("p", "fireSeqSearch launched!")
        p.style = "margin: 0; padding: 0;" // reset for google and duckduckgo
        div.appendChild(p);
        div.setAttribute("id", fireSeqSearchDomId);
        div.style = "padding: 20px; border: thin solid gray; border-radius: 5px;"

        // document.body.insertBefore(div, document.body.firstChild);
        // consoleLogForDebug("inserted");
        return div;
    }

    let dom = createFireSeqDom();
    dom.appendChild(createTitleBarDom(rawSearchResult.length));
    consoleLogForDebug(dom);

    let hitList = document.createElement("ul");
    hitList.style.marginLeft = "20px";
    hitList.style.marginTop = "10px";
    consoleLogForDebug(rawSearchResult);
    for (let rawRecord of rawSearchResult) {
        // const e = document.createTextNode(record);
        consoleLogForDebug(rawRecord);
        const record = JSON.parse(rawRecord);
        consoleLogForDebug(typeof record);
        let li =  createElementWithText("li", "");
        li.style.fontSize = "16px";
        li.style.listStyle = 'disc';
        if (firefoxExtensionUserOption.ShowScore) {
            const score = createElementWithText("span", String(record.score));
            li.appendChild(score);
        }
        let href = createHrefToLogseq(record, serverInfo);
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
    hitList.style.lineHeight = "150%";
    dom.appendChild(hitList);

    if (firefoxExtensionUserOption.ExperimentalLayout) {
        // Inspired by https://twitter.com/rockucn
        // https://greasyfork.org/en/scripts/446492-%E6%90%9C%E7%B4%A2%E5%BC%95%E6%93%8E%E5%88%87%E6%8D%A2%E5%99%A8-search-engine-switcher/code
        dom.style = `
            position: fixed;
            top: 140px;
            right: 12px;
            width: 200px;
            background-color: hsla(200, 40%, 96%, .8);
            font-size: 12px;
            border-radius: 6px;
            z-index: 99999;`;

    }
    container.prepend(dom);
}

function getSearchParameterFromCurrentPage() {
    let searchParam;

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

    consoleLogForDebug("Got search param: " + searchParam);
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

    console.log(searchParameter);

    GM.xmlHttpRequest({
        method: "GET",
        url: "http://127.0.0.1:3030/server_info",
        onload: function(response) {
            let server_info = JSON.parse(response.responseText);
            consoleLogForDebug(server_info);
            GM.xmlHttpRequest({
                method: "GET",
                url: "http://127.0.0.1:3030/query/" + searchParameter,
                onload: function(response) {
                    let hit = JSON.parse(response.responseText);
                    // consoleLogForDebug(hit);
                    consoleLogForDebug(typeof hit);
                    waitForContainer().then((container) => {
                        appendResultToSearchResult([server_info, hit], container)
                            .then((_e) => {
                                const highlightedItems = document.querySelectorAll(".fireSeqSearchHighlight");
                                consoleLogForDebug(highlightedItems);
                                highlightedItems.forEach((element) => {
                                    element.style.color = 'red';
                                })})
                                .catch(function (error) {
                                consoleLogForDebug(error);
                            });
                    })

                }
            });
        }
    });
    /*

        //https://gomakethings.com/waiting-for-multiple-all-api-responses-to-complete-with-the-vanilla-js-promise.all-method/
        Promise.all([
            fetch("http://127.0.0.1:3030/server_info"),
            fetch("http://127.0.0.1:3030/query/" + searchParameter)
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
