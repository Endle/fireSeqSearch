##### fire_seq_search_server

Currently, this server is running at hard-coded port <http://127.0.0.1:3030>  (or http://localhost:3030)

### Endpoints

#### GET `/server_info`


#### GET `/query/%s`

Returns an array of `hit`s.

Schema of `hit`  (**unstable**)  
title: The title of the logseq page  
summary   
score  

#### GET `/suggest/%s`

Returns a minified JSON array of results as defined by the [OpenSearch Suggestions Extension 1.1](https://web.archive.org/web/20180406023110/http://www.opensearch.org/Specifications/OpenSearch/Extensions/Suggestions/1.1).

This endpoint enables the user to define fireSeqSearch as a native browser search engine with suggestions.

Limited to first ten matches.

```json
[
  "user's search query",
  ["Result 1 Title", "R2 Title", "etc..."],
  ["Result 1 summary", "R2 summary", "or empty string"],
  [
    "http://localhost:3030/lucky/Result+1+Title",
    "http://localhost:3030/lucky/R2+Title",
    "etc..."
  ]
]
```

This API response format is also used by many search engines such as Google, DuckDuckGo, Bing, and Wikipedia. Vivaldi is one browser that allows explicitly defining this "Suggest URL".

#### GET `/lucky/%s`

Responds with a redirect to the best-match Logseq app page for a given title. Used in combination with suggest, this enables a native search experience in certain browsers.

Supported by most Chrome-based browsers.
