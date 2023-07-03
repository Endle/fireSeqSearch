### 0.1.3  

#### New Feature: Generate wordcloud.  

Just visit `http://127.0.0.1:3030/wordcloud`, and fireSeqSearch will generate a wordcloud with your logseq notes. Each word in the cloud is clickable. With a single click, you can search your top-words in search engines and your personal notes simultaneously.  

[This demo video](https://github.com/Endle/fireSeqSearch/assets/3221521/524fe70d-a128-4393-bd26-bee71871f38e) used `note of greek myth`, created by [yongerG](https://www.douban.com/note/807432536/?_i=8350280BMJZhl7). This note is [licensed with CC-BY-SA-4.0 license](https://github.com/Lihaogx/graph-note-of-greek-myth/blob/main/LICENSE).  

Thanks to [timdream](https://timdream.org/) and other contributors for the amazing library [wordcloud2.js](https://github.com/timdream/wordcloud2.js). 

#### New Feature:  Allow to filter out zotero imported pages [Issue 122](https://github.com/Endle/fireSeqSearch/issues/122)

### 0.1.2  
New server-side feature: [Read and Search PDF contents](https://github.com/Endle/fireSeqSearch/issues/63)! In a logseq page, the PDF link `![title](../assets/doc_000123_0.pdf)` will be parsed, and appended to the document.

#### How to use it  
This feature is turned off by default. Adding `--parse-pdf-links` to enable PDF parsing. [See example](https://github.com/Endle/fireSeqSearch/blob/81a9c2fc53ef589e8e63d19467825d63a84bd404/fire_seq_search_server/debug_server.sh#L8)

Deficient: Performance. It needs further evaluation.

#### Thanks  
The crate [PDF-extract](https://github.com/jrmuizel/pdf-extract) makes this new feature possible. Thanks [Jeff Muizelaar](https://github.com/jrmuizel) and [Joep Meindertsma](https://github.com/joepio) for it.  


[Clifford Enoc](https://github.com/cliffordx) created this feature request.  


### 0.1.1  
This is the first time for bumping the **MINOR version** for a big new feature:

ObsidianMD support!

Bug fixes with contribution of xxchan.
Dev change: Added sccache with the support of xuanwo.

This a server side update.

### 0.0.22
This is both server-side and client-side update.  

New feature: [include journal pages in search results](https://github.com/Endle/fireSeqSearch/issues/65). This feature is turned off by default. Use `--enable-journal-query` to enable it.

Currently, I haven't figured out an approach to generate the Logseq URI for a specific journal page.

### 0.0.19
This is a server-side update.  

1. Fixed [highlight for Cyrillic letters](https://github.com/Endle/fireSeqSearch/issues/59).  
2. Improvement: When a paragraph is too long, use its summary (See [Issue 57](https://github.com/Endle/fireSeqSearch/issues/57) and [commit](https://github.com/Endle/fireSeqSearch/commit/fb15a17bb9a47754bb7817891b01f08108c8c952))  

### 0.0.18
Exciting new UI by @phoenixeliot and @yoyurec  
Thank you for your contribution!

No change at server side. All you need is to update Firefox extension or user script.

### 0.0.16

1. Experimental support to search summary.
2. Parse markdown before feeding to tantivy. It expects to reduce false positive in search hits.

#### How to enable search summary
1. Update server and Firefox extension to last version.
2. Firefox Tools->Settings->Extension->fireSeqSearch, enable "Show Summary"

#### Deficient
If the block is very long, for example, you clipped a long article into logseq, then the summary would be hard (or useless) to read. That's why there is a "Hide Summary" button.

#### Thanks
@raphlinus and other https://github.com/raphlinus/pulldown-cmark developers  
@arranf and @fbecart for https://github.com/fbecart/markdown_to_text

