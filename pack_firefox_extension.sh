cd fireSeqSearch_addon
zip -r -FS ../fireSeqSearch.zip * --exclude '*.git*' --exclude "monkeyscript.user.js"
cd ..
cp -f fireSeqSearch.zip /dev/shm
