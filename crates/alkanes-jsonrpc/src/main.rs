mod config;
mod handler;
mod jsonrpc;
mod proxy;
mod sandshrew;

use actix_cors::Cors;
use actix_web::{middleware::Logger, web, App, HttpRequest, HttpResponse, HttpServer};
use config::Config;
use jsonrpc::{JsonRpcRequest, JsonRpcResponse, INTERNAL_ERROR};
use proxy::ProxyClient;
use std::sync::Arc;

struct AppState {
    proxy: Arc<ProxyClient>,
}

async fn handle_jsonrpc(
    req: HttpRequest,
    body: web::Json<JsonRpcRequest>,
    state: web::Data<AppState>,
) -> HttpResponse {
    let ip = req
        .headers()
        .get("X-Real-IP")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("unknown");

    log::info!("{}|{}", ip, serde_json::to_string(&body.0).unwrap_or_default());

    match handler::handle_request(&body.0, &state.proxy).await {
        Ok(response) => HttpResponse::Ok().json(response),
        Err(e) => {
            log::error!("Error handling request: {:?}", e);
            HttpResponse::Ok().json(JsonRpcResponse::error(
                INTERNAL_ERROR,
                e.to_string(),
                body.0.id.clone(),
            ))
        }
    }
}



#[actix_web::main]
async fn main() -> std::io::Result<()> {
    env_logger::init_from_env(env_logger::Env::new().default_filter_or("info"));

    let config = Config::from_env();
    
    log::info!("Starting alkanes-jsonrpc server");
    log::info!("Server: http://{}:{}", config.server_host, config.server_port);
    log::info!("Bitcoin RPC: {}", config.bitcoin_rpc_url);
    log::info!("Metashrew: {}", config.metashrew_url);
    log::info!("Memshrew: {}", config.memshrew_url);
    log::info!("Ord: {}", config.ord_url);
    log::info!("Esplora: {}", config.esplora_url);

    let proxy = Arc::new(ProxyClient::new(config.clone()));

    let server_host = config.server_host.clone();
    let server_port = config.server_port;

    HttpServer::new(move || {
        let cors = Cors::default()
            .allow_any_origin()
            .allow_any_method()
            .allow_any_header()
            .max_age(3600);

        App::new()
            .app_data(web::Data::new(AppState {
                proxy: proxy.clone(),
            }))
            .app_data(web::JsonConfig::default().limit(100 * 1024 * 1024))
            .wrap(cors)
            .wrap(Logger::default())
            .route("/", web::post().to(handle_jsonrpc))
            .route("/{tail:.*}", web::post().to(handle_jsonrpc))
    })
    .bind((server_host.as_str(), server_port))?
    .run()
    .await
}
