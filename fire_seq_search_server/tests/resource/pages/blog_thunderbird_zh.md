- [Aug 3, 2021 - 使用 git shallow clone 下载并编译 Thunderbird](https://endle.github.io/2021/08/03/git-shallow-clone-build-thunderbird/)
  
            
  
            
  
  最近在尝试编译 Thunderbird. [官方的手册](https://developer.thunderbird.net/thunderbird-development/getting-started) 的建议是
  
  ```
  hg clone https://hg.mozilla.org/mozilla-central source/
  cd source/
  hg clone https://hg.mozilla.org/comm-central comm/
  ```
  
  因为我网络情况不好，硬盘空间也有些捉襟见肘，就只想下载最新的版本。可是,[Mercurial HG 并不支持](https://stackoverflow.com/a/4205246/1166518).
  
  Mozilla 已经在 GitHub 上有了实验性的 Mirror. 因此，我使用如下的方式下载 Thunderbird 的代码。
  
  ```
  # My personal habit
  cd ~/src/mozilla
  git clone --depth=1 https://github.com/mozilla/gecko-dev.git mozilla-central
  git clone --depth=1 https://github.com/mozilla/releases-comm-central comm-central
  cp -R --reflink=auto comm-central/ mozilla-central/comm
  ```
  
  我会使用如下代码进行更新。
  
  ```
  cd mozilla-central && git pull origin master && trash comm && cd ..
  cd comm-central && git pull origin master && cd ..
  cp -R --reflink=auto comm-central/ mozilla-central/comm
  cd mozilla-central
  ```
-
-
-
- Source: https://endle.github.io/2021/08/03/git-shallow-clone-build-thunderbird/
- CC-BY 4.0 Zhenbo Li