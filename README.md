fireSeqSearch: Append Logseq notes while Googling

Introduction
--------
[fireSeqSearch](https://github.com/Endle/fireSeqSearch) is inspired by [Evernote](https://evernote.com)'s browser extension - if we search a term, for example, `softmax` in Google, [fireSeqSearch](https://github.com/Endle/fireSeqSearch) will also search in our personal notebook, and append the hits into Google results.


With [logseq 0.6.6](https://discuss.logseq.com/t/done-deep-linking-or-url-scheme-allow-linking-to-logseq-pages-from-outside-the-app/3146/26?u=endle), [Logseq URL Protocol](http://discordapp.com/channels/725182569297215569/756886540038438992/965024044183339088) ,  it's time for [fireSeqSearch](https://github.com/Endle/fireSeqSearch) to support jumping into Logseq!


More examples at <https://github.com/Endle/fireSeqSearch/blob/master/docs/examples.md>



How to use it
------------------
fireSeqSearch will only read your logseq notebooks, which is unlikely to cause data loss.

fireSeqSearch has a server-side app and a browser extension.

### Install Browser Extension  
1. Install latest web extension <https://addons.mozilla.org/en-US/firefox/addon/fireseqsearch/>   
2. If you're using other browser, you can install [Tampermonkey](https://www.tampermonkey.net/), then install the [script version](https://github.com/Endle/fireSeqSearch/raw/master/fireSeqSearch_addon/monkeyscript.user.js)

### Install Local Server

**Obsidian MD** users: Run `fire_seq_search_server --notebook_path <path> --obsidian-md`. [Example obsidian.sh](https://github.com/Endle/fireSeqSearch/blob/master/fire_seq_search_server/obsidian.sh)  


#### Windows
Steps:  
1. Download the latest release at <https://github.com/Endle/fireSeqSearch/releases>
2. If you're using PowerShell, run `.\fire_seq_search_server.exe  --notebook_path C:\Users\li\logseq_notebook`
3. If you're using Msys2, run `./fire_seq_search_server --notebook_path /c/Users/li/logseq_notebook`
4. Please remember to change the path to your notebook

#### Linux and macOS
1. Install rust. See <https://doc.rust-lang.org/cargo/getting-started/installation.html>
2. `git clone https://github.com/Endle/fireSeqSearch`
3. `cd fire_seq_search_server && cargo build`
4. `target/debug/fire_seq_search_server --notebook_path /home/li/my_notebook`
5. Min rust version: See https://github.com/Endle/fireSeqSearch/blob/master/.github/workflows/rust.yml#L21


#### Docker (experimental)

```
git clone https://github.com/Endle/fireSeqSearch && cd fireSeqSearch
```

Configure the path to your logseq notebook by

```
cp example.env .env
```

and edit `.env`.

Finally run

```
docker-compose up -d
```

> **Note**: Alternatively, you can also run docker directly without docker-compose via:

```bash
export $(cat .env | xargs)
docker run -d -v $NOTEBOOK_DIR:$NOTEBOOK_DIR -p 127.0.0.1:3030:3030 --env-file .env ghcr.io/endle/fireseqsearch:latest
```


License
----------------
This project (both server and addon) is using MIT license. Some third party library may have other licenses (see source code)


<a href="https://www.flaticon.com/free-icons/ui" title="ui icons">Ui icons created by manshagraphics - Flaticon</a>


LOGO link: <https://www.flaticon.com/free-icon/web-browser_7328762>


LOGO license: Flaticon license


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

Star History
--------
## Star History

[![Star History Chart](https://api.star-history.com/svg?repos=Endle/fireSeqSearch&type=Date)](https://star-history.com/#Endle/fireSeqSearch&Date)

Provided by <https://star-history.com>
