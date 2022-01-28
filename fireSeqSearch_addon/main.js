// MIT License
// Copyright (c) 2021-2022 Zhenbo Li


function performSearchAgainstLogseq(keywords, outputDom) {
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
    function writeResult(rawSearchResult, dom) {

        // Very hacky for google
        if (window.location.toString().includes("google")) {
            for (var i=0; i<6; ++i) {
                dom.innerHTML += "<br/>";
            }
        }
        const count = rawSearchResult.length;

        dom.innerHTML += "<p>We found " + count.toString() + " results in your logseq notebook</p>";

        for (let record of rawSearchResult) {
            dom.innerHTML += "<p>" +  record + "</p>";
        }
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

    // document.body.style.border = "5px solid red";

    function getSearchParameterFromCurrentPage() {
        // https://stackoverflow.com/a/901144/1166518
        const urlParams = new URLSearchParams(window.location.search);
        // console.log(urlParams);
        const searchParam = urlParams.get('q');
        // console.log(searchParam);
        return searchParam;
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
        div.innerHTML = "<p>fireSeqSearch launched!</p>";
        div.setAttribute("id", fireSeqSearchDomId);
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

    const searchParameter = getSearchParameterFromCurrentPage();


    const searchResult = performSearchAgainstLogseq(searchParameter, fireSeqDom);


    // fireSeqDom.innerHTML += searchResult;
    // writeResult(searchResult);


    // document.body.style.border = "5px solid blue";
})();
