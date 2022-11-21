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

