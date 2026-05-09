FROM fedora:43

RUN dnf install -y \
      cmake gcc-c++ git \
      vulkan-headers vulkan-loader-devel \
      glslc glslang \
      libcurl-devel \
    && dnf clean all

RUN git clone --depth=1 https://github.com/ggerganov/llama.cpp /src
WORKDIR /src
RUN cmake -B build -DGGML_VULKAN=ON -DLLAMA_CURL=ON \
 && cmake --build build --config Release -j"$(nproc)" --target llama-server
