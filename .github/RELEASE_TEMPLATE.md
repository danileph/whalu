Download **Whalu-macOS-universal.zip** below. It runs on Apple silicon and Intel Macs with macOS 15 or later.

1. Open the ZIP and move Whalu to Applications. Quit an existing Whalu installation before replacing it.
2. Open Whalu. This build is ad-hoc signed and is not notarized by Apple. If macOS blocks it, review [Apple's instructions for opening an unnotarized app](https://support.apple.com/102445).
3. Follow Whalu's setup to allow Accessibility for window switching and Screen & System Audio Recording for previews. Whalu does not record audio. After an update, macOS may require you to remove and re-add Whalu's permission entries.

The `.sha256` file contains the archive checksum. Verify it with `shasum -a 256 -c Whalu-macOS-universal.zip.sha256` in the download folder.
