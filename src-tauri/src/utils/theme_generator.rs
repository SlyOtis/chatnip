use serde::{Deserialize, Serialize};
use sha2::{Sha256, Digest};
use sqlx::Row;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderTheme {
    pub id: String,
    pub provider_id: String,
    pub theme_name: String,
    pub primary_50: String,
    pub primary_100: String,
    pub primary_200: String,
    pub primary_300: String,
    pub primary_400: String,
    pub primary_500: String,
    pub primary_600: String,
    pub primary_700: String,
    pub primary_800: String,
    pub primary_900: String,
    pub primary_950: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone)]
pub struct ThemeColor {
    pub h: f32,
    pub s: f32,
    pub l: f32,
}

impl ThemeColor {
    pub fn new(h: f32, s: f32, l: f32) -> Self {
        Self { h, s, l }
    }
    
    pub fn to_hex(&self) -> String {
        let (r, g, b) = hsl_to_rgb(self.h, self.s, self.l);
        format!("#{:02x}{:02x}{:02x}", r, g, b)
    }
}

fn hsl_to_rgb(h: f32, s: f32, l: f32) -> (u8, u8, u8) {
    let c = (1.0 - (2.0 * l - 1.0).abs()) * s;
    let x = c * (1.0 - ((h / 60.0) % 2.0 - 1.0).abs());
    let m = l - c / 2.0;
    
    let (r, g, b) = match (h / 60.0) as u8 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        5 => (c, 0.0, x),
        _ => (0.0, 0.0, 0.0),
    };
    
    (
        ((r + m) * 255.0) as u8,
        ((g + m) * 255.0) as u8,
        ((b + m) * 255.0) as u8,
    )
}

pub fn generate_color_palette(base_color: ThemeColor) -> Vec<String> {
    vec![
        ThemeColor::new(base_color.h, base_color.s * 0.3, 0.98).to_hex(),  // 50
        ThemeColor::new(base_color.h, base_color.s * 0.4, 0.95).to_hex(),  // 100
        ThemeColor::new(base_color.h, base_color.s * 0.5, 0.90).to_hex(),  // 200
        ThemeColor::new(base_color.h, base_color.s * 0.6, 0.80).to_hex(),  // 300
        ThemeColor::new(base_color.h, base_color.s * 0.7, 0.70).to_hex(),  // 400
        ThemeColor::new(base_color.h, base_color.s * 0.8, 0.50).to_hex(),  // 500
        ThemeColor::new(base_color.h, base_color.s * 0.9, 0.40).to_hex(),  // 600
        ThemeColor::new(base_color.h, base_color.s * 1.0, 0.30).to_hex(),  // 700
        ThemeColor::new(base_color.h, base_color.s * 1.0, 0.20).to_hex(),  // 800
        ThemeColor::new(base_color.h, base_color.s * 1.0, 0.15).to_hex(),  // 900
        ThemeColor::new(base_color.h, base_color.s * 1.0, 0.10).to_hex(),  // 950
    ]
}

pub fn generate_provider_theme(provider_id: &str, profile_id: &str, existing_themes_count: usize) -> ProviderTheme {
    let mut hasher = Sha256::new();
    hasher.update(provider_id);
    hasher.update(profile_id);
    let hash = hasher.finalize();
    let hash_value = u32::from_be_bytes([hash[0], hash[1], hash[2], hash[3]]);
    
    let base_hues = [
        210.0,  // Blue
        280.0,  // Purple
        340.0,  // Pink
        30.0,   // Orange
        160.0,  // Cyan
        130.0,  // Green
        60.0,   // Yellow
        0.0,    // Red
    ];
    
    let hue_offset = (existing_themes_count % base_hues.len()) as usize;
    let base_hue = base_hues[hue_offset];
    
    let hue_variation = (hash_value % 60) as f32 - 30.0;
    let final_hue = (base_hue + hue_variation).rem_euclid(360.0);
    
    let saturation = 0.7 + ((hash_value / 60) % 30) as f32 / 100.0;
    
    let base_color = ThemeColor::new(final_hue, saturation, 0.5);
    let palette = generate_color_palette(base_color);
    
    let now = chrono::Utc::now().to_rfc3339();
    
    ProviderTheme {
        id: uuid::Uuid::new_v4().to_string(),
        provider_id: provider_id.to_string(),
        theme_name: format!("Provider Theme {}", existing_themes_count + 1),
        primary_50: palette[0].clone(),
        primary_100: palette[1].clone(),
        primary_200: palette[2].clone(),
        primary_300: palette[3].clone(),
        primary_400: palette[4].clone(),
        primary_500: palette[5].clone(),
        primary_600: palette[6].clone(),
        primary_700: palette[7].clone(),
        primary_800: palette[8].clone(),
        primary_900: palette[9].clone(),
        primary_950: palette[10].clone(),
        created_at: now.clone(),
        updated_at: now,
    }
}

pub async fn create_provider_theme(
    pool: &sqlx::Pool<sqlx::Sqlite>,
    provider_id: &str,
    profile_id: &str,
) -> Result<ProviderTheme, String> {
    let existing_themes_count = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM provider_theme WHERE provider_id IN (SELECT id FROM provider WHERE profile_id = ?1)"
    )
    .bind(profile_id)
    .fetch_one(pool)
    .await
    .map_err(|e| format!("Failed to count existing themes: {}", e))?;
    
    let theme = generate_provider_theme(provider_id, profile_id, existing_themes_count as usize);
    
    sqlx::query(
        r#"INSERT OR REPLACE INTO provider_theme (
            id, provider_id, theme_name,
            primary_50, primary_100, primary_200, primary_300, primary_400,
            primary_500, primary_600, primary_700, primary_800, primary_900, primary_950,
            created_at, updated_at
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16)"#
    )
    .bind(&theme.id)
    .bind(&theme.provider_id)
    .bind(&theme.theme_name)
    .bind(&theme.primary_50)
    .bind(&theme.primary_100)
    .bind(&theme.primary_200)
    .bind(&theme.primary_300)
    .bind(&theme.primary_400)
    .bind(&theme.primary_500)
    .bind(&theme.primary_600)
    .bind(&theme.primary_700)
    .bind(&theme.primary_800)
    .bind(&theme.primary_900)
    .bind(&theme.primary_950)
    .bind(&theme.created_at)
    .bind(&theme.updated_at)
    .execute(pool)
    .await
    .map_err(|e| format!("Failed to insert provider theme: {}", e))?;
    
    Ok(theme)
}

pub async fn get_or_create_provider_theme(
    pool: &sqlx::Pool<sqlx::Sqlite>,
    provider_id: &str,
    profile_id: &str,
) -> Result<ProviderTheme, String> {
    if let Some(theme) = get_provider_theme(pool, provider_id).await? {
        Ok(theme)
    } else {
        create_provider_theme(pool, provider_id, profile_id).await
    }
}

pub async fn get_provider_theme(
    pool: &sqlx::Pool<sqlx::Sqlite>,
    provider_id: &str,
) -> Result<Option<ProviderTheme>, String> {
    let row = sqlx::query(
        "SELECT * FROM provider_theme WHERE provider_id = ?1"
    )
    .bind(provider_id)
    .fetch_optional(pool)
    .await
    .map_err(|e| format!("Failed to get provider theme: {}", e))?;
    
    if let Some(row) = row {
        Ok(Some(ProviderTheme {
            id: row.get("id"),
            provider_id: row.get("provider_id"),
            theme_name: row.get("theme_name"),
            primary_50: row.get("primary_50"),
            primary_100: row.get("primary_100"),
            primary_200: row.get("primary_200"),
            primary_300: row.get("primary_300"),
            primary_400: row.get("primary_400"),
            primary_500: row.get("primary_500"),
            primary_600: row.get("primary_600"),
            primary_700: row.get("primary_700"),
            primary_800: row.get("primary_800"),
            primary_900: row.get("primary_900"),
            primary_950: row.get("primary_950"),
            created_at: row.get("created_at"),
            updated_at: row.get("updated_at"),
        }))
    } else {
        Ok(None)
    }
}