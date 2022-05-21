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

function performSearchAgainstLogseq(keywords, outputDom, serverInfo) {
    const search_url = "http://127.0.0.1:3030/query/" + keywords;

    function reqListener () {
        console.log(this);
    }

    function uglyExtraLine() {
        let x = createElementWithText("br", "");
        return x;
    }
    console.log(search_url);

    function writeResult(rawSearchResult, dom) {

        // Very hacky for google
        if (window.location.toString().includes("google")) {
            for (let i=0; i<6; ++i) {
                dom.appendChild(uglyExtraLine());
            }
        }
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

    window.fetch(search_url)
        // .then(response => console.log(response));
        .then(response => response.json())
        .then(data => {
            console.log(data);
            writeResult(data, outputDom)
        });




    // writeResult(searchResult);
}




(function() {
    const fireSeqSearchDomId = "fireSeqSearchDom";

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

        console.log("Got search param: ");
        console.log(searchParam);
        return searchParam;
    }


    function insertFireSeqDomToWebpage() {
        let div = document.createElement("div");
        div.appendChild(createElementWithText("p", "fireSeqSearch launched!"));
        div.setAttribute("id", fireSeqSearchDomId);
        // console.log(div);
        // console.log(contentDom.firstChild);

        document.body.insertBefore(div, document.body.firstChild);
        console.log("inserted");
        return div;
    }



    function getFireSeqDomToWebpage() {
        let fireDom = document.getElementById(fireSeqSearchDomId);
        if (fireDom === null) {
            fireDom = insertFireSeqDomToWebpage();
        }
        return fireDom;
    }

    let fireSeqDom = getFireSeqDomToWebpage();

    const searchParameter = getSearchParameterFromCurrentPage();


    window.fetch("http://127.0.0.1:3030/server_info")
        // .then(response => console.log(response));
        .then(response => response.json())
        .then(serverInfo => {
            console.log(serverInfo);
            performSearchAgainstLogseq(searchParameter, fireSeqDom, serverInfo);
        });


})();
