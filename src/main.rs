use std::collections::HashMap;
use std::net::{Ipv4Addr, SocketAddrV4};

use axum::body::Body;
use axum::extract::Path;
use axum::extract::State;
use axum::http::header;
use axum::http::Request;
use axum::http::StatusCode;
use axum::response::{Html, IntoResponse, Response};
use axum::routing;
use axum::Form;
use axum::Router;

use bb8::Pool;
use bb8_redis::RedisConnectionManager;
use redis::AsyncCommands;
use sha2::Digest;
use sha2::Sha256;

const SALT: &'static str = "UNIQUE_SAL";
const DEFAULT_NAME: &'static str = "Joe Bloggs";

#[tokio::main]
async fn main() {
    let subscriber = tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .with_level(true)
        .finish();
    tracing::subscriber::set_global_default(subscriber)
        .expect("Failed to set up tracing");

    tracing::info!("Starting..");

    let manager = RedisConnectionManager::new("redis://redis:6379/0").unwrap();
    let pool = bb8::Pool::builder()
        .build(manager)
        .await
        .expect("Failed to build pool!");

    let app = Router::new()
        .route("/monster/:name", routing::get(get_identicon))
        .route("/", routing::post(handler))
        .route("/", routing::get(default))
        .with_state(pool);

    let server = axum::Server::bind(&std::net::SocketAddr::V4(
        SocketAddrV4::new(Ipv4Addr::new(0, 0, 0, 0), 9090),
    ))
    .serve(app.into_make_service());
    let _ = server.await;
    tracing::info!("Stopping..");
}

async fn default(_: Request<Body>) -> Html<String> {
    tracing::info!("Default request");
    cook_response(DEFAULT_NAME.to_string())
}

async fn handler(
    Form(name): Form<HashMap<String, String>>,
) -> impl IntoResponse {
    tracing::info!("handler request");
    let name = name.get("name").unwrap().clone();
    cook_response(name)
}

fn cook_response(name: String) -> Html<String> {
    let salted_name = format!("{}{}", name, SALT);
    let hashed_name = calculate_hash(&salted_name);
    let header = "<html><head><title>Identidock</title></head><body>";
    let body = format!(
        r#"<form method="POST">
              Hello <input type="text" name="name" value="{}">
              <input type="submit" value="submit">
              </form>
              <p>You look like a:
              <img src="/monster/{}"/>"#,
        name, hashed_name
    );
    let footer = "</body></html>";
    Html(format!("{}{}{}", header, body, footer))
}

async fn get_identicon(
    State(pool): State<Pool<RedisConnectionManager>>,
    Path(name): Path<String>,
) -> impl IntoResponse {
    tracing::info!("Get identicon request");

    let mut redis_connection = pool.get().await.unwrap();
    let image = match redis_connection.get::<_, Vec<u8>>(&name).await {
        Ok(image) if !image.is_empty() => {
            tracing::info!("Got an image from redis..");
            image
        }
        _ => {
            tracing::info!("No image in redis cache, generate a new image..");
            let response = reqwest::get(format!(
                "http://dnmonster:8080/monster/{}?size=80",
                name
            ))
            .await
            .unwrap();
            let image = response.bytes().await.unwrap().to_vec();
            let _: redis::RedisResult<()> =
                redis_connection.set(&name, image.clone()).await;
            image
        }
    };

    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "image/png")
        .body(image.into_response())
        .unwrap()
}

fn calculate_hash<T: AsRef<[u8]>>(t: &T) -> String {
    let mut hasher = Sha256::new();
    hasher.update(t);
    let hash = hasher.finalize();
    base16ct::lower::encode_string(&hash)
}
