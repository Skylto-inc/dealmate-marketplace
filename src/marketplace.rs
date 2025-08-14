use bigdecimal::BigDecimal;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::Type, PartialEq)]
#[sqlx(type_name = "text")]
#[sqlx(rename_all = "snake_case")]
pub enum ListingType {
    DiscountCode,
    GiftCard,
    ReferralLink,
    LocationDeal,
    CashbackOffer,
    LoyaltyPoints,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "text")]
#[sqlx(rename_all = "snake_case")]
pub enum ListingStatus {
    Active,
    Sold,
    Expired,
    Suspended,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "text")]
#[sqlx(rename_all = "snake_case")]
pub enum TransactionStatus {
    Pending,
    Escrow,
    Completed,
    Cancelled,
    Disputed,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "text")]
#[sqlx(rename_all = "snake_case")]
pub enum PaymentType {
    Card,
    Paypal,
    Upi,
    Wallet,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "text")]
#[sqlx(rename_all = "snake_case")]
pub enum VerificationStatus {
    Pending,
    InProgress,
    Verified,
    Rejected,
}

// Marketplace Listing Model
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct MarketplaceListing {
    pub id: Uuid,
    pub seller_id: String,
    pub listing_type: String, // We'll use String for DB compatibility
    pub title: String,
    pub description: Option<String>,
    pub category: String,
    pub brand_name: Option<String>,
    pub original_value: Option<BigDecimal>,
    pub selling_price: BigDecimal,
    pub discount_percentage: Option<BigDecimal>,
    pub expiration_date: Option<DateTime<Utc>>,
    pub proof_image_url: Option<String>,
    pub status: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub view_count: i32,
    pub tags: Vec<String>,
    pub is_verified: bool,
    pub verification_date: Option<DateTime<Utc>>,
}

// Create Listing Request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateListingRequest {
    pub listing_type: ListingType,
    pub title: String,
    pub description: Option<String>,
    pub category: String,
    pub brand_name: Option<String>,
    pub original_value: Option<BigDecimal>,
    pub selling_price: BigDecimal,
    pub discount_percentage: Option<BigDecimal>,
    pub expiration_date: Option<DateTime<Utc>>,
    pub proof_image_url: Option<String>,
    pub tags: Vec<String>,
    pub coupon_code: Option<String>, // For discount code listings
}

// Update Listing Request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateListingRequest {
    pub title: Option<String>,
    pub description: Option<String>,
    pub category: Option<String>,
    pub brand_name: Option<String>,
    pub original_value: Option<f64>,
    pub selling_price: Option<f64>,
    pub discount_percentage: Option<f64>,
    pub expiration_date: Option<DateTime<Utc>>,
    pub proof_image_url: Option<String>,
    pub tags: Option<Vec<String>>,
}

// Marketplace Transaction Model
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct MarketplaceTransaction {
    pub id: Uuid,
    pub listing_id: Uuid,
    pub buyer_id: String,
    pub seller_id: String,
    pub amount: f64,
    pub status: String,
    pub payment_method: Option<String>,
    pub payment_id: Option<String>,
    pub escrow_release_date: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
    pub cancellation_reason: Option<String>,
    pub dispute_reason: Option<String>,
}

// Create Transaction Request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateTransactionRequest {
    pub listing_id: Uuid,
    pub payment_method: String,
}

// Update Transaction Request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateTransactionRequest {
    pub status: Option<String>,
    pub cancellation_reason: Option<String>,
    pub dispute_reason: Option<String>,
}

// Marketplace Review Model
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct MarketplaceReview {
    pub id: Uuid,
    pub transaction_id: Uuid,
    pub reviewer_id: String,
    pub reviewed_user_id: String,
    pub rating: i32,
    pub review_text: Option<String>,
    pub deal_verified: bool,
    pub created_at: DateTime<Utc>,
    pub is_buyer_review: bool,
}

// Create Review Request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateReviewRequest {
    pub transaction_id: Uuid,
    pub rating: i32,
    pub review_text: Option<String>,
    pub deal_verified: bool,
}

// Trust Score Model
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct MarketplaceTrustScore {
    pub user_id: String,
    pub total_transactions: i32,
    pub successful_transactions: i32,
    pub average_rating: f64,
    pub total_reviews: i32,
    pub verified_seller: bool,
    pub trust_score: f64,
    pub last_calculated: DateTime<Utc>,
}

// Payment Method Model
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct UserPaymentMethod {
    pub id: Uuid,
    pub user_id: String,
    pub payment_type: String,
    pub provider_customer_id: Option<String>,
    pub last_four: Option<String>,
    pub card_brand: Option<String>,
    pub is_default: bool,
    pub created_at: DateTime<Utc>,
}

// Create Payment Method Request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreatePaymentMethodRequest {
    pub payment_type: String,
    pub provider_customer_id: Option<String>,
    pub last_four: Option<String>,
    pub card_brand: Option<String>,
    pub is_default: bool,
}

// Verification Queue Model
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct MarketplaceVerificationQueue {
    pub id: Uuid,
    pub listing_id: Uuid,
    pub verifier_id: Option<String>,
    pub verification_status: String,
    pub verification_notes: Option<String>,
    pub submitted_at: DateTime<Utc>,
    pub verified_at: Option<DateTime<Utc>>,
}

// Notification Model
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct MarketplaceNotification {
    pub id: Uuid,
    pub user_id: String,
    pub notification_type: String,
    pub title: String,
    pub message: String,
    pub related_listing_id: Option<Uuid>,
    pub related_transaction_id: Option<Uuid>,
    pub is_read: bool,
    pub created_at: DateTime<Utc>,
}

// Create Notification Request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateNotificationRequest {
    pub user_id: String,
    pub notification_type: String,
    pub title: String,
    pub message: String,
    pub related_listing_id: Option<Uuid>,
    pub related_transaction_id: Option<Uuid>,
}

// Listing Filter Options
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListingFilters {
    pub category: Option<String>,
    pub listing_type: Option<String>,
    pub min_price: Option<f64>,
    pub max_price: Option<f64>,
    pub seller_id: Option<String>,
    pub status: Option<String>,
    pub is_verified: Option<bool>,
    pub search_query: Option<String>,
    pub sort_by: Option<String>, // "price_asc", "price_desc", "created_at", "popularity"
    pub page: Option<i64>,
    pub limit: Option<i64>,
}

// Marketplace Profile Response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketplaceProfile {
    pub user_id: String,
    pub username: String,
    pub profile_image_url: Option<String>,
    pub trust_score: MarketplaceTrustScore,
    pub total_listings: i64,
    pub active_listings: i64,
    pub completed_sales: i64,
    pub member_since: DateTime<Utc>,
}

// Transaction Summary for Dashboard
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransactionSummary {
    pub total_sales: f64,
    pub total_purchases: f64,
    pub pending_transactions: i64,
    pub completed_transactions: i64,
    pub average_transaction_value: f64,
}

// Listing with Seller Info
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListingWithSeller {
    #[serde(flatten)]
    pub listing: MarketplaceListing,
    pub seller_username: String,
    pub seller_trust_score: f64,
    pub seller_profile_image: Option<String>,
}

// Transaction Detail with Listing and User Info
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransactionDetail {
    #[serde(flatten)]
    pub transaction: MarketplaceTransaction,
    pub listing: MarketplaceListing,
    pub buyer_username: String,
    pub seller_username: String,
    pub can_review: bool,
    pub has_reviewed: bool,
}

// Notification Settings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotificationSettings {
    pub email_notifications: bool,
    pub push_notifications: bool,
    pub new_listing_alerts: bool,
    pub price_drop_alerts: bool,
    pub transaction_updates: bool,
    pub review_notifications: bool,
}
