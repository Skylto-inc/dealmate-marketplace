pub mod routes;
pub mod duplicate_detector;
pub mod rate_limiter;
pub mod cache;

use crate::auth::AuthUser;
use crate::error::AppError;
use crate::models::marketplace::*;
use crate::services::encryption::EncryptionService;
use chrono::Utc;
use sqlx::{PgPool, Row};
use uuid::Uuid;
use self::duplicate_detector::DuplicateDetector;
use self::rate_limiter::{RateLimiter, ActionType};
use self::cache::{MarketplaceCache, cache_ttl};

pub struct MarketplaceService {
    pool: PgPool,
}

impl MarketplaceService {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    // Listing Management
    pub async fn create_listing(
        &self,
        auth_user: &AuthUser,
        request: CreateListingRequest,
    ) -> Result<MarketplaceListing, AppError> {
        let listing_id = Uuid::new_v4();
        let now = Utc::now();

        // Calculate discount percentage if original value is provided
        let discount_percentage = request.original_value.as_ref().map(|original| {
            let hundred = bigdecimal::BigDecimal::from(100);
            let diff = original - &request.selling_price;
            let percentage = (diff / original) * hundred;
            percentage
        });

        let query = r#"
            INSERT INTO marketplace_listings (
                id, seller_id, listing_type, title, description, category,
                brand_name, original_value, selling_price, discount_percentage,
                expiration_date, proof_image_url, tags, created_at, updated_at
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15)
            RETURNING *
        "#;

        let listing = sqlx::query_as::<_, MarketplaceListing>(query)
            .bind(listing_id)
            .bind(&auth_user.0.auth0_id)
            .bind(&request.listing_type)
            .bind(&request.title)
            .bind(&request.description)
            .bind(&request.category)
            .bind(&request.brand_name)
            .bind(request.original_value)
            .bind(request.selling_price)
            .bind(discount_percentage)
            .bind(request.expiration_date)
            .bind(&request.proof_image_url)
            .bind(&request.tags)
            .bind(now)
            .bind(now)
            .fetch_one(&self.pool)
            .await?;

        // Store coupon code securely if it's a discount code listing
        if request.listing_type == ListingType::DiscountCode {
            if let Some(coupon_code) = request.coupon_code {
                // Get encryption key from environment or generate one
                let encryption_key = std::env::var("ENCRYPTION_KEY")
                    .unwrap_or_else(|_| EncryptionService::generate_key());
                let encryption_service = EncryptionService::new(&encryption_key)?;
                
                // Encrypt the coupon code
                let (encrypted_code, nonce) = encryption_service.encrypt_string(&coupon_code)?;
                
                // Store encrypted code with nonce
                let combined = format!("{}:{}", encrypted_code, nonce);
                
                sqlx::query(
                    "INSERT INTO marketplace_coupon_codes (listing_id, encrypted_code) VALUES ($1, $2)"
                )
                .bind(listing_id)
                .bind(&combined)
                .execute(&self.pool)
                .await?;
            }
        }

        // Create trust score entry for new sellers
        self.ensure_trust_score(&auth_user.0.auth0_id).await?;

        Ok(listing)
    }

    pub async fn get_listing(&self, listing_id: Uuid) -> Result<ListingWithSeller, AppError> {
        // Increment view count
        sqlx::query("UPDATE marketplace_listings SET view_count = view_count + 1 WHERE id = $1")
            .bind(listing_id)
            .execute(&self.pool)
            .await?;

        let query = r#"
            SELECT 
                l.*,
                u.username as seller_username,
                COALESCE(ts.trust_score, 50.0) as seller_trust_score,
                u.email as seller_profile_image
            FROM marketplace_listings l
            LEFT JOIN users u ON l.seller_id = u.auth0_id
            LEFT JOIN marketplace_trust_scores ts ON l.seller_id = ts.user_id
            WHERE l.id = $1
        "#;

        let row = sqlx::query(query)
            .bind(listing_id)
            .fetch_optional(&self.pool)
            .await?
            .ok_or_else(|| AppError::NotFound("Listing not found".to_string()))?;

        let listing = MarketplaceListing {
            id: row.get("id"),
            seller_id: row.get("seller_id"),
            listing_type: row.get("listing_type"),
            title: row.get("title"),
            description: row.get("description"),
            category: row.get("category"),
            brand_name: row.get("brand_name"),
            original_value: row.get("original_value"),
            selling_price: row.get("selling_price"),
            discount_percentage: row.get("discount_percentage"),
            expiration_date: row.get("expiration_date"),
            proof_image_url: row.get("proof_image_url"),
            status: row.get("status"),
            created_at: row.get("created_at"),
            updated_at: row.get("updated_at"),
            view_count: row.get("view_count"),
            tags: row.get("tags"),
            is_verified: row.get("is_verified"),
            verification_date: row.get("verification_date"),
        };

        Ok(ListingWithSeller {
            listing,
            seller_username: row.get("seller_username"),
            seller_trust_score: row.get("seller_trust_score"),
            seller_profile_image: row.get("seller_profile_image"),
        })
    }

    pub async fn get_listings(
        &self,
        filters: ListingFilters,
    ) -> Result<Vec<ListingWithSeller>, AppError> {
        let mut query = r#"
            SELECT 
                l.*,
                u.username as seller_username,
                COALESCE(ts.trust_score, 50.0) as seller_trust_score,
                u.email as seller_profile_image
            FROM marketplace_listings l
            LEFT JOIN users u ON l.seller_id = u.auth0_id
            LEFT JOIN marketplace_trust_scores ts ON l.seller_id = ts.user_id
            WHERE 1=1
        "#.to_string();

        let mut bindings = vec![];
        let mut bind_count = 1;

        // Apply filters
        if let Some(category) = &filters.category {
            query.push_str(&format!(" AND l.category = ${}", bind_count));
            bindings.push(category.clone());
            bind_count += 1;
        }

        if let Some(listing_type) = &filters.listing_type {
            query.push_str(&format!(" AND l.listing_type = ${}", bind_count));
            bindings.push(listing_type.clone());
            bind_count += 1;
        }

        if let Some(min_price) = filters.min_price {
            query.push_str(&format!(" AND l.selling_price >= ${}", bind_count));
            bindings.push(min_price.to_string());
            bind_count += 1;
        }

        if let Some(max_price) = filters.max_price {
            query.push_str(&format!(" AND l.selling_price <= ${}", bind_count));
            bindings.push(max_price.to_string());
            bind_count += 1;
        }

        if let Some(seller_id) = &filters.seller_id {
            query.push_str(&format!(" AND l.seller_id = ${}", bind_count));
            bindings.push(seller_id.clone());
            bind_count += 1;
        }

        if let Some(status) = &filters.status {
            query.push_str(&format!(" AND l.status = ${}", bind_count));
            bindings.push(status.clone());
            bind_count += 1;
        }

        if let Some(is_verified) = filters.is_verified {
            query.push_str(&format!(" AND l.is_verified = ${}", bind_count));
            bindings.push(is_verified.to_string());
            bind_count += 1;
        }

        if let Some(search_query) = &filters.search_query {
            query.push_str(&format!(
                " AND (l.title ILIKE ${} OR l.description ILIKE ${} OR l.brand_name ILIKE ${})",
                bind_count,
                bind_count + 1,
                bind_count + 2
            ));
            let search_pattern = format!("%{}%", search_query);
            bindings.push(search_pattern.clone());
            bindings.push(search_pattern.clone());
            bindings.push(search_pattern);
            bind_count += 3;
        }

        // Apply sorting
        match filters.sort_by.as_deref() {
            Some("price_asc") => query.push_str(" ORDER BY l.selling_price ASC"),
            Some("price_desc") => query.push_str(" ORDER BY l.selling_price DESC"),
            Some("popularity") => query.push_str(" ORDER BY l.view_count DESC"),
            _ => query.push_str(" ORDER BY l.created_at DESC"),
        }

        // Apply pagination
        let limit = filters.limit.unwrap_or(20).min(100);
        let offset = filters.page.unwrap_or(0) * limit;
        query.push_str(&format!(" LIMIT {} OFFSET {}", limit, offset));

        // Execute query with dynamic bindings
        let mut sql_query = sqlx::query(&query);
        for binding in bindings {
            sql_query = sql_query.bind(binding);
        }

        let rows = sql_query
            .fetch_all(&self.pool)
            .await?;

        let listings = rows
            .into_iter()
            .map(|row| {
                let listing = MarketplaceListing {
                    id: row.get("id"),
                    seller_id: row.get("seller_id"),
                    listing_type: row.get("listing_type"),
                    title: row.get("title"),
                    description: row.get("description"),
                    category: row.get("category"),
                    brand_name: row.get("brand_name"),
                    original_value: row.get("original_value"),
                    selling_price: row.get("selling_price"),
                    discount_percentage: row.get("discount_percentage"),
                    expiration_date: row.get("expiration_date"),
                    proof_image_url: row.get("proof_image_url"),
                    status: row.get("status"),
                    created_at: row.get("created_at"),
                    updated_at: row.get("updated_at"),
                    view_count: row.get("view_count"),
                    tags: row.get("tags"),
                    is_verified: row.get("is_verified"),
                    verification_date: row.get("verification_date"),
                };

                ListingWithSeller {
                    listing,
                    seller_username: row.get("seller_username"),
                    seller_trust_score: row.get("seller_trust_score"),
                    seller_profile_image: row.get("seller_profile_image"),
                }
            })
            .collect();

        Ok(listings)
    }

    pub async fn update_listing(
        &self,
        auth_user: &AuthUser,
        listing_id: Uuid,
        request: UpdateListingRequest,
    ) -> Result<MarketplaceListing, AppError> {
        // Verify ownership
        let existing = sqlx::query("SELECT seller_id FROM marketplace_listings WHERE id = $1")
            .bind(listing_id)
            .fetch_optional(&self.pool)
            .await?
            .ok_or_else(|| AppError::NotFound("Listing not found".to_string()))?;

        let seller_id: String = existing.get("seller_id");
        if seller_id != auth_user.0.auth0_id {
            return Err(AppError::NotFound("You can only update your own listings".to_string()));
        }

        // Build update query dynamically
        let mut query = "UPDATE marketplace_listings SET updated_at = CURRENT_TIMESTAMP".to_string();
        let mut bindings = vec![];
        let mut bind_count = 1;

        if let Some(title) = &request.title {
            query.push_str(&format!(", title = ${}", bind_count));
            bindings.push(title.clone());
            bind_count += 1;
        }

        // Add other fields similarly...

        query.push_str(&format!(" WHERE id = ${} RETURNING *", bind_count));

        let mut sql_query = sqlx::query_as::<_, MarketplaceListing>(&query);
        for binding in bindings {
            sql_query = sql_query.bind(binding);
        }
        sql_query = sql_query.bind(listing_id);

        let listing = sql_query
            .fetch_one(&self.pool)
            .await?;

        Ok(listing)
    }

    pub async fn delete_listing(
        &self,
        auth_user: &AuthUser,
        listing_id: Uuid,
    ) -> Result<(), AppError> {
        let result = sqlx::query(
            "DELETE FROM marketplace_listings WHERE id = $1 AND seller_id = $2"
        )
        .bind(listing_id)
        .bind(&auth_user.0.auth0_id)
        .execute(&self.pool)
        .await?;

        if result.rows_affected() == 0 {
            return Err(AppError::NotFound("Listing not found or you don't have permission".to_string()));
        }

        Ok(())
    }

    // Transaction Management
    pub async fn create_transaction(
        &self,
        auth_user: &AuthUser,
        request: CreateTransactionRequest,
    ) -> Result<MarketplaceTransaction, AppError> {
        // Get listing details
        let listing = sqlx::query(
            "SELECT seller_id, selling_price, status FROM marketplace_listings WHERE id = $1"
        )
        .bind(request.listing_id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| AppError::NotFound("Listing not found".to_string()))?;

        let seller_id: String = listing.get("seller_id");
        let selling_price: f64 = listing.get("selling_price");
        let status: String = listing.get("status");

        // Verify listing is active
        if status != "active" {
            return Err(AppError::NotFound("Listing is not available for purchase".to_string()));
        }

        // Prevent self-purchase
        if seller_id == auth_user.0.auth0_id {
            return Err(AppError::NotFound("You cannot purchase your own listing".to_string()));
        }

        // Create transaction
        let transaction_id = Uuid::new_v4();
        let query = r#"
            INSERT INTO marketplace_transactions (
                id, listing_id, buyer_id, seller_id, amount, 
                payment_method, status, created_at
            ) VALUES ($1, $2, $3, $4, $5, $6, 'pending', CURRENT_TIMESTAMP)
            RETURNING *
        "#;

        let transaction = sqlx::query_as::<_, MarketplaceTransaction>(query)
            .bind(transaction_id)
            .bind(request.listing_id)
            .bind(&auth_user.0.auth0_id)
            .bind(&seller_id)
            .bind(selling_price)
            .bind(&request.payment_method)
            .fetch_one(&self.pool)
            .await?;

        // Update listing status
        sqlx::query("UPDATE marketplace_listings SET status = 'sold' WHERE id = $1")
            .bind(request.listing_id)
            .execute(&self.pool)
            .await?;

        // Create notification for seller
        self.create_notification(
            &seller_id,
            "new_sale",
            "New Sale!",
            &format!("Your listing has been purchased"),
            Some(request.listing_id),
            Some(transaction_id),
        ).await?;

        Ok(transaction)
    }

    pub async fn complete_transaction(
        &self,
        auth_user: &AuthUser,
        transaction_id: Uuid,
    ) -> Result<MarketplaceTransaction, AppError> {
        // Get transaction details
        let transaction = self.get_transaction_by_id(transaction_id).await?;

        // Verify buyer
        if transaction.buyer_id != auth_user.0.auth0_id {
            return Err(AppError::NotFound("Only the buyer can complete this transaction".to_string()));
        }

        // Verify status
        if transaction.status != "escrow" {
            return Err(AppError::NotFound("Transaction is not in escrow status".to_string()));
        }

        // Update transaction
        let query = r#"
            UPDATE marketplace_transactions 
            SET status = 'completed', completed_at = CURRENT_TIMESTAMP
            WHERE id = $1
            RETURNING *
        "#;

        let updated = sqlx::query_as::<_, MarketplaceTransaction>(query)
            .bind(transaction_id)
            .fetch_one(&self.pool)
            .await?;

        // Grant access to coupon code if applicable
        sqlx::query(
            r#"
            INSERT INTO marketplace_coupon_access (listing_id, user_id, transaction_id)
            VALUES ($1, $2, $3)
            ON CONFLICT (listing_id, user_id) DO NOTHING
            "#
        )
        .bind(transaction.listing_id)
        .bind(&auth_user.0.auth0_id)
        .bind(transaction_id)
        .execute(&self.pool)
        .await?;

        // Update trust scores
        self.update_trust_score_after_transaction(&transaction.seller_id, true).await?;

        // Create notification for seller
        self.create_notification(
            &transaction.seller_id,
            "transaction_completed",
            "Transaction Completed!",
            "Your sale has been completed and funds will be released",
            Some(transaction.listing_id),
            Some(transaction_id),
        ).await?;

        Ok(updated)
    }

    // Review Management
    pub async fn create_review(
        &self,
        auth_user: &AuthUser,
        request: CreateReviewRequest,
    ) -> Result<MarketplaceReview, AppError> {
        // Get transaction details
        let transaction = self.get_transaction_by_id(request.transaction_id).await?;

        // Verify transaction is completed
        if transaction.status != "completed" {
            return Err(AppError::NotFound("Can only review completed transactions".to_string()));
        }

        // Determine if this is a buyer or seller review
        let (reviewed_user_id, is_buyer_review) = if transaction.buyer_id == auth_user.0.auth0_id {
            (transaction.seller_id.clone(), true)
        } else if transaction.seller_id == auth_user.0.auth0_id {
            (transaction.buyer_id.clone(), false)
        } else {
            return Err(AppError::NotFound("You are not part of this transaction".to_string()));
        };

        // Check if already reviewed
        let existing = sqlx::query(
            "SELECT id FROM marketplace_reviews WHERE transaction_id = $1 AND reviewer_id = $2"
        )
        .bind(request.transaction_id)
        .bind(&auth_user.0.auth0_id)
        .fetch_optional(&self.pool)
        .await?;

        if existing.is_some() {
            return Err(AppError::NotFound("You have already reviewed this transaction".to_string()));
        }

        // Create review
        let review_id = Uuid::new_v4();
        let query = r#"
            INSERT INTO marketplace_reviews (
                id, transaction_id, reviewer_id, reviewed_user_id, 
                rating, review_text, deal_verified, is_buyer_review, created_at
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, CURRENT_TIMESTAMP)
            RETURNING *
        "#;

        let review = sqlx::query_as::<_, MarketplaceReview>(query)
            .bind(review_id)
            .bind(request.transaction_id)
            .bind(&auth_user.0.auth0_id)
            .bind(&reviewed_user_id)
            .bind(request.rating)
            .bind(&request.review_text)
            .bind(request.deal_verified)
            .bind(is_buyer_review)
            .fetch_one(&self.pool)
            .await?;

        // Update trust score
        self.recalculate_trust_score(&reviewed_user_id).await?;

        // Create notification
        self.create_notification(
            &reviewed_user_id,
            "new_review",
            "New Review Received",
            &format!("You received a {}-star review", request.rating),
            None,
            Some(request.transaction_id),
        ).await?;

        Ok(review)
    }

    // Trust Score Management
    async fn ensure_trust_score(&self, user_id: &str) -> Result<(), AppError> {
        sqlx::query(
            r#"
            INSERT INTO marketplace_trust_scores (user_id, trust_score, last_calculated)
            VALUES ($1, 50.0, CURRENT_TIMESTAMP)
            ON CONFLICT (user_id) DO NOTHING
            "#
        )
        .bind(user_id)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    async fn update_trust_score_after_transaction(
        &self,
        user_id: &str,
        successful: bool,
    ) -> Result<(), AppError> {
        let query = if successful {
            r#"
            UPDATE marketplace_trust_scores 
            SET total_transactions = total_transactions + 1,
                successful_transactions = successful_transactions + 1,
                last_calculated = CURRENT_TIMESTAMP
            WHERE user_id = $1
            "#
        } else {
            r#"
            UPDATE marketplace_trust_scores 
            SET total_transactions = total_transactions + 1,
                last_calculated = CURRENT_TIMESTAMP
            WHERE user_id = $1
            "#
        };

        sqlx::query(query)
            .bind(user_id)
            .execute(&self.pool)
            .await?;

        self.recalculate_trust_score(user_id).await?;
        Ok(())
    }

    async fn recalculate_trust_score(&self, user_id: &str) -> Result<(), AppError> {
        // Get current stats
        let stats = sqlx::query(
            r#"
            SELECT 
                ts.total_transactions,
                ts.successful_transactions,
                ts.verified_seller,
                COUNT(r.id) as review_count,
                AVG(r.rating) as avg_rating
            FROM marketplace_trust_scores ts
            LEFT JOIN marketplace_reviews r ON r.reviewed_user_id = ts.user_id
            WHERE ts.user_id = $1
            GROUP BY ts.user_id, ts.total_transactions, ts.successful_transactions, ts.verified_seller
            "#
        )
        .bind(user_id)
        .fetch_optional(&self.pool)
        .await?;

        if let Some(row) = stats {
            let total_transactions: i32 = row.get("total_transactions");
            let successful_transactions: i32 = row.get("successful_transactions");
            let verified_seller: bool = row.get("verified_seller");
            let review_count: i64 = row.get("review_count");
            let avg_rating: Option<f64> = row.get("avg_rating");

            // Calculate trust score (0-100)
            let mut score: f64 = 50.0; // Base score

            // Transaction success rate (up to 30 points)
            if total_transactions > 0 {
                let success_rate = successful_transactions as f64 / total_transactions as f64;
                score += success_rate * 30.0;
            }

            // Average rating (up to 30 points)
            if let Some(rating) = avg_rating {
                score += (rating / 5.0) * 30.0;
            }

            // Review count bonus (up to 10 points)
            score += (review_count as f64).min(10.0);

            // Verified seller bonus
            if verified_seller {
                score += 10.0;
            }

            // Cap at 100
            score = score.min(100.0);

            // Update score
            sqlx::query(
                r#"
                UPDATE marketplace_trust_scores 
                SET trust_score = $1,
                    average_rating = $2,
                    total_reviews = $3,
                    last_calculated = CURRENT_TIMESTAMP
                WHERE user_id = $4
                "#
            )
            .bind(score)
            .bind(avg_rating.unwrap_or(0.0))
            .bind(review_count as i32)
            .bind(user_id)
            .execute(&self.pool)
            .await?;
        }

        Ok(())
    }

    // Notification Management
    async fn create_notification(
        &self,
        user_id: &str,
        notification_type: &str,
        title: &str,
        message: &str,
        listing_id: Option<Uuid>,
        transaction_id: Option<Uuid>,
    ) -> Result<(), AppError> {
        let notification_id = Uuid::new_v4();
        let query = r#"
            INSERT INTO marketplace_notifications (
                id, user_id, notification_type, title, message,
                related_listing_id, related_transaction_id, created_at
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, CURRENT_TIMESTAMP)
        "#;

        sqlx::query(query)
            .bind(notification_id)
            .bind(user_id)
            .bind(notification_type)
            .bind(title)
            .bind(message)
            .bind(listing_id)
            .bind(transaction_id)
            .execute(&self.pool)
            .await?;

        Ok(())
    }

    // Helper Methods
    async fn get_transaction_by_id(&self, transaction_id: Uuid) -> Result<MarketplaceTransaction, AppError> {
        sqlx::query_as::<_, MarketplaceTransaction>(
            "SELECT * FROM marketplace_transactions WHERE id = $1"
        )
        .bind(transaction_id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| AppError::NotFound("Transaction not found".to_string()))
    }

    pub async fn get_user_profile(
        &self,
        user_id: &str,
    ) -> Result<MarketplaceProfile, AppError> {
        // Get user info
        let user = sqlx::query("SELECT username, email, created_at FROM users WHERE auth0_id = $1")
            .bind(user_id)
            .fetch_optional(&self.pool)
            .await?
            .ok_or_else(|| AppError::NotFound("User not found".to_string()))?;

        // Get trust score
        self.ensure_trust_score(user_id).await?;
        let trust_score = sqlx::query_as::<_, MarketplaceTrustScore>(
            "SELECT * FROM marketplace_trust_scores WHERE user_id = $1"
        )
        .bind(user_id)
        .fetch_one(&self.pool)
        .await?;

        // Get listing stats
        let listing_stats = sqlx::query(
            r#"
            SELECT 
                COUNT(*) as total_listings,
                COUNT(*) FILTER (WHERE status = 'active') as active_listings,
                COUNT(*) FILTER (WHERE status = 'sold') as completed_sales
            FROM marketplace_listings
            WHERE seller_id = $1
            "#
        )
        .bind(user_id)
        .fetch_one(&self.pool)
        .await?;

        Ok(MarketplaceProfile {
            user_id: user_id.to_string(),
            username: user.get("username"),
            profile_image_url: user.get("email"),
            trust_score,
            total_listings: listing_stats.get("total_listings"),
            active_listings: listing_stats.get("active_listings"),
            completed_sales: listing_stats.get("completed_sales"),
            member_since: user.get("created_at"),
        })
    }

    // Coupon Code Management
    pub async fn get_coupon_code(
        &self,
        auth_user: &AuthUser,
        listing_id: Uuid,
    ) -> Result<Option<String>, AppError> {
        // Check if user has access (either seller or has purchased)
        let has_access = sqlx::query(
            r#"
            SELECT 1 FROM marketplace_listings WHERE id = $1 AND seller_id = $2
            UNION
            SELECT 1 FROM marketplace_coupon_access WHERE listing_id = $1 AND user_id = $2
            "#
        )
        .bind(listing_id)
        .bind(&auth_user.0.auth0_id)
        .fetch_optional(&self.pool)
        .await?;

        if has_access.is_none() {
            return Ok(None);
        }

        // Get encrypted code
        let result = sqlx::query(
            "SELECT encrypted_code FROM marketplace_coupon_codes WHERE listing_id = $1"
        )
        .bind(listing_id)
        .fetch_optional(&self.pool)
        .await?;

        if let Some(row) = result {
            let encrypted_code: String = row.get("encrypted_code");
            
            // Split the encrypted code and nonce
            let parts: Vec<&str> = encrypted_code.split(':').collect();
            if parts.len() != 2 {
                return Err(AppError::InternalError("Invalid encrypted data format".to_string()));
            }
            
            // Get encryption key from environment
            let encryption_key = std::env::var("ENCRYPTION_KEY")
                .unwrap_or_else(|_| EncryptionService::generate_key());
            let encryption_service = EncryptionService::new(&encryption_key)?;
            
            // Decrypt the coupon code
            let decrypted_code = encryption_service.decrypt_string(parts[0], parts[1])?;
            Ok(Some(decrypted_code))
        } else {
            Ok(None)
        }
    }
}
