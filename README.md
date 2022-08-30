fireSeqSearch: Append Logseq notes while Googling

Introduction
--------
[fireSeqSearch](https://github.com/Endle/fireSeqSearch) is inspired by [Evernote](https://evernote.com)'s browser extension - if we search a term, for example, `softmax` in Google, [fireSeqSearch](https://github.com/Endle/fireSeqSearch) will also search in our personal notebook, and append the hits into Google results.


![screenshot_demo](https://user-images.githubusercontent.com/3221521/168455027-965da612-b783-4d92-83e2-4cd7b4830a43.png)


With [logseq 0.6.6](https://discuss.logseq.com/t/done-deep-linking-or-url-scheme-allow-linking-to-logseq-pages-from-outside-the-app/3146/26?u=endle), [Logseq URL Protocol](http://discordapp.com/channels/725182569297215569/756886540038438992/965024044183339088) ,  it's time for [fireSeqSearch](https://github.com/Endle/fireSeqSearch) to support jumping into Logseq!


<video src="https://user-images.githubusercontent.com/3221521/168455012-e1183f62-4682-4230-84e7-8a461d8985a0.mp4"></video>







How to use it
------------------
This project is in **VERY EARLY** DEVELOPMENT! But don't panic. fireSeqSearch will only read your logseq notebooks, which is unlikely to cause data loss.

### Install Browser Extension  
1. Install latest web extension <https://addons.mozilla.org/en-US/firefox/addon/fireseqsearch/>   
2. Google require all new Chrome extensions use manifest V3, while providing little examples for it.  
3. Edge is rejecting my extension, without explaining why.   

### Install Local Server

#### Windows
Steps:  
1. Download the latest release at <https://github.com/Endle/fireSeqSearch/releases>
2. If you're using PowerShell, run `.\fire_seq_search_server.exe  --notebook_path C:\Users\li\logseq_notebook`
3. If you're using Msys2, run `./fire_seq_search_server --notebook_path /c/Users/li/logseq_notebook`
4. Please remember to change the path to your notebook

#### Linux and macOS
1. Install rust. See https://doc.rust-lang.org/cargo/getting-started/installation.html
2. `git clone https://github.com/Endle/fireSeqSearch`
3. `cd fire_seq_search_server && cargo build`
4. `target/debug/fire_seq_search_server --notebook_path /home/li/my_notebook`
5. Min rust version: See https://github.com/Endle/fireSeqSearch/blob/master/.github/workflows/rust.yml#L21


How it works
---------
This is what [fireSeqSearch](https://github.com/Endle/fireSeqSearch) does on my logseq notebook. I had to split it into two parts because Firefox extensions are not allowed to access local files.

fireSeqSearch has two parts:

### 1. search server
It read all local loseq notebooks, and hosts logseq pages on http://127.0.0.1:3030

It provides the API `http://127.0.0.1:3030/query/`


### 2. Browser extension
Every time we use search engine, it will fetch `http://127.0.0.1:3030/query/keywords`and append all hits to the web page.


Similar Projects
--------------
* [karlicoss/promnesia](https://github.com/karlicoss/promnesia)  - [Promnesia](https://github.com/karlicoss/promnesia) is a mature and interesting project, aming a more ambitious goal. [fireSeqSearch](https://github.com/Endle/fireSeqSearch) only does one thing - append logseq hits to search engine results.
