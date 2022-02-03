use actix_web::{post, web, App, HttpResponse, HttpServer, Responder};
use reqwest::Url;
use serde_json::json;
use tokio::sync::oneshot;
use truelayer_rust::apis::auth::Credentials;
use uuid::Uuid;

#[derive(Clone)]
struct MockServerConfiguration {
    client_id: String,
    client_secret: String,
    certificate_id: String,
    certificate_public_key: Vec<u8>,
    access_token: String,
}

/// Simple mock server for TrueLayer APIs used in local integration tests.
pub struct TrueLayerMockServer {
    url: Url,
    shutdown: Option<oneshot::Sender<()>>,
}

impl TrueLayerMockServer {
    pub async fn start(
        client_id: &str,
        client_secret: &str,
        certificate_id: &str,
        certificate_public_key: Vec<u8>,
    ) -> Self {
        let configuration = MockServerConfiguration {
            client_id: client_id.to_string(),
            client_secret: client_secret.to_string(),
            certificate_id: certificate_id.to_string(),
            certificate_public_key,
            access_token: Uuid::new_v4().to_string(),
        };

        // Setup the mock HTTP server and bind it to a random port
        let http_server_factory = HttpServer::new(move || {
            App::new()
                .app_data(web::Data::new(configuration.clone()))
                .service(post_auth)
        })
        .workers(1)
        .bind("127.0.0.1:0")
        .unwrap();

        // Retrieve the address and port the server was bound to
        let addr = http_server_factory.addrs().first().cloned().unwrap();

        // Prepare a oneshot channel to kill the HTTP server when this struct is dropped
        let (shutdown_sender, shutdown_recv) = oneshot::channel();

        // Start the server in another task
        let http_server = http_server_factory.run();
        tokio::spawn(async move {
            tokio::select! {
                _ = http_server => panic!("HTTP server crashed"),
                _ = shutdown_recv => { /* Intentional shutdown */ }
            }
        });

        Self {
            url: Url::parse(&format!("http://{}", addr)).unwrap(),
            shutdown: Some(shutdown_sender),
        }
    }

    pub fn url(&self) -> &Url {
        &self.url
    }
}

impl Drop for TrueLayerMockServer {
    fn drop(&mut self) {
        let _ = self.shutdown.take().unwrap().send(());
    }
}

#[post("/connect/token")]
async fn post_auth(
    configuration: web::Data<MockServerConfiguration>,
    incoming: web::Json<Credentials>,
) -> impl Responder {
    match incoming.into_inner() {
        Credentials::ClientCredentials {
            client_id,
            client_secret,
            ..
        } if client_id == configuration.client_id
            && client_secret == configuration.client_secret =>
        {
            HttpResponse::Ok().json(json!({
                "token_type": "Bearer",
                "access_token": configuration.access_token,
                "expires_in": 3600
            }))
        }
        _ => HttpResponse::BadRequest().json(json!({
            "error": "invalid_client"
        })),
    }
}
