cp -r fireSeqSearch_addon chrome_tmp
cd chrome_tmp
rm manifest.json
mv manifest_chrome.json manifest.json
zip -r -FS ../fireSeqSearch_chrome.zip * --exclude '*.git*'
cd ..
rm -rf chrome_tmp
cp -f fireSeqSearch.zip /dev/shm