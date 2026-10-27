cask "chatnip" do
  version "0.1.0"
  sha256 "0000000000000000000000000000000000000000000000000000000000000000"

  url "https://github.com/SlyOtis/chatnip/releases/download/v#{version}/Chatnip_#{version}_universal.dmg"
  name "Chatnip"
  desc "Every inbox, one engine: Teams, Outlook, Gmail, Slack, IMAP and calendars"
  homepage "https://github.com/SlyOtis/chatnip"

  livecheck do
    url :url
    strategy :github_latest
  end

  depends_on macos: ">= :big_sur"

  app "Chatnip.app"
  binary "#{appdir}/Chatnip.app/Contents/MacOS/chatnip"
  binary "#{appdir}/Chatnip.app/Contents/MacOS/chatnip", target: "cnip"

  zap trash: [
    "~/.chatnip",
    "~/Library/Application Support/com.slyotis.chatnip",
    "~/Library/Caches/com.slyotis.chatnip",
    "~/Library/Preferences/com.slyotis.chatnip.plist",
    "~/Library/Saved Application State/com.slyotis.chatnip.savedState",
    "~/Library/WebKit/com.slyotis.chatnip",
  ]
end
