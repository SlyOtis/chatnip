# Microsoft Graph API Reference

## Overview

Microsoft Graph is a RESTful web API that enables access to Microsoft 365 services through a single endpoint: `https://graph.microsoft.com`. It provides a unified programmability model to access data in Microsoft Entra ID, Microsoft 365, Windows, and Enterprise Mobility + Security.

## Authentication & Authorization

### Core Concepts

#### Access Scenarios
1. **Delegated Access** - App acts on behalf of a signed-in user
   - Uses delegated permissions (scopes)
   - Limited by both app permissions AND user permissions
   - Required for `/me` endpoints

2. **App-Only Access** - App acts with its own identity
   - Uses application permissions (app roles)
   - No user context required
   - Used for background services, automation

#### Permission Types
- **Delegated Permissions (Scopes)**: App acts on behalf of user
- **Application Permissions (App Roles)**: App acts independently

### OAuth 2.0 Flows

#### Authorization Code Flow (Most Common)
1. Request authorization: `https://login.microsoftonline.com/{tenant}/oauth2/v2.0/authorize`
2. Exchange code for token: `https://login.microsoftonline.com/{tenant}/oauth2/v2.0/token`
3. Use access token in Authorization header: `Bearer {token}`

#### Client Credentials Flow (App-Only)
- Direct token request with client_id and client_secret
- No user interaction required

### Access Token Structure
```http
GET https://graph.microsoft.com/v1.0/me
Authorization: Bearer {access_token}
```

## Common Permissions

### User Permissions
- `User.Read` - Read signed-in user's profile
- `User.ReadWrite` - Read and update signed-in user's profile
- `User.Read.All` - Read all users' profiles (app-only)
- `User.ReadWrite.All` - Read and write all users' profiles (app-only)

### Mail Permissions
- `Mail.Read` - Read user's mail
- `Mail.ReadWrite` - Read and write user's mail
- `Mail.Send` - Send mail as the user

### Calendar Permissions
- `Calendars.Read` - Read user's calendars
- `Calendars.ReadWrite` - Read and write user's calendars

### Files Permissions
- `Files.Read` - Read user's files
- `Files.ReadWrite` - Read and write user's files
- `Files.Read.All` - Read all files user can access

### Teams Permissions
- `Team.ReadBasic.All` - Read teams' names and descriptions
- `TeamSettings.Read.All` - Read all teams' settings
- `TeamsTab.Create` - Create tabs in teams

### Groups Permissions
- `Group.Read.All` - Read all groups
- `Group.ReadWrite.All` - Read and write all groups
- `GroupMember.Read.All` - Read group memberships

## Core API Endpoints

### User Operations
```http
GET /v1.0/me                              # Get current user
GET /v1.0/users                           # List all users
GET /v1.0/users/{id or userPrincipalName} # Get specific user
GET /v1.0/users/{id}/photo/$value         # Get user photo
GET /v1.0/users/{id}/manager              # Get user's manager
GET /v1.0/users/{id}/directReports        # Get direct reports
PATCH /v1.0/users/{id}                    # Update user
DELETE /v1.0/users/{id}                   # Delete user
```

### Mail Operations
```http
GET /v1.0/me/messages                     # List messages
GET /v1.0/me/messages/{id}                # Get specific message
POST /v1.0/me/sendMail                    # Send mail
POST /v1.0/me/messages                    # Create draft
DELETE /v1.0/me/messages/{id}             # Delete message
GET /v1.0/me/mailFolders                  # List mail folders
POST /v1.0/me/messages/{id}/send          # Send draft
```

### Calendar Operations
```http
GET /v1.0/me/calendar                     # Get calendar
GET /v1.0/me/calendars                    # List calendars
GET /v1.0/me/events                       # List events
POST /v1.0/me/events                      # Create event
PATCH /v1.0/me/events/{id}                # Update event
DELETE /v1.0/me/events/{id}               # Delete event
POST /v1.0/me/events/{id}/cancel          # Cancel event
POST /v1.0/me/findMeetingTimes            # Find meeting times
```

### OneDrive & Files
```http
GET /v1.0/me/drive                        # Get user's drive
GET /v1.0/me/drive/root                   # Get root folder
GET /v1.0/me/drive/root/children          # List root contents
GET /v1.0/drives/{drive-id}/items/{item-id} # Get item
PUT /v1.0/me/drive/root:/{filename}:/content # Upload file
POST /v1.0/me/drive/items/{item-id}/copy  # Copy item
DELETE /v1.0/me/drive/items/{item-id}     # Delete item
GET /v1.0/me/drive/items/{item-id}/content # Download file
POST /v1.0/me/drive/root/children         # Create folder
```

### Teams Operations
```http
GET /v1.0/me/joinedTeams                  # List user's teams
GET /v1.0/teams/{id}                      # Get team
POST /v1.0/teams                          # Create team
PATCH /v1.0/teams/{id}                    # Update team
DELETE /v1.0/teams/{id}                   # Delete team
GET /v1.0/teams/{id}/channels             # List channels
POST /v1.0/teams/{id}/channels            # Create channel
GET /v1.0/teams/{id}/members              # List members
POST /v1.0/teams/{id}/members             # Add member
```

### Groups Operations
```http
GET /v1.0/groups                          # List groups
GET /v1.0/groups/{id}                     # Get group
POST /v1.0/groups                         # Create group
PATCH /v1.0/groups/{id}                   # Update group
DELETE /v1.0/groups/{id}                  # Delete group
GET /v1.0/groups/{id}/members             # List members
POST /v1.0/groups/{id}/members/$ref       # Add member
GET /v1.0/groups/{id}/owners              # List owners
```

### Chat & Messages
```http
GET /v1.0/me/chats                        # List chats
GET /v1.0/chats/{id}                      # Get chat
POST /v1.0/chats                          # Create chat
GET /v1.0/chats/{id}/messages             # List messages
POST /v1.0/chats/{id}/messages            # Send message
GET /v1.0/teams/{id}/channels/{id}/messages # Channel messages
```

## Query Parameters

### Common Parameters
- `$select` - Select specific fields: `?$select=id,displayName,mail`
- `$filter` - Filter results: `?$filter=startswith(displayName,'A')`
- `$orderby` - Sort results: `?$orderby=displayName desc`
- `$top` - Limit results: `?$top=10`
- `$skip` - Skip results: `?$skip=20`
- `$count` - Include count: `?$count=true`
- `$expand` - Expand relationships: `?$expand=manager`
- `$search` - Search query: `?$search="displayName:john"`

### Filter Examples
```http
GET /v1.0/users?$filter=accountEnabled eq true
GET /v1.0/users?$filter=startswith(displayName,'A')
GET /v1.0/messages?$filter=from/emailAddress/address eq 'user@example.com'
GET /v1.0/events?$filter=start/dateTime ge '2024-01-01'
```

## Request Headers

### Required Headers
```http
Authorization: Bearer {access_token}
Content-Type: application/json
```

### Optional Headers
```http
Prefer: outlook.timezone="Pacific Standard Time"
Prefer: outlook.body-content-type="text"
ConsistencyLevel: eventual  # Required for advanced queries
```

## Response Format

### Success Response
```json
{
  "@odata.context": "https://graph.microsoft.com/v1.0/$metadata#users/$entity",
  "id": "87d349ed-44d7-43e1-9a83-5f2406dee5bd",
  "businessPhones": ["+1 425 555 0109"],
  "displayName": "Adele Vance",
  "givenName": "Adele",
  "surname": "Vance",
  "mail": "AdeleV@contoso.com",
  "userPrincipalName": "AdeleV@contoso.com"
}
```

### Collection Response
```json
{
  "@odata.context": "https://graph.microsoft.com/v1.0/$metadata#users",
  "@odata.nextLink": "https://graph.microsoft.com/v1.0/users?$skip=10",
  "value": [
    {
      "id": "87d349ed-44d7-43e1-9a83-5f2406dee5bd",
      "displayName": "Adele Vance"
    }
  ]
}
```

### Error Response
```json
{
  "error": {
    "code": "Request_ResourceNotFound",
    "message": "Resource 'user@example.com' does not exist",
    "innerError": {
      "date": "2024-01-01T00:00:00",
      "request-id": "guid",
      "client-request-id": "guid"
    }
  }
}
```

## Common Error Codes

- `InvalidAuthenticationToken` - Token is invalid or expired
- `InsufficientPrivileges` - Missing required permissions
- `Request_ResourceNotFound` - Resource doesn't exist
- `Request_BadRequest` - Invalid request format
- `Request_Throttled` - Too many requests
- `InvalidRequest` - Request is malformed
- `AuthenticationError` - Authentication failed

## Pagination

For large result sets, use `@odata.nextLink`:
```http
GET https://graph.microsoft.com/v1.0/users?$top=5

Response:
{
  "@odata.nextLink": "https://graph.microsoft.com/v1.0/users?$skip=5&$top=5",
  "value": [...]
}
```

## Delta Queries

Track changes over time:
```http
GET /v1.0/users/delta
GET /v1.0/users/delta?$deltatoken={token}
```

## Batch Requests

Send multiple requests in one call:
```http
POST https://graph.microsoft.com/v1.0/$batch
Content-Type: application/json

{
  "requests": [
    {
      "id": "1",
      "method": "GET",
      "url": "/me"
    },
    {
      "id": "2",
      "method": "GET",
      "url": "/me/messages"
    }
  ]
}
```

## Change Notifications (Webhooks)

Subscribe to changes:
```http
POST /v1.0/subscriptions
{
  "changeType": "created,updated",
  "notificationUrl": "https://webhook.example.com/api",
  "resource": "/me/messages",
  "expirationDateTime": "2024-12-31T00:00:00Z",
  "clientState": "secret"
}
```

## SDK Support

### Available SDKs
- .NET / C#
- JavaScript/TypeScript
- Java
- Python
- PHP
- Go
- PowerShell

### Example (JavaScript)
```javascript
const { Client } = require('@microsoft/microsoft-graph-client');

const client = Client.init({
  authProvider: (done) => {
    done(null, accessToken);
  }
});

const user = await client.api('/me').get();
```

## Best Practices

1. **Use least privilege** - Request only necessary permissions
2. **Handle throttling** - Implement exponential backoff
3. **Use select** - Request only needed fields
4. **Batch requests** - Combine multiple calls when possible
5. **Cache tokens** - Don't request new tokens for each call
6. **Use delta queries** - For tracking changes efficiently
7. **Handle errors gracefully** - Implement proper error handling
8. **Use SDK when available** - Avoid raw HTTP when possible

## Rate Limits

- **Per app across all tenants**: 2000 requests per second
- **Per app per tenant**: Service-specific limits apply
- **Throttling response**: HTTP 429 with Retry-After header

## Additional Resources

- [Microsoft Graph Explorer](https://developer.microsoft.com/graph/graph-explorer)
- [Permissions Reference](https://learn.microsoft.com/graph/permissions-reference)
- [API Reference](https://learn.microsoft.com/graph/api/overview)
- [Authentication Docs](https://learn.microsoft.com/graph/auth/)
- [SDK Documentation](https://learn.microsoft.com/graph/sdks/sdks-overview)