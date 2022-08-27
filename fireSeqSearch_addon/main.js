// MIT License
// Copyright (c) 2021-2022 Zhenbo Li

function createElementWithText(type, text) {
    let x = document.createElement(type);
    x.textContent = text;
    return x;
}

function wrapRawRecordIntoElement(rawRecord, serverInfo) {
    // rawRecord is String   https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Operators/typeof

    const name = serverInfo.notebook_name;
    console.log("wrapping " + String(rawRecord) + " to notebook " + name);
    console.log(typeof rawRecord);

    const record = JSON.parse(rawRecord);
    console.log(typeof record);

    const title = record.title;
    const target = "logseq://graph/" + name + "?page=" + title;

    let li =  createElementWithText("li", "");
    li.style.fontSize = "16px";
    let a = document.createElement('a');
    let text = document.createTextNode(title);
    a.appendChild(text);
    a.title = title;
    a.href = target;
    console.log(a);
    li.appendChild(a);
    console.log(li);
    return li;
}


function uglyExtraLine() {
    let x = createElementWithText("br", "");
    return x;
}

function getFireSeqDomToWebpage() {
    const fireSeqSearchDomId = "fireSeqSearchDom";
    function insertFireSeqDomToWebpage() {
        let div = document.createElement("div");
        div.appendChild(createElementWithText("p", "fireSeqSearch launched!"));
        div.setAttribute("id", fireSeqSearchDomId);

        document.body.insertBefore(div, document.body.firstChild);
        console.log("inserted");
        // Very hacky for google
        if (window.location.toString().includes("google")) {
            for (let i=0; i<6; ++i) {
                div.appendChild(uglyExtraLine());
            }
        }

        return div;
    }
    let fireDom = document.getElementById(fireSeqSearchDomId);
    if (fireDom === null) {
        fireDom = insertFireSeqDomToWebpage();
    }
    return fireDom;
}

function appendResultToSearchResult(rawSearchResult, dom) {


    const count = rawSearchResult.length;

    let hitCount = createElementWithText("div",
        "We found " + count.toString() + " results in your logseq notebook");
    hitCount.style.fontSize = "large";
    dom.appendChild(hitCount);
    dom.appendChild(uglyExtraLine());

    let hitList = document.createElement("ul");
    for (let rawRecord of rawSearchResult) {
        // const e = document.createTextNode(record);
        let e = wrapRawRecordIntoElement(rawRecord, serverInfo);
        // e.style.
        hitList.appendChild(e);
        // console.log("Added an element to the list");
    }
    hitList.style.lineHeight = "150%";
    dom.appendChild(hitList);
}

function performSearchAgainstLogseq(keywords, serverInfo) {
    const search_url = "http://127.0.0.1:3030/query/" + keywords;
    console.log(search_url);

    // let fireSeqDom = getFireSeqDomToWebpage();
    return window.fetch(search_url);
    /*
        .then(response => response.json())
        .then(searchResult => {
            console.log(searchResult);
            return searchResult;
            // appendResultToSearchResult(data, fireSeqDom)
        });

     */
}



function getSearchParameterFromCurrentPage() {
    let searchParam;

    function getSearchParameterOfSearx() {
        let inputBox = document.getElementById("q");
        // console.log(inputBox);
        return inputBox.value;
    }

    if (window.location.toString().includes("searx")) {
        searchParam = getSearchParameterOfSearx();
    } else {
        // https://stackoverflow.com/a/901144/1166518
        const urlParams = new URLSearchParams(window.location.search);
        // console.log(urlParams);
        searchParam = urlParams.get('q');
    }

    console.log("Got search param: " + searchParam);
    return searchParam;
}


(function() {

    const searchParameter = getSearchParameterFromCurrentPage();
    //https://gomakethings.com/waiting-for-multiple-all-api-responses-to-complete-with-the-vanilla-js-promise.all-method/
    Promise.all([
        fetch("http://127.0.0.1:3030/server_info"),
        fetch("http://127.0.0.1:3030/query/" + searchParameter)
    ]).then(function (responses) {
        // Get a JSON object from each of the responses
        return Promise.all(responses.map(function (response) {
            return response.json();
        }));
    }).then(function (data) {
        // Log the data to the console
        // You would do something with both sets of data here
        console.log(data);
        appendResultToSearchResult(data);
    }).catch(function (error) {
        // if there's an error, log it
        console.log(error);
    });
/*
    window.fetch()
        // .then(response => console.log(response));
        .then(response => response.json())
        .then(serverInfo => performSearchAgainstLogseq(searchParameter, serverInfo))
        .then(searchResult => {
            console.log('search finished');
            console.log(searchResult);
        });



 */
})();
