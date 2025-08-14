use axum::{routing::{get, post}, Router, Json};
use serde_json::{json, Value};
use tower_http::cors::CorsLayer;

#[tokio::main]
async fn main() {
    let app = Router::new()
        .route("/health", get(health))
        .route("/marketplace/products", get(get_marketplace_products))
        .route("/marketplace/vendors", get(get_vendors))
        .route("/marketplace/products", post(add_product))
        .route("/marketplace/vendors", post(add_vendor))
        .layer(CorsLayer::permissive());

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3004").await.unwrap();
    println!("🏪 Marketplace Service running on port 3004");
    axum::serve(listener, app).await.unwrap();
}

async fn health() -> Json<Value> {
    Json(json!({"status": "healthy", "service": "marketplace-service", "features": ["vendor_management", "product_listings"]}))
}

async fn get_marketplace_products() -> Json<Value> {
    Json(json!({
        "products": [
            {"id": "mp_1", "name": "Vendor Laptop", "vendor": "TechVendor", "price": 899.99}
        ],
        "service": "marketplace-service"
    }))
}

async fn get_vendors() -> Json<Value> {
    Json(json!({
        "vendors": [
            {"id": "vendor_1", "name": "TechVendor", "rating": 4.5, "products": 150}
        ],
        "service": "marketplace-service"
    }))
}

async fn add_product() -> Json<Value> {
    Json(json!({"message": "Product added to marketplace", "service": "marketplace-service"}))
}

async fn add_vendor() -> Json<Value> {
    Json(json!({"message": "Vendor added to marketplace", "service": "marketplace-service"}))
}
