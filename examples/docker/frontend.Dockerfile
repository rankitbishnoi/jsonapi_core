FROM rust:1.95-slim AS build
RUN rustup target add wasm32-unknown-unknown \
 && cargo install trunk --locked wasm-bindgen-cli
WORKDIR /src
COPY . /src
# API base is baked at build time; override at runtime via window.__SHOWCASE_CONFIG__.
ARG API_BASE=http://localhost:8080
ENV API_BASE=${API_BASE}
RUN cd examples/frontend && trunk build --release

FROM nginx:1.27-alpine
COPY --from=build /src/examples/frontend/dist /usr/share/nginx/html
EXPOSE 80
