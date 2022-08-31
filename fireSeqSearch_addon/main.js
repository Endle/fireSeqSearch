// MIT License
// Copyright (c) 2021-2022 Zhenbo Li

const fireSeqSearchDomId = "fireSeqSearchDom";


function createElementWithText(type, text) {
    let x = document.createElement(type);
    x.textContent = text;
    return x;
}

function createHrefToLogseq(record, serverInfo) {
    const name = serverInfo.notebook_name;

    const title = record.title;
    const prettyTitle = title.replace("%2F", "/");
    const target = "logseq://graph/" + name + "?page=" + title;
    let a = document.createElement('a');
    let text = document.createTextNode(prettyTitle);
    a.appendChild(text);
    a.title = prettyTitle;
    a.href = target;
    console.log(a);
    return a;
}



function uglyExtraLine() {
    return createElementWithText("br", "");
}


function checkUserOptions() {
    return Promise.all([
        /*global browser */
        browser.storage.sync.get("debugStr"),
        browser.storage.sync.get("ExperimentalLayout"),
        browser.storage.sync.get("ShowHighlight"),
        browser.storage.sync.get("ShowScore")
    ]).then(function(res) {
        console.log(res);

        const options = {
            debugStr: res[0].debugStr,
            ExperimentalLayout: res[1].ExperimentalLayout,
            ShowHighlight: res[2].ShowHighlight,
            ShowScore: res[3].ShowScore
        }
        console.log(options);
        return options;
    });
}

async function appendResultToSearchResult(fetchResultArray) {
    const serverInfo = fetchResultArray[0];
    const rawSearchResult = fetchResultArray[1];
    const firefoxExtensionUserOption = await checkUserOptions();
    const count = rawSearchResult.length;

    console.log(firefoxExtensionUserOption);

    let hitCount = createElementWithText("div",
        "We found " + count.toString() + " results in your logseq notebook");
    hitCount.style.fontSize = "large";

    function createFireSeqDom() {
        let div = document.createElement("div");
        div.appendChild(createElementWithText("p", "fireSeqSearch launched!"));
        div.setAttribute("id", fireSeqSearchDomId);

        // document.body.insertBefore(div, document.body.firstChild);
        // console.log("inserted");
        // Very hacky for google
        if (window.location.toString().includes("google")) {
            for (let i=0; i<6; ++i) {
                div.appendChild(uglyExtraLine());
            }
        }
        return div;
    }

    let dom = createFireSeqDom();
    dom.appendChild(hitCount);
    dom.appendChild(uglyExtraLine());

    let hitList = document.createElement("ul");
    for (let rawRecord of rawSearchResult) {
        // const e = document.createTextNode(record);
        const record = JSON.parse(rawRecord);
        console.log(typeof record);
        let li =  createElementWithText("li", "");
        li.style.fontSize = "16px";
        if (firefoxExtensionUserOption.ShowScore) {
            const score = createElementWithText("span", String(record.score));
            li.appendChild(score);
        }
        let href = createHrefToLogseq(record, serverInfo);
        li.appendChild(href);
        // let e = wrapRawRecordIntoElement(record, serverInfo);

        // e.style.
        hitList.appendChild(li);
        // console.log("Added an element to the list");
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
    document.body.insertBefore(dom, document.body.firstChild);
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
        return Promise.all(responses.map(function (response) {return response.json();}));
    }).then(function (data) {
        console.log(data);
        appendResultToSearchResult(data);
    }).catch(function (error) {
        console.log(error);
    });

})();
