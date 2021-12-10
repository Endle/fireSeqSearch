// MIT License
// Copyright (c) 2021 Zhenbo Li


function performSearchAgainstLogseq(keywords) {
    // https://stackoverflow.com/a/44516256/1166518
    const logseqPagesPath = "/home/lizhenbo/src/logseq_notebook/pages";

    const notename = "Softmax.md";

    const filename = "file://" + logseqPagesPath + "/" + notename;


    let reader = new FileReader();




    reader.onload = function(){
        // this will then display a text file
        console.log(reader.result);
    };
    reader.onerror = function(e) {
        console.log('got event: ' + e);
    }

    console.log(reader);
    console.log(filename);
    reader.readAsText(filename);

    console.log(reader);

    return "<p>" + keywords + "</p>";
}




(function() {

    document.body.style.border = "5px solid red";

    function getSearchParameterFromCurrentPagg() {
        return "linear";
    }


    function writeResult(searchResult) {
        console.log(searchResult);
    }



    const searchParameter = getSearchParameterFromCurrentPagg();


    const searchResult = performSearchAgainstLogseq(searchParameter);


    writeResult(searchResult);

})();
