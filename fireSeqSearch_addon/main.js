// MIT License
// Copyright (c) 2021 Zhenbo Li


function performSearchAgainstLogseq(keywords) {
    const search_url = "http://127.0.0.1:3030/query/" + keywords;

    function reqListener () {
        console.log(this);
    }

    // let oReq = new XMLHttpRequest();
    // // oReq.addEventListener("load", reqListener);
    // oReq.onreadystatechange = reqListener;
    // oReq.open("GET", search_url);
    // oReq.send();
    console.log(search_url);
    window.fetch(search_url)
        // .then(response => console.log(response));
        .then(response => response.json())
        .then(data => console.log(data));

    return "<p>" + keywords + "</p>";
}




(function() {
    const fireSeqSearchDomId = "fireSeqSearchDom";

    document.body.style.border = "5px solid red";

    function getSearchParameterFromCurrentPagg() {
        //Hacky
        return "linear";
    }


    function writeResult(searchResult) {
        console.log(searchResult);
    }

    /*

    function getSearchEngineResultBody() {
        //bing
        let bing =  document.getElementById("b_content");
        console.log(bing);
        return bing;
    }

    let contentDom = getSearchEngineResultBody();
*/


    function insertFireSeqDomToWebpage() {
        let div = document.createElement("div");
        div.innerHTML = "Paragraph changed!";
        div.setAttribute("id", "fireSeqSearchDom");
        // console.log(div);
        // console.log(contentDom.firstChild);

        document.body.insertBefore(div, document.body.firstChild);
        console.log("inserted");
        return div;
    }



    function getFireSeqDomToWebpage() {
        let fireDom = document.getElementById(fireSeqSearchDomId);
        if (fireDom == null) {
            fireDom = insertFireSeqDomToWebpage();
        }
        return fireDom;
    }

    let fireSeqDom = getFireSeqDomToWebpage();

    const searchParameter = getSearchParameterFromCurrentPagg();


    const searchResult = performSearchAgainstLogseq(searchParameter);


    fireSeqDom.innerHTML += searchResult;
    writeResult(searchResult);



    // browser.permissions.getAll().then((result) => {
    //     console.log(result.permissions); // [ "webRequest", "tabs" ]
    //     console.log(result.origins)      // [ "*://*.mozilla.org/*" ]
    // });
    // browser.permissions.getAll();

    //
    // let port = browser.runtime.connectNative("fire_seq_search_server");
    // console.log(port);
    // port.onMessage.addListener((response) => {
    //     console.log("Received: " + response);
    // });
    // port.postMessage("searchResult");

    document.body.style.border = "5px solid blue";
})();
