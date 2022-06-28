cd fireSeqSearch_addon
zip -r -FS ../fireSeqSearch.zip * --exclude '*.git*' --exclude "manifest_chrome.json"
cd ..
cp -f fireSeqSearch.zip /dev/shm