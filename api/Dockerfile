# Build the loan API and run it in a minimal image (no shell, non-root).
FROM rust:1-slim-bookworm AS build
WORKDIR /src
COPY Cargo.toml Cargo.lock ./
COPY src ./src
RUN cargo build --release --locked

FROM gcr.io/distroless/cc-debian12:nonroot
COPY --from=build /src/target/release/loan-api /usr/local/bin/loan-api
ENV PORT=8080
EXPOSE 8080
USER nonroot
ENTRYPOINT ["/usr/local/bin/loan-api"]
