use crate::error::AppError;
use crate::models::marketplace::{ListingWithSeller, MarketplaceProfile};
use redis::{AsyncCommands, Client};
use serde::{Deserialize, Serialize};
use std::time::Duration;
use uuid::Uuid;

pub struct MarketplaceCache {
    redis_client: Option<Client>,
}

impl MarketplaceCache {
    pub fn new(redis_url: Option<String>) -> Self {
        let redis_client = redis_url.and_then(|url| {
            Client::open(url).ok()
        });

        Self { redis_client }
    }

    /// Cache listing data
    pub async fn cache_listing(
        &self,
        listing_id: &Uuid,
        listing: &ListingWithSeller,
        ttl_seconds: u64,
    ) -> Result<(), AppError> {
        if let Some(client) = &self.redis_client {
            let mut conn = client.get_async_connection().await
                .map_err(|e| AppError::InternalError(format!("Redis connection error: {}", e)))?;

            let key = format!("listing:{}", listing_id);
            let serialized = serde_json::to_string(listing)
                .map_err(|e| AppError::InternalError(format!("Serialization error: {}", e)))?;

            conn.set_ex::<_, _, ()>(&key, serialized, ttl_seconds).await
                .map_err(|e| AppError::InternalError(format!("Redis set error: {}", e)))?;
        }
        Ok(())
    }

    /// Get cached listing
    pub async fn get_listing(&self, listing_id: &Uuid) -> Result<Option<ListingWithSeller>, AppError> {
        if let Some(client) = &self.redis_client {
            let mut conn = client.get_async_connection().await
                .map_err(|e| AppError::InternalError(format!("Redis connection error: {}", e)))?;

            let key = format!("listing:{}", listing_id);
            let result: Option<String> = conn.get(&key).await
                .map_err(|e| AppError::InternalError(format!("Redis get error: {}", e)))?;

            if let Some(data) = result {
                let listing = serde_json::from_str(&data)
                    .map_err(|e| AppError::InternalError(format!("Deserialization error: {}", e)))?;
                return Ok(Some(listing));
            }
        }
        Ok(None)
    }

    /// Invalidate listing cache
    pub async fn invalidate_listing(&self, listing_id: &Uuid) -> Result<(), AppError> {
        if let Some(client) = &self.redis_client {
            let mut conn = client.get_async_connection().await
                .map_err(|e| AppError::InternalError(format!("Redis connection error: {}", e)))?;

            let key = format!("listing:{}", listing_id);
            conn.del::<_, ()>(&key).await
                .map_err(|e| AppError::InternalError(format!("Redis del error: {}", e)))?;
        }
        Ok(())
    }

    /// Cache user profile
    pub async fn cache_profile(
        &self,
        user_id: &str,
        profile: &MarketplaceProfile,
        ttl_seconds: u64,
    ) -> Result<(), AppError> {
        if let Some(client) = &self.redis_client {
            let mut conn = client.get_async_connection().await
                .map_err(|e| AppError::InternalError(format!("Redis connection error: {}", e)))?;

            let key = format!("profile:{}", user_id);
            let serialized = serde_json::to_string(profile)
                .map_err(|e| AppError::InternalError(format!("Serialization error: {}", e)))?;

            conn.set_ex::<_, _, ()>(&key, serialized, ttl_seconds).await
                .map_err(|e| AppError::InternalError(format!("Redis set error: {}", e)))?;
        }
        Ok(())
    }

    /// Get cached profile
    pub async fn get_profile(&self, user_id: &str) -> Result<Option<MarketplaceProfile>, AppError> {
        if let Some(client) = &self.redis_client {
            let mut conn = client.get_async_connection().await
                .map_err(|e| AppError::InternalError(format!("Redis connection error: {}", e)))?;

            let key = format!("profile:{}", user_id);
            let result: Option<String> = conn.get(&key).await
                .map_err(|e| AppError::InternalError(format!("Redis get error: {}", e)))?;

            if let Some(data) = result {
                let profile = serde_json::from_str(&data)
                    .map_err(|e| AppError::InternalError(format!("Deserialization error: {}", e)))?;
                return Ok(Some(profile));
            }
        }
        Ok(None)
    }

    /// Cache category statistics
    pub async fn cache_category_stats(
        &self,
        category: &str,
        stats: &CategoryStats,
        ttl_seconds: u64,
    ) -> Result<(), AppError> {
        if let Some(client) = &self.redis_client {
            let mut conn = client.get_async_connection().await
                .map_err(|e| AppError::InternalError(format!("Redis connection error: {}", e)))?;

            let key = format!("category_stats:{}", category);
            let serialized = serde_json::to_string(stats)
                .map_err(|e| AppError::InternalError(format!("Serialization error: {}", e)))?;

            conn.set_ex::<_, _, ()>(&key, serialized, ttl_seconds).await
                .map_err(|e| AppError::InternalError(format!("Redis set error: {}", e)))?;
        }
        Ok(())
    }

    /// Get cached category statistics
    pub async fn get_category_stats(&self, category: &str) -> Result<Option<CategoryStats>, AppError> {
        if let Some(client) = &self.redis_client {
            let mut conn = client.get_async_connection().await
                .map_err(|e| AppError::InternalError(format!("Redis connection error: {}", e)))?;

            let key = format!("category_stats:{}", category);
            let result: Option<String> = conn.get(&key).await
                .map_err(|e| AppError::InternalError(format!("Redis get error: {}", e)))?;

            if let Some(data) = result {
                let stats = serde_json::from_str(&data)
                    .map_err(|e| AppError::InternalError(format!("Deserialization error: {}", e)))?;
                return Ok(Some(stats));
            }
        }
        Ok(None)
    }

    /// Increment view count in cache
    pub async fn increment_view_count(&self, listing_id: &Uuid) -> Result<(), AppError> {
        if let Some(client) = &self.redis_client {
            let mut conn = client.get_async_connection().await
                .map_err(|e| AppError::InternalError(format!("Redis connection error: {}", e)))?;

            let key = format!("views:{}", listing_id);
            conn.incr::<_, _, ()>(&key, 1).await
                .map_err(|e| AppError::InternalError(format!("Redis incr error: {}", e)))?;

            // Set expiry to 1 hour if not already set
            conn.expire::<_, ()>(&key, 3600).await
                .map_err(|e| AppError::InternalError(format!("Redis expire error: {}", e)))?;
        }
        Ok(())
    }

    /// Get view count from cache
    pub async fn get_view_count(&self, listing_id: &Uuid) -> Result<Option<i32>, AppError> {
        if let Some(client) = &self.redis_client {
            let mut conn = client.get_async_connection().await
                .map_err(|e| AppError::InternalError(format!("Redis connection error: {}", e)))?;

            let key = format!("views:{}", listing_id);
            let result: Option<i32> = conn.get(&key).await
                .map_err(|e| AppError::InternalError(format!("Redis get error: {}", e)))?;

            return Ok(result);
        }
        Ok(None)
    }

    /// Cache search results
    pub async fn cache_search_results(
        &self,
        query_hash: &str,
        results: &[ListingWithSeller],
        ttl_seconds: u64,
    ) -> Result<(), AppError> {
        if let Some(client) = &self.redis_client {
            let mut conn = client.get_async_connection().await
                .map_err(|e| AppError::InternalError(format!("Redis connection error: {}", e)))?;

            let key = format!("search:{}", query_hash);
            let serialized = serde_json::to_string(results)
                .map_err(|e| AppError::InternalError(format!("Serialization error: {}", e)))?;

            conn.set_ex::<_, _, ()>(&key, serialized, ttl_seconds).await
                .map_err(|e| AppError::InternalError(format!("Redis set error: {}", e)))?;
        }
        Ok(())
    }

    /// Get cached search results
    pub async fn get_search_results(&self, query_hash: &str) -> Result<Option<Vec<ListingWithSeller>>, AppError> {
        if let Some(client) = &self.redis_client {
            let mut conn = client.get_async_connection().await
                .map_err(|e| AppError::InternalError(format!("Redis connection error: {}", e)))?;

            let key = format!("search:{}", query_hash);
            let result: Option<String> = conn.get(&key).await
                .map_err(|e| AppError::InternalError(format!("Redis get error: {}", e)))?;

            if let Some(data) = result {
                let results = serde_json::from_str(&data)
                    .map_err(|e| AppError::InternalError(format!("Deserialization error: {}", e)))?;
                return Ok(Some(results));
            }
        }
        Ok(None)
    }

    /// Clear all caches for a user (useful when profile or listings change)
    pub async fn clear_user_caches(&self, user_id: &str) -> Result<(), AppError> {
        if let Some(client) = &self.redis_client {
            let mut conn = client.get_async_connection().await
                .map_err(|e| AppError::InternalError(format!("Redis connection error: {}", e)))?;

            // Clear profile cache
            let profile_key = format!("profile:{}", user_id);
            conn.del::<_, ()>(&profile_key).await
                .map_err(|e| AppError::InternalError(format!("Redis del error: {}", e)))?;

            // Clear user's listings (would need to track them separately)
            // For now, we'll rely on TTL expiration
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CategoryStats {
    pub total_listings: i64,
    pub avg_price: f64,
    pub min_price: f64,
    pub max_price: f64,
    pub median_price: f64,
    pub top_brands: Vec<(String, i64)>,
}

impl CategoryStats {
    pub fn cache_duration() -> u64 {
        300 // 5 minutes
    }
}

// Cache TTL constants
pub mod cache_ttl {
    pub const LISTING: u64 = 300; // 5 minutes
    pub const PROFILE: u64 = 600; // 10 minutes
    pub const SEARCH_RESULTS: u64 = 180; // 3 minutes
    pub const CATEGORY_STATS: u64 = 300; // 5 minutes
}
