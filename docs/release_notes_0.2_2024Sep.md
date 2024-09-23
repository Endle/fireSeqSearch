### 0.2.1  

New feature: Note Summarization with Local LLM. 

What happens locally, what stays locally. 

#### Run server with local LLM  
fireSeqSearch facilitates [llamafile](https://github.com/Mozilla-Ocho/llamafile) by [Mozilla](https://github.com/Mozilla-Ocho). 

```
mkdir -pv ~/.llamafile && cd ~/.llamafile
wget https://huggingface.co/Mozilla/Mistral-7B-Instruct-v0.2-llamafile/resolve/main/mistral-7b-instruct-v0.2.Q4_0.llamafile?download=true
chmod +x mistral-7b-instruct-v0.2.Q4_0.llamafile
```

After that, compile and run fireSeqSearch with LLM   
```
cargo build --features llm
target/debug/fire_seq_search_server --notebook_path ~/logseq
# Obsidian users
target/debug/fire_seq_search_server --notebook_path ~/obsidian --obsidian-md
```

Finally, update the [Firefox Addon](https://addons.mozilla.org/en-US/firefox/addon/fireseqsearch/).

#### Demo Video
https://github.com/user-attachments/assets/b0a4ca66-0a33-401a-a916-af7a69f2ae7b

This demo used [AstroWiki](https://github.com/AYelland/AstroWiki_2.0), which is licensed under MIT license.
