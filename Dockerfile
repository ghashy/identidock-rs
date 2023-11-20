# Build stage
FROM rust:slim-bookworm AS base
WORKDIR /app

FROM base as chef
RUN cargo install cargo-chef --locked

FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

FROM chef AS builder 
COPY --from=planner /app/recipe.json recipe.json
# Build dependencies - this is the caching Docker layer!
RUN cargo chef cook --release --recipe-path recipe.json

# Build application
COPY . .
RUN cargo build --release

# Runtime stage
FROM debian:bookworm-slim AS runtime
RUN groupadd -r myuser && useradd -r -g myuser myuser
WORKDIR /app
COPY --from=builder /app/target/release/identidock identidock

EXPOSE 9090
USER myuser

CMD [ "/app/identidock" ]