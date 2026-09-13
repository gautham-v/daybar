# The Homebrew cask for daybar.
#
# The template for the tap. On each `v*` tag `.github/workflows/release.yml`
# builds the universal .app, fills `version` and `sha256` in below, and pushes
# the result to github.com/gautham-v/homebrew-tap as `Casks/daybar.rb`;
# users install with `brew install --cask gautham-v/tap/daybar`.
# The values here are the last release's and need no hand edits.
cask "daybar" do
  version "0.1.0"
  sha256 "0000000000000000000000000000000000000000000000000000000000000000"

  url "https://github.com/gautham-v/daybar/releases/download/v#{version}/Daybar-#{version}.zip"
  name "Daybar"
  desc "Calendar in the menu bar, read from Calendar.app"
  homepage "https://github.com/gautham-v/daybar"

  # SMAppService, which launch-at-login uses, is macOS 13+.
  depends_on macos: :ventura

  app "Daybar.app"

  caveats <<~EOS
    On first launch macOS asks for calendar access; Daybar shows no events
    until it is granted. Change it later under:

      System Settings → Privacy & Security → Calendars

    Turn on "Launch at login" from the ··· menu if you want it.
  EOS

  uninstall quit: "com.gauthamv.daybar"
end
