use actix_web::{web, App, HttpServer};
use log::{info, warn};
use sqlx::sqlite::SqlitePoolOptions;
use tokio::sync::mpsc;

use biliupmgr::config::ManagerConfig;
use biliupmgr::db;
use biliupmgr::recorder::RecorderEvent;
use biliupmgr::upload;
use biliupmgr::webhook;
use biliupmgr::webhook::AppState;

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    env_logger::init();

    let config = ManagerConfig::load("config.yaml").expect("Failed to load config");

    let dao = {
        let pool = SqlitePoolOptions::new()
            .max_connections(5)
            .connect("sqlite:biliup.db")
            .await
            .expect("Failed to connect to database");

        let dao = db::BiliupDao::new(pool);
        web::Data::new(dao)
    };

    let (tx, mut rx) = mpsc::channel::<RecorderEvent>(16);
    let tx = web::Data::new(tx);

    let state = web::Data::new(AppState::default());

    let bind_addr = (config.host.clone(), config.port);

    {
        let dao = dao.clone();
        let state = state.clone();
        tokio::spawn(async move {
            while let Some(event) = rx.recv().await {
                match upload::upload(&config, &dao, &event, &state).await {
                    Ok(_) => (),
                    Err(e) => warn!("{}", e),
                }
            }
        });
    }

    info!("Starting server at http://{}:{}", bind_addr.0, bind_addr.1);
    HttpServer::new(move || {
        App::new()
            .app_data(tx.clone())
            .app_data(dao.clone())
            .app_data(state.clone())
            .service(webhook::status)
            .service(webhook::status_ok)
            .service(webhook::recorder)
            .service(webhook::retry)
    })
    .bind(bind_addr)?
    .run()
    .await
}
