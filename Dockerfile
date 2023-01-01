FROM rust:1.65-buster

WORKDIR /fire_seq_search_server
COPY ./fire_seq_search_server .

RUN cargo install --path .

ENV RUST_LOG=debug
CMD ["fire_seq_search_server", "--notebook_path", "/notebook", "--host", "0.0.0.0:3030"]
