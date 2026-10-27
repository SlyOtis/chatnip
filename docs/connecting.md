# Connecting Microsoft 365, Google and Slack

Each service needs one app registration that says "this is Chatnip". The registration gives no access by itself. Every person still signs in as themselves in the browser, and their messages and tokens stay on their own Mac, in their own profile.

You create these registrations by hand in each service's console. Chatnip never creates anything in the cloud.

When a registration is ready, paste its values into Chatnip under **Settings → Connections → App registrations**.

## Microsoft 365 (Outlook, Teams, calendars)

**Create the app**

1. In the Microsoft Entra admin center, go to App registrations and choose New registration.
2. Name it Chatnip and pick "Accounts in any organizational directory" (multitenant).
3. Add the redirect `http://localhost:48731/callback/microsoft` with the platform "Mobile and desktop applications".
4. Copy the Application (client) ID into Chatnip. No client secret is needed.

**Permissions**

Add these Microsoft Graph delegated permissions:

- `User.Read`, `User.ReadBasic.All`, `People.Read`, `offline_access`
- `Mail.ReadWrite`, `Mail.Send`
- `Chat.ReadWrite`, `ChannelMessage.Send`, `Team.ReadBasic.All`, `Channel.ReadBasic.All`
- `Calendars.ReadWrite`, `OnlineMeetings.ReadWrite`
- `Presence.Read.All`, `Presence.ReadWrite`
- `Files.ReadWrite`, `Files.Read.All`
- `ChannelMessage.Read.All` and `ChannelMessage.ReadWrite`, which need an administrator and are only used when "Teams channels" is switched on

What each one is for:
- `User.ReadBasic.All` reads the names and photos of the people you chat with, and `People.Read` the people you work with most.
- `Presence.ReadWrite` sets your status message from Chatnip.
- `Files.ReadWrite` uploads files you send in a chat to your OneDrive, the way Teams does, and `Files.Read.All` opens files others share with you.
- `ChannelMessage.ReadWrite` edits and deletes your own channel messages.

**Other organisations**

Many organisations only let an administrator approve new apps. Chatnip shows an "Administrator approval" link for this. The administrator opens it once, and after that everyone in that organisation signs in on their own.

To avoid the "unverified" warning, verify the publisher. That needs a Microsoft Partner ID and a domain you own, set under Branding and properties.

**Who can sign in**

The tenant setting decides this:
- `organizations` (default): any work or school account.
- `common`: also personal accounts, which have no Teams.
- A tenant ID or domain: only that one organisation.

## Google (Gmail, Calendar)

1. In the Google Cloud console, create a project and enable the Gmail API and the Google Calendar API.
2. Set up the consent screen. Choose Internal for your own Workspace only, or External for anyone.
3. Enable the People API too, and add the scopes `gmail.modify`, `calendar`, `userinfo.email`, `userinfo.profile`, `contacts.readonly` and `contacts.other.readonly`. The two contacts scopes read your contacts and the people you have written to, with their photos.
4. Create a client of type Desktop app and copy its client ID and secret into Chatnip.

For an External app, Google limits you to 100 test users until the app is verified. Their sign-in expires after 7 days.

Gmail access is "restricted", so opening it to everyone needs two things:
- Google's app verification.
- A yearly paid security assessment.

Plan for this to take weeks. Internal apps skip all of it.

Google Chat is not connected yet. Only the test client imitates it.

## Slack (channels, direct messages)

1. At api.slack.com/apps, choose Create New App, then From a manifest, and paste `docs/slack-manifest.json`.
   The manifest asks for `emoji:read` for the workspace's own emoji, `pins:read` and `pins:write` for pinned messages, and `users.profile:write` for your status.
2. Copy the Client ID into Chatnip. No secret is needed.
3. To use it in more than one workspace, turn on public distribution under Manage Distribution. No App Directory listing is needed.

Chatnip signs in as the person with a user token, never as a bot. The token renews itself. If Chatnip isn't opened for 30 days, connect again.

Without a registration, a person can still paste their own `xoxp-` token behind the key button.

## When the app asks for more

When Chatnip starts to use a new permission, connections made before it show **Needs consent again** under Settings → Accounts, with the permissions that are missing. Add them to the app registration first, then sign in again from there. Connections keep working in the meantime; only the new parts wait.

This release added:
- Microsoft 365: `User.ReadBasic.All`, `People.Read`, `Presence.ReadWrite`, `Files.ReadWrite`, `Files.Read.All`, and for administrators `ChannelMessage.ReadWrite`.
- Google: `contacts.readonly` and `contacts.other.readonly`, and the People API.
- Slack: `emoji:read`, `pins:read`, `pins:write` and `users.profile:write`. Update the app from the manifest, then reinstall it to the workspace.

A Slack token pasted by hand carries no scope list, so Chatnip cannot tell what it lacks.

## Good to know

- Port 48731 on this Mac must be free while connecting.
- Values can also come from `.env` on first start. See `.env.example`.
- Client secrets are kept in the macOS Keychain, not in the database.
