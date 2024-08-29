FROM lukemathwalker/cargo-chef:latest-rust-latest AS chef
WORKDIR /app

FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

FROM chef AS builder
COPY --from=planner /app/recipe.json recipe.json
RUN cargo chef cook --release --recipe-path recipe.json
WORKDIR /app
COPY . .
RUN cargo build --release

FROM debian:latest AS runtime
RUN apt-get update -y \
    && apt-get install -y libpq5
COPY --from=builder /app/target/release/newsletter newsletter
COPY configuration configuration
ENV APP_ENVIRONMENT=production

ENTRYPOINT ["./newsletter"]
