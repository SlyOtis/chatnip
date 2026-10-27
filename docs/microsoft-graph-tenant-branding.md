# Microsoft Graph Tenant Branding API Documentation

## Overview
Microsoft Graph API provides endpoints to fetch organizational branding assets including logos and backgrounds. This document consolidates the correct endpoints and implementation details.

## API Endpoints (v1.0 - Stable)

### Get Organization Branding
```http
GET https://graph.microsoft.com/v1.0/organization/{organization-id}/branding
```

### Get Specific Logo Images
To retrieve Stream-type properties (actual image data), use the localization endpoint:

#### Square Logo (Main Organization Logo)
```http
GET https://graph.microsoft.com/v1.0/organization/{organization-id}/branding/localizations/default/squareLogo
```

#### Banner Logo  
```http
GET https://graph.microsoft.com/v1.0/organization/{organization-id}/branding/localizations/default/bannerLogo
```

#### Square Logo Dark Mode
```http
GET https://graph.microsoft.com/v1.0/organization/{organization-id}/branding/localizations/default/squareLogoDark
```

## Logo Specifications

### Square Logo
- **Format**: PNG or JPEG
- **Max dimensions**: 240 x 240 pixels
- **Max file size**: 10 KB
- **Usage**: Windows 10 OOBE, Windows Autopilot, Teams avatar
- **Recommendation**: Transparent background with no padding

### Banner Logo
- **Format**: PNG or JPEG  
- **Max dimensions**: 36 x 245 pixels
- **Usage**: Sign-in page header
- **Recommendation**: Transparent background with no padding

## Required Permissions

### Read Operations
- **Delegated (work/school)**: `User.Read` or `OrganizationalBranding.Read.All`
- **Application**: `OrganizationalBranding.Read.All`
- **Roles**: Global Reader (least privileged)

### Update Operations
- **Delegated (work/school)**: `OrganizationalBranding.ReadWrite.All`
- **Application**: `OrganizationalBranding.ReadWrite.All`
- **Roles**: Organizational Branding Administrator (least privileged)

## Licensing Requirements
Custom branding requires one of:
- Microsoft Entra ID P1 or P2
- Microsoft Entra ID Premium 2
- Office 365 (for Office 365 apps)

## Implementation in Chatnip

### Current Implementation (needs update)
```rust
// Currently using beta endpoint - should be updated
let logo_url = format!(
    "https://graph.microsoft.com/beta/organization/{}/branding/squareLogo",
    tenant_id
);
```

### Recommended Implementation
```rust
// Use v1.0 stable endpoint with localizations
let logo_url = format!(
    "https://graph.microsoft.com/v1.0/organization/{}/branding/localizations/default/squareLogo",
    tenant_id
);
```

### Fetching Organization Info First
```rust
// Step 1: Get organization details including tenant ID
let org_response = client
    .get("https://graph.microsoft.com/v1.0/organization")
    .bearer_auth(&access_token)
    .send()
    .await?;

let org_data = org_response.json::<Value>().await?;
let tenant_id = org_data["value"][0]["id"].as_str().unwrap();

// Step 2: Fetch the square logo
let logo_response = client
    .get(&format!(
        "https://graph.microsoft.com/v1.0/organization/{}/branding/localizations/default/squareLogo",
        tenant_id
    ))
    .bearer_auth(&access_token)
    .send()
    .await?;

// Step 3: Save logo bytes if successful
if logo_response.status().is_success() {
    let logo_bytes = logo_response.bytes().await?;
    // Save to assets directory
}
```

## Error Handling

### Common Response Codes
- **200 OK**: Logo exists and returned
- **204 No Content**: Endpoint valid but no logo configured
- **404 Not Found**: No branding configured for organization
- **401 Unauthorized**: Invalid or expired token
- **403 Forbidden**: Insufficient permissions

### Fallback Strategy
1. Try to fetch tenant square logo
2. If not available, try banner logo
3. If no branding configured, use default Microsoft/Teams icon based on conversation type

## Testing Endpoints

Use Microsoft Graph Explorer to test:
1. Sign in at https://developer.microsoft.com/graph/graph-explorer
2. Ensure proper permissions granted
3. Test endpoints with your tenant ID

## Notes
- The `/beta` endpoint should not be used in production
- Always use `/v1.0` for stable, production-ready features
- Logo URLs from the branding properties (like `squareLogoRelativeUrl`) point to CDN-hosted versions
- Direct logo fetch returns binary image data, not JSON