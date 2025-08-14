use crate::error::AppError;
use chrono::{Duration, Utc};
use sqlx::PgPool;
use std::collections::HashMap;

pub struct RateLimiter {
    pool: PgPool,
    limits: HashMap<ActionType, RateLimit>,
}

#[derive(Debug, Clone, Hash, Eq, PartialEq)]
pub enum ActionType {
    CreateListing,
    CreateTransaction,
    CreateReview,
    SendMessage,
}

#[derive(Debug, Clone)]
pub struct RateLimit {
    max_attempts: i32,
    window_minutes: i32,
}

impl RateLimiter {
    pub fn new(pool: PgPool) -> Self {
        let mut limits = HashMap::new();
        
        // Define rate limits for different actions
        limits.insert(ActionType::CreateListing, RateLimit {
            max_attempts: 10,
            window_minutes: 60, // 10 listings per hour
        });
        
        limits.insert(ActionType::CreateTransaction, RateLimit {
            max_attempts: 50,
            window_minutes: 60, // 50 purchases per hour
        });
        
        limits.insert(ActionType::CreateReview, RateLimit {
            max_attempts: 20,
            window_minutes: 60, // 20 reviews per hour
        });
        
        limits.insert(ActionType::SendMessage, RateLimit {
            max_attempts: 100,
            window_minutes: 60, // 100 messages per hour
        });

        Self { pool, limits }
    }

    /// Check if an action is allowed and increment the counter
    pub async fn check_and_increment(
        &self,
        user_id: &str,
        action: ActionType,
    ) -> Result<RateLimitResult, AppError> {
        let limit = self.limits.get(&action)
            .ok_or_else(|| AppError::InternalError("Unknown action type".to_string()))?;

        let action_str = self.action_to_string(&action);
        let window_start = Utc::now().naive_utc() - Duration::minutes(limit.window_minutes as i64);

        // Clean up old entries
        self.cleanup_old_entries().await?;

        // Check current count
        let result = sqlx::query!(
            r#"
            SELECT count, window_start
            FROM marketplace_rate_limits
            WHERE user_id = $1 
            AND action_type = $2
            AND window_start > $3
            "#,
            user_id,
            action_str,
            window_start
        )
        .fetch_optional(&self.pool)
        .await?;

        match result {
            Some(row) => {
                // Existing record within window
                let count = row.count.unwrap_or(0);
                let window_start_time = row.window_start.unwrap_or(Utc::now().naive_utc());
                
                if count >= limit.max_attempts {
                    // Rate limit exceeded
                    let reset_time = window_start_time + Duration::minutes(limit.window_minutes as i64);
                    return Ok(RateLimitResult {
                        allowed: false,
                        remaining: 0,
                        reset_at: chrono::DateTime::<Utc>::from_naive_utc_and_offset(reset_time, Utc),
                        retry_after: (reset_time - Utc::now().naive_utc()).num_seconds().max(0) as u64,
                    });
                }

                // Increment counter
                let new_count = count + 1;
                sqlx::query!(
                    r#"
                    UPDATE marketplace_rate_limits
                    SET count = $1
                    WHERE user_id = $2 AND action_type = $3
                    "#,
                    new_count,
                    user_id,
                    action_str
                )
                .execute(&self.pool)
                .await?;

                Ok(RateLimitResult {
                    allowed: true,
                    remaining: limit.max_attempts - new_count,
                    reset_at: chrono::DateTime::<Utc>::from_naive_utc_and_offset(
                        window_start_time + Duration::minutes(limit.window_minutes as i64), 
                        Utc
                    ),
                    retry_after: 0,
                })
            }
            None => {
                // No record or expired, create new one
                sqlx::query!(
                    r#"
                    INSERT INTO marketplace_rate_limits (user_id, action_type, count, window_start)
                    VALUES ($1, $2, 1, $3)
                    ON CONFLICT (user_id, action_type) 
                    DO UPDATE SET count = 1, window_start = $3
                    "#,
                    user_id,
                    action_str,
                    Utc::now().naive_utc()
                )
                .execute(&self.pool)
                .await?;

                Ok(RateLimitResult {
                    allowed: true,
                    remaining: limit.max_attempts - 1,
                    reset_at: Utc::now() + Duration::minutes(limit.window_minutes as i64),
                    retry_after: 0,
                })
            }
        }
    }

    /// Check rate limit without incrementing
    pub async fn check_only(
        &self,
        user_id: &str,
        action: ActionType,
    ) -> Result<RateLimitResult, AppError> {
        let limit = self.limits.get(&action)
            .ok_or_else(|| AppError::InternalError("Unknown action type".to_string()))?;

        let action_str = self.action_to_string(&action);
        let window_start = Utc::now().naive_utc() - Duration::minutes(limit.window_minutes as i64);

        let result = sqlx::query!(
            r#"
            SELECT count, window_start
            FROM marketplace_rate_limits
            WHERE user_id = $1 
            AND action_type = $2
            AND window_start > $3
            "#,
            user_id,
            action_str,
            window_start
        )
        .fetch_optional(&self.pool)
        .await?;

        match result {
            Some(row) => {
                let count = row.count.unwrap_or(0);
                let window_start_time = row.window_start.unwrap_or(Utc::now().naive_utc());
                let reset_time = window_start_time + Duration::minutes(limit.window_minutes as i64);
                let allowed = count < limit.max_attempts;
                let remaining = (limit.max_attempts - count).max(0);

                Ok(RateLimitResult {
                    allowed,
                    remaining,
                    reset_at: chrono::DateTime::<Utc>::from_naive_utc_and_offset(reset_time, Utc),
                    retry_after: if allowed { 0 } else {
                        (reset_time - Utc::now().naive_utc()).num_seconds().max(0) as u64
                    },
                })
            }
            None => {
                // No record, so allowed
                Ok(RateLimitResult {
                    allowed: true,
                    remaining: limit.max_attempts,
                    reset_at: Utc::now() + Duration::minutes(limit.window_minutes as i64),
                    retry_after: 0,
                })
            }
        }
    }

    /// Clean up old rate limit entries
    async fn cleanup_old_entries(&self) -> Result<(), AppError> {
        sqlx::query!("SELECT cleanup_old_rate_limits()")
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    fn action_to_string(&self, action: &ActionType) -> &'static str {
        match action {
            ActionType::CreateListing => "create_listing",
            ActionType::CreateTransaction => "create_transaction",
            ActionType::CreateReview => "create_review",
            ActionType::SendMessage => "send_message",
        }
    }
}

#[derive(Debug, Clone)]
pub struct RateLimitResult {
    pub allowed: bool,
    pub remaining: i32,
    pub reset_at: chrono::DateTime<Utc>,
    pub retry_after: u64, // seconds
}

impl RateLimitResult {
    /// Add rate limit headers to HTTP response
    pub fn to_headers(&self) -> Vec<(&'static str, String)> {
        vec![
            ("X-RateLimit-Limit", self.remaining.to_string()),
            ("X-RateLimit-Remaining", self.remaining.to_string()),
            ("X-RateLimit-Reset", self.reset_at.timestamp().to_string()),
            ("Retry-After", self.retry_after.to_string()),
        ]
    }
}
