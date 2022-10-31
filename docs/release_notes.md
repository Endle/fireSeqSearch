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

