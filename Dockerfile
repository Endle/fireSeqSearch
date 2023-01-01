FROM rust:1.65-buster AS builder

WORKDIR /fire_seq_search_server
COPY ./fire_seq_search_server .

RUN cargo install --path .

FROM ubuntu:20.04
COPY --from=builder /usr/local/cargo/bin/fire_seq_search_server /usr/local/bin/fire_seq_search_server

ENV RUST_LOG=debug
CMD ["sh", "-c", "fire_seq_search_server --notebook_path $NOTEBOOK_DIR --host 0.0.0.0:3030"]
