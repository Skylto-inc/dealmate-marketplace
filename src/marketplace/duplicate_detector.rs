use crate::error::AppError;
use sha2::{Sha256, Digest};
use sqlx::PgPool;
use std::collections::HashMap;
use uuid::Uuid;

pub struct DuplicateDetector {
    pool: PgPool,
}

impl DuplicateDetector {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Generate a fingerprint for a coupon code
    fn generate_fingerprint(code: &str, category: &str, brand: Option<&str>) -> String {
        let mut hasher = Sha256::new();
        
        // Normalize the code (uppercase, remove spaces)
        let normalized_code = code.to_uppercase().replace(" ", "").replace("-", "");
        hasher.update(normalized_code.as_bytes());
        hasher.update(category.as_bytes());
        
        if let Some(b) = brand {
            hasher.update(b.to_lowercase().as_bytes());
        }
        
        format!("{:x}", hasher.finalize())
    }

    /// Check if a similar listing already exists
    pub async fn check_duplicate(
        &self,
        coupon_code: &str,
        category: &str,
        brand: Option<&str>,
        seller_id: &str,
    ) -> Result<Option<DuplicateInfo>, AppError> {
        let _fingerprint = Self::generate_fingerprint(coupon_code, category, brand);
        
        // For now, skip exact match checking due to encryption complexity
        // In production, you'd decrypt and compare or use a separate hash field
        let exact_match: Option<DuplicateInfo> = None;

        if let Some(duplicate) = exact_match {
            return Ok(Some(duplicate));
        }

        // Check for similar patterns (fuzzy matching)
        let similar_matches = self.find_similar_listings(
            coupon_code,
            category,
            brand,
            seller_id
        ).await?;

        Ok(similar_matches.into_iter().next())
    }

    /// Find listings with similar coupon patterns
    async fn find_similar_listings(
        &self,
        coupon_code: &str,
        category: &str,
        brand: Option<&str>,
        seller_id: &str,
    ) -> Result<Vec<DuplicateInfo>, AppError> {
        // Get active listings in the same category
        let listings = sqlx::query!(
            r#"
            SELECT 
                ml.id,
                ml.title,
                ml.seller_id,
                ml.brand_name,
                u.username as seller_username
            FROM marketplace_listings ml
            LEFT JOIN users u ON ml.seller_id = u.auth0_id
            WHERE ml.status = 'active'
            AND ml.category = $1
            AND ml.seller_id != $2
            AND ($3::text IS NULL OR ml.brand_name = $3)
            ORDER BY ml.created_at DESC
            LIMIT 100
            "#,
            category,
            seller_id,
            brand
        )
        .fetch_all(&self.pool)
        .await?;

        let mut duplicates = Vec::new();
        let _code_pattern = Self::extract_pattern(coupon_code);

        for listing in listings {
            // Calculate similarity based on title and brand
            let title_similarity = Self::calculate_similarity(&listing.title, coupon_code);
            let brand_match = brand.is_some() && 
                listing.brand_name.as_deref() == brand;

            let confidence = if brand_match && title_similarity > 0.7 {
                85
            } else if title_similarity > 0.8 {
                75
            } else {
                continue;
            };

            duplicates.push(DuplicateInfo {
                listing_id: listing.id.to_string(),
                title: listing.title,
                seller_username: listing.seller_username,
                match_type: MatchType::Similar,
                confidence,
            });
        }

        Ok(duplicates)
    }

    /// Extract pattern from coupon code (e.g., "SAVE20" -> "SAVE##")
    fn extract_pattern(code: &str) -> String {
        code.chars()
            .map(|c| if c.is_numeric() { '#' } else { c.to_uppercase().next().unwrap() })
            .collect()
    }

    /// Calculate similarity between two strings (simple Jaccard similarity)
    fn calculate_similarity(s1: &str, s2: &str) -> f64 {
        let s1_lower = s1.to_lowercase();
        let s2_lower = s2.to_lowercase();
        
        let s1_tokens: std::collections::HashSet<_> = s1_lower
            .split_whitespace()
            .collect();
        let s2_tokens: std::collections::HashSet<_> = s2_lower
            .split_whitespace()
            .collect();

        if s1_tokens.is_empty() || s2_tokens.is_empty() {
            return 0.0;
        }

        let intersection = s1_tokens.intersection(&s2_tokens).count() as f64;
        let union = s1_tokens.union(&s2_tokens).count() as f64;

        intersection / union
    }

    /// Store fingerprint for new listing
    pub async fn store_fingerprint(
        &self,
        listing_id: &str,
        coupon_code: &str,
        category: &str,
        brand: Option<&str>,
    ) -> Result<(), AppError> {
        let fingerprint = Self::generate_fingerprint(coupon_code, category, brand);
        
        // Parse the listing_id string to UUID
        let listing_uuid = Uuid::parse_str(listing_id)
            .map_err(|_| AppError::BadRequest("Invalid listing ID format".to_string()))?;
        
        // Store in a separate fingerprints table for faster lookups
        sqlx::query!(
            r#"
            INSERT INTO marketplace_fingerprints (listing_id, fingerprint, created_at)
            VALUES ($1, $2, CURRENT_TIMESTAMP)
            ON CONFLICT (listing_id) DO UPDATE SET fingerprint = $2
            "#,
            listing_uuid,
            fingerprint
        )
        .execute(&self.pool)
        .await?;

        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct DuplicateInfo {
    pub listing_id: String,
    pub title: String,
    pub seller_username: String,
    pub match_type: MatchType,
    pub confidence: u8, // 0-100
}

#[derive(Debug, Clone)]
pub enum MatchType {
    Exact,
    Similar,
}
