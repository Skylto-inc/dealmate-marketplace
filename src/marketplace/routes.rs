use crate::auth::AuthUser;
use crate::error::AppError;
use crate::marketplace::MarketplaceService;
use crate::models::marketplace::*;
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{delete, get, post, put},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

pub fn public_routes(pool: PgPool) -> Router {
    Router::new()
        .route("/api/marketplace/listings", get(get_listings))
        .route("/api/marketplace/listings/:id", get(get_listing))
        .route("/api/marketplace/profile/:user_id", get(get_user_profile))
        .with_state(pool)
}

pub fn authenticated_routes(pool: PgPool) -> Router {
    Router::new()
        // Listing management
        .route("/api/marketplace/listings", post(create_listing))
        .route("/api/marketplace/listings/:id", put(update_listing))
        .route("/api/marketplace/listings/:id", delete(delete_listing))
        .route("/api/marketplace/listings/:id/verify", post(submit_for_verification))
        .route("/api/marketplace/listings/:id/coupon", get(get_coupon_code))
        
        // Transaction management
        .route("/api/marketplace/transactions", post(create_transaction))
        .route("/api/marketplace/transactions", get(get_user_transactions))
        .route("/api/marketplace/transactions/:id", get(get_transaction))
        .route("/api/marketplace/transactions/:id/complete", put(complete_transaction))
        .route("/api/marketplace/transactions/:id/cancel", put(cancel_transaction))
        .route("/api/marketplace/transactions/:id/dispute", post(dispute_transaction))
        
        // Review management
        .route("/api/marketplace/reviews", post(create_review))
        .route("/api/marketplace/reviews/user/:user_id", get(get_user_reviews))
        .route("/api/marketplace/reviews/listing/:listing_id", get(get_listing_reviews))
        
        // Payment methods
        .route("/api/marketplace/payment-methods", post(add_payment_method))
        .route("/api/marketplace/payment-methods", get(get_payment_methods))
        .route("/api/marketplace/payment-methods/:id", delete(delete_payment_method))
        
        // Notifications
        .route("/api/marketplace/notifications", get(get_notifications))
        .route("/api/marketplace/notifications/:id/read", put(mark_notification_read))
        .route("/api/marketplace/notifications/settings", get(get_notification_settings))
        .route("/api/marketplace/notifications/settings", put(update_notification_settings))
        
        // Dashboard
        .route("/api/marketplace/dashboard", get(get_dashboard))
        .route("/api/marketplace/my-listings", get(get_my_listings))
        .with_state(pool)
}

// Public endpoints

async fn get_listings(
    State(pool): State<PgPool>,
    Query(filters): Query<ListingFilters>,
) -> Result<impl IntoResponse, AppError> {
    let service = MarketplaceService::new(pool);
    let listings = service.get_listings(filters).await?;
    Ok(Json(listings))
}

async fn get_listing(
    State(pool): State<PgPool>,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let service = MarketplaceService::new(pool);
    let listing = service.get_listing(id).await?;
    Ok(Json(listing))
}

async fn get_coupon_code(
    State(pool): State<PgPool>,
    auth_user: AuthUser,
    Path(listing_id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let service = MarketplaceService::new(pool);
    let coupon_code = service.get_coupon_code(&auth_user, listing_id).await?;
    
    #[derive(Debug, Clone, Serialize)]
    struct CouponResponse {
        coupon_code: Option<String>,
        has_access: bool,
    }
    
    let response = CouponResponse {
        has_access: coupon_code.is_some(),
        coupon_code,
    };
    
    Ok(Json(response))
}

async fn get_user_profile(
    State(pool): State<PgPool>,
    Path(user_id): Path<String>,
) -> Result<impl IntoResponse, AppError> {
    let service = MarketplaceService::new(pool);
    let profile = service.get_user_profile(&user_id).await?;
    Ok(Json(profile))
}

// Authenticated endpoints

async fn create_listing(
    State(pool): State<PgPool>,
    auth_user: AuthUser,
    Json(request): Json<CreateListingRequest>,
) -> Result<impl IntoResponse, AppError> {
    let service = MarketplaceService::new(pool);
    
    // Validate discount code listings have coupon codes
    if request.listing_type == ListingType::DiscountCode && request.coupon_code.is_none() {
        return Err(AppError::BadRequest(
            "Discount code listings must include a coupon code".to_string()
        ));
    }
    
    let listing = service.create_listing(&auth_user, request).await?;
    Ok((StatusCode::CREATED, Json(listing)))
}

async fn update_listing(
    State(pool): State<PgPool>,
    auth_user: AuthUser,
    Path(id): Path<Uuid>,
    Json(request): Json<UpdateListingRequest>,
) -> Result<impl IntoResponse, AppError> {
    let service = MarketplaceService::new(pool);
    let listing = service.update_listing(&auth_user, id, request).await?;
    Ok(Json(listing))
}

async fn delete_listing(
    State(pool): State<PgPool>,
    auth_user: AuthUser,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let service = MarketplaceService::new(pool);
    service.delete_listing(&auth_user, id).await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn submit_for_verification(
    State(_pool): State<PgPool>,
    _auth_user: AuthUser,
    Path(_id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    // TODO: Implement verification submission
    Ok(StatusCode::ACCEPTED)
}

async fn create_transaction(
    State(pool): State<PgPool>,
    auth_user: AuthUser,
    Json(request): Json<CreateTransactionRequest>,
) -> Result<impl IntoResponse, AppError> {
    let service = MarketplaceService::new(pool);
    let transaction = service.create_transaction(&auth_user, request).await?;
    Ok((StatusCode::CREATED, Json(transaction)))
}

async fn get_user_transactions(
    State(_pool): State<PgPool>,
    _auth_user: AuthUser,
    Query(_params): Query<TransactionFilters>,
) -> Result<impl IntoResponse, AppError> {
    // TODO: Implement get user transactions
    Ok(Json(Vec::<MarketplaceTransaction>::new()))
}

async fn get_transaction(
    State(_pool): State<PgPool>,
    _auth_user: AuthUser,
    Path(_id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    // TODO: Implement get transaction with auth check
    Ok(StatusCode::OK)
}

async fn complete_transaction(
    State(pool): State<PgPool>,
    auth_user: AuthUser,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let service = MarketplaceService::new(pool);
    let transaction = service.complete_transaction(&auth_user, id).await?;
    Ok(Json(transaction))
}

async fn cancel_transaction(
    State(_pool): State<PgPool>,
    _auth_user: AuthUser,
    Path(_id): Path<Uuid>,
    Json(_request): Json<CancelTransactionRequest>,
) -> Result<impl IntoResponse, AppError> {
    // TODO: Implement cancel transaction
    Ok(StatusCode::OK)
}

async fn dispute_transaction(
    State(_pool): State<PgPool>,
    _auth_user: AuthUser,
    Path(_id): Path<Uuid>,
    Json(_request): Json<DisputeTransactionRequest>,
) -> Result<impl IntoResponse, AppError> {
    // TODO: Implement dispute transaction
    Ok(StatusCode::ACCEPTED)
}

async fn create_review(
    State(pool): State<PgPool>,
    auth_user: AuthUser,
    Json(request): Json<CreateReviewRequest>,
) -> Result<impl IntoResponse, AppError> {
    let service = MarketplaceService::new(pool);
    let review = service.create_review(&auth_user, request).await?;
    Ok((StatusCode::CREATED, Json(review)))
}

async fn get_user_reviews(
    State(_pool): State<PgPool>,
    Path(_user_id): Path<String>,
    Query(_params): Query<ReviewFilters>,
) -> Result<impl IntoResponse, AppError> {
    // TODO: Implement get user reviews
    Ok(Json(Vec::<MarketplaceReview>::new()))
}

async fn get_listing_reviews(
    State(_pool): State<PgPool>,
    Path(_listing_id): Path<Uuid>,
    Query(_params): Query<ReviewFilters>,
) -> Result<impl IntoResponse, AppError> {
    // TODO: Implement get listing reviews
    Ok(Json(Vec::<MarketplaceReview>::new()))
}

async fn add_payment_method(
    State(_pool): State<PgPool>,
    _auth_user: AuthUser,
    Json(_request): Json<CreatePaymentMethodRequest>,
) -> Result<impl IntoResponse, AppError> {
    // TODO: Implement add payment method with Stripe
    Ok(StatusCode::CREATED)
}

async fn get_payment_methods(
    State(_pool): State<PgPool>,
    _auth_user: AuthUser,
) -> Result<impl IntoResponse, AppError> {
    // TODO: Implement get payment methods
    Ok(Json(Vec::<UserPaymentMethod>::new()))
}

async fn delete_payment_method(
    State(_pool): State<PgPool>,
    _auth_user: AuthUser,
    Path(_id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    // TODO: Implement delete payment method
    Ok(StatusCode::NO_CONTENT)
}

async fn get_notifications(
    State(_pool): State<PgPool>,
    _auth_user: AuthUser,
    Query(_params): Query<NotificationFilters>,
) -> Result<impl IntoResponse, AppError> {
    // TODO: Implement get notifications
    Ok(Json(Vec::<MarketplaceNotification>::new()))
}

async fn mark_notification_read(
    State(_pool): State<PgPool>,
    _auth_user: AuthUser,
    Path(_id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    // TODO: Implement mark notification as read
    Ok(StatusCode::OK)
}

async fn get_notification_settings(
    State(_pool): State<PgPool>,
    _auth_user: AuthUser,
) -> Result<impl IntoResponse, AppError> {
    // TODO: Implement get notification settings
    Ok(Json(NotificationSettings {
        email_notifications: true,
        push_notifications: false,
        new_listing_alerts: true,
        price_drop_alerts: true,
        transaction_updates: true,
        review_notifications: true,
    }))
}

async fn update_notification_settings(
    State(_pool): State<PgPool>,
    _auth_user: AuthUser,
    Json(settings): Json<NotificationSettings>,
) -> Result<impl IntoResponse, AppError> {
    // TODO: Implement update notification settings
    Ok(Json(settings))
}

async fn get_dashboard(
    State(pool): State<PgPool>,
    auth_user: AuthUser,
) -> Result<impl IntoResponse, AppError> {
    let service = MarketplaceService::new(pool);
    // TODO: Implement dashboard data aggregation
    let dashboard = DashboardData {
        profile: service.get_user_profile(&auth_user.0.auth0_id).await?,
        transaction_summary: TransactionSummary {
            total_sales: 0.0,
            total_purchases: 0.0,
            pending_transactions: 0,
            completed_transactions: 0,
            average_transaction_value: 0.0,
        },
        recent_listings: vec![],
        recent_transactions: vec![],
        unread_notifications: 0,
    };
    Ok(Json(dashboard))
}

async fn get_my_listings(
    State(pool): State<PgPool>,
    auth_user: AuthUser,
    Query(mut filters): Query<ListingFilters>,
) -> Result<impl IntoResponse, AppError> {
    let service = MarketplaceService::new(pool);
    filters.seller_id = Some(auth_user.0.auth0_id);
    let listings = service.get_listings(filters).await?;
    Ok(Json(listings))
}

// Additional types for API

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransactionFilters {
    pub status: Option<String>,
    pub role: Option<String>, // "buyer" or "seller"
    pub page: Option<i64>,
    pub limit: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReviewFilters {
    pub is_buyer_review: Option<bool>,
    pub min_rating: Option<i32>,
    pub page: Option<i64>,
    pub limit: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotificationFilters {
    pub is_read: Option<bool>,
    pub notification_type: Option<String>,
    pub page: Option<i64>,
    pub limit: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CancelTransactionRequest {
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DisputeTransactionRequest {
    pub reason: String,
    pub evidence: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DashboardData {
    pub profile: MarketplaceProfile,
    pub transaction_summary: TransactionSummary,
    pub recent_listings: Vec<ListingWithSeller>,
    pub recent_transactions: Vec<TransactionDetail>,
    pub unread_notifications: i64,
}
