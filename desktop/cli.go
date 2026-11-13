package main

import "slices"

const commandLineHelp = `Usage: meron [options] [mailto:URL | file.eml ...]

Options:
  -h, --help    Show this help and exit.
  --background  Start with the main window hidden and keep syncing mail.

Open the window from the tray icon or launch Meron without --background.
`

func cliOptionRequested(args []string, options ...string) bool {
	for _, arg := range args {
		if arg == "--" {
			break
		}
		if slices.Contains(options, arg) {
			return true
		}
	}
	return false
}

func backgroundRequested(args []string) bool {
	// An explicit request to compose mail must open a visible window, just
	// as it does when delivered to an already running instance.
	return cliOptionRequested(args, "--background") && len(mailtoURLs(args)) == 0
}
