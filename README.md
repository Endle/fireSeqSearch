Introduction
--------
[fireSeqSearch](https://github.com/Endle/fireSeqSearch) is inspired by [Evernote](https://evernote.com)'s browser extension - if we search a term, for example, `softmax` in Google, the extension will also do this search against the personal notebook.

![screenshot for google](docs/screenshot_demo.png)



How it works
---------
This is what [fireSeqSearch](https://github.com/Endle/fireSeqSearch) does on my logseq notebook. I had to split it into two parts because it has 

It has two parts:

### 1. search server
It read all local loseq notebooks, and hosts logseq pages on http://127.0.0.1:3030

It provides the API `http://127.0.0.1:3030/query/`


### 2. Browser extension
Every time we use search engine, it will fetch `http://127.0.0.1:3030/query/keywords`and append all hits to the web page.



How to use it
------------------
This project is in **VERY EARLY** DEVELOPMENT! Please go ahead only if you'reprepared to share feedbacks.

Don't panic. fireSeqSearch will only read your logseq notebooks, which is unlikely to cause data loss.

1. `git clone https://github.com/Endle/fireSeqSearch`
2. Edit `run_server.sh`, pointing it to your local logseq notebook path
3. Execute `run_server.sh`
4. Install web extension <https://addons.mozilla.org/en-US/firefox/addon/fireseqsearch/>

PowerShell:  .\fire_seq_search_server.exe  --notebook_path C:\Users\li\logseq_notebook
